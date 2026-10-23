//! flow_assignee_sync 领域模型(outbox 补偿任务行)。

use app_core::vo::page::PageQuery;
use serde::{Deserialize, Serialize};

use crate::assignee_sync::repository::entity::flow_assignee_sync;

/// 一条办理人重解析补偿任务。
///
/// `user_ids` 在库里是 JSON 数组字符串(实体列 text),领域层同样以
/// 字符串承载 —— 序列化/反序列化是 enqueue/worker 的职责,
/// 模型不藏隐式转换。
///
/// 本表无 Java 对应物(模块头裁定),无平价 VO 可分;出站字段 camelCase
/// 对齐前端契约,`id` 由响应边界的 stringify_ids 统一转字符串。
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssigneeSync {
    pub id: i64,
    /// 变更对象类型: user / role / dept
    pub auth_type: String,
    /// 操作类型: add / update / delete
    pub oper_type: String,
    /// 变更对象标识(用户名/角色key),可为空
    pub auth_code: Option<String>,
    /// 波及用户ID(JSON 数组字符串),可为空
    pub user_ids: Option<String>,
    /// 0待处理 1已处理 2失败
    pub status: String,
    pub retry_count: i32,
    pub error_message: Option<String>,
    pub create_time: Option<chrono::NaiveDateTime>,
    pub update_time: Option<chrono::NaiveDateTime>,
}

impl From<&flow_assignee_sync::Model> for AssigneeSync {
    fn from(m: &flow_assignee_sync::Model) -> Self {
        Self {
            id: m.id,
            auth_type: m.auth_type.clone(),
            oper_type: m.oper_type.clone(),
            auth_code: m.auth_code.clone(),
            user_ids: m.user_ids.clone(),
            status: m.status.clone(),
            retry_count: m.retry_count,
            error_message: m.error_message.clone(),
            create_time: m.create_time,
            update_time: m.update_time,
        }
    }
}

/// 补偿任务分页查询 BO(list 端点)。
///
/// 三个过滤条件都是可选精确匹配(队列运维场景,值域由
/// auth_type/oper_type/status 的枚举性保证,无需模糊)。
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssigneeSyncPageQuery {
    #[serde(flatten)]
    pub page: PageQuery,
    /// 变更对象类型: user / role / dept
    pub auth_type: Option<String>,
    /// 操作类型: add / update / delete
    pub oper_type: Option<String>,
    /// 0待处理 1已处理 2失败
    pub status: Option<String>,
}
