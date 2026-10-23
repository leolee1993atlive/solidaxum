//! flow_assignee_sync 仓储(**全豁免模块**)。
//!
//! 裁定(无 Java 对应物,参考实现自有设计):outbox 队列数据非行级,
//! 每个读方法显式 `without_row_scope()`;DataPolicy 见
//! `system/src/policy.rs`(CONSTRAINED=false、全 None —— 无软删列、
//! 审计列时间戳为 `Option<NaiveDateTime>` 与漏斗盖章签名不匹配,
//! 由 service 手工盖,flow 引擎表 A 组同款裁定)。
//!
//! | 本文件方法 | 用途 | 漏斗路径 |
//! |---|---|---|
//! | `insert_in_txn` | 挂钩点同事务写入(enqueue) | `tx.insert_returning_nothing` |
//! | `select_by_id_in_txn` | 同事务回读(TDD 钉事务可见性) | `tx.select` + `without_row_scope()` |
//! | `select_all_in_txn` | 事务内全量回读(TDD 用) | `tx.select` + `without_row_scope()` |
//! | `select_page` | 管理端 list 分页查询(D4) | 逃生舱 `without_row_scope()` + `page()` |
//! | `select_pending` | worker 拉取待处理行(status='0' 或 失败且未超Retry) | 逃生舱 `all` + `without_row_scope()` |
//! | `mark_done_in_txn` | worker 消费成功置 '1' | `tx.update_where` |
//! | `mark_failed_in_txn` | worker 消费失败置 '2' + 重试计数 + 原因 | `tx.update_where` |

use app_core::{repo::str_trim_or_empty, vo::page::TableDataInfo};
use app_data::{
    DataContext, DataError, TxnContext,
    sea_orm::{
        ActiveValue::Set, ColumnTrait, Condition, EntityTrait, QueryFilter,
        QueryOrder,
    },
};

use crate::assignee_sync::{
    model::assignee_sync::{AssigneeSync, AssigneeSyncPageQuery},
    repository::entity::flow_assignee_sync::{ActiveModel, Column, Entity},
};

fn to_active_model(row: &AssigneeSync) -> ActiveModel {
    ActiveModel {
        id: Set(row.id),
        auth_type: Set(row.auth_type.clone()),
        oper_type: Set(row.oper_type.clone()),
        auth_code: Set(row.auth_code.clone()),
        user_ids: Set(row.user_ids.clone()),
        status: Set(row.status.clone()),
        retry_count: Set(row.retry_count),
        error_message: Set(row.error_message.clone()),
        create_time: Set(row.create_time),
        update_time: Set(row.update_time),
    }
}

/// 补偿任务数据访问(无状态关联函数;写路径一律事务内 —— outbox 行的
/// 存在性必须与业务变更同事务,见模块头设计锚)
pub struct AssigneeSyncRepository;

impl AssigneeSyncRepository {
    /// 事务内插入一条补偿任务(enqueue 的唯一写入口)。
    pub async fn insert_in_txn(
        tx: &TxnContext<'_>,
        row: &AssigneeSync,
    ) -> Result<(), DataError> {
        tx.insert_returning_nothing(to_active_model(row)).await?;
        Ok(())
    }

    /// 事务内按主键回读(TDD 钉「同事务可见」)。
    pub async fn select_by_id_in_txn(
        tx: &TxnContext<'_>,
        id: i64,
    ) -> Result<Option<AssigneeSync>, DataError> {
        let model = tx
            .select(Entity::find_by_id(id))
            .without_row_scope()
            .one()
            .await?;
        Ok(model.as_ref().map(AssigneeSync::from))
    }

    /// 事务内全量回读(TDD 钉「同事务可见」)。
    pub async fn select_all_in_txn(
        tx: &TxnContext<'_>,
    ) -> Result<Vec<AssigneeSync>, DataError> {
        let models =
            tx.select(Entity::find()).without_row_scope().all().await?;
        Ok(models.iter().map(AssigneeSync::from).collect())
    }

    /// 分页查询补偿任务(D4 管理端 list)。全豁免:队列数据非行级,
    /// 三个过滤条件为可选精确匹配,按 id 倒序(新的补偿在前)。
    pub async fn select_page(
        ctx: &DataContext<'_>,
        query: &AssigneeSyncPageQuery,
        page_no: u64,
        page_size: u64,
    ) -> Result<TableDataInfo<AssigneeSync>, DataError> {
        let mut select = Entity::find();

        let auth_type = str_trim_or_empty(&query.auth_type);
        if !auth_type.is_empty() {
            select = select.filter(Column::AuthType.eq(&auth_type));
        }
        let oper_type = str_trim_or_empty(&query.oper_type);
        if !oper_type.is_empty() {
            select = select.filter(Column::OperType.eq(&oper_type));
        }
        let status = str_trim_or_empty(&query.status);
        if !status.is_empty() {
            select = select.filter(Column::Status.eq(&status));
        }

        let page = ctx
            .select(select.order_by_desc(Column::Id))
            .without_row_scope()
            .page(page_no as i64, page_size as i64)
            .await?;
        let rows: Vec<AssigneeSync> =
            page.rows.iter().map(AssigneeSync::from).collect();
        Ok(TableDataInfo::from_page_rows(&page, rows))
    }

    /// worker 拉取待处理行:status='0'(待处理)或
    /// (status='2' 且 retry_count < max_retry)(失败未超重试上限),
    /// 按 id 升序(先到先得,先发生的变更先补偿)。
    pub async fn select_pending(
        ctx: &DataContext<'_>,
        max_retry: u32,
    ) -> Result<Vec<AssigneeSync>, DataError> {
        let models = ctx
            .select(
                Entity::find()
                    .filter(
                        Condition::any().add(Column::Status.eq("0")).add(
                            Condition::all()
                                .add(Column::Status.eq("2"))
                                .add(Column::RetryCount.lt(max_retry as i32)),
                        ),
                    )
                    .order_by_asc(Column::Id),
            )
            .without_row_scope()
            .all()
            .await?;
        Ok(models.iter().map(AssigneeSync::from).collect())
    }

    /// 消费成功:status='1',盖 update_time。
    pub async fn mark_done_in_txn(
        tx: &TxnContext<'_>,
        id: i64,
        update_time: chrono::NaiveDateTime,
    ) -> Result<(), DataError> {
        tx.update_where::<Entity>(
            Condition::all().add(Column::Id.eq(id)),
            ActiveModel {
                status: Set("1".to_string()),
                update_time: Set(Some(update_time)),
                ..Default::default()
            },
        )
        .await?;
        Ok(())
    }

    /// 消费失败:status='2',retry_count 置为传入值(调用方 = 旧值+1),
    /// error_message 截断为 2000 字符(与 DDL varchar(2000) 对齐),
    /// 盖 update_time。0 行(行已被并发消费)不报错 —— worker 幂等。
    pub async fn mark_failed_in_txn(
        tx: &TxnContext<'_>,
        id: i64,
        retry_count: i32,
        error_message: &str,
        update_time: chrono::NaiveDateTime,
    ) -> Result<(), DataError> {
        let truncated: String = error_message.chars().take(2000).collect();
        tx.update_where::<Entity>(
            Condition::all().add(Column::Id.eq(id)),
            ActiveModel {
                status: Set("2".to_string()),
                retry_count: Set(retry_count),
                error_message: Set(Some(truncated)),
                update_time: Set(Some(update_time)),
                ..Default::default()
            },
        )
        .await?;
        Ok(())
    }
}
