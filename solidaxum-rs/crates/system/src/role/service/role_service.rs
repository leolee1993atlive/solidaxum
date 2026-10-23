//! role 服务。
//!
//! 方法级 Java 审计表在 repo 模块头(`role_repo.rs`),本层只列 service 级
//! 裁定:
//! - **ctx 线程**:全部方法首参 `ctx: &DataContext<'_>`,由 handler 请求期
//!   从 `infra.data_source()` 构造;测试等无请求身份的调用方传 `system_context`
//!   (ScopeFilter 恒 All,软删/审计照常)。
//! - **事务边界**:`update_role_permission` 是基础字段 + role_menu +
//!   role_dept 三段复合写,Java `updateRolePermission`
//!   (ServiceImpl:337,@Transactional)→ `ctx.transaction` 闭包内经
//!   TxnContext 原语直接执行(delete_where + insert_many 批插);
//!   审计 update_* 由漏斗盖(principal 取自 ctx)。
//!   自参考实现 起,`cancel_auth_user`/`cancel_auth_users`/
//!   `select_auth_users` 三个授权变更入口也各自事务化(授权写 +
//!   flow_assignee_sync 补偿任务同事务,见各方法注释)。
//! - **invalidate 接线**:写-后-失效 ——
//!   角色级变更(update_by_bo/update_role_permission/change_status/
//!   delete_by_id)`slot_invalidate_all`;用户级变更(cancel_auth_user/
//!   cancel_auth_users/select_auth_users)`slot_invalidate(user_id)` 定点。

use app_core::{
    error::{AppError, AppResult},
    infra::Infrastructure,
    middleware::extractors::CurrentUser,
    model::status::DelFlag,
    vo::page::{TableDataInfo, build_order_by, normalize_paging},
};
use app_data::{
    DataContext,
    sea_orm::{ActiveValue::Set, ColumnTrait, Condition},
};

use crate::{
    assignee_sync::service::assignee_sync_service::AssigneeSyncService,
    permission::scope_resolver::{
        ScopeCacheSlot, empty_scope_cache_slot, slot_invalidate,
        slot_invalidate_all,
    },
    relation::{role_dept, role_menu},
    role::{
        handler::role_handler::RolePageQuery,
        model::role::{
            Role, RoleCreateBo, RoleListVo, RoleOptionVo, RoleUpdateBo,
        },
        repository::role_repo::{
            ActiveModel as RoleActiveModel, RoleRepository,
        },
    },
    token::token_service::TokenService,
    user::{handler::user_handler::UserPageQuery, model::user::UserListVo},
};

/// 内置角色 id(禁止编辑/删除)。对齐 Java `UserConstants.SUPER_ADMIN_ROLE_ID = 1`。
/// 当前 seed 中 role_id=1 未启用,但保留常量供未来扩展(如 admin 内置角色)。
const SUPER_ADMIN_ROLE_ID: i64 = 1;

#[derive(Clone)]
pub struct RoleService {
    role_repo: RoleRepository,
    token_service: Option<TokenService>,
    /// 数据范围缓存失效槽:角色写路径经它主动失效 resolver 缓存。
    /// 默认空槽(失效 no-op),生产由 ServiceContainer::new 注入共享槽。
    scope_cache: ScopeCacheSlot,
}

impl RoleService {
    /// 装配期不取任何快照:本服务全部方法只
    /// **转发** ctx,用户请求的 ctx 由 handler 请求期从 `infra.data_source()`
    /// 构造(见 app.rs 装配顺序注释)。
    pub fn new(infra: &Infrastructure) -> Self {
        Self {
            role_repo: RoleRepository::new(),
            token_service: Some(TokenService::new(infra)),
            scope_cache: empty_scope_cache_slot(),
        }
    }

    /// 测试用构造器: 不接 Redis(只做 DB 层断言)。仓储无状态,不消费连接。
    pub fn for_test() -> Self {
        Self {
            role_repo: RoleRepository::new(),
            token_service: None,
            scope_cache: empty_scope_cache_slot(),
        }
    }

    /// 装配/测试用:接上共享失效槽(链式)。生产由 ServiceContainer::new
    /// 统一注入;接上后角色写路径的失效调用立即生效。
    pub fn with_scope_cache(mut self, slot: ScopeCacheSlot) -> Self {
        self.scope_cache = slot;
        self
    }

    /// 角色变更后清理受影响会话;若 token_service 不存在则跳过(测试模式)。
    async fn invalidate_sessions(&self, role_key: &str) {
        if let Some(ref ts) = self.token_service
            && let Err(e) = ts.clean_sessions_by_role(role_key).await
        {
            tracing::warn!(
                role_key = %role_key,
                error = %e,
                "清理角色相关会话失败(忽略,继续执行)"
            );
        }
    }

    /// 越权保护: 禁止对超级管理员角色做 edit/delete。
    /// 对齐 Java `SysRoleServiceImpl.checkRoleAllowed`。
    fn check_role_allowed(role_id: i64) -> AppResult<()> {
        if role_id == SUPER_ADMIN_ROLE_ID {
            return Err(AppError::BadRequest(
                "不允许操作超级管理员角色".to_string(),
            ));
        }
        Ok(())
    }

    /// 越权保护:对齐 Java `SysRoleServiceImpl.checkRoleDataScope`
    /// (:260-281)—— roleId 空 → 放行;超管 → 放行(生产由
    /// CachedScopeResolver 解析为 All,探针经漏斗自然通过,无特判);
    /// 其余走带注解的 `selectRoleCount` 探针(Mapper.java:61-66)。
    /// count==0(不存在**或**越权)→ NotFound,不泄露存在性。
    ///
    /// 写路径无行级范围,role 的越权拦截**只能**靠它 ——
    /// Java 对位在 SysRoleController 的 getInfo:83 / edit:120 /
    /// editPermission:146 / changeStatus:166 四处。
    pub async fn check_role_data_scope(
        &self,
        ctx: &DataContext<'_>,
        role_id: i64,
    ) -> AppResult<()> {
        if self.role_repo.count_role_by_id(ctx, role_id).await? == 0 {
            return Err(AppError::NotFound(format!(
                "没有权限访问角色数据 (role_id={})",
                role_id
            )));
        }
        Ok(())
    }

    /// 唯一性校验: roleKey + roleName 都不能与他人冲突。
    /// `exclude_id`: 更新时传当前角色 id 以排除自身。
    /// (豁免读:唯一性须跨部门全量,见 repo 审计表。)
    async fn ensure_unique(
        &self,
        ctx: &DataContext<'_>,
        role_key: &str,
        role_name: &str,
        exclude_id: Option<i64>,
    ) -> AppResult<()> {
        if self
            .role_repo
            .select_by_role_key(ctx, role_key)
            .await?
            .filter(|r| r.role_id != exclude_id)
            .is_some()
        {
            return Err(AppError::Conflict(format!(
                "角色标识已存在: '{}'",
                role_key
            )));
        }
        if self
            .role_repo
            .select_by_role_name(ctx, role_name, exclude_id)
            .await?
            .is_some()
        {
            return Err(AppError::Conflict(format!(
                "角色名称已存在: '{}'",
                role_name
            )));
        }
        Ok(())
    }

    #[tracing::instrument(skip(self, ctx))]
    pub async fn list(
        &self,
        ctx: &DataContext<'_>,
    ) -> AppResult<Vec<RoleListVo>> {
        // 不分页列表:对齐 Java selectRoleAll(空 bo,无 status 条件)
        let result = self.role_repo.select_all(ctx, &None).await?;
        Ok(result.into_iter().map(|r| r.to_list_vo()).collect())
    }

    #[tracing::instrument(skip(self, ctx))]
    pub async fn get_roles_by_user_id(
        &self,
        ctx: &DataContext<'_>,
        user_id: i64,
    ) -> AppResult<Vec<RoleListVo>> {
        let roles =
            self.role_repo.select_roles_by_user_id(ctx, user_id).await?;
        Ok(roles.into_iter().map(|r| r.to_list_vo()).collect())
    }

    #[tracing::instrument(skip(self, ctx))]
    pub async fn change_status(
        &self,
        ctx: &DataContext<'_>,
        role_id: i64,
        status: String,
    ) -> AppResult<()> {
        // 取角色 key 用于会话失效(受辖读,越权行同 NotFound)
        let role = self.get_role_by_id(ctx, role_id).await?;
        self.role_repo.update_status(ctx, role_id, &status).await?;
        // 角色状态变更 → 清理受影响会话
        self.invalidate_sessions(&role.role_key).await;
        // 停启用改变角色是否参与范围计算(select_roles_by_user_id
        // 过滤 status='0')→ 全清数据范围缓存。诚实对照:Java changeStatus
        // 无 @CacheEvict —— 它每请求实时算范围,无缓存即无陈旧;我们引入
        // 了 user_id 键的跨请求缓存,必须自兜失效(否则停用角色的人仍按
        // 旧范围查数,最长 60s)。
        slot_invalidate_all(&self.scope_cache);
        Ok(())
    }

    #[tracing::instrument(skip(self, ctx))]
    pub async fn option_select(
        &self,
        ctx: &DataContext<'_>,
        role_ids: Option<Vec<i64>>,
    ) -> AppResult<Vec<RoleOptionVo>> {
        // 下拉只出正常状态角色(Java 平价锚
        // SysRoleServiceImpl:189 `.eq(SysRole::getStatus, NORMAL)`);
        // 不分页列表(list)无此过滤。
        let roles = self
            .role_repo
            .select_all(ctx, &Some("0".to_string()))
            .await?;
        let filter_set = role_ids.map(|ids| {
            ids.into_iter().collect::<std::collections::HashSet<_>>()
        });
        Ok(roles
            .into_iter()
            .filter(|r| {
                filter_set.as_ref().is_none_or(|set| {
                    r.role_id.is_some_and(|id| set.contains(&id))
                })
            })
            .map(|r| RoleOptionVo {
                role_id: r.role_id.unwrap_or_default(),
                role_name: r.role_name,
                role_key: r.role_key,
                status: r.status,
            })
            .collect())
    }

    #[tracing::instrument(skip(self, ctx))]
    pub async fn allocated_user_page(
        &self,
        ctx: &DataContext<'_>,
        role_id: i64,
        query: &UserPageQuery,
    ) -> AppResult<TableDataInfo<UserListVo>> {
        let (users, total) = self
            .role_repo
            .select_allocated_user_page(ctx, role_id, query)
            .await?;
        let rows = users.into_iter().map(|u| u.to_list_vo()).collect();
        Ok(TableDataInfo::new(rows, total as i64))
    }

    #[tracing::instrument(skip(self, ctx))]
    pub async fn unallocated_user_page(
        &self,
        ctx: &DataContext<'_>,
        role_id: i64,
        query: &UserPageQuery,
    ) -> AppResult<TableDataInfo<UserListVo>> {
        let (users, total) = self
            .role_repo
            .select_unallocated_user_page(ctx, role_id, query)
            .await?;
        let rows = users.into_iter().map(|u| u.to_list_vo()).collect();
        Ok(TableDataInfo::new(rows, total as i64))
    }

    #[tracing::instrument(skip(self, ctx))]
    pub async fn cancel_auth_user(
        &self,
        ctx: &DataContext<'_>,
        role_id: i64,
        user_id: i64,
    ) -> AppResult<()> {
        // 补偿行的 auth_code = role_key(可读标识;豁免读,理由见
        // select_role_key_by_id)。角色缺失不阻断变更,降级 None 并告警。
        let role_key = self.role_key_for_sync(ctx, role_id).await;
        // **行为加固(事务化)**:原实现单写无事务,现把取消授权与
        // 补偿任务(flow_assignee_sync)收进**同一事务** —— 补偿行与
        // 授权变更同生共死(参考实现 outbox 可靠性不变量)。
        // repo clone + owned 数据 move 进闭包(HRTB 不收 &self/借用切片,
        // update_role_permission 同款处理)。
        let role_repo = self.role_repo.clone();
        ctx.transaction(|tx| {
            Box::pin(async move {
                role_repo
                    .delete_user_roles_in_txn(tx, role_id, &[user_id])
                    .await?;
                AssigneeSyncService::enqueue_tx(
                    tx,
                    "role",
                    "delete",
                    role_key.as_deref(),
                    &[user_id],
                )
                .await?;
                Ok(())
            })
        })
        .await?;
        // 用户级变更(角色侧入口)→ 定点失效该用户。Java
        // SysRoleServiceImpl.authUserCancel 无 @CacheEvict(它无 user 级
        // 缓存可失效);我们按 user_id 键缓存,取消授权后该用户的已缓存
        // 范围立即作废,他人不动。
        slot_invalidate(&self.scope_cache, user_id);
        Ok(())
    }

    #[tracing::instrument(skip(self, ctx))]
    pub async fn cancel_auth_users(
        &self,
        ctx: &DataContext<'_>,
        role_id: i64,
        user_ids: &[i64],
    ) -> AppResult<()> {
        let role_key = self.role_key_for_sync(ctx, role_id).await;
        // 同 cancel_auth_user:取消授权 + 补偿任务同事务(行为加固)。
        // owned 副本 move 进闭包(HRTB 不收借用切片)。
        let role_repo = self.role_repo.clone();
        let owned_ids: Vec<i64> = user_ids.to_vec();
        ctx.transaction(|tx| {
            Box::pin(async move {
                role_repo
                    .delete_user_roles_in_txn(tx, role_id, &owned_ids)
                    .await?;
                AssigneeSyncService::enqueue_tx(
                    tx,
                    "role",
                    "delete",
                    role_key.as_deref(),
                    &owned_ids,
                )
                .await?;
                Ok(())
            })
        })
        .await?;
        // 同 cancel_auth_user,批量取消逐个定点失效(user_id 已知,
        // 不必全清)。
        for uid in user_ids {
            slot_invalidate(&self.scope_cache, *uid);
        }
        Ok(())
    }

    #[tracing::instrument(skip(self, ctx))]
    pub async fn select_auth_users(
        &self,
        ctx: &DataContext<'_>,
        role_id: i64,
        user_ids: &[i64],
    ) -> AppResult<()> {
        let role_key = self.role_key_for_sync(ctx, role_id).await;
        // **行为加固(事务化)**:原实现单写无事务,现把批量授权与
        // 补偿任务(flow_assignee_sync)收进**同一事务**(参考实现
        // outbox 可靠性不变量,理由同 cancel_auth_user)。
        // owned 副本 move 进闭包(HRTB 不收借用切片)。
        let role_repo = self.role_repo.clone();
        let owned_ids: Vec<i64> = user_ids.to_vec();
        ctx.transaction(|tx| {
            Box::pin(async move {
                role_repo
                    .insert_user_roles_in_txn(tx, role_id, &owned_ids)
                    .await?;
                AssigneeSyncService::enqueue_tx(
                    tx,
                    "role",
                    "add",
                    role_key.as_deref(),
                    &owned_ids,
                )
                .await?;
                Ok(())
            })
        })
        .await?;
        // 批量授予同 cancel_auth_users —— 定点失效被授予的用户。
        for uid in user_ids {
            slot_invalidate(&self.scope_cache, *uid);
        }
        Ok(())
    }

    /// 取 role_key 作补偿任务的 auth_code。角色缺失/读取失败不阻断
    /// 授权变更本身,降级为 None 并告警(补偿仍按 user_ids 生效)。
    async fn role_key_for_sync(
        &self,
        ctx: &DataContext<'_>,
        role_id: i64,
    ) -> Option<String> {
        match self.role_repo.select_role_key_by_id(ctx, &role_id).await {
            Ok(Some(key)) => Some(key),
            Ok(None) => {
                tracing::warn!(
                    role_id,
                    "角色授权变更: 角色不存在,补偿任务 auth_code 置空"
                );
                None
            }
            Err(e) => {
                tracing::warn!(
                    role_id,
                    error = %e,
                    "角色授权变更: role_key 读取失败,补偿任务 auth_code 置空"
                );
                None
            }
        }
    }

    #[tracing::instrument(skip(self, ctx))]
    pub async fn page(
        &self,
        ctx: &DataContext<'_>,
        query: &RolePageQuery,
    ) -> AppResult<TableDataInfo<RoleListVo>> {
        let (current, size, _offset) = normalize_paging(&query.page);
        let allowed = [
            ("roleId", "role_id"),
            ("roleName", "role_name"),
            ("roleSort", "role_sort"),
            ("createTime", "create_time"),
        ];
        let order_by = build_order_by(
            &query.page.order_by_column,
            &query.page.is_asc,
            &allowed,
        )
        .unwrap_or_else(|| "role_sort asc".to_string());

        let page_result = self
            .role_repo
            .select_page(ctx, query, current as u64, size as u64, &order_by)
            .await?;
        if page_result.total <= 0 {
            return Ok(TableDataInfo::empty());
        }
        tracing::info!(
            current,
            size,
            total = page_result.total,
            "角色分页查询成功"
        );
        Ok(page_result)
    }

    /// 按查询条件取全部角色(导出用),与 page 共用同一套 filter/order。
    #[tracing::instrument(skip(self, ctx))]
    pub async fn list_all(
        &self,
        ctx: &DataContext<'_>,
        query: RolePageQuery,
    ) -> AppResult<Vec<Role>> {
        let allowed = [
            ("roleId", "role_id"),
            ("roleName", "role_name"),
            ("roleSort", "role_sort"),
            ("createTime", "create_time"),
        ];
        let order_by = build_order_by(
            &query.page.order_by_column,
            &query.page.is_asc,
            &allowed,
        )
        .unwrap_or_else(|| "role_sort asc".to_string());

        let list = self.role_repo.select_list(ctx, &query, &order_by).await?;
        Ok(list)
    }

    #[tracing::instrument(skip(self, ctx), fields(role_id = id))]
    pub async fn get_role_by_id(
        &self,
        ctx: &DataContext<'_>,
        id: i64,
    ) -> AppResult<Role> {
        let role =
            self.role_repo
                .select_by_id(ctx, &id)
                .await?
                .ok_or_else(|| {
                    AppError::NotFound(format!("角色不存在 (id={})", id))
                })?;
        Ok(role)
    }

    /// 获取角色已分配的部门 ID 列表
    pub async fn get_role_dept_ids(
        &self,
        ctx: &DataContext<'_>,
        role_id: i64,
    ) -> AppResult<Vec<i64>> {
        Ok(self
            .role_repo
            .select_dept_ids_by_role_ids(ctx, &[role_id])
            .await?)
    }

    /// 更新角色权限 (菜单 + 部门数据权限) — 全部在同一事务中执行。
    ///
    /// 事务形状对齐 user_service 的 delete-then-insert(TxnContext 原语
    /// 直接执行,repo 方法收 DataContext 进不了闭包):基础字段经
    /// `tx.update_without_row_scope`(审计 update_* 由漏斗盖,principal
    /// 取自 ctx);
    /// role_menu/role_dept 经 `tx.delete_where`(豁免实体)+
    /// `tx.insert_many` 批插。
    ///
    /// **写路径不受行级范围**:Java 写方法无
    /// @DataPermission,越权拦截在 controller 的 checkRoleDataScope。
    /// WHERE = 主键 + 软删存活,0 行 → NotFound;审计盖 update_*。
    #[allow(clippy::too_many_arguments)]
    pub async fn update_role_permission(
        &self,
        ctx: &DataContext<'_>,
        role_id: i64,
        menu_ids: &[i64],
        dept_ids: &[i64],
        data_scope: &str,
        menu_check_strictly: bool,
        dept_check_strictly: bool,
        _user: &CurrentUser,
    ) -> AppResult<()> {
        // 受辖读:拿 role_key 做会话失效(越权行同 NotFound)
        let role = self.get_role_by_id(ctx, role_id).await?;

        // 只更新权限相关字段(Java ServiceImpl:339-340「仅更新权限配置,
        // 数据范围、树联动」);越权已由 handler 探针拦截,tx 内 WHERE 只剩
        // 主键 + 软删存活,0 行 → NotFound。
        let role_update = RoleActiveModel {
            role_id: Set(role_id),
            data_scope: Set(data_scope.to_string()),
            menu_check_strictly: Set(menu_check_strictly),
            dept_check_strictly: Set(dept_check_strictly),
            ..Default::default()
        };

        // owned 行先进闭包:transaction 的 HRTB(FnOnce for<'c → BoxFuture<'c>)
        // 不允许捕获调用方切片的借用(user_service 同款)。
        let menu_models: Vec<role_menu::ActiveModel> = menu_ids
            .iter()
            .map(|&mid| role_menu::ActiveModel {
                role_id: Set(role_id),
                menu_id: Set(mid),
            })
            .collect();
        let dept_models: Vec<role_dept::ActiveModel> = dept_ids
            .iter()
            .map(|&did| role_dept::ActiveModel {
                role_id: Set(role_id),
                dept_id: Set(did),
            })
            .collect();

        ctx.transaction(|tx| {
            Box::pin(async move {
                // 1. 更新角色基础权限字段(data_scope/树联动)
                tx.update_without_row_scope(role_update).await?;
                // 2. 替换菜单权限:先清旧,再批插(空集即清空,Java 同语义)
                tx.delete_where::<role_menu::Entity>(
                    Condition::all().add(role_menu::Column::RoleId.eq(role_id)),
                )
                .await?;
                tx.insert_many(menu_models).await?;
                // 3. 替换数据权限部门:同上
                tx.delete_where::<role_dept::Entity>(
                    Condition::all().add(role_dept::Column::RoleId.eq(role_id)),
                )
                .await?;
                tx.insert_many(dept_models).await?;
                Ok(())
            })
        })
        .await?;
        // 权限变更 → 清理受影响会话
        self.invalidate_sessions(&role.role_key).await;
        // data_scope / role_dept 写路径提交 → 全清。Java 对照:
        // SysRoleServiceImpl:334 `@CacheEvict(SYS_ROLE_CUSTOM, key="#bo.roleId")`
        // 按 role 定点失效角色自定义部门缓存;我们的缓存按 user_id 键,
        // 无法从 role_id 反查受影响用户(反向索引的成本裁定:不做),
        // 取 Java 批量删除自用的 allEntries=true 语义(:433)—— 全清,
        // 冷启动一次查库即逐条重建。
        slot_invalidate_all(&self.scope_cache);
        Ok(())
    }

    #[tracing::instrument(skip(self, ctx, bo, current_user), fields(operator = %current_user.0.username))]
    pub async fn insert_by_bo(
        &self,
        ctx: &DataContext<'_>,
        bo: RoleCreateBo,
        current_user: &CurrentUser,
    ) -> AppResult<()> {
        // 唯一性校验(roleKey + roleName)
        self.ensure_unique(ctx, &bo.role_key, &bo.role_name, None)
            .await?;
        // 领域层的审计值(create_*)会被漏斗按 ctx principal 覆盖 ——
        // 生产路径两者同源(handler 用同一 login_user 构造 ctx)。
        let menu_ids = bo.menu_ids.clone().unwrap_or_default();
        let role = Role::from_create_bo(bo, current_user);
        self.role_repo
            .insert_with_menus(ctx, &role, &menu_ids)
            .await?;
        tracing::info!(
            role_key = ?role.role_key,
            menus = menu_ids.len(),
            operator = %current_user.0.username,
            "创建角色成功"
        );
        Ok(())
    }

    #[tracing::instrument(skip(self, ctx, bo, current_user), fields(operator = %current_user.0.username, role_id = ?bo.role_id))]
    pub async fn update_by_bo(
        &self,
        ctx: &DataContext<'_>,
        bo: RoleUpdateBo,
        current_user: &CurrentUser,
    ) -> AppResult<()> {
        let role_id = bo.role_id.ok_or_else(|| {
            AppError::BadRequest("更新角色失败: 角色ID为空".to_string())
        })?;

        // 越权保护(禁操作内置/超管角色)
        Self::check_role_allowed(role_id)?;

        // 唯一性校验(排除自身)
        self.ensure_unique(ctx, &bo.role_key, &bo.role_name, Some(role_id))
            .await?;

        let mut role = self
            .role_repo
            .select_by_id(ctx, &role_id)
            .await?
            .ok_or_else(|| {
                AppError::NotFound(format!(
                    "更新角色失败: 角色不存在 (id={})",
                    role_id
                ))
            })?;

        role.from_update_bo(bo, current_user);
        self.role_repo.update_by_id(ctx, &role, &role_id).await?;
        // 角色信息变更 → 清理受影响会话
        self.invalidate_sessions(&role.role_key).await;
        // RoleUpdateBo 带 data_scope(from_update_bo 会应用,role.rs
        // model)→ 角色级变更,全清。诚实对照:Java updateRole 无
        // @CacheEvict,因其刻意「仅更新角色基础字段,避免影响权限分配」
        // (SysRoleServiceImpl:325-326),data_scope 不落库;我们的 BO 面
        // 允许 data_scope 进来,失效必须接。注意 insert_by_bo(新建角色)
        // 不接:新角色尚无持有者,不可能出现在任何已缓存范围里,Java
        // insertRole 同样无 @CacheEvict。
        slot_invalidate_all(&self.scope_cache);
        tracing::info!(
            role_id = role_id,
            operator = %current_user.0.username,
            "更新角色成功"
        );
        Ok(())
    }

    #[tracing::instrument(skip(self, ctx, current_user), fields(operator = %current_user.0.username, role_id = id))]
    pub async fn delete_by_id(
        &self,
        ctx: &DataContext<'_>,
        id: i64,
        current_user: &CurrentUser,
    ) -> AppResult<()> {
        // 越权保护
        Self::check_role_allowed(id)?;

        let mut role = self
            .role_repo
            .select_by_id(ctx, &id)
            .await?
            .ok_or_else(|| {
                AppError::NotFound(format!(
                    "软删除角色失败: 角色不存在 (id={})",
                    id
                ))
            })?;

        // 越权拦截靠上面的 `select_by_id` —— 它是**受辖读**
        // (role_repo.rs:203,无 without_row_scope,对齐 Java
        // SysRoleMapper.selectRoleById:75-79 带 @DataPermission 双列):
        // 不在范围内的角色读不到 → NotFound,写根本走不到。Java 的对位
        // 探针在 service(SysRoleServiceImpl.deleteRoleByIds:437
        // checkRoleDataScope),语义等价、位置不同。
        // 软删 = del_flag 置 '1' 的一次 update;**写侧无行级范围**,
        // 漏斗只施加审计盖章与软删存活过滤。
        role.del_flag = Some(DelFlag::Deleted.as_str().to_string());
        role.base_entity.update(current_user.get_user_id());
        self.role_repo.update_by_id(ctx, &role, &id).await?;
        // 角色删除 → 清理受影响会话(使原持有者权限刷新后看到角色消失)
        self.invalidate_sessions(&role.role_key).await;
        // 软删角色(del_flag='1' 被范围查询过滤)→ 全清。Java对照:
        // deleteRoleById `@CacheEvict(SYS_ROLE_CUSTOM, key="#roleId")`(:416)
        // 按 role 定点,批量 deleteRoleByIds 用 allEntries=true(:433);
        // user 键缓存无法按 role 定点,统一取 allEntries 语义。
        slot_invalidate_all(&self.scope_cache);
        tracing::info!(
            role_id = id,
            operator = %current_user.0.username,
            "软删除角色成功"
        );
        Ok(())
    }
}
