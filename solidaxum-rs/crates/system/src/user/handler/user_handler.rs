use std::sync::Arc;

use app_core::{
    error::AppError,
    extract::AppJson,
    middleware::extractors::CurrentUser,
    util::excel::ExcelBuilder,
    vo::{
        page::{PageQuery, TableDataInfo},
        response::R,
    },
};
use axum::{
    body::Body,
    extract::{Form, Multipart, Path, Query, State},
    http::{HeaderMap, HeaderValue, header},
};
use serde::Deserialize;
use validator::Validate;

use crate::{
    app::AppState,
    dept::model::dept::DeptQueryBo,
    permission::guard,
    user::{
        model::user::{
            UserChangeStatusBo, UserCreateBo, UserExportRow, UserImportRow,
            UserInfo, UserListVo, UserResetPwdBo, UserUpdateBo,
            UserUpdatePasswordBo,
        },
        service::user_service::UserService,
    },
};

#[derive(Debug, Clone, Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct UserPageQuery {
    #[serde(flatten)]
    #[validate(nested)]
    pub page: PageQuery,
    pub dept_id: Option<i64>,
    pub user_name: Option<String>,
    pub nick_name: Option<String>,
    pub phone_number: Option<String>,
    pub status: Option<String>,
    #[serde(rename = "params[beginTime]")]
    pub begin_time: Option<String>,
    #[serde(rename = "params[endTime]")]
    pub end_time: Option<String>,
}

#[tracing::instrument(skip(state))]
pub async fn list_user(
    State(state): State<Arc<AppState>>,
    CurrentUser(login_user): CurrentUser,
) -> Result<R<Vec<UserListVo>>, AppError> {
    guard::require_permission(&login_user, "system:user:list")?;
    // ctx 请求期构造(post 模块同款)—— infra.data_source() 此刻必带
    // resolver;绑定 Arc 局部变量再借出,避免临时值悬垂。范围由漏斗按
    // sys_user 的 DataPolicy 自动施加(Java selectUserList 带注解)。
    let ds = state.infra.data_source();
    let ctx = ds.context(app_data::Principal::from(&login_user));
    let data = state.services.user_service.list(&ctx).await?;
    Ok(R::ok_with_data(data))
}

#[tracing::instrument(skip(state), fields(page = query.page.page_num, size = query.page.page_size))]
pub async fn list_user_page(
    State(state): State<Arc<AppState>>,
    CurrentUser(login_user): CurrentUser,
    Query(query): Query<UserPageQuery>,
) -> Result<R<TableDataInfo<UserListVo>>, AppError> {
    guard::require_permission(&login_user, "system:user:list")?;
    // 范围移交漏斗(Java selectPageUserList 带注解,自动施加)
    let ds = state.infra.data_source();
    let ctx = ds.context(app_data::Principal::from(&login_user));
    let mut result = state.services.user_service.page(&ctx, query).await?;
    let dept_ids: Vec<i64> =
        result.rows.iter().filter_map(|r| r.dept_id).collect();
    if !dept_ids.is_empty() {
        let name_map = state
            .services
            .translation_service
            .dept_id_to_name(&ctx, &dept_ids)
            .await?;
        for row in &mut result.rows {
            if let Some(did) = row.dept_id {
                row.dept_name = name_map.get(&did).cloned();
            }
        }
    }
    let create_by_ids: Vec<i64> = result
        .rows
        .iter()
        .filter_map(|r| r.base_vo.create_by)
        .collect();
    if !create_by_ids.is_empty() {
        let user_map = state
            .services
            .translation_service
            .user_id_to_name(&ctx, &create_by_ids)
            .await?;
        for row in &mut result.rows {
            if let Some(cb) = row.base_vo.create_by {
                row.create_by_name = user_map.get(&cb).cloned();
            }
        }
    }
    Ok(R::ok_with_data(result))
}

#[tracing::instrument(skip(state), fields(username = %current_user.0.username))]
pub async fn get_user_info(
    State(state): State<Arc<AppState>>,
    current_user: CurrentUser,
) -> Result<R<UserInfo>, AppError> {
    let ds = state.infra.data_source();
    let ctx = ds.context(app_data::Principal::from(&current_user.0));
    let user = state
        .services
        .user_service
        .get_user_by_user_name(&ctx, &current_user.0.username)
        .await?;
    Ok(R::ok_with_data(UserInfo {
        user_id: user.user_id.unwrap_or_default(),
        username: user.user_name,
    }))
}

#[tracing::instrument(skip(state), fields(user_id = id))]
pub async fn get_user_by_id(
    State(state): State<Arc<AppState>>,
    current_user: CurrentUser,
    Path(id): Path<i64>,
) -> Result<R<crate::user::model::user::SysUserInfoVo>, AppError> {
    guard::require_permission(&current_user.0, "system:user:query")?;
    let ds = state.infra.data_source();
    let ctx = ds.context(app_data::Principal::from(&current_user.0));
    // 读路径越权判定(Java getInfo → checkUserDataScope,Mapper.java:130-134
    // 的带注解 countUserById):漏斗形态的受辖探针,越权/不存在 → NotFound
    state
        .services
        .user_service
        .check_user_data_scope(&ctx, id)
        .await?;
    let mut info = state.services.user_service.get_user_info(&ctx, id).await?;
    // 头像 OSS ID → URL（对齐 RuoYi 6.X avatarUrl 翻译）
    info.user.avatar_url = state
        .services
        .translation_service
        .oss_id_to_url_single(&ctx, info.user.avatar)
        .await?;
    if let Some(dept_id) = info.user.dept_id {
        let name_map = state
            .services
            .translation_service
            .dept_id_to_name(&ctx, &[dept_id])
            .await?;
        if let Some(name) = name_map.get(&dept_id) {
            info.user.dept_name = Some(name.clone());
            info.user.dept = Some(crate::user::model::user::DeptBrief {
                dept_id,
                dept_name: name.clone(),
            });
        }
    }
    let role_briefs: Vec<crate::user::model::user::RoleBrief> = state
        .services
        .role_service
        .list(&ctx)
        .await?
        .into_iter()
        .map(|r| crate::user::model::user::RoleBrief {
            role_id: r.role_id,
            role_name: r.role_name,
            role_key: r.role_key,
            status: r.status,
        })
        .collect();
    info.user.roles = role_briefs
        .iter()
        .filter(|rb| info.role_ids.contains(&rb.role_id))
        .cloned()
        .collect();
    info.roles = role_briefs;
    // post_service.list 是 Java 豁免方法(selectPostAll 无 @DataPermission),
    // repo 内部 without_row_scope;此处传用户 ctx 保留软删/审计口径。
    let all_posts = state.services.post_service.list(&ctx).await?;
    info.posts = all_posts
        .into_iter()
        .map(|p| crate::user::model::user::PostBrief {
            post_id: p.post_id,
            post_name: p.post_name,
            post_code: p.post_code,
            status: p.status,
        })
        .collect();
    Ok(R::ok_with_data(info))
}

/// 新建用户表单初始化数据(GET `/system/user/`,无 userId)。
/// 对齐 Java `SysUserController#getInfo` 的"新增"分支:返回全部可选
/// 角色/岗位,user 为空。
#[tracing::instrument(skip(state))]
pub async fn get_add_user_info(
    State(state): State<Arc<AppState>>,
    current_user: CurrentUser,
) -> Result<R<serde_json::Value>, AppError> {
    guard::require_permission(&current_user.0, "system:user:query")?;
    let ds = state.infra.data_source();
    let ctx = ds.context(app_data::Principal::from(&current_user.0));
    let roles: Vec<crate::user::model::user::RoleBrief> = state
        .services
        .role_service
        .list(&ctx)
        .await?
        .into_iter()
        .map(|r| crate::user::model::user::RoleBrief {
            role_id: r.role_id,
            role_name: r.role_name,
            role_key: r.role_key,
            status: r.status,
        })
        .collect();
    // post_service.list 是 Java 豁免方法(selectPostAll 无 @DataPermission),
    // 传用户 ctx 保留软删/审计口径。
    let posts: Vec<crate::user::model::user::PostBrief> = state
        .services
        .post_service
        .list(&ctx)
        .await?
        .into_iter()
        .map(|p| crate::user::model::user::PostBrief {
            post_id: p.post_id,
            post_name: p.post_name,
            post_code: p.post_code,
            status: p.status,
        })
        .collect();
    Ok(R::ok_with_data(serde_json::json!({
        "user": serde_json::Value::Null,
        "roleIds": [],
        "postIds": [],
        "roles": roles,
        "posts": posts,
    })))
}

#[tracing::instrument(
    skip(state, bo),
    fields(username = %current_user.0.username, new_user_name = ?bo.user_name)
)]
pub async fn create_user(
    State(state): State<Arc<AppState>>,
    current_user: CurrentUser,
    AppJson(bo): AppJson<UserCreateBo>,
) -> Result<R<()>, AppError> {
    guard::require_permission(&current_user.0, "system:user:add")?;
    bo.validate()?;
    let ds = state.infra.data_source();
    let ctx = ds.context(app_data::Principal::from(&current_user.0));
    // 部门维度越权(Java add:177 → checkDeptDataScope(user.getDeptId())):
    // 目标部门经用户数据范围过滤不可见 → 拒。探针在唯一性校验之前,
    // 同 Java controller 顺序。
    state
        .services
        .dept_service
        .check_dept_data_scope(&ctx, bo.dept_id)
        .await?;
    state
        .services
        .user_service
        .insert_by_bo(&ctx, bo, &current_user)
        .await?;
    Ok(R::ok())
}

#[tracing::instrument(
    skip(state, bo),
    fields(operator = %current_user.0.username, target_user_id = ?bo.user_id)
)]
pub async fn update_user(
    State(state): State<Arc<AppState>>,
    current_user: CurrentUser,
    AppJson(bo): AppJson<UserUpdateBo>,
) -> Result<R<()>, AppError> {
    guard::require_permission(&current_user.0, "system:user:edit")?;
    bo.validate()?;
    let user_id = bo
        .user_id
        .ok_or_else(|| AppError::BadRequest("用户ID为空".to_string()))?;
    let ds = state.infra.data_source();
    let ctx = ds.context(app_data::Principal::from(&current_user.0));
    // Java edit:200-201:checkUserAllowed 先于 checkUserDataScope ——
    // 受限操作者打超管靶标先回 400「不允许操作超级管理员」,而非探针的
    // 404。service 内 update_by_bo 有同款调用,此处前置对齐 Java 顺序
    // (双查与 role_id 双重校验同款)。
    UserService::check_user_allowed(user_id)?;
    // Java edit:201 checkUserDataScope(user.getUserId()):用户维度的
    // 越权拦截**只能**靠这道探针。
    state
        .services
        .user_service
        .check_user_data_scope(&ctx, user_id)
        .await?;
    // 部门维度越权(Java edit:202 → checkDeptDataScope(user.getDeptId())):
    // 与 user 维度不同 —— 上面的用户探针锚的是**旧行**,把用户改挂到
    // 越权部门不会被它拦住,目标部门必须显式探针。
    state
        .services
        .dept_service
        .check_dept_data_scope(&ctx, bo.dept_id)
        .await?;
    state
        .services
        .user_service
        .update_by_bo(&ctx, bo, &current_user)
        .await?;
    Ok(R::ok())
}

#[tracing::instrument(
    skip(state),
    fields(operator = %current_user.0.username, user_ids = ids)
)]
pub async fn delete_user(
    State(state): State<Arc<AppState>>,
    current_user: CurrentUser,
    Path(ids): Path<String>,
) -> Result<R<()>, AppError> {
    guard::require_permission(&current_user.0, "system:user:remove")?;
    let mut id_list: Vec<i64> = ids
        .split(',')
        .map(|s| {
            s.trim().parse::<i64>().map_err(|_| {
                AppError::BadRequest(format!("无效的用户ID: {}", s))
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    if id_list.is_empty() {
        return Err(AppError::BadRequest("无效的用户ID".into()));
    }
    id_list.sort();
    id_list.dedup();
    let ds = state.infra.data_source();
    let ctx = ds.context(app_data::Principal::from(&current_user.0));
    for id in id_list {
        if id == current_user.0.user_id {
            return Err(AppError::BadRequest("不能删除当前登录用户".into()));
        }
        // 越权拦截在 service::delete_by_id 内的 check_user_data_scope ——
        // Java 的软删探针在 service 层(SysUserServiceImpl
        // .deleteUserByIds:531-534),controller remove:221-228 只有
        // 「不能删自己」。此处不重复探针,与 Java 的分层一致。
        state
            .services
            .user_service
            .delete_by_id(&ctx, id, &current_user)
            .await?;
    }
    Ok(R::ok())
}

#[tracing::instrument(
    skip(state, bo),
    fields(operator = %current_user.0.username, user_id = ?bo.user_id)
)]
pub async fn update_password(
    State(state): State<Arc<AppState>>,
    current_user: CurrentUser,
    AppJson(bo): AppJson<UserUpdatePasswordBo>,
) -> Result<R<()>, AppError> {
    guard::require_permission(&current_user.0, "system:user:resetPwd")?;
    bo.validate()?;
    let user_id = bo
        .user_id
        .ok_or_else(|| AppError::BadRequest("用户ID为空".to_string()))?;
    let ds = state.infra.data_source();
    let ctx = ds.context(app_data::Principal::from(&current_user.0));
    // Java resetPwd:255 checkUserAllowed 先于 :256 checkUserDataScope;
    // Rust 双端点(/system/user/resetPwd 与 /system/user/password)共用
    // system:user:resetPwd 权限,锚同一 Java 探针对,顺序一致。
    // service 内 update_password 有同款调用,此处前置对齐 Java(双查)。
    UserService::check_user_allowed(user_id)?;
    // Java resetPwd:256 checkUserDataScope:越权拦截由这道探针承担
    state
        .services
        .user_service
        .check_user_data_scope(&ctx, user_id)
        .await?;
    state
        .services
        .user_service
        .update_password(&ctx, user_id, bo.password)
        .await?;
    Ok(R::ok())
}

#[tracing::instrument(skip(state), fields(username = %login_user.username))]
pub async fn get_info(
    State(state): State<Arc<AppState>>,
    CurrentUser(login_user): CurrentUser,
) -> Result<R<serde_json::Value>, AppError> {
    let ds = state.infra.data_source();
    let ctx = ds.context(app_data::Principal::from(&login_user));
    let user = state
        .services
        .user_service
        .get_user_by_id(&ctx, login_user.user_id)
        .await?;

    let permissions: Vec<String> =
        login_user.permissions.iter().cloned().collect();
    let roles: Vec<String> = login_user.roles.iter().cloned().collect();

    let avatar_url = state
        .services
        .translation_service
        .oss_id_to_url_single(&ctx, user.avatar)
        .await?;

    // 对齐 Java SysUserVo.deptName(@Translation DEPT_ID_TO_NAME)
    let dept_name = match user.dept_id {
        Some(dept_id) => {
            let mut map = state
                .services
                .translation_service
                .dept_id_to_name(&ctx, &[dept_id])
                .await?;
            map.remove(&dept_id)
        }
        None => None,
    };

    // 对齐 Java SysUserVo.roles(List<SysRoleVo>)。
    // 注意:顶层 roles 仍是角色 key 集合,两者并存,不可互相顶替。
    // (selectRolesByUserId 是豁免方法,ctx 只承载软删过滤。)
    let user_roles = state
        .services
        .role_service
        .get_roles_by_user_id(&ctx, login_user.user_id)
        .await?;

    let user_vo = serde_json::json!({
        "userId": user.user_id,
        "deptId": user.dept_id,
        "deptName": dept_name,
        "userName": user.user_name,
        "nickName": user.nick_name,
        "userType": user.user_type,
        "email": user.email,
        "phoneNumber": user.phone_number,
        "gender": user.gender,
        "avatar": user.avatar,
        "avatarUrl": avatar_url,
        "status": user.status,
        "loginIp": user.login_ip,
        "loginDate": user.login_date,
        "remark": user.remark,
        "roles": user_roles,
        "createTime": user.base_entity.create_time,
        "updateTime": user.base_entity.update_time,
    });

    Ok(R::ok_with_data(serde_json::json!({
        "user": user_vo,
        "roles": roles,
        "permissions": permissions,
    })))
}

/// 获取用户已分配的角色列表
#[tracing::instrument(skip(state), fields(user_id = user_id))]
pub async fn get_auth_role(
    State(state): State<Arc<AppState>>,
    CurrentUser(login_user): CurrentUser,
    Path(user_id): Path<i64>,
) -> Result<R<serde_json::Value>, AppError> {
    guard::require_permission(&login_user, "system:user:query")?;
    let ds = state.infra.data_source();
    let ctx = ds.context(app_data::Principal::from(&login_user));
    // Java authRole GET → checkUserDataScope(读路径,带注解探针)
    state
        .services
        .user_service
        .check_user_data_scope(&ctx, user_id)
        .await?;
    let user = state
        .services
        .user_service
        .get_user_by_id(&ctx, user_id)
        .await?;
    // 获取所有角色(受辖读走用户 ctx —— 对齐
    // Java authRole → selectRoleAll → selectRoleList Mapper:47 带注解)
    let all_roles = state.services.role_service.list(&ctx).await?;
    // 标记用户已分配的角色(豁免方法,ctx 只承载软删过滤)
    let user_roles = state
        .services
        .role_service
        .get_roles_by_user_id(&ctx, user_id)
        .await?;
    let assigned: std::collections::HashSet<i64> =
        user_roles.iter().map(|r| r.role_id).collect();
    let roles: Vec<serde_json::Value> = all_roles
        .into_iter()
        .map(|r| {
            serde_json::json!({
                "roleId": r.role_id,
                "roleName": r.role_name,
                "roleKey": r.role_key,
                "status": r.status,
                "createTime": r.base_vo.create_time,
                "flag": assigned.contains(&r.role_id),
            })
        })
        .collect();
    Ok(R::ok_with_data(serde_json::json!({
        "roles": roles,
        "user": {
            "userId": user.user_id.unwrap_or_default(),
            "userName": user.user_name,
            "nickName": user.nick_name,
        },
    })))
}

/// 分配用户角色 (query params: userId, roleIds)
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthRoleQuery {
    pub user_id: Option<i64>,
    pub role_ids: Option<String>,
}

#[tracing::instrument(skip(state), fields(operator = %login_user.username))]
pub async fn auth_role_assign(
    State(state): State<Arc<AppState>>,
    CurrentUser(login_user): CurrentUser,
    Query(query): Query<AuthRoleQuery>,
) -> Result<R<()>, AppError> {
    guard::require_permission(&login_user, "system:user:edit")?;
    let user_id = query
        .user_id
        .ok_or_else(|| AppError::BadRequest("userId为空".to_string()))?;
    let ds = state.infra.data_source();
    let ctx = ds.context(app_data::Principal::from(&login_user));
    // Java authRole PUT → checkUserDataScope:关联表(sys_user_role)本身
    // 豁免(无 @DataPermission),越权拦截**只能**靠这道探针。
    state
        .services
        .user_service
        .check_user_data_scope(&ctx, user_id)
        .await?;
    let role_ids: Vec<i64> = query
        .role_ids
        .as_deref()
        .unwrap_or("")
        .split(',')
        .filter_map(|s| s.trim().parse().ok())
        .collect();
    state
        .services
        .user_service
        .insert_user_auth(&ctx, user_id, &role_ids)
        .await?;
    Ok(R::ok())
}

/// 重置密码 (Vue 契约路径 /system/user/resetPwd)
#[tracing::instrument(
    skip(state, bo),
    fields(operator = %current_user.0.username, user_id = ?bo.user_id)
)]
pub async fn reset_password(
    State(state): State<Arc<AppState>>,
    current_user: CurrentUser,
    AppJson(bo): AppJson<UserResetPwdBo>,
) -> Result<R<()>, AppError> {
    guard::require_permission(&current_user.0, "system:user:resetPwd")?;
    bo.validate()?;
    let user_id = bo
        .user_id
        .ok_or_else(|| AppError::BadRequest("用户ID为空".to_string()))?;
    let ds = state.infra.data_source();
    let ctx = ds.context(app_data::Principal::from(&current_user.0));
    // Java resetPwd:255 checkUserAllowed 先于 :256 checkUserDataScope;
    // Rust 双端点(/system/user/resetPwd 与 /system/user/password)共用
    // system:user:resetPwd 权限,锚同一 Java 探针对,顺序一致。
    // service 内 update_password 有同款调用,此处前置对齐 Java(双查)。
    UserService::check_user_allowed(user_id)?;
    // Java resetPwd:256 checkUserDataScope:越权拦截由这道探针承担
    state
        .services
        .user_service
        .check_user_data_scope(&ctx, user_id)
        .await?;
    state
        .services
        .user_service
        .update_password(&ctx, user_id, bo.password)
        .await?;
    Ok(R::ok())
}

/// 修改用户状态
#[tracing::instrument(
    skip(state, bo),
    fields(operator = %current_user.0.username, user_id = ?bo.user_id)
)]
pub async fn change_status(
    State(state): State<Arc<AppState>>,
    current_user: CurrentUser,
    AppJson(bo): AppJson<UserChangeStatusBo>,
) -> Result<R<()>, AppError> {
    guard::require_permission(&current_user.0, "system:user:edit")?;
    bo.validate()?;
    let user_id = bo
        .user_id
        .ok_or_else(|| AppError::BadRequest("用户ID为空".to_string()))?;
    let ds = state.infra.data_source();
    let ctx = ds.context(app_data::Principal::from(&current_user.0));
    // Java changeStatus:272 checkUserAllowed 先于 :273
    // checkUserDataScope;Java service 侧 updateUserStatus 无此守卫,
    // 此处是**唯一**一道(Rust service::change_status 同样不设)。
    UserService::check_user_allowed(user_id)?;
    // Java changeStatus:273 checkUserDataScope:越权拦截由这道探针承担
    state
        .services
        .user_service
        .check_user_data_scope(&ctx, user_id)
        .await?;
    state
        .services
        .user_service
        .change_status(&ctx, user_id, bo.status)
        .await?;
    Ok(R::ok())
}

/// 解锁用户账号
#[tracing::instrument(skip(state), fields(user_id = id))]
pub async fn unlock(
    State(state): State<Arc<AppState>>,
    current_user: CurrentUser,
    Path(id): Path<i64>,
) -> Result<R<()>, AppError> {
    guard::require_permission(&current_user.0, "system:user:edit")?;
    let ds = state.infra.data_source();
    let ctx = ds.context(app_data::Principal::from(&current_user.0));
    // Java unlock/{userId} 无 checkUserDataScope(越权解锁只影响 Redis
    // 计数,不读改他人数据),按 Java 基线不设探针。
    state.services.user_service.unlock(&ctx, id).await?;
    Ok(R::ok())
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserOptionSelectQuery {
    pub user_ids: Option<String>,
    pub dept_id: Option<i64>,
}

fn parse_user_option_ids(raw: &str) -> Result<Vec<i64>, AppError> {
    let mut ids = Vec::new();
    for s in raw.split(',') {
        let trimmed = s.trim();
        if trimmed.is_empty() {
            continue;
        }
        let id: i64 = trimmed.parse().map_err(|_| {
            AppError::BadRequest(format!(
                "用户下拉选择失败: 无效的用户ID: {}",
                trimmed
            ))
        })?;
        ids.push(id);
    }
    Ok(ids)
}

/// 用户下拉选项（对齐 Java `selectUserByIds`）
#[tracing::instrument(skip(state, query))]
pub async fn option_select(
    State(state): State<Arc<AppState>>,
    CurrentUser(login_user): CurrentUser,
    Query(query): Query<UserOptionSelectQuery>,
) -> Result<R<Vec<crate::user::model::user::UserOptionVo>>, AppError> {
    guard::require_permission(&login_user, "system:user:query")?;
    // Java selectUserByIds 走带注解的 selectUserList → 漏斗按 ctx 施加
    // (与 post 的下拉豁免相反,user 的下拉在 Java 是受辖的)。
    let ds = state.infra.data_source();
    let ctx = ds.context(app_data::Principal::from(&login_user));
    let user_ids = query
        .user_ids
        .as_deref()
        .filter(|s| !s.is_empty())
        .map(parse_user_option_ids)
        .transpose()?;
    let data = state
        .services
        .user_service
        .option_select(&ctx, user_ids, query.dept_id)
        .await?;
    Ok(R::ok_with_data(data))
}

/// 用户部门树(复用部门列表,按数据权限过滤)
#[tracing::instrument(skip(state))]
pub async fn dept_tree(
    State(state): State<Arc<AppState>>,
    CurrentUser(login_user): CurrentUser,
) -> Result<R<Vec<crate::dept::model::dept::DeptTreeSelectVo>>, AppError> {
    guard::require_permission(&login_user, "system:user:list")?;
    let ds = state.infra.data_source();
    let ctx = ds.context(app_data::Principal::from(&login_user));
    // Java SysUserController.deptTree(:341-343)→
    // selectDeptTreeList → selectDeptList(SysDeptMapper.java:36 带
    // @DataPermission dept_id)—— 范围经 sys_dept 的漏斗 DataPolicy
    // 自动施加。
    //
    // **行为注意(非直觉)**:sys_dept 的
    // owner_column 是 None,故 SelfOnly 落 scope_condition 的
    // `(None, Some(_))` 分支 → 恒假 `0 = 1` → **空树**;DeptInOrSelf
    // 退化为 dept-only,不含「本人所在部门」并集项。两者都是 Java
    // elseSql `1 = 0` 的平价形态(dept_repo.rs 文件头有完整论证),
    // 行为可接受 —— 仅本人角色的用户看到的是空部门树。
    let depts = state
        .services
        .dept_service
        .list(
            &ctx,
            DeptQueryBo {
                dept_name: None,
                status: None,
                dept_id: None,
                dept_category: None,
            },
        )
        .await?;
    let tree = crate::dept::model::dept::DeptTreeSelectVo::build(&depts);
    Ok(R::ok_with_data(tree))
}

/// 按部门查询用户列表（Vue 契约：裸数组，非分页）
#[tracing::instrument(skip(state), fields(dept_id = dept_id))]
pub async fn list_user_by_dept(
    State(state): State<Arc<AppState>>,
    CurrentUser(login_user): CurrentUser,
    Path(dept_id): Path<i64>,
) -> Result<R<Vec<UserListVo>>, AppError> {
    guard::require_permission(&login_user, "system:user:list")?;
    // Java selectUserListByDept 无注解(lambda().voList)→ repo 内部
    // without_row_scope,该路径本就不受辖;传用户 ctx 保留软删口径。
    let ds = state.infra.data_source();
    let ctx = ds.context(app_data::Principal::from(&login_user));
    let data = state
        .services
        .user_service
        .list_by_dept_id(&ctx, dept_id)
        .await?;
    Ok(R::ok_with_data(data))
}

/// 导入用户(Excel 上传)
#[tracing::instrument(skip(state, current_user, multipart), fields(operator = %current_user.0.username))]
pub async fn import_data(
    State(state): State<Arc<AppState>>,
    current_user: CurrentUser,
    mut multipart: Multipart,
) -> Result<R<String>, AppError> {
    guard::require_permission(&current_user.0, "system:user:import")?;

    let mut file_bytes: Option<Vec<u8>> = None;
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| AppError::BadRequest(format!("读取上传文件失败: {}", e)))?
    {
        let name = field.name().unwrap_or("").to_string();
        if name == "file" {
            let data = field.bytes().await.map_err(|e| {
                AppError::Internal(format!("读取文件内容失败: {}", e))
            })?;
            file_bytes = Some(data.to_vec());
        }
    }

    let data = file_bytes
        .ok_or_else(|| AppError::BadRequest("未找到上传文件".to_string()))?;

    let ds = state.infra.data_source();
    let ctx = ds.context(app_data::Principal::from(&current_user.0));

    // 从配置读取初始密码(直接查 DB,不依赖 Redis;
    // 传操作人的 ctx —— 配置读豁免,ctx 只承载连接与审计语义)
    let init_password = state
        .services
        .sys_config_service
        .get_value_or_default(&ctx, "sys.user.initPassword", "123456")
        .await;

    let result = state
        .services
        .user_service
        .import_data(&ctx, &data, &init_password, &current_user)
        .await?;

    Ok(R::ok_with_data(result))
}

/// 下载导入模板(空数据 + 表头的 xlsx)
#[tracing::instrument]
pub async fn import_template() -> Result<(HeaderMap, Body), AppError> {
    let rows: Vec<UserImportRow> = Vec::new();
    let bytes = ExcelBuilder::<UserImportRow>::export(&rows)?;

    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static(
            "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        ),
    );
    headers.insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_static(
            "attachment; filename=\"user_import_template.xlsx\"",
        ),
    );
    Ok((headers, Body::from(bytes)))
}

/// 导出用户 xlsx POST /system/user/export
/// 对齐 Java SysUserController.export 与本仓 role/post 两个导出端点的形状。
#[tracing::instrument(skip(state, login_user))]
pub async fn export_user(
    State(state): State<Arc<AppState>>,
    CurrentUser(login_user): CurrentUser,
    // Vue download() 发 application/x-www-form-urlencoded(request.ts:259),
    // 故用 Form 提取(与 GET list 的 Query 同走 serde_urlencoded),不能用 AppJson。
    Form(query): Form<UserPageQuery>,
) -> Result<(HeaderMap, Body), AppError> {
    guard::require_permission(&login_user, "system:user:export")?;
    // Java selectUserExportList 带注解(与 post 相反)→ 漏斗按 ctx 施加
    let ds = state.infra.data_source();
    let ctx = ds.context(app_data::Principal::from(&login_user));
    let mut rows = state.services.user_service.list_all(&ctx, &query).await?;

    // 与 list_user_page 一致:批量翻译 dept_id → dept_name,否则导出的"部门"列全空。
    let dept_ids: Vec<i64> = rows.iter().filter_map(|r| r.dept_id).collect();
    if !dept_ids.is_empty() {
        let name_map = state
            .services
            .translation_service
            .dept_id_to_name(&ctx, &dept_ids)
            .await?;
        for row in &mut rows {
            if let Some(did) = row.dept_id {
                row.dept_name = name_map.get(&did).cloned();
            }
        }
    }

    let export_rows: Vec<UserExportRow> =
        rows.iter().map(UserExportRow::from_list_vo).collect();
    let bytes = ExcelBuilder::<UserExportRow>::export(&export_rows)?;

    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static(
            "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        ),
    );
    headers.insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_static("attachment; filename=\"user.xlsx\""),
    );
    Ok((headers, Body::from(bytes)))
}
