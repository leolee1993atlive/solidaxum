use app_data::sea_orm::{self, *};

// 字段对齐 MySQL 真实 schema（ry_workflow.sql）：
//   id(PK), apply_code, leave_type, start_date, end_date, leave_days(int),
//   remark, status, create_dept, create_by(bigint), create_time,
//   update_by(bigint), update_time
//
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "test_leave")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: i64,
    pub apply_code: Option<String>,
    pub leave_type: String,
    pub start_date: Option<DateTime>,
    pub end_date: Option<DateTime>,
    pub leave_days: Option<i32>,
    pub remark: Option<String>,
    pub status: Option<String>,
    pub create_dept: Option<i64>,
    pub create_by: Option<i64>,
    pub create_time: Option<DateTime>,
    pub update_by: Option<i64>,
    pub update_time: Option<DateTime>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
