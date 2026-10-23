use app_core::error::AppResult;
use async_trait::async_trait;

/// 办理人权限处理器（对应 warm-flow PermissionHandler）
///
/// 由宿主应用实现并注入 FlowState：
/// - `permissions()` 返回当前用户的权限标识集合（用户 ID、角色、部门等）
/// - `get_handler()` 返回当前用户唯一标识（通常为用户 ID）
/// - `convert_permissions()` 将 permission_flag 中的角色/部门转换为具体用户 ID 列表
#[async_trait]
pub trait PermissionHandler: Send + Sync {
    /// 当前用户的权限标识列表（用于与 flow_user.processed_by 匹配）
    async fn permissions(&self) -> AppResult<Vec<String>>;

    /// 当前用户唯一标识（写入历史记录的 approver 字段）
    async fn get_handler(&self) -> AppResult<String>;

    /// 将 permission_flag 解析出的标识（可能含角色/部门 ID）转换为用户 ID 列表
    async fn convert_permissions(
        &self,
        perms: Vec<String>,
    ) -> AppResult<Vec<String>>;
}

/// 默认空实现：直接透传，不做转换（用于测试和简单场景）
pub struct DefaultPermissionHandler;

#[async_trait]
impl PermissionHandler for DefaultPermissionHandler {
    async fn permissions(&self) -> AppResult<Vec<String>> {
        Ok(vec![])
    }

    async fn get_handler(&self) -> AppResult<String> {
        Ok(String::new())
    }

    async fn convert_permissions(
        &self,
        perms: Vec<String>,
    ) -> AppResult<Vec<String>> {
        Ok(perms)
    }
}
