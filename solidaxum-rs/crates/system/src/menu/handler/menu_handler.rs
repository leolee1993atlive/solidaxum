use std::sync::Arc;

use app_core::{
    error::AppError,
    extract::AppJson,
    middleware::extractors::CurrentUser,
    model::{login_user::LoginUser, status::Visible},
    vo::response::R,
};
use axum::extract::{Path, Query, State};
use serde::Serialize;
use validator::Validate;

fn require_super_admin(user: &LoginUser) -> Result<(), AppError> {
    if user.is_super_admin() {
        Ok(())
    } else {
        tracing::warn!(user_id = %user.user_id, "菜单管理仅限超级管理员");
        Err(AppError::Forbidden("菜单管理仅限超级管理员".to_string()))
    }
}

use crate::{
    app::AppState,
    menu::{
        model::menu::{
            Menu, MenuCreateBo, MenuListVo, MenuQueryBo, MenuTreeSelectVo,
            MenuUpdateBo, RoleMenuTreeSelectVo, TOP_PARENT_ID,
        },
        repository::menu_repo::{Entity as MenuEntity, MenuRepository},
    },
    permission::guard,
};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RouterVo {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hidden: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub redirect: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub component: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub query: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub always_show: Option<bool>,
    pub meta: MetaVo,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<RouterVo>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MetaVo {
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub no_cache: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub link: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_menu: Option<String>,
}

pub async fn get_routers(
    State(state): State<Arc<AppState>>,
    CurrentUser(login_user): CurrentUser,
) -> Result<R<Vec<RouterVo>>, AppError> {
    // permission 服务收 ctx —— 请求期构造(与 dept/
    // role/user handler 同款,`infra.data_source()` 此刻必带 resolver)。
    // 读为豁免方法(SysMenuMapper 零 @DataPermission),
    // ctx 只承担统一执行面。
    let ds = state.infra.data_source();
    let ctx = ds.context(app_data::Principal::from(&login_user));
    let menus = state
        .services
        .permission_service
        .get_menu_tree(&ctx, login_user.user_id)
        .await?;

    let routers = build_router_tree(&menus, TOP_PARENT_ID, true);

    Ok(R::ok_with_data(routers))
}

fn build_router_tree(
    all_menus: &[Menu],
    parent_id: i64,
    is_top_level: bool,
) -> Vec<RouterVo> {
    all_menus
        .iter()
        .filter(|m| m.parent_id == parent_id)
        .filter(|m| m.is_dir() || m.is_menu())
        .map(|menu| {
            let children = build_router_tree(all_menus, menu.menu_id, false);
            if is_top_level && (is_menu_frame(menu) || is_top_inner_link(menu))
            {
                let child = menu_to_router(menu, false, children);
                wrap_top_level(menu, child)
            } else {
                menu_to_router(menu, is_top_level, children)
            }
        })
        .collect()
}

fn is_menu_frame(menu: &Menu) -> bool {
    menu.parent_id == TOP_PARENT_ID && menu.is_menu() && menu.is_frame == "1"
}

fn is_top_inner_link(menu: &Menu) -> bool {
    menu.parent_id == TOP_PARENT_ID
        && (menu.path.starts_with("http://")
            || menu.path.starts_with("https://"))
        && menu.is_frame == "0"
}

fn wrap_top_level(menu: &Menu, child: RouterVo) -> RouterVo {
    let hidden = if menu.visible == Visible::Hide.as_str() {
        Some(true)
    } else {
        None
    };

    RouterVo {
        name: None,
        path: "/".to_string(),
        hidden,
        redirect: None,
        component: Some("Layout".to_string()),
        query: None,
        always_show: None,
        meta: MetaVo {
            title: menu.menu_name.clone(),
            icon: if menu.icon.as_deref() == Some("#") {
                None
            } else {
                menu.icon.clone()
            },
            no_cache: None,
            link: None,
            active_menu: None,
        },
        children: vec![child],
    }
}

fn has_component(menu: &Menu) -> bool {
    menu.component
        .as_deref()
        .map(|s| !s.is_empty())
        .unwrap_or(false)
}

fn menu_to_router(
    menu: &Menu,
    is_top_level: bool,
    children: Vec<RouterVo>,
) -> RouterVo {
    let name =
        Some(format!("{}{}", capitalize_first(&menu.path), menu.menu_id));

    let hidden = if menu.visible == Visible::Hide.as_str() {
        Some(true)
    } else {
        None
    };

    let is_inner_link =
        menu.path.starts_with("http://") || menu.path.starts_with("https://");

    let path = if is_top_level && menu.is_dir() && !is_inner_link {
        format!("/{}", menu.path)
    } else {
        menu.path.clone()
    };

    let component = if is_inner_link {
        // 外链菜单 → InnerLink(优先级最高,覆盖 Layout/ParentView)
        Some("InnerLink".to_string())
    } else if is_top_level && !has_component(menu) {
        Some("Layout".to_string())
    } else if has_component(menu) {
        menu.component.clone()
    } else if !is_top_level && menu.is_dir() {
        Some("ParentView".to_string())
    } else {
        None
    };

    let always_show = if menu.is_dir() && !children.is_empty() {
        Some(true)
    } else {
        None
    };

    let redirect = if menu.is_dir() && !children.is_empty() {
        Some("noRedirect".to_string())
    } else {
        None
    };

    let no_cache = if menu.is_cache == "1" {
        // 1=不缓存
        Some(true)
    } else {
        None
    };

    let link = if is_inner_link {
        Some(menu.path.clone())
    } else {
        None
    };

    let active_menu =
        if menu.active_menu.as_deref().unwrap_or("").starts_with('/') {
            menu.active_menu.clone()
        } else {
            None
        };

    let meta = MetaVo {
        title: menu.menu_name.clone(),
        icon: if menu.icon.as_deref() == Some("#") {
            None
        } else {
            menu.icon.clone()
        },
        no_cache,
        link,
        active_menu,
    };

    RouterVo {
        name,
        path,
        hidden,
        redirect,
        component,
        query: menu.query_param.as_ref().filter(|s| !s.is_empty()).cloned(),
        always_show,
        meta,
        children,
    }
}

fn capitalize_first(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

#[tracing::instrument(skip(state, query))]
pub async fn list_menu(
    State(state): State<Arc<AppState>>,
    CurrentUser(login_user): CurrentUser,
    Query(query): Query<MenuQueryBo>,
) -> Result<R<Vec<MenuListVo>>, AppError> {
    guard::require_permission(&login_user, "system:menu:list")?;
    require_super_admin(&login_user)?;
    // ctx 请求期构造(role/user 模块同款)—— infra.data_source()
    // 此刻必带 resolver(见 app.rs 装配顺序注释)。menu 全豁免,
    // ctx 只承担审计盖章与统一执行面。
    let ds = state.infra.data_source();
    let ctx = ds.context(app_data::Principal::from(&login_user));
    let menus = state
        .services
        .menu_repo
        .select_list(&ctx, &query.menu_name, &query.status)
        .await?;
    Ok(R::ok_with_data(
        menus.iter().map(|m| m.to_list_vo()).collect(),
    ))
}

#[tracing::instrument(skip(state), fields(menu_id = id))]
pub async fn get_menu(
    State(state): State<Arc<AppState>>,
    CurrentUser(login_user): CurrentUser,
    Path(id): Path<i64>,
) -> Result<R<MenuListVo>, AppError> {
    guard::require_permission(&login_user, "system:menu:query")?;
    require_super_admin(&login_user)?;
    let ds = state.infra.data_source();
    let ctx = ds.context(app_data::Principal::from(&login_user));
    let menu = state
        .services
        .menu_repo
        .select_by_id(&ctx, id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("菜单不存在 (id={})", id)))?;
    Ok(R::ok_with_data(menu.to_list_vo()))
}

#[tracing::instrument(skip(state))]
pub async fn treeselect(
    State(state): State<Arc<AppState>>,
    CurrentUser(login_user): CurrentUser,
) -> Result<R<Vec<MenuTreeSelectVo>>, AppError> {
    guard::require_permission(&login_user, "system:menu:query")?;
    let ds = state.infra.data_source();
    let ctx = ds.context(app_data::Principal::from(&login_user));
    let menus = state.services.menu_repo.select_all(&ctx).await?;
    let tree = MenuTreeSelectVo::build_tree(&menus, TOP_PARENT_ID);
    Ok(R::ok_with_data(tree))
}

#[tracing::instrument(skip(state), fields(role_id = id))]
pub async fn role_menu_treeselect(
    State(state): State<Arc<AppState>>,
    CurrentUser(login_user): CurrentUser,
    Path(id): Path<i64>,
) -> Result<R<RoleMenuTreeSelectVo>, AppError> {
    guard::require_permission(&login_user, "system:menu:query")?;
    let ds = state.infra.data_source();
    let ctx = ds.context(app_data::Principal::from(&login_user));
    let menus = state.services.menu_repo.select_all(&ctx).await?;
    let tree = MenuTreeSelectVo::build_tree(&menus, TOP_PARENT_ID);
    let checked_keys = state
        .services
        .menu_repo
        .select_menu_ids_by_role_id(&ctx, id)
        .await?;
    let data = RoleMenuTreeSelectVo {
        checked_keys,
        menus: tree,
    };
    Ok(R::ok_with_data(data))
}

#[tracing::instrument(
    skip(state, bo),
    fields(operator = %current_user.0.username, menu_name = ?bo.menu_name)
)]
pub async fn add_menu(
    State(state): State<Arc<AppState>>,
    current_user: CurrentUser,
    AppJson(bo): AppJson<MenuCreateBo>,
) -> Result<R<i64>, AppError> {
    guard::require_permission(&current_user.0, "system:menu:add")?;
    require_super_admin(&current_user.0)?;
    bo.validate()?;
    let ds = state.infra.data_source();
    let ctx = ds.context(app_data::Principal::from(&current_user.0));
    state
        .services
        .menu_service
        .validate_create(&ctx, &bo)
        .await?;
    // 领域层的审计值(create_*)会被漏斗按 ctx principal 覆盖 ——
    // 生产路径两者同源(handler 用同一 login_user 构造 ctx)。
    let menu = Menu::from_create_bo(bo, &current_user);
    let menu_id = state.services.menu_repo.insert(&ctx, &menu).await?;
    Ok(R::ok_with_data(menu_id))
}

#[tracing::instrument(
    skip(state, bo),
    fields(operator = %current_user.0.username, menu_id = ?bo.menu_id)
)]
pub async fn edit_menu(
    State(state): State<Arc<AppState>>,
    current_user: CurrentUser,
    AppJson(bo): AppJson<MenuUpdateBo>,
) -> Result<R<()>, AppError> {
    guard::require_permission(&current_user.0, "system:menu:edit")?;
    require_super_admin(&current_user.0)?;
    bo.validate()?;
    let ds = state.infra.data_source();
    let ctx = ds.context(app_data::Principal::from(&current_user.0));
    state
        .services
        .menu_service
        .validate_update(&ctx, &bo)
        .await?;
    let menu_id = bo.menu_id.ok_or_else(|| {
        AppError::BadRequest("更新菜单失败: 菜单ID为空".to_string())
    })?;

    let mut menu = state
        .services
        .menu_repo
        .select_by_id(&ctx, menu_id)
        .await?
        .ok_or_else(|| {
            AppError::NotFound(format!(
                "更新菜单失败: 菜单不存在 (id={})",
                menu_id
            ))
        })?;

    menu.from_update_bo(bo, &current_user);
    state.services.menu_repo.update_by_id(&ctx, &menu).await?;
    Ok(R::ok())
}

#[tracing::instrument(
    skip(state),
    fields(operator = %current_user.0.username, menu_id = id)
)]
pub async fn delete_menu(
    State(state): State<Arc<AppState>>,
    current_user: CurrentUser,
    Path(id): Path<i64>,
) -> Result<R<()>, AppError> {
    guard::require_permission(&current_user.0, "system:menu:remove")?;
    require_super_admin(&current_user.0)?;
    let ds = state.infra.data_source();
    let ctx = ds.context(app_data::Principal::from(&current_user.0));

    if state.services.menu_repo.has_child(&ctx, id).await? {
        return Err(AppError::BadRequest("存在子菜单,不允许删除".to_string()));
    }

    if state.services.menu_repo.count_role_menu(&ctx, id).await? > 0 {
        return Err(AppError::BadRequest("菜单已分配,不允许删除".to_string()));
    }

    state.services.menu_repo.delete_by_id(&ctx, id).await?;
    Ok(R::ok())
}

#[tracing::instrument(
    skip(state),
    fields(operator = %current_user.0.username, menu_ids = ids)
)]
pub async fn delete_menu_cascade(
    State(state): State<Arc<AppState>>,
    current_user: CurrentUser,
    Path(ids): Path<String>,
) -> Result<R<()>, AppError> {
    guard::require_permission(&current_user.0, "system:menu:remove")?;
    require_super_admin(&current_user.0)?;

    let mut menu_ids: Vec<i64> = Vec::new();

    for id_str in ids.split(',') {
        if id_str.is_empty() {
            continue;
        }
        let id: i64 = id_str.parse().map_err(|_| {
            AppError::BadRequest(format!(
                "删除菜单失败: 无效的菜单ID: {}",
                id_str
            ))
        })?;
        menu_ids.push(id);
    }

    let ds = state.infra.data_source();
    let ctx = ds.context(app_data::Principal::from(&current_user.0));

    // 对齐 Java 排除语义:子节点同批在删除集合内不算阻碍(单删 delete_menu
    // 仍用 has_child)。守卫是纯读(sys_menu/role_menu 均豁免实体,无锁语义
    // 变化),写在事务外与写在事务内可观察行为一致。
    if state
        .services
        .menu_repo
        .has_child_not_in(&ctx, &menu_ids)
        .await?
    {
        return Err(AppError::BadRequest("存在子菜单,不允许删除".to_string()));
    }

    for menu_id in &menu_ids {
        if state
            .services
            .menu_repo
            .count_role_menu(&ctx, *menu_id)
            .await?
            > 0
        {
            return Err(AppError::BadRequest(
                "菜单已分配,不允许删除".to_string(),
            ));
        }
    }

    // 写半边同一事务(Java deleteMenuById(Collection) ServiceImpl:328 平价;
    // 原 db().begin() 手工事务 → ctx.transaction)。owned 行先进闭包:
    // transaction 的 HRTB(FnOnce for<'c → BoxFuture<'c>)不允许捕获调用方
    // 切片的借用(role/user_service 同款)。
    let ids_owned = menu_ids;
    ctx.transaction(|tx| {
        Box::pin(async move {
            // 1. 清菜单的角色关联(role_menu 豁免实体 → delete_where)
            MenuRepository::delete_role_menus_by_menu_ids(tx, &ids_owned)
                .await?;
            // 2. 逐行物理删 sys_menu(豁免实体,WHERE=主键,0 行 → NotFound,
            //    触发整体回滚 —— 契约测试 cascade_delete_rolls_back 钉住)
            for menu_id in ids_owned {
                tx.delete::<MenuEntity>(menu_id).await?;
            }
            Ok(())
        })
    })
    .await?;
    Ok(R::ok())
}

#[cfg(test)]
mod tests {
    use app_core::model::status::Status;

    use super::*;
    use crate::menu::model::menu::Menu;

    fn mk_menu(id: i64, parent_id: i64, path: &str, mtype: &str) -> Menu {
        Menu {
            menu_id: id,
            parent_id,
            menu_name: format!("menu-{id}"),
            order_num: 0,
            path: path.to_string(),
            component: None,
            query_param: None,
            is_frame: "1".to_string(),
            is_cache: "0".to_string(),
            menu_type: mtype.to_string(),
            visible: Visible::Show.as_str().to_string(),
            status: Status::Normal.as_str().to_string(),
            perms: None,
            icon: None,
            active_menu: None,
            ext: None,
            remark: None,
            base_entity: app_core::model::entity::BaseEntity::new(0),
        }
    }

    #[test]
    fn dir_with_children_has_always_show_and_noredirect() {
        let parent = mk_menu(1, 0, "system", "M");
        let child = mk_menu(2, 1, "user", "C");
        let menus = vec![parent, child];
        let routers = build_router_tree(&menus, 0, true);
        assert_eq!(routers.len(), 1);
        let r = &routers[0];
        assert_eq!(
            r.always_show,
            Some(true),
            "目录且有子节点 → alwaysShow=true"
        );
        assert_eq!(r.redirect.as_deref(), Some("noRedirect"));
        assert_eq!(r.path, "/system");
        assert_eq!(r.component.as_deref(), Some("Layout"));
    }

    #[test]
    fn dir_without_children_no_always_show() {
        let parent = mk_menu(1, 0, "system", "M");
        let menus = vec![parent];
        let routers = build_router_tree(&menus, 0, true);
        let r = &routers[0];
        assert!(
            r.always_show.is_none(),
            "目录但无子节点 → alwaysShow 应为 None"
        );
        assert!(r.redirect.is_none(), "无子节点不应有 noRedirect");
    }

    #[test]
    fn nested_dir_uses_parent_view() {
        let top = mk_menu(1, 0, "system", "M");
        let nested = mk_menu(2, 1, "nested", "M");
        let leaf = mk_menu(3, 2, "page", "C");
        let menus = vec![top, nested, leaf];
        let routers = build_router_tree(&menus, 0, true);
        assert_eq!(routers.len(), 1);
        let nested_routers = &routers[0].children;
        assert_eq!(nested_routers.len(), 1);
        let nested_router = &nested_routers[0];
        assert_eq!(
            nested_router.component.as_deref(),
            Some("ParentView"),
            "非顶层目录无 component → ParentView"
        );
    }

    #[test]
    fn inner_link_uses_inner_link_component() {
        // 非顶层外链 → 直接 InnerLink，不包 Layout
        let parent = mk_menu(1, 0, "system", "M");
        let mut m = mk_menu(2, 1, "https://example.com", "C");
        m.is_frame = "0".to_string();
        let menus = vec![parent, m];
        let routers = build_router_tree(&menus, 0, true);
        let child = &routers[0].children[0];
        assert_eq!(
            child.component.as_deref(),
            Some("InnerLink"),
            "外链菜单 → InnerLink 组件"
        );
        assert_eq!(
            child.meta.link.as_deref(),
            Some("https://example.com"),
            "外链 meta.link 应为完整 URL"
        );
    }

    #[test]
    fn top_level_menu_frame_wrapped_in_layout() {
        // parent_id=0, menu_type=C, is_frame=1（非外链）→ Layout 父 + 单子节点
        let mut m = mk_menu(10, 0, "dashboard", "C");
        m.is_frame = "1".to_string();
        m.component = Some("system/dashboard/index".to_string());
        let routers = build_router_tree(&[m], 0, true);
        assert_eq!(routers.len(), 1);
        let parent = &routers[0];
        assert_eq!(parent.path, "/");
        assert_eq!(parent.component.as_deref(), Some("Layout"));
        assert_eq!(parent.children.len(), 1);
        assert_eq!(parent.children[0].path, "dashboard");
    }

    #[test]
    fn top_level_inner_link_wrapped_in_layout() {
        let mut m = mk_menu(11, 0, "https://ruoyi.vip", "C");
        m.is_frame = "0".to_string(); // 是外链
        let routers = build_router_tree(&[m], 0, true);
        assert_eq!(routers.len(), 1);
        let parent = &routers[0];
        assert_eq!(parent.component.as_deref(), Some("Layout"));
        assert_eq!(parent.children[0].component.as_deref(), Some("InnerLink"));
    }

    #[test]
    fn query_param_only_set_when_non_empty() {
        let parent = mk_menu(1, 0, "system", "M");
        let mut m = mk_menu(2, 1, "user", "C");
        m.query_param = Some(String::new()); // 空字符串
        let menus = vec![parent, m.clone()];
        let routers = build_router_tree(&menus, 0, true);
        let child = &routers[0].children[0];
        assert!(
            child.query.is_none(),
            "空字符串 query_param 应序列化为 None"
        );

        let mut m2 = m;
        m2.query_param = Some("id=1".to_string());
        let parent2 = mk_menu(1, 0, "system", "M");
        let routers2 = build_router_tree(&[parent2, m2], 0, true);
        assert_eq!(routers2[0].children[0].query.as_deref(), Some("id=1"));
    }

    #[test]
    fn top_level_dir_empty_component_uses_layout() {
        let mut dir = mk_menu(1, 0, "workflow", "M");
        dir.component = Some("".to_string());
        let routers = build_router_tree(&[dir], 0, true);
        assert_eq!(routers.len(), 1);
        assert_eq!(
            routers[0].component.as_deref(),
            Some("Layout"),
            "顶层目录 component 为空字符串时应回退到 Layout"
        );
    }

    #[test]
    fn nested_dir_empty_component_uses_parent_view() {
        let top = mk_menu(1, 0, "workflow", "M");
        let mut nested = mk_menu(2, 1, "processMonitor", "M");
        nested.component = Some("".to_string());
        let routers = build_router_tree(&[top, nested], 0, true);
        let nested_router = &routers[0].children[0];
        assert_eq!(
            nested_router.component.as_deref(),
            Some("ParentView"),
            "非顶层目录 component 为空字符串时应回退到 ParentView"
        );
    }

    #[test]
    fn buttons_filtered_out_of_router() {
        let dir = mk_menu(1, 0, "system", "M");
        let btn = mk_menu(2, 1, "add", "F");
        let menus = vec![dir, btn];
        let routers = build_router_tree(&menus, 0, true);
        assert_eq!(routers.len(), 1, "目录在");
        assert!(
            routers[0].children.is_empty(),
            "按钮(F)不应出现在 router 树"
        );
    }
}
