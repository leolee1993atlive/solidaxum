use app_core::{model::entity::BaseEntity, vo::page::PageQuery};
use serde::{Deserialize, Serialize};
use validator::Validate;

use crate::codegen::model::gen_column::{GenColumnUpdateBo, GenTableColumn};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GenTable {
    pub table_id: i64,
    pub data_name: String,
    pub table_name: String,
    pub table_comment: String,
    pub class_name: String,
    pub tpl_category: String,
    pub frontend_type: String,
    pub package_name: String,
    pub module_name: String,
    pub business_name: String,
    pub function_name: String,
    pub function_author: String,
    pub gen_type: String,
    pub gen_path: String,
    pub options: Option<String>,
    pub remark: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub pk_column: Option<GenTableColumn>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub columns: Option<Vec<GenTableColumn>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tree_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tree_parent_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tree_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub menu_ids: Option<Vec<i64>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_menu_id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_menu_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enable_export: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enable_status: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status_field: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enable_unique: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unique_fields: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enable_sort: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sort_field: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tree_root_value: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tree_ancestors_field: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tree_order_field: Option<String>,

    #[serde(flatten)]
    pub base_entity: BaseEntity,
}

impl GenTable {
    pub const TPL_CRUD: &'static str = "crud";
    pub const TPL_TREE: &'static str = "tree";

    pub fn is_tree(&self) -> bool {
        self.tpl_category == Self::TPL_TREE
    }

    pub fn is_crud(&self) -> bool {
        self.tpl_category == Self::TPL_CRUD
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GenTableQueryBo {
    pub table_name: Option<String>,
    pub table_comment: Option<String>,
    pub data_name: Option<String>,
    #[serde(rename = "params[beginTime]")]
    pub begin_time: Option<String>,
    #[serde(rename = "params[endTime]")]
    pub end_time: Option<String>,
    #[serde(flatten)]
    pub page: PageQuery,
}

#[derive(Clone, Debug, Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct GenTableUpdateBo {
    pub table_id: i64,
    pub table_name: String,
    pub table_comment: String,
    pub class_name: String,
    pub tpl_category: String,
    pub frontend_type: String,
    pub package_name: String,
    pub module_name: String,
    pub business_name: String,
    pub function_name: String,
    pub function_author: String,
    pub gen_type: Option<String>,
    pub gen_path: Option<String>,
    pub options: Option<String>,
    pub remark: Option<String>,
    pub columns: Option<Vec<GenColumnUpdateBo>>,
    pub tree_code: Option<String>,
    pub tree_parent_code: Option<String>,
    pub tree_name: Option<String>,
    pub parent_menu_id: Option<i64>,
    pub parent_menu_name: Option<String>,
    pub enable_export: Option<bool>,
    pub enable_status: Option<bool>,
    pub status_field: Option<String>,
    pub enable_unique: Option<bool>,
    pub unique_fields: Option<Vec<String>>,
    pub enable_sort: Option<bool>,
    pub sort_field: Option<String>,
    pub tree_root_value: Option<String>,
    pub tree_ancestors_field: Option<String>,
    pub tree_order_field: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DbTableQueryBo {
    pub data_name: Option<String>,
    pub table_name: Option<String>,
    pub table_comment: Option<String>,
    #[serde(flatten)]
    pub page: PageQuery,
}

/// 数据库表导入对话框 VO（4 字段满足 Vue 契约；Java 复用 GenTable 为冗余，YAGNI）。
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DbTableVo {
    pub table_name: String,
    pub table_comment: String,
    pub create_time: Option<String>,
    pub update_time: Option<String>,
}
