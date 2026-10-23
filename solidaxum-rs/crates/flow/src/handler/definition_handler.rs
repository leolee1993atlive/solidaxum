use std::sync::Arc;

use app_core::{
    error::AppError,
    extract::AppJson,
    middleware::extractors::CurrentUser,
    vo::{
        page::{PageQuery, PageResult},
        response::R,
    },
};
use axum::{
    body::Body,
    extract::{Multipart, Path, Query, State},
    http::{HeaderMap, HeaderValue, header},
};
use serde::Deserialize;
use validator::Validate;

use crate::{
    handler::{FlowState, to_ctx, user_ctx},
    model::{
        def_json::{DefJson, NodeJson},
        dto::{DefinitionListVo, DefinitionVo},
    },
};

#[allow(clippy::too_many_arguments)]
fn save_bo_to_def_json(
    flow_code: String,
    flow_name: String,
    version: String,
    model_value: Option<String>,
    id: Option<i64>,
    category: Option<String>,
    form_custom: Option<String>,
    form_path: Option<String>,
    listener_type: Option<String>,
    listener_path: Option<String>,
    ext: Option<String>,
    node_list: Vec<NodeJson>,
) -> DefJson {
    DefJson {
        id,
        flow_code,
        flow_name,
        model_value: model_value.unwrap_or_else(|| "CLASSICS".to_string()),
        category,
        version,
        is_publish: Some(0),
        form_custom,
        form_path,
        listener_type,
        listener_path,
        ext,
        create_by: None,
        update_by: None,
        node_list,
        chart_status_color: None,
        top_text: None,
        top_text_show: false,
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DefinitionPageQuery {
    #[serde(flatten)]
    pub page: PageQuery,
    pub flow_code: Option<String>,
    pub flow_name: Option<String>,
    pub category: Option<String>,
}

/// 新建流程定义的默认版本号 —— 对齐 Java/warm-flow `checkAndSave`：
/// 版本号由服务端给，前端「添加流程」表单里根本没有这个字段
/// （Vue `index.vue:389-397` 的 initFormData 不含 version）。
fn default_version() -> String {
    "1".to_string()
}

#[derive(Debug, Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct DefinitionAddBo {
    #[validate(length(min = 1, message = "流程编码不能为空"))]
    pub flow_code: String,
    #[validate(length(min = 1, message = "流程名称不能为空"))]
    pub flow_name: String,
    #[serde(default = "default_version")]
    #[validate(length(min = 1, message = "版本号不能为空"))]
    pub version: String,
    pub model_value: Option<String>,
    pub category: Option<String>,
    pub form_custom: Option<String>,
    pub form_path: Option<String>,
    pub listener_type: Option<String>,
    pub listener_path: Option<String>,
    pub ext: Option<String>,
    #[serde(default)]
    pub node_list: Vec<NodeJson>,
}

#[derive(Debug, Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct DefinitionEditBo {
    pub id: i64,
    #[validate(length(min = 1, message = "流程编码不能为空"))]
    pub flow_code: String,
    #[validate(length(min = 1, message = "流程名称不能为空"))]
    pub flow_name: String,
    #[serde(default = "default_version")]
    #[validate(length(min = 1, message = "版本号不能为空"))]
    pub version: String,
    pub model_value: Option<String>,
    pub category: Option<String>,
    pub form_custom: Option<String>,
    pub form_path: Option<String>,
    pub listener_type: Option<String>,
    pub listener_path: Option<String>,
    pub ext: Option<String>,
    #[serde(default)]
    pub node_list: Vec<NodeJson>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActiveQuery {
    #[serde(deserialize_with = "deserialize_bool_str")]
    pub active: bool,
}

fn deserialize_bool_str<'de, D>(deserializer: D) -> Result<bool, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::Error;
    let s = String::deserialize(deserializer)?;
    match s.as_str() {
        "true" | "1" => Ok(true),
        "false" | "0" => Ok(false),
        _ => Err(D::Error::custom(format!("无效的 active 值: {}", s))),
    }
}

async fn fill_category_names(
    state: &Arc<FlowState>,
    dctx: &app_data::DataContext<'_>,
    vos: &mut [DefinitionListVo],
) -> Result<(), AppError> {
    // flow_definition.category 存的是 categoryId（Java 语义），按 id 解析后查 name
    let map = state.category_service.id_to_name_map(dctx).await?;
    for vo in vos {
        if let Some(ref cat_id_str) = vo.category
            && let Ok(cat_id) = cat_id_str.parse::<i64>()
        {
            vo.category_name = map.get(&cat_id).cloned();
        }
    }
    Ok(())
}

async fn fill_category_name_single(
    state: &Arc<FlowState>,
    dctx: &app_data::DataContext<'_>,
    vo: &mut DefinitionListVo,
) -> Result<(), AppError> {
    fill_category_names(state, dctx, std::slice::from_mut(vo)).await
}

async fn fill_category_name_vo(
    state: &Arc<FlowState>,
    dctx: &app_data::DataContext<'_>,
    vo: &mut DefinitionVo,
) -> Result<(), AppError> {
    let map = state.category_service.id_to_name_map(dctx).await?;
    if let Some(ref cat_id_str) = vo.category
        && let Ok(cat_id) = cat_id_str.parse::<i64>()
    {
        vo.category_name = map.get(&cat_id).cloned();
    }
    Ok(())
}

#[tracing::instrument(skip(state, login_user))]
pub async fn list_definition(
    State(state): State<Arc<FlowState>>,
    CurrentUser(login_user): CurrentUser,
    Query(query): Query<DefinitionPageQuery>,
) -> Result<R<PageResult<DefinitionListVo>>, AppError> {
    app_core::permission::require_permission(
        &login_user,
        "workflow:definition:list",
    )?;
    let dctx = user_ctx(&state, &CurrentUser(login_user));
    let (page_no, page_size, _offset) =
        app_core::vo::page::normalize_paging(&query.page);
    let page = state
        .def_service
        .list_page(
            &dctx,
            query.flow_code.as_deref(),
            query.flow_name.as_deref(),
            query.category.as_deref(),
            page_no as u64,
            page_size as u64,
        )
        .await?;
    let mut rows: Vec<DefinitionListVo> =
        page.rows.iter().map(DefinitionListVo::from).collect();
    fill_category_names(&state, &dctx, &mut rows).await?;
    Ok(R::ok_with_data(PageResult::from_page_rows(&page, rows)))
}

#[tracing::instrument(skip(state, login_user))]
pub async fn un_publish_list(
    State(state): State<Arc<FlowState>>,
    CurrentUser(login_user): CurrentUser,
    Query(query): Query<DefinitionPageQuery>,
) -> Result<R<PageResult<DefinitionListVo>>, AppError> {
    app_core::permission::require_permission(
        &login_user,
        "workflow:definition:list",
    )?;
    let dctx = user_ctx(&state, &CurrentUser(login_user));
    let (page_no, page_size, _offset) =
        app_core::vo::page::normalize_paging(&query.page);
    let page = state
        .def_service
        .unpublished_list_page(
            &dctx,
            query.flow_code.as_deref(),
            query.flow_name.as_deref(),
            query.category.as_deref(),
            page_no as u64,
            page_size as u64,
        )
        .await?;
    let mut rows: Vec<DefinitionListVo> =
        page.rows.iter().map(DefinitionListVo::from).collect();
    fill_category_names(&state, &dctx, &mut rows).await?;
    Ok(R::ok_with_data(PageResult::from_page_rows(&page, rows)))
}

#[tracing::instrument(skip(state, login_user))]
pub async fn get_definition(
    State(state): State<Arc<FlowState>>,
    CurrentUser(login_user): CurrentUser,
    Path(id): Path<i64>,
) -> Result<R<DefinitionVo>, AppError> {
    app_core::permission::require_permission(
        &login_user,
        "workflow:definition:query",
    )?;
    let dctx = user_ctx(&state, &CurrentUser(login_user));
    let def =
        state
            .def_service
            .get_by_id(&dctx, id)
            .await?
            .ok_or_else(|| {
                AppError::NotFound(format!("流程定义不存在 (id={})", id))
            })?;
    let mut vo = DefinitionVo::from(&def);
    fill_category_name_vo(&state, &dctx, &mut vo).await?;
    Ok(R::ok_with_data(vo))
}

#[tracing::instrument(skip(state, login_user, bo))]
pub async fn add_definition(
    State(state): State<Arc<FlowState>>,
    CurrentUser(login_user): CurrentUser,
    AppJson(bo): AppJson<DefinitionAddBo>,
) -> Result<R<DefinitionListVo>, AppError> {
    app_core::permission::require_permission(
        &login_user,
        "workflow:definition:add",
    )?;
    bo.validate()?;
    let current_user = CurrentUser(login_user);
    let dctx = user_ctx(&state, &current_user);
    let ctx = to_ctx(&current_user);
    let def_json = save_bo_to_def_json(
        bo.flow_code,
        bo.flow_name,
        bo.version,
        bo.model_value,
        None,
        bo.category,
        bo.form_custom,
        bo.form_path,
        bo.listener_type,
        bo.listener_path,
        bo.ext,
        bo.node_list,
    );
    let def = state.def_service.save_def(&dctx, &def_json, &ctx).await?;
    let mut vo = DefinitionListVo::from(&def);
    fill_category_name_single(&state, &dctx, &mut vo).await?;
    Ok(R::ok_with_data(vo))
}

#[tracing::instrument(skip(state, login_user, bo))]
pub async fn edit_definition(
    State(state): State<Arc<FlowState>>,
    CurrentUser(login_user): CurrentUser,
    AppJson(bo): AppJson<DefinitionEditBo>,
) -> Result<R<DefinitionListVo>, AppError> {
    app_core::permission::require_permission(
        &login_user,
        "workflow:definition:edit",
    )?;
    bo.validate()?;
    let current_user = CurrentUser(login_user);
    let dctx = user_ctx(&state, &current_user);
    let ctx = to_ctx(&current_user);
    let def_json = save_bo_to_def_json(
        bo.flow_code,
        bo.flow_name,
        bo.version,
        bo.model_value,
        Some(bo.id),
        bo.category,
        bo.form_custom,
        bo.form_path,
        bo.listener_type,
        bo.listener_path,
        bo.ext,
        bo.node_list,
    );
    let def = state.def_service.save_def(&dctx, &def_json, &ctx).await?;
    let mut vo = DefinitionListVo::from(&def);
    fill_category_name_single(&state, &dctx, &mut vo).await?;
    Ok(R::ok_with_data(vo))
}

#[tracing::instrument(skip(state, login_user))]
pub async fn publish_definition(
    State(state): State<Arc<FlowState>>,
    CurrentUser(login_user): CurrentUser,
    Path(id): Path<i64>,
) -> Result<R<()>, AppError> {
    app_core::permission::require_permission(
        &login_user,
        "workflow:definition:publish",
    )?;
    let current_user = CurrentUser(login_user);
    let dctx = user_ctx(&state, &current_user);
    let ctx = to_ctx(&current_user);
    state.def_service.publish(&dctx, id, &ctx).await?;
    Ok(R::ok())
}

#[tracing::instrument(skip(state, login_user))]
pub async fn un_publish_definition(
    State(state): State<Arc<FlowState>>,
    CurrentUser(login_user): CurrentUser,
    Path(id): Path<i64>,
) -> Result<R<()>, AppError> {
    app_core::permission::require_permission(
        &login_user,
        "workflow:definition:publish",
    )?;
    let current_user = CurrentUser(login_user);
    let dctx = user_ctx(&state, &current_user);
    let ctx = to_ctx(&current_user);
    state.def_service.unpublish(&dctx, id, &ctx).await?;
    Ok(R::ok())
}

#[tracing::instrument(skip(state, login_user))]
pub async fn copy_definition(
    State(state): State<Arc<FlowState>>,
    CurrentUser(login_user): CurrentUser,
    Path(id): Path<i64>,
) -> Result<R<DefinitionListVo>, AppError> {
    app_core::permission::require_permission(
        &login_user,
        "workflow:definition:copy",
    )?;
    let current_user = CurrentUser(login_user);
    let dctx = user_ctx(&state, &current_user);
    let ctx = to_ctx(&current_user);
    let def = state.def_service.copy_def(&dctx, id, &ctx).await?;
    let mut vo = DefinitionListVo::from(&def);
    fill_category_name_single(&state, &dctx, &mut vo).await?;
    Ok(R::ok_with_data(vo))
}

#[tracing::instrument(skip(state, login_user))]
pub async fn active_definition(
    State(state): State<Arc<FlowState>>,
    CurrentUser(login_user): CurrentUser,
    Path(id): Path<i64>,
    Query(query): Query<ActiveQuery>,
) -> Result<R<()>, AppError> {
    app_core::permission::require_permission(
        &login_user,
        "workflow:definition:active",
    )?;
    let current_user = CurrentUser(login_user);
    let dctx = user_ctx(&state, &current_user);
    let ctx = to_ctx(&current_user);
    state
        .def_service
        .set_active(&dctx, id, query.active, &ctx)
        .await?;
    Ok(R::ok())
}

#[tracing::instrument(skip(state, login_user, multipart))]
pub async fn import_def(
    State(state): State<Arc<FlowState>>,
    CurrentUser(login_user): CurrentUser,
    mut multipart: Multipart,
) -> Result<R<()>, AppError> {
    app_core::permission::require_permission(
        &login_user,
        "workflow:definition:import",
    )?;

    let current_user = CurrentUser(login_user);
    let dctx = user_ctx(&state, &current_user);
    let ctx = to_ctx(&current_user);

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| AppError::BadRequest(format!("读取上传文件失败: {}", e)))?
    {
        let original_name = match field.file_name() {
            Some(name) => name.to_string(),
            None => continue,
        };
        let data = field.bytes().await.map_err(|e| {
            AppError::Internal(format!("读取文件内容失败: {}", e))
        })?;

        state
            .def_service
            .import_def(&dctx, &original_name, &data, &ctx)
            .await?;
        return Ok(R::ok());
    }

    Err(AppError::BadRequest("未找到上传文件".to_string()))
}

fn export_headers(filename: &str) -> HeaderMap {
    let disposition =
        format!("attachment; filename=\"{}\"", filename.replace('"', "\\\""));
    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/octet-stream"),
    );
    headers.insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::try_from(disposition)
            .unwrap_or_else(|_| HeaderValue::from_static("attachment")),
    );
    headers
}

#[tracing::instrument(skip(state, login_user))]
pub async fn export_def(
    State(state): State<Arc<FlowState>>,
    CurrentUser(login_user): CurrentUser,
    Path(id): Path<i64>,
) -> Result<(HeaderMap, Body), AppError> {
    app_core::permission::require_permission(
        &login_user,
        "workflow:definition:export",
    )?;
    let dctx = user_ctx(&state, &CurrentUser(login_user));
    let json = state.def_service.export_json_string(&dctx, id).await?;
    let filename = format!("definition_{}.json", id);
    let headers = export_headers(&filename);
    Ok((headers, Body::from(json)))
}

#[tracing::instrument(skip(state, login_user))]
pub async fn xml_string(
    State(state): State<Arc<FlowState>>,
    CurrentUser(login_user): CurrentUser,
    Path(id): Path<i64>,
) -> Result<R<String>, AppError> {
    app_core::permission::require_permission(
        &login_user,
        "workflow:definition:query",
    )?;
    let dctx = user_ctx(&state, &CurrentUser(login_user));
    let json = state.def_service.export_json_string(&dctx, id).await?;
    Ok(R::ok_with_data(json))
}

#[tracing::instrument(skip(state, login_user))]
pub async fn delete_definition(
    State(state): State<Arc<FlowState>>,
    CurrentUser(login_user): CurrentUser,
    Path(ids): Path<String>,
) -> Result<R<()>, AppError> {
    app_core::permission::require_permission(
        &login_user,
        "workflow:definition:remove",
    )?;
    let current_user = CurrentUser(login_user);
    let dctx = user_ctx(&state, &current_user);
    let ctx = to_ctx(&current_user);
    let id_list: Vec<i64> = ids
        .split(',')
        .filter(|s| !s.is_empty())
        .map(|s| {
            s.parse()
                .map_err(|_| AppError::BadRequest(format!("无效的ID: {}", s)))
        })
        .collect::<Result<Vec<_>, _>>()?;
    state.def_service.delete_defs(&dctx, &id_list, &ctx).await?;
    Ok(R::ok())
}
