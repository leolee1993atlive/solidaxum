use app_core::util::snowflake;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoginInfo {
    pub info_id: Option<i64>,
    pub user_name: String,
    pub client_key: Option<String>,
    pub device_type: Option<String>,
    pub ipaddr: String,
    pub login_location: String,
    pub browser: String,
    pub os: String,
    pub status: String,
    pub msg: String,
    pub login_time: Option<DateTime<Utc>>,
}

impl LoginInfo {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        user_name: String,
        status: String,
        msg: String,
        ipaddr: String,
        browser: String,
        os: String,
        client_key: Option<String>,
        device_type: Option<String>,
    ) -> Self {
        Self {
            info_id: Some(snowflake::next_id()),
            user_name,
            client_key,
            device_type,
            login_location: login_location_from_ip(&ipaddr),
            ipaddr,
            browser,
            os,
            status,
            msg,
            login_time: Some(Utc::now()),
        }
    }
}

/// 根据 IP 推断登录地点:内网地址返回"内网IP",其余返回"未知"。
fn login_location_from_ip(ip: &str) -> String {
    if ip.is_empty()
        || ip.starts_with("127.")
        || ip.starts_with("192.168.")
        || ip.starts_with("10.")
        || ip.starts_with("172.")
    {
        "内网IP".to_string()
    } else {
        "未知".to_string()
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoginInfoQuery {
    #[serde(flatten)]
    pub page: app_core::vo::page::PageQuery,
    pub user_name: Option<String>,
    pub status: Option<String>,
    pub ipaddr: Option<String>,
    #[serde(rename = "params[beginTime]")]
    pub begin_time: Option<String>,
    #[serde(rename = "params[endTime]")]
    pub end_time: Option<String>,
}

/// 登录日志导出行(对齐 Java SysLoginInfoVo 的 @ExcelProperty)。
#[derive(Clone, Debug)]
pub struct LoginInfoExportRow {
    pub info_id: i64,
    pub user_name: String,
    pub client_key: String,
    pub device_type: String,
    pub status: String,
    pub ipaddr: String,
    pub login_location: String,
    pub browser: String,
    pub os: String,
    pub msg: String,
    pub login_time: String,
}

impl LoginInfoExportRow {
    pub fn from_login_info(info: &LoginInfo) -> Self {
        Self {
            info_id: info.info_id.unwrap_or_default(),
            user_name: info.user_name.clone(),
            client_key: info.client_key.clone().unwrap_or_default(),
            device_type: info.device_type.clone().unwrap_or_default(),
            status: info.status.clone(),
            ipaddr: info.ipaddr.clone(),
            login_location: info.login_location.clone(),
            browser: info.browser.clone(),
            os: info.os.clone(),
            msg: info.msg.clone(),
            login_time: info
                .login_time
                .map(|t| t.to_rfc3339())
                .unwrap_or_default(),
        }
    }
}

impl app_core::util::excel::ExcelRow for LoginInfoExportRow {
    fn headers() -> Vec<&'static str> {
        vec![
            "序号",
            "用户账号",
            "客户端",
            "设备类型",
            "登录状态",
            "登录地址",
            "登录地点",
            "浏览器",
            "操作系统",
            "提示消息",
            "访问时间",
        ]
    }

    fn to_row(&self) -> Vec<String> {
        vec![
            self.info_id.to_string(),
            self.user_name.clone(),
            self.client_key.clone(),
            self.device_type.clone(),
            self.status.clone(),
            self.ipaddr.clone(),
            self.login_location.clone(),
            self.browser.clone(),
            self.os.clone(),
            self.msg.clone(),
            self.login_time.clone(),
        ]
    }

    fn from_record(_: &[String]) -> app_core::error::AppResult<Self> {
        Err(app_core::error::AppError::BadRequest(
            "登录日志不支持导入".to_string(),
        ))
    }
}
