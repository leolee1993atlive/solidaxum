use app_data::sea_orm::{self, *};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "flow_user")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: i64,
    #[sea_orm(column_name = "type")]
    pub type_: String, // DB 列 `type` 为 char(1)，存 "1"/"2"/"3"；type 是 Rust 关键字故用 type_
    pub processed_by: Option<String>,
    pub associated: i64,
    pub create_time: Option<DateTime>,
    pub create_by: Option<String>,
    pub update_time: Option<DateTime>,
    pub update_by: Option<String>,
    pub del_flag: Option<String>,
    pub tenant_id: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::flow_task::Entity",
        from = "Column::Associated",
        to = "super::flow_task::Column::Id"
    )]
    Task,
}

impl ActiveModelBehavior for ActiveModel {}

impl Related<super::flow_task::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Task.def()
    }
}
