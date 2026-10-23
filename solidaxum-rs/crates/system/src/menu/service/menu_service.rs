//! menu 服务。
//!
//! 本层只做校验(query 框架校验之外的唯一性/外链规则);数据方法由
//! handler 直连仓储(menu 模块自始如此,与 user/role 的 service 形态不同,
//! 纯换轨不改变分层)。**ctx 线程**:全部方法首参 `ctx: &DataContext<'_>`,
//! 由 handler 请求期从 `infra.data_source()` 构造;测试等无请求身份的调用方传
//! `system_context`(ScopeFilter 恒 All,审计照常)。方法级 Java 审计表
//! 在 repo 模块头(`menu_repo.rs`)—— 全豁免,统一锚「SysMenuMapper 无
//! @DataPermission —— 权限树数据非行级」。

use app_core::{
    error::{AppError, AppResult},
    infra::Infrastructure,
};
use app_data::DataContext;

use crate::menu::{
    model::menu::{MenuCreateBo, MenuUpdateBo},
    repository::menu_repo::MenuRepository,
};

#[derive(Clone)]
pub struct MenuService {
    menu_repo: MenuRepository,
}

impl MenuService {
    /// 装配期不取任何快照:本服务全部方法只
    /// **转发** ctx,用户请求的 ctx 由 handler 请求期从 `infra.data_source()`
    /// 构造(见 app.rs 装配顺序注释)。形参仅保装配调用面形状。
    pub fn new(_infra: &Infrastructure) -> Self {
        Self {
            menu_repo: MenuRepository::new(),
        }
    }

    pub async fn validate_create(
        &self,
        ctx: &DataContext<'_>,
        bo: &MenuCreateBo,
    ) -> AppResult<()> {
        Self::validate_frame_path(
            &bo.is_frame,
            &bo.path,
            &bo.menu_name,
            false,
        )?;
        if self
            .menu_repo
            .exists_by_parent_and_name(ctx, bo.parent_id, &bo.menu_name, None)
            .await?
        {
            return Err(AppError::BadRequest(format!(
                "新增菜单'{}'失败，菜单名称已存在",
                bo.menu_name
            )));
        }
        Ok(())
    }

    pub async fn validate_update(
        &self,
        ctx: &DataContext<'_>,
        bo: &MenuUpdateBo,
    ) -> AppResult<()> {
        let menu_id = bo.menu_id.ok_or_else(|| {
            AppError::BadRequest("更新菜单失败: 菜单ID为空".to_string())
        })?;

        if menu_id == bo.parent_id {
            return Err(AppError::BadRequest(format!(
                "修改菜单'{}'失败，上级菜单不能选择自己",
                bo.menu_name
            )));
        }

        Self::validate_frame_path(&bo.is_frame, &bo.path, &bo.menu_name, true)?;

        if self
            .menu_repo
            .exists_by_parent_and_name(
                ctx,
                bo.parent_id,
                &bo.menu_name,
                Some(menu_id),
            )
            .await?
        {
            return Err(AppError::BadRequest(format!(
                "修改菜单'{}'失败，菜单名称已存在",
                bo.menu_name
            )));
        }
        Ok(())
    }

    fn validate_frame_path(
        is_frame: &str,
        path: &str,
        menu_name: &str,
        is_update: bool,
    ) -> AppResult<()> {
        if is_frame == "0"
            && !path.starts_with("http://")
            && !path.starts_with("https://")
        {
            let op = if is_update { "修改" } else { "新增" };
            return Err(AppError::BadRequest(format!(
                "{op}菜单'{menu_name}'失败，地址必须以http(s)://开头"
            )));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 回归测试: 锁定 is_frame 值域语义 —— 0=是外链, 1=非外链
    #[test]
    fn validate_frame_path_zero_means_external_link() {
        // is_frame='0' 且路径以 http(s):// 开头 → 通过
        assert!(
            MenuService::validate_frame_path(
                "0",
                "https://example.com",
                "外链菜单",
                false
            )
            .is_ok(),
            "0=外链时合法 URL 应通过"
        );

        // is_frame='0' 但路径不是 http(s):// 开头 → 报错(核心语义)
        let err = MenuService::validate_frame_path(
            "0",
            "/local/path",
            "外链菜单",
            false,
        )
        .expect_err("0=外链时非 http(s) 路径应被拒绝");
        assert!(
            matches!(err, AppError::BadRequest(_)),
            "0=外链时非 http(s) 路径应返回 BadRequest"
        );
    }

    #[test]
    fn validate_frame_path_one_allows_any_path() {
        // is_frame='1' 表示非外链, 无论路径是什么都通过
        assert!(
            MenuService::validate_frame_path(
                "1",
                "/local/path",
                "普通菜单",
                false
            )
            .is_ok(),
            "1=非外链时普通路径应通过"
        );
        assert!(
            MenuService::validate_frame_path(
                "1",
                "https://example.com",
                "普通菜单",
                false
            )
            .is_ok(),
            "1=非外链时即使给 URL 也应通过"
        );
    }
}
