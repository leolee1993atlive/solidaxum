use std::collections::HashMap;

use app_core::util::serde_util::de_i64_from_any;
use serde::{Deserialize, Serialize};
use validator::Validate;

use crate::enums::ActivityStatus;

/// 切分逗号分隔的 id 列表，去空白与空项。前端多选 join(',') 后即为此形状。
pub fn split_id_list(raw: Option<&str>) -> Vec<String> {
    raw.filter(|s| !s.is_empty())
        .map(|s| {
            s.split(',')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

/// Task 分页查询的过滤参数（HTTP Query → Service → Repo 传递用）。
/// 字段为 snake_case；HTTP 侧通过 #[serde(rename_all = "camelCase")] 映射。
#[derive(Debug, Clone, Default)]
pub struct TaskPageFilter {
    /// 申请人 id 列表（前端「选择申请人」多选，逗号分隔）。
    pub create_by_ids: Option<String>,
    /// 当前节点名称模糊匹配。
    pub node_name: Option<String>,
    /// 流程定义名称模糊匹配。
    pub flow_name: Option<String>,
    /// 流程定义编码模糊匹配（对齐 Java `likeIfText`）。
    pub flow_code: Option<String>,
}

impl TaskPageFilter {
    /// 切分逗号分隔的申请人 id 列表，去空白与空项。
    ///
    /// 「申请人」语义统一为 `flow_instance.create_by`（对齐 Java
    /// `FlwTaskMapper` / `FlwHisTaskMapper` 的
    /// `inIfNotEmpty(.., FlowInstance::getCreateBy, ..)`）——
    /// **不是** `flow_task.create_by`（那是创建该任务行的操作者，
    /// 首节点之后即上一个审批人）。
    pub fn create_by_id_list(&self) -> Vec<String> {
        split_id_list(self.create_by_ids.as_deref())
    }
}

/// Instance 分页查询的过滤参数（HTTP Query → Service → Repo 传递用）。
#[derive(Debug, Clone, Default)]
pub struct InstancePageFilter {
    /// 流程定义编码模糊匹配。
    pub flow_code: Option<String>,
    /// 业务 id 模糊匹配。
    pub business_id: Option<String>,
    /// 流程分类 id（含子孙分类，对齐 Java）。
    pub category: Option<String>,
    /// 发起人 id 列表（逗号分隔）。
    pub create_by_ids: Option<String>,
    /// 当前节点名称模糊匹配。
    pub node_name: Option<String>,
    /// 流程定义名称模糊匹配。
    pub flow_name: Option<String>,
}

impl InstancePageFilter {
    /// 切分逗号分隔的发起人 id 列表（对应 `flow_instance.create_by`）。
    pub fn create_by_id_list(&self) -> Vec<String> {
        split_id_list(self.create_by_ids.as_deref())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DefinitionVo {
    pub id: i64,
    pub flow_code: String,
    pub flow_name: String,
    pub model_value: String,
    pub category: Option<String>,
    #[serde(rename = "categoryName", skip_serializing_if = "Option::is_none")]
    pub category_name: Option<String>,
    pub version: String,
    pub is_publish: i8,
    pub form_custom: Option<String>,
    pub form_path: Option<String>,
    pub activity_status: i8,
    pub listener_type: Option<String>,
    pub listener_path: Option<String>,
    pub ext: Option<String>,
    pub create_time: Option<chrono::NaiveDateTime>,
    pub create_by: Option<String>,
    pub update_time: Option<chrono::NaiveDateTime>,
    pub update_by: Option<String>,
}

impl From<&super::definition::Definition> for DefinitionVo {
    fn from(d: &super::definition::Definition) -> Self {
        Self {
            id: d.id.unwrap_or(0),
            flow_code: d.flow_code.clone(),
            flow_name: d.flow_name.clone(),
            model_value: d.model_value.clone(),
            category: d.category.clone(),
            category_name: None,
            version: d.version.clone(),
            is_publish: d.is_publish,
            form_custom: d.form_custom.clone(),
            form_path: d.form_path.clone(),
            activity_status: d.activity_status,
            listener_type: d.listener_type.clone(),
            listener_path: d.listener_path.clone(),
            ext: d.ext.clone(),
            create_time: d.create_time,
            create_by: d.create_by.clone(),
            update_time: d.update_time,
            update_by: d.update_by.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DefinitionListVo {
    pub id: i64,
    pub flow_code: String,
    pub flow_name: String,
    pub model_value: String,
    pub category: Option<String>,
    #[serde(rename = "categoryName", skip_serializing_if = "Option::is_none")]
    pub category_name: Option<String>,
    pub version: String,
    pub is_publish: i8,
    pub activity_status: i8,
    pub create_time: Option<chrono::NaiveDateTime>,
    pub create_by: Option<String>,
    pub update_time: Option<chrono::NaiveDateTime>,
}

impl From<&super::definition::Definition> for DefinitionListVo {
    fn from(d: &super::definition::Definition) -> Self {
        Self {
            id: d.id.unwrap_or(0),
            flow_code: d.flow_code.clone(),
            flow_name: d.flow_name.clone(),
            model_value: d.model_value.clone(),
            category: d.category.clone(),
            category_name: None,
            version: d.version.clone(),
            is_publish: d.is_publish,
            activity_status: d.activity_status,
            create_time: d.create_time,
            create_by: d.create_by.clone(),
            update_time: d.update_time,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceListVo {
    pub id: i64,
    pub definition_id: i64,
    pub business_id: String,
    pub node_type: i8,
    pub node_code: String,
    pub node_name: Option<String>,
    pub flow_status: String,
    pub activity_status: i8,
    pub create_time: Option<chrono::NaiveDateTime>,
    pub create_by: Option<String>,
    pub update_time: Option<chrono::NaiveDateTime>,
}

impl From<&super::instance::Instance> for InstanceListVo {
    fn from(i: &super::instance::Instance) -> Self {
        Self {
            id: i.id.unwrap_or(0),
            definition_id: i.definition_id,
            business_id: i.business_id.clone(),
            node_type: i.node_type,
            node_code: i.node_code.clone(),
            node_name: i.node_name.clone(),
            flow_status: i.flow_status.clone(),
            activity_status: i.activity_status,
            create_time: i.create_time,
            create_by: i.create_by.clone(),
            update_time: i.update_time,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskListVo {
    pub id: i64,
    pub instance_id: i64,
    pub node_code: String,
    pub node_name: Option<String>,
    pub node_type: i8,
    pub flow_status: String,
    pub create_time: Option<chrono::NaiveDateTime>,
}

impl From<&super::task::Task> for TaskListVo {
    fn from(t: &super::task::Task) -> Self {
        Self {
            id: t.id.unwrap_or(0),
            instance_id: t.instance_id,
            node_code: t.node_code.clone(),
            node_name: t.node_name.clone(),
            node_type: t.node_type,
            flow_status: t.flow_status.clone(),
            create_time: t.create_time,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FlowInstanceVo {
    pub id: i64,
    pub definition_id: i64,
    pub flow_name: Option<String>,
    pub flow_code: Option<String>,
    pub business_id: String,
    pub node_type: i8,
    pub node_code: String,
    pub node_name: Option<String>,
    pub variable: Option<serde_json::Value>,
    pub flow_status: String,
    #[serde(rename = "flowStatusName")]
    pub flow_status_label: Option<String>,
    pub activity_status: i8,
    /// 是否挂起。由 activity_status 派生,口径对齐 Vue(见 PIN11)。
    pub is_suspended: bool,
    pub form_custom: Option<String>,
    pub form_path: Option<String>,
    pub def_json: Option<serde_json::Value>,
    pub ext: Option<String>,
    pub version: Option<String>,
    pub category: Option<String>,
    #[serde(rename = "categoryName", skip_serializing_if = "Option::is_none")]
    pub category_name: Option<String>,
    #[serde(rename = "createByName", skip_serializing_if = "Option::is_none")]
    pub create_by_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub business_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub business_title: Option<String>,
    pub button_permission: serde_json::Value,
    pub create_time: Option<chrono::NaiveDateTime>,
    pub create_by: Option<String>,
    pub update_time: Option<chrono::NaiveDateTime>,
    pub update_by: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub del_flag: Option<String>,
}

impl FlowInstanceVo {
    pub fn status_label(status: &str) -> Option<String> {
        let label = match status {
            "0" => "待提交",
            "1" => "审批中",
            "2" => "已通过",
            "3" => "自动通过",
            "4" => "已终止",
            "5" => "已作废",
            "6" => "已取消",
            "7" => "已撤回",
            "8" => "已完成",
            "9" => "已驳回",
            "10" => "已作废",
            "11" => "已退回",
            "12" => "重新发起",
            "13" => "待处理",
            _ => return None,
        };
        Some(label.to_string())
    }

    pub fn from_instance(i: &super::instance::Instance) -> Self {
        let variable = i.variable.as_deref().and_then(|s| {
            if s.is_empty() {
                None
            } else {
                serde_json::from_str(s).ok()
            }
        });
        let def_json = i.def_json.as_deref().and_then(|s| {
            if s.is_empty() {
                None
            } else {
                serde_json::from_str(s).ok()
            }
        });
        Self {
            id: i.id.unwrap_or(0),
            definition_id: i.definition_id,
            flow_name: None,
            flow_code: None,
            business_id: i.business_id.clone(),
            node_type: i.node_type,
            node_code: i.node_code.clone(),
            node_name: i.node_name.clone(),
            variable,
            flow_status: i.flow_status.clone(),
            flow_status_label: Self::status_label(&i.flow_status),
            activity_status: i.activity_status,
            is_suspended: i.activity_status == ActivityStatus::Suspended as i8,
            form_custom: None,
            form_path: None,
            def_json,
            ext: i.ext.clone(),
            version: None,
            category: None,
            category_name: None,
            create_by_name: None,
            business_code: None,
            business_title: None,
            button_permission: serde_json::Value::Object(serde_json::Map::new()),
            create_time: i.create_time,
            create_by: i.create_by.clone(),
            update_time: i.update_time,
            update_by: i.update_by.clone(),
            del_flag: i.del_flag.clone(),
        }
    }
}

impl From<&super::instance::Instance> for FlowInstanceVo {
    fn from(i: &super::instance::Instance) -> Self {
        Self::from_instance(i)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HisTaskListVo {
    pub id: i64,
    pub instance_id: i64,
    pub node_code: Option<String>,
    pub node_name: Option<String>,
    pub skip_type: String,
    pub flow_status: String,
    pub approver: Option<String>,
    pub cooperate_type: Option<i8>,
    pub collaborator: Option<String>,
    pub message: Option<String>,
    pub target_node_code: Option<String>,
    pub target_node_name: Option<String>,
    pub create_time: Option<chrono::NaiveDateTime>,
}

impl From<&super::his_task::HisTask> for HisTaskListVo {
    fn from(h: &super::his_task::HisTask) -> Self {
        Self {
            id: h.id.unwrap_or(0),
            instance_id: h.instance_id,
            node_code: h.node_code.clone(),
            node_name: h.node_name.clone(),
            skip_type: h.skip_type.clone(),
            flow_status: h.flow_status.clone(),
            approver: h.approver.clone(),
            cooperate_type: Some(h.cooperate_type),
            collaborator: h.collaborator.clone(),
            message: h.message.clone(),
            target_node_code: h.target_node_code.clone(),
            target_node_name: h.target_node_name.clone(),
            create_time: h.create_time,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FlowTaskVo {
    pub id: i64,
    pub definition_id: i64,
    pub instance_id: i64,
    pub node_code: String,
    pub node_name: Option<String>,
    pub node_type: i8,
    pub flow_name: Option<String>,
    pub flow_code: Option<String>,
    pub flow_status: String,
    #[serde(rename = "flowStatusName")]
    pub flow_status_label: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    #[serde(rename = "categoryName", skip_serializing_if = "Option::is_none")]
    pub category_name: Option<String>,
    pub form_custom: Option<String>,
    pub form_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    pub assignee_ids: Option<String>,
    pub assignee_names: Option<String>,
    #[serde(rename = "createByName", skip_serializing_if = "Option::is_none")]
    pub create_by_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub processed_by: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub processed_by_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub node_ratio: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "type")]
    pub type_: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub apply_node: Option<bool>,
    pub button_permission: serde_json::Value,
    pub business_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub business_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub business_title: Option<String>,
    pub create_time: Option<String>,
    pub create_by: Option<String>,
    pub update_time: Option<chrono::NaiveDateTime>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub del_flag: Option<String>,
    // 对齐 Java FlowTaskVo:列表字段始终序列化为数组(空也为 []),
    // 前端 submitVerify.vue 依赖 copyList.map()/buttonList.forEach() 恒为数组。
    #[serde(default)]
    pub permission_list: Vec<String>,
    #[serde(default)]
    pub user_list: Vec<FlowUserVo>,
    #[serde(default)]
    pub button_list: Vec<ButtonPermissionVo>,
    #[serde(default)]
    pub copy_list: Vec<FlowCopyVo>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub var_list: Option<std::collections::HashMap<String, String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FlowHisTaskVo {
    pub id: i64,
    pub definition_id: i64,
    pub instance_id: i64,
    /// 关联的(原)任务 ID(对齐 Java FlowHisTaskVo.taskId)。
    pub task_id: i64,
    pub node_code: Option<String>,
    pub node_name: Option<String>,
    pub node_type: Option<i8>,
    /// 目标节点(对齐 Java targetNodeCode/targetNodeName)。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_node_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_node_name: Option<String>,
    pub flow_name: Option<String>,
    pub flow_code: Option<String>,
    pub flow_status: String,
    #[serde(rename = "flowStatusName")]
    pub flow_status_label: Option<String>,
    /// 任务状态(对齐 Java FlowHisTaskVo.flowTaskStatus / wf_task_status 字典)。
    /// 由历史任务记录的 cooperate_type/skip_type/flow_status 派生。
    pub flow_task_status: Option<String>,
    /// 协作方式 + 名称(对齐 Java cooperateType/cooperateTypeName)。
    pub cooperate_type: i8,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cooperate_type_name: Option<String>,
    /// 办理人 + 名称(对齐 Java approver/approverName)。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub approver: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub approver_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub collaborator: Option<String>,
    /// 跳转类型(对齐 Java skipType)。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skip_type: Option<String>,
    /// 审批意见(对齐 Java message)。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    /// 运行时长(对齐 Java runDuration,createTime→updateTime)。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub run_duration: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ext: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    #[serde(rename = "categoryName", skip_serializing_if = "Option::is_none")]
    pub category_name: Option<String>,
    pub form_custom: Option<String>,
    pub form_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    pub assignee_ids: Option<String>,
    pub assignee_names: Option<String>,
    #[serde(rename = "createByName", skip_serializing_if = "Option::is_none")]
    pub create_by_name: Option<String>,
    pub button_permission: serde_json::Value,
    pub business_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub business_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub business_title: Option<String>,
    pub create_time: Option<String>,
    pub create_by: Option<String>,
    pub update_time: Option<chrono::NaiveDateTime>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub del_flag: Option<String>,
    // 对齐 Java FlowHisTaskVo:列表字段始终序列化为数组(空也为 [])。
    #[serde(default)]
    pub permission_list: Vec<String>,
    #[serde(default)]
    pub user_list: Vec<FlowUserVo>,
    #[serde(default)]
    pub button_list: Vec<ButtonPermissionVo>,
    #[serde(default)]
    pub copy_list: Vec<FlowCopyVo>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub var_list: Option<std::collections::HashMap<String, String>>,
}

impl FlowHisTaskVo {
    /// 从历史任务记录派生 `flow_task_status`,值对齐 wf_task_status 字典。
    ///
    /// wf_task_status 字典(dict_value): cancel/pass/waiting/invalid/back/
    /// termination/transfer/depute/copy/sign/sign_off/timeout。
    /// 当前 schema 无独立任务状态列,故按 cooperate_type / skip_type /
    /// flow_status 综合推断。
    pub fn derive_flow_task_status(
        cooperate_type: i8,
        skip_type: Option<&str>,
        flow_status: &str,
    ) -> Option<String> {
        use crate::enums::CooperateType;

        // 非审批类协作方式直接映射
        match CooperateType::from_i8(cooperate_type) {
            Some(CooperateType::Transfer) => {
                return Some("transfer".to_string());
            }
            Some(CooperateType::Depute) => return Some("depute".to_string()),
            Some(CooperateType::AddSignature) => {
                return Some("sign".to_string());
            }
            Some(CooperateType::ReductionSignature) => {
                return Some("sign_off".to_string());
            }
            _ => {}
        }

        // 审批/会签/票签按 skip_type 与 flow_status 细分
        if skip_type == Some("REJECT") {
            return Some("back".to_string());
        }

        Some(
            match flow_status {
                "2" | "3" | "8" | "12" => "pass",
                "4" => "termination",
                "5" | "10" => "invalid",
                "6" | "7" => "cancel",
                "9" | "11" => "back",
                _ => "pass",
            }
            .to_string(),
        )
    }
}

/// 当前任务办理人视图对象
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CurrentTaskUserVo {
    pub user_id: i64,
    pub nick_name: String,
}

/// 按钮权限视图对象（对齐 Java ButtonPermissionVo）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ButtonPermissionVo {
    pub code: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    pub show: bool,
}

/// 流程抄送视图对象（对齐 Java FlowCopyVo）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FlowCopyVo {
    pub user_id: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nick_name: Option<String>,
}

/// 流程用户视图对象（用于 task userList）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FlowUserVo {
    pub user_id: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nick_name: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct FlowInstanceBizExtBo {
    #[serde(default)]
    pub business_code: Option<String>,
    #[serde(default)]
    pub business_title: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartProcessBo {
    pub business_id: String,
    pub flow_code: String,
    #[serde(default)]
    pub variables: HashMap<String, serde_json::Value>,
    #[serde(default)]
    pub biz_ext: FlowInstanceBizExtBo,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StartProcessReturnDTO {
    pub process_instance_id: i64,
    pub task_id: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompleteTaskBo {
    pub task_id: i64,
    #[serde(default)]
    pub message: Option<String>,
    #[serde(default)]
    pub variables: HashMap<String, serde_json::Value>,
    #[serde(default)]
    pub flow_copy_list: Vec<FlowCopyBo>,
    /// 弹窗选人(对齐 Java 基线 CompleteTaskBo.assigneeMap):
    /// key = 下一节点编码,value = 用户ID(逗号串或数组)
    #[serde(default)]
    pub assignee_map: Option<HashMap<String, serde_json::Value>>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FlowCopyBo {
    pub user_id: i64,
    #[serde(default)]
    pub user_name: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskOperationBo {
    #[serde(deserialize_with = "de_i64_from_any")]
    pub task_id: i64,
    #[serde(default)]
    pub user_id: Option<String>,
    #[serde(default)]
    pub user_ids: Vec<String>,
    #[serde(default)]
    pub message: Option<String>,
}

impl TaskOperationBo {
    /// 合并 user_id(单值)与 user_ids(数组)为统一办理人列表
    pub fn handlers(&self) -> Vec<String> {
        let mut v = self.user_ids.clone();
        if let Some(uid) = &self.user_id
            && !uid.is_empty()
            && !v.contains(uid)
        {
            v.push(uid.clone());
        }
        v
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackProcessBo {
    pub task_id: i64,
    pub node_code: String,
    #[serde(default)]
    pub message: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FlowTerminationBo {
    pub task_id: i64,
    #[serde(default)]
    pub message: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FlowNextNodeBo {
    pub task_id: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FlowNextNodeVo {
    pub node_code: String,
    pub node_name: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FlowUrgeTaskBo {
    #[serde(deserialize_with = "de_i64_from_any")]
    pub task_id: i64,
    #[serde(default)]
    pub message: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackNodeVo {
    pub node_code: Option<String>,
    pub node_name: Option<String>,
}

#[derive(Debug, Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct CategoryAddBo {
    #[validate(length(min = 1, message = "分类名称不能为空"))]
    pub category_name: String,
    #[validate(range(min = 0))]
    pub parent_id: i64,
    #[validate(range(min = 0))]
    pub order_num: i32,
}

#[derive(Debug, Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct CategoryEditBo {
    pub category_id: i64,
    #[validate(length(min = 1, message = "分类名称不能为空"))]
    pub category_name: String,
    #[validate(range(min = 0))]
    pub parent_id: i64,
    #[validate(range(min = 0))]
    pub order_num: i32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CategoryQueryBo {
    pub category_name: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CategoryVo {
    pub category_id: i64,
    pub parent_id: i64,
    #[serde(rename = "parentName", skip_serializing_if = "Option::is_none")]
    pub parent_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ancestors: Option<String>,
    pub category_name: String,
    pub order_num: i32,
    pub create_time: Option<chrono::NaiveDateTime>,
}

impl From<&crate::repository::entity::flow_category::Model> for CategoryVo {
    fn from(m: &crate::repository::entity::flow_category::Model) -> Self {
        Self {
            category_id: m.category_id,
            parent_id: m.parent_id,
            parent_name: None,
            ancestors: m.ancestors.clone(),
            category_name: m.category_name.clone(),
            order_num: m.order_num,
            create_time: m.create_time,
        }
    }
}

/// categoryTree 接口的树节点（对齐 Java hutool Tree 形态）：
///   { id(String=categoryId), parentId(String), label(=categoryName),
///     weight(=orderNum), children }
/// Vue 端 tree-select/value 绑定此形态，不能用 categoryName/orderNum。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CategoryTreeVo {
    pub id: String,
    pub parent_id: String,
    pub label: String,
    pub weight: i64,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<CategoryTreeVo>,
}

impl CategoryTreeVo {
    pub fn build_tree(
        categories: &[crate::repository::entity::flow_category::Model],
        parent_id: i64,
    ) -> Vec<CategoryTreeVo> {
        let mut children: Vec<CategoryTreeVo> = categories
            .iter()
            .filter(|c| c.parent_id == parent_id)
            .map(|c| CategoryTreeVo {
                id: c.category_id.to_string(),
                parent_id: c.parent_id.to_string(),
                label: c.category_name.clone(),
                weight: i64::from(c.order_num),
                children: Self::build_tree(categories, c.category_id),
            })
            .collect();
        children.sort_by(|a, b| {
            a.weight.cmp(&b.weight).then_with(|| a.id.cmp(&b.id))
        });
        children
    }
}

#[derive(Debug, Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct SpelAddBo {
    pub component_name: Option<String>,
    pub method_name: Option<String>,
    pub method_params: Option<String>,
    #[validate(length(min = 1, message = "表达式不能为空"))]
    pub view_spel: String,
    pub remark: Option<String>,
    pub status: Option<String>,
}

#[derive(Debug, Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct SpelEditBo {
    pub id: i64,
    pub component_name: Option<String>,
    pub method_name: Option<String>,
    pub method_params: Option<String>,
    #[validate(length(min = 1, message = "表达式不能为空"))]
    pub view_spel: String,
    pub remark: Option<String>,
    pub status: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpelQueryBo {
    pub component_name: Option<String>,
    pub method_name: Option<String>,
    pub method_params: Option<String>,
    pub view_spel: Option<String>,
    pub status: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpelVo {
    pub id: i64,
    pub component_name: Option<String>,
    pub method_name: Option<String>,
    pub method_params: Option<String>,
    pub view_spel: Option<String>,
    pub remark: Option<String>,
    pub status: Option<String>,
    pub create_time: Option<chrono::NaiveDateTime>,
}

impl From<&crate::repository::entity::flow_spel::Model> for SpelVo {
    fn from(m: &crate::repository::entity::flow_spel::Model) -> Self {
        Self {
            id: m.id,
            component_name: m.component_name.clone(),
            method_name: m.method_name.clone(),
            method_params: m.method_params.clone(),
            view_spel: m.view_spel.clone(),
            remark: m.remark.clone(),
            status: m.status.clone(),
            create_time: m.create_time,
        }
    }
}

#[derive(Debug, Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct VariableAddBo {
    #[validate(length(min = 1, max = 64, message = "变量名长度须为 1-64"))]
    pub variable_name: String,
    #[validate(length(min = 1, max = 64, message = "中文名长度须为 1-64"))]
    pub label: String,
    #[validate(length(min = 1, max = 32, message = "数据类型长度须为 1-32"))]
    pub data_type: String,
    pub source: Option<String>,
    pub dict_type: Option<String>,
    pub remark: Option<String>,
    pub status: Option<String>,
}

#[derive(Debug, Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct VariableEditBo {
    pub id: i64,
    #[validate(length(min = 1, max = 64, message = "变量名长度须为 1-64"))]
    pub variable_name: String,
    #[validate(length(min = 1, max = 64, message = "中文名长度须为 1-64"))]
    pub label: String,
    #[validate(length(min = 1, max = 32, message = "数据类型长度须为 1-32"))]
    pub data_type: String,
    pub source: Option<String>,
    pub dict_type: Option<String>,
    pub remark: Option<String>,
    pub status: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VariableVo {
    pub id: i64,
    pub variable_name: String,
    pub label: String,
    pub data_type: String,
    pub source: Option<String>,
    pub dict_type: Option<String>,
    pub remark: Option<String>,
    pub status: String,
    pub create_time: Option<chrono::NaiveDateTime>,
}

impl From<&crate::repository::entity::flow_variable::Model> for VariableVo {
    fn from(m: &crate::repository::entity::flow_variable::Model) -> Self {
        Self {
            id: m.id,
            variable_name: m.variable_name.clone(),
            label: m.label.clone(),
            data_type: m.data_type.clone(),
            source: m.source.clone(),
            dict_type: m.dict_type.clone(),
            remark: m.remark.clone(),
            status: m.status.clone(),
            create_time: m.create_time,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeConfQuery {
    pub definition_id: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeConfVo {
    pub id: i64,
    pub node_code: String,
    pub node_name: Option<String>,
    pub node_type: i8,
    pub permission_flag: Option<String>,
    pub node_ratio: Option<String>,
}

impl From<&crate::model::node::Node> for NodeConfVo {
    fn from(n: &crate::model::node::Node) -> Self {
        Self {
            id: n.id.unwrap_or(0),
            node_code: n.node_code.clone(),
            node_name: n.node_name.clone(),
            node_type: n.node_type,
            permission_flag: n.permission_flag.clone(),
            node_ratio: n.node_ratio.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkipConfVo {
    pub id: i64,
    pub now_node_code: String,
    pub next_node_code: String,
    pub skip_name: Option<String>,
    pub skip_type: Option<String>,
    pub skip_condition: Option<String>,
}

impl From<&crate::model::skip::Skip> for SkipConfVo {
    fn from(s: &crate::model::skip::Skip) -> Self {
        Self {
            id: s.id.unwrap_or(0),
            now_node_code: s.now_node_code.clone(),
            next_node_code: s.next_node_code.clone(),
            skip_name: s.skip_name.clone(),
            skip_type: s.skip_type.clone(),
            skip_condition: s.skip_condition.clone(),
        }
    }
}

#[derive(Debug, Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct NodeConfPermissionBo {
    pub definition_id: i64,
    #[validate(length(min = 1, message = "节点编码不能为空"))]
    pub node_code: String,
    /// 允许置空(清空办理人配置),对齐 Java 的显式 set
    pub permission_flag: Option<String>,
}

#[derive(Debug, Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct SkipConfConditionBo {
    pub skip_id: i64,
    /// 允许置空(清空跳转条件即无条件通过)
    pub skip_condition: Option<String>,
}

#[derive(Debug, Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct LeaveAddBo {
    /// 主键。**仅 `submitAndFlowStart` 消费**:有值 = 就地更新既有单据后再发起流程,
    /// 为空 = 新建(对齐 Java TestLeaveServiceImpl#submitAndFlowStart 的 `insertOrUpdate`)。
    ///
    /// 🔴 少了它,从「修改」页点提交就会凭空多出一条单据、原草稿仍停在 draft。
    /// `POST /workflow/leave`(新增)走 `add()`,那里自己生成雪花 id,不读这个字段。
    pub id: Option<i64>,
    #[validate(length(min = 1, message = "请假类型不能为空"))]
    pub leave_type: String,
    #[validate(range(min = 1, message = "请假天数至少 1 天"))]
    pub leave_days: i32,
    pub remark: Option<String>,
    pub status: Option<String>,
    /// 流程编码(流程定义下拉选值),为空回退 "leave1"。对齐 Java TestLeaveBo.flowCode
    pub flow_code: Option<String>,
}

#[derive(Debug, Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct LeaveEditBo {
    pub id: i64,
    #[validate(length(min = 1, message = "请假类型不能为空"))]
    pub leave_type: String,
    #[validate(range(min = 1, message = "请假天数至少 1 天"))]
    pub leave_days: i32,
    pub remark: Option<String>,
    pub status: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LeaveQueryBo {
    pub leave_type: Option<String>,
    pub status: Option<String>,
    pub start_leave_days: Option<i32>,
    pub end_leave_days: Option<i32>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LeaveVo {
    pub id: i64,
    pub apply_code: Option<String>,
    pub leave_type: String,
    pub start_date: Option<chrono::NaiveDateTime>,
    pub end_date: Option<chrono::NaiveDateTime>,
    pub leave_days: Option<i32>,
    pub remark: Option<String>,
    pub status: Option<String>,
    pub create_time: Option<chrono::NaiveDateTime>,
}

impl From<&crate::repository::entity::test_leave::Model> for LeaveVo {
    fn from(m: &crate::repository::entity::test_leave::Model) -> Self {
        Self {
            id: m.id,
            apply_code: m.apply_code.clone(),
            leave_type: m.leave_type.clone(),
            start_date: m.start_date,
            end_date: m.end_date,
            leave_days: m.leave_days,
            remark: m.remark.clone(),
            status: m.status.clone(),
            create_time: m.create_time,
        }
    }
}

/// 请假导出行(对齐 Vue 请假表格可见列)。
///
/// 导出的是原始码值(`leave_type` 为 `1`、`status` 为 `draft`),与
/// role/post/dict 的既有导出一致 —— 那几个也都导码不导标签。
/// 要改成标签是全站议题,此处不动。
#[derive(Clone, Debug)]
pub struct LeaveExportRow {
    pub leave_type: String,
    pub start_date: String,
    pub end_date: String,
    pub leave_days: i32,
    pub remark: String,
    pub status: String,
}

impl LeaveExportRow {
    pub fn from_vo(vo: &LeaveVo) -> Self {
        Self {
            leave_type: vo.leave_type.clone(),
            start_date: vo
                .start_date
                .map(|d| d.format("%Y-%m-%d %H:%M:%S").to_string())
                .unwrap_or_default(),
            end_date: vo
                .end_date
                .map(|d| d.format("%Y-%m-%d %H:%M:%S").to_string())
                .unwrap_or_default(),
            leave_days: vo.leave_days.unwrap_or(0),
            remark: vo.remark.clone().unwrap_or_default(),
            status: vo.status.clone().unwrap_or_default(),
        }
    }
}

impl app_core::util::excel::ExcelRow for LeaveExportRow {
    fn headers() -> Vec<&'static str> {
        vec![
            "请假类型",
            "开始时间",
            "结束时间",
            "请假天数",
            "请假原因",
            "流程状态",
        ]
    }

    fn to_row(&self) -> Vec<String> {
        vec![
            self.leave_type.clone(),
            self.start_date.clone(),
            self.end_date.clone(),
            self.leave_days.to_string(),
            self.remark.clone(),
            self.status.clone(),
        ]
    }

    fn from_record(_: &[String]) -> app_core::error::AppResult<Self> {
        Err(app_core::error::AppError::BadRequest(
            "请假不支持导入".to_string(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn instance_vo_serializes_flow_status_name() {
        let inst = super::super::instance::Instance {
            id: Some(1),
            definition_id: 100,
            business_id: "biz_1".into(),
            node_type: 1,
            node_code: "start".into(),
            node_name: Some("开始节点".into()),
            variable: None,
            flow_status: "1".into(),
            activity_status: 1,
            def_json: None,
            create_time: None,
            create_by: None,
            update_time: None,
            update_by: None,
            ext: None,
            del_flag: None,
            tenant_id: None,
        };
        let vo = FlowInstanceVo::from(&inst);
        let v = serde_json::to_value(&vo).unwrap();
        assert!(
            v.get("flowStatusName").is_some(),
            "应为 flowStatusName（对齐 Java）"
        );
        assert!(
            v.get("flowStatusLabel").is_none(),
            "不应再有 flowStatusLabel"
        );
    }

    #[test]
    fn category_vo_uses_category_id_and_order_num() {
        let m = crate::repository::entity::flow_category::Model {
            category_id: 5,
            parent_id: 0,
            ancestors: Some("0".into()),
            category_name: "请假".into(),
            order_num: 3,
            del_flag: Some("0".into()),
            create_dept: None,
            create_by: None,
            create_time: None,
            update_by: None,
            update_time: None,
        };
        let v = serde_json::to_value(CategoryVo::from(&m)).unwrap();
        assert_eq!(v["categoryId"], 5);
        assert_eq!(v["categoryName"], "请假");
        assert_eq!(v["orderNum"], 3);
        assert!(
            v.get("id").is_none()
                && v.get("sort").is_none()
                && v.get("categoryCode").is_none()
                && v.get("status").is_none()
        );
    }

    #[test]
    fn category_tree_uses_hutool_shape() {
        use crate::repository::entity::flow_category::Model;
        let cats = vec![
            Model {
                category_id: 1,
                parent_id: 0,
                ancestors: Some("0".into()),
                category_name: "OA审批".into(),
                order_num: 0,
                del_flag: Some("0".into()),
                create_dept: None,
                create_by: None,
                create_time: None,
                update_by: None,
                update_time: None,
            },
            Model {
                category_id: 2,
                parent_id: 1,
                ancestors: Some("0,1".into()),
                category_name: "请假".into(),
                order_num: 1,
                del_flag: Some("0".into()),
                create_dept: None,
                create_by: None,
                create_time: None,
                update_by: None,
                update_time: None,
            },
        ];
        let tree = CategoryTreeVo::build_tree(&cats, 0);
        let v = serde_json::to_value(&tree).unwrap();
        let root = &v[0];
        // hutool Tree 形态：id/parentId 为 String，label 而非 categoryName，weight 而非 orderNum
        assert_eq!(root["id"], "1");
        assert_eq!(root["parentId"], "0");
        assert_eq!(root["label"], "OA审批");
        assert_eq!(root["weight"], 0);
        assert!(
            root.get("categoryName").is_none()
                && root.get("orderNum").is_none()
        );
        // 子节点存在
        assert_eq!(root["children"][0]["id"], "2");
        assert_eq!(root["children"][0]["label"], "请假");
    }

    #[test]
    fn spel_vo_uses_java_field_names() {
        let m = crate::repository::entity::flow_spel::Model {
            id: 1,
            component_name: Some("DeptComponent".into()),
            method_name: Some("selectDeptLeaderById".into()),
            method_params: Some("initiatorDeptId".into()),
            view_spel: Some("#{...}".into()),
            remark: Some("说明".into()),
            status: Some("0".into()),
            del_flag: Some("0".into()),
            create_dept: None,
            create_by: None,
            create_time: None,
            update_by: None,
            update_time: None,
        };
        let v = serde_json::to_value(SpelVo::from(&m)).unwrap();
        assert_eq!(v["componentName"], "DeptComponent");
        assert_eq!(v["methodName"], "selectDeptLeaderById");
        assert_eq!(v["methodParams"], "initiatorDeptId");
        assert_eq!(v["viewSpel"], "#{...}");
        assert_eq!(v["remark"], "说明");
        assert!(
            v.get("name").is_none()
                && v.get("spel").is_none()
                && v.get("description").is_none()
        );
    }

    #[test]
    fn leave_vo_uses_leave_days() {
        let m = crate::repository::entity::test_leave::Model {
            id: 1,
            leave_type: "事假".into(),
            remark: None,
            status: Some("0".into()),
            apply_code: Some("AP001".into()),
            start_date: None,
            end_date: None,
            leave_days: Some(3),
            create_dept: None,
            create_by: None,
            create_time: None,
            update_by: None,
            update_time: None,
        };
        let v = serde_json::to_value(LeaveVo::from(&m)).unwrap();
        assert_eq!(v["leaveDays"], 3);
        assert_eq!(v["applyCode"], "AP001");
        assert!(v.get("day").is_none());
    }
}
