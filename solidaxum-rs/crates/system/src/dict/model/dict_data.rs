use app_core::{
    middleware::extractors::CurrentUser,
    model::entity::{BaseEntity, BaseVo},
    util::snowflake,
};
use serde::{Deserialize, Serialize};
use validator::Validate;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DictData {
    pub dict_code: Option<i64>,
    pub dict_sort: i32,
    pub dict_label: String,
    pub dict_value: String,
    pub dict_type: String,
    pub css_class: Option<String>,
    pub list_class: Option<String>,
    pub is_default: String,
    pub remark: Option<String>,
    #[serde(flatten)]
    pub base_entity: BaseEntity,
}

impl DictData {
    pub fn from_create_bo(bo: DictDataCreateBo, user: &CurrentUser) -> Self {
        Self {
            dict_code: Some(snowflake::next_id()),
            dict_sort: bo.dict_sort.unwrap_or(0),
            dict_label: bo.dict_label,
            dict_value: bo.dict_value,
            dict_type: bo.dict_type,
            css_class: bo.css_class,
            list_class: bo.list_class,
            is_default: bo.is_default.unwrap_or_else(|| "N".to_string()),
            remark: bo.remark,
            base_entity: BaseEntity::new(user.get_user_id()),
        }
    }

    pub fn from_update_bo(&mut self, bo: DictDataUpdateBo, user: &CurrentUser) {
        self.dict_sort = bo.dict_sort.unwrap_or(self.dict_sort);
        self.dict_label = bo.dict_label;
        self.dict_value = bo.dict_value;
        self.dict_type = bo.dict_type;
        self.css_class = bo.css_class;
        self.list_class = bo.list_class;
        self.is_default =
            bo.is_default.unwrap_or_else(|| self.is_default.clone());
        self.remark = bo.remark;
        self.base_entity.update(user.get_user_id());
    }

    pub fn to_list_vo(&self) -> DictDataListVo {
        DictDataListVo {
            dict_code: self.dict_code.unwrap_or_default(),
            dict_sort: self.dict_sort,
            dict_label: self.dict_label.clone(),
            dict_value: self.dict_value.clone(),
            dict_type: self.dict_type.clone(),
            css_class: self.css_class.clone(),
            list_class: self.list_class.clone(),
            is_default: self.is_default.clone(),
            remark: self.remark.clone(),
            base_vo: BaseVo::from_base(&self.base_entity),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct DictDataCreateBo {
    pub dict_sort: Option<i32>,
    #[validate(length(min = 1, max = 100))]
    pub dict_label: String,
    #[validate(length(min = 1, max = 100))]
    pub dict_value: String,
    #[validate(length(min = 1, max = 100))]
    pub dict_type: String,
    pub css_class: Option<String>,
    pub list_class: Option<String>,
    pub is_default: Option<String>,
    pub remark: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct DictDataUpdateBo {
    #[validate(required)]
    pub dict_code: Option<i64>,
    pub dict_sort: Option<i32>,
    #[validate(length(min = 1, max = 100))]
    pub dict_label: String,
    #[validate(length(min = 1, max = 100))]
    pub dict_value: String,
    #[validate(length(min = 1, max = 100))]
    pub dict_type: String,
    pub css_class: Option<String>,
    pub list_class: Option<String>,
    pub is_default: Option<String>,
    pub remark: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DictDataListVo {
    pub dict_code: i64,
    pub dict_sort: i32,
    pub dict_label: String,
    pub dict_value: String,
    pub dict_type: String,
    pub css_class: Option<String>,
    pub list_class: Option<String>,
    pub is_default: String,
    pub remark: Option<String>,
    #[serde(flatten)]
    pub base_vo: BaseVo,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DictDataPageQuery {
    #[serde(flatten)]
    pub page: app_core::vo::page::PageQuery,
    pub dict_type: Option<String>,
    pub dict_label: Option<String>,
}

/// 字典数据导出行(对齐 Java SysDictDataVo 的 @ExcelProperty)。
/// 仅用于 export;不支持导入。
#[derive(Clone, Debug)]
pub struct DictDataExportRow {
    pub dict_code: i64,
    pub dict_sort: i32,
    pub dict_label: String,
    pub dict_value: String,
    pub dict_type: String,
    pub is_default: String,
    pub remark: String,
}

impl app_core::util::excel::ExcelRow for DictDataExportRow {
    fn headers() -> Vec<&'static str> {
        vec![
            "字典编码",
            "字典排序",
            "字典标签",
            "字典键值",
            "字典类型",
            "是否默认",
            "备注",
        ]
    }

    fn to_row(&self) -> Vec<String> {
        vec![
            self.dict_code.to_string(),
            self.dict_sort.to_string(),
            self.dict_label.clone(),
            self.dict_value.clone(),
            self.dict_type.clone(),
            self.is_default.clone(),
            self.remark.clone(),
        ]
    }

    fn from_record(_: &[String]) -> app_core::error::AppResult<Self> {
        Err(app_core::error::AppError::BadRequest(
            "字典数据不支持导入".to_string(),
        ))
    }
}
