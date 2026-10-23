//! flow_assignee_sync 表实体(参考实现自有 outbox 表,无 Java 对应)。
//!
//! 时间列用 `Option<NaiveDateTime>`(与 flow 引擎表 A 组同型),
//! 审计列由 service 手工盖(DataPolicy 全 None,见 system/src/policy.rs)。

use app_data::sea_orm::{
    self, ActiveModelBehavior, DateTime, DeriveEntityModel, DerivePrimaryKey,
    DeriveRelation, EntityTrait, EnumIter, PrimaryKeyTrait,
};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "flow_assignee_sync")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: i64,
    /// 变更对象类型: user/role/dept
    pub auth_type: String,
    /// 操作类型: add/update/delete
    pub oper_type: String,
    /// 变更对象标识(用户名/角色key)
    pub auth_code: Option<String>,
    /// 波及用户ID(JSON数组)
    pub user_ids: Option<String>,
    /// 0待处理 1已处理 2失败
    pub status: String,
    pub retry_count: i32,
    pub error_message: Option<String>,
    pub create_time: Option<DateTime>,
    pub update_time: Option<DateTime>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
