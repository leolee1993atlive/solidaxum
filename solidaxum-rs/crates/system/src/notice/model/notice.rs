use app_core::{
    middleware::extractors::CurrentUser,
    model::{
        entity::{BaseEntity, BaseVo},
        status::Status,
    },
    util::snowflake,
};
use serde::{Deserialize, Serialize};
use validator::Validate;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Notice {
    pub notice_id: Option<i64>,
    pub notice_title: String,
    pub notice_type: String,
    pub notice_content: Option<String>,
    pub status: String,
    pub remark: Option<String>,
    #[serde(flatten)]
    pub base_entity: BaseEntity,
}

impl Notice {
    pub fn from_create_bo(bo: NoticeCreateBo, user: &CurrentUser) -> Self {
        Self {
            notice_id: Some(snowflake::next_id()),
            notice_title: bo.notice_title,
            notice_type: bo.notice_type.unwrap_or_else(|| "1".to_string()),
            notice_content: bo.notice_content,
            status: bo
                .status
                .unwrap_or_else(|| Status::Normal.as_str().to_string()),
            remark: bo.remark,
            base_entity: BaseEntity::new(user.get_user_id()),
        }
    }

    pub fn from_update_bo(&mut self, bo: NoticeUpdateBo, user: &CurrentUser) {
        self.notice_title = bo.notice_title;
        self.notice_type =
            bo.notice_type.unwrap_or_else(|| self.notice_type.clone());
        self.notice_content = bo.notice_content;
        self.status = bo.status.unwrap_or_else(|| self.status.clone());
        self.remark = bo.remark;
        self.base_entity.update(user.get_user_id());
    }

    pub fn to_list_vo(&self) -> NoticeListVo {
        NoticeListVo {
            notice_id: self.notice_id.unwrap_or_default(),
            notice_title: self.notice_title.clone(),
            notice_type: self.notice_type.clone(),
            notice_content: self.notice_content.clone(),
            status: self.status.clone(),
            remark: self.remark.clone(),
            create_by_name: None,
            base_vo: BaseVo::from_base(&self.base_entity),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct NoticeCreateBo {
    #[validate(length(min = 1, max = 50))]
    pub notice_title: String,
    pub notice_type: Option<String>,
    pub notice_content: Option<String>,
    pub status: Option<String>,
    pub remark: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct NoticeUpdateBo {
    #[validate(required)]
    pub notice_id: Option<i64>,
    #[validate(length(min = 1, max = 50))]
    pub notice_title: String,
    pub notice_type: Option<String>,
    pub notice_content: Option<String>,
    pub status: Option<String>,
    pub remark: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NoticeListVo {
    pub notice_id: i64,
    pub notice_title: String,
    pub notice_type: String,
    pub notice_content: Option<String>,
    pub status: String,
    pub remark: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub create_by_name: Option<String>,
    #[serde(flatten)]
    pub base_vo: BaseVo,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NoticePageQuery {
    #[serde(flatten)]
    pub page: app_core::vo::page::PageQuery,
    pub notice_title: Option<String>,
    pub notice_type: Option<String>,
    pub status: Option<String>,
    pub create_by_name: Option<String>,
}
