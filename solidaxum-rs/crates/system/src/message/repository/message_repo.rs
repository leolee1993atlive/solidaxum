//! message 仓储(**全豁免模块**)。
//!
//! 漏斗公共契约(执行面/错误)见 `app_data` 模块文档。**方法级裁定(Java 基线逐方法核对)**:现网
//! `SysMessageMapper.java` **零 `@DataPermission`**(接口体为空,全走
//! BaseMapperPlus)—— 站内消息的可见性由 send_user_ids(目标用户串,
//! "0" 表示全局)表达,不是数据权限。统一锚:**SysMessageMapper 无
//! @DataPermission —— 消息可见性由目标串表达,非行级**。由此读方法
//! 显式 `without_row_scope()`;sys_message 无软删列(policy.rs 如实)。
//!
//! | 本文件方法 | Java 锚(SysMessageController/ServiceImpl → SysMessageMapper) | @DataPermission | 漏斗路径 |
//! |---|---|---|---|
//! | `select_message_box` | getBox(Controller:32)→ `queryMessageBox`(SysMessageServiceImpl:76)→ `selectMessageList`(ServiceImpl:206)mapper.lambda() + `Page(1,100,false)` | ❌ | `without_row_scope()`(LIMIT/ORDER 在 Select 构件上) |
//!
//! **写面说明**:Java 的推送入库路径(storeMessage:SysMessageServiceImpl
//! 188-193,`messageMapper.insert`)在 Rust 尚未移植 —— 本 crate 的 push
//! 模块是 SSE 广播,不落库。后续移植写路径时走 `ctx.insert`(审计五件套
//! 漏斗盖),勿再引入裸 `Entity::insert`。

use app_data::{
    DataContext, DataError, DataSource,
    sea_orm::{
        self, ActiveModelBehavior, ColumnTrait, Condition, DateTimeUtc,
        DeriveEntityModel, DerivePrimaryKey, DeriveRelation, EntityTrait,
        EnumIter, PrimaryKeyTrait, QueryFilter, QueryOrder, QuerySelect,
    },
};
use chrono::Utc;

use crate::message::model::message::{MessageBoxVo, MessageVo};

/// 消息分类常量（对齐 Java SysMessageServiceImpl）。
const CATEGORY_SYSTEM: &str = "system";
const CATEGORY_NOTICE: &str = "notice";
const CATEGORY_WORKFLOW: &str = "workflow";

/// 全局广播用户标识（所有用户可见）。
const GLOBAL_USER_IDS: &str = "0";

/// 消息盒子每页展示最大条数。
const BOX_LIMIT: u64 = 100;

/// 消息盒子展示天数（仅展示近 N 天）。
const BOX_DAYS: i64 = 30;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "sys_message")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub message_id: i64,
    pub category: String,
    #[sea_orm(column_name = "type")]
    pub r#type: String,
    pub source: String,
    pub title: Option<String>,
    pub message: Option<String>,
    pub content: Option<String>,
    pub data_json: Option<String>,
    pub path: Option<String>,
    /// 目标用户ID串，0 表示全局。
    pub send_user_ids: String,
    pub create_dept: Option<i64>,
    pub create_by: Option<i64>,
    pub create_time: Option<DateTimeUtc>,
    pub update_by: Option<i64>,
    pub update_time: Option<DateTimeUtc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

#[derive(Clone)]
pub struct MessageRepository {
    /// 装配期快照:方法只转发调用方 ctx(豁免读不解析 resolver,
    /// 降级快照无影响)。
    ds: std::sync::Arc<DataSource>,
}

impl MessageRepository {
    pub fn new(ds: std::sync::Arc<DataSource>) -> Self {
        Self { ds }
    }

    /// 测试/工具桥接面:system_context。生产零消费,notice/client 同形。
    pub fn system_context(&self) -> DataContext<'_> {
        self.ds.system_context()
    }

    /// 查询当前用户消息盒子数据，按 system/notice/workflow 分类返回。
    ///
    /// 对齐 Java `selectMessageList`：仅近 30 天、每类最多 100 条、
    /// 按创建时间倒序；全局消息(`send_user_ids='0'`)或当前用户在接收范围内可见。
    ///
    /// 豁免裁定:SysMessageMapper 无 @DataPermission —— 消息可见性由
    /// 目标串表达,非行级(Java 锚 queryMessageBox
    /// SysMessageServiceImpl:76 → selectMessageList:206)。
    pub async fn select_message_box(
        &self,
        ctx: &DataContext<'_>,
        user_id: i64,
    ) -> Result<MessageBoxVo, DataError> {
        let since = Utc::now() - chrono::Duration::days(BOX_DAYS);
        Ok(MessageBoxVo {
            system_list: self
                .select_category_list(ctx, CATEGORY_SYSTEM, user_id, since)
                .await?,
            notice_list: self
                .select_category_list(ctx, CATEGORY_NOTICE, user_id, since)
                .await?,
            workflow_list: self
                .select_category_list(ctx, CATEGORY_WORKFLOW, user_id, since)
                .await?,
        })
    }

    /// 查询单个分类的消息列表(见模块头豁免锚)。
    ///
    /// 用户匹配条件用可移植的 LIKE 组合替代 MySQL 专用的 FIND_IN_SET，
    /// 以兼容 SQLite 测试环境：
    ///   send_user_ids = '0'                      (全局)
    ///   OR send_user_ids = '<uid>'               (精确单用户)
    ///   OR send_user_ids LIKE '<uid>,%'           (位于开头)
    ///   OR send_user_ids LIKE '%,<uid>'           (位于结尾)
    ///   OR send_user_ids LIKE '%,<uid>,%'         (位于中间)
    async fn select_category_list(
        &self,
        ctx: &DataContext<'_>,
        category: &str,
        user_id: i64,
        since: DateTimeUtc,
    ) -> Result<Vec<MessageVo>, DataError> {
        let uid = user_id.to_string();
        let rows = ctx
            .select(
                Entity::find()
                    .filter(Column::Category.eq(category))
                    .filter(Column::CreateTime.gte(since))
                    .filter(user_match_condition(&uid))
                    .order_by_desc(Column::CreateTime)
                    .order_by_desc(Column::MessageId)
                    .limit(BOX_LIMIT),
            )
            .without_row_scope()
            .all()
            .await?;
        Ok(rows.iter().map(MessageVo::from_model).collect())
    }
}

/// 构造「全局消息或当前用户在接收范围内」的查询条件。
fn user_match_condition(uid: &str) -> Condition {
    Condition::any()
        .add(Column::SendUserIds.eq(GLOBAL_USER_IDS))
        .add(Column::SendUserIds.eq(uid))
        .add(Column::SendUserIds.like(format!("{uid},%")))
        .add(Column::SendUserIds.like(format!("%,{uid}")))
        .add(Column::SendUserIds.like(format!("%,{uid},%")))
}
