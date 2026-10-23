use app_core::model::entity::BaseEntity;
use serde::{Deserialize, Serialize};
use validator::Validate;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GenTableColumn {
    pub column_id: i64,
    pub table_id: i64,
    pub column_name: String,
    pub column_comment: Option<String>,
    pub column_type: String,
    pub java_type: String,
    pub java_field: String,
    pub is_pk: String,
    pub is_increment: String,
    pub is_required: String,
    pub is_insert: String,
    pub is_edit: String,
    pub is_list: String,
    pub is_query: String,
    pub query_type: String,
    pub html_type: String,
    pub dict_type: Option<String>,
    pub sort: i32,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub cap_java_field: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub super_column: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usable_column: Option<bool>,

    #[serde(flatten)]
    pub base_entity: BaseEntity,
}

impl GenTableColumn {
    fn is_flag_one(value: &str) -> bool {
        value == "1"
    }

    pub fn is_pk(&self) -> bool {
        Self::is_flag_one(&self.is_pk)
    }
    pub fn is_increment(&self) -> bool {
        Self::is_flag_one(&self.is_increment)
    }
    pub fn is_required(&self) -> bool {
        Self::is_flag_one(&self.is_required)
    }
    pub fn is_insert(&self) -> bool {
        Self::is_flag_one(&self.is_insert)
    }
    pub fn is_edit(&self) -> bool {
        Self::is_flag_one(&self.is_edit)
    }
    pub fn is_list(&self) -> bool {
        Self::is_flag_one(&self.is_list)
    }
    pub fn is_query(&self) -> bool {
        Self::is_flag_one(&self.is_query)
    }

    pub fn is_super_column(&self) -> bool {
        matches!(
            self.java_field.as_str(),
            "createBy"
                | "createTime"
                | "updateBy"
                | "updateTime"
                | "parentName"
                | "parentId"
        )
    }

    pub fn is_usable_column(&self) -> bool {
        matches!(self.java_field.as_str(), "parentId" | "orderNum" | "remark")
    }

    pub fn is_dict_column(&self) -> bool {
        if let Some(ref dict) = self.dict_type {
            if dict.is_empty() {
                return false;
            }
            matches!(
                self.html_type.as_str(),
                "select" | "radio" | "checkbox" | "switch"
            )
        } else {
            false
        }
    }
}

#[derive(Clone, Debug, Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct GenColumnUpdateBo {
    pub column_id: i64,
    pub table_id: i64,
    pub column_name: String,
    pub column_comment: Option<String>,
    pub column_type: String,
    pub java_type: String,
    pub java_field: String,
    pub is_pk: String,
    pub is_increment: String,
    pub is_required: String,
    pub is_insert: String,
    pub is_edit: String,
    pub is_list: String,
    pub is_query: String,
    pub query_type: String,
    pub html_type: String,
    pub dict_type: Option<String>,
    pub sort: i32,
}
