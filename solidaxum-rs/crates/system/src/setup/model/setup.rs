use serde::{Deserialize, Serialize};
use validator::Validate;

/// 初始化状态探测(仅需登录;前端据此决定是否强导向导)。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SetupStatusVo {
    pub completed: bool,
}

/// 聚合完成初始化的入参。
///
/// 密码长度规则与 [`crate::user::model::user::SysUserPasswordBo`]
/// 对齐(6-50);system_name 对齐 sys.title 的 varchar(90)。
#[derive(Clone, Debug, Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct SetupCompleteBo {
    /// 系统名称 → sys_config.`sys.title`(顶栏标题,全站可见)
    #[validate(length(min = 1, max = 90))]
    pub system_name: String,
    /// 管理员当前密码(首装即种子默认 admin123;复用 profile 改密
    /// 的旧密码核对,证明会话持有者真的掌握当前凭据)
    #[validate(length(min = 6, max = 50))]
    pub admin_old_password: String,
    #[validate(length(min = 6, max = 50))]
    pub admin_new_password: String,
    /// 用户默认密码 → sys_config.`sys.user.initPassword`(Excel 批量
    /// 导入新账号的初始密码)
    #[validate(length(min = 6, max = 50))]
    pub user_init_password: String,
}
