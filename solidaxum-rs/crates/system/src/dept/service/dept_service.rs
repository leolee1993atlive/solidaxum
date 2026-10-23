use std::collections::HashSet;

use app_core::{
    error::{AppError, AppResult},
    infra::Infrastructure,
    middleware::extractors::CurrentUser,
    model::status::{DelFlag, Status},
};
use app_data::DataContext;

use crate::{
    dept::{
        model::dept::{
            DEFAULT_DEPT_ID, Dept, DeptCreateBo, DeptListVo, DeptQueryBo,
            DeptUpdateBo, ROOT_PARENT_ID,
        },
        repository::dept_repo::DeptRepository,
    },
    permission::scope_resolver::{
        ScopeCacheSlot, empty_scope_cache_slot, slot_invalidate_all,
    },
    post::repository::post_repo::PostRepository,
    user::repository::user_repo::UserRepository,
};

#[derive(Clone)]
pub struct DeptService {
    dept_repo: DeptRepository,
    user_repo: UserRepository,
    post_repo: PostRepository,
    /// 数据范围缓存失效槽:部门树写路径经它主动失效
    /// resolver 缓存。默认空槽(失效 no-op),生产由 ServiceContainer::new
    /// 注入共享槽。
    scope_cache: ScopeCacheSlot,
}

impl DeptService {
    /// 装配期不取任何快照:本服务全部
    /// 方法只 **转发** ctx,用户请求的 ctx 由 handler 请求期从
    /// `infra.data_source()` 构造(见 app.rs 装配顺序注释)。形参仅保装配
    /// 调用面形状。
    pub fn new(_infra: &Infrastructure) -> Self {
        Self {
            dept_repo: DeptRepository::new(),
            user_repo: UserRepository::new(),
            post_repo: PostRepository::new(),
            scope_cache: empty_scope_cache_slot(),
        }
    }

    /// 测试用构造器(仓储无状态、不消费连接,测试的 ctx 一律经自有
    /// `ds.system_context()` 构造)。
    pub fn for_test() -> Self {
        Self {
            dept_repo: DeptRepository::new(),
            user_repo: UserRepository::new(),
            post_repo: PostRepository::new(),
            scope_cache: empty_scope_cache_slot(),
        }
    }

    /// 装配/测试用:接上共享失效槽(链式)。生产由 ServiceContainer::new
    /// 统一注入;接上后部门树写路径的失效调用立即生效。
    pub fn with_scope_cache(mut self, slot: ScopeCacheSlot) -> Self {
        self.scope_cache = slot;
        self
    }

    pub fn dept_repo(&self) -> &DeptRepository {
        &self.dept_repo
    }

    /// 越权保护(部门维度):对齐 Java `SysDeptServiceImpl:292-302` 的
    /// checkDeptDataScope —— deptId 空 → 放行;超管 → 放行(生产由
    /// CachedScopeResolver 解析为 All,探针经漏斗自然通过,无特判,
    /// 与 user 的 check_user_data_scope 同口径);其余走带注解的
    /// `countDeptById` 探针(SysDeptMapper.java:63-68,@DataPermission 锚
    /// 本表主键 dept_id)。count==0(不存在**或**越权)→ NotFound,
    /// 不泄露存在性 —— 错误变体沿用 checkUserDataScope 的先例
    /// (NotFound 而非 Forbidden:与读路径"这行对你不存在"同形态)。
    /// sys_dept 受辖于自身主键,受辖 ctx 下的 count 自动带范围 ——
    /// funnel-native,无需手工范围匹配。
    pub async fn check_dept_data_scope(
        &self,
        ctx: &DataContext<'_>,
        dept_id: Option<i64>,
    ) -> AppResult<()> {
        let Some(dept_id) = dept_id else {
            return Ok(());
        };
        if self.dept_repo.count_dept_by_id(ctx, dept_id).await? == 0 {
            return Err(AppError::NotFound(format!(
                "没有权限访问部门数据 (dept_id={})",
                dept_id
            )));
        }
        Ok(())
    }

    #[tracing::instrument(skip(self, ctx, query))]
    pub async fn list(
        &self,
        ctx: &DataContext<'_>,
        query: DeptQueryBo,
    ) -> AppResult<Vec<DeptListVo>> {
        let depts = self
            .dept_repo
            .select_list(
                ctx,
                &query.dept_name,
                &query.status,
                &query.dept_category,
            )
            .await?;
        let vos: Vec<DeptListVo> =
            depts.iter().map(|d| d.to_list_vo()).collect();
        Ok(vos)
    }

    /// 排除节点列表(对齐 Java excludeChild,SysDeptController:57-62):
    /// Java 走受辖 `selectDeptList(new SysDeptBo())` 再内存 removeIf
    /// (ancestors 含 deptId);此处同构 —— 受辖列表 + 内存按后代集剔除
    /// (find_descendant_ids 恒含自身)。
    #[tracing::instrument(skip(self, ctx))]
    pub async fn list_exclude(
        &self,
        ctx: &DataContext<'_>,
        exclude_id: i64,
    ) -> AppResult<Vec<DeptListVo>> {
        let depts =
            self.dept_repo.select_list(ctx, &None, &None, &None).await?;
        let excluded = self
            .dept_repo
            .find_descendant_ids(ctx, exclude_id)
            .await?
            .into_iter()
            .collect::<HashSet<_>>();
        let vos: Vec<DeptListVo> = depts
            .iter()
            .filter(|d| !excluded.contains(&d.dept_id))
            .map(|d| d.to_list_vo())
            .collect();
        Ok(vos)
    }

    #[tracing::instrument(skip(self, ctx), fields(dept_id = id))]
    pub async fn get_by_id(
        &self,
        ctx: &DataContext<'_>,
        id: i64,
    ) -> AppResult<Dept> {
        let dept =
            self.dept_repo
                .select_by_id(ctx, &id)
                .await?
                .ok_or_else(|| {
                    AppError::NotFound(format!("部门不存在 (id={})", id))
                })?;
        Ok(dept)
    }

    /// Java add(SysDeptController:87-92)**无** checkDeptDataScope ——
    /// 新增不探针;父部门存在性/状态守卫与 Java insertDept
    /// (ServiceImpl:312-324)同构。
    #[tracing::instrument(skip(self, ctx, bo, current_user), fields(operator = %current_user.0.username))]
    pub async fn insert_by_bo(
        &self,
        ctx: &DataContext<'_>,
        bo: DeptCreateBo,
        current_user: &CurrentUser,
    ) -> AppResult<()> {
        if self
            .dept_repo
            .select_by_name_and_parent(ctx, &bo.dept_name, bo.parent_id)
            .await?
            .is_some()
        {
            return Err(AppError::BadRequest(format!(
                "同一父部门下已存在部门名称: {}",
                bo.dept_name
            )));
        }

        let parent = if bo.parent_id == ROOT_PARENT_ID {
            virtual_root_dept()
        } else {
            self.dept_repo
                .select_by_id(ctx, &bo.parent_id)
                .await?
                .ok_or_else(|| {
                    AppError::NotFound(format!(
                        "父部门不存在 (id={})",
                        bo.parent_id
                    ))
                })?
        };

        let dept = Dept::from_create_bo(bo, &parent, current_user);
        // 审计五列由漏斗 insert 按 ctx 的 principal 盖章
        self.dept_repo.insert(ctx, &dept).await?;
        // 新增子部门扩大父部门的 DeptAndBelow 子树展开
        // (范围条目内嵌子树,新增即陈旧)→ 写-后-失效,全清。粒度取舍
        // 同 role 写路径(user_id 键缓存无法反查受影响用户,1024 容量下
        // 全清代价可忽略,对齐 Java @CacheEvict(allEntries) 语义)。
        slot_invalidate_all(&self.scope_cache);
        tracing::info!(
            dept_id = dept.dept_id,
            operator = %current_user.0.username,
            "创建部门成功"
        );
        Ok(())
    }

    /// Java edit(SysDeptController:104-119)对 **bo.deptId** 的探针在
    /// handler 承担(:106);这里承担 Java ServiceImpl:344-347 的**换父
    /// 探针** —— parent 变化时先校验新父部门权限(避免越权),后才
    /// selectById(新父)/循环检测,顺序同 Java。
    #[tracing::instrument(skip(self, ctx, bo, current_user), fields(operator = %current_user.0.username, dept_id = ?bo.dept_id))]
    pub async fn update_by_bo(
        &self,
        ctx: &DataContext<'_>,
        bo: DeptUpdateBo,
        current_user: &CurrentUser,
    ) -> AppResult<()> {
        let dept_id = bo.dept_id.ok_or_else(|| {
            AppError::BadRequest("更新部门失败: 部门ID为空".to_string())
        })?;

        if bo.parent_id == dept_id {
            return Err(AppError::BadRequest("上级部门不能是自己".to_string()));
        }

        let mut dept = self
            .dept_repo
            .select_by_id(ctx, &dept_id)
            .await?
            .ok_or_else(|| {
                AppError::NotFound(format!(
                    "更新部门失败: 部门不存在 (id={})",
                    dept_id
                ))
            })?;

        if let Some(existing) = self
            .dept_repo
            .select_by_name_and_parent(ctx, &bo.dept_name, bo.parent_id)
            .await?
            && existing.dept_id != dept_id
        {
            return Err(AppError::BadRequest(format!(
                "同一父部门下已存在部门名称: {}",
                bo.dept_name
            )));
        }

        let parent_changed = bo.parent_id != dept.parent_id;
        let old_ancestors = dept.ancestors.clone();

        // 停用部门时,若仍含正常状态子部门则拒(对齐 Java SysDeptController:104-142)
        let being_disabled = bo.status.as_deref()
            == Some(Status::Disabled.as_str())
            && dept.status == Status::Normal.as_str();
        if being_disabled {
            let active_children =
                self.dept_repo.count_normal_children(ctx, dept_id).await?;
            if active_children > 0 {
                return Err(AppError::BadRequest(
                    "部门包含未停用的子部门，不能停用".to_string(),
                ));
            }
            // checkDeptExistUser(对齐 SysDeptController.edit:114-116):
            // 部门下存在正常用户时禁止停用。守卫计数是
            // Java 豁免方法(无 @DataPermission,without_row_scope),
            // 直接透传请求 ctx,语义等价。
            let user_count =
                self.user_repo.count_normal_by_dept_id(ctx, dept_id).await?;
            if user_count > 0 {
                return Err(AppError::BadRequest(
                    "该部门下存在已分配用户,不能禁用".to_string(),
                ));
            }
        }

        if parent_changed {
            // Java ServiceImpl:344-346:换父先校验新父部门权限 ——
            // checkDeptDataScope(dept.getParentId())。注意 parent_id=0
            // (挂根)时 Java 同样探针(countDeptById(0)=0 → 拒),非超管
            // 不能挂根是上游平价行为,原样保留不特判。
            self.check_dept_data_scope(ctx, Some(bo.parent_id)).await?;
            if bo.parent_id == ROOT_PARENT_ID {
                dept.parent_id = ROOT_PARENT_ID;
                dept.ancestors = Some("0".to_string());
            } else {
                let parent = self
                    .dept_repo
                    .select_by_id(ctx, &bo.parent_id)
                    .await?
                    .ok_or_else(|| {
                        AppError::NotFound(format!(
                            "父部门不存在 (id={})",
                            bo.parent_id
                        ))
                    })?;

                let descendants = self
                    .dept_repo
                    .find_descendant_ids(ctx, dept_id)
                    .await?
                    .into_iter()
                    .collect::<HashSet<_>>();
                if descendants.contains(&parent.dept_id) {
                    return Err(AppError::BadRequest(
                        "上级部门不能是自己的下级部门".to_string(),
                    ));
                }

                dept.parent_id = parent.dept_id;
                dept.ancestors = Some(format!(
                    "{},{}",
                    parent.ancestors.as_deref().unwrap_or("0"),
                    parent.dept_id
                ));
            }
        }

        dept.from_update_bo(bo, current_user);
        // Java `updateDept` 带 @Transactional(ServiceImpl:337):换父 =
        // 自身 update + 子孙 ancestors 逐行级联,两段写必须同事务。
        // 级联写不受行级范围(跨范围子部门不让换父整单失败),事务包裹:
        // 真出错时不留
        // "父已移、子未级联"的撕裂祖先链。闭包形状对齐
        // user_service::update_user_posts 先例;HRTB 不收借用,owned
        // 数据(dept/cascade)先进闭包。非换父路径是单写,天然原子。
        if parent_changed {
            let cascade = match (old_ancestors, dept.ancestors.clone()) {
                (Some(old), Some(new)) => Some((old, new)),
                _ => None,
            };
            ctx.transaction(|tx| {
                Box::pin(async move {
                    DeptRepository::update_by_id_in_txn(tx, &dept, &dept_id)
                        .await?;
                    if let Some((old, new)) = cascade {
                        DeptRepository::update_children_ancestors_in_txn(
                            tx, dept_id, &old, &new,
                        )
                        .await?;
                    }
                    Ok(())
                })
            })
            .await?;
        } else {
            // update_* 审计由漏斗 update 盖章;已删行 0 行 → NotFound
            // (写路径不受行级范围)
            self.dept_repo.update_by_id(ctx, &dept, &dept_id).await?;
        }

        // 换父把子树迁出/迁入范围条目(子孙 ancestors 级联,
        // DeptAndBelow 展开随之变化)→ 写-后-失效,全清(非换父的改名/
        // 停启用不动子树,随同全清:粗粒度取舍同 role,代价可忽略)。
        slot_invalidate_all(&self.scope_cache);
        tracing::info!(
            dept_id = dept_id,
            operator = %current_user.0.username,
            "更新部门成功"
        );
        Ok(())
    }

    /// Java remove(SysDeptController:130-145)的守卫全在此:默认部门 →
    /// 子部门 → 用户 → 岗位,最后 checkDeptDataScope(:143)再软删 ——
    /// Java 守卫在 controller、探针居末;Rust 守卫在 service,探针随置
    /// 守卫之后,错误优先级同 Java(先业务守卫后越权判定)。
    #[tracing::instrument(skip(self, ctx, current_user), fields(operator = %current_user.0.username, dept_id = id))]
    pub async fn delete_by_id(
        &self,
        ctx: &DataContext<'_>,
        id: i64,
        current_user: &CurrentUser,
    ) -> AppResult<()> {
        // dept 删除守卫(对齐 Java SysDeptController:130-142)
        if id == DEFAULT_DEPT_ID {
            return Err(AppError::BadRequest("默认部门不能删除".to_string()));
        }

        let count = self.dept_repo.count_normal_children(ctx, id).await?;
        if count > 0 {
            return Err(AppError::BadRequest(
                "存在正常状态的子部门，不能删除".to_string(),
            ));
        }

        // 含正常用户 → 拒(豁免计数,透传请求 ctx 等价)
        let user_count =
            self.user_repo.count_normal_by_dept_id(ctx, id).await?;
        if user_count > 0 {
            return Err(AppError::BadRequest(
                "部门存在用户，不能删除".to_string(),
            ));
        }

        // 含岗位 → 拒(count_by_dept_id 是 Java 豁免方法,
        // 无 @DataPermission,透传请求 ctx 口径一致)
        let post_count = self.post_repo.count_by_dept_id(ctx, id).await?;
        if post_count > 0 {
            return Err(AppError::BadRequest(
                "部门存在岗位，不能删除".to_string(),
            ));
        }

        // 越权探针(Java remove:143,居守卫之后):
        // 不存在或不在数据范围内 → NotFound,不泄露存在性。
        self.check_dept_data_scope(ctx, Some(id)).await?;

        let mut dept = self
            .dept_repo
            .select_by_id(ctx, &id)
            .await?
            .ok_or_else(|| {
                AppError::NotFound(format!(
                    "删除部门失败: 部门不存在 (id={})",
                    id
                ))
            })?;

        dept.del_flag = Some(DelFlag::Deleted.as_str().to_string());
        // update_*(含软删)由漏斗 update 统一盖章,不再手工 stamp
        self.dept_repo.update_by_id(ctx, &dept, &id).await?;
        // 软删把部门从任何 DeptAndBelow 子树移除(范围条目
        // 内嵌子树,删除即陈旧)→ 写-后-失效,全清。
        slot_invalidate_all(&self.scope_cache);
        tracing::info!(
            dept_id = id,
            operator = %current_user.0.username,
            "删除部门成功"
        );
        Ok(())
    }
}

fn virtual_root_dept() -> Dept {
    Dept {
        dept_id: ROOT_PARENT_ID,
        parent_id: ROOT_PARENT_ID,
        ancestors: Some("0".to_string()),
        dept_name: String::new(),
        dept_category: None,
        order_num: 0,
        leader: None,
        phone: None,
        email: None,
        status: Status::Normal.as_str().to_string(),
        del_flag: Some(DelFlag::Alive.as_str().to_string()),
        base_entity: app_core::model::entity::BaseEntity {
            create_dept: None,
            create_by: None,
            create_time: None,
            update_by: None,
            update_time: None,
        },
    }
}
