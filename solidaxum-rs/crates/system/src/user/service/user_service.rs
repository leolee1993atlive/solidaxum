use app_core::{
    error::{AppError, AppResult},
    infra::Infrastructure,
    is_admin_user,
    middleware::extractors::CurrentUser,
    model::status::{DelFlag, Status},
    util::{
        crypto_util::hash_password,
        excel::{ExcelBuilder, guard_export_rows},
        snowflake,
    },
    vo::page::{TableDataInfo, build_order_by, normalize_paging},
};
use app_data::{
    DataContext, DataError, TxnContext,
    sea_orm::{ActiveValue::Set, ColumnTrait, Condition},
};

use crate::{
    assignee_sync::service::assignee_sync_service::AssigneeSyncService,
    permission::scope_resolver::{
        ScopeCacheSlot, empty_scope_cache_slot, slot_invalidate,
    },
    relation::{user_post, user_role},
    user::{
        handler::user_handler::UserPageQuery,
        model::user::{
            SysUserInfoVo, User, UserCreateBo, UserImportRow, UserListVo,
            UserOptionVo, UserUpdateBo,
        },
        repository::user_repo::UserRepository,
    },
};

#[derive(Clone)]
pub struct UserService {
    user_repo: UserRepository,
    redis_pool: deadpool_redis::Pool,
    /// 数据范围缓存失效槽:用户写路径(authRole/转部门)经它
    /// 定点失效 resolver 缓存。默认空槽(失效 no-op),生产由
    /// ServiceContainer::new 注入共享槽。
    scope_cache: ScopeCacheSlot,
}

impl UserService {
    pub fn new(infra: &Infrastructure) -> Self {
        Self {
            // 服务不自持 db:关联表事务一律走调用方传入的
            // ctx.transaction(user 请求路径由 handler 构造用户 ctx,登录/注册
            // 等无身份路径由调用方传 system_context)。
            user_repo: UserRepository::new(),
            redis_pool: infra.pool.redis_pool.clone(),
            scope_cache: empty_scope_cache_slot(),
        }
    }

    /// 装配/测试用:接上共享失效槽(链式)。生产由 ServiceContainer::new
    /// 统一注入;接上后用户写路径的失效调用立即生效。
    pub fn with_scope_cache(mut self, slot: ScopeCacheSlot) -> Self {
        self.scope_cache = slot;
        self
    }

    /// 越权保护: 禁止对超级管理员做 update/delete/resetPwd 等敏感操作。
    /// 对齐 Java `SysUserServiceImpl.checkUserAllowed`。pub 给 handler:
    /// Java controller 侧 checkUserAllowed 先于 checkUserDataScope
    /// (edit:200-201 / resetPwd:255-256 / changeStatus:272-273),
    /// handler 需在数据范围探针前调它,保证受限操作者打超管靶标时先回
    /// 400「不允许操作超级管理员」而非探针的 404。
    pub fn check_user_allowed(target_user_id: i64) -> AppResult<()> {
        if is_admin_user(target_user_id) {
            return Err(AppError::BadRequest(
                "不允许操作超级管理员".to_string(),
            ));
        }
        Ok(())
    }

    /// 越权保护:对齐 Java `SysUserServiceImpl.checkUserDataScope`
    /// (ServiceImpl:282-292)—— 超管直通(resolver 恒 All),其余走带注解的
    /// `countUserById` 探针;0 行(不存在**或**越权)→ NotFound,不泄露
    /// 存在性。读路径与写路径共用:写路径无行级范围,越权拦截由 handler
    /// 在 edit/resetPwd/changeStatus 显式调它承担(对齐 Java controller
    /// 探针模型)。
    pub async fn check_user_data_scope(
        &self,
        ctx: &DataContext<'_>,
        user_id: i64,
    ) -> AppResult<()> {
        if self.user_repo.count_user_by_id(ctx, user_id).await? == 0 {
            return Err(AppError::NotFound(format!(
                "没有权限访问用户数据 (user_id={})",
                user_id
            )));
        }
        Ok(())
    }

    /// 唯一性校验: 用户名 / 手机号 / 邮箱 在创建或更新时不能与其它用户冲突。
    /// `exclude_id`: 更新时传当前用户 id 以排除自身。
    ///
    /// 唯一性查询在 Java 侧无 @DataPermission(lambda().exists()),repo 内部
    /// `without_row_scope` —— 传入的 ctx 是用户 ctx 也照常全量(唯一性必须
    /// 跨部门判定)。
    async fn ensure_unique(
        &self,
        ctx: &DataContext<'_>,
        user_name: &str,
        phone_number: &Option<String>,
        email: &Option<String>,
        exclude_id: Option<i64>,
    ) -> AppResult<()> {
        if self
            .user_repo
            .exists_by_user_name(ctx, user_name, exclude_id)
            .await?
        {
            return Err(AppError::Conflict(format!(
                "用户名已存在: '{}'",
                user_name
            )));
        }
        if let Some(phone) = phone_number
            && !phone.trim().is_empty()
            && self
                .user_repo
                .exists_by_phone_number(ctx, phone, exclude_id)
                .await?
        {
            return Err(AppError::Conflict(format!(
                "手机号已存在: '{}'",
                phone
            )));
        }
        if let Some(email) = email
            && !email.trim().is_empty()
            && self
                .user_repo
                .exists_by_email(ctx, email, exclude_id)
                .await?
        {
            return Err(AppError::Conflict(format!("邮箱已存在: '{}'", email)));
        }
        Ok(())
    }

    #[tracing::instrument(skip(self, ctx, user))]
    pub async fn register(
        &self,
        ctx: &DataContext<'_>,
        user: User,
    ) -> AppResult<()> {
        match self.get_user_by_user_name(ctx, &user.user_name).await {
            Ok(_) => {
                tracing::warn!(user_name = %user.user_name, "注册失败: 用户名已存在");
                return Err(AppError::Conflict(format!(
                    "用户名已存在: {}",
                    user.user_name
                )));
            }
            Err(AppError::NotFound(_)) => {}
            Err(e) => return Err(e),
        }
        // 注册路径无请求身份:调用方(auth 模块)传 system_context,
        // 审计人 = 0(系统)(设计 7.3)。
        self.user_repo.insert(ctx, &user).await?;
        tracing::info!(user_name = %user.user_name, "用户注册成功");
        Ok(())
    }

    /// 查询用户列表:范围由漏斗按调用方 ctx 自动施加 —— Java
    /// `selectUserList(Wrapper)`
    /// (SysUserMapper.java:51-57 带 @DataPermission)。SelfOnly 只见
    /// create_by=本人 的行(owner 列 = create_by,Java 基线)。
    #[tracing::instrument(skip(self, ctx))]
    pub async fn list(
        &self,
        ctx: &DataContext<'_>,
    ) -> AppResult<Vec<UserListVo>> {
        let result = self.user_repo.select_all(ctx).await?;
        tracing::info!(count = result.len(), "查询用户列表成功");
        Ok(result.into_iter().map(|user| user.to_list_vo()).collect())
    }

    #[tracing::instrument(skip(self, ctx), fields(page = query.page.page_num, size = query.page.page_size))]
    pub async fn page(
        &self,
        ctx: &DataContext<'_>,
        query: UserPageQuery,
    ) -> AppResult<TableDataInfo<UserListVo>> {
        let (current, size, _offset) = normalize_paging(&query.page);
        let allowed = [
            ("userId", "user_id"),
            ("userName", "user_name"),
            ("nickName", "nick_name"),
            ("createTime", "create_time"),
        ];
        let order_by = build_order_by(
            &query.page.order_by_column,
            &query.page.is_asc,
            &allowed,
        )
        .unwrap_or_else(|| "user_id desc".to_string());

        // Java selectPageUserList 带 @DataPermission → 漏斗按 ctx 施加范围
        let page_result = self
            .user_repo
            .select_page(ctx, &query, current as u64, size as u64, &order_by)
            .await?;
        if page_result.total <= 0 {
            return Ok(TableDataInfo::empty());
        }
        tracing::info!(
            current,
            size,
            total = page_result.total,
            "用户分页查询成功"
        );
        Ok(page_result)
    }

    /// 按查询条件取全部用户(导出用),排序白名单与 page 一致。
    /// Java selectUserExportList 带 @DataPermission(**与 post 相反**,
    /// user 的导出在 Java 基线就是受辖的)→ 漏斗按 ctx 施加。
    #[tracing::instrument(skip(self, ctx))]
    pub async fn list_all(
        &self,
        ctx: &DataContext<'_>,
        query: &UserPageQuery,
    ) -> AppResult<Vec<UserListVo>> {
        let allowed = [
            ("userId", "user_id"),
            ("userName", "user_name"),
            ("nickName", "nick_name"),
            ("createTime", "create_time"),
        ];
        let order_by = build_order_by(
            &query.page.order_by_column,
            &query.page.is_asc,
            &allowed,
        )
        .unwrap_or_else(|| "user_id desc".to_string());
        // 取数前先 COUNT:用户表随组织规模增长无硬上限,不带筛选条件的
        // 导出会把受辖范围内的全部用户拉进内存再抛错。
        let total = self.user_repo.count_by_query(ctx, query).await?;
        guard_export_rows(total as usize)?;
        Ok(self.user_repo.select_list(ctx, query, &order_by).await?)
    }

    #[tracing::instrument(skip(self, ctx), fields(user_name = user_name))]
    pub async fn get_user_by_user_name(
        &self,
        ctx: &DataContext<'_>,
        user_name: &str,
    ) -> AppResult<User> {
        // Java lambda().voOne() 无注解 → repo 内部 without_row_scope;
        // 登录路径(无身份)传 system_context,已登录路径传用户 ctx 均可。
        let user = self
            .user_repo
            .select_by_user_name(ctx, user_name)
            .await?
            .ok_or_else(|| {
                AppError::NotFound(format!(
                    "用户不存在 (user_name={})",
                    user_name
                ))
            })?;
        tracing::info!(user_name = user_name, "根据用户名查询用户成功");
        Ok(user)
    }

    #[tracing::instrument(skip(self, ctx), fields(user_id = id))]
    pub async fn get_user_by_id(
        &self,
        ctx: &DataContext<'_>,
        id: i64,
    ) -> AppResult<User> {
        // Java selectVoById 无注解 → repo 内部 without_row_scope;
        // 读路径的越权判定见 check_user_data_scope(调用方决定是否探针)。
        let user =
            self.user_repo
                .select_by_id(ctx, &id)
                .await?
                .ok_or_else(|| {
                    AppError::NotFound(format!("用户不存在 (id={})", id))
                })?;
        tracing::info!(user_id = id, "根据ID查询用户成功");
        Ok(user)
    }

    #[tracing::instrument(skip(self, ctx, bo, current_user), fields(operator = %current_user.0.username, new_user_name = ?bo.user_name))]
    pub async fn insert_by_bo(
        &self,
        ctx: &DataContext<'_>,
        bo: UserCreateBo,
        current_user: &CurrentUser,
    ) -> AppResult<()> {
        // 唯一性校验(用户名/手机/邮箱)
        self.ensure_unique(
            ctx,
            &bo.user_name,
            &bo.phone_number,
            &bo.email,
            None,
        )
        .await?;

        let post_ids = bo.post_ids.clone();
        let role_ids = bo.role_ids.clone();
        let user = User::from_create_bo(bo).map_err(|e| {
            AppError::Internal(format!("创建用户失败: 构建用户实体失败: {}", e))
        })?;
        // 审计五列由漏斗按 tx(携带同一 principal)盖章 ——
        // 领域层 from_create_bo 不手工填审计值。
        //
        // **行为加固(事务化)**:原实现「插用户 → 各自开事务换岗位/角色」
        // 三段无事务写,任一段失败会留下半截数据(用户已建但岗位/角色
        // 缺失);现收进单事务,失败整笔回滚。补偿任务(flow_assignee_sync)
        // 与变更**同事务**写入 —— 变更失败则补偿行不存在,补偿行存在则
        // 变更必已提交(outbox 可靠性不变量,参考实现)。
        let uid = user.user_id.unwrap_or_default();
        let user_name = user.user_name.clone();
        let logged_name = user_name.clone();
        // owned 行/模型先进闭包(transaction 的 HRTB 不收借用切片,
        // 同 update_user_posts 的既有处理);repo 同理 —— clone 后 move
        // 进闭包,不捕获 &self(HRTB 对捕获的寿命 universally quantified,
        // role_service::update_role_permission 同款处理)
        let user_repo = self.user_repo.clone();
        let post_models: Option<Vec<user_post::ActiveModel>> =
            post_ids.as_ref().map(|ids| {
                ids.iter()
                    .map(|&pid| user_post::ActiveModel {
                        user_id: Set(uid),
                        post_id: Set(pid),
                    })
                    .collect()
            });
        let role_models: Option<Vec<user_role::ActiveModel>> =
            role_ids.as_ref().map(|ids| {
                ids.iter()
                    .map(|&rid| user_role::ActiveModel {
                        user_id: Set(uid),
                        role_id: Set(rid),
                    })
                    .collect()
            });
        ctx.transaction(|tx| {
            Box::pin(async move {
                user_repo.insert_in_txn(tx, &user).await?;
                if let Some(models) = post_models {
                    // 原 DELETE FROM sys_user_post WHERE user_id = ?
                    // (关联表 DataPolicy 全 None → delete_where 唯一合法落点)
                    tx.delete_where::<user_post::Entity>(
                        Condition::all().add(user_post::Column::UserId.eq(uid)),
                    )
                    .await?;
                    tx.insert_many(models).await?;
                }
                if let Some(models) = role_models {
                    Self::replace_user_roles_in_txn(tx, uid, models).await?;
                }
                // 补偿任务与用户创建同事务,失败同事务回滚
                AssigneeSyncService::enqueue_tx(
                    tx,
                    "user",
                    "add",
                    Some(&user_name),
                    &[uid],
                )
                .await?;
                Ok(())
            })
        })
        .await?;
        // 新用户范围缓存必不存在,此处失效幂等 no-op(原经 insert_user_auth
        // 内失效的平价保留)。
        slot_invalidate(&self.scope_cache, uid);
        tracing::info!(
            user_name = %logged_name,
            operator = %current_user.0.username,
            "创建用户成功"
        );
        Ok(())
    }

    #[tracing::instrument(skip(self, ctx, bo, current_user), fields(operator = %current_user.0.username, user_id = ?bo.user_id))]
    pub async fn update_by_bo(
        &self,
        ctx: &DataContext<'_>,
        bo: UserUpdateBo,
        current_user: &CurrentUser,
    ) -> AppResult<()> {
        let user_id = bo.user_id.ok_or_else(|| {
            AppError::BadRequest("更新用户失败: 用户ID为空".to_string())
        })?;

        // 越权保护(禁操作超管)
        Self::check_user_allowed(user_id)?;

        // 唯一性校验(排除自身)
        self.ensure_unique(
            ctx,
            &bo.user_name,
            &bo.phone_number,
            &bo.email,
            Some(user_id),
        )
        .await?;

        let post_ids = bo.post_ids.clone();
        let role_ids = bo.role_ids.clone();
        // selectVoById 无注解(豁免读);越权目标的拦截在 update_by_id 的
        // WHERE(Java updateById 带 @DataPermission:158-163,漏斗同口径):
        // 越权行 0 行匹配 → NotFound。
        let mut user = self
            .user_repo
            .select_by_id(ctx, &user_id)
            .await?
            .ok_or_else(|| {
                AppError::NotFound(format!(
                    "更新用户失败: 用户不存在 (id={})",
                    user_id
                ))
            })?;

        user.from_update_bo(bo);
        // **行为加固(事务化)**:原实现「更新用户 → 各自开事务换岗位/
        // 角色」无事务拼接,失败留半截数据;现收进单事务整笔回滚。
        // 补偿任务(flow_assignee_sync)与变更**同事务**写入(参考实现
        // outbox 可靠性不变量,理由同 insert_by_bo)。
        let user_name = user.user_name.clone();
        // repo clone 后 move 进闭包(HRTB 不捕获 &self,理由同 insert_by_bo)
        let user_repo = self.user_repo.clone();
        let post_models: Option<Vec<user_post::ActiveModel>> =
            post_ids.as_ref().map(|ids| {
                ids.iter()
                    .map(|&pid| user_post::ActiveModel {
                        user_id: Set(user_id),
                        post_id: Set(pid),
                    })
                    .collect()
            });
        let role_models: Option<Vec<user_role::ActiveModel>> =
            role_ids.as_ref().map(|ids| {
                ids.iter()
                    .map(|&rid| user_role::ActiveModel {
                        user_id: Set(user_id),
                        role_id: Set(rid),
                    })
                    .collect()
            });
        ctx.transaction(|tx| {
            Box::pin(async move {
                user_repo.update_by_id_in_txn(tx, &user, &user_id).await?;
                if let Some(models) = post_models {
                    tx.delete_where::<user_post::Entity>(
                        Condition::all()
                            .add(user_post::Column::UserId.eq(user_id)),
                    )
                    .await?;
                    tx.insert_many(models).await?;
                }
                if let Some(models) = role_models {
                    Self::replace_user_roles_in_txn(tx, user_id, models)
                        .await?;
                }
                // 补偿任务与用户更新同事务,失败同事务回滚
                AssigneeSyncService::enqueue_tx(
                    tx,
                    "user",
                    "update",
                    Some(&user_name),
                    &[user_id],
                )
                .await?;
                Ok(())
            })
        })
        .await?;
        // 转部门 → 定点失效该用户。缓存条目内嵌了**旧部门**的子树
        // 展开(DeptAndBelow 贡献),部门变了旧展开即陈旧(UserUpdateBo.
        // dept_id 由 from_update_bo 应用)。role_ids 变更路径的事务内
        // 替换不再经 insert_user_auth,失效集中在此一次覆盖。
        // Java 无对应失效(它无 user 级缓存,每请求实时算)。
        slot_invalidate(&self.scope_cache, user_id);
        tracing::info!(
            user_id = user_id,
            operator = %current_user.0.username,
            "更新用户成功"
        );
        Ok(())
    }

    #[tracing::instrument(skip(self, ctx, current_user), fields(operator = %current_user.0.username, user_id = id))]
    pub async fn delete_by_id(
        &self,
        ctx: &DataContext<'_>,
        id: i64,
        current_user: &CurrentUser,
    ) -> AppResult<()> {
        // 不能删超管 + 不能删当前登录用户
        Self::check_user_allowed(id)?;
        // Java 的软删探针在 **service 层**:SysUserServiceImpl
        // .deleteUserByIds:531-534 对每个 userId 依次 checkUserAllowed +
        // checkUserDataScope(controller remove:221-228 只有「不能删自己」,
        // 只看 controller 会误判成「Java 无探针」)。写路径无行级范围,
        // 软删的越权拦截**只能**靠这道探针,顺序与 Java 逐字对位。
        self.check_user_data_scope(ctx, id).await?;
        if id == current_user.0.user_id {
            return Err(AppError::BadRequest(
                "不能删除当前登录用户".to_string(),
            ));
        }

        // 软删 = del_flag 置 '1' 的一次 update:漏斗盖 update_*(审计人 =
        // ctx principal),WHERE = 主键 + 软删存活,已删行 → NotFound。
        // 越权拦截与 Java remove 平价(不存在),见 handler 注释。
        let mut user = self
            .user_repo
            .select_by_id(ctx, &id)
            .await?
            .ok_or_else(|| {
                AppError::NotFound(format!(
                    "软删除用户失败: 用户不存在 (id={})",
                    id
                ))
            })?;

        user.del_flag = Some(DelFlag::Deleted.as_str().to_string());

        self.user_repo.update_by_id(ctx, &user, &id).await?;
        tracing::info!(
            user_id = id,
            operator = %current_user.0.username,
            "软删除用户成功"
        );
        Ok(())
    }

    #[tracing::instrument(skip(self, ctx, password), fields(user_id = id))]
    pub async fn update_password(
        &self,
        ctx: &DataContext<'_>,
        id: i64,
        password: String,
    ) -> AppResult<()> {
        // 重置密码也禁对超管
        Self::check_user_allowed(id)?;

        let mut user = self
            .user_repo
            .select_by_id(ctx, &id)
            .await?
            .ok_or_else(|| {
                AppError::NotFound(format!(
                    "更新密码失败: 用户不存在 (id={})",
                    id
                ))
            })?;

        user.password = hash_password(password.as_str()).map_err(|e| {
            AppError::Internal(format!("更新密码失败: 密码哈希失败: {}", e))
        })?;

        // 越权拦截在 handler 的 check_user_data_scope 探针;
        // 此处 update 只按主键 + 软删存活。
        self.user_repo.update_by_id(ctx, &user, &id).await?;
        tracing::info!(user_id = id, "更新密码成功");
        Ok(())
    }

    /// 更新用户岗位关联 (事务: 先删后插)。`ctx.transaction` 闭包内
    /// `tx.delete_where`(豁免关联表才放行的窄原语)+ `tx.insert_many`
    /// 批插;Err → 整体回滚。注意:insert_by_bo/update_by_bo 自
    ///参考实现 事务化后不再调用本方法(岗位替换已内联进各自的
    /// 外层事务,与补偿任务同事务);本方法保持独立事务入口,供其余
    /// 调用方使用。
    pub async fn update_user_posts(
        &self,
        ctx: &DataContext<'_>,
        user_id: i64,
        post_ids: &[i64],
    ) -> AppResult<()> {
        // 先构造 owned 关联行再进闭包:transaction 的 HRTB(FnOnce for<'c>
        // → BoxFuture<'c>)不允许捕获调用方切片的借用。
        let models: Vec<user_post::ActiveModel> = post_ids
            .iter()
            .map(|&pid| user_post::ActiveModel {
                user_id: Set(user_id),
                post_id: Set(pid),
            })
            .collect();
        ctx.transaction(|tx| {
            Box::pin(async move {
                // 原 DELETE FROM sys_user_post WHERE user_id = ?
                // (关联表 DataPolicy 全 None → delete_where 唯一合法落点)
                tx.delete_where::<user_post::Entity>(
                    Condition::all().add(user_post::Column::UserId.eq(user_id)),
                )
                .await?;
                tx.insert_many(models).await?;
                Ok(())
            })
        })
        .await?;
        Ok(())
    }

    /// 分配用户角色 (事务: 先删后插)。事务形状同
    /// [`Self::update_user_posts`](漏斗 delete_where + insert_many 批插)。
    ///
    /// **行为加固(事务化)**:原实现单写无事务,现把授权替换与
    /// 补偿任务(flow_assignee_sync)收进**同一事务** —— 补偿行与
    /// 授权变更同生共死(参考实现 outbox 可靠性不变量)。
    pub async fn insert_user_auth(
        &self,
        ctx: &DataContext<'_>,
        user_id: i64,
        role_ids: &[i64],
    ) -> AppResult<()> {
        // 同 update_user_posts:owned 行先进闭包(HRTB 不收借用切片)
        let models: Vec<user_role::ActiveModel> = role_ids
            .iter()
            .map(|&rid| user_role::ActiveModel {
                user_id: Set(user_id),
                role_id: Set(rid),
            })
            .collect();
        // 补偿行的 auth_code = 用户名(可读标识)。用户行缺失不阻断
        // 授权变更,降级为 None 并告警(事件兜底仍按 user_ids 生效)。
        let user_name = self
            .user_repo
            .select_by_id(ctx, &user_id)
            .await?
            .map(|u| u.user_name);
        if user_name.is_none() {
            tracing::warn!(
                user_id,
                "insert_user_auth: 用户不存在,补偿任务 auth_code 置空"
            );
        }
        ctx.transaction(|tx| {
            Box::pin(async move {
                Self::replace_user_roles_in_txn(tx, user_id, models).await?;
                // 补偿任务与授权变更同事务,失败同事务回滚
                AssigneeSyncService::enqueue_tx(
                    tx,
                    "user",
                    "update",
                    user_name.as_deref(),
                    &[user_id],
                )
                .await?;
                Ok(())
            })
        })
        .await?;
        // 用户角色集被整体替换(authRole 分配、新建/编辑用户携带
        // roleIds)→ 定点失效该 user。Java 无对应失效(它无 user 级缓存,
        // 每请求实时算范围);我们按 user_id 键缓存,此处一处接线覆盖
        // 独立 authRole 入口(insert_by_bo/update_by_bo 已改由外层事务
        // 统一失效)。
        slot_invalidate(&self.scope_cache, user_id);
        Ok(())
    }

    /// 用户角色授权替换(先删后插)的事务核心。
    ///
    /// **不在此处写补偿任务**:insert_by_bo/update_by_bo 由外层写
    /// user/add|update 补偿行,独立 insert_user_auth 入口写 user/update
    /// 行 —— 核心层若统一写,嵌套入口会产生重复补偿行。
    async fn replace_user_roles_in_txn(
        tx: &TxnContext<'_>,
        user_id: i64,
        models: Vec<user_role::ActiveModel>,
    ) -> Result<(), DataError> {
        // 原 DELETE FROM sys_user_role WHERE user_id = ?
        tx.delete_where::<user_role::Entity>(
            Condition::all().add(user_role::Column::UserId.eq(user_id)),
        )
        .await?;
        tx.insert_many(models).await?;
        Ok(())
    }

    /// 获取用户完整信息(含角色/岗位 ID 与可选列表)
    #[tracing::instrument(skip(self, ctx), fields(user_id = id))]
    pub async fn get_user_info(
        &self,
        ctx: &DataContext<'_>,
        id: i64,
    ) -> AppResult<SysUserInfoVo> {
        let user = self.get_user_by_id(ctx, id).await?;
        // 关联表读(全 None 豁免)
        let role_ids =
            self.user_repo.select_role_ids_by_user_id(ctx, id).await?;
        let post_ids =
            self.user_repo.select_post_ids_by_user_id(ctx, id).await?;
        Ok(SysUserInfoVo {
            user: user.to_detail_vo(),
            role_ids,
            post_ids,
            roles: Vec::new(),
            posts: Vec::new(),
        })
    }

    /// 变更用户状态
    #[tracing::instrument(skip(self, ctx), fields(user_id = id))]
    pub async fn change_status(
        &self,
        ctx: &DataContext<'_>,
        id: i64,
        status: String,
    ) -> AppResult<()> {
        // 漏斗 update WHERE=pk+软删存活,已删 0 行 → NotFound;越权拦截
        // 在 handler 的 check_user_data_scope 探针。
        self.user_repo.update_status(ctx, id, &status).await?;
        tracing::info!(user_id = id, status = %status, "修改用户状态成功");
        Ok(())
    }

    /// 解锁用户账号:清除 Redis 中的密码错误计数
    #[tracing::instrument(skip(self, ctx), fields(user_id = id))]
    pub async fn unlock(
        &self,
        ctx: &DataContext<'_>,
        id: i64,
    ) -> AppResult<()> {
        let user = self.get_user_by_id(ctx, id).await?;
        let key = format!("pwd_err_cnt:{}", user.user_name);
        if let Ok(mut conn) = self.redis_pool.get().await {
            let _: () = redis::cmd("DEL")
                .arg(&key)
                .query_async(&mut *conn)
                .await
                .unwrap_or(());
        }
        tracing::info!(
            user_id = id,
            user_name = %user.user_name,
            "解锁用户账号成功:清除密码错误计数"
        );
        Ok(())
    }

    /// 用户下拉选项（对齐 Java `selectUserByIds`：status=0 + 可选 dept/userIds）
    /// Java selectUserByIds 走**带注解的** selectUserList → 漏斗按 ctx 施加
    /// (与 post 的下拉豁免相反)。
    #[tracing::instrument(skip(self, ctx))]
    pub async fn option_select(
        &self,
        ctx: &DataContext<'_>,
        user_ids: Option<Vec<i64>>,
        dept_id: Option<i64>,
    ) -> AppResult<Vec<UserOptionVo>> {
        let ids_ref = user_ids.as_deref().filter(|ids| !ids.is_empty());
        let users = self
            .user_repo
            .select_option_by_ids_and_dept(ctx, ids_ref, dept_id)
            .await?;
        Ok(users
            .into_iter()
            .map(|u| UserOptionVo {
                user_id: u.user_id.unwrap_or_default(),
                user_name: u.user_name,
                nick_name: u.nick_name,
            })
            .collect())
    }

    /// 按部门 ID 查询用户列表（Vue 契约：数组）
    /// Java selectUserListByDept 无注解 → repo 内部 without_row_scope。
    #[tracing::instrument(skip(self, ctx), fields(dept_id = dept_id))]
    pub async fn list_by_dept_id(
        &self,
        ctx: &DataContext<'_>,
        dept_id: i64,
    ) -> AppResult<Vec<UserListVo>> {
        let rows = self.user_repo.select_list_by_dept_id(ctx, dept_id).await?;
        Ok(rows)
    }

    /// 按部门 ID 分页查询用户(Java 无独立分页锚,同走 lambda 豁免语义)
    #[tracing::instrument(skip(self, ctx), fields(dept_id = dept_id))]
    pub async fn page_by_dept_id(
        &self,
        ctx: &DataContext<'_>,
        dept_id: i64,
        page_no: u64,
        page_size: u64,
    ) -> AppResult<TableDataInfo<UserListVo>> {
        let result = self
            .user_repo
            .select_page_by_dept_id(ctx, dept_id, page_no, page_size)
            .await?;
        Ok(result)
    }

    /// 导入用户:解析 Excel → 逐行校验/插入 → 返回汇总消息
    #[tracing::instrument(skip(self, ctx, data, current_user), fields(operator = %current_user.0.username))]
    pub async fn import_data(
        &self,
        ctx: &DataContext<'_>,
        data: &[u8],
        init_password: &str,
        current_user: &CurrentUser,
    ) -> AppResult<String> {
        let rows = ExcelBuilder::<UserImportRow>::import(data)?;
        let total = rows.len();
        if total == 0 {
            return Err(AppError::BadRequest("导入数据为空".to_string()));
        }

        let hashed_pwd = hash_password(init_password).map_err(|e| {
            AppError::Internal(format!("初始密码哈希失败: {}", e))
        })?;

        let mut success = 0u32;
        let mut failure = 0u32;
        let mut messages = Vec::new();

        for row in rows {
            // 用户名必填(已在 from_record 校验,这里再防御)
            if row.user_name.is_empty() {
                failure += 1;
                messages.push(format!(
                    "第 {} 行: 用户账号为空, 跳过",
                    success + failure
                ));
                continue;
            }
            // 唯一性校验:用户名已存在则跳过(豁免方法,ctx 不影响判定)
            if self
                .user_repo
                .select_by_user_name(ctx, &row.user_name)
                .await?
                .is_some()
            {
                failure += 1;
                messages.push(format!("用户名 {} 已存在, 跳过", row.user_name));
                continue;
            }

            let user = User {
                user_id: Some(snowflake::next_id()),
                dept_id: row.dept_id,
                user_name: row.user_name.clone(),
                nick_name: row.nick_name.clone(),
                password: hashed_pwd.clone(),
                user_type: Some("sys_user".to_string()),
                email: row.email.clone(),
                phone_number: row.phone_number.clone(),
                gender: row.gender.clone(),
                avatar: None,
                status: row
                    .status
                    .clone()
                    .unwrap_or_else(|| Status::Normal.as_str().to_string()),
                del_flag: Some(DelFlag::Alive.as_str().to_string()),
                login_ip: None,
                login_date: None,
                remark: None,
                // 审计由漏斗按 ctx 盖章(导入人 = principal)
                base_entity: app_core::model::entity::BaseEntity::default(),
            };

            match self.user_repo.insert(ctx, &user).await {
                Ok(_) => {
                    success += 1;
                }
                Err(e) => {
                    failure += 1;
                    messages.push(format!(
                        "用户 {} 插入失败: {}",
                        row.user_name, e
                    ));
                }
            }
        }

        let mut msg = format!("导入成功 {} 条, 失败 {} 条", success, failure);
        if !messages.is_empty() {
            msg.push('\n');
            msg.push_str(&messages.join("\n"));
        }
        tracing::info!(
            operator = %current_user.0.username,
            total, success, failure,
            "用户导入完成"
        );
        Ok(msg)
    }
}
