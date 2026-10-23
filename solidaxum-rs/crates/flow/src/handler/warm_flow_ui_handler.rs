use std::sync::Arc;

use app_core::{extract::AppJson, vo::response::IdJson};
use axum::{
    extract::{Path, Query, RawQuery, State},
    http::HeaderMap,
    response::IntoResponse,
};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::{handler::FlowState, model::def_json::DefJson};

// ─── /warm-flow-ui/config ───────────────────────────────────────────────────

/// 返回 token 头名称，设计器据此从 URL param 读取 token
pub(crate) fn warm_flow_config_data() -> Value {
    json!({
        "tokenNameList": ["Authorization"],
        "framework": "axum"
    })
}

pub async fn get_config() -> impl IntoResponse {
    IdJson(json!({ "code": 200, "data": warm_flow_config_data() }))
}

// ─── /warm-flow/save-json ───────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct SaveJsonQuery {
    // 若为 true，只更新节点和条件，不更新基础信息
    #[serde(rename = "onlyNodeSkip")]
    pub only_node_skip: Option<bool>,
}

/// POST /warm-flow/save-json
/// Body: DefJson（warm-flow 格式）
/// Header: onlyNodeSkip: true/false（可选）
pub async fn save_json(
    State(state): State<Arc<FlowState>>,
    headers: HeaderMap,
    AppJson(def_json): AppJson<DefJson>,
) -> impl IntoResponse {
    let only_node_skip = headers
        .get("onlyNodeSkip")
        .and_then(|v| v.to_str().ok())
        .map(|s| s == "true")
        .unwrap_or(false);

    // /warm-flow/* 路由已收 auth(锚 Java sa-token 白名单,见
    // route/flow_route.rs::designer_api_route);handler 内仍
    // system_context + 豁免(flow 全豁免实体,豁免读不解析范围,与用户
    // ctx 行为等价);受辖锚(handler-result 三型)的数据权限尚未施加。
    let dctx = state.ds.system_context();
    match state
        .def_service
        .save_def_json(&dctx, def_json, only_node_skip)
        .await
    {
        Ok(_) => IdJson(json!({ "code": 200, "msg": "保存成功" })),
        Err(e) => IdJson(json!({ "code": 500, "msg": e.to_string() })),
    }
}

// ─── /warm-flow/query-def/{id} ──────────────────────────────────────────────

/// GET /warm-flow/query-def/{id}
/// 返回 DefJson 供设计器渲染（id 为 0 或不传时返回空模板）
pub async fn query_def(
    State(state): State<Arc<FlowState>>,
    id: Option<Path<i64>>,
) -> impl IntoResponse {
    let def_id = id.map(|p| p.0).unwrap_or(0);
    if def_id == 0 {
        return IdJson(json!({
            "code": 200,
            "data": {
                "flowName": "新建流程",
                "flowCode": "",
                "version": "1",
                "isPublish": 0,
                "formCustom": "N",
                "modelValue": "mimic",
                "nodeList": [],
                "skipList": []
            }
        }));
    }

    // 路由已收 auth,同 save_json:仍
    // system_context + 豁免(受辖锚数据权限尚未施加)
    let dctx = state.ds.system_context();
    match state.def_service.get_def_json(&dctx, def_id).await {
        Ok(def_json) => IdJson(json!({ "code": 200, "data": def_json })),
        Err(e) => IdJson(json!({ "code": 500, "msg": e.to_string() })),
    }
}

// ─── /warm-flow/query-flow-chart/{id} ──────────────────────────────────────

/// GET /warm-flow/query-flow-chart/{id}（id = 实例 id）
/// 对齐 Java WarmFlowService.queryFlowChart:返回实例 def_json(DefJson,
/// 含每节点 status 高亮)+ 状态三原色,供设计器 FlowChart 模式渲染流程图。
pub async fn query_flow_chart(
    State(state): State<Arc<FlowState>>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    // 路由已收 auth,同 query_def:仍
    // system_context + 豁免(受辖锚数据权限尚未施加)
    let dctx = state.ds.system_context();
    match state.ins_service.flow_chart(&dctx, id).await {
        Ok(data) => IdJson(json!({ "code": 200, "data": data })),
        Err(e) => IdJson(json!({ "code": 500, "msg": e.to_string() })),
    }
}

// ─── /warm-flow/listener-list ───────────────────────────────────────────────

/// GET /warm-flow/listener-list
/// 返回 ListenerRegistry 中注册的监听器名称列表，供设计器节点配置选择
pub async fn listener_list(
    State(state): State<Arc<FlowState>>,
) -> impl IntoResponse {
    let names: Vec<String> = state
        .listener_registry
        .names()
        .into_iter()
        .map(String::from)
        .collect();
    IdJson(json!({ "code": 200, "data": names }))
}

// ─── 存根接口 ─────────────────────────────────────────────────────────────────
// 以下接口在 RuoYi 集成后替换为真实实现

/// GET /warm-flow/handler-type
/// 办理人类型 tabs（选人弹窗的标签页）
/// 前端直接把数组元素当作 tab 文本渲染，因此必须返回字符串列表（与 Java
/// TaskAssigneeEnum.getAssigneeTypeList() 一致）。点击 tab 后会以
/// `handlerType=<该字符串>` 回传到 handler-result。
pub async fn handler_type() -> impl IntoResponse {
    IdJson(json!({
        "code": 200,
        "data": ["用户", "角色", "部门"]
    }))
}

#[derive(Deserialize)]
pub struct HandlerQuery {
    #[serde(rename = "pageNum")]
    pub page_num: Option<u64>,
    #[serde(rename = "pageSize")]
    pub page_size: Option<u64>,
    pub status: Option<String>,
    /// 办理人类型描述（"用户" / "角色" / "部门"），由 handler-type 的 tab 回传。
    #[serde(rename = "handlerType")]
    pub handler_type: Option<String>,
}

/// 将前端的类型描述映射为内部代码。
fn handler_type_code(desc: &str) -> &'static str {
    match desc {
        "角色" => "role",
        "部门" => "dept",
        _ => "user",
    }
}

/// GET /warm-flow/handler-result
/// 办理人列表（选人弹窗候选数据）
pub async fn handler_result(
    State(state): State<Arc<FlowState>>,
    Query(q): Query<HandlerQuery>,
) -> impl IntoResponse {
    let handler_type =
        handler_type_code(q.handler_type.as_deref().unwrap_or("用户"));
    let page_num = q.page_num.unwrap_or(1);
    let page_size = q.page_size.unwrap_or(10);
    let status = q.status.as_deref();

    match state
        .handler_select
        .get_handlers(handler_type, status, page_num, page_size)
        .await
    {
        Ok(result) => IdJson(json!({
            "code": 200,
            "data": {
                "handlerAuths": {
                    "rows": result.rows,
                    "total": result.total,
                },
                "treeSelections": result.tree_selections,
            }
        })),
        Err(e) => IdJson(json!({
            "code": 500,
            "msg": e.to_string(),
            "data": {
                "handlerAuths": { "rows": [], "total": 0 },
                "treeSelections": [],
            }
        })),
    }
}

/// GET /warm-flow/handler-feedback
/// 办理人名称回显（根据 storageId 查名称）。
/// 前端有两种传参形态，都需兼容：
///   - 逗号拼接：`storageIds=role:1,user:2`
///   - 数组/重复键：`storageIds[]=role:1&storageIds[]=user:2` 或 `storageIds=role:1&storageIds=user:2`
pub async fn handler_feedback(
    State(state): State<Arc<FlowState>>,
    RawQuery(raw): RawQuery,
) -> impl IntoResponse {
    let storage_ids = parse_storage_ids(raw.as_deref().unwrap_or(""));

    match state.handler_select.feedback(&storage_ids).await {
        Ok(list) => IdJson(json!({ "code": 200, "data": list })),
        Err(e) => IdJson(json!({
            "code": 500,
            "msg": e.to_string(),
            "data": [],
        })),
    }
}

/// 从原始 query string 中提取所有 storageIds 值（兼容逗号拼接与数组形态）。
fn parse_storage_ids(raw: &str) -> Vec<String> {
    let mut out = Vec::new();
    for pair in raw.split('&') {
        let Some((k, v)) = pair.split_once('=') else {
            continue;
        };
        if k != "storageIds" && k != "storageIds[]" {
            continue;
        }
        let decoded = percent_decode(v);
        for part in decoded.split(',') {
            let part = part.trim();
            if !part.is_empty() {
                out.push(part.to_string());
            }
        }
    }
    out
}

/// 简单 percent-decode（含 `+` -> 空格），用于解析 query 值。
fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' if i + 2 < bytes.len() => {
                let hi = (bytes[i + 1] as char).to_digit(16);
                let lo = (bytes[i + 2] as char).to_digit(16);
                if let (Some(h), Some(l)) = (hi, lo) {
                    out.push((h * 16 + l) as u8);
                    i += 3;
                    continue;
                }
                out.push(b'%');
                i += 1;
            }
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            b => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// GET /warm-flow/handler-dict
/// 办理人固定选项（如特殊角色）
pub async fn handler_dict() -> impl IntoResponse {
    IdJson(json!({ "code": 200, "data": [] }))
}

/// GET /warm-flow/node-ext
/// 节点扩展配置（自定义节点属性）
pub async fn node_ext() -> impl IntoResponse {
    IdJson(json!({ "code": 200, "data": [] }))
}

/// GET /warm-flow/published-form
/// 已发布表单列表（供表单设计器绑定）
pub async fn published_form() -> impl IntoResponse {
    IdJson(json!({ "code": 200, "data": [] }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_response_contains_authorization() {
        let cfg = warm_flow_config_data();
        let names: Vec<&str> = cfg["tokenNameList"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        assert!(names.contains(&"Authorization"));
    }
}
