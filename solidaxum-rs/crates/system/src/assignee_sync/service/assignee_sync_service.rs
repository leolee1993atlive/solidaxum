//! 补偿任务 enqueue 服务(outbox 写侧)。
//!
//! 无状态关联函数集合 —— enqueue 的唯一入口是 `enqueue_tx`(事务内),
//! 刻意**不提供**连接路径写入口:outbox 行的可靠性恰在于
//! 「业务变更失败 ⟹ 补偿行不存在;补偿行存在 ⟹ 业务变更已提交」,
//! 非事务写会破坏该不变量。

use app_core::{util::snowflake, vo::page::TableDataInfo};
use app_data::{DataContext, DataError, TxnContext, sea_orm};

use crate::assignee_sync::{
    model::assignee_sync::{AssigneeSync, AssigneeSyncPageQuery},
    repository::assignee_sync_repo::AssigneeSyncRepository,
};

pub struct AssigneeSyncService;

impl AssigneeSyncService {
    /// 在调用方事务内向 flow_assignee_sync 写一行补偿任务。
    ///
    /// - `auth_type`: 变更对象类型(user/role/dept)
    /// - `oper_type`: 操作类型(add/update/delete)
    /// - `auth_code`: 变更对象标识(用户名/角色key),无则 None
    /// - `user_ids`: 波及用户ID;序列化为 JSON 数组字符串落库(空切片 → "[]")
    ///
    /// id 用雪花(`snowflake::next_id()`,flow crate persist.rs 同款);
    /// 时间戳手工盖 UTC(本表审计列无漏斗托管,见模块头)。
    pub async fn enqueue_tx(
        tx: &TxnContext<'_>,
        auth_type: &str,
        oper_type: &str,
        auth_code: Option<&str>,
        user_ids: &[i64],
    ) -> Result<(), DataError> {
        let now = chrono::Utc::now().naive_utc();
        let row = AssigneeSync {
            id: snowflake::next_id(),
            auth_type: auth_type.to_string(),
            oper_type: oper_type.to_string(),
            auth_code: auth_code.map(str::to_string),
            user_ids: Some(serde_json::to_string(user_ids).map_err(|e| {
                DataError::Backend(sea_orm::DbErr::Custom(format!(
                    "序列化补偿任务 user_ids 失败: {e}"
                )))
            })?),
            status: "0".to_string(),
            retry_count: 0,
            error_message: None,
            create_time: Some(now),
            update_time: Some(now),
        };
        AssigneeSyncRepository::insert_in_txn(tx, &row).await
    }

    /// 分页查询补偿任务(D4 管理端 list 端点;全豁免读,连接路径)。
    pub async fn page(
        ctx: &DataContext<'_>,
        query: &AssigneeSyncPageQuery,
        page_no: u64,
        page_size: u64,
    ) -> Result<TableDataInfo<AssigneeSync>, DataError> {
        AssigneeSyncRepository::select_page(ctx, query, page_no, page_size)
            .await
    }
}
