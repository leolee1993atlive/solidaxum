// 实体定义经 app-data 受限 re-export:派生宏展开走 `sea_orm::…`
// 相对路径,由 self 绑定接管解析;NaiveDateTime 由受限 re-export
// 直接供给(与 chrono::NaiveDateTime 同型)。
use app_data::sea_orm::{self, *};
use serde::{Deserialize, Serialize};

#[derive(
    Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize,
)]
#[sea_orm(table_name = "sys_job")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub job_id: i64,
    pub job_name: String,
    pub job_group: String,
    pub invoke_target: String,
    pub cron_expression: String,
    pub job_params: Option<String>,
    /// 1=default 2=run_once 3=abandon
    pub misfire_policy: i8,
    /// 0=disallow concurrent 1=allow
    pub concurrent: i8,
    /// '0'=paused '1'=running
    pub status: String,
    pub remark: Option<String>,
    pub create_by: Option<String>,
    pub update_by: Option<String>,
    pub create_time: Option<NaiveDateTime>,
    pub update_time: Option<NaiveDateTime>,
    /// '0'=normal '2'=deleted
    pub del_flag: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
