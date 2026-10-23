use app_core::{
    error::{AppError, AppResult},
    middleware::extractors::CurrentUser,
    model::{
        entity::{BaseEntity, BaseVo},
        status::{DelFlag, Status},
    },
    util::snowflake,
};
use serde::{Deserialize, Serialize};
use validator::Validate;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SysClient {
    pub id: Option<i64>,
    pub client_id: String,
    pub client_key: String,
    pub client_secret: String,
    pub grant_type: String,
    pub device_type: Option<String>,
    pub access_path: Option<String>,
    pub ip_whitelist: Option<String>,
    pub active_timeout: i32,
    pub timeout: i32,
    pub status: String,
    pub del_flag: Option<String>,
    #[serde(flatten)]
    pub base_entity: BaseEntity,
}

impl SysClient {
    pub fn is_normal(&self) -> bool {
        self.status == Status::Normal.as_str()
    }

    pub fn supports_grant_type(&self, grant_type: &str) -> bool {
        self.grant_type.split(',').any(|g| g.trim() == grant_type)
    }

    pub fn from_create_bo(
        bo: ClientCreateBo,
        user: &CurrentUser,
    ) -> AppResult<Self> {
        let grant_type = match &bo.grant_type {
            Some(gt) => gt.clone(),
            None => bo
                .grant_type_list
                .as_ref()
                .map(|l| l.join(","))
                .unwrap_or_default(),
        };
        let client_id = bo.client_id.unwrap_or_else(|| {
            use md5::{Digest, Md5};
            let mut hasher = Md5::new();
            hasher.update(
                format!("{}{}", bo.client_key, bo.client_secret).as_bytes(),
            );
            format!("{:x}", hasher.finalize())
        });
        Ok(Self {
            id: Some(snowflake::next_id()),
            client_id,
            client_key: bo.client_key,
            client_secret: bo.client_secret,
            grant_type,
            device_type: bo.device_type,
            access_path: bo.access_path,
            ip_whitelist: bo.ip_whitelist,
            active_timeout: bo.active_timeout,
            timeout: bo.timeout,
            status: bo
                .status
                .unwrap_or_else(|| Status::Normal.as_str().to_string()),
            del_flag: Some(DelFlag::Alive.as_str().to_string()),
            base_entity: BaseEntity::new(user.get_user_id()),
        })
    }

    pub fn from_update_bo(&mut self, bo: ClientUpdateBo, user: &CurrentUser) {
        self.client_id = bo.client_id;
        self.client_key = bo.client_key;
        // GET 列表/详情把 clientSecret 掩码为 "******";编辑回提时若仍是掩码,
        // 说明用户未改密钥,跳过写入以免覆盖真实值(对齐 oss from_update_bo)。
        if bo.client_secret != "******" {
            self.client_secret = bo.client_secret;
        }
        self.grant_type = match &bo.grant_type {
            Some(gt) => gt.clone(),
            None => bo
                .grant_type_list
                .as_ref()
                .map(|l| l.join(","))
                .unwrap_or_else(|| self.grant_type.clone()),
        };
        self.device_type = bo.device_type;
        self.access_path = bo.access_path;
        self.ip_whitelist = bo.ip_whitelist;
        self.active_timeout = bo.active_timeout;
        self.timeout = bo.timeout;
        self.status = bo.status.unwrap_or_else(|| self.status.clone());
        self.base_entity.update(user.get_user_id());
    }

    pub fn to_list_vo(&self) -> ClientListVo {
        ClientListVo {
            id: self.id.unwrap_or_default(),
            client_id: self.client_id.clone(),
            client_key: self.client_key.clone(),
            client_secret: "******".to_string(),
            grant_type: self.grant_type.clone(),
            grant_type_list: split_comma_list(&self.grant_type),
            device_type: self.device_type.clone(),
            access_path: self.access_path.clone(),
            access_path_list: split_optional_comma_list(&self.access_path),
            ip_whitelist: self.ip_whitelist.clone(),
            ip_whitelist_list: split_optional_comma_list(&self.ip_whitelist),
            active_timeout: self.active_timeout,
            timeout: self.timeout,
            status: self.status.clone(),
            del_flag: self.del_flag.clone(),
            base_vo: BaseVo::from_base(&self.base_entity),
        }
    }
}

/// 逗号分隔字符串 → 数组(对齐 Java fillClientRuleFields / grantTypeList)。
fn split_comma_list(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(String::from)
        .collect()
}

fn split_optional_comma_list(value: &Option<String>) -> Vec<String> {
    value.as_deref().map(split_comma_list).unwrap_or_default()
}

#[derive(Clone, Debug, Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct ClientCreateBo {
    pub client_id: Option<String>,
    #[validate(length(min = 1, message = "客户端key不能为空"))]
    pub client_key: String,
    #[validate(length(min = 1, message = "客户端密钥不能为空"))]
    pub client_secret: String,
    pub grant_type: Option<String>,
    pub grant_type_list: Option<Vec<String>>,
    pub device_type: Option<String>,
    pub access_path: Option<String>,
    pub ip_whitelist: Option<String>,
    #[validate(range(min = 1, message = "活动超时时间必须大于0"))]
    pub active_timeout: i32,
    #[validate(range(min = 1, message = "超时时间必须大于0"))]
    pub timeout: i32,
    pub status: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct ClientUpdateBo {
    #[validate(required)]
    pub id: Option<i64>,
    #[validate(length(min = 1, message = "客户端ID不能为空"))]
    pub client_id: String,
    #[validate(length(min = 1, message = "客户端key不能为空"))]
    pub client_key: String,
    #[validate(length(min = 1, message = "客户端密钥不能为空"))]
    pub client_secret: String,
    pub grant_type: Option<String>,
    pub grant_type_list: Option<Vec<String>>,
    pub device_type: Option<String>,
    pub access_path: Option<String>,
    pub ip_whitelist: Option<String>,
    #[validate(range(min = 1, message = "活动超时时间必须大于0"))]
    pub active_timeout: i32,
    #[validate(range(min = 1, message = "超时时间必须大于0"))]
    pub timeout: i32,
    pub status: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct ClientChangeStatusBo {
    #[validate(length(min = 1, message = "客户端ID不能为空"))]
    pub client_id: String,
    #[validate(length(min = 1, message = "状态不能为空"))]
    pub status: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientListVo {
    pub id: i64,
    pub client_id: String,
    pub client_key: String,
    pub client_secret: String,
    pub grant_type: String,
    pub grant_type_list: Vec<String>,
    pub device_type: Option<String>,
    pub access_path: Option<String>,
    pub access_path_list: Vec<String>,
    pub ip_whitelist: Option<String>,
    pub ip_whitelist_list: Vec<String>,
    pub active_timeout: i32,
    pub timeout: i32,
    pub status: String,
    pub del_flag: Option<String>,
    #[serde(flatten)]
    pub base_vo: BaseVo,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientPageQuery {
    #[serde(flatten)]
    pub page: app_core::vo::page::PageQuery,
    pub client_key: Option<String>,
    pub client_secret: Option<String>,
    pub status: Option<String>,
}

/// 客户端导出行(对齐 Vue 客户端表格可见列,**不含 clientSecret**)。
#[derive(Clone, Debug)]
pub struct ClientExportRow {
    pub client_key: String,
    pub grant_type: String,
    pub device_type: Option<String>,
    pub status: String,
    pub create_time: String,
}

impl ClientExportRow {
    pub fn from_client(client: &SysClient) -> Self {
        Self {
            client_key: client.client_key.clone(),
            grant_type: client.grant_type.clone(),
            device_type: client.device_type.clone(),
            status: client.status.clone(),
            create_time: client
                .base_entity
                .create_time
                .map(|t| t.to_rfc3339())
                .unwrap_or_default(),
        }
    }
}

impl app_core::util::excel::ExcelRow for ClientExportRow {
    fn headers() -> Vec<&'static str> {
        vec!["客户端key", "授权类型", "设备类型", "状态", "创建时间"]
    }

    fn to_row(&self) -> Vec<String> {
        vec![
            self.client_key.clone(),
            self.grant_type.clone(),
            self.device_type.clone().unwrap_or_default(),
            self.status.clone(),
            self.create_time.clone(),
        ]
    }

    fn from_record(_: &[String]) -> AppResult<Self> {
        Err(AppError::BadRequest("客户端不支持导入".to_string()))
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use app_core::model::login_user::LoginUser;

    use super::*;

    fn test_user() -> CurrentUser {
        CurrentUser(LoginUser {
            user_id: 1,
            dept_id: None,
            username: "test".into(),
            nick_name: None,
            user_type: None,
            client_id: "pc".into(),
            client_key: None,
            device_type: None,
            permissions: HashSet::new(),
            roles: HashSet::new(),
            login_ip: None,
            login_date: None,
        })
    }

    fn sample_client(secret: &str) -> SysClient {
        SysClient {
            id: Some(1),
            client_id: "cid".into(),
            client_key: "pc".into(),
            client_secret: secret.into(),
            grant_type: "password".into(),
            device_type: None,
            access_path: None,
            ip_whitelist: None,
            active_timeout: 1800,
            timeout: 604800,
            status: Status::Normal.as_str().into(),
            del_flag: Some(DelFlag::Alive.as_str().into()),
            base_entity: BaseEntity::new(0),
        }
    }

    fn update_bo(secret: &str) -> ClientUpdateBo {
        ClientUpdateBo {
            id: Some(1),
            client_id: "cid".into(),
            client_key: "pc".into(),
            client_secret: secret.into(),
            grant_type: Some("password".into()),
            grant_type_list: None,
            device_type: None,
            access_path: None,
            ip_whitelist: None,
            active_timeout: 1800,
            timeout: 604800,
            status: None,
        }
    }

    #[test]
    fn from_update_bo_skips_masked_secret() {
        // GET 返回掩码 "******";编辑回提时不应把掩码落库覆盖真实密钥。
        let mut client = sample_client("REAL_SECRET");
        client.from_update_bo(update_bo("******"), &test_user());
        assert_eq!(
            client.client_secret, "REAL_SECRET",
            "掩码值不应覆盖真实 clientSecret"
        );
    }

    #[test]
    fn from_update_bo_applies_real_secret() {
        let mut client = sample_client("OLD_SECRET");
        client.from_update_bo(update_bo("NEW_SECRET"), &test_user());
        assert_eq!(client.client_secret, "NEW_SECRET", "真实新密钥应正常更新");
    }

    #[test]
    fn create_bo_accepts_grant_type_list_without_client_id() {
        let json = serde_json::json!({
            "clientKey": "pc",
            "clientSecret": "secret",
            "grantTypeList": ["password", "social"],
            "activeTimeout": 1800,
            "timeout": 604800
        });
        let bo: ClientCreateBo =
            serde_json::from_value(json).expect("应能反序列化 Vue 载荷");
        assert!(bo.client_id.is_none(), "clientId 应可缺省");
        assert_eq!(
            bo.grant_type_list,
            Some(vec!["password".to_string(), "social".to_string()])
        );
    }
}
