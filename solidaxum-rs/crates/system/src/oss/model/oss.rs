use app_core::{
    middleware::extractors::CurrentUser,
    model::entity::{BaseEntity, BaseVo},
    util::snowflake,
    vo::page::PageQuery,
};
use serde::{Deserialize, Serialize};
use validator::Validate;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SysOss {
    pub oss_id: Option<i64>,
    pub file_name: String,
    pub original_name: String,
    pub file_suffix: String,
    pub url: String,
    pub service: String,
    pub ext1: Option<String>,
    #[serde(flatten)]
    pub base_entity: BaseEntity,
}

impl SysOss {
    pub fn new_upload(
        file_name: String,
        original_name: String,
        file_suffix: String,
        url: String,
        service: String,
        user: &CurrentUser,
    ) -> Self {
        Self {
            oss_id: Some(snowflake::next_id()),
            file_name,
            original_name,
            file_suffix,
            url,
            service,
            ext1: None,
            base_entity: BaseEntity::new(user.get_user_id()),
        }
    }

    pub fn to_vo(&self) -> SysOssVo {
        SysOssVo {
            oss_id: self.oss_id.unwrap_or_default(),
            file_name: self.file_name.clone(),
            original_name: self.original_name.clone(),
            file_suffix: self.file_suffix.clone(),
            url: self.url.clone(),
            service: self.service.clone(),
            create_by_name: None,
            base_vo: BaseVo::from_base(&self.base_entity),
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SysOssVo {
    pub oss_id: i64,
    pub file_name: String,
    pub original_name: String,
    pub file_suffix: String,
    pub url: String,
    pub service: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub create_by_name: Option<String>,
    #[serde(flatten)]
    pub base_vo: BaseVo,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SysOssConfig {
    pub oss_config_id: Option<i64>,
    pub config_key: String,
    pub access_key: String,
    pub secret_key: String,
    pub bucket_name: String,
    pub prefix: String,
    pub endpoint: String,
    pub domain_url: String,
    pub is_https: String,
    pub region: String,
    pub status: String,
    pub access_policy: String,
    pub ext1: Option<String>,
    pub remark: Option<String>,
    #[serde(flatten)]
    pub base_entity: BaseEntity,
}

impl SysOssConfig {
    pub fn from_create_bo(bo: OssConfigCreateBo, user: &CurrentUser) -> Self {
        Self {
            oss_config_id: Some(snowflake::next_id()),
            config_key: bo.config_key,
            access_key: bo.access_key,
            secret_key: bo.secret_key,
            bucket_name: bo.bucket_name,
            prefix: bo.prefix.unwrap_or_default(),
            endpoint: bo.endpoint,
            domain_url: bo.domain_url.unwrap_or_default(),
            is_https: bo.is_https.unwrap_or_else(|| "N".to_string()),
            region: bo.region.unwrap_or_default(),
            status: bo.status.unwrap_or_else(|| "N".to_string()),
            access_policy: bo.access_policy.unwrap_or_else(|| "1".to_string()),
            ext1: None,
            remark: bo.remark,
            base_entity: BaseEntity::new(user.get_user_id()),
        }
    }

    pub fn from_update_bo(
        &mut self,
        bo: OssConfigUpdateBo,
        user: &CurrentUser,
    ) {
        self.config_key = bo.config_key;
        if bo.access_key != "******" {
            self.access_key = bo.access_key;
        }
        if bo.secret_key != "******" {
            self.secret_key = bo.secret_key;
        }
        self.bucket_name = bo.bucket_name;
        self.prefix = bo.prefix.unwrap_or_else(|| self.prefix.clone());
        self.endpoint = bo.endpoint;
        self.domain_url =
            bo.domain_url.unwrap_or_else(|| self.domain_url.clone());
        self.is_https = bo.is_https.unwrap_or_else(|| self.is_https.clone());
        self.region = bo.region.unwrap_or_else(|| self.region.clone());
        self.status = bo.status.unwrap_or_else(|| self.status.clone());
        self.access_policy = bo
            .access_policy
            .unwrap_or_else(|| self.access_policy.clone());
        self.remark = bo.remark;
        self.base_entity.update(user.get_user_id());
    }

    pub fn to_vo(&self) -> OssConfigVo {
        OssConfigVo {
            oss_config_id: self.oss_config_id.unwrap_or_default(),
            config_key: self.config_key.clone(),
            access_key: self.access_key.clone(),
            secret_key: self.secret_key.clone(),
            bucket_name: self.bucket_name.clone(),
            prefix: self.prefix.clone(),
            endpoint: self.endpoint.clone(),
            domain_url: self.domain_url.clone(),
            is_https: self.is_https.clone(),
            region: self.region.clone(),
            status: self.status.clone(),
            access_policy: self.access_policy.clone(),
            ext1: self.ext1.clone(),
            remark: self.remark.clone(),
            base_vo: BaseVo::from_base(&self.base_entity),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct OssQueryBo {
    #[serde(flatten)]
    #[validate(nested)]
    pub page: PageQuery,
    pub file_name: Option<String>,
    pub original_name: Option<String>,
    pub file_suffix: Option<String>,
    pub service: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OssUploadVo {
    pub oss_id: i64,
    pub url: String,
    pub file_name: String,
    pub original_name: String,
    pub file_suffix: String,
    pub service: String,
}

#[derive(Clone, Debug, Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct OssConfigCreateBo {
    #[validate(length(min = 1, max = 20))]
    pub config_key: String,
    #[validate(length(min = 1, max = 255))]
    pub access_key: String,
    #[validate(length(min = 1, max = 255))]
    pub secret_key: String,
    #[validate(length(min = 1, max = 255))]
    pub bucket_name: String,
    #[validate(length(max = 255))]
    pub prefix: Option<String>,
    #[validate(length(min = 1, max = 255))]
    pub endpoint: String,
    #[validate(length(max = 255))]
    pub domain_url: Option<String>,
    pub is_https: Option<String>,
    pub region: Option<String>,
    pub status: Option<String>,
    pub access_policy: Option<String>,
    #[validate(length(max = 500))]
    pub remark: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct OssConfigUpdateBo {
    #[validate(required)]
    pub oss_config_id: Option<i64>,
    #[validate(length(min = 1, max = 20))]
    pub config_key: String,
    #[validate(length(min = 1, max = 255))]
    pub access_key: String,
    #[validate(length(min = 1, max = 255))]
    pub secret_key: String,
    #[validate(length(min = 1, max = 255))]
    pub bucket_name: String,
    #[validate(length(max = 255))]
    pub prefix: Option<String>,
    #[validate(length(min = 1, max = 255))]
    pub endpoint: String,
    #[validate(length(max = 255))]
    pub domain_url: Option<String>,
    pub is_https: Option<String>,
    pub region: Option<String>,
    pub status: Option<String>,
    pub access_policy: Option<String>,
    #[validate(length(max = 500))]
    pub remark: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OssConfigVo {
    pub oss_config_id: i64,
    pub config_key: String,
    pub access_key: String,
    pub secret_key: String,
    pub bucket_name: String,
    pub prefix: String,
    pub endpoint: String,
    pub domain_url: String,
    pub is_https: String,
    pub region: String,
    pub status: String,
    pub access_policy: String,
    pub ext1: Option<String>,
    pub remark: Option<String>,
    #[serde(flatten)]
    pub base_vo: BaseVo,
}

#[derive(Clone, Debug, Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct OssConfigQueryBo {
    #[serde(flatten)]
    #[validate(nested)]
    pub page: PageQuery,
    pub config_key: Option<String>,
    pub bucket_name: Option<String>,
    pub status: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct OssConfigChangeStatusBo {
    #[validate(required)]
    pub oss_config_id: Option<i64>,
    pub status: Option<String>,
}

#[cfg(test)]
mod oss_config_tests {
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

    #[test]
    fn from_update_bo_skips_masked_secrets() {
        let mut cfg = SysOssConfig {
            oss_config_id: Some(1),
            config_key: "minio".into(),
            access_key: "REAL_AK".into(),
            secret_key: "REAL_SK".into(),
            bucket_name: "b".into(),
            prefix: String::new(),
            endpoint: "http://x".into(),
            domain_url: String::new(),
            is_https: "N".into(),
            region: String::new(),
            status: "N".into(),
            access_policy: "1".into(),
            ext1: None,
            remark: None,
            base_entity: BaseEntity::new(0),
        };
        let bo = OssConfigUpdateBo {
            oss_config_id: Some(1),
            config_key: "minio".into(),
            access_key: "******".into(),
            secret_key: "******".into(),
            bucket_name: "b".into(),
            prefix: None,
            endpoint: "http://x".into(),
            domain_url: None,
            is_https: None,
            region: None,
            status: None,
            access_policy: None,
            remark: None,
        };
        cfg.from_update_bo(bo, &test_user());
        assert_eq!(cfg.access_key, "REAL_AK", "掩码值不应覆盖真实 accessKey");
        assert_eq!(cfg.secret_key, "REAL_SK", "掩码值不应覆盖真实 secretKey");
    }

    #[test]
    fn to_vo_returns_plaintext_secrets() {
        let cfg = SysOssConfig {
            oss_config_id: Some(1),
            config_key: "minio".into(),
            access_key: "PLAIN_AK".into(),
            secret_key: "PLAIN_SK".into(),
            bucket_name: "b".into(),
            prefix: String::new(),
            endpoint: "http://x".into(),
            domain_url: String::new(),
            is_https: "N".into(),
            region: String::new(),
            status: "N".into(),
            access_policy: "1".into(),
            ext1: None,
            remark: None,
            base_entity: BaseEntity::new(0),
        };
        let vo = cfg.to_vo();
        assert_eq!(vo.access_key, "PLAIN_AK");
        assert_eq!(vo.secret_key, "PLAIN_SK");
        let v = serde_json::to_value(&vo).unwrap();
        assert_eq!(v["accessKey"], "PLAIN_AK");
        assert_eq!(v["secretKey"], "PLAIN_SK");
    }
}
