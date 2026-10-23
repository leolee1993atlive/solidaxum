use app_data::sea_orm::{self, *};

// 字段对齐 MySQL 真实 schema（ry_workflow.sql）：
//   id(PK), component_name, method_name, method_params, view_spel, remark,
//   status, del_flag, create_dept, create_by(bigint), create_time,
//   update_by(bigint), update_time
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "flow_spel")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: i64,
    pub component_name: Option<String>,
    pub method_name: Option<String>,
    pub method_params: Option<String>,
    pub view_spel: Option<String>,
    pub remark: Option<String>,
    pub status: Option<String>,
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
