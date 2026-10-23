//! HTTP 集成测试脚手架 (feature = "test-util")。
//!
//! 提供:
//! - [`test_auth`]:绕过 Redis 的鉴权中间件,从请求头注入 `LoginUser`。
//! - [`TestApp`]:基于 SQLite 内存库构建完整应用路由,并用
//!   `tower::ServiceExt::oneshot` 驱动请求(无需真实网络/Redis)。
//!
//! 用法见 `crates/admin/tests/`。契约测试应以原版 Vue 前端的 API 调用为准。
//!
//! 注意:DDL/seed(脚手架自带的精简集,覆盖 RBAC 核心 + notice)由
//! `app_data::testkit` 持有,此处只接线不再持有。
//! 依赖 Redis 缓存的接口(config/dict/oss 等)需另接测试用 Redis,暂不在
//! 默认脚手架覆盖范围内。

use std::sync::Arc;

use app_core::{
    config::CodegenConfig,
    infra::{ConnectionPool, Infrastructure},
    model::login_user::LoginUser,
    vo::response::R,
};
use axum::{
    Router,
    body::Body,
    extract::State,
    http::{HeaderMap, Method, Request, StatusCode, header},
    middleware::Next,
    response::{IntoResponse, Response},
};
use base64::{Engine, engine::general_purpose::STANDARD as BASE64};
use flow::repository::entity::{
    flow_category, flow_definition, flow_his_task, flow_instance,
    flow_instance_biz_ext, flow_node, flow_skip, flow_spel, flow_task,
    flow_user, test_leave,
};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use system::{
    app::{AppState, ServiceContainer},
    push::sse::PushHub,
};
use tower::ServiceExt;

use crate::{AuthMode, build_router};

/// 携带 base64(JSON(LoginUser)) 的测试鉴权请求头。
pub const TEST_USER_HEADER: &str = "x-test-login-user";

/// 测试用鉴权中间件:从 [`TEST_USER_HEADER`] 解出 `LoginUser` 注入 extensions,
/// 不读 Redis。缺失/非法则返回 401(与真实 auth 的"缺少凭据"语义一致),
/// 以便测试未登录访问。
pub async fn test_auth(
    State(_state): State<Arc<AppState>>,
    mut req: Request<Body>,
    next: Next,
) -> Response {
    let login_user = req
        .headers()
        .get(TEST_USER_HEADER)
        .and_then(|v| v.to_str().ok())
        .and_then(|b64| BASE64.decode(b64).ok())
        .and_then(|bytes| serde_json::from_slice::<LoginUser>(&bytes).ok());

    let login_user = match login_user {
        Some(u) => u,
        None => {
            return R::<()>::error_with_code_and_message(
                401,
                "请求缺少认证凭据".to_string(),
            )
            .into_response();
        }
    };

    // 与真实 auth 一致:注入 request extensions(供 CurrentUser 提取),
    // 并在 response extensions 注入(供 OperLogLayer 读取)。
    req.extensions_mut().insert(login_user.clone());
    let mut resp = next.run(req).await;
    resp.extensions_mut().insert(login_user);
    resp
}

/// testkit 专用:默认恒返回 `ScopeFilter::All` 的 resolver(装配理由见
/// [`TestApp::new`] 的装配点注释);[`AllScopeResolver::with_user`] 可给
/// 个别 user_id 钉具体范围 —— 契约测试要断言范围行为时用。
/// 经 [`TestApp::new_with_resolver`] 接线(范围探针契约
/// 测试,见 contract_scope_probe_test.rs);默认 [`TestApp::new`] 仍走 All。
/// app-data 的 `FixedResolver` 在其 test-util feature 之后,而 admin 的
/// testkit 已有自己的 feature 门,这里就地实现,不动 Cargo 特性面。
#[derive(Default)]
pub struct AllScopeResolver {
    /// user_id → 指定范围;未命中恒答 All。
    overrides: std::collections::HashMap<i64, app_data::ScopeFilter>,
}

impl AllScopeResolver {
    /// 链式:给指定 user_id 覆盖范围,其余用户仍答 All。
    pub fn with_user(
        mut self,
        user_id: i64,
        filter: app_data::ScopeFilter,
    ) -> Self {
        self.overrides.insert(user_id, filter);
        self
    }
}

#[async_trait::async_trait]
impl app_data::ScopeResolver for AllScopeResolver {
    async fn resolve(
        &self,
        p: &app_data::Principal,
    ) -> Result<app_data::ScopeFilter, app_data::DataError> {
        Ok(self
            .overrides
            .get(&p.user_id)
            .cloned()
            .unwrap_or(app_data::ScopeFilter::All))
    }
}

/// 集成测试应用:持有已组装好的 Router,提供请求/断言辅助方法。
/// 种子/直查需 DataSource 时经 `app.state.infra.data_source()`(装配后
/// 即同一 Arc;`TestApp.db` 裸连接字段已随 legacy 删除)。
pub struct TestApp {
    pub router: Router,
    pub push_hub: PushHub,
    /// 供需要直接驱动 service 的测试使用(如授权判定)。
    pub state: Arc<AppState>,
}

impl TestApp {
    /// 构建一个带 SQLite 内存库 + 种子数据的完整应用(test 鉴权模式)。
    /// resolver 恒答 All(契约测试默认只钉 API 形状);需要断言**范围
    /// 行为**的测试用 [`Self::new_with_resolver`] 配
    /// [`AllScopeResolver::with_user`]。
    pub async fn new() -> Self {
        Self::new_with_resolver(AllScopeResolver::default()).await
    }

    /// [`Self::new`] 的可配置变体:注入带 per-user 覆盖的 resolver ——
    /// 范围探针契约测试用:给目标 user_id 钉 `ScopeFilter`
    /// (如 DeptIn([101])),其余用户仍答 All。
    pub async fn new_with_resolver(resolver: AllScopeResolver) -> Self {
        // ds0:resolver-less 母体(setup_db 建库灌种;同时是下方
        // CachedScopeResolver 的 unscoped 通道)。
        let ds0 = setup_db().await;

        // 惰性 Redis 池:不会真正连接,只要测试路径不触发 Redis 即可。
        let redis_pool =
            deadpool_redis::Config::from_url("redis://127.0.0.1:6379")
                .create_pool(Some(deadpool_redis::Runtime::Tokio1))
                .expect("创建惰性 redis 池失败");

        let infra = Infrastructure::new(
            Arc::new(ConnectionPool { redis_pool }),
            false,
            1800,
        );

        // 与生产 app.rs 同构的装配点(set_data_source 必须先于
        // ServiceContainer —— 容器内服务构造期就调 infra.data_source()
        // 捕获快照,未装配会 panic)。post 等模块的 handler 用
        // `context(Principal)` 走行级范围 —— 无 resolver 会 fail-closed
        // Unsupported。testkit 没有真实角色语义(种子角色不反映 HTTP 测试
        // 用户的数据范围),故 resolver 恒答 All:契约测试只钉 API 形状,
        // 数据范围的分支语义由 system crate 的 repo 级测试(post_repo_test)
        // 承担。范围相关的契约测试出现时,给目标 user_id 配
        // `AllScopeResolver::with_user`(经 `TestApp::new_with_resolver`
        // 注入,见 contract_scope_probe_test.rs)。
        assert!(
            infra.set_data_source(Arc::new(
                ds0.derive_with_resolver(Arc::new(resolver))
            )),
            "testkit 的 DataSource 装配只此一次"
        );

        let services =
            ServiceContainer::new(&infra, None, &CodegenConfig::default())
                .expect("构建 ServiceContainer 失败");

        // 失效槽的第二投(与生产同构)。DataSource 侧仍用上面的
        // AllScopeResolver(契约测试理由见上);scope_cache 槽填**真**
        // CachedScopeResolver(包同一份 permission_service)—— role/user
        // 写路径的 invalidate*/all 经它真实执行(测试里没人读这本缓存,
        // 失效为幂等 no-op)。装配本身是测试面:若生产改了槽形状而忘了
        // testkit,这里会在 set_scope_cache 处暴露,避免「生产双投、
        // 测试缺投」的分叉静默通过。
        assert!(
            services.set_scope_cache(Arc::new(
                // 第一参:resolver 自有 unscoped 通道(与生产 app.rs 同款
                // —— ds0 本尊,derive 不染母体)。
                system::permission::scope_resolver::CachedScopeResolver::new(
                    ds0.clone(),
                    Arc::new(services.permission_service.clone()),
                )
            )),
            "testkit 的 scope_cache 装配只此一次"
        );

        let app_state = Arc::new(AppState {
            infra: infra.clone(),
            services,
        });
        let push_hub = app_state.services.push_hub.clone();

        let monitor_state = Arc::new(monitor::OperLogState {
            services: monitor::MonitorServices::new(&infra),
            ds: infra.data_source(),
            title_rules: &[],
        });

        let mut registry = job::handler_trait::JobRegistry::new();
        job::demo_handlers::register_builtin_handlers(&mut registry);
        // data_source() 取上面 set_data_source 装配的同一份 Arc —— 读序
        // 由既有断言锚定("testkit 的 DataSource 装配只此一次"):monitor/
        // job 的构造必在装配之后,否则这里会撞上未装配 panic。
        // 调度器全走 system_context,不解析 resolver,语义不受快照影响。
        let scheduler = job::scheduler::SchedulerService::new(
            infra.data_source(),
            Arc::new(infra.pool.redis_pool.clone()),
            Arc::new(registry),
        )
        .await
        .expect("构建调度器失败");
        // 不调用 scheduler.init():避免依赖 sys_job 表与启动后台任务。
        let job_state = Arc::new(job::JobState {
            services: job::JobServices::new(&infra),
            scheduler,
        });

        let router = build_router(
            app_state.clone(),
            monitor_state,
            job_state,
            // 测试沿用默认 10MB(对齐 OssConfig::default),
            // 不关心真实配置值;契约测试自行构造小体积 body。
            10 * 1024 * 1024,
            AuthMode::Test,
        );

        Self {
            router,
            push_hub,
            state: app_state,
        }
    }

    /// 发起一次请求,返回 (状态码, 响应体 JSON)。
    pub async fn request(
        &self,
        method: Method,
        uri: &str,
        body: Option<Value>,
        user: Option<&LoginUser>,
    ) -> (StatusCode, Value) {
        let mut builder = Request::builder().method(method).uri(api_uri(uri));
        if let Some(u) = user {
            let payload = serde_json::to_vec(u).expect("序列化 LoginUser 失败");
            builder = builder.header(TEST_USER_HEADER, BASE64.encode(payload));
        }
        let req = match body {
            Some(v) => builder
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(v.to_string()))
                .expect("构建请求失败"),
            None => builder.body(Body::empty()).expect("构建请求失败"),
        };

        let resp = self
            .router
            .clone()
            .oneshot(req)
            .await
            .expect("路由处理失败");
        let status = resp.status();
        let bytes = resp
            .into_body()
            .collect()
            .await
            .expect("读取响应体失败")
            .to_bytes();
        let json = if bytes.is_empty() {
            Value::Null
        } else {
            serde_json::from_slice(&bytes).unwrap_or(Value::Null)
        };
        (status, json)
    }

    pub async fn get(
        &self,
        uri: &str,
        user: Option<&LoginUser>,
    ) -> (StatusCode, Value) {
        self.request(Method::GET, uri, None, user).await
    }

    pub async fn post(
        &self,
        uri: &str,
        body: Value,
        user: Option<&LoginUser>,
    ) -> (StatusCode, Value) {
        self.request(Method::POST, uri, Some(body), user).await
    }

    pub async fn put(
        &self,
        uri: &str,
        body: Value,
        user: Option<&LoginUser>,
    ) -> (StatusCode, Value) {
        self.request(Method::PUT, uri, Some(body), user).await
    }

    pub async fn delete(
        &self,
        uri: &str,
        user: Option<&LoginUser>,
    ) -> (StatusCode, Value) {
        self.request(Method::DELETE, uri, None, user).await
    }

    /// 发起请求并返回原始字节 + 响应头(供二进制响应如 xlsx 导出断言)。
    /// `request` 对非 JSON 响应体返回 `Value::Null` 会丢字节,故导出测试用此方法。
    pub async fn request_bytes(
        &self,
        method: Method,
        uri: &str,
        body: Option<Value>,
        user: Option<&LoginUser>,
    ) -> (StatusCode, HeaderMap, Vec<u8>) {
        let mut builder = Request::builder().method(method).uri(api_uri(uri));
        if let Some(u) = user {
            let payload = serde_json::to_vec(u).expect("序列化 LoginUser 失败");
            builder = builder.header(TEST_USER_HEADER, BASE64.encode(payload));
        }
        let req = match body {
            Some(v) => builder
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(v.to_string()))
                .expect("构建请求失败"),
            None => builder.body(Body::empty()).expect("构建请求失败"),
        };

        let resp = self
            .router
            .clone()
            .oneshot(req)
            .await
            .expect("路由处理失败");
        let status = resp.status();
        let headers = resp.headers().clone();
        let bytes = resp
            .into_body()
            .collect()
            .await
            .expect("读取响应体失败")
            .to_bytes();
        (status, headers, bytes.to_vec())
    }

    /// 以 `application/x-www-form-urlencoded` 发起请求并返回原始字节 + 响应头。
    /// 复刻 Vue `download()`(`request.ts:259`)的真实导出契约:body 为
    /// `tansParams` 编码的表单串(含 `params[beginTime]` 等扁平键),而非 JSON。
    pub async fn request_form(
        &self,
        method: Method,
        uri: &str,
        form_body: &str,
        user: Option<&LoginUser>,
    ) -> (StatusCode, HeaderMap, Vec<u8>) {
        let mut builder = Request::builder().method(method).uri(api_uri(uri));
        if let Some(u) = user {
            let payload = serde_json::to_vec(u).expect("序列化 LoginUser 失败");
            builder = builder.header(TEST_USER_HEADER, BASE64.encode(payload));
        }
        let req = builder
            .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
            .body(Body::from(form_body.to_owned()))
            .expect("构建请求失败");

        let resp = self
            .router
            .clone()
            .oneshot(req)
            .await
            .expect("路由处理失败");
        let status = resp.status();
        let headers = resp.headers().clone();
        let bytes = resp
            .into_body()
            .collect()
            .await
            .expect("读取响应体失败")
            .to_bytes();
        (status, headers, bytes.to_vec())
    }
}

/// 业务 API 统一在 `/api` 前缀下(见 `router::build_router`,含
/// /api/health、/api/metrics)。测试调用点仍写业务路径
/// (`/system/user/list`),这里统一补前缀;已带 `/api` 的原样透传。
///
/// `pub` 的原因:个别测试(multipart 上传/二进制下载)绕开 `request()`
/// 直接用 `app.router` 发裸请求,同样需要这层前缀逻辑。
pub fn api_uri(uri: &str) -> String {
    if uri.starts_with("/api") {
        uri.to_owned()
    } else {
        format!("/api{uri}")
    }
}

/// 构造一个带指定权限的普通登录用户(user_id=301, dept=101)。
pub fn user_with_perms(perms: &[&str]) -> LoginUser {
    LoginUser {
        user_id: 301,
        dept_id: Some(101),
        username: "test_user".to_string(),
        nick_name: Some("tester".to_string()),
        user_type: Some("sys_user".to_string()),
        client_id: "test_client".to_string(),
        client_key: Some("test".to_string()),
        device_type: None,
        permissions: perms.iter().map(|s| s.to_string()).collect(),
        roles: std::collections::HashSet::new(),
        login_ip: Some("127.0.0.1".to_string()),
        login_date: None,
    }
}

/// 超级管理员(拥有 `*:*:*`)。
pub fn super_admin() -> LoginUser {
    let mut u = user_with_perms(&["*:*:*"]);
    u.user_id = app_core::SUPER_ADMIN_USER_ID;
    u.username = "admin".to_string();
    u
}

/// 从响应信封读取 `code` 字段。
pub fn code(body: &Value) -> i64 {
    body.get("code").and_then(Value::as_i64).unwrap_or(-1)
}

/// 断言响应为成功信封(HTTP 200 且 code==200)。
pub fn assert_ok(status: StatusCode, body: &Value) {
    assert_eq!(status, StatusCode::OK, "HTTP 状态非 200: body={body}");
    assert_eq!(code(body), 200, "业务 code 非 200: body={body}");
}

/// 创建 SQLite 内存库,建表 + 灌入种子数据(返回 resolver-less 的
/// `Arc<DataSource>` 母体,由装配层 derive 派生)。
pub async fn setup_db() -> Arc<app_data::DataSource> {
    // 连接只经 app-data 漏斗构造(单连接内存库语义收敛在
    // sqlite_memory_empty 内);DDL/种子字符串脚本下沉 app_data::testkit,
    // 本文件不再持有原生 SQL(check-no-raw-sql 白名单已收窄锚到那边)。
    let ds = app_data::testing::sqlite_memory_empty().await;
    app_data::testkit::run_script(&ds, app_data::testkit::SYSTEM_TABLE_DDL)
        .await
        .expect("DDL 执行失败");
    app_data::testkit::run_script(&ds, app_data::testkit::SYSTEM_SEED)
        .await
        .expect("seed 执行失败");

    // flow 业务表(实体衍生 13 张,经 `DataSource::create_table::<E>` 建表
    // (test-util 面,app-data 内部持 Schema);`.ok()` 吞表已存在类错误)。
    ds.create_table(flow_definition::Entity).await.ok();
    ds.create_table(flow_node::Entity).await.ok();
    ds.create_table(flow_skip::Entity).await.ok();
    ds.create_table(flow_instance::Entity).await.ok();
    ds.create_table(flow_instance_biz_ext::Entity).await.ok();
    ds.create_table(flow_task::Entity).await.ok();
    ds.create_table(flow_his_task::Entity).await.ok();
    ds.create_table(flow_user::Entity).await.ok();
    ds.create_table(flow_category::Entity).await.ok();
    ds.create_table(flow_spel::Entity).await.ok();
    ds.create_table(test_leave::Entity).await.ok();
    //参考实现 outbox 表(办理人补偿任务;挂钩点事务内写入,
    // user/role 写路径契约测试需要它存在)
    ds.create_table(
        system::assignee_sync::repository::entity::flow_assignee_sync::Entity,
    )
    .await
    .ok();

    // flow 种子数据。严格断言:种子 SQL 与实体表结构不一致(如多列/少列)必须立刻暴露,
    // 不能被静默吞掉,否则定义缺节点/跳转会让 startWorkFlow 之后才神秘报错。
    app_data::testkit::run_script(&ds, app_data::testkit::FLOW_SEED)
        .await
        .expect("flow 种子执行失败");

    Arc::new(ds)
}

/// 便捷:把任意可序列化值转 JSON(测试请求体用)。
pub fn body(v: Value) -> Value {
    json!(v)
}

/// panic 探针的固定文案 —— 测试断言它**不会**出现在响应体里。
pub const PANIC_PROBE_MESSAGE: &str = "testkit 故意触发的 panic";

async fn always_panics() -> &'static str {
    panic!("{PANIC_PROBE_MESSAGE}")
}

/// 一条必定 panic 的探针路由,由 [`build_router`](crate::build_router) 在
/// `test-util` 下并入 `/api`。生产不存在这条路由,它唯一的用途是让
/// panic 兜底层(`CatchPanicLayer`)可被集成测试驱动 —— 否则"panic 会不会
/// 变成 500 信封"只能靠人肉验证。不过 auth:探针要测的是兜底层,
/// 不是鉴权。
pub fn panic_route() -> Router {
    Router::new().route("/testkit/panic", axum::routing::get(always_panics))
}
