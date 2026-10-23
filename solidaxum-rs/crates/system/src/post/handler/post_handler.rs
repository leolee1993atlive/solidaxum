use std::{collections::HashSet, sync::Arc};

use app_core::{
    error::AppError,
    extract::AppJson,
    middleware::extractors::CurrentUser,
    util::excel::ExcelBuilder,
    vo::{page::TableDataInfo, response::R},
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
    post::model::post::{
        PostCreateBo, PostExportRow, PostListVo, PostPageQuery, PostUpdateBo,
    },
};

#[tracing::instrument(skip(state), fields(page = ?query.page.page_num, size = ?query.page.page_size))]
pub async fn list_post(
    State(state): State<Arc<AppState>>,
    CurrentUser(login_user): CurrentUser,
    Query(query): Query<PostPageQuery>,
) -> Result<R<TableDataInfo<PostListVo>>, AppError> {
    guard::require_permission(&login_user, "system:post:list")?;

    // ctx 请求期构造 —— `infra.data_source()` 此刻必带 resolver
    // (装配在容器构造后、开服务前完成,见 admin/app.rs 装配顺序注释);
    // 绑定 Arc 局部变量再借出,避免临时值悬垂。
    // belongDeptId 的子树展开在 service 层(belong_subtree_condition):
    // 它是业务搜索过滤,与漏斗行级范围正交叠加。
    let ds = state.infra.data_source();
    let ctx = ds.context(app_data::Principal::from(&login_user));

    let mut result = state.services.post_service.page(&ctx, query).await?;

    // 批量翻译 dept_id → dept_name
    let dept_ids: Vec<i64> = result.rows.iter().map(|r| r.dept_id).collect();
    if !dept_ids.is_empty() {
        let name_map = state
            .services
            .translation_service
            .dept_id_to_name(&ctx, &dept_ids)
            .await?;
        for row in &mut result.rows {
            row.dept_name = name_map.get(&row.dept_id).cloned();
        }
    }

    Ok(R::ok_with_data(result))
}

/// 岗位管理-部门树。对齐 Java `SysPostController#deptTree`:返回完整部门
/// TreeSelect 结构(id/label/children),仅需登录、无额外权限。
#[tracing::instrument(skip(state))]
pub async fn dept_tree(
    State(state): State<Arc<AppState>>,
    CurrentUser(login_user): CurrentUser,
) -> Result<R<Vec<crate::dept::model::dept::DeptTreeSelectVo>>, AppError> {
    // Java SysPostController.deptTree(:164-166)→ selectDeptTreeList
    // → selectDeptList(SysDeptMapper.java:36 带 @DataPermission)——
    // 受辖,传用户 ctx;软删/范围由漏斗施加。
    let ds = state.infra.data_source();
    let ctx = ds.context(app_data::Principal::from(&login_user));
    let depts = state
        .services
        .dept_service
        .list(
            &ctx,
            crate::dept::model::dept::DeptQueryBo {
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

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PostOptionSelectQuery {
    pub post_ids: Option<String>,
    pub dept_id: Option<i64>,
}

#[tracing::instrument(skip(state, query))]
pub async fn option_select(
    State(state): State<Arc<AppState>>,
    CurrentUser(login_user): CurrentUser,
    Query(query): Query<PostOptionSelectQuery>,
) -> Result<R<Vec<PostListVo>>, AppError> {
    guard::require_permission(&login_user, "system:post:query")?;
    // 对齐 Java SysPostController.optionselect:145-155 互斥分支
    //   - deptId 非 null → 按部门过滤(deptId 优先,即使 postIds 也传了)
    //   - else postIds 非 null → 按 id 列表过滤
    //   - else → 空数组(Java else 分支返回 new ArrayList<>())
    // list 是 Java 豁免方法(selectPostAll 无 @DataPermission),
    // repo 内部 without_row_scope;此处仍传用户 ctx 以保留软删/审计口径。
    let ds = state.infra.data_source();
    let ctx = ds.context(app_data::Principal::from(&login_user));
    let all = state.services.post_service.list(&ctx).await?;
    let data: Vec<PostListVo> = if let Some(dept_id) = query.dept_id {
        all.into_iter().filter(|p| p.dept_id == dept_id).collect()
    } else if let Some(ids) =
        query.post_ids.as_deref().filter(|s| !s.is_empty())
    {
        let mut id_set = HashSet::new();
        for s in ids.split(',') {
            let id: i64 = s.parse().map_err(|_| {
                AppError::BadRequest(format!(
                    "岗位下拉选择失败: 无效的岗位ID: {}",
                    s
                ))
            })?;
            id_set.insert(id);
        }
        all.into_iter()
            .filter(|p| id_set.contains(&p.post_id))
            .collect()
    } else {
        Vec::new()
    };
    Ok(R::ok_with_data(data))
}

#[tracing::instrument(skip(state), fields(post_id = id))]
pub async fn get_post_by_id(
    State(state): State<Arc<AppState>>,
    CurrentUser(login_user): CurrentUser,
    Path(id): Path<i64>,
) -> Result<R<PostListVo>, AppError> {
    guard::require_permission(&login_user, "system:post:query")?;
    let ds = state.infra.data_source();
    let ctx = ds.context(app_data::Principal::from(&login_user));
    let post = state.services.post_service.get_by_id(&ctx, id).await?;
    Ok(R::ok_with_data(post.to_list_vo()))
}

#[tracing::instrument(
    skip(state, bo),
    fields(operator = %current_user.0.username, post_code = ?bo.post_code)
)]
pub async fn create_post(
    State(state): State<Arc<AppState>>,
    current_user: CurrentUser,
    AppJson(bo): AppJson<PostCreateBo>,
) -> Result<R<()>, AppError> {
    guard::require_permission(&current_user.0, "system:post:add")?;
    bo.validate()?;
    // 审计盖章人取 ctx 的 principal(漏斗 insert 统一盖)
    let ds = state.infra.data_source();
    let ctx = ds.context(app_data::Principal::from(&current_user.0));
    state
        .services
        .post_service
        .insert_by_bo(&ctx, bo, &current_user)
        .await?;
    Ok(R::ok())
}

#[tracing::instrument(
    skip(state, bo),
    fields(operator = %current_user.0.username, post_id = ?bo.post_id)
)]
pub async fn update_post(
    State(state): State<Arc<AppState>>,
    current_user: CurrentUser,
    AppJson(bo): AppJson<PostUpdateBo>,
) -> Result<R<()>, AppError> {
    guard::require_permission(&current_user.0, "system:post:edit")?;
    bo.validate()?;
    let ds = state.infra.data_source();
    let ctx = ds.context(app_data::Principal::from(&current_user.0));
    state
        .services
        .post_service
        .update_by_bo(&ctx, bo, &current_user)
        .await?;
    Ok(R::ok())
}

#[tracing::instrument(
    skip(state),
    fields(operator = %current_user.0.username, post_ids = ids)
)]
pub async fn delete_post(
    State(state): State<Arc<AppState>>,
    current_user: CurrentUser,
    Path(ids): Path<String>,
) -> Result<R<()>, AppError> {
    guard::require_permission(&current_user.0, "system:post:remove")?;
    let ds = state.infra.data_source();
    let ctx = ds.context(app_data::Principal::from(&current_user.0));
    for id_str in ids.split(',') {
        if id_str.is_empty() {
            continue;
        }
        let id: i64 = id_str.parse().map_err(|_| {
            AppError::BadRequest(format!(
                "删除岗位失败: 无效的岗位ID: {}",
                id_str
            ))
        })?;
        state.services.post_service.delete_by_id(&ctx, id).await?;
    }
    Ok(R::ok())
}

/// 导出岗位 xlsx POST /system/post/export
/// 对齐 Java SysPostController.export。
#[tracing::instrument(skip(state, login_user))]
pub async fn export_post(
    State(state): State<Arc<AppState>>,
    CurrentUser(login_user): CurrentUser,
    // Vue download() 发 application/x-www-form-urlencoded(见 request.ts:259),
    // 故用 Form 提取(与 GET list 的 Query 同走 serde_urlencoded),不能用 AppJson。
    Form(query): Form<PostPageQuery>,
) -> Result<(HeaderMap, Body), AppError> {
    guard::require_permission(&login_user, "system:post:export")?;

    let ds = state.infra.data_source();
    let ctx = ds.context(app_data::Principal::from(&login_user));

    let list = state.services.post_service.list_all(&ctx, query).await?;
    let rows: Vec<PostExportRow> =
        list.iter().map(PostExportRow::from_post).collect();
    let bytes = ExcelBuilder::<PostExportRow>::export(&rows)?;

    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static(
            "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        ),
    );
    headers.insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_static("attachment; filename=\"post.xlsx\""),
    );
    Ok((headers, Body::from(bytes)))
}
