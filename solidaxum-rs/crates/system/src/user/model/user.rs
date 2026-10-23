use app_core::{
    error::{AppError, AppResult},
    model::{
        entity::{BaseEntity, BaseVo},
        status::{DelFlag, Status},
    },
    util::{crypto_util::hash_password, excel::ExcelRow, snowflake},
};
use lazy_static::lazy_static;
use regex::Regex;
use serde::{Deserialize, Serialize};
use validator::Validate;

lazy_static! {
    static ref STATUS_RE: Regex =
        Regex::new(r"^[01]$").expect("常量正则必合法");
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct User {
    pub user_id: Option<i64>,
    pub dept_id: Option<i64>,
    pub user_name: String,
    pub nick_name: String,
    #[serde(skip_serializing)]
    pub password: String,
    pub user_type: Option<String>,
    pub email: Option<String>,
    pub phone_number: Option<String>,
    pub gender: Option<String>,
    pub avatar: Option<i64>,
    pub status: String,
    pub del_flag: Option<String>,
    pub login_ip: Option<String>,
    pub login_date: Option<chrono::DateTime<chrono::Utc>>,
    pub remark: Option<String>,
    #[serde(flatten)]
    pub base_entity: BaseEntity,
}

impl User {
    /// 审计五列(create_dept/create_by/create_time/update_by/update_time)
    /// 不在领域层填 —— 漏斗 insert 按 ctx 的 principal 统一盖章,
    /// 领域层手工填的值会被漏斗覆盖。
    pub fn from_create_bo(bo: UserCreateBo) -> AppResult<Self> {
        Ok(Self {
            user_id: Some(snowflake::next_id()),
            dept_id: bo.dept_id,
            user_name: bo.user_name,
            nick_name: bo.nick_name,
            password: hash_password(&bo.password)?,
            user_type: Some("sys_user".to_string()),
            email: bo.email,
            phone_number: bo.phone_number.clone(),
            gender: bo.gender,
            avatar: None,
            status: bo
                .status
                .unwrap_or_else(|| Status::Normal.as_str().to_string()),
            del_flag: Some(DelFlag::Alive.as_str().to_string()),
            login_ip: None,
            login_date: None,
            remark: bo.remark,
            base_entity: BaseEntity::default(),
        })
    }

    /// update_* 审计列由漏斗的 update 路径按 ctx 盖章,领域层不手工盖章。
    #[allow(clippy::wrong_self_convention)]
    pub fn from_update_bo(&mut self, bo: UserUpdateBo) {
        self.dept_id = bo.dept_id;
        self.user_name = bo.user_name;
        self.nick_name = bo.nick_name;
        self.email = bo.email;
        self.phone_number = bo.phone_number.clone();
        self.gender = bo.gender;
        self.status = bo.status.unwrap_or_else(|| self.status.clone());
        self.remark = bo.remark;
    }

    pub fn to_list_vo(&self) -> UserListVo {
        UserListVo {
            user_id: self.user_id.unwrap_or_default(),
            dept_id: self.dept_id,
            user_name: self.user_name.clone(),
            nick_name: self.nick_name.clone(),
            email: self.email.clone(),
            phone_number: self.phone_number.clone(),
            gender: self.gender.clone(),
            status: self.status.clone(),
            del_flag: self.del_flag.clone(),
            remark: self.remark.clone(),
            dept_name: None,
            create_by_name: None,
            base_vo: BaseVo::from_base(&self.base_entity),
        }
    }

    pub fn to_detail_vo(&self) -> SysUserVo {
        SysUserVo {
            user_id: self.user_id.unwrap_or_default(),
            dept_id: self.dept_id,
            user_name: self.user_name.clone(),
            nick_name: self.nick_name.clone(),
            user_type: self.user_type.clone(),
            email: self.email.clone(),
            phone_number: self.phone_number.clone(),
            gender: self.gender.clone(),
            avatar: self.avatar,
            avatar_url: None,
            status: self.status.clone(),
            login_ip: self.login_ip.clone(),
            login_date: self.login_date,
            remark: self.remark.clone(),
            dept_name: None,
            dept: None,
            roles: Vec::new(),
            base_vo: BaseVo::from_base(&self.base_entity),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct UserCreateBo {
    #[validate(length(min = 1, message = "用户名不能为空"))]
    pub user_name: String,
    #[validate(length(min = 5, max = 20, message = "密码长度5-20位"))]
    pub password: String,
    pub dept_id: Option<i64>,
    pub nick_name: String,
    pub phone_number: Option<String>,
    pub email: Option<String>,
    pub gender: Option<String>,
    pub status: Option<String>,
    pub remark: Option<String>,
    pub post_ids: Option<Vec<i64>>,
    pub role_ids: Option<Vec<i64>>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct UserUpdateBo {
    #[validate(required)]
    pub user_id: Option<i64>,
    #[validate(length(min = 1, message = "用户名不能为空"))]
    pub user_name: String,
    pub dept_id: Option<i64>,
    pub nick_name: String,
    pub phone_number: Option<String>,
    pub email: Option<String>,
    pub gender: Option<String>,
    pub status: Option<String>,
    pub remark: Option<String>,
    pub post_ids: Option<Vec<i64>>,
    pub role_ids: Option<Vec<i64>>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct UserUpdatePasswordBo {
    #[validate(required)]
    pub user_id: Option<i64>,
    #[validate(length(
        min = 6,
        max = 50,
        message = "密码长度至少6位且不超过50位"
    ))]
    pub password: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserListVo {
    pub user_id: i64,
    pub dept_id: Option<i64>,
    pub user_name: String,
    pub nick_name: String,
    pub email: Option<String>,
    pub phone_number: Option<String>,
    pub gender: Option<String>,
    pub status: String,
    pub del_flag: Option<String>,
    pub remark: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dept_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub create_by_name: Option<String>,
    #[serde(flatten)]
    pub base_vo: BaseVo,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserInfo {
    pub user_id: i64,
    pub username: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileVo {
    pub user: ProfileUserVo,
    pub role_group: String,
    pub post_group: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileUserVo {
    pub user_id: i64,
    pub dept_id: Option<i64>,
    pub user_name: String,
    pub nick_name: String,
    pub user_type: Option<String>,
    pub email: Option<String>,
    pub phone_number: Option<String>,
    pub gender: Option<String>,
    /// 头像 OSS ID（对齐 RuoYi 6.X：avatar 保留原始 ID，不再被翻译覆盖）
    pub avatar: Option<i64>,
    /// 头像 URL（翻译后，对齐 RuoYi 6.X avatarUrl）
    pub avatar_url: Option<String>,
    pub login_ip: Option<String>,
    pub login_date: Option<chrono::DateTime<chrono::Utc>>,
    pub create_time: Option<chrono::DateTime<chrono::Utc>>,
    pub dept_name: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct SysUserProfileBo {
    pub nick_name: Option<String>,
    pub email: Option<String>,
    pub phone_number: Option<String>,
    pub gender: Option<String>,
    pub avatar: Option<i64>,
}

#[derive(Clone, Debug, Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct SysUserPasswordBo {
    #[validate(length(min = 6, max = 50))]
    pub old_password: String,
    #[validate(length(min = 6, max = 50))]
    pub new_password: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeptBrief {
    pub dept_id: i64,
    pub dept_name: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RoleBrief {
    pub role_id: i64,
    pub role_name: String,
    pub role_key: String,
    pub status: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PostBrief {
    pub post_id: i64,
    pub post_name: String,
    pub post_code: String,
    pub status: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SysUserVo {
    pub user_id: i64,
    pub dept_id: Option<i64>,
    pub user_name: String,
    pub nick_name: String,
    pub user_type: Option<String>,
    pub email: Option<String>,
    pub phone_number: Option<String>,
    pub gender: Option<String>,
    /// 头像 OSS ID（对齐 RuoYi 6.X：avatar 保留原始 ID）
    pub avatar: Option<i64>,
    /// 头像 URL（翻译后，对齐 RuoYi 6.X avatarUrl）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avatar_url: Option<String>,
    pub status: String,
    pub login_ip: Option<String>,
    pub login_date: Option<chrono::DateTime<chrono::Utc>>,
    pub remark: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dept_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dept: Option<DeptBrief>,
    pub roles: Vec<RoleBrief>,
    #[serde(flatten)]
    pub base_vo: BaseVo,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SysUserInfoVo {
    pub user: SysUserVo,
    pub role_ids: Vec<i64>,
    pub post_ids: Vec<i64>,
    pub roles: Vec<RoleBrief>,
    pub posts: Vec<PostBrief>,
}

#[derive(Clone, Debug, Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct UserChangeStatusBo {
    #[validate(required)]
    pub user_id: Option<i64>,
    #[validate(regex(path = "*STATUS_RE", message = "状态只能是0或1"))]
    pub status: String,
}

#[derive(Clone, Debug, Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct UserResetPwdBo {
    #[validate(required)]
    pub user_id: Option<i64>,
    #[validate(length(
        min = 6,
        max = 50,
        message = "密码长度至少6位且不超过50位"
    ))]
    pub password: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UserOptionVo {
    pub user_id: i64,
    pub user_name: String,
    pub nick_name: String,
}

// ────────────────────── Excel 导入导出 ──────────────────────

/// 用户导入 Excel 行(对齐 Java SysUserImportVo 列顺序)
#[derive(Clone, Debug)]
pub struct UserImportRow {
    pub user_id: Option<i64>,
    pub dept_id: Option<i64>,
    pub user_name: String,
    pub nick_name: String,
    pub email: Option<String>,
    pub phone_number: Option<String>,
    pub gender: Option<String>,
    pub status: Option<String>,
}

impl ExcelRow for UserImportRow {
    fn headers() -> Vec<&'static str> {
        vec![
            "用户序号",
            "部门名称",
            "用户账号",
            "用户昵称",
            "用户邮箱",
            "手机号码",
            "用户性别",
            "账号状态",
        ]
    }

    fn to_row(&self) -> Vec<String> {
        vec![
            self.user_id.map(|i| i.to_string()).unwrap_or_default(),
            self.dept_id.map(|i| i.to_string()).unwrap_or_default(),
            self.user_name.clone(),
            self.nick_name.clone(),
            self.email.clone().unwrap_or_default(),
            self.phone_number.clone().unwrap_or_default(),
            self.gender.clone().unwrap_or_default(),
            self.status.clone().unwrap_or_default(),
        ]
    }

    fn from_record(record: &[String]) -> AppResult<Self> {
        if record.len() < 4 {
            return Err(AppError::BadRequest(
                "导入数据列数不足,至少需要用户账号和用户昵称".to_string(),
            ));
        }
        let get = |idx: usize| -> Option<String> {
            record
                .get(idx)
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
        };

        // 用户序号 — 可选
        let user_id = get(0).and_then(|s| s.parse::<i64>().ok());
        // 部门: MVP 仅接受数字直接作为 dept_id
        let dept_id = get(1).and_then(|s| s.parse::<i64>().ok());
        let user_name = get(2).ok_or_else(|| {
            AppError::BadRequest("用户账号不能为空".to_string())
        })?;
        let nick_name = get(3).ok_or_else(|| {
            AppError::BadRequest("用户昵称不能为空".to_string())
        })?;
        let email = get(4);
        let phone_number = get(5);
        let gender = get(6);
        let status = get(7);

        Ok(Self {
            user_id,
            dept_id,
            user_name,
            nick_name,
            email,
            phone_number,
            gender,
            status,
        })
    }
}

/// 用户导出行(对齐 Vue 用户表格可见列 + Java SysUserExportVo)。
#[derive(Clone, Debug)]
pub struct UserExportRow {
    pub user_id: i64,
    pub user_name: String,
    pub nick_name: String,
    pub dept_name: String,
    pub phone_number: String,
    pub email: String,
    pub gender: String,
    pub status: String,
    pub create_time: String,
}

impl UserExportRow {
    /// 从列表 VO 构造 —— dept_name 由 handler 侧批量翻译后填好再进来。
    pub fn from_list_vo(vo: &UserListVo) -> Self {
        Self {
            user_id: vo.user_id,
            user_name: vo.user_name.clone(),
            nick_name: vo.nick_name.clone(),
            dept_name: vo.dept_name.clone().unwrap_or_default(),
            phone_number: vo.phone_number.clone().unwrap_or_default(),
            email: vo.email.clone().unwrap_or_default(),
            gender: vo.gender.clone().unwrap_or_default(),
            status: vo.status.clone(),
            create_time: vo
                .base_vo
                .create_time
                .map(|t| t.to_rfc3339())
                .unwrap_or_default(),
        }
    }
}

impl ExcelRow for UserExportRow {
    fn headers() -> Vec<&'static str> {
        vec![
            "用户编号",
            "用户账号",
            "用户昵称",
            "部门",
            "手机号码",
            "邮箱",
            "用户性别",
            "账号状态",
            "创建时间",
        ]
    }

    fn to_row(&self) -> Vec<String> {
        vec![
            self.user_id.to_string(),
            self.user_name.clone(),
            self.nick_name.clone(),
            self.dept_name.clone(),
            self.phone_number.clone(),
            self.email.clone(),
            self.gender.clone(),
            self.status.clone(),
            self.create_time.clone(),
        ]
    }

    fn from_record(_: &[String]) -> AppResult<Self> {
        Err(AppError::BadRequest("用户导出行不支持导入".to_string()))
    }
}
