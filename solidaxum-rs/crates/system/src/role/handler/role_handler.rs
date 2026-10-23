use std::sync::Arc;

use app_core::{
    error::AppError,
    extract::AppJson,
    middleware::extractors::CurrentUser,
    model::status::Status,
    util::excel::ExcelBuilder,
    vo::{
        page::{PageQuery, TableDataInfo},
        response::R,
    },
};
use axum::{
    body::Body,
    extract::{Form, Path, Query, State},
    http::{HeaderMap, HeaderValue, header},
};
use serde::Deserialize;
use validator::Validate;

use crate::{
    app::AppState,
    permission::guard,
    role::model::role::{
        RoleCreateBo, RoleExportRow, RoleListVo, RoleOptionVo, RoleUpdateBo,
    },
    user::{handler::user_handler::UserPageQuery, model::user::UserListVo},
};

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RolePageQuery {
    #[serde(flatten)]
    pub page: PageQuery,
    pub keyword: Option<String>,
    pub role_name: Option<String>,
    pub role_key: Option<String>,
    pub status: Option<String>,
    #[serde(rename = "params[beginTime]")]
    pub begin_time: Option<String>,
    #[serde(rename = "params[endTime]")]
    pub end_time: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct RoleChangeStatusBo {
    #[validate(required)]
    pub role_id: Option<i64>,
    #[validate(custom(function = "validate_status"))]
    pub status: String,
}

fn validate_status(status: &str) -> Result<(), validator::ValidationError> {
    if status == Status::Normal.as_str() || status == Status::Disabled.as_str()
    {
        Ok(())
    } else {
        let mut err = validator::ValidationError::new("status");
        err.message = Some("状态只能是0或1".into());
        Err(err)
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthUserQuery {
    #[serde(flatten)]
    pub page: PageQuery,
    pub role_id: Option<i64>,
    pub user_name: Option<String>,
    pub phone_number: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthUserCancelBo {
    pub user_id: Option<i64>,
    pub role_id: Option<i64>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthUserBatchQuery {
    pub role_id: Option<i64>,
    pub user_ids: Option<String>,
}

fn parse_user_ids(s: &str) -> Result<Vec<i64>, AppError> {
    let mut ids: Vec<i64> = s
        .split(',')
        .map(|s| {
            s.trim().parse::<i64>().map_err(|_| {
                AppError::BadRequest(format!("无效的用户ID: {}", s))
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    if ids.is_empty() {
        return Err(AppError::BadRequest("无效的用户ID".into()));
    }
    ids.sort();
    ids.dedup();
    Ok(ids)
}

#[tracing::instrument(skip(state))]
pub async fn list_role(
    State(state): State<Arc<AppState>>,
    CurrentUser(login_user): CurrentUser,
) -> Result<R<Vec<RoleListVo>>, AppError> {
    guard::require_permission(&login_user, "system:role:list")?;
    // ctx 请求期构造(post/user 模块同款)—— infra.data_source() 此刻
    // 必带 resolver(见 app.rs 装配顺序注释)。
    let ds = state.infra.data_source();
    let ctx = ds.context(app_data::Principal::from(&login_user));
    let data = state.services.role_service.list(&ctx).await?;
    Ok(R::ok_with_data(data))
}

#[tracing::instrument(skip(state))]
pub async fn list_role_page(
    State(state): State<Arc<AppState>>,
    CurrentUser(login_user): CurrentUser,
    Query(query): Query<RolePageQuery>,
) -> Result<R<TableDataInfo<RoleListVo>>, AppError> {
    guard::require_permission(&login_user, "system:role:list")?;
    let ds = state.infra.data_source();
    let ctx = ds.context(app_data::Principal::from(&login_user));
    let result = state.services.role_service.page(&ctx, &query).await?;
    Ok(R::ok_with_data(result))
}

#[tracing::instrument(skip(state), fields(role_id = id))]
pub async fn get_role_by_id(
    State(state): State<Arc<AppState>>,
    CurrentUser(login_user): CurrentUser,
    Path(id): Path<i64>,
) -> Result<R<RoleListVo>, AppError> {
    guard::require_permission(&login_user, "system:role:query")?;
    let ds = state.infra.data_source();
    let ctx = ds.context(app_data::Principal::from(&login_user));
    // Java SysRoleController.getInfo:83 checkRoleDataScope:越权拦截由这道探针承担
    state
        .services
        .role_service
        .check_role_data_scope(&ctx, id)
        .await?;
    let role = state.services.role_service.get_role_by_id(&ctx, id).await?;
    Ok(R::ok_with_data(role.to_list_vo()))
}

#[tracing::instrument(
    skip(state, bo),
    fields(operator = %current_user.0.username, role_key = ?bo.role_key)
)]
pub async fn create_role(
    State(state): State<Arc<AppState>>,
    current_user: CurrentUser,
    AppJson(bo): AppJson<RoleCreateBo>,
) -> Result<R<()>, AppError> {
    guard::require_permission(&current_user.0, "system:role:add")?;
    bo.validate()?;
    let ds = state.infra.data_source();
    let ctx = ds.context(app_data::Principal::from(&current_user.0));
    state
        .services
        .role_service
        .insert_by_bo(&ctx, bo, &current_user)
        .await?;
    Ok(R::ok())
}

#[tracing::instrument(
    skip(state, bo),
    fields(operator = %current_user.0.username, role_id = ?bo.role_id)
)]
pub async fn update_role(
    State(state): State<Arc<AppState>>,
    current_user: CurrentUser,
    AppJson(bo): AppJson<RoleUpdateBo>,
) -> Result<R<()>, AppError> {
    guard::require_permission(&current_user.0, "system:role:edit")?;
    bo.validate()?;
    let role_id = bo
        .role_id
        .ok_or_else(|| AppError::BadRequest("角色ID为空".to_string()))?;
    let ds = state.infra.data_source();
    let ctx = ds.context(app_data::Principal::from(&current_user.0));
    // Java SysRoleController.edit:120 checkRoleDataScope:越权拦截由这道探针承担
    state
        .services
        .role_service
        .check_role_data_scope(&ctx, role_id)
        .await?;
    state
        .services
        .role_service
        .update_by_bo(&ctx, bo, &current_user)
        .await?;
    Ok(R::ok())
}

#[tracing::instrument(
    skip(state),
    fields(operator = %current_user.0.username, role_ids = ids)
)]
pub async fn delete_role(
    State(state): State<Arc<AppState>>,
    current_user: CurrentUser,
    Path(ids): Path<String>,
) -> Result<R<()>, AppError> {
    guard::require_permission(&current_user.0, "system:role:remove")?;
    let mut id_list: Vec<i64> = ids
        .split(',')
        .map(|s| {
            s.trim().parse::<i64>().map_err(|_| {
                AppError::BadRequest(format!("无效的角色ID: {}", s))
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    if id_list.is_empty() {
        return Err(AppError::BadRequest("无效的角色ID".into()));
    }
    id_list.sort();
    id_list.dedup();
    let ds = state.infra.data_source();
    let ctx = ds.context(app_data::Principal::from(&current_user.0));
    for id in id_list {
        state
            .services
            .role_service
            .delete_by_id(&ctx, id, &current_user)
            .await?;
    }
    Ok(R::ok())
}

/// 更新角色权限 (菜单 + 数据权限)
#[tracing::instrument(skip(state, bo), fields(operator = %current_user.0.username))]
pub async fn update_role_permission(
    State(state): State<Arc<AppState>>,
    current_user: CurrentUser,
    AppJson(bo): AppJson<crate::role::model::role::SysRolePermissionBo>,
) -> Result<R<()>, AppError> {
    guard::require_permission(&current_user.0, "system:role:edit")?;
    bo.validate()?;
    let role_id = bo
        .role_id
        .ok_or_else(|| AppError::BadRequest("roleId为空".to_string()))?;
    let ds = state.infra.data_source();
    let ctx = ds.context(app_data::Principal::from(&current_user.0));
    // Java SysRoleController.editPermission:146 checkRoleDataScope:越权拦截由这道探针承担
    state
        .services
        .role_service
        .check_role_data_scope(&ctx, role_id)
        .await?;
    state
        .services
        .role_service
        .update_role_permission(
            &ctx,
            role_id,
            &bo.menu_ids.unwrap_or_default(),
            &bo.dept_ids.unwrap_or_default(),
            &bo.data_scope.unwrap_or_else(|| "1".to_string()),
            bo.menu_check_strictly.unwrap_or(true),
            bo.dept_check_strictly.unwrap_or(true),
            &current_user,
        )
        .await?;
    Ok(R::ok())
}

/// 获取角色部门树 (返回部门列表 + 已选中的部门ID)
#[tracing::instrument(skip(state), fields(role_id = role_id))]
pub async fn get_role_dept_tree(
    State(state): State<Arc<AppState>>,
    CurrentUser(login_user): CurrentUser,
    Path(role_id): Path<i64>,
) -> Result<R<serde_json::Value>, AppError> {
    guard::require_permission(&login_user, "system:role:query")?;
    use crate::dept::model::dept::DeptQueryBo;
    // Java SysRoleController.deptTree(:277-281)→ selectDeptTreeList
    // → selectDeptList(SysDeptMapper.java:36 带 @DataPermission)——
    // 受辖,传用户 ctx;软删/范围由漏斗施加。
    let ds = state.infra.data_source();
    let ctx = ds.context(app_data::Principal::from(&login_user));
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
    let checked_keys = state
        .services
        .role_service
        .get_role_dept_ids(&ctx, role_id)
        .await?;
    // 前端 el-tree 用 node-key="id"、props label='label';必须返回
    // TreeSelect 结构(id/label/children),否则树能点但不显示名字。
    let tree = crate::dept::model::dept::DeptTreeSelectVo::build(&depts);
    Ok(R::ok_with_data(serde_json::json!({
        "checkedKeys": checked_keys,
        "depts": tree,
    })))
}

/// 修改角色状态
#[tracing::instrument(
    skip(state, bo),
    fields(operator = %current_user.0.username, role_id = ?bo.role_id)
)]
pub async fn change_status(
    State(state): State<Arc<AppState>>,
    current_user: CurrentUser,
    AppJson(bo): AppJson<RoleChangeStatusBo>,
) -> Result<R<()>, AppError> {
    guard::require_permission(&current_user.0, "system:role:edit")?;
    bo.validate()?;
    let role_id = bo
        .role_id
        .ok_or_else(|| AppError::BadRequest("角色ID为空".to_string()))?;
    let ds = state.infra.data_source();
    let ctx = ds.context(app_data::Principal::from(&current_user.0));
    // Java SysRoleController.changeStatus:166 checkRoleDataScope:越权拦截由这道探针承担
    state
        .services
        .role_service
        .check_role_data_scope(&ctx, role_id)
        .await?;
    state
        .services
        .role_service
        .change_status(&ctx, role_id, bo.status)
        .await?;
    Ok(R::ok())
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RoleOptionSelectQuery {
    pub role_ids: Option<String>,
}

fn parse_role_option_ids(raw: &str) -> Result<Vec<i64>, AppError> {
    let mut ids = Vec::new();
    for s in raw.split(',') {
        let trimmed = s.trim();
        if trimmed.is_empty() {
            continue;
        }
        let id: i64 = trimmed.parse().map_err(|_| {
            AppError::BadRequest(format!(
                "角色下拉选择失败: 无效的角色ID: {}",
                trimmed
            ))
        })?;
        ids.push(id);
    }
    Ok(ids)
}

/// 角色下拉选项
#[tracing::instrument(skip(state, query))]
pub async fn option_select(
    State(state): State<Arc<AppState>>,
    CurrentUser(login_user): CurrentUser,
    Query(query): Query<RoleOptionSelectQuery>,
) -> Result<R<Vec<RoleOptionVo>>, AppError> {
    guard::require_permission(&login_user, "system:role:query")?;
    let role_ids = query
        .role_ids
        .as_deref()
        .filter(|s| !s.is_empty())
        .map(parse_role_option_ids)
        .transpose()?;
    let ds = state.infra.data_source();
    let ctx = ds.context(app_data::Principal::from(&login_user));
    let data = state
        .services
        .role_service
        .option_select(&ctx, role_ids)
        .await?;
    Ok(R::ok_with_data(data))
}

fn to_user_page_query(query: AuthUserQuery) -> UserPageQuery {
    UserPageQuery {
        page: query.page,
        dept_id: None,
        user_name: query.user_name,
        nick_name: None,
        phone_number: query.phone_number,
        status: None,
        begin_time: None,
        end_time: None,
    }
}

/// 查询角色已分配用户列表
#[tracing::instrument(skip(state), fields(role_id = ?query.role_id))]
pub async fn allocated_user_list(
    State(state): State<Arc<AppState>>,
    CurrentUser(login_user): CurrentUser,
    Query(query): Query<AuthUserQuery>,
) -> Result<R<TableDataInfo<UserListVo>>, AppError> {
    guard::require_permission(&login_user, "system:role:list")?;
    let role_id = query
        .role_id
        .ok_or_else(|| AppError::BadRequest("角色ID为空".to_string()))?;
    let user_query = to_user_page_query(query);
    let ds = state.infra.data_source();
    let ctx = ds.context(app_data::Principal::from(&login_user));
    let result = state
        .services
        .role_service
        .allocated_user_page(&ctx, role_id, &user_query)
        .await?;
    Ok(R::ok_with_data(result))
}

/// 查询角色未分配用户列表
#[tracing::instrument(skip(state), fields(role_id = ?query.role_id))]
pub async fn unallocated_user_list(
    State(state): State<Arc<AppState>>,
    CurrentUser(login_user): CurrentUser,
    Query(query): Query<AuthUserQuery>,
) -> Result<R<TableDataInfo<UserListVo>>, AppError> {
    guard::require_permission(&login_user, "system:role:list")?;
    let role_id = query
        .role_id
        .ok_or_else(|| AppError::BadRequest("角色ID为空".to_string()))?;
    let user_query = to_user_page_query(query);
    let ds = state.infra.data_source();
    let ctx = ds.context(app_data::Principal::from(&login_user));
    let result = state
        .services
        .role_service
        .unallocated_user_page(&ctx, role_id, &user_query)
        .await?;
    Ok(R::ok_with_data(result))
}

/// 取消单个用户的角色授权
#[tracing::instrument(skip(state, bo), fields(operator = %current_user.0.username))]
pub async fn cancel_auth_user(
    State(state): State<Arc<AppState>>,
    current_user: CurrentUser,
    AppJson(bo): AppJson<AuthUserCancelBo>,
) -> Result<R<()>, AppError> {
    guard::require_permission(&current_user.0, "system:role:edit")?;
    let role_id = bo
        .role_id
        .ok_or_else(|| AppError::BadRequest("角色ID为空".to_string()))?;
    let user_id = bo
        .user_id
        .ok_or_else(|| AppError::BadRequest("用户ID为空".to_string()))?;
    let ds = state.infra.data_source();
    let ctx = ds.context(app_data::Principal::from(&current_user.0));
    state
        .services
        .role_service
        .cancel_auth_user(&ctx, role_id, user_id)
        .await?;
    Ok(R::ok())
}

/// 批量取消用户的角色授权
#[tracing::instrument(skip(state), fields(operator = %current_user.0.username))]
pub async fn cancel_auth_users(
    State(state): State<Arc<AppState>>,
    current_user: CurrentUser,
    Query(query): Query<AuthUserBatchQuery>,
) -> Result<R<()>, AppError> {
    guard::require_permission(&current_user.0, "system:role:edit")?;
    let role_id = query
        .role_id
        .ok_or_else(|| AppError::BadRequest("角色ID为空".to_string()))?;
    let user_ids = parse_user_ids(query.user_ids.as_deref().unwrap_or(""))?;
    let ds = state.infra.data_source();
    let ctx = ds.context(app_data::Principal::from(&current_user.0));
    state
        .services
        .role_service
        .cancel_auth_users(&ctx, role_id, &user_ids)
        .await?;
    Ok(R::ok())
}

/// 批量授予用户角色
#[tracing::instrument(skip(state), fields(operator = %current_user.0.username))]
pub async fn select_auth_users(
    State(state): State<Arc<AppState>>,
    current_user: CurrentUser,
    Query(query): Query<AuthUserBatchQuery>,
) -> Result<R<()>, AppError> {
    guard::require_permission(&current_user.0, "system:role:edit")?;
    let role_id = query
        .role_id
        .ok_or_else(|| AppError::BadRequest("角色ID为空".to_string()))?;
    let user_ids = parse_user_ids(query.user_ids.as_deref().unwrap_or(""))?;
    let ds = state.infra.data_source();
    let ctx = ds.context(app_data::Principal::from(&current_user.0));
    // Java selectAuthUserAll:265-266 首行即 checkRoleDataScope(roleId)。
    // sys_user_role 是关联表(CONSTRAINED=false,漏斗不施范围),这条批量
    // 授予路径的角色维度越权判定**只能**靠这道探针 —— 否则受限管理员
    // 可把对其不可见的高权角色批量授予给用户。同族的 cancelAuthUser /
    // cancelAuthUserAll 在 Java 侧确无探针(已核 :234/:249),不补。
    state
        .services
        .role_service
        .check_role_data_scope(&ctx, role_id)
        .await?;
    state
        .services
        .role_service
        .select_auth_users(&ctx, role_id, &user_ids)
        .await?;
    Ok(R::ok())
}

/// 导出角色 xlsx POST /system/role/export
/// 对齐 Java SysRoleController.export。
#[tracing::instrument(skip(state, _login_user))]
pub async fn export_role(
    State(state): State<Arc<AppState>>,
    CurrentUser(_login_user): CurrentUser,
    // Vue download() 发 application/x-www-form-urlencoded(见 request.ts:259),
    // 故用 Form 提取(与 GET list 的 Query 同走 serde_urlencoded),不能用 AppJson。
    Form(query): Form<RolePageQuery>,
) -> Result<(HeaderMap, Body), AppError> {
    guard::require_permission(&_login_user, "system:role:export")?;
    let ds = state.infra.data_source();
    let ctx = ds.context(app_data::Principal::from(&_login_user));
    let list = state.services.role_service.list_all(&ctx, query).await?;
    let rows: Vec<RoleExportRow> =
        list.iter().map(RoleExportRow::from_role).collect();
    let bytes = ExcelBuilder::<RoleExportRow>::export(&rows)?;

    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static(
            "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        ),
    );
    headers.insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_static("attachment; filename=\"role.xlsx\""),
    );
    Ok((headers, Body::from(bytes)))
}
