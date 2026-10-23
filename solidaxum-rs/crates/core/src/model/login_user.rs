use std::collections::HashSet;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LoginUser {
    pub user_id: i64,
    pub dept_id: Option<i64>,
    pub username: String,
    pub nick_name: Option<String>,
    pub user_type: Option<String>,
    pub client_id: String,
    pub client_key: Option<String>,
    pub device_type: Option<String>,
    pub permissions: HashSet<String>,
    pub roles: HashSet<String>,
    pub login_ip: Option<String>,
    pub login_date: Option<chrono::DateTime<chrono::Utc>>,
}

impl LoginUser {
    pub fn is_super_admin(&self) -> bool {
        self.user_id == crate::SUPER_ADMIN_USER_ID
            || self.permissions.contains("*:*:*")
            || self.roles.contains("superadmin")
    }

    pub fn has_permission(&self, perm: &str) -> bool {
        self.is_super_admin() || self.permissions.contains(perm)
    }

    pub fn has_role(&self, role: &str) -> bool {
        self.roles.contains(role)
    }
}

/// 请求身份 → 数据层身份(ds.context(...) 入参)。
/// LoginUser 的角色是 key 集合,数据范围由 ScopeResolver 按 user_id 查库解析,
/// 不随身份携带 —— 对齐 get_data_scope(user_id, dept_id) 的既有签名。
impl From<&LoginUser> for app_data::Principal {
    fn from(u: &LoginUser) -> Self {
        Self {
            user_id: u.user_id,
            dept_id: u.dept_id,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    fn normal_user() -> LoginUser {
        LoginUser {
            user_id: 301,
            dept_id: Some(101),
            username: "testuser".to_string(),
            nick_name: Some("测试".to_string()),
            user_type: Some("sys_user".to_string()),
            client_id: "client123".to_string(),
            client_key: Some("key".to_string()),
            device_type: None,
            permissions: HashSet::new(),
            roles: HashSet::new(),
            login_ip: Some("127.0.0.1".to_string()),
            login_date: None,
        }
    }

    #[test]
    fn test_is_super_admin_by_user_id() {
        let mut user = normal_user();
        user.user_id = crate::SUPER_ADMIN_USER_ID;
        assert!(user.is_super_admin());
    }

    #[test]
    fn test_is_super_admin_by_all_permission() {
        let mut user = normal_user();
        user.permissions.insert("*:*:*".to_string());
        assert!(user.is_super_admin());
    }

    #[test]
    fn test_is_super_admin_by_role() {
        let mut user = normal_user();
        user.roles.insert("superadmin".to_string());
        assert!(user.is_super_admin());
    }

    #[test]
    fn test_is_not_super_admin() {
        let user = normal_user();
        assert!(!user.is_super_admin());
    }

    #[test]
    fn test_has_permission_normal_user_hit() {
        let mut user = normal_user();
        user.permissions.insert("system:user:list".to_string());
        assert!(user.has_permission("system:user:list"));
    }

    #[test]
    fn test_has_permission_normal_user_miss() {
        let user = normal_user();
        assert!(!user.has_permission("system:user:list"));
    }

    #[test]
    fn test_has_permission_super_admin_always_true() {
        let mut user = normal_user();
        user.permissions.insert("*:*:*".to_string());
        assert!(user.has_permission("anything:at:all"));
    }

    #[test]
    fn test_has_role_hit() {
        let mut user = normal_user();
        user.roles.insert("admin".to_string());
        assert!(user.has_role("admin"));
    }

    #[test]
    fn test_has_role_miss() {
        let user = normal_user();
        assert!(!user.has_role("admin"));
    }

    #[test]
    fn test_serialization_roundtrip() {
        let mut user = normal_user();
        user.permissions.insert("system:user:list".to_string());
        user.permissions.insert("system:role:list".to_string());
        user.roles.insert("admin".to_string());

        let json = serde_json::to_string(&user).expect("序列化失败");
        let deserialized: LoginUser =
            serde_json::from_str(&json).expect("反序列化失败");

        assert_eq!(deserialized.user_id, user.user_id);
        assert_eq!(deserialized.dept_id, user.dept_id);
        assert_eq!(deserialized.username, user.username);
        assert_eq!(deserialized.nick_name, user.nick_name);
        assert_eq!(deserialized.user_type, user.user_type);
        assert_eq!(deserialized.client_id, user.client_id);
        assert_eq!(deserialized.client_key, user.client_key);
        assert_eq!(deserialized.device_type, user.device_type);
        assert_eq!(deserialized.permissions, user.permissions);
        assert_eq!(deserialized.roles, user.roles);
        assert_eq!(deserialized.login_ip, user.login_ip);
        assert_eq!(deserialized.login_date, user.login_date);
    }

    /// LoginUser 无 Default:按既有测试的构造方式写全字段字面量。
    #[test]
    fn principal_from_login_user_picks_identity_fields() {
        let u = LoginUser {
            user_id: 7,
            dept_id: Some(103),
            username: "admin".to_string(),
            nick_name: None,
            user_type: Some("sys_user".to_string()),
            client_id: "pc".to_string(),
            client_key: None,
            device_type: None,
            permissions: HashSet::new(),
            roles: HashSet::new(),
            login_ip: None,
            login_date: None,
        };
        let p = app_data::Principal::from(&u);
        assert_eq!(p.user_id, 7);
        assert_eq!(p.dept_id, Some(103));

        // 无部门挂靠(如超管)也要中转:None 原样传递
        let mut no_dept = u.clone();
        no_dept.dept_id = None;
        let p = app_data::Principal::from(&no_dept);
        assert_eq!(p.user_id, 7);
        assert_eq!(p.dept_id, None);
    }
}
