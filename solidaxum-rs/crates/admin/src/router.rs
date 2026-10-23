//! 应用路由组装。
//!
//! [`build_router`] 把所有路由组(public/system/flow/monitor/job)按与生产
//! 完全一致的层次组装成一个 `Router`,供 `main.rs` 与集成测试共用。
//! 所有端点(含 /health、/metrics)统一在 `/api` 前缀下。
//!
//! 注意:依赖 Redis 的全局中间件(限流/防重提交/XSS)以及静态资源托管 **不**
//! 在此处应用 —— 它们由 `main.rs` 叠加。这样测试可以驱动真实的业务路由
//! 而无需 Redis。

use std::{any::Any, sync::Arc};

use app_core::vo::response::R;
use axum::{
    Router, http::StatusCode, middleware::from_fn_with_state,
    response::IntoResponse, routing::delete,
};
use flow::{
    handler::FlowState,
    resolver::{BusinessStatusCallback, HandlerSelectService, UserResolver},
    route::flow_route::{
        designer_api_route, designer_public_route, flow_route,
    },
};
use job::{
    job::{handler::JobHandlerState, route::job_route},
    joblog::{handler::JobLogHandlerState, route::job_log_route},
};
use monitor::{
    cache::route::cache_route, logininfo::route::login_info_route,
    middleware::oper_log::OperLogLayer, online::route::online_user_route,
    operlog::route::oper_log_route,
};
use system::{
    app::AppState,
    assignee_sync::route::assignee_sync_route::assignee_sync_route,
    client::route::client_route::client_route,
    codegen::route::gen_route::gen_route,
    config::route::config_route::config_route,
    dept::route::dept_route::dept_route,
    dict::route::{
        dict_data_route::dict_data_route, dict_type_route::dict_type_route,
    },
    menu::route::menu_route::menu_route,
    message::route::message_route::message_route,
    middleware::auth::auth,
    notice::route::notice_route::notice_route,
    oss::route::oss_route::{oss_public_route, oss_route},
    post::route::post_route::post_route,
    profile::route::profile_route::profile_route,
    push::route::push_route::push_route,
    role::route::role_route::role_route,
    social::route::social_route::social_route,
    user::route::user_route::user_route,
};
use tower_http::catch_panic::CatchPanicLayer;

use crate::{
    flow_notifier::arc_notifier,
    flow_resolver::{
        LeaveStatusCallback, SystemHandlerSelectService, SystemUserResolver,
    },
};

/// 鉴权模式。
///
/// 生产始终使用 [`AuthMode::Real`](AuthMode::Real)(校验 Redis 会话)。
/// [`AuthMode::Test`](AuthMode::Test) 仅在启用 `test-util` feature 时存在,
/// 使用 [`testkit::test_auth`](crate::testkit::test_auth) 从请求头注入
/// `LoginUser`,绕过 Redis。
#[derive(Clone, Copy, Debug)]
pub enum AuthMode {
    Real,
    #[cfg(feature = "test-util")]
    Test,
}

/// 按所选鉴权模式给某个路由组挂上鉴权 `route_layer`。
fn apply_auth<S>(
    router: Router<S>,
    app_state: Arc<AppState>,
    mode: AuthMode,
) -> Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    match mode {
        AuthMode::Real => {
            router.route_layer(from_fn_with_state(app_state, auth))
        }
        #[cfg(feature = "test-util")]
        AuthMode::Test => router.route_layer(from_fn_with_state(
            app_state,
            crate::testkit::test_auth,
        )),
    }
}

/// panic 兜底:把 handler panic 变成 500 + 项目标准 `{code, msg}` 信封。
///
/// 不兜底时 hyper 直接丢连接,客户端拿到连接重置而非可读错误,访问日志里
/// 也没有 500 —— 现场排障时线索全无。
///
/// panic 原文**只进日志**:它常带内部路径、SQL 片段、字段名,回给客户端
/// 等于白送情报。
fn panic_response(
    err: Box<dyn Any + Send + 'static>,
) -> axum::response::Response {
    let detail = err
        .downcast_ref::<String>()
        .map(String::as_str)
        .or_else(|| err.downcast_ref::<&'static str>().copied())
        .unwrap_or("<非字符串 panic 载荷>");
    tracing::error!(panic = detail, "请求处理 panic,已兜底为 500");

    let status = StatusCode::INTERNAL_SERVER_ERROR;
    let body = R::<()>::error_with_code_and_message(
        status.as_u16() as i32,
        "服务器内部错误".to_string(),
    );
    (status, body).into_response()
}

/// 组装完整应用路由(不含依赖 Redis 的全局中间件与静态托管)。
///
/// 层次顺序与 `main.rs` 保持一致:
/// `handler ← oper_log_layer ← auth_middleware`,请求流为
/// `auth → oper_log → handler → oper_log → auth`。
pub fn build_router(
    app_state: Arc<AppState>,
    monitor_state: Arc<monitor::OperLogState>,
    job_state: Arc<job::JobState>,
    oss_upload_body_limit: usize,
    mode: AuthMode,
) -> Router {
    let oper_log_layer = OperLogLayer::new(
        monitor_state.services.oper_log_service.clone(),
        monitor_state.title_rules,
    );

    // 公开路由 (不经过 auth 中间件)
    let auth_public = Router::new()
        .route(
            "/auth/code",
            axum::routing::get(crate::auth::handler::auth_handler::get_captcha),
        )
        .route(
            "/auth/login",
            axum::routing::post(crate::auth::handler::auth_handler::login),
        )
        .route(
            "/auth/logout",
            axum::routing::post(crate::auth::handler::auth_handler::logout),
        )
        .route(
            "/auth/register",
            axum::routing::post(crate::auth::handler::auth_handler::register),
        )
        .route(
            "/auth/{source}/callback",
            axum::routing::get(
                crate::auth::handler::auth_handler::social_callback_stub,
            ),
        )
        .route(
            "/auth/{source}/bind",
            axum::routing::post(
                crate::auth::handler::auth_handler::social_bind_stub,
            ),
        )
        .layer(oper_log_layer.clone())
        .with_state((app_state.clone(), monitor_state.clone()));

    // Flow 路由(三 adapter 与 FlowState 统一持 infra.data_source() ——
    // 装配后带 resolver 的同一 Arc;adapter 查询全显式豁免,不解析
    // resolver,语义不受快照影响,见 flow_resolver.rs 文件头裁定)
    let resolver: Arc<dyn UserResolver> =
        Arc::new(SystemUserResolver::new(app_state.infra.data_source()));
    let business_callback: Arc<dyn BusinessStatusCallback> =
        Arc::new(LeaveStatusCallback::new(app_state.infra.data_source()));
    let flow_notifier = arc_notifier(app_state.services.push_hub.clone());
    let handler_select: Arc<dyn HandlerSelectService> = Arc::new(
        SystemHandlerSelectService::new(app_state.infra.data_source()),
    );
    let mut flow_state = FlowState::with_resolver_callback_and_notifier(
        app_state.infra.data_source(),
        Arc::new(flow::listener::ListenerRegistry::new()),
        Arc::new(flow::permission::DefaultPermissionHandler),
        resolver,
        business_callback,
        flow_notifier,
    );
    flow_state.handler_select = handler_select;
    let flow_state = Arc::new(flow_state);
    let flow_routes = apply_auth(
        flow_route().layer(oper_log_layer.clone()),
        app_state.clone(),
        mode,
    )
    .with_state(flow_state.clone());

    // warm-flow 设计器公开面:静态资源 + /warm-flow-ui/config,公开访问,
    // 不经过 auth(锚 Java sa-token excludes,见 flow_route.rs 头注释);
    // 静态资源 fetch 不过 oper_log, 避免刷爆操作日志(同 oss_public 口径;
    // Java 侧静态也不经过滤器)。
    let designer_public =
        designer_public_route().with_state(flow_state.clone());

    // warm-flow 设计器 API 面:/warm-flow/* 全量
    // 过鉴权,与 flow_routes 同款形态(oper_log + auth)。
    let designer_api_routes = apply_auth(
        designer_api_route().layer(oper_log_layer.clone()),
        app_state.clone(),
        mode,
    )
    .with_state(flow_state);

    // local OSS 静态直链: 公开访问(等价 public bucket), 不过 auth;
    // 静态资源 fetch 不过 oper_log, 避免刷爆操作日志。
    let oss_public = oss_public_route().with_state(app_state.clone());

    // 系统管理路由
    let system_routes = apply_auth(
        Router::new()
            .route(
                "/auth/unlock/{social_id}",
                delete(crate::auth::handler::auth_handler::unlock_social),
            )
            .merge(user_route())
            .merge(role_route())
            .merge(menu_route())
            .merge(dept_route())
            .merge(post_route())
            .merge(client_route())
            .merge(notice_route())
            .merge(dict_type_route())
            .merge(dict_data_route())
            .merge(config_route())
            .merge(profile_route())
            .merge(oss_route(oss_upload_body_limit))
            .merge(push_route())
            .merge(gen_route())
            .merge(social_route())
            .merge(message_route())
            //参考实现: 办理人重解析补偿记录查询(outbox 队列表)
            .merge(assignee_sync_route())
            .layer(oper_log_layer.clone()),
        app_state.clone(),
        mode,
    )
    .with_state(app_state.clone());

    // 监控路由
    let monitor_routes = apply_auth(
        Router::new()
            .merge(oper_log_route(oper_log_layer.clone()))
            .merge(login_info_route(oper_log_layer.clone()))
            .merge(online_user_route(oper_log_layer.clone()))
            .merge(cache_route(oper_log_layer.clone())),
        app_state.clone(),
        mode,
    )
    .with_state(monitor_state);

    let job_handler_state = Arc::new(JobHandlerState {
        services: job_state.services.clone(),
        scheduler: Arc::clone(&job_state.scheduler),
        // 与 infra.data_source() 同一 Arc(装配后带 resolver);
        // job 全豁免读不解析 resolver,语义不受快照影响
        // (monitor_state 在 app.rs 同款装配)。
        ds: app_state.infra.data_source(),
    });
    let job_log_handler_state = Arc::new(JobLogHandlerState {
        services: job_state.services.clone(),
        ds: app_state.infra.data_source(),
    });

    // 全部业务 API 统一挂在 /api 前缀下(含 /api/health、/api/metrics
    // 运维端点);根路径只留 SPA 静态 fallback。
    let api = Router::new()
        .route("/health", axum::routing::get(|| async { "ok" }))
        .route(
            "/metrics",
            axum::routing::get(|| async {
                let body = app_core::util::metrics::metrics_output();
                (
                    axum::http::StatusCode::OK,
                    [(
                        axum::http::header::CONTENT_TYPE,
                        "text/plain; version=0.0.4; charset=utf-8",
                    )],
                    body,
                )
            }),
        )
        .merge(auth_public)
        .merge(designer_public)
        .merge(designer_api_routes)
        .merge(oss_public)
        .merge(system_routes)
        .merge(flow_routes)
        .merge(monitor_routes)
        // SEC-1: job / jobLog 必须与 system/monitor 一样经过鉴权,
        // 否则匿名请求可增删改并立即执行定时任务。
        // state 已在 job_route / job_log_route 内部 with_state,此处 S = (),
        // 与 apply_auth 的泛型签名兼容(route_layer 覆盖已注册的全部路由)。
        .nest(
            "/job",
            apply_auth(job_route(job_handler_state), app_state.clone(), mode),
        )
        .nest(
            "/monitor/jobLog",
            apply_auth(
                job_log_route(job_log_handler_state),
                app_state.clone(),
                mode,
            ),
        );

    // panic 兜底探针,仅 test-util(见 testkit::panic_route 的文件注释)。
    #[cfg(feature = "test-util")]
    let api = api.merge(crate::testkit::panic_route());

    // panic 兜底挂在 build_router 的最外层:罩住全部 handler 与本函数内的
    // 中间件(auth / oper_log)。main.rs 之后叠加的层(Redis 三件套、
    // 静态托管)在它**之外**,不受其保护 —— 那几层都很薄,
    // 且放进来会连带把静态资源也套一层,得不偿失。挂在这里的另一个好处是
    // 集成测试能直接驱动它(见 tests/contract_panic_test.rs)。
    Router::new()
        .nest("/api", api)
        .layer(CatchPanicLayer::custom(panic_response))
}
