use app_core::{
    middleware::extractors::CurrentUser,
    model::{
        entity::{BaseEntity, BaseVo},
        status::{DelFlag, Status},
    },
    util::snowflake,
};
use serde::{Deserialize, Serialize};
use validator::Validate;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Dept {
    pub dept_id: i64,
    pub parent_id: i64,
    pub ancestors: Option<String>,
    pub dept_name: String,
    pub dept_category: Option<String>,
    pub order_num: i32,
    pub leader: Option<i64>,
    pub phone: Option<String>,
    pub email: Option<String>,
    pub status: String,
    pub del_flag: Option<String>,
    #[serde(flatten)]
    pub base_entity: BaseEntity,
}

impl Dept {
    pub fn from_create_bo(
        bo: DeptCreateBo,
        parent: &Dept,
        user: &CurrentUser,
    ) -> Self {
        let ancestors = if parent.dept_id == 0 {
            Some("0".to_string())
        } else {
            Some(format!(
                "{},{}",
                parent.ancestors.as_deref().unwrap_or("0"),
                parent.dept_id
            ))
        };
        Self {
            dept_id: snowflake::next_id(),
            parent_id: bo.parent_id,
            ancestors,
            dept_name: bo.dept_name,
            dept_category: bo.dept_category,
            order_num: bo.order_num,
            leader: bo.leader,
            phone: bo.phone,
            email: bo.email,
            status: bo
                .status
                .unwrap_or_else(|| Status::Normal.as_str().to_string()),
            del_flag: Some(DelFlag::Alive.as_str().to_string()),
            base_entity: BaseEntity::new(user.get_user_id()),
        }
    }

    pub fn from_update_bo(&mut self, bo: DeptUpdateBo, user: &CurrentUser) {
        self.dept_name = bo.dept_name;
        self.dept_category = bo.dept_category;
        self.order_num = bo.order_num;
        self.leader = bo.leader;
        self.phone = bo.phone;
        self.email = bo.email;
        self.status = bo.status.unwrap_or_else(|| self.status.clone());
        self.base_entity.update(user.get_user_id());
    }

    pub fn to_list_vo(&self) -> DeptListVo {
        DeptListVo {
            dept_id: self.dept_id,
            parent_id: self.parent_id,
            ancestors: self.ancestors.clone(),
            dept_name: self.dept_name.clone(),
            dept_category: self.dept_category.clone(),
            order_num: self.order_num,
            leader: self.leader,
            leader_name: None,
            parent_name: None,
            phone: self.phone.clone(),
            email: self.email.clone(),
            status: self.status.clone(),
            del_flag: self.del_flag.clone(),
            base_vo: BaseVo::from_base(&self.base_entity),
            children: None,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct DeptCreateBo {
    #[validate(range(min = 0))]
    pub parent_id: i64,
    #[validate(length(min = 1, max = 30, message = "部门名称不能为空"))]
    pub dept_name: String,
    pub dept_category: Option<String>,
    #[validate(range(min = 0))]
    pub order_num: i32,
    pub leader: Option<i64>,
    #[validate(length(max = 11))]
    pub phone: Option<String>,
    #[validate(length(max = 50))]
    pub email: Option<String>,
    pub status: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct DeptUpdateBo {
    #[validate(required)]
    pub dept_id: Option<i64>,
    #[validate(range(min = 0))]
    pub parent_id: i64,
    #[validate(length(min = 1, max = 30, message = "部门名称不能为空"))]
    pub dept_name: String,
    pub dept_category: Option<String>,
    #[validate(range(min = 0))]
    pub order_num: i32,
    pub leader: Option<i64>,
    #[validate(length(max = 11))]
    pub phone: Option<String>,
    #[validate(length(max = 50))]
    pub email: Option<String>,
    pub status: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct DeptQueryBo {
    pub dept_name: Option<String>,
    pub status: Option<String>,
    pub dept_id: Option<i64>,
    pub dept_category: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeptListVo {
    pub dept_id: i64,
    pub parent_id: i64,
    pub ancestors: Option<String>,
    pub dept_name: String,
    pub dept_category: Option<String>,
    pub order_num: i32,
    pub leader: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub leader_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_name: Option<String>,
    pub phone: Option<String>,
    pub email: Option<String>,
    pub status: String,
    pub del_flag: Option<String>,
    #[serde(flatten)]
    pub base_vo: BaseVo,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub children: Option<Vec<DeptListVo>>,
}

pub const DEFAULT_DEPT_ID: i64 = 100;
pub const ROOT_PARENT_ID: i64 = 0;

/// 部门 TreeSelect 结构(对齐前端 el-tree `props: { value: 'id',
/// label: 'label', children: 'children' }`,见用户管理左侧部门树)。
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeptTreeSelectVo {
    pub id: i64,
    pub label: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<DeptTreeSelectVo>,
}

impl DeptTreeSelectVo {
    /// 从扁平部门列表构建 TreeSelect 树。
    ///
    /// 以"父节点不在集合内"的节点作为根,从而兼容按数据权限过滤后的
    /// 子树(被过滤掉父级的节点仍作为根展示,而非整树消失)。同级按
    /// `order_num` 排序。
    pub fn build(depts: &[DeptListVo]) -> Vec<DeptTreeSelectVo> {
        let ids: std::collections::HashSet<i64> =
            depts.iter().map(|d| d.dept_id).collect();
        let mut roots: Vec<&DeptListVo> = depts
            .iter()
            .filter(|d| !ids.contains(&d.parent_id))
            .collect();
        roots.sort_by_key(|d| d.order_num);
        roots.into_iter().map(|d| Self::node(depts, d)).collect()
    }

    fn node(depts: &[DeptListVo], d: &DeptListVo) -> DeptTreeSelectVo {
        let mut kids: Vec<&DeptListVo> =
            depts.iter().filter(|c| c.parent_id == d.dept_id).collect();
        kids.sort_by_key(|c| c.order_num);
        DeptTreeSelectVo {
            id: d.dept_id,
            label: d.dept_name.clone(),
            children: kids.into_iter().map(|c| Self::node(depts, c)).collect(),
        }
    }
}
