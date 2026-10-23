use app_data::sea_orm::{self, *};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "flow_node")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: i64,
    pub node_type: i8,
    pub definition_id: i64,
    pub node_code: String,
    pub node_name: Option<String>,
    pub permission_flag: Option<String>,
    pub node_ratio: Option<String>,
    pub coordinate: Option<String>,
    pub any_node_skip: Option<String>,
    pub listener_type: Option<String>,
    pub listener_path: Option<String>,
    pub form_custom: Option<String>,
    pub form_path: Option<String>,
    pub version: String,
    pub create_time: Option<DateTime>,
    pub create_by: Option<String>,
    pub update_time: Option<DateTime>,
    pub update_by: Option<String>,
    pub ext: Option<String>,
    pub del_flag: Option<String>,
    pub tenant_id: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
