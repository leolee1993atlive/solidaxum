use app_data::sea_orm::{self, *};

// 字段对齐参考实现 update_flow_registry.sql 的 DDL:
//   id(PK), variable_name(64, uk_flow_variable_name 唯一), label(64),
//   data_type(32, String/Long/Boolean), source(255, 注入点说明),
//   dict_type(100), remark(500), status('0'正常/'1'停用),
//   create_dept, create_by(bigint), create_time, update_by(bigint),
//   update_time。
// 与 flow_spel 不同:**无 del_flag**,删除一律物理删(见 variable_repo)。
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "flow_variable")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: i64,
    #[sea_orm(unique)]
    pub variable_name: String,
    pub label: String,
    pub data_type: String,
    pub source: Option<String>,
    pub dict_type: Option<String>,
    pub remark: Option<String>,
    pub status: String,
    pub create_dept: Option<i64>,
    pub create_by: Option<i64>,
    pub create_time: Option<DateTime>,
    pub update_by: Option<i64>,
    pub update_time: Option<DateTime>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
