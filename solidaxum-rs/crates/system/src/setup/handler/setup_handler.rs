use std::sync::Arc;

use app_core::{
    error::AppError, extract::AppJson, middleware::extractors::CurrentUser,
    vo::response::R,
};
use app_data::Principal;
use axum::extract::State;
use validator::Validate;

use crate::{
    app::AppState,
    config::handler::config_handler::SYS_INFO_TITLE_KEY,
    setup::model::setup::{SetupCompleteBo, SetupStatusVo},
};

/// 用户默认密码的 sys_config 键(与 solidaxum_init.sql 种子及 user_handler
/// 导入路径同键)。
const USER_INIT_PASSWORD_KEY: &str = "sys.user.initPassword";
/// 首装初始化完成标记键。**键不存在 = 已完成**(老系统语义)。
pub const SETUP_COMPLETED_KEY: &str = "sys.setup.completed";

/// 探测初始化状态(仅需登录)。
///
/// 前端登录成功后与 (app) 布局挂载时各探一次,未完成即强导向导。
#[tracing::instrument(skip(state))]
pub async fn setup_status(
    State(state): State<Arc<AppState>>,
    CurrentUser(login_user): CurrentUser,
) -> Result<R<SetupStatusVo>, AppError> {
    let ds = state.infra.data_source();
    let ctx = ds.context(Principal::from(&login_user));
    let value = state
        .services
        .sys_config_service
        .get_value_or_default(&ctx, SETUP_COMPLETED_KEY, "true")
        .await;

    Ok(R::ok_with_data(SetupStatusVo {
        completed: value == "true",
    }))
}

/// 聚合完成初始化:改管理员密码 + 写系统名称/用户默认密码 + 落完成标记。
///
/// 守卫(防重放):仅超级管理员,且当前状态必须是"未完成"—— 已初始化
/// 的系统再调本端点等于一个"重置 admin 密码"的后门,必须关死。
///
/// 顺序刻意为:先改密(失败即整体中止,旧密码错误时不落任何配置),
/// 再写两个配置,最后落完成标记。四步不在一个事务里:若中途失败,
/// 标记未落、向导可重试(重试幂等 —— 配置按键覆盖,改密以新密码为旧
/// 密码继续)。
#[tracing::instrument(skip(state, bo), fields(operator = %current_user.0.username))]
pub async fn setup_complete(
    State(state): State<Arc<AppState>>,
    current_user: CurrentUser,
    AppJson(bo): AppJson<SetupCompleteBo>,
) -> Result<R<()>, AppError> {
    if !current_user.0.is_super_admin() {
        return Err(AppError::Forbidden("仅超级管理员可执行系统初始化".into()));
    }
    bo.validate()?;

    let login_user = &current_user.0;
    let ds = state.infra.data_source();
    let ctx = ds.context(Principal::from(login_user));
    let config = &state.services.sys_config_service;

    // 1. 防重放:已完成(或键不存在=老系统)一律拒绝
    let completed = config
        .get_value_or_default(&ctx, SETUP_COMPLETED_KEY, "true")
        .await
        == "true";
    if completed {
        return Err(AppError::BadRequest(
            "系统已完成初始化,不可重复执行".into(),
        ));
    }

    // 2. 管理员改密(复用 profile 链路:旧密码核对 + bcrypt + 新旧不同校验)
    state
        .services
        .profile_service
        .update_password(
            &ctx,
            login_user.user_id,
            &bo.admin_old_password,
            &bo.admin_new_password,
        )
        .await?;

    // 3. 系统名称(=sys.title 顶栏标题)与用户默认密码
    config
        .update_by_key(&ctx, SYS_INFO_TITLE_KEY, &bo.system_name, &current_user)
        .await?;
    config
        .update_by_key(
            &ctx,
            USER_INIT_PASSWORD_KEY,
            &bo.user_init_password,
            &current_user,
        )
        .await?;

    // 4. 落完成标记(新种子必带该行;键不存在的情况已被第 1 步拒绝)
    config
        .update_by_key(&ctx, SETUP_COMPLETED_KEY, "true", &current_user)
        .await?;

    tracing::info!(
        operator = %login_user.username,
        system_name = %bo.system_name,
        "系统初始化向导已完成"
    );
    Ok(R::ok())
}
