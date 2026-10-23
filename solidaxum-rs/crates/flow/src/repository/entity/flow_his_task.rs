use app_data::sea_orm::{self, *};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "flow_his_task")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: i64,
    pub definition_id: i64,
    pub instance_id: i64,
    pub task_id: i64,
    pub node_code: Option<String>,
    pub node_name: Option<String>,
    pub node_type: Option<i8>,
    pub target_node_code: Option<String>,
    pub target_node_name: Option<String>,
    pub approver: Option<String>,
    pub cooperate_type: i8,
    pub collaborator: Option<String>,
    pub skip_type: String,
    pub flow_status: String,
    pub form_custom: Option<String>,
    pub form_path: Option<String>,
    pub message: Option<String>,
    pub variable: Option<String>,
    pub ext: Option<String>,
    pub create_time: Option<DateTime>,
    pub update_time: Option<DateTime>,
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
    #[sea_orm(
        belongs_to = "super::flow_instance::Entity",
        from = "Column::InstanceId",
        to = "super::flow_instance::Column::Id"
    )]
    Instance,
}

impl ActiveModelBehavior for ActiveModel {}

impl Related<super::flow_definition::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Definition.def()
    }
}

impl Related<super::flow_instance::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Instance.def()
    }
}
