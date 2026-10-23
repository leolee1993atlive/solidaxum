use std::sync::Arc;

use app_core::{
    error::AppError,
    extract::AppJson,
    middleware::extractors::CurrentUser,
    model::{entity::BaseEntity, login_user::LoginUser},
    util::{captcha, client_info, crypto_util::hash_password, snowflake},
    vo::response::R,
};
use axum::extract::{Path, State};
use monitor::{OperLogState, logininfo::model::LoginInfo};
use system::{app::AppState, user::model::user::User};
use validator::Validate;

use crate::auth::model::auth::{CaptchaVo, LoginBody, LoginVo, RegisterBody};

/// 登录失败次数 Redis key 前缀(锁定/计数)。
const PWD_ERR_CNT_PREFIX: &str = "pwd_err_cnt:";
/// 登录失败锁定阈值(超过则锁定账号)。
const LOGIN_RETRY_THRESHOLD: u32 = 5;

/// 判断登录失败次数是否达到锁定阈值
fn is_locked(count: u32, threshold: u32) -> bool {
    count >= threshold
}

#[tracing::instrument(skip(payload, state, monitor_state))]
pub async fn login(
    State((state, monitor_state)): State<(Arc<AppState>, Arc<OperLogState>)>,
    headers: axum::http::HeaderMap,
    AppJson(payload): AppJson<LoginBody>,
) -> Result<R<LoginVo>, AppError> {
    payload.validate()?;

    // 提前提取客户端信息,无论成败都记录
    let ipaddr = client_info::extract_client_ip(&headers);
    let ua = headers
        .get(axum::http::header::USER_AGENT)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    let (browser, os_str) = client_info::parse_user_agent(ua);

    // 1. 验证客户端
    // 登录路径无请求身份 → system_context;ctx 提前到这里建一次,下方查
    // 用户复用同一个(client 读豁免,system_context 无行范围,语义不变)。
    let login_ds = state.infra.data_source();
    let login_ctx = login_ds.system_context();
    let client = state
        .services
        .client_service
        .query_by_client_id(&login_ctx, &payload.client_id)
        .await
        .map_err(|e| {
            tracing::warn!(client_id = %payload.client_id, error = %e, "客户端验证失败");
            AppError::Auth("无效的客户端".to_string())
        })?;

    if !client.is_normal() {
        return Err(AppError::Auth("客户端已禁用".to_string()));
    }

    if !client.supports_grant_type(&payload.grant_type) {
        return Err(AppError::Auth("不支持的授权类型".to_string()));
    }

    // 2. 仅支持 password 授权类型
    if payload.grant_type != "password" {
        return Err(AppError::Auth("仅支持password授权类型".to_string()));
    }

    let username = payload.username.as_deref().unwrap_or("");
    let password = payload.password.as_deref().unwrap_or("");

    if username.is_empty() || password.is_empty() {
        return Err(AppError::Auth("用户名和密码不能为空".to_string()));
    }

    validate_captcha(&state, payload.uuid.as_deref(), payload.code.as_deref())
        .await?;

    // 3. 查找用户
    // 登录路径无请求身份 → system_context(ctx 在步骤 1 已建,此处复用)。
    let user = match state
        .services
        .user_service
        .get_user_by_user_name(&login_ctx, username)
        .await
    {
        Ok(u) => u,
        Err(_) => {
            record_login_fail(
                &monitor_state,
                LoginFailCtx {
                    username: username.to_string(),
                    ipaddr: &ipaddr,
                    browser: &browser,
                    os_str: &os_str,
                    client_key: &client.client_key,
                    device_type: &client.device_type,
                    msg: "用户不存在",
                },
            );
            return Err(AppError::Auth("用户或密码错误".to_string()));
        }
    };

    // 4. 检查用户状态
    if user.status != "0" {
        record_login_fail(
            &monitor_state,
            LoginFailCtx {
                username: username.to_string(),
                ipaddr: &ipaddr,
                browser: &browser,
                os_str: &os_str,
                client_key: &client.client_key,
                device_type: &client.device_type,
                msg: "用户已停用",
            },
        );
        return Err(AppError::Auth("用户已停用".to_string()));
    }

    // 5. 锁定门:达到阈值则拒绝,不再校验密码
    let err_count = read_pwd_err_count(&state, username).await;
    if is_locked(err_count, LOGIN_RETRY_THRESHOLD) {
        record_login_fail(
            &monitor_state,
            LoginFailCtx {
                username: username.to_string(),
                ipaddr: &ipaddr,
                browser: &browser,
                os_str: &os_str,
                client_key: &client.client_key,
                device_type: &client.device_type,
                msg: "账号已锁定",
            },
        );
        return Err(AppError::Auth(
            "账号已锁定,请稍后再试或联系管理员".to_string(),
        ));
    }

    // 6. 验证密码 (BCrypt)
    if !bcrypt::verify(password.as_bytes(), &user.password).unwrap_or(false) {
        tracing::warn!(username = %username, "密码错误");
        let remaining = bump_pwd_err_count(&state, username).await;
        let msg = if remaining == 0 {
            "密码错误,账号已锁定".to_string()
        } else {
            format!("密码错误,剩余重试次数 {}", remaining)
        };
        record_login_fail(
            &monitor_state,
            LoginFailCtx {
                username: username.to_string(),
                ipaddr: &ipaddr,
                browser: &browser,
                os_str: &os_str,
                client_key: &client.client_key,
                device_type: &client.device_type,
                msg: &msg,
            },
        );
        return Err(AppError::Auth("用户或密码错误".to_string()));
    }

    // 7. 计算权限和角色
    // 登录路径无请求身份(同上 user 查找),复用 login_ctx;读为豁免方法
    //(SysMenuMapper / SysRoleMapper.selectRolesByUserId 零 @DataPermission),
    // ctx 形态不影响结果。
    let user_id = user.user_id.unwrap_or(0);
    let permissions = state
        .services
        .permission_service
        .get_menu_permission(&login_ctx, user_id)
        .await?;
    let roles = state
        .services
        .permission_service
        .get_role_permission(&login_ctx, user_id)
        .await?;

    // 8. 构建 LoginUser
    let login_user = LoginUser {
        user_id,
        dept_id: user.dept_id,
        username: user.user_name.clone(),
        nick_name: Some(user.nick_name.clone()),
        user_type: user.user_type.clone(),
        client_id: payload.client_id.clone(),
        client_key: Some(client.client_key.clone()),
        device_type: client.device_type.clone(),
        permissions,
        roles,
        login_ip: Some(ipaddr.clone()),
        login_date: Some(chrono::Utc::now()),
    };

    // 9. 创建 token session
    let ttl = client.timeout as u64;
    let token_value = state
        .services
        .token_service
        .create_session(&login_user, ttl)
        .await?;

    // 登录成功:清零失败计数
    clear_pwd_err_count(&state, username).await;

    tracing::info!(
        user_id = %user_id,
        username = %username,
        "登录成功"
    );

    // 10. 记录登录成功日志(含 ip/browser/os)
    let info = LoginInfo::new(
        username.to_string(),
        "0".to_string(),
        "登录成功".to_string(),
        ipaddr,
        browser,
        os_str,
        Some(client.client_key.clone()),
        client.device_type.clone(),
    );
    let login_info_svc = monitor_state.services.login_info_service.clone();
    tokio::spawn(async move {
        // 无会话路径(登录成功,会话刚建立):system_context,审计人=系统;
        // sys_login_info 无审计列/软删列/范围列,与用户 ctx 行为逐点一致。
        let ctx = login_info_svc.system_context();
        if let Err(e) = login_info_svc.record(&ctx, &info).await {
            tracing::warn!(error = %e, "异步记录登录日志失败");
        }
    });

    // 11. 返回 LoginVo
    Ok(R::ok_with_data(LoginVo {
        access_token: token_value,
        expire_in: ttl,
        client_id: payload.client_id,
        refresh_token: None,
        refresh_expire_in: None,
        scope: None,
        openid: None,
    }))
}

/// 读取当前登录失败次数(Redis 不可用时返回 0,不阻塞登录)
async fn read_pwd_err_count(state: &AppState, username: &str) -> u32 {
    let key = format!("{}{}", PWD_ERR_CNT_PREFIX, username);
    let Ok(mut conn) = state.infra.pool.redis_pool.get().await else {
        return 0;
    };
    redis::cmd("GET")
        .arg(&key)
        .query_async::<Option<String>>(&mut *conn)
        .await
        .ok()
        .flatten()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0)
}

/// 累加登录失败计数,返回剩余可用重试次数(0 表示已锁定)。
/// Redis 失败或不可用时不阻塞登录流程,返回阈值 - 1。
async fn bump_pwd_err_count(state: &AppState, username: &str) -> u32 {
    let key = format!("{}{}", PWD_ERR_CNT_PREFIX, username);
    let Ok(mut conn) = state.infra.pool.redis_pool.get().await else {
        return LOGIN_RETRY_THRESHOLD.saturating_sub(1);
    };
    let count: u32 = redis::cmd("INCR")
        .arg(&key)
        .query_async(&mut *conn)
        .await
        .unwrap_or(0);
    // 首次失败时设置过期(短 TTL,例如 10 分钟)
    if count == 1 {
        let _: () = redis::cmd("EXPIRE")
            .arg(&key)
            .arg(600_i32)
            .query_async(&mut *conn)
            .await
            .unwrap_or(());
    }
    LOGIN_RETRY_THRESHOLD.saturating_sub(count)
}

/// 登录成功时清零失败计数(忽略 Redis 错误)。
async fn clear_pwd_err_count(state: &AppState, username: &str) {
    let key = format!("{}{}", PWD_ERR_CNT_PREFIX, username);
    if let Ok(mut conn) = state.infra.pool.redis_pool.get().await {
        let _: () = redis::cmd("DEL")
            .arg(&key)
            .query_async(&mut *conn)
            .await
            .unwrap_or(());
    }
}

/// 登录失败上下文(用于 `record_login_fail`,避免参数过多)。
struct LoginFailCtx<'a> {
    username: String,
    ipaddr: &'a str,
    browser: &'a str,
    os_str: &'a str,
    client_key: &'a str,
    device_type: &'a Option<String>,
    msg: &'a str,
}

/// 记录登录失败日志(异步,不阻塞登录返回)。
fn record_login_fail(monitor_state: &OperLogState, ctx: LoginFailCtx<'_>) {
    let info = LoginInfo::new(
        ctx.username,
        "1".to_string(), // status=1 失败
        format!("登录失败: {}", ctx.msg),
        ctx.ipaddr.to_string(),
        ctx.browser.to_string(),
        ctx.os_str.to_string(),
        Some(ctx.client_key.to_string()),
        ctx.device_type.clone(),
    );
    let svc = monitor_state.services.login_info_service.clone();
    tokio::spawn(async move {
        // 无会话路径(登录失败,会话尚未建立):system_context,审计人=系统;
        // sys_login_info 无审计列/软删列/范围列,与用户 ctx 行为逐点一致。
        let ctx = svc.system_context();
        if let Err(e) = svc.record(&ctx, &info).await {
            tracing::warn!(error = %e, "异步记录登录失败日志失败");
        }
    });
}

async fn validate_captcha(
    state: &AppState,
    uuid: Option<&str>,
    code: Option<&str>,
) -> Result<(), AppError> {
    if !state.infra.captcha_enabled {
        return Ok(());
    }

    let uuid = uuid
        .filter(|s| !s.is_empty())
        .ok_or_else(|| AppError::Auth("验证码UUID不能为空".to_string()))?;
    let code = code
        .filter(|s| !s.is_empty())
        .ok_or_else(|| AppError::Auth("验证码不能为空".to_string()))?;

    let redis_key = format!("global:captcha_codes:{}", uuid);
    let mut conn = state
        .infra
        .pool
        .redis_pool
        .get()
        .await
        .map_err(|e| AppError::Internal(format!("Redis连接失败: {}", e)))?;

    let stored: Option<String> = redis::cmd("GET")
        .arg(&redis_key)
        .query_async(&mut *conn)
        .await
        .map_err(|e| {
            AppError::Internal(format!("Redis读取验证码失败: {}", e))
        })?;

    let stored = stored
        .ok_or_else(|| AppError::Auth("验证码已过期或不存在".to_string()))?;

    if !stored.eq_ignore_ascii_case(code) {
        return Err(AppError::Auth("验证码错误".to_string()));
    }

    redis::cmd("DEL")
        .arg(&redis_key)
        .exec_async(&mut *conn)
        .await
        .map_err(|e| {
            AppError::Internal(format!("Redis删除验证码失败: {}", e))
        })?;

    Ok(())
}

#[tracing::instrument(skip(state, monitor_state))]
pub async fn logout(
    State((state, monitor_state)): State<(Arc<AppState>, Arc<OperLogState>)>,
    headers: axum::http::HeaderMap,
) -> Result<R<()>, AppError> {
    if let Some(token) = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|auth| auth.strip_prefix("Bearer "))
        .map(|t| t.to_string())
    {
        // 提取客户端信息(登出日志)
        let ipaddr = client_info::extract_client_ip(&headers);
        let ua = headers
            .get(axum::http::header::USER_AGENT)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        let (browser, os_str) = client_info::parse_user_agent(ua);

        let session_info = state
            .services
            .token_service
            .get_session(&token)
            .await
            .ok()
            .flatten();

        if let Err(e) =
            state.services.token_service.revoke_session(&token).await
        {
            tracing::warn!(error = %e, "删除 token session 失败 (登出)");
        } else {
            tracing::info!("用户登出成功");
        }

        // 记录登出日志(异步,含 ip/browser/os)
        if let Some(login_user) = session_info {
            let info = LoginInfo::new(
                login_user.username.clone(),
                "0".to_string(),
                "登出成功".to_string(),
                ipaddr,
                browser,
                os_str,
                login_user.client_key,
                login_user.device_type,
            );
            let login_info_svc =
                monitor_state.services.login_info_service.clone();
            tokio::spawn(async move {
                // 无会话路径(登出,会话已随 revoke 销毁):system_context,
                // 审计人=系统;sys_login_info 无审计列/软删列/范围列,
                // 与用户 ctx 行为逐点一致。
                let ctx = login_info_svc.system_context();
                if let Err(e) = login_info_svc.record(&ctx, &info).await {
                    tracing::warn!(error = %e, "异步记录登出日志失败");
                }
            });
        }
    }

    Ok(R::ok())
}

#[tracing::instrument(skip(payload, state, monitor_state))]
pub async fn register(
    State((state, monitor_state)): State<(Arc<AppState>, Arc<OperLogState>)>,
    headers: axum::http::HeaderMap,
    AppJson(payload): AppJson<RegisterBody>,
) -> Result<R<()>, AppError> {
    payload.validate()?;

    // 注册开关 —— 未开放注册直接拒
    // 注册路径无请求身份 → system_context(ctx 提前建一次,下方注册调用复用)。
    let reg_ds = state.infra.data_source();
    let reg_ctx = reg_ds.system_context();
    let switch = state
        .services
        .sys_config_service
        .get_value_or_default(&reg_ctx, "sys.account.registerUser", "false")
        .await;
    if switch.trim() != "true" {
        return Err(AppError::BadRequest("当前系统未开放注册".to_string()));
    }

    validate_captcha(&state, payload.uuid.as_deref(), payload.code.as_deref())
        .await?;

    if let Some(ref confirm) = payload.confirm_password
        && confirm != &payload.password
    {
        return Err(AppError::BadRequest("两次输入的密码不一致".to_string()));
    }

    let password = hash_password(&payload.password).map_err(|e| {
        AppError::Internal(format!("注册失败: 密码哈希失败: {}", e))
    })?;

    let username = payload.username.clone();

    let user = User {
        user_id: Some(snowflake::next_id()),
        dept_id: None,
        user_name: payload.username,
        nick_name: username.clone(),
        password,
        user_type: Some("sys_user".to_string()),
        email: None,
        phone_number: None,
        gender: None,
        avatar: None,
        status: "0".to_string(),
        del_flag: Some("0".to_string()),
        login_ip: None,
        login_date: None,
        remark: Some("自主注册".to_string()),
        base_entity: BaseEntity::new(0),
    };

    // 注册路径无请求身份 → system_context(审计人=0,与上面 User 构造里
    // 的 BaseEntity::new(0) 同口径);reg_ctx 在开关校验处已建,此处复用。
    state.services.user_service.register(&reg_ctx, user).await?;

    // 记录注册日志(状态=REGISTER)
    let ipaddr = client_info::extract_client_ip(&headers);
    let ua = headers
        .get(axum::http::header::USER_AGENT)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    let (browser, os_str) = client_info::parse_user_agent(ua);
    let info = LoginInfo::new(
        username,
        "0".to_string(),
        "注册成功".to_string(),
        ipaddr,
        browser,
        os_str,
        None,
        None,
    );
    let login_info_svc = monitor_state.services.login_info_service.clone();
    tokio::spawn(async move {
        // 无会话路径(注册,公开接口无会话):system_context,审计人=系统;
        // sys_login_info 无审计列/软删列/范围列,与用户 ctx 行为逐点一致。
        let ctx = login_info_svc.system_context();
        if let Err(e) = login_info_svc.record(&ctx, &info).await {
            tracing::warn!(error = %e, "异步记录注册日志失败");
        }
    });

    Ok(R::ok())
}

#[tracing::instrument(skip(state, _monitor_state))]
pub async fn get_captcha(
    State((state, _monitor_state)): State<(Arc<AppState>, Arc<OperLogState>)>,
) -> Result<R<CaptchaVo>, AppError> {
    let captcha_enabled = state.infra.captcha_enabled;

    if !captcha_enabled {
        return Ok(R::ok_with_data(CaptchaVo {
            captcha_enabled: false,
            uuid: None,
            img: None,
        }));
    }

    let uuid = captcha::generate_uuid();
    let (expression, answer) = captcha::generate_math_captcha();

    let redis_key = format!("global:captcha_codes:{}", uuid);
    let mut conn = state
        .infra
        .pool
        .redis_pool
        .get()
        .await
        .map_err(|e| AppError::Internal(format!("Redis连接失败: {}", e)))?;

    redis::cmd("SET")
        .arg(&redis_key)
        .arg(&answer)
        .arg("EX")
        .arg(120i32)
        .exec_async(&mut *conn)
        .await
        .map_err(|e| {
            AppError::Internal(format!("Redis存储验证码失败: {}", e))
        })?;

    // 渲染为 PNG 后返回裸 base64；前端会自行拼接 data:image/gif;base64, 前缀。
    let img = captcha::render_png_base64(&expression);

    Ok(R::ok_with_data(CaptchaVo {
        captcha_enabled: true,
        uuid: Some(uuid),
        img: Some(img),
    }))
}

#[tracing::instrument(skip(_state, _monitor_state))]
pub async fn social_callback_stub(
    State((_state, _monitor_state)): State<(Arc<AppState>, Arc<OperLogState>)>,
) -> Result<R<()>, AppError> {
    Ok(R::error_with_code_and_message(
        501,
        "未启用社交登录".to_string(),
    ))
}

#[tracing::instrument(skip(_state, _monitor_state))]
pub async fn social_bind_stub(
    State((_state, _monitor_state)): State<(Arc<AppState>, Arc<OperLogState>)>,
) -> Result<R<()>, AppError> {
    Ok(R::error_with_code_and_message(
        501,
        "未启用社交登录".to_string(),
    ))
}

#[tracing::instrument(skip(state), fields(user_id = %login_user.user_id, social_id = id))]
pub async fn unlock_social(
    State(state): State<Arc<AppState>>,
    CurrentUser(login_user): CurrentUser,
    Path(id): Path<i64>,
) -> Result<R<()>, AppError> {
    // ctx 按请求身份构造(social 全豁免,软删写的存活过滤与审计由漏斗施加)。
    let ds = state.infra.data_source();
    let ctx = ds.context(app_data::Principal::from(&login_user));
    state
        .services
        .social_service
        .delete_by_id_for_user(&ctx, id, login_user.user_id)
        .await?;
    Ok(R::ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lock_gate_blocks_at_threshold() {
        assert!(!is_locked(4, 5));
        assert!(is_locked(5, 5));
        assert!(is_locked(6, 5));
    }
}
