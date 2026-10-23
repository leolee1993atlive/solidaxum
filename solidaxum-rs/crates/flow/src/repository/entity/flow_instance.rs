use app_data::sea_orm::{self, *};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "flow_instance")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: i64,
    pub definition_id: i64,
    pub business_id: String,
    pub node_type: i8,
    pub node_code: String,
    pub node_name: Option<String>,
    pub variable: Option<String>,
    pub flow_status: String,
    pub activity_status: i8,
    pub def_json: Option<String>,
    pub create_time: Option<DateTime>,
    pub create_by: Option<String>,
    pub update_time: Option<DateTime>,
    pub update_by: Option<String>,
    pub ext: Option<String>,
    pub del_flag: Option<String>,
    pub tenant_id: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::flow_definition::Entity",
        from = "Column::DefinitionId",
        to = "super::flow_definition::Column::Id"
    )]
    Definition,
}

impl ActiveModelBehavior for ActiveModel {}

impl Related<super::flow_definition::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Definition.def()
    }
}
