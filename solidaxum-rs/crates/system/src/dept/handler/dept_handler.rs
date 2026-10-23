use std::{collections::HashSet, sync::Arc};

use app_core::{
    error::AppError, extract::AppJson, middleware::extractors::CurrentUser,
    model::status::Status, vo::response::R,
};
use axum::extract::{Path, Query, State};
use serde::Deserialize;
use validator::Validate;

use crate::{
    app::AppState,
    dept::model::dept::{DeptCreateBo, DeptListVo, DeptQueryBo, DeptUpdateBo},
    permission::guard,
};

/// 批量填充 leader_name(经 translation_service 翻译 user_id)
/// 与 parent_name(用列表自身构建 dept_id→dept_name 映射)。
/// 供 list/exclude/optionselect 复用,对齐 Java selectDeptById 的 parentName
/// 与 @Translation(USER_ID_TO_NAME) 的 leaderName 行为。
/// ctx 由调用方传入(翻译读豁免,ctx 的范围不影响解析口径)。
async fn fill_leader_and_parent_names(
    state: &Arc<AppState>,
    ctx: &app_data::DataContext<'_>,
    rows: &mut [DeptListVo],
) -> Result<(), AppError> {
    // parent_name: 用列表内已有的 dept 构建 id→name 映射(单批内存,无需二次查库)
    let id_to_name: std::collections::HashMap<i64, String> = rows
        .iter()
        .map(|d| (d.dept_id, d.dept_name.clone()))
        .collect();
    for row in rows.iter_mut() {
        row.parent_name = id_to_name.get(&row.parent_id).cloned();
    }

    // leader_name: 经 translation_service 批量翻译 user_id
    let leader_ids: Vec<i64> = rows.iter().filter_map(|d| d.leader).collect();
    if !leader_ids.is_empty() {
        let name_map = state
            .services
            .translation_service
            .user_id_to_name(ctx, &leader_ids)
            .await?;
        for row in rows.iter_mut() {
            if let Some(lid) = row.leader {
                row.leader_name = name_map.get(&lid).cloned();
            }
        }
    }
    Ok(())
}

#[tracing::instrument(skip(state, query))]
pub async fn list_dept(
    State(state): State<Arc<AppState>>,
    CurrentUser(login_user): CurrentUser,
    Query(query): Query<DeptQueryBo>,
) -> Result<R<Vec<DeptListVo>>, AppError> {
    guard::require_permission(&login_user, "system:dept:list")?;
    // ctx 请求期构造 —— `infra.data_source()` 此刻必带 resolver
    // (装配在容器构造后、开服务前完成,见 admin/app.rs 装配顺序注释);
    // 绑定 Arc 局部变量再借出,避免临时值悬垂。
    let ds = state.infra.data_source();
    let ctx = ds.context(app_data::Principal::from(&login_user));
    let mut data = state.services.dept_service.list(&ctx, query).await?;
    fill_leader_and_parent_names(&state, &ctx, &mut data).await?;
    Ok(R::ok_with_data(data))
}

#[tracing::instrument(skip(state), fields(dept_id = id))]
pub async fn list_dept_exclude(
    State(state): State<Arc<AppState>>,
    CurrentUser(login_user): CurrentUser,
    Path(id): Path<i64>,
) -> Result<R<Vec<DeptListVo>>, AppError> {
    guard::require_permission(&login_user, "system:dept:list")?;
    let ds = state.infra.data_source();
    let ctx = ds.context(app_data::Principal::from(&login_user));
    let mut data = state.services.dept_service.list_exclude(&ctx, id).await?;
    fill_leader_and_parent_names(&state, &ctx, &mut data).await?;
    Ok(R::ok_with_data(data))
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeptOptionSelectQuery {
    pub dept_ids: Option<String>,
}

/// Java optionselect(SysDeptController:155-157)→ selectDeptByIds
/// (ServiceImpl:182-188)→ 受辖 selectDeptList(Mapper:36)—— 下拉在
/// Java 是**受辖**方法,与 post 的下拉豁免相反。status=NORMAL 过滤
/// (ServiceImpl:185 `.eq(SysDept::getStatus, NORMAL)`)经 DeptQueryBo
/// 的 status 条件施加(select_list → Column::Status.eq)。
#[tracing::instrument(skip(state, query))]
pub async fn option_select(
    State(state): State<Arc<AppState>>,
    CurrentUser(login_user): CurrentUser,
    Query(query): Query<DeptOptionSelectQuery>,
) -> Result<R<Vec<DeptListVo>>, AppError> {
    guard::require_permission(&login_user, "system:dept:query")?;
    let ds = state.infra.data_source();
    let ctx = ds.context(app_data::Principal::from(&login_user));
    let mut data = state
        .services
        .dept_service
        .list(
            &ctx,
            DeptQueryBo {
                // 下拉只出正常状态部门(Java 平价锚
                // SysDeptServiceImpl:185);list 端点无此过滤。
                status: Some(Status::Normal.as_str().to_string()),
                ..DeptQueryBo::default()
            },
        )
        .await?;

    if let Some(ids) = query.dept_ids.filter(|s| !s.is_empty()) {
        let mut set = HashSet::new();
        for s in ids.split(',') {
            let id: i64 = s.parse().map_err(|_| {
                AppError::BadRequest(format!(
                    "部门下拉选择失败: 无效的部门ID: {}",
                    s
                ))
            })?;
            set.insert(id);
        }
        data.retain(|d| set.contains(&d.dept_id));
    }

    fill_leader_and_parent_names(&state, &ctx, &mut data).await?;
    Ok(R::ok_with_data(data))
}

/// Java getInfo(SysDeptController:73)先 checkDeptDataScope 再
/// selectDeptById —— 探针在前,不存在/越权同形态 NotFound。
#[tracing::instrument(skip(state), fields(dept_id = id))]
pub async fn get_dept_by_id(
    State(state): State<Arc<AppState>>,
    CurrentUser(login_user): CurrentUser,
    Path(id): Path<i64>,
) -> Result<R<DeptListVo>, AppError> {
    guard::require_permission(&login_user, "system:dept:query")?;
    let ds = state.infra.data_source();
    let ctx = ds.context(app_data::Principal::from(&login_user));
    state
        .services
        .dept_service
        .check_dept_data_scope(&ctx, Some(id))
        .await?;
    let dept = state.services.dept_service.get_by_id(&ctx, id).await?;
    let mut vo = dept.to_list_vo();

    // parent_name: 按 parent_id 二次查(对齐 Java selectDeptById:167-171)
    // parent_id=0(根部门)时跳过,get_by_id 找不到时保留 None。
    // 父部门查询在 Java 是无注解 lambda,这里也无探针(同口径)。
    if dept.parent_id != 0
        && let Ok(parent) = state
            .services
            .dept_service
            .get_by_id(&ctx, dept.parent_id)
            .await
    {
        vo.parent_name = Some(parent.dept_name);
    }

    // leader_name: 经 translation_service 翻译 user_id
    if let Some(lid) = vo.leader {
        let name_map = state
            .services
            .translation_service
            .user_id_to_name(&ctx, &[lid])
            .await?;
        vo.leader_name = name_map.get(&lid).cloned();
    }

    Ok(R::ok_with_data(vo))
}

/// Java add(SysDeptController:87-92)**无** checkDeptDataScope ——
/// 新增不探针(与 user add 不同),仅唯一性守卫。
#[tracing::instrument(
    skip(state, bo),
    fields(operator = %current_user.0.username, dept_name = ?bo.dept_name)
)]
pub async fn create_dept(
    State(state): State<Arc<AppState>>,
    current_user: CurrentUser,
    AppJson(bo): AppJson<DeptCreateBo>,
) -> Result<R<()>, AppError> {
    guard::require_permission(&current_user.0, "system:dept:add")?;
    bo.validate()?;
    let ds = state.infra.data_source();
    let ctx = ds.context(app_data::Principal::from(&current_user.0));
    state
        .services
        .dept_service
        .insert_by_bo(&ctx, bo, &current_user)
        .await?;
    Ok(R::ok())
}

/// Java edit(SysDeptController:106)先 checkDeptDataScope(bo.deptId)
/// 再唯一性/守卫/update —— 探针在前,同 Java controller 顺序。
/// 换父时对新父部门的探针(ServiceImpl:346)由 service 承担。
#[tracing::instrument(
    skip(state, bo),
    fields(operator = %current_user.0.username, dept_id = ?bo.dept_id)
)]
pub async fn update_dept(
    State(state): State<Arc<AppState>>,
    current_user: CurrentUser,
    AppJson(bo): AppJson<DeptUpdateBo>,
) -> Result<R<()>, AppError> {
    guard::require_permission(&current_user.0, "system:dept:edit")?;
    bo.validate()?;
    let ds = state.infra.data_source();
    let ctx = ds.context(app_data::Principal::from(&current_user.0));
    state
        .services
        .dept_service
        .check_dept_data_scope(&ctx, bo.dept_id)
        .await?;
    state
        .services
        .dept_service
        .update_by_bo(&ctx, bo, &current_user)
        .await?;
    Ok(R::ok())
}

/// Java remove(SysDeptController:143)的探针居守卫之后,由 service 的
/// delete_by_id 承担(守卫也在 service,错误优先级同 Java)。
#[tracing::instrument(
    skip(state),
    fields(operator = %current_user.0.username, dept_ids = ids)
)]
pub async fn delete_dept(
    State(state): State<Arc<AppState>>,
    current_user: CurrentUser,
    Path(ids): Path<String>,
) -> Result<R<()>, AppError> {
    guard::require_permission(&current_user.0, "system:dept:remove")?;
    let ds = state.infra.data_source();
    let ctx = ds.context(app_data::Principal::from(&current_user.0));
    for id_str in ids.split(',') {
        if id_str.is_empty() {
            continue;
        }
        let id: i64 = id_str.parse().map_err(|_| {
            AppError::BadRequest(format!(
                "删除部门失败: 无效的部门ID: {}",
                id_str
            ))
        })?;
        state
            .services
            .dept_service
            .delete_by_id(&ctx, id, &current_user)
            .await?;
    }
    Ok(R::ok())
}
