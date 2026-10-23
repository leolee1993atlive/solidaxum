use app_core::{
    error::{AppError, AppResult},
    middleware::extractors::CurrentUser,
    model::{
        entity::{BaseEntity, BaseVo},
        status::DelFlag,
    },
    util::snowflake,
};
use serde::{Deserialize, Serialize};
use validator::Validate;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Role {
    pub role_id: Option<i64>,
    pub role_name: String,
    pub role_key: String,
    /// 上级角色ID(参考实现定制: 上级办理人解析)。
    pub parent_id: Option<i64>,
    pub role_sort: i32,
    pub data_scope: String,
    pub menu_check_strictly: bool,
    pub dept_check_strictly: bool,
    pub status: String,
    pub del_flag: Option<String>,
    pub remark: Option<String>,
    #[serde(flatten)]
    pub base_entity: BaseEntity,
}

impl Role {
    pub fn from_create_bo(bo: RoleCreateBo, user: &CurrentUser) -> Self {
        Self {
            role_id: Some(snowflake::next_id()),
            role_name: bo.role_name,
            role_key: bo.role_key,
            parent_id: bo.parent_id,
            role_sort: bo.role_sort.unwrap_or(0),
            data_scope: bo.data_scope.unwrap_or_else(|| "1".to_string()),
            menu_check_strictly: true,
            dept_check_strictly: true,
            status: bo.status,
            del_flag: Some(DelFlag::Alive.as_str().to_string()),
            remark: bo.remark,
            base_entity: BaseEntity::new(user.get_user_id()),
        }
    }

    #[allow(clippy::wrong_self_convention)]
    pub fn from_update_bo(&mut self, bo: RoleUpdateBo, user: &CurrentUser) {
        self.role_id = bo.role_id;
        self.role_name = bo.role_name;
        self.role_key = bo.role_key;
        self.parent_id = bo.parent_id;
        self.role_sort = bo.role_sort.unwrap_or(0);
        self.data_scope =
            bo.data_scope.unwrap_or_else(|| self.data_scope.clone());
        self.status = bo.status;
        self.remark = bo.remark;
        self.base_entity.update(user.get_user_id());
    }

    pub fn to_list_vo(&self) -> RoleListVo {
        RoleListVo {
            role_id: self.role_id.unwrap_or_default(),
            role_name: self.role_name.clone(),
            role_key: self.role_key.clone(),
            parent_id: self.parent_id,
            role_sort: self.role_sort,
            data_scope: self.data_scope.clone(),
            menu_check_strictly: self.menu_check_strictly,
            dept_check_strictly: self.dept_check_strictly,
            status: self.status.clone(),
            del_flag: self.del_flag.clone(),
            remark: self.remark.clone(),
            base_vo: BaseVo::from_base(&self.base_entity),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct RoleCreateBo {
    #[validate(length(min = 1, message = "角色名称不能为空"))]
    pub role_name: String,
    #[validate(length(min = 1, message = "角色标识不能为空"))]
    pub role_key: String,
    /// 上级角色ID(参考实现定制: 上级办理人解析)。
    pub parent_id: Option<i64>,
    pub role_sort: Option<i32>,
    pub data_scope: Option<String>,
    pub status: String,
    pub remark: Option<String>,
    /// 菜单权限(对齐 Java SysRoleBo.menuIds):insertRole → insertRoleMenu
    /// 同事务落 sys_role_menu。此前缺失该字段,serde 静默丢弃调用方的
    /// menuIds,新建角色零权限。
    pub menu_ids: Option<Vec<i64>>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct RoleUpdateBo {
    #[validate(required)]
    pub role_id: Option<i64>,
    #[validate(length(min = 1, message = "角色名称不能为空"))]
    pub role_name: String,
    #[validate(length(min = 1, message = "角色标识不能为空"))]
    pub role_key: String,
    /// 上级角色ID(参考实现定制: 上级办理人解析)。
    pub parent_id: Option<i64>,
    pub role_sort: Option<i32>,
    pub data_scope: Option<String>,
    pub status: String,
    pub remark: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RoleListVo {
    pub role_id: i64,
    pub role_name: String,
    pub role_key: String,
    /// 上级角色ID(参考实现定制: 上级办理人解析)。
    pub parent_id: Option<i64>,
    pub role_sort: i32,
    pub data_scope: String,
    pub menu_check_strictly: bool,
    pub dept_check_strictly: bool,
    pub status: String,
    pub del_flag: Option<String>,
    pub remark: Option<String>,
    #[serde(flatten)]
    pub base_vo: BaseVo,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RoleOptionVo {
    pub role_id: i64,
    pub role_name: String,
    pub role_key: String,
    pub status: String,
}

#[derive(Clone, Debug, Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct SysRolePermissionBo {
    #[validate(required)]
    pub role_id: Option<i64>,
    pub menu_ids: Option<Vec<i64>>,
    pub dept_ids: Option<Vec<i64>>,
    pub data_scope: Option<String>,
    pub menu_check_strictly: Option<bool>,
    pub dept_check_strictly: Option<bool>,
}

/// 角色导出行(对齐 Vue 角色表格可见列)。
#[derive(Clone, Debug)]
pub struct RoleExportRow {
    pub role_id: i64,
    pub role_name: String,
    pub role_key: String,
    pub role_sort: i32,
    pub status: String,
    pub create_time: String,
}

impl RoleExportRow {
    pub fn from_role(role: &Role) -> Self {
        Self {
            role_id: role.role_id.unwrap_or_default(),
            role_name: role.role_name.clone(),
            role_key: role.role_key.clone(),
            role_sort: role.role_sort,
            status: role.status.clone(),
            create_time: role
                .base_entity
                .create_time
                .map(|t| t.to_rfc3339())
                .unwrap_or_default(),
        }
    }
}

impl app_core::util::excel::ExcelRow for RoleExportRow {
    fn headers() -> Vec<&'static str> {
        vec![
            "角色编号",
            "角色名称",
            "权限字符",
            "显示顺序",
            "状态",
            "创建时间",
        ]
    }

    fn to_row(&self) -> Vec<String> {
        vec![
            self.role_id.to_string(),
            self.role_name.clone(),
            self.role_key.clone(),
            self.role_sort.to_string(),
            self.status.clone(),
            self.create_time.clone(),
        ]
    }

    fn from_record(_: &[String]) -> AppResult<Self> {
        Err(AppError::BadRequest("角色不支持导入".to_string()))
    }
}
