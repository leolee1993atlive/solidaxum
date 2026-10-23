use app_data::sea_orm::{self, *};

// 字段对齐 MySQL 真实 schema（ry_workflow.sql）：
//   category_id(PK), parent_id, ancestors, category_name, order_num, del_flag,
//   create_dept, create_by(bigint), create_time, update_by(bigint), update_time
// 注意：ruoyi-workflow 自有表的 create_by/update_by 为 bigint，区别于 warm-flow 引擎表的 varchar(64)。
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "flow_category")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub category_id: i64,
    pub parent_id: i64,
    pub ancestors: Option<String>,
    pub category_name: String,
    pub order_num: i32,
    pub del_flag: Option<String>,
    pub create_dept: Option<i64>,
    pub create_by: Option<i64>,
    pub create_time: Option<DateTime>,
    pub update_by: Option<i64>,
    pub update_time: Option<DateTime>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
