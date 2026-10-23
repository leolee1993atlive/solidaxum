use app_core::util::snowflake;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OperLog {
    pub oper_id: Option<i64>,
    pub title: String,
    pub business_type: i32,
    pub method: Option<String>,
    pub request_method: String,
    pub operator_type: i32,
    pub oper_name: String,
    pub dept_name: Option<String>,
    pub oper_url: String,
    pub oper_ip: String,
    pub oper_location: String,
    pub oper_param: String,
    pub json_result: String,
    pub status: i32,
    pub error_msg: String,
    pub oper_time: Option<DateTime<Utc>>,
    pub cost_time: i64,
    pub user_id: Option<i64>,
    pub dept_id: Option<i64>,
    pub client_key: Option<String>,
    pub device_type: Option<String>,
    pub browser: Option<String>,
    pub os: Option<String>,
}

impl OperLog {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        title: String,
        business_type: i32,
        operator_type: i32,
        oper_name: String,
        oper_url: String,
        request_method: String,
        oper_ip: String,
        oper_param: String,
        json_result: String,
        status: i32,
        error_msg: String,
        cost_time: i64,
        user_id: Option<i64>,
        dept_id: Option<i64>,
        dept_name: Option<String>,
        client_key: Option<String>,
        device_type: Option<String>,
        browser: Option<String>,
        os: Option<String>,
    ) -> Self {
        Self {
            oper_id: Some(snowflake::next_id()),
            title,
            business_type,
            method: None,
            request_method,
            operator_type,
            oper_name,
            dept_name,
            oper_url,
            oper_ip,
            oper_location: String::new(),
            oper_param,
            json_result,
            status,
            error_msg,
            oper_time: Some(Utc::now()),
            cost_time,
            user_id,
            dept_id,
            client_key,
            device_type,
            browser,
            os,
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OperLogQuery {
    #[serde(flatten)]
    pub page: app_core::vo::page::PageQuery,
    pub title: Option<String>,
    pub business_type: Option<i32>,
    pub business_types: Option<Vec<i32>>,
    pub status: Option<i32>,
    pub oper_name: Option<String>,
    pub oper_ip: Option<String>,
    pub client_key: Option<String>,
    pub browser: Option<String>,
    pub os: Option<String>,
    pub device_type: Option<String>,
    #[serde(rename = "params[beginTime]")]
    pub begin_time: Option<String>,
    #[serde(rename = "params[endTime]")]
    pub end_time: Option<String>,
}

pub struct BusinessType;
impl BusinessType {
    pub const OTHER: i32 = 0;
    pub const INSERT: i32 = 1;
    pub const UPDATE: i32 = 2;
    pub const DELETE: i32 = 3;
    pub const GRANT: i32 = 4;
    pub const EXPORT: i32 = 5;
    pub const IMPORT: i32 = 6;
    pub const FORCE: i32 = 7;
    pub const GENCODE: i32 = 8;
    pub const CLEAN: i32 = 9;
}

pub struct OperatorType;
impl OperatorType {
    pub const OTHER: i32 = 0;
    pub const MANAGE: i32 = 1;
    pub const MOBILE: i32 = 2;
}

pub fn business_type_from_method(method: &str) -> i32 {
    match method {
        "POST" => BusinessType::INSERT,
        "PUT" => BusinessType::UPDATE,
        "DELETE" => BusinessType::DELETE,
        _ => BusinessType::OTHER,
    }
}

/// 操作日志导出行(对齐 Java SysOperLogVo 的 @ExcelProperty 核心列)。
#[derive(Clone, Debug)]
pub struct OperLogExportRow {
    pub oper_id: i64,
    pub title: String,
    pub business_type: i32,
    pub request_method: String,
    pub oper_name: String,
    pub oper_ip: String,
    pub status: i32,
    pub oper_time: String,
    pub cost_time: i64,
}

impl OperLogExportRow {
    pub fn from_oper_log(log: &OperLog) -> Self {
        Self {
            oper_id: log.oper_id.unwrap_or_default(),
            title: log.title.clone(),
            business_type: log.business_type,
            request_method: log.request_method.clone(),
            oper_name: log.oper_name.clone(),
            oper_ip: log.oper_ip.clone(),
            status: log.status,
            oper_time: log
                .oper_time
                .map(|t| t.to_rfc3339())
                .unwrap_or_default(),
            cost_time: log.cost_time,
        }
    }
}

impl app_core::util::excel::ExcelRow for OperLogExportRow {
    fn headers() -> Vec<&'static str> {
        vec![
            "日志主键",
            "操作模块",
            "业务类型",
            "请求方式",
            "操作人员",
            "操作地址",
            "状态",
            "操作时间",
            "消耗时间",
        ]
    }

    fn to_row(&self) -> Vec<String> {
        vec![
            self.oper_id.to_string(),
            self.title.clone(),
            self.business_type.to_string(),
            self.request_method.clone(),
            self.oper_name.clone(),
            self.oper_ip.clone(),
            self.status.to_string(),
            self.oper_time.clone(),
            self.cost_time.to_string(),
        ]
    }

    fn from_record(_: &[String]) -> app_core::error::AppResult<Self> {
        Err(app_core::error::AppError::BadRequest(
            "操作日志不支持导入".to_string(),
        ))
    }
}
