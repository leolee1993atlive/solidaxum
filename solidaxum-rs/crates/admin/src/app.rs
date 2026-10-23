use std::{sync::Arc, time::Duration};

use anyhow::{Context, anyhow};
use app_core::{
    config::AppConfig,
    // 连接构建走 app-data 漏斗(connect 是 DataSource 关联函数,
    // admin 不再命名 sea-orm 的 Database;ConnectOptions 经受限 re-export,
    // 它自己不能连接)
    infra::{ConnectionPool, Infrastructure},
};
use app_data::sea_orm::ConnectOptions;
use job::{
    JobServices, JobState, demo_handlers::register_builtin_handlers,
    handler_trait::JobRegistry, scheduler::SchedulerService,
};
use monitor::MonitorServices;
use system::app::{AppState, ServiceContainer};

pub struct AdminResources {
    pub app_state: Arc<AppState>,
    pub monitor_state: Arc<monitor::OperLogState>,
    pub job_state: Arc<job::JobState>,
}

#[tracing::instrument(skip(config))]
pub async fn init_app(config: &AppConfig) -> anyhow::Result<AdminResources> {
    // DSN 构建复用 installer::dsn —— 生产路径与 --check-config 用同一份逻辑,
    // 含 percent-encoding(密码含 @:/ 等字符也能正确连接)。
    let redis_url = crate::installer::dsn::build_redis_url(&config.redis);
    let mut redis_cfg = deadpool_redis::Config::from_url(redis_url.clone());
    redis_cfg.pool = Some(deadpool_redis::PoolConfig {
        max_size: config.pool.redis_max_connections,
        timeouts: deadpool_redis::Timeouts {
            wait: Some(Duration::from_secs(5)),
            create: Some(Duration::from_secs(5)),
            recycle: Some(Duration::from_secs(30)),
        },
        ..Default::default()
    });
    let redis_pool = redis_cfg
        .create_pool(Some(deadpool_redis::Runtime::Tokio1))
        .map_err(|e| {
            anyhow!("创建Redis连接池失败 (url={}): {e:?}", redis_url,)
        })?;
    tracing::info!(
        host = %config.redis.url,
        port = config.redis.port,
        db = config.redis.db,
        "成功连接 Redis"
    );

    let dsn = crate::installer::dsn::build_db_dsn(&config.db)?;

    let mut opt = ConnectOptions::new(&dsn);
    opt.max_connections(config.pool.db_max_connections)
        .min_connections(config.pool.db_min_connections)
        .connect_timeout(Duration::from_secs(30))
        .acquire_timeout(Duration::from_secs(30))
        .idle_timeout(Duration::from_secs(600))
        .max_lifetime(Duration::from_secs(600))
        .sqlx_logging(false);

    // 达梦:裸表名在哪个 schema 下解析。达梦是单实例单库,MySQL 的 database 对应它的
    // schema,而 schema 即用户 —— 装表账号与跑应用的账号不同一个时必须显式指定,
    // 否则报 `无效的表或视图名[sys_user]`。PG 也认这个字段,故不限定 db_type。
    // 取用规则与 --check-config 共用同一个函数,防两条路径分叉。
    if let Some(schema) = crate::installer::dsn::schema_search_path(&config.db)
    {
        opt.set_schema_search_path(schema);
    }

    // ds0:resolver-less 母体 —— 它同时是 resolver 的 unscoped 通道(见下),
    // 也是派生主 ds 的连接来源。core 不持连接,这是装配层唯一的
    // 连接构造点。
    let ds0 = Arc::new(
        app_data::DataSource::connect(opt)
            .await
            .with_context(|| format!("数据库连接失败: {}", config.db.db_name))?
            .with_count_mode(config.app.count_mode()),
    );

    tracing::info!(
        db_type = %config.db.db_type,
        db_name = %config.db.db_name,
        count_mode = %config.app.count_mode,
        "成功连接数据库"
    );

    let infra = Infrastructure::new(
        Arc::new(ConnectionPool { redis_pool }),
        config.captcha.enable,
        config.jwt.refresh_ttl as u64,
    );

    // 行级范围 resolver 的装配环在此解开(顺序是硬约束,勿调):
    //   ds0(连接,零 resolver)→ Infrastructure(零连接,ds 槽待注入)
    //   → CachedScopeResolver(依赖 PermissionService;unscoped 通道 =
    //   **ds0 本尊**,不再第二个 from_connection)
    //   → ds1 = ds0.derive_with_resolver(只读派生,母体不受染)
    //   → infra.set_data_source(ds1)
    //   → ServiceContainer(其构造期就有约 8 个服务调 infra.data_source()
    //   捕获快照 —— config/dict/client/codegen 等,全豁免读走
    //   system_context 不解析 resolver;**必须**在 set 之后构造,否则
    //   撞上未装配 panic)
    //   → 同一份 resolver Arc 第二投:services.set_scope_cache(角色/用户
    //   写路径的主动失效)
    //
    // resolver 先于容器构造的解点:PermissionService::new(infra) 完全
    // 无状态(= for_test(),三个无状态仓储),此处自建一份与容器内部的
    // services.permission_service 同型等价,不构成第二本可漂移的状态。**双投必须是同一个 resolver Arc**:若各造
    // 一份 resolver,失效只清其一,另一本仍按 60s TTL 返回陈旧范围。
    // ds0 作为 resolver 第一参是**自有 unscoped 通道**:resolver-free
    // 快照(结构上不可能再触发解析,
    // 详见 scope_resolver.rs 的 ds 字段注释)—— derive 只读派生保证 ds0
    // 不被 resolver 污染。
    let resolver = Arc::new(
        system::permission::scope_resolver::CachedScopeResolver::new(
            ds0.clone(),
            Arc::new(system::permission::service::PermissionService::new(
                &infra,
            )),
        ),
    );
    let ds1 = ds0.derive_with_resolver(resolver.clone());
    if !infra.set_data_source(Arc::new(ds1)) {
        // 单调用点 + OnceLock:唯一可能是重复装配(编程错误)。首次句柄
        // 仍在生效,不炸进程,但必须让启动日志可见。
        tracing::error!("DataSource 重复装配被拒绝,首次注入的句柄继续生效");
    }
    let services = ServiceContainer::new(
        &infra,
        config.security.aes_key.clone(),
        &config.codegen,
    )?;
    // 第二投 —— 失效槽。role/user 写路径经它调用
    // invalidate()/invalidate_all(),消除「角色变更后最长 60s 陈旧」的
    // 活体缺口。重复装配(编程错误)同 set_data_source:恒 false,首次
    // 句柄继续生效,启动日志可见即可。
    if !services.set_scope_cache(resolver) {
        tracing::error!("scope_cache 重复装配被拒绝,首次注入的句柄继续生效");
    }

    let monitor_services = MonitorServices::new(&infra);

    let app_state = Arc::new(AppState { infra, services });
    let monitor_state = Arc::new(monitor::OperLogState {
        services: monitor_services,
        // 与 infra.data_source() 同一 Arc(装配后带 resolver);
        // monitor 全豁免读不解析 resolver,语义不受快照影响。
        ds: app_state.infra.data_source(),
        // 基座自身无补充路径名规则,留空;下游产品在此注册。
        title_rules: &[],
    });

    let job_services = JobServices::new(&app_state.infra);

    let mut registry = JobRegistry::new();
    register_builtin_handlers(&mut registry);
    let registry = Arc::new(registry);

    let scheduler = SchedulerService::new(
        // 与 monitor_state 同一 Arc(set_data_source 已在前装配,
        // data_source() 取到带 resolver 的句柄);调度器全走
        // system_context,不解析 resolver,语义不受快照影响。
        app_state.infra.data_source(),
        Arc::new(app_state.infra.pool.redis_pool.clone()),
        registry,
    )
    .await
    .context("初始化定时任务调度器失败")?;
    scheduler.init().await.context("加载定时任务失败")?;

    let job_state = Arc::new(JobState {
        services: job_services,
        scheduler,
    });

    Ok(AdminResources {
        app_state,
        monitor_state,
        job_state,
    })
}
