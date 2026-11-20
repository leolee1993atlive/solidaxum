use app_core::{
    middleware::extractors::CurrentUser,
    model::{
        entity::{BaseEntity, BaseVo},
        status::Status,
    },
    util::snowflake,
};
use app_data::sea_orm::DateTimeUtc;
use serde::{Deserialize, Serialize};
use validator::Validate;

/// 公告目标类型:全部用户。
pub const TARGET_ALL: &str = "ALL";
/// 公告目标类型:指定用户。
pub const TARGET_USER: &str = "USER";
/// 发送状态:草稿(未发布)。
pub const SEND_STATUS_DRAFT: &str = "0";
/// 发送状态:已发布。
pub const SEND_STATUS_PUBLISHED: &str = "1";

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Notice {
    pub notice_id: Option<i64>,
    pub notice_title: String,
    pub notice_type: String,
    pub notice_content: Option<String>,
    pub status: String,
    /// 目标类型 ALL(全部) / USER(指定用户)。
    pub target_type: String,
    /// 指定用户 ID 串(逗号分隔,target_type=USER 时生效)。
    pub user_ids: Option<String>,
    /// 发送状态 0 草稿 / 1 已发布 / 2 已撤回。
    pub send_status: String,
    pub send_time: Option<DateTimeUtc>,
    pub remark: Option<String>,
    #[serde(flatten)]
    pub base_entity: BaseEntity,
}

impl Notice {
    pub fn from_create_bo(bo: NoticeCreateBo, user: &CurrentUser) -> Self {
        let target_type = normalize_target_type(bo.target_type.as_deref());
        let user_ids = normalize_user_ids(bo.user_ids.as_deref());
        Self {
            notice_id: Some(snowflake::next_id()),
            notice_title: bo.notice_title,
            notice_type: bo.notice_type.unwrap_or_else(|| "1".to_string()),
            notice_content: bo.notice_content,
            status: bo
                .status
                .unwrap_or_else(|| Status::Normal.as_str().to_string()),
            target_type,
            user_ids,
            send_status: SEND_STATUS_DRAFT.to_string(),
            send_time: None,
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
        if let Some(target_type) = bo.target_type.as_deref() {
            self.target_type = normalize_target_type(Some(target_type));
        }
        if bo.user_ids.is_some() || self.target_type == TARGET_ALL {
            self.user_ids = normalize_user_ids(bo.user_ids.as_deref());
        }
        self.remark = bo.remark;
        self.base_entity.update(user.get_user_id());
    }

    /// 指定用户 ID 列表(target_type=USER 时解析 CSV)。
    pub fn target_user_id_list(&self) -> Vec<i64> {
        if self.target_type != TARGET_USER {
            return Vec::new();
        }
        self.user_ids
            .as_deref()
            .map(parse_user_ids)
            .unwrap_or_default()
    }

    /// 公告对指定用户是否可见:ALL 全部可见;USER 需命中 CSV。
    pub fn visible_to(&self, user_id: i64) -> bool {
        match self.target_type.as_str() {
            TARGET_USER => self.target_user_id_list().contains(&user_id),
            _ => true,
        }
    }

    pub fn to_list_vo(&self) -> NoticeListVo {
        NoticeListVo {
            notice_id: self.notice_id.unwrap_or_default(),
            notice_title: self.notice_title.clone(),
            notice_type: self.notice_type.clone(),
            notice_content: self.notice_content.clone(),
            status: self.status.clone(),
            target_type: self.target_type.clone(),
            user_ids: self.user_ids.clone(),
            send_status: self.send_status.clone(),
            send_time: self.send_time,
            remark: self.remark.clone(),
            create_by_name: None,
            base_vo: BaseVo::from_base(&self.base_entity),
        }
    }
}

fn normalize_target_type(value: Option<&str>) -> String {
    match value.map(str::trim).map(str::to_ascii_uppercase) {
        Some(v) if v == TARGET_USER => TARGET_USER.to_string(),
        _ => TARGET_ALL.to_string(),
    }
}

fn normalize_user_ids(value: Option<&str>) -> Option<String> {
    let ids = parse_user_ids(value.unwrap_or_default());
    if ids.is_empty() {
        None
    } else {
        Some(ids.iter().map(i64::to_string).collect::<Vec<_>>().join(","))
    }
}

fn parse_user_ids(value: &str) -> Vec<i64> {
    value
        .split(',')
        .filter_map(|s| s.trim().parse::<i64>().ok())
        .collect()
}

#[derive(Clone, Debug, Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct NoticeCreateBo {
    #[validate(length(min = 1, max = 50))]
    pub notice_title: String,
    pub notice_type: Option<String>,
    pub notice_content: Option<String>,
    pub status: Option<String>,
    pub target_type: Option<String>,
    pub user_ids: Option<String>,
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
    pub target_type: Option<String>,
    pub user_ids: Option<String>,
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
    pub target_type: String,
    pub user_ids: Option<String>,
    pub send_status: String,
    pub send_time: Option<DateTimeUtc>,
    pub remark: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub create_by_name: Option<String>,
    #[serde(flatten)]
    pub base_vo: BaseVo,
}

/// 当前用户可见公告(含已读回执)。
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NoticeMyVo {
    pub notice_id: i64,
    pub notice_title: String,
    pub notice_type: String,
    pub notice_content: Option<String>,
    pub remark: Option<String>,
    pub target_type: String,
    pub send_time: Option<DateTimeUtc>,
    pub read_flag: String,
    pub read_time: Option<DateTimeUtc>,
    pub star_flag: String,
    pub create_time: Option<DateTimeUtc>,
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

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NoticeMyQuery {
    #[serde(flatten)]
    pub page: app_core::vo::page::PageQuery,
}
