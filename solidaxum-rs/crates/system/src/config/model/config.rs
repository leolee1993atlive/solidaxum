use app_core::{
    error::{AppError, AppResult},
    middleware::extractors::CurrentUser,
    model::entity::{BaseEntity, BaseVo},
    util::snowflake,
};
use serde::{Deserialize, Serialize};
use validator::Validate;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Config {
    pub config_id: Option<i64>,
    pub config_name: String,
    pub config_key: String,
    pub config_value: String,
    pub config_type: String,
    pub remark: Option<String>,
    #[serde(flatten)]
    pub base_entity: BaseEntity,
}

impl Config {
    pub fn from_create_bo(bo: ConfigCreateBo, user: &CurrentUser) -> Self {
        Self {
            config_id: Some(snowflake::next_id()),
            config_name: bo.config_name,
            config_key: bo.config_key,
            config_value: bo.config_value,
            config_type: bo.config_type.unwrap_or_else(|| "N".to_string()),
            remark: bo.remark,
            base_entity: BaseEntity::new(user.get_user_id()),
        }
    }

    pub fn from_update_bo(&mut self, bo: ConfigUpdateBo, user: &CurrentUser) {
        self.config_name = bo.config_name;
        self.config_key = bo.config_key;
        self.config_value = bo.config_value;
        self.config_type =
            bo.config_type.unwrap_or_else(|| self.config_type.clone());
        self.remark = bo.remark;
        self.base_entity.update(user.get_user_id());
    }

    pub fn to_list_vo(&self) -> ConfigListVo {
        ConfigListVo {
            config_id: self.config_id.unwrap_or_default(),
            config_name: self.config_name.clone(),
            config_key: self.config_key.clone(),
            config_value: self.config_value.clone(),
            config_type: self.config_type.clone(),
            remark: self.remark.clone(),
            base_vo: BaseVo::from_base(&self.base_entity),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct ConfigCreateBo {
    #[validate(length(min = 1, max = 100))]
    pub config_name: String,
    #[validate(length(min = 1, max = 100))]
    pub config_key: String,
    #[validate(length(min = 1))]
    pub config_value: String,
    pub config_type: Option<String>,
    pub remark: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct ConfigUpdateBo {
    #[validate(required)]
    pub config_id: Option<i64>,
    #[validate(length(min = 1, max = 100))]
    pub config_name: String,
    #[validate(length(min = 1, max = 100))]
    pub config_key: String,
    #[validate(length(min = 1))]
    pub config_value: String,
    pub config_type: Option<String>,
    pub remark: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct ConfigUpdateByKeyBo {
    #[validate(length(min = 1, max = 100))]
    pub config_key: String,
    #[validate(length(min = 1, max = 500))]
    pub config_value: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigListVo {
    pub config_id: i64,
    pub config_name: String,
    pub config_key: String,
    pub config_value: String,
    pub config_type: String,
    pub remark: Option<String>,
    #[serde(flatten)]
    pub base_vo: BaseVo,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigPageQuery {
    #[serde(flatten)]
    pub page: app_core::vo::page::PageQuery,
    pub config_name: Option<String>,
    pub config_key: Option<String>,
    pub config_type: Option<String>,
}

/// 参数配置导出行(对齐 Vue 参数配置表格可见列)。
#[derive(Clone, Debug)]
pub struct ConfigExportRow {
    pub config_id: i64,
    pub config_name: String,
    pub config_key: String,
    pub config_value: String,
    pub config_type: String,
    pub remark: String,
}

impl ConfigExportRow {
    pub fn from_config(config: &Config) -> Self {
        Self {
            config_id: config.config_id.unwrap_or_default(),
            config_name: config.config_name.clone(),
            config_key: config.config_key.clone(),
            config_value: config.config_value.clone(),
            config_type: config.config_type.clone(),
            remark: config.remark.clone().unwrap_or_default(),
        }
    }
}

impl app_core::util::excel::ExcelRow for ConfigExportRow {
    fn headers() -> Vec<&'static str> {
        vec!["参数主键", "参数名称", "键名", "键值", "系统内置", "备注"]
    }

    fn to_row(&self) -> Vec<String> {
        vec![
            self.config_id.to_string(),
            self.config_name.clone(),
            self.config_key.clone(),
            self.config_value.clone(),
            self.config_type.clone(),
            self.remark.clone(),
        ]
    }

    fn from_record(_: &[String]) -> AppResult<Self> {
        Err(AppError::BadRequest("参数配置不支持导入".to_string()))
    }
}
