use std::sync::Arc;

use axum::{
    Router,
    routing::{delete, get, post, put},
};

use crate::handler::{
    FlowState, category_handler, chart_handler, definition_handler,
    designer_handler, instance_handler, leave_handler, node_conf_handler,
    spel_handler, task_handler, variable_handler, warm_flow_ui_handler,
};

/// warm-flow 设计器**公开面**路由(不经过 auth 中间件)。
///
/// 锚 Java sa-token 白名单(RuoYi-Vue-Plus application.yml:100-113 的
/// excludes):公开面 = 设计器静态资源(`*.html/*.css/*.js` 等)+
/// `/warm-flow-ui/config`,仅此而已;其余 `/warm-flow/*` API 一律要求
/// 登录,见 [`designer_api_route`]。
pub fn designer_public_route() -> Router<Arc<FlowState>> {
    Router::new()
        // ── 静态文件 ─────────────────────────────────────────────────────────
        .route("/designer", get(designer_handler::serve_designer))
        .route("/designer/", get(designer_handler::serve_designer))
        .route(
            "/designer/{*path}",
            get(|path: Option<axum::extract::Path<String>>| async move {
                designer_handler::serve_designer(path).await
            }),
        )
        .route("/warm-flow-ui", get(designer_handler::serve_designer))
        .route("/warm-flow-ui/", get(designer_handler::serve_designer))
        .route(
            "/warm-flow-ui/{*path}",
            get(|path: Option<axum::extract::Path<String>>| async move {
                designer_handler::serve_designer(path).await
            }),
        )
        // ── config(公开面唯一的 API) ──────────────────────────────────────
        .route(
            "/warm-flow-ui/config",
            get(warm_flow_ui_handler::get_config),
        )
}

/// warm-flow 设计器 **API 面**路由。
///
/// 全部 `/warm-flow/*` API 由 admin/router.rs 经 `apply_auth` + oper_log
/// 装配,与 `flow_route` 同款形态。锚 Java 侧:sa-token excludes 只放静态与
/// config,其余 `/warm-flow/*`(含写端点 save-json 与用户/部门枚举
/// handler-result)全部要求登录。
pub fn designer_api_route() -> Router<Arc<FlowState>> {
    Router::new()
        .route(
            "/warm-flow/save-json",
            post(warm_flow_ui_handler::save_json),
        )
        .route("/warm-flow/query-def", get(warm_flow_ui_handler::query_def))
        .route(
            "/warm-flow/query-def/{id}",
            get(warm_flow_ui_handler::query_def),
        )
        .route(
            "/warm-flow/query-flow-chart/{id}",
            get(warm_flow_ui_handler::query_flow_chart),
        )
        .route(
            "/warm-flow/handler-type",
            get(warm_flow_ui_handler::handler_type),
        )
        .route(
            "/warm-flow/handler-result",
            get(warm_flow_ui_handler::handler_result),
        )
        .route(
            "/warm-flow/handler-feedback",
            get(warm_flow_ui_handler::handler_feedback),
        )
        .route(
            "/warm-flow/handler-dict",
            get(warm_flow_ui_handler::handler_dict),
        )
        .route(
            "/warm-flow/listener-list",
            get(warm_flow_ui_handler::listener_list),
        )
        .route("/warm-flow/node-ext", get(warm_flow_ui_handler::node_ext))
        .route(
            "/warm-flow/published-form",
            get(warm_flow_ui_handler::published_form),
        )
}

pub fn flow_route() -> Router<Arc<FlowState>> {
    Router::new()
        // category routes
        .route(
            "/workflow/category/list",
            get(category_handler::list_category),
        )
        .route(
            "/workflow/category/export",
            post(category_handler::export_category),
        )
        .route(
            "/workflow/category/categoryTree",
            get(category_handler::category_tree),
        )
        .route(
            "/workflow/category",
            post(category_handler::add_category)
                .put(category_handler::edit_category),
        )
        .route(
            "/workflow/category/{id}",
            get(category_handler::get_category)
                .delete(category_handler::delete_category),
        )
        // spel routes
        .route("/workflow/spel/list", get(spel_handler::list_spel))
        .route(
            "/workflow/spel",
            post(spel_handler::add_spel).put(spel_handler::edit_spel),
        )
        .route(
            "/workflow/spel/{ids}",
            get(spel_handler::get_spel).delete(spel_handler::delete_spel),
        )
        // variable routes
        .route(
            "/workflow/variable/list",
            get(variable_handler::list_variable),
        )
        .route(
            "/workflow/variable",
            post(variable_handler::add_variable)
                .put(variable_handler::edit_variable),
        )
        .route(
            "/workflow/variable/{ids}",
            get(variable_handler::get_variable)
                .delete(variable_handler::delete_variable),
        )
        // definition routes
        .route(
            "/workflow/definition/list",
            get(definition_handler::list_definition),
        )
        .route(
            "/workflow/definition/unPublishList",
            get(definition_handler::un_publish_list),
        )
        .route(
            "/workflow/definition",
            post(definition_handler::add_definition)
                .put(definition_handler::edit_definition),
        )
        .route(
            "/workflow/definition/publish/{id}",
            put(definition_handler::publish_definition),
        )
        .route(
            "/workflow/definition/unPublish/{id}",
            put(definition_handler::un_publish_definition),
        )
        .route(
            "/workflow/definition/copy/{id}",
            post(definition_handler::copy_definition),
        )
        .route(
            "/workflow/definition/active/{id}",
            put(definition_handler::active_definition),
        )
        .route(
            "/workflow/definition/importDef",
            post(definition_handler::import_def),
        )
        .route(
            "/workflow/definition/exportDef/{id}",
            post(definition_handler::export_def),
        )
        .route(
            "/workflow/definition/xmlString/{id}",
            get(definition_handler::xml_string),
        )
        .route(
            "/workflow/definition/{ids}",
            get(definition_handler::get_definition)
                .delete(definition_handler::delete_definition),
        )
        // nodeConf routes(单点更新 + 清单查询,对齐参考实现 FlowNodeConfController)
        .route(
            "/workflow/nodeConf/nodes",
            get(node_conf_handler::list_nodes),
        )
        .route(
            "/workflow/nodeConf/skips",
            get(node_conf_handler::list_skips),
        )
        .route(
            "/workflow/nodeConf/permissionFlag",
            put(node_conf_handler::update_permission_flag),
        )
        .route(
            "/workflow/nodeConf/skipCondition",
            put(node_conf_handler::update_skip_condition),
        )
        // instance routes
        .route(
            "/workflow/instance/pageByRunning",
            get(instance_handler::page_by_running),
        )
        .route(
            "/workflow/instance/pageByFinish",
            get(instance_handler::page_by_finish),
        )
        .route(
            "/workflow/instance/pageByCurrent",
            get(instance_handler::page_by_current),
        )
        .route(
            "/workflow/instance/getInfo/{business_id}",
            get(instance_handler::get_info),
        )
        .route(
            "/workflow/instance/deleteByBusinessIds/{ids}",
            delete(instance_handler::delete_by_business_ids),
        )
        .route(
            "/workflow/instance/deleteByInstanceIds/{ids}",
            delete(instance_handler::delete_by_instance_ids),
        )
        .route(
            "/workflow/instance/deleteHisByInstanceIds/{ids}",
            delete(instance_handler::delete_his_by_instance_ids),
        )
        .route(
            "/workflow/instance/cancelProcessApply",
            put(instance_handler::cancel_process_apply),
        )
        .route(
            "/workflow/instance/active/{id}",
            put(instance_handler::active_instance),
        )
        .route(
            "/workflow/instance/flowHisTaskList/{business_id}",
            get(instance_handler::flow_his_task_list),
        )
        .route(
            "/workflow/instance/instanceVariable/{instance_id}",
            get(instance_handler::instance_variable),
        )
        .route(
            "/workflow/instance/updateVariable",
            put(instance_handler::update_variable),
        )
        .route(
            "/workflow/instance/invalid",
            post(instance_handler::invalid_instance),
        )
        // task routes
        .route(
            "/workflow/task/startWorkFlow",
            post(task_handler::start_work_flow),
        )
        .route(
            "/workflow/task/completeTask",
            post(task_handler::complete_task),
        )
        .route(
            "/workflow/task/pageByTaskWait",
            get(task_handler::page_by_task_wait),
        )
        .route(
            "/workflow/task/pageByTaskFinish",
            get(task_handler::page_by_task_finish),
        )
        .route(
            "/workflow/task/pageByAllTaskWait",
            get(task_handler::page_by_all_task_wait),
        )
        .route(
            "/workflow/task/pageByAllTaskFinish",
            get(task_handler::page_by_all_task_finish),
        )
        .route(
            "/workflow/task/pageByTaskCopy",
            get(task_handler::page_by_task_copy),
        )
        .route(
            "/workflow/task/getTask/{task_id}",
            get(task_handler::get_task_vo),
        )
        .route(
            "/workflow/task/getNextNodeList",
            post(task_handler::get_next_node_list),
        )
        .route(
            "/workflow/task/terminationTask",
            post(task_handler::termination_task),
        )
        .route(
            "/workflow/task/taskOperation/{op}",
            post(task_handler::task_operation),
        )
        .route(
            "/workflow/task/updateAssignee/{user_id}",
            put(task_handler::update_assignee),
        )
        .route(
            "/workflow/task/backProcess",
            post(task_handler::back_process),
        )
        .route(
            "/workflow/task/getBackTaskNode/{task_id}/{now_node_code}",
            get(task_handler::get_back_task_node),
        )
        .route(
            "/workflow/task/currentTaskAllUser/{task_id}",
            get(task_handler::current_task_all_user),
        )
        .route("/workflow/task/urgeTask", post(task_handler::urge_task))
        //参考实现: 管理端兜底重解析(补偿记录查询在 system 侧
        // /system/assigneeSync/list)
        .route(
            "/workflow/task/reResolveAssignee",
            post(task_handler::re_resolve_assignee),
        )
        // leave routes
        .route("/workflow/leave/list", get(leave_handler::list_leave))
        .route("/workflow/leave/export", post(leave_handler::export_leave))
        .route(
            "/workflow/leave/submitAndFlowStart",
            post(leave_handler::submit_and_start),
        )
        .route(
            "/workflow/leave",
            post(leave_handler::add_leave).put(leave_handler::edit_leave),
        )
        .route(
            "/workflow/leave/{ids}",
            get(leave_handler::get_leave).delete(leave_handler::delete_leave),
        )
        // chart routes
        .route(
            "/workflow/chart/{instanceId}",
            get(chart_handler::get_chart),
        )
}
