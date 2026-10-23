// 实体定义经 app-data 受限 re-export,同 sys_job.rs 头注。
use app_data::sea_orm::{self, *};
use serde::{Deserialize, Serialize};

#[derive(
    Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize,
)]
#[sea_orm(table_name = "sys_job_log")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub job_log_id: i64,
    pub job_name: String,
    pub job_group: String,
    pub invoke_target: String,
    pub job_params: Option<String>,
    pub job_message: Option<String>,
    /// '0'=fail '1'=success
    pub status: String,
    pub exception_info: Option<String>,
    pub start_time: Option<NaiveDateTime>,
    pub stop_time: Option<NaiveDateTime>,
    pub create_time: Option<NaiveDateTime>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
