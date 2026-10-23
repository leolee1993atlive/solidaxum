pub const SUPER_ADMIN_USER_ID: i64 = 1761100000000000001;

/// 判断给定用户 id 是否为超级管理员(对齐 Java UserConstants.isAdmin)。
pub fn is_admin_user(user_id: i64) -> bool {
    user_id == SUPER_ADMIN_USER_ID
}

pub mod config;
pub mod error;
pub mod extract;
pub mod infra;
pub mod middleware;
pub mod model;
pub mod permission;
pub mod repo;
pub mod serde;
pub mod util;
pub mod vo;
