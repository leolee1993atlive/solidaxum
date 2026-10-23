use app_data::sea_orm::{self, *};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "flow_definition")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: i64,
    pub flow_code: String,
    pub flow_name: String,
    pub model_value: String,
    pub category: Option<String>,
    pub version: String,
    pub is_publish: i8,
    pub form_custom: Option<String>,
    pub form_path: Option<String>,
    pub activity_status: i8,
    pub listener_type: Option<String>,
    pub listener_path: Option<String>,
    pub ext: Option<String>,
    pub create_time: Option<DateTime>,
    pub create_by: Option<String>,
    pub update_time: Option<DateTime>,
    pub update_by: Option<String>,
    pub del_flag: Option<String>,
    pub tenant_id: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::flow_instance::Entity")]
    Instance,
}

impl ActiveModelBehavior for ActiveModel {}

impl Related<super::flow_instance::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Instance.def()
    }
}
