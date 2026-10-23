use app_core::{
    middleware::extractors::CurrentUser,
    model::{
        entity::{BaseEntity, BaseVo},
        status::{Status, Visible},
    },
    util::snowflake,
};
use serde::{Deserialize, Serialize};
use validator::Validate;

pub const TYPE_DIR: &str = "M";
pub const TYPE_MENU: &str = "C";
pub const TYPE_BUTTON: &str = "F";

pub const TOP_PARENT_ID: i64 = 0;
pub const SUPER_ADMIN_USER_ID: i64 = 1761100000000000001;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Menu {
    pub menu_id: i64,
    pub menu_name: String,
    pub parent_id: i64,
    pub order_num: i32,
    pub path: String,
    pub component: Option<String>,
    pub query_param: Option<String>,
    pub is_frame: String,
    pub is_cache: String,
    pub menu_type: String,
    pub visible: String,
    pub status: String,
    pub perms: Option<String>,
    pub icon: Option<String>,
    pub active_menu: Option<String>,
    pub ext: Option<String>,
    pub remark: Option<String>,
    #[serde(flatten)]
    pub base_entity: BaseEntity,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MenuListVo {
    pub menu_id: i64,
    pub menu_name: String,
    pub parent_id: i64,
    pub order_num: i32,
    pub path: String,
    pub component: Option<String>,
    pub query_param: Option<String>,
    pub is_frame: String,
    pub is_cache: String,
    pub menu_type: String,
    pub visible: String,
    pub status: String,
    pub perms: Option<String>,
    pub icon: Option<String>,
    pub active_menu: Option<String>,
    pub ext: Option<String>,
    pub remark: Option<String>,
    #[serde(flatten)]
    pub base_vo: BaseVo,
}

impl Menu {
    pub fn is_dir(&self) -> bool {
        self.menu_type == TYPE_DIR
    }

    pub fn is_menu(&self) -> bool {
        self.menu_type == TYPE_MENU
    }

    pub fn is_button(&self) -> bool {
        self.menu_type == TYPE_BUTTON
    }

    pub fn is_visible(&self) -> bool {
        self.visible == Visible::Show.as_str()
    }

    pub fn is_normal(&self) -> bool {
        self.status == Status::Normal.as_str()
    }

    pub fn to_list_vo(&self) -> MenuListVo {
        MenuListVo {
            menu_id: self.menu_id,
            menu_name: self.menu_name.clone(),
            parent_id: self.parent_id,
            order_num: self.order_num,
            path: self.path.clone(),
            component: self.component.clone(),
            query_param: self.query_param.clone(),
            is_frame: self.is_frame.clone(),
            is_cache: self.is_cache.clone(),
            menu_type: self.menu_type.clone(),
            visible: self.visible.clone(),
            status: self.status.clone(),
            perms: self.perms.clone(),
            icon: self.icon.clone(),
            active_menu: self.active_menu.clone(),
            ext: self.ext.clone(),
            remark: self.remark.clone(),
            base_vo: BaseVo::from_base(&self.base_entity),
        }
    }

    pub fn from_create_bo(bo: MenuCreateBo, user: &CurrentUser) -> Self {
        Self {
            menu_id: snowflake::next_id(),
            menu_name: bo.menu_name,
            parent_id: bo.parent_id,
            order_num: bo.order_num,
            path: bo.path,
            component: bo.component,
            query_param: bo.query_param,
            is_frame: bo.is_frame,
            is_cache: bo.is_cache,
            menu_type: bo.menu_type,
            visible: bo.visible,
            status: bo.status,
            perms: bo.perms,
            icon: bo.icon,
            active_menu: bo.active_menu,
            ext: bo.ext,
            remark: bo.remark,
            base_entity: BaseEntity::new(user.get_user_id()),
        }
    }

    pub fn from_update_bo(&mut self, bo: MenuUpdateBo, user: &CurrentUser) {
        self.menu_name = bo.menu_name;
        self.parent_id = bo.parent_id;
        self.order_num = bo.order_num;
        self.path = bo.path;
        self.component = bo.component;
        self.query_param = bo.query_param;
        self.is_frame = bo.is_frame;
        self.is_cache = bo.is_cache;
        self.menu_type = bo.menu_type;
        self.visible = bo.visible;
        self.status = bo.status;
        self.perms = bo.perms;
        self.icon = bo.icon;
        self.active_menu = bo.active_menu;
        self.ext = bo.ext;
        self.remark = bo.remark;
        self.base_entity.update(user.get_user_id());
    }
}

#[derive(Clone, Debug, Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct MenuCreateBo {
    #[validate(length(min = 1, max = 50, message = "菜单名称不能为空"))]
    pub menu_name: String,
    #[validate(range(min = 0))]
    pub parent_id: i64,
    #[validate(range(min = 0))]
    pub order_num: i32,
    #[validate(length(min = 1, max = 200, message = "路由地址不能为空"))]
    pub path: String,
    pub component: Option<String>,
    pub query_param: Option<String>,
    #[validate(length(max = 1))]
    pub is_frame: String,
    #[validate(length(max = 1))]
    pub is_cache: String,
    #[validate(length(max = 1))]
    pub menu_type: String,
    #[validate(length(max = 1))]
    pub visible: String,
    #[validate(length(max = 1))]
    pub status: String,
    pub perms: Option<String>,
    pub icon: Option<String>,
    pub active_menu: Option<String>,
    pub ext: Option<String>,
    pub remark: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct MenuUpdateBo {
    #[validate(required)]
    pub menu_id: Option<i64>,
    #[validate(length(min = 1, max = 50, message = "菜单名称不能为空"))]
    pub menu_name: String,
    #[validate(range(min = 0))]
    pub parent_id: i64,
    #[validate(range(min = 0))]
    pub order_num: i32,
    #[validate(length(min = 1, max = 200, message = "路由地址不能为空"))]
    pub path: String,
    pub component: Option<String>,
    pub query_param: Option<String>,
    #[validate(length(max = 1))]
    pub is_frame: String,
    #[validate(length(max = 1))]
    pub is_cache: String,
    #[validate(length(max = 1))]
    pub menu_type: String,
    #[validate(length(max = 1))]
    pub visible: String,
    #[validate(length(max = 1))]
    pub status: String,
    pub perms: Option<String>,
    pub icon: Option<String>,
    pub active_menu: Option<String>,
    pub ext: Option<String>,
    pub remark: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct MenuQueryBo {
    pub menu_name: Option<String>,
    pub status: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MenuTreeSelectVo {
    pub id: i64,
    pub label: String,
    pub parent_id: i64,
    pub weight: i32,
    pub menu_type: String,
    pub visible: String,
    pub status: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<MenuTreeSelectVo>,
}

impl MenuTreeSelectVo {
    pub fn build_tree(menus: &[Menu], parent_id: i64) -> Vec<MenuTreeSelectVo> {
        let mut children: Vec<_> = menus
            .iter()
            .filter(|m| m.parent_id == parent_id)
            .map(|m| MenuTreeSelectVo {
                id: m.menu_id,
                label: m.menu_name.clone(),
                parent_id: m.parent_id,
                weight: m.order_num,
                menu_type: m.menu_type.clone(),
                visible: m.visible.clone(),
                status: m.status.clone(),
                children: Self::build_tree(menus, m.menu_id),
            })
            .collect();
        children.sort_by_key(|node| node.weight);
        children
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RoleMenuTreeSelectVo {
    pub checked_keys: Vec<i64>,
    pub menus: Vec<MenuTreeSelectVo>,
}
