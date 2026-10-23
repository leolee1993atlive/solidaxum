use app_data::sea_orm::{self, *};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "flow_skip")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: i64,
    pub definition_id: i64,
    pub now_node_code: String,
    pub now_node_type: Option<i8>,
    pub next_node_code: String,
    pub next_node_type: Option<i8>,
    // SQL flow_skip 表无 node_id 列（warm-flow Java 表结构亦无），此字段仅为运行时
    // 缓存（node_code -> node_id 映射），不入库。加 ignore 避免 INSERT/SELECT 生成该列，
    // 否则报 "None of the records are inserted"（实际为 Unknown column 'node_id'）。
    #[sea_orm(ignore)]
    pub node_id: Option<i64>,
    pub skip_name: Option<String>,
    pub skip_type: Option<String>,
    pub skip_condition: Option<String>,
    pub coordinate: Option<String>,
    pub create_time: Option<DateTime>,
    pub create_by: Option<String>,
    pub update_time: Option<DateTime>,
    pub update_by: Option<String>,
    pub del_flag: Option<String>,
    pub tenant_id: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
