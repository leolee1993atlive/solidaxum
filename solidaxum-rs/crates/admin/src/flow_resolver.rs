//! flow ↔ system 桥接 adapter —— 查询面经 app-data 漏斗(执行面统一
//! `DataSource` + 逃生舱,错误统一 `DataError` 经 `From` 转 `AppError`)。
//!
//! # Java 锚实读裁定(plus-ui 设计器选人弹窗 → warm-flow
//! # WarmFlowController /warm-flow/handler-result → ruoyi-workflow
//! # FlwTaskAssigneeServiceImpl → ruoyi-system mapper 链,逐方法核注解)
//!
//! 锚链分两半,**注解结论相反**:
//!
//! - **弹窗列表/部门树(受辖)**:Java 侧 `SysTaskAssigneeServiceImpl`
//!   (ruoyi-workflow)的列表查询全部落到 ruoyi-system 带注解的分页方法 ——
//!   `@DataPermission` 在位,弹窗按当前登录者的数据范围过滤。
//! - **名称回显/办理人解析(不受辖)**:同一文件的 feedback/引擎解析路径
//!   全部走 `xxxMapper.lambda()` 链或 BaseMapperPlus 基方法
//!   (`selectVoById`/`selectVoList`),**零注解** —— RuoYi 的行级权限只在
//!   显式注解的方法上施加,lambda 链不受辖(这正是 SysUserMapper 要对
//!   `update`/`updateById` 显式加注解的原因)。
//!
//! | 本文件方法 | Java 锚 | @DataPermission | 漏斗路径 |
//! |---|---|---|---|
//! | `get_handlers`("user") | SysTaskAssigneeServiceImpl.java:128 → selectUsersByTaskAssigneeList(:121-131)→ SysUserServiceImpl.selectPageUserList → SysUserMapper.java:37-43 `selectPageUserList` | ✅(dept_id / create_by) | `without_row_scope()`(见下「路由偏差」) |
//! | `get_handlers`("role") | 同上 :128 → selectRolesByTaskAssigneeList(:50-59)→ SysRoleMapper.java:33-39 `selectPageRoleList` | ✅(create_dept / create_by) | 同上 |
//! | `get_handlers`("dept") | 同上 :128 → selectDeptsByTaskAssigneeList(:97-107)→ SysDeptMapper.java:50-55 `selectPageDeptList` | ✅(dept_id) | 同上 |
//! | `build_dept_tree` | FlwTaskAssigneeServiceImpl.java:141-145 fetchDeptData → SysDeptServiceImpl.java:226-231 selectDeptsByList → SysDeptMapper.java:36-41 `selectDeptList` | ✅(dept_id) | 同上 |
//! | `feedback`(user 名) | handlerFeedback(FlwTaskAssigneeServiceImpl.java:90)→ getNamesByType(:281-289)→ SysUserServiceImpl.java:766-780 selectUserNicksByIds(`userMapper.lambda()`) | ❌ | `without_row_scope()`(锚忠实) |
//! | `feedback`(role 名) | 同上 → SysRoleServiceImpl.java:609-618 selectRoleNamesByIds(`roleMapper.lambda()`) | ❌ | 同上 |
//! | `feedback`(dept 名) | 同上 → SysDeptServiceImpl.java:428-437 selectDeptNamesByIds(`deptMapper.lambda()`) | ❌ | 同上 |
//! | `user_ids_by_role_ids` | WorkflowPermissionHandler.java:57 convertPermissions → FlwTaskAssigneeServiceImpl.java:261-266 getUsersByType → SysUserServiceImpl.java:705-717 selectUsersByRoleIds(`userRoleMapper.lambda()`) | ❌ | `without_row_scope()` |
//! | `user_ids_by_dept_ids` | 同上 → SysUserServiceImpl.java:726-738 selectUsersByDeptIds(`userMapper.lambda()`) | ❌ | 同上(软删存活漏斗滤) |
//! | `user_ids_by_post_ids` | 同上 → SysUserServiceImpl.java:745-757 selectUsersByPostIds(`userPostMapper.lambda()`) | ❌ | 同上 |
//! | `user_names` | 同上 → SysUserServiceImpl.java:766-780 selectUserNicksByIds | ❌ | 同上 |
//! | LeaveStatusCallback 读/写 | leave_repo canonical 面(flow 侧已锚,见该文件头) | ❌ | `system_context` / 调用方 `&TxnContext` |
//!
//! # 路由偏差披露(受辖锚 × 公开路由)
//!
//! 上表前四行(弹窗列表 + 部门树)的 Java 锚**是受辖的**,Java 侧
//! `/warm-flow/handler-result` 在登录会话内执行(sa-token 白名单仅
//! `/warm-flow-ui/config`,application.yml:102-113)。本仓路由面已收进
//! auth(`designer_api_route`,router.rs 装配),
//! 但 `HandlerSelectService` trait 面仍无 `Principal` 可施范围 ——
//! 受辖查询在 `system_context` 下会被静默解析为全量(结构上"看似受辖、
//! 实为全量"),不如显式豁免可 grep、可审计。故本文件统一
//! `system_context()` + `without_row_scope()`;"弹窗列表按锚受辖"的
//! 补齐(Principal 穿透 `HandlerSelectService` trait)属接口面变更,
//! 尚未做。
//!
//! # ctx 构造裁定
//!
//! 全部方法用 `system_context()`:①引擎路径(user_names/
//! resolve_designators)在用户操作事务外由流程引擎驱动,trait 面无会话
//! 概念,且锚本就不受辖(principal 只影响范围,不影响软删/结果);
//! ②弹窗路径(feedback/get_handlers)如上 —— 路由虽已收 auth,
//! trait 面仍无会话可穿。纯读路径,
//! principal 唯一会影响的是行级范围,而范围按锚本就豁免(见上表)。

use std::{collections::HashMap, sync::Arc};

use app_core::error::AppResult;
use app_data::{
    DataError, DataSource,
    sea_orm::{ColumnTrait, EntityTrait, Order, QueryFilter, QueryOrder},
};
use flow::resolver::{
    BusinessStatusCallback, HandlerFeedback, HandlerRow, HandlerSelectResult,
    HandlerSelectService, InstanceAssigneeContext, UserResolver,
};
use serde_json::json;
use system::relation::{user_post, user_role};

/// 请假流程编码前缀(对齐 Java `@EventListener(condition="#flowCode.startsWith('leave')")`)。
/// 回调据此判断"这条流程是不是请假业务",非请假流程安静跳过。
const LEAVE_FLOW_PREFIX: &str = "leave";

/// 基于 system 表的用户解析器。
#[derive(Clone)]
pub struct SystemUserResolver {
    ds: Arc<DataSource>,
}

impl SystemUserResolver {
    pub fn new(ds: Arc<DataSource>) -> Self {
        Self { ds }
    }

    async fn user_ids_by_role_ids(
        &self,
        role_ids: &[i64],
    ) -> Result<Vec<i64>, DataError> {
        if role_ids.is_empty() {
            return Ok(Vec::new());
        }
        // 逃生舱只回 Model,DISTINCT 无法表达 → 取回整行后 Rust 排序去重
        // (同一 user 多 role 命中不重复;关联行量小,等价)。
        let models = self
            .ds
            .system_context()
            .select(user_role::Entity::find().filter(
                user_role::Column::RoleId.is_in(role_ids.iter().copied()),
            ))
            .without_row_scope()
            .all()
            .await?;
        let mut ids: Vec<i64> = models.into_iter().map(|m| m.user_id).collect();
        ids.sort_unstable();
        ids.dedup();
        Ok(ids)
    }

    async fn user_ids_by_dept_ids(
        &self,
        dept_ids: &[i64],
    ) -> Result<Vec<i64>, DataError> {
        if dept_ids.is_empty() {
            return Ok(Vec::new());
        }
        use system::user::repository::user_repo::{Column, Entity};
        // 软删存活过滤由漏斗接管(user_repo DataPolicy 声明);
        // user 一行只属一 dept,IN 过滤无重复,无需去重。
        let models = self
            .ds
            .system_context()
            .select(
                Entity::find()
                    .filter(Column::DeptId.is_in(dept_ids.iter().copied())),
            )
            .without_row_scope()
            .all()
            .await?;
        Ok(models.into_iter().map(|m| m.user_id).collect())
    }

    async fn user_ids_by_post_ids(
        &self,
        post_ids: &[i64],
    ) -> Result<Vec<i64>, DataError> {
        if post_ids.is_empty() {
            return Ok(Vec::new());
        }
        // 同 role 路径:DISTINCT 改为 Rust 排序去重。
        let models = self
            .ds
            .system_context()
            .select(user_post::Entity::find().filter(
                user_post::Column::PostId.is_in(post_ids.iter().copied()),
            ))
            .without_row_scope()
            .all()
            .await?;
        let mut ids: Vec<i64> = models.into_iter().map(|m| m.user_id).collect();
        ids.sort_unstable();
        ids.dedup();
        Ok(ids)
    }

    /// 科室 ∩ 角色 交集查用户(对齐 Java `deptRole:` 惰性 token 的下游
    /// selectUsersByRoleIdAndDeptIds):先按角色查关联表,再在 sys_user 里
    /// 按科室 + user_id IN 收窄。pairs 为 (dept_id, role_id)。
    async fn user_ids_by_dept_role(
        &self,
        pairs: &[(i64, i64)],
    ) -> Result<Vec<i64>, DataError> {
        use system::user::repository::user_repo::{Column, Entity};
        let mut result = Vec::new();
        for (dept_id, role_id) in pairs {
            let role_users = self.user_ids_by_role_ids(&[*role_id]).await?;
            if role_users.is_empty() {
                continue;
            }
            // 软删存活过滤由漏斗接管(user_repo DataPolicy 声明)
            let models = self
                .ds
                .system_context()
                .select(
                    Entity::find().filter(Column::DeptId.eq(*dept_id)).filter(
                        Column::UserId.is_in(role_users.iter().copied()),
                    ),
                )
                .without_row_scope()
                .all()
                .await?;
            result.extend(models.into_iter().map(|m| m.user_id));
        }
        Ok(result)
    }

    /// roleKey → roleId 翻译(deptrolekey 设计符用;查不到的 key 静默
    /// 跳过,对齐参考实现 Flow.deptRoleKeys)。
    async fn role_ids_by_keys(
        &self,
        keys: &[String],
    ) -> Result<HashMap<String, i64>, DataError> {
        use system::role::repository::role_repo::{Column, Entity};
        if keys.is_empty() {
            return Ok(HashMap::new());
        }
        let models = self
            .ds
            .system_context()
            .select(
                Entity::find()
                    .filter(Column::RoleKey.is_in(keys.iter().cloned())),
            )
            .without_row_scope()
            .all()
            .await?;
        Ok(models
            .into_iter()
            .map(|m| (m.role_key, m.role_id))
            .collect())
    }

    /// 取单个 task 的全部 flow_user 的 processed_by(解析 i64)。
    /// flow_user.type('1'审批/'2'转办/'3'委派)不区分 —— 对齐 Java
    /// 取 flw_user 全量办理人。
    /// ⚠️ 必须 include_deleted:任务完成后办理人行被引擎逻辑删
    /// (skip/persist.rs),preStep/sameStep 查的恰是已完成历史任务的
    /// 办理人 —— 走默认软删存活过滤会恒空,退回 initiator 回退。
    async fn users_of_task(&self, task_id: i64) -> Vec<i64> {
        use flow::repository::entity::flow_user::{Column, Entity};
        let rows = match self
            .ds
            .system_context()
            .select(Entity::find().filter(Column::Associated.eq(task_id)))
            .without_row_scope()
            .include_deleted()
            .all()
            .await
        {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!(task_id, error = %e, "flow_user 查询失败");
                return vec![];
            }
        };
        rows.into_iter()
            .filter_map(|u| u.processed_by.and_then(|s| s.parse().ok()))
            .collect()
    }

    /// sameStep:{nodeCode}:该实例该节点**最早**历史任务(id 升序第一条)
    /// 的 flow_user 办理人(对齐 Java FlwTaskAssigneeServiceImpl.java:
    /// 523-536)。解析不到 → warn + 空(有意差异:Java 抛
    /// ServiceException,解析面无错误通道)。
    ///
    /// 不消费在途快照:sameStep 语义是「最早」历史行,返工回路下在途行
    /// (id 最大)本就不该命中;且计数读场景无在途行,库查权威。
    async fn same_step_users(
        &self,
        instance_id: i64,
        node_code: &str,
    ) -> Vec<i64> {
        use flow::repository::entity::flow_his_task::{Column, Entity};
        let his = match self
            .ds
            .system_context()
            .select(
                Entity::find()
                    .filter(Column::InstanceId.eq(instance_id))
                    .filter(Column::NodeCode.eq(node_code))
                    .order_by(Column::Id, Order::Asc),
            )
            .without_row_scope()
            .all()
            .await
        {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!(instance_id, node_code, error = %e,
                    "sameStep 历史任务查询失败");
                return vec![];
            }
        };
        let Some(earliest) = his.first() else {
            tracing::warn!(instance_id, node_code, "sameStep 未找到历史任务");
            return vec![];
        };
        self.users_of_task(earliest.task_id).await
    }

    /// preStep/preStepApper 共用的历史任务匹配:拉实例全部 his_task,
    /// Rust 侧过滤 skip_type=PASS 且 target_node_code 匹配(含逗号则拆分
    /// 后与 [node_code] 集合相等,否则单值相等),取 **id 最大**一条。
    async fn latest_matched_his_task(
        &self,
        instance_id: i64,
        node_code: &str,
    ) -> Option<flow::repository::entity::flow_his_task::Model> {
        use flow::repository::entity::flow_his_task::{Column, Entity};
        let his = self
            .ds
            .system_context()
            .select(
                Entity::find()
                    .filter(Column::InstanceId.eq(instance_id))
                    .order_by(Column::Id, Order::Asc),
            )
            .without_row_scope()
            .all()
            .await;
        let Ok(his) = his else {
            tracing::warn!(instance_id, node_code, "preStep 历史任务查询失败");
            return None;
        };
        his.into_iter()
            .filter(|h| {
                h.skip_type == flow::enums::SkipType::Pass.as_str()
                    && match h.target_node_code.as_deref() {
                        Some(t) if !t.is_empty() => {
                            if t.contains(',') {
                                let set: std::collections::HashSet<&str> =
                                    t.split(',').collect();
                                set.len() == 1 && set.contains(node_code)
                            } else {
                                t == node_code
                            }
                        }
                        _ => false,
                    }
            })
            .max_by_key(|h| h.id)
    }

    /// 回退 initiator 变量(数字/字符串数字皆可解析,缺省返回空)。
    fn initiator_fallback(&self, ctx: &InstanceAssigneeContext) -> Vec<i64> {
        ctx.variable
            .get("initiator")
            .and_then(|v| {
                v.as_i64()
                    .or_else(|| v.as_str().and_then(|s| s.parse().ok()))
            })
            .into_iter()
            .collect()
    }

    /// preStep:{nodeCode}:id 最大且 PASS 的目标节点历史任务的 flow_user;
    /// 空则回退 variable['initiator']。
    /// **有意差异**:Java 仅在源节点为 ae_rep 时回退发起人(源节点取自
    /// 当前办理节点编码),Rust 解析面无「当前节点」之外的节点类型信息,
    /// 泛化为所有节点回退(见计划 Global Constraints 登记)。
    ///
    /// 命中键 = 在途 his 行的 target(= 本次推进目标节点)时直接取上下文
    /// 快照 —— 解析发生在 skip 事务内,该行未提交,库查恒空而误走回退(D14)。
    async fn pre_step_users(
        &self,
        ctx: &InstanceAssigneeContext,
        node_code: &str,
    ) -> Vec<i64> {
        // 在途当前步:命中键 = 在途 his 行的 target(本次推进的目标节点)。
        // 解析发生在 skip 事务内该行未提交,独立连接查不到(D14)。
        if let Some(cur) = &ctx.current
            && cur.skip_target_node_code == node_code
        {
            return cur.handlers.clone();
        }
        let instance_id = ctx.instance_id;
        let Some(his) =
            self.latest_matched_his_task(instance_id, node_code).await
        else {
            tracing::warn!(
                instance_id,
                node_code,
                "preStep 未找到匹配历史任务"
            );
            return self.initiator_fallback(ctx);
        };
        let users = self.users_of_task(his.task_id).await;
        if users.is_empty() {
            self.initiator_fallback(ctx)
        } else {
            users
        }
    }

    /// preStepApper:{nodeCode}:同 preStep 找历史任务,但只取 approver
    /// 字段(一人),不看 flow_user。在途命中键同 pre_step_users(D14)。
    async fn pre_step_approver(
        &self,
        ctx: &InstanceAssigneeContext,
        node_code: &str,
    ) -> Vec<i64> {
        if let Some(cur) = &ctx.current
            && cur.skip_target_node_code == node_code
        {
            return cur.approver.into_iter().collect();
        }
        let instance_id = ctx.instance_id;
        let Some(his) =
            self.latest_matched_his_task(instance_id, node_code).await
        else {
            tracing::warn!(
                instance_id,
                node_code,
                "preStepApper 未找到匹配历史任务"
            );
            return vec![];
        };
        his.approver
            .and_then(|s| s.parse().ok())
            .into_iter()
            .collect()
    }

    /// 上级角色用户 ∩ 同部门展开(对齐旧 SysUserServiceImpl.java:908-929,
    /// preStepParent/initiatorParent 用):输入用户 → 其角色 → 这些角色
    /// 非空的 parent_id(sys_role.del_flag='0' 由漏斗接管)→ 持 parent
    /// 角色且与输入用户同 dept(sys_user.del_flag='0' 由漏斗接管)的用户集。
    async fn parent_users_of(&self, user_ids: &[i64]) -> Vec<i64> {
        if user_ids.is_empty() {
            return vec![];
        }
        // 输入用户的角色
        let rels = match self
            .ds
            .system_context()
            .select(user_role::Entity::find().filter(
                user_role::Column::UserId.is_in(user_ids.iter().copied()),
            ))
            .without_row_scope()
            .all()
            .await
        {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!(error = %e, "上级解析:用户角色查询失败");
                return vec![];
            }
        };
        let role_ids: Vec<i64> = rels
            .iter()
            .map(|r| r.role_id)
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();
        if role_ids.is_empty() {
            return vec![];
        }
        // parent 角色 id(非空 parent_id)
        use system::role::repository::role_repo::{
            Column as RoleColumn, Entity as RoleEntity,
        };
        let roles = match self
            .ds
            .system_context()
            .select(
                RoleEntity::find()
                    .filter(RoleColumn::RoleId.is_in(role_ids.iter().copied())),
            )
            .without_row_scope()
            .all()
            .await
        {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!(error = %e, "上级解析:角色查询失败");
                return vec![];
            }
        };
        let parent_ids: Vec<i64> = roles
            .into_iter()
            .filter_map(|r| r.parent_id)
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();
        if parent_ids.is_empty() {
            return vec![];
        }
        // 持 parent 角色的候选用户
        let parent_rels = match self
            .ds
            .system_context()
            .select(user_role::Entity::find().filter(
                user_role::Column::RoleId.is_in(parent_ids.iter().copied()),
            ))
            .without_row_scope()
            .all()
            .await
        {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!(error = %e, "上级解析:parent 角色用户查询失败");
                return vec![];
            }
        };
        let candidates: Vec<i64> = parent_rels
            .iter()
            .map(|r| r.user_id)
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();
        if candidates.is_empty() {
            return vec![];
        }
        // 输入用户的部门集合 + 候选 ∩ 同部门
        use system::user::repository::user_repo::{
            Column as UserColumn, Entity as UserEntity,
        };
        let inputs = match self
            .ds
            .system_context()
            .select(
                UserEntity::find()
                    .filter(UserColumn::UserId.is_in(user_ids.iter().copied())),
            )
            .without_row_scope()
            .all()
            .await
        {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!(error = %e, "上级解析:用户查询失败");
                return vec![];
            }
        };
        let dept_ids: Vec<i64> = inputs
            .iter()
            .filter_map(|u| u.dept_id)
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();
        if dept_ids.is_empty() {
            return vec![];
        }
        let rows = match self
            .ds
            .system_context()
            .select(
                UserEntity::find()
                    .filter(
                        UserColumn::UserId.is_in(candidates.iter().copied()),
                    )
                    .filter(UserColumn::DeptId.is_in(dept_ids.iter().copied())),
            )
            .without_row_scope()
            .all()
            .await
        {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!(error = %e, "上级解析:同部门用户查询失败");
                return vec![];
            }
        };
        let mut out: Vec<i64> = rows.into_iter().map(|u| u.user_id).collect();
        out.sort_unstable();
        out.dedup();
        out
    }
}

#[async_trait::async_trait]
impl UserResolver for SystemUserResolver {
    async fn user_names(&self, ids: &[i64]) -> HashMap<i64, String> {
        if ids.is_empty() {
            return HashMap::new();
        }
        use system::user::repository::user_repo::{Column, Entity};
        let models = match self
            .ds
            .system_context()
            .select(
                Entity::find()
                    .filter(Column::UserId.is_in(ids.iter().copied())),
            )
            .without_row_scope()
            .all()
            .await
        {
            Ok(m) => m,
            Err(_) => return HashMap::new(),
        };
        models
            .into_iter()
            .map(|m| (m.user_id, m.nick_name))
            .collect()
    }

    async fn resolve_designators(&self, designators: &[String]) -> Vec<i64> {
        let mut role_ids = Vec::new();
        let mut dept_ids = Vec::new();
        let mut post_ids = Vec::new();
        let mut user_ids = Vec::new();
        let mut dept_role_pairs: Vec<(i64, i64)> = Vec::new();
        let mut dept_role_key_pairs: Vec<(i64, String)> = Vec::new();

        for d in designators {
            if let Some(rest) = d.strip_prefix("deptrolekey:") {
                // deptrolekey:{deptId}:{roleKey} —— key 翻译 id 后交集
                if let Some((dept, key)) = rest.split_once(':')
                    && let Ok(dept_id) = dept.parse::<i64>()
                {
                    dept_role_key_pairs.push((dept_id, key.to_string()));
                }
            } else if let Some(rest) = d.strip_prefix("deptrole:") {
                // deptrole:{deptId}:{roleId} —— 科室 ∩ 角色 交集
                if let Some((dept, role)) = rest.split_once(':')
                    && let (Ok(dept_id), Ok(role_id)) =
                        (dept.parse::<i64>(), role.parse::<i64>())
                {
                    dept_role_pairs.push((dept_id, role_id));
                }
            } else if let Some(rest) = d.strip_prefix("role:") {
                if let Ok(id) = rest.parse::<i64>() {
                    role_ids.push(id);
                }
            } else if let Some(rest) = d.strip_prefix("dept:") {
                if let Ok(id) = rest.parse::<i64>() {
                    dept_ids.push(id);
                }
            } else if let Some(rest) = d.strip_prefix("post:") {
                if let Ok(id) = rest.parse::<i64>() {
                    post_ids.push(id);
                }
            } else if let Some(rest) = d.strip_prefix("user:")
                && let Ok(id) = rest.parse::<i64>()
            {
                user_ids.push(id);
            }
        }

        let mut result: Vec<i64> = Vec::new();
        if !role_ids.is_empty()
            && let Ok(ids) = self.user_ids_by_role_ids(&role_ids).await
        {
            result.extend(ids);
        }
        if !dept_ids.is_empty()
            && let Ok(ids) = self.user_ids_by_dept_ids(&dept_ids).await
        {
            result.extend(ids);
        }
        if !post_ids.is_empty()
            && let Ok(ids) = self.user_ids_by_post_ids(&post_ids).await
        {
            result.extend(ids);
        }
        if !dept_role_pairs.is_empty()
            && let Ok(ids) = self.user_ids_by_dept_role(&dept_role_pairs).await
        {
            result.extend(ids);
        }
        if !dept_role_key_pairs.is_empty() {
            // roleKey → roleId 翻译后复用交集管道;查不到的 key 静默跳过
            let keys: Vec<String> =
                dept_role_key_pairs.iter().map(|(_, k)| k.clone()).collect();
            if let Ok(key_to_id) = self.role_ids_by_keys(&keys).await {
                let pairs: Vec<(i64, i64)> = dept_role_key_pairs
                    .iter()
                    .filter_map(|(dept, key)| {
                        key_to_id.get(key).map(|id| (*dept, *id))
                    })
                    .collect();
                if let Ok(ids) = self.user_ids_by_dept_role(&pairs).await {
                    result.extend(ids);
                }
            }
        }
        result.extend(user_ids);

        result.sort_unstable();
        result.dedup();
        result
    }

    /// 实例级设计符解析(sameStep:/preStep:/preStepApper:/preStepParent:/
    /// initiator/initiatorParent),按前缀分派;语义锚 Java
    /// FlwTaskAssigneeServiceImpl.java:523-583(逐行核实)。
    async fn resolve_instance_designators(
        &self,
        designators: &[String],
        ctx: &InstanceAssigneeContext,
    ) -> Vec<i64> {
        let mut out = Vec::new();
        for d in designators {
            let mut ids = if d == "initiator" {
                ctx.variable
                    .get("initiator")
                    .and_then(|v| {
                        v.as_i64()
                            .or_else(|| v.as_str().and_then(|s| s.parse().ok()))
                    })
                    .into_iter()
                    .collect()
            } else if d == "initiatorParent" {
                match ctx.variable.get("initiator").and_then(|v| v.as_i64()) {
                    Some(uid) => self.parent_users_of(&[uid]).await,
                    None => vec![],
                }
            } else if let Some(code) = d.strip_prefix("sameStep:") {
                self.same_step_users(ctx.instance_id, code).await
            } else if let Some(code) = d.strip_prefix("preStepApper:") {
                self.pre_step_approver(ctx, code).await
            } else if let Some(code) = d.strip_prefix("preStepParent:") {
                let users = self.pre_step_users(ctx, code).await;
                self.parent_users_of(&users).await
            } else if let Some(code) = d.strip_prefix("preStep:") {
                self.pre_step_users(ctx, code).await
            } else {
                vec![]
            };
            out.append(&mut ids);
        }
        out.sort_unstable();
        out.dedup();
        out
    }

    /// hasRole 预热快照的数据源:sys_user_role 按 user_id IN 现查分组
    /// (锚不受辖:userRoleMapper.lambda() 链,同 user_ids_by_role_ids)。
    async fn user_role_ids(&self, user_ids: &[i64]) -> HashMap<i64, Vec<i64>> {
        if user_ids.is_empty() {
            return HashMap::new();
        }
        let models = match self
            .ds
            .system_context()
            .select(user_role::Entity::find().filter(
                user_role::Column::UserId.is_in(user_ids.iter().copied()),
            ))
            .without_row_scope()
            .all()
            .await
        {
            Ok(m) => m,
            Err(_) => return HashMap::new(),
        };
        let mut map: HashMap<i64, Vec<i64>> = HashMap::new();
        for m in models {
            map.entry(m.user_id).or_default().push(m.role_id);
        }
        map
    }

    /// hasRoleKeys 预热快照的数据源:两跳翻译
    /// (sys_user_role 取 role_id → sys_role 取 role_key)。
    async fn user_role_keys(
        &self,
        user_ids: &[i64],
    ) -> HashMap<i64, Vec<String>> {
        let id_map = self.user_role_ids(user_ids).await;
        let all_ids: Vec<i64> = id_map.values().flatten().copied().collect();
        if all_ids.is_empty() {
            return id_map.into_keys().map(|u| (u, Vec::new())).collect();
        }
        use system::role::repository::role_repo::{Column, Entity};
        let key_of: HashMap<i64, String> = match self
            .ds
            .system_context()
            .select(
                Entity::find()
                    .filter(Column::RoleId.is_in(all_ids.iter().copied())),
            )
            .without_row_scope()
            .all()
            .await
        {
            Ok(models) => models
                .into_iter()
                .map(|m| (m.role_id, m.role_key))
                .collect(),
            Err(_) => return HashMap::new(),
        };
        id_map
            .into_iter()
            .map(|(u, ids)| {
                (
                    u,
                    ids.iter()
                        .filter_map(|id| key_of.get(id).cloned())
                        .collect(),
                )
            })
            .collect()
    }
}

/// 请假单流程状态回写回调。
///
/// 持 `Arc<DataSource>`(连接路径经 `system_context()`);事务路径由回调
/// 入参 `&TxnContext<'_>` 驱动(`on_finished`/`on_started` 收 canonical
/// 形态,事务句柄唯一(`&TxnContext`),无会话概念)。
#[derive(Clone)]
pub struct LeaveStatusCallback {
    ds: Arc<DataSource>,
}

impl LeaveStatusCallback {
    pub fn new(ds: Arc<DataSource>) -> Self {
        Self { ds }
    }

    /// 在调用方事务内回写请假单状态(canonical 形态,TxnContext 切割面)。
    /// 路由约定:businessId 即请假记录主键。
    /// 非数字 businessId(非请假业务)或记录不存在 → 安静跳过(Ok),避免把
    /// 无关流程的操作整笔回滚。真正的写库错误会向上传播以触发回滚。
    async fn write_status(
        &self,
        txn: &app_data::TxnContext<'_>,
        business_id: &str,
        status: &str,
    ) -> AppResult<()> {
        let Ok(id) = business_id.parse::<i64>() else {
            return Ok(());
        };
        let Some(mut model) =
            flow::repository::leave_repo::select_by_id_in_txn(txn, id).await?
        else {
            return Ok(());
        };
        model.status = Some(status.to_string());
        model.update_time = Some(chrono::Local::now().naive_local());
        flow::repository::leave_repo::update_by_id_in_txn(txn, &model).await?;
        Ok(())
    }
}

#[async_trait::async_trait]
impl BusinessStatusCallback for LeaveStatusCallback {
    async fn on_finished(
        &self,
        txn: &app_data::TxnContext<'_>,
        flow_code: &str,
        business_id: &str,
        status: &str,
    ) -> AppResult<()> {
        // flowCode 守卫:非请假流程不处理(对齐 Java flowCode 路由)。
        if !flow_code.starts_with(LEAVE_FLOW_PREFIX) {
            return Ok(());
        }
        self.write_status(txn, business_id, status).await
    }

    async fn on_started(
        &self,
        txn: &app_data::TxnContext<'_>,
        flow_code: &str,
        business_id: &str,
    ) -> AppResult<()> {
        if !flow_code.starts_with(LEAVE_FLOW_PREFIX) {
            return Ok(());
        }
        // 提交申请 → 待审核(对齐 RuoYi BusinessStatusEnum.WAITING)。
        self.write_status(txn, business_id, "waiting").await
    }

    async fn get_business_info(
        &self,
        business_id: &str,
    ) -> AppResult<Option<flow::resolver::BusinessInfo>> {
        let id: i64 = match business_id.parse() {
            Ok(id) => id,
            Err(_) => return Ok(None),
        };
        let Some(model) = flow::repository::leave_repo::select_by_id(
            &self.ds.system_context(),
            id,
        )
        .await?
        else {
            return Ok(None);
        };
        let days = model.leave_days.unwrap_or(0);
        let title = format!("{}请假{}天", model.leave_type, days);
        Ok(Some(flow::resolver::BusinessInfo {
            code: model.apply_code.clone(),
            title: Some(title),
        }))
    }
}

// ─── 办理人候选列表 ────────────────────────────────────────────────────────────

pub struct SystemHandlerSelectService {
    ds: Arc<DataSource>,
}

impl SystemHandlerSelectService {
    pub fn new(ds: Arc<DataSource>) -> Self {
        Self { ds }
    }
}

#[async_trait::async_trait]
impl HandlerSelectService for SystemHandlerSelectService {
    async fn get_handlers(
        &self,
        handler_type: &str,
        status: Option<&str>,
        page_num: u64,
        page_size: u64,
    ) -> AppResult<HandlerSelectResult> {
        // 分页经逃生舱 `page`(ORDER BY 进 Select 构件);page_num 从 1 起、
        // size 经 Paging 钳 1..=200。
        match handler_type {
            "role" => self.get_roles(status, page_num, page_size).await,
            "dept" => self.get_depts(status, page_num, page_size).await,
            _ => self.get_users(status, page_num, page_size).await,
        }
    }

    async fn feedback(
        &self,
        storage_ids: &[String],
    ) -> AppResult<Vec<HandlerFeedback>> {
        if storage_ids.is_empty() {
            return Ok(Vec::new());
        }

        // 按类型分组解析 id（user 无前缀或 user: 前缀；role:/dept: 带前缀）
        let mut user_ids = Vec::new();
        let mut role_ids = Vec::new();
        let mut dept_ids = Vec::new();
        for sid in storage_ids {
            if let Some(rest) = sid.strip_prefix("role:") {
                if let Ok(id) = rest.parse::<i64>() {
                    role_ids.push(id);
                }
            } else if let Some(rest) = sid.strip_prefix("dept:") {
                if let Ok(id) = rest.parse::<i64>() {
                    dept_ids.push(id);
                }
            } else {
                let bare = sid.strip_prefix("user:").unwrap_or(sid);
                if let Ok(id) = bare.parse::<i64>() {
                    user_ids.push(id);
                }
            }
        }

        // 批量查询名称映射(锚不受辖:lambda 链,见文件头审计表;
        // 查询失败静默降级为名称缺失)
        let mut names: HashMap<String, String> = HashMap::new();
        if !user_ids.is_empty() {
            use system::user::repository::user_repo::{Column, Entity};
            if let Ok(rows) = self
                .ds
                .system_context()
                .select(
                    Entity::find()
                        .filter(Column::UserId.is_in(user_ids.iter().copied())),
                )
                .without_row_scope()
                .all()
                .await
            {
                for r in rows {
                    names.insert(format!("user:{}", r.user_id), r.nick_name);
                }
            }
        }
        if !role_ids.is_empty() {
            use system::role::repository::role_repo::{Column, Entity};
            if let Ok(rows) = self
                .ds
                .system_context()
                .select(
                    Entity::find()
                        .filter(Column::RoleId.is_in(role_ids.iter().copied())),
                )
                .without_row_scope()
                .all()
                .await
            {
                for r in rows {
                    names.insert(format!("role:{}", r.role_id), r.role_name);
                }
            }
        }
        if !dept_ids.is_empty() {
            use system::dept::repository::dept_repo::{Column, Entity};
            if let Ok(rows) = self
                .ds
                .system_context()
                .select(
                    Entity::find()
                        .filter(Column::DeptId.is_in(dept_ids.iter().copied())),
                )
                .without_row_scope()
                .all()
                .await
            {
                for r in rows {
                    names.insert(format!("dept:{}", r.dept_id), r.dept_name);
                }
            }
        }

        // 按入参顺序回显，名称缺失则回退为空串（与 Java 一致）
        Ok(storage_ids
            .iter()
            .map(|sid| {
                let key = if sid.contains(':') {
                    sid.clone()
                } else {
                    format!("user:{sid}")
                };
                HandlerFeedback {
                    storage_id: sid.clone(),
                    handler_name: names.get(&key).cloned().unwrap_or_default(),
                }
            })
            .collect())
    }
}

impl SystemHandlerSelectService {
    async fn get_users(
        &self,
        status: Option<&str>,
        page_num: u64,
        page_size: u64,
    ) -> AppResult<HandlerSelectResult> {
        use system::user::repository::user_repo::{Column, Entity};
        let mut q = Entity::find().order_by(Column::CreateTime, Order::Desc);
        if let Some(s) = status {
            q = q.filter(Column::Status.eq(s));
        }
        let page = self
            .ds
            .system_context()
            .select(q)
            .without_row_scope()
            .page(page_num as i64, page_size as i64)
            .await?;
        let rows = page
            .rows
            .into_iter()
            .map(|u| HandlerRow {
                storage_id: format!("user:{}", u.user_id),
                handler_code: u.user_name,
                handler_name: u.nick_name,
                group_name: None,
                create_time: u
                    .create_time
                    .map(|t| t.format("%Y-%m-%d %H:%M:%S").to_string()),
            })
            .collect();
        // 左侧部门树
        let tree_selections = self.build_dept_tree().await;
        Ok(HandlerSelectResult {
            rows,
            total: page.total.max(0) as u64,
            tree_selections,
        })
    }

    async fn get_roles(
        &self,
        status: Option<&str>,
        page_num: u64,
        page_size: u64,
    ) -> AppResult<HandlerSelectResult> {
        use system::role::repository::role_repo::{Column, Entity};
        let mut q = Entity::find().order_by(Column::RoleSort, Order::Asc);
        if let Some(s) = status {
            q = q.filter(Column::Status.eq(s));
        }
        let page = self
            .ds
            .system_context()
            .select(q)
            .without_row_scope()
            .page(page_num as i64, page_size as i64)
            .await?;
        let rows = page
            .rows
            .into_iter()
            .map(|r| HandlerRow {
                storage_id: format!("role:{}", r.role_id),
                handler_code: r.role_key,
                handler_name: r.role_name,
                group_name: None,
                create_time: r
                    .create_time
                    .map(|t| t.format("%Y-%m-%d %H:%M:%S").to_string()),
            })
            .collect();
        Ok(HandlerSelectResult {
            rows,
            total: page.total.max(0) as u64,
            tree_selections: vec![],
        })
    }

    async fn get_depts(
        &self,
        status: Option<&str>,
        page_num: u64,
        page_size: u64,
    ) -> AppResult<HandlerSelectResult> {
        use system::dept::repository::dept_repo::{Column, Entity};
        let mut q = Entity::find().order_by(Column::OrderNum, Order::Asc);
        if let Some(s) = status {
            q = q.filter(Column::Status.eq(s));
        }
        let page = self
            .ds
            .system_context()
            .select(q)
            .without_row_scope()
            .page(page_num as i64, page_size as i64)
            .await?;
        let rows = page
            .rows
            .into_iter()
            .map(|d| HandlerRow {
                storage_id: format!("dept:{}", d.dept_id),
                handler_code: d.dept_id.to_string(),
                handler_name: d.dept_name,
                group_name: None,
                create_time: d
                    .create_time
                    .map(|t| t.format("%Y-%m-%d %H:%M:%S").to_string()),
            })
            .collect();
        Ok(HandlerSelectResult {
            rows,
            total: page.total.max(0) as u64,
            tree_selections: vec![],
        })
    }

    /// 构建设计器左侧部门树（el-tree 需要嵌套 `children`，
    /// 节点字段 id / name / children 与前端 props 对齐）。
    async fn build_dept_tree(&self) -> Vec<serde_json::Value> {
        use std::collections::HashMap;

        use system::dept::repository::dept_repo::{Column, Entity};
        let Ok(depts) = self
            .ds
            .system_context()
            .select(
                Entity::find()
                    .filter(Column::Status.eq("0"))
                    .order_by(Column::OrderNum, Order::Asc),
            )
            .without_row_scope()
            .all()
            .await
        else {
            return vec![];
        };

        // parent_id -> 子部门列表（保持 order_num 排序）
        let mut children_map: HashMap<i64, Vec<(i64, String)>> = HashMap::new();
        let mut all_ids = std::collections::HashSet::new();
        for d in &depts {
            all_ids.insert(d.dept_id);
            children_map
                .entry(d.parent_id)
                .or_default()
                .push((d.dept_id, d.dept_name.clone()));
        }

        fn build_node(
            id: i64,
            name: &str,
            children_map: &HashMap<i64, Vec<(i64, String)>>,
        ) -> serde_json::Value {
            let children: Vec<serde_json::Value> = children_map
                .get(&id)
                .map(|kids| {
                    kids.iter()
                        .map(|(cid, cname)| {
                            build_node(*cid, cname, children_map)
                        })
                        .collect()
                })
                .unwrap_or_default();
            json!({
                "id": id.to_string(),
                "name": name,
                "children": children,
            })
        }

        // 根节点：parent_id 不在结果集中的部门（通常为顶级部门）
        depts
            .iter()
            .filter(|d| !all_ids.contains(&d.parent_id))
            .map(|d| build_node(d.dept_id, &d.dept_name, &children_map))
            .collect()
    }
}
