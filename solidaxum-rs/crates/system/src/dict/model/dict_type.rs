use app_core::{
    middleware::extractors::CurrentUser,
    model::entity::{BaseEntity, BaseVo},
    util::snowflake,
};
use serde::{Deserialize, Serialize};
use validator::Validate;

/// 校验字典类型格式,对齐 Java RegexConstants.DICTIONARY_TYPE = "^[a-z][a-z0-9_]*$"
/// (小写字母开头,后续仅允许小写字母/数字/下划线)。纯 char range 实现,零依赖。
fn validate_dict_type(value: &str) -> Result<(), validator::ValidationError> {
    let mut chars = value.chars();
    let ok = matches!(chars.next(), Some('a'..='z'))
        && chars.all(|c| matches!(c, 'a'..='z' | '0'..='9' | '_'));
    if ok {
        Ok(())
    } else {
        let mut err = validator::ValidationError::new("dict_type");
        err.message =
            Some("字典类型须以小写字母开头,仅含小写字母/数字/下划线".into());
        Err(err)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DictType {
    pub dict_id: Option<i64>,
    pub dict_name: String,
    pub dict_type: String,
    pub remark: Option<String>,
    #[serde(flatten)]
    pub base_entity: BaseEntity,
}

impl DictType {
    pub fn from_create_bo(bo: DictTypeCreateBo, user: &CurrentUser) -> Self {
        Self {
            dict_id: Some(snowflake::next_id()),
            dict_name: bo.dict_name,
            dict_type: bo.dict_type,
            remark: bo.remark,
            base_entity: BaseEntity::new(user.get_user_id()),
        }
    }

    pub fn from_update_bo(&mut self, bo: DictTypeUpdateBo, user: &CurrentUser) {
        self.dict_name = bo.dict_name;
        self.dict_type = bo.dict_type;
        self.remark = bo.remark;
        self.base_entity.update(user.get_user_id());
    }

    pub fn to_list_vo(&self) -> DictTypeListVo {
        DictTypeListVo {
            dict_id: self.dict_id.unwrap_or_default(),
            dict_name: self.dict_name.clone(),
            dict_type: self.dict_type.clone(),
            remark: self.remark.clone(),
            base_vo: BaseVo::from_base(&self.base_entity),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct DictTypeCreateBo {
    #[validate(length(min = 1, max = 100))]
    pub dict_name: String,
    #[validate(
        length(min = 1, max = 100),
        custom(function = "validate_dict_type")
    )]
    pub dict_type: String,
    pub remark: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct DictTypeUpdateBo {
    #[validate(required)]
    pub dict_id: Option<i64>,
    #[validate(length(min = 1, max = 100))]
    pub dict_name: String,
    #[validate(
        length(min = 1, max = 100),
        custom(function = "validate_dict_type")
    )]
    pub dict_type: String,
    pub remark: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DictTypeListVo {
    pub dict_id: i64,
    pub dict_name: String,
    pub dict_type: String,
    pub remark: Option<String>,
    #[serde(flatten)]
    pub base_vo: BaseVo,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DictTypePageQuery {
    #[serde(flatten)]
    pub page: app_core::vo::page::PageQuery,
    pub dict_name: Option<String>,
    pub dict_type: Option<String>,
}

/// 字典类型导出行(对齐 Java SysDictTypeVo 的 @ExcelProperty)。
/// 仅用于 export;不支持导入(from_record 返回错误)。
#[derive(Clone, Debug)]
pub struct DictTypeExportRow {
    pub dict_id: i64,
    pub dict_name: String,
    pub dict_type: String,
    pub remark: String,
    pub create_time: String,
}

impl app_core::util::excel::ExcelRow for DictTypeExportRow {
    fn headers() -> Vec<&'static str> {
        vec!["字典主键", "字典名称", "字典类型", "备注", "创建时间"]
    }

    fn to_row(&self) -> Vec<String> {
        vec![
            self.dict_id.to_string(),
            self.dict_name.clone(),
            self.dict_type.clone(),
            self.remark.clone(),
            self.create_time.clone(),
        ]
    }

    fn from_record(_: &[String]) -> app_core::error::AppResult<Self> {
        Err(app_core::error::AppError::BadRequest(
            "字典类型不支持导入".to_string(),
        ))
    }
}
