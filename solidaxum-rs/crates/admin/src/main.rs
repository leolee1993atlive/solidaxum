use std::{path::Path, sync::Arc};

use ::config::{Config, File};
use admin::{AuthMode, app, build_router};
use anyhow::Context;
use app_core::{
    config::AppConfig,
    middleware::{
        rate_limiter::rate_limit_middleware,
        repeat_submit::repeat_submit_middleware,
        state::{
            MiddlewareState, RateLimitConfig, RepeatSubmitConfig, XssConfig,
        },
        xss_filter::xss_filter_middleware,
    },
};
use axum::middleware::from_fn_with_state;
use tower_http::{
    cors::{Any, CorsLayer},
    trace::TraceLayer,
};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

fn init_tracing(log_config: &app_core::config::LogConfig) {
    // RUST_LOG 未设置时的默认过滤器。注意:业务代码分布在 admin/system/
    // monitor/flow/job 等多个 crate 中,事件 target 以各自 crate 名为前缀,
    // 因此不能只放行二进制 crate(solidaxum),否则登录等日志会被丢弃。
    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| {
            "info,admin=debug,system=debug,monitor=debug,flow=debug,\
             job=debug,app_core=debug,sqlx=warn"
                .into()
        });

    let log_dir = Path::new(&log_config.dir);
    if let Err(e) = std::fs::create_dir_all(log_dir) {
        eprintln!("警告:无法创建日志目录 {:?}: {}", log_dir, e);
    }

    let file_appender =
        tracing_appender::rolling::daily(log_dir, &log_config.prefix);
    let (file_writer, _guard) = tracing_appender::non_blocking(file_appender);

    let console_layer = tracing_subscriber::fmt::layer();
    let file_layer = tracing_subscriber::fmt::layer()
        .with_writer(file_writer)
        .with_ansi(false);

    tracing_subscriber::registry()
        .with(env_filter)
        .with(console_layer)
        .with(file_layer)
        .init();

    std::mem::forget(_guard);
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 安装子命令在最早处 dispatch,先于 tracing/init_app —— 这些子命令应
    // 在无日志初始化、无服务装配的状态下快速完成。
    let cli = admin::installer::cli::parse(std::env::args())?;
    match cli {
        admin::installer::cli::Cli::WriteConfig { config, answers } => {
            let answers = admin::installer::answers::Answers::load(&answers)
                .context("加载 answers.toml 失败")?;
            admin::installer::writer::write_config(&config, &answers)
                .context("写入 config.toml 失败")?;
            println!("config.toml 已写入: {}", config.display());
            return Ok(());
        }
        admin::installer::cli::Cli::CheckConfig { config } => {
            admin::installer::checker::check_config(&config)
                .await
                .context("配置检测失败")?;
            println!("配置检测通过: {}", config.display());
            return Ok(());
        }
        admin::installer::cli::Cli::Serve => {}
    }

    let config = Config::builder()
        .add_source(File::with_name("config.toml"))
        .build()
        .context("读取 config.toml 失败")?
        .try_deserialize::<AppConfig>()
        .context("解析配置文件失败")?;

    config.validate().context("配置校验失败")?;

    if let Some(ref key) = config.security.aes_key
        && !key.is_empty()
    {
        use base64::{Engine, engine::general_purpose::STANDARD};
        let bytes = STANDARD
            .decode(key)
            .expect("security.aes_key 必须是有效的 base64 字符串");
        assert_eq!(
            bytes.len(),
            32,
            "security.aes_key 解码后必须是 32 字节（AES-256）"
        );
    }

    init_tracing(&config.log);

    app_core::util::metrics::init_metrics();

    let resources = app::init_app(&config).await?;
    let app_state = resources.app_state;
    let monitor_state = resources.monitor_state;
    let job_state = resources.job_state;

    // 中间件共享状态
    let middleware_state = Arc::new(MiddlewareState {
        redis: app_state.infra.pool.clone(),
        rate_limit_config: RateLimitConfig {
            capacity: config.security.rate_limit_capacity,
            window_secs: config.security.rate_limit_window_secs,
            exclude_paths: vec![],
        },
        repeat_submit_config: RepeatSubmitConfig {
            ttl_seconds: config.security.repeat_submit_ttl_seconds,
        },
        xss_config: XssConfig {
            exclude_paths: config.security.xss_exclude_paths.clone(),
        },
    });

    // 业务路由(与测试共用的组装逻辑)
    let app = build_router(
        app_state.clone(),
        monitor_state,
        job_state,
        config.oss.upload_body_limit,
        AuthMode::Real,
    );

    let mut app = app;

    if let Some(dir) = &config.server.static_dir {
        if std::path::Path::new(dir).is_dir() {
            let index = std::path::Path::new(dir).join("index.html");
            // SPA 深链回退到 index.html。必须用
            // `fallback` 而不是 `not_found_service` —— 后者会把响应状态
            // 强制改写成 404(SetStatus),浏览器硬刷新深链时拿到 404 状态。
            app = app.fallback_service(
                tower_http::services::ServeDir::new(dir)
                    .fallback(tower_http::services::ServeFile::new(index)),
            );
            tracing::info!(static_dir = %dir, "已启用静态文件托管");
        } else {
            tracing::warn!(
                static_dir = %dir,
                "static_dir 不存在,跳过静态托管"
            );
        }
    }

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    // 依赖 Redis 的全局中间件在此叠加(测试路由不含这些层)
    let app = app
        .layer(TraceLayer::new_for_http())
        .layer(cors)
        .layer(from_fn_with_state(
            middleware_state.clone(),
            xss_filter_middleware,
        ))
        .layer(from_fn_with_state(
            middleware_state.clone(),
            repeat_submit_middleware,
        ))
        .layer(from_fn_with_state(
            middleware_state.clone(),
            rate_limit_middleware,
        ));

    // 办理人补偿消费(参考实现):用户/角色变更的 outbox 行由这里
    // 逐条重解析运行中实例的办理人
    admin::assignee_sync_worker::spawn_assignee_sync_worker(
        app_state.clone(),
        config.flow.assignee_sync_interval_secs,
        config.flow.assignee_sync_max_retry,
    );

    let listener = tokio::net::TcpListener::bind(format!(
        "{}:{}",
        config.server.host, config.server.port
    ))
    .await
    .context("绑定监听地址失败")?;
    let addr = listener.local_addr().context("读取监听地址失败")?;
    tracing::info!(addr = %addr, "服务启动监听");
    axum::serve(listener, app)
        .await
        .context("启动 HTTP 服务失败")?;
    Ok(())
}
