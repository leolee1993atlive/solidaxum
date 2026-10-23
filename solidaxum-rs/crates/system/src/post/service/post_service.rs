use app_core::{
    error::{AppError, AppResult},
    infra::Infrastructure,
    middleware::extractors::CurrentUser,
    vo::page::{TableDataInfo, build_order_by, normalize_paging},
};
use app_data::{
    DataContext,
    sea_orm::{ColumnTrait, Condition},
};

use crate::{
    dept::repository::dept_repo::DeptRepository,
    post::{
        model::post::{
            Post, PostCreateBo, PostListVo, PostPageQuery, PostUpdateBo,
        },
        repository::post_repo::{Column as PostColumn, PostRepository},
    },
};

#[derive(Clone)]
pub struct PostService {
    post_repo: PostRepository,
    /// belongDeptId 子树业务搜索用(find_descendant_ids 是 Java 无注解
    /// 方法,without_row_scope,只读部门树无行级范围语义 —— 该过滤是
    /// 查询条件不是权限)。
    dept_repo: DeptRepository,
}

impl PostService {
    /// 装配期不取任何快照:本服务全部方法只
    /// **转发** ctx,用户请求的 ctx 由 handler 请求期从 `infra.data_source()`
    /// 构造(见 app.rs 装配顺序注释)。形参仅保装配调用面形状。
    pub fn new(_infra: &Infrastructure) -> Self {
        Self {
            post_repo: PostRepository::new(),
            dept_repo: DeptRepository::new(),
        }
    }

    /// belongDeptId 子树业务搜索过滤:对齐 Java
    /// `SysPostServiceImpl.buildQueryWrapper:93-102` —— belongDeptId 与
    /// @DataPermission **并存且正交**:漏斗负责数据范围收窄("能看哪些行"),
    /// 这里负责搜索意图("想搜哪个子树"),SQL 里 AND 叠加。
    /// deptId 精确匹配优先(给了 deptId 就忽略 belongDeptId,与 Java 一致)。
    ///
    /// find_descendant_ids 收 ctx,透传调用方的
    /// 请求 ctx —— Java selectDeptAndChildById 无注解(Mapper:76-94),
    /// without_row_scope,ctx 的范围不影响子树展开口径。
    async fn belong_subtree_condition(
        &self,
        ctx: &DataContext<'_>,
        query: &PostPageQuery,
    ) -> AppResult<Option<Condition>> {
        if query.dept_id.is_some() {
            // Java:优先单部门搜索,belongDeptId 被忽略
            return Ok(None);
        }
        match query.belong_dept_id {
            Some(belong_dept_id) => {
                let ids = self
                    .dept_repo
                    .find_descendant_ids(ctx, belong_dept_id)
                    .await?;
                // find_descendant_ids 恒含自身;空集(理论不可达)不施加过滤
                if ids.is_empty() {
                    return Ok(None);
                }
                Ok(Some(Condition::all().add(PostColumn::DeptId.is_in(ids))))
            }
            None => Ok(None),
        }
    }

    #[tracing::instrument(skip(self, ctx))]
    pub async fn list(
        &self,
        ctx: &DataContext<'_>,
    ) -> AppResult<Vec<PostListVo>> {
        let result = self.post_repo.select_all(ctx).await?;
        Ok(result.into_iter().map(|p| p.to_list_vo()).collect())
    }

    #[tracing::instrument(skip(self, ctx), fields(page = query.page.page_num, size = query.page.page_size))]
    pub async fn page(
        &self,
        ctx: &DataContext<'_>,
        query: PostPageQuery,
    ) -> AppResult<TableDataInfo<PostListVo>> {
        let (current, size, _offset) = normalize_paging(&query.page);
        let allowed = [
            ("postId", "post_id"),
            ("postCode", "post_code"),
            ("postSort", "post_sort"),
            ("createTime", "create_time"),
        ];
        let order_by = build_order_by(
            &query.page.order_by_column,
            &query.page.is_asc,
            &allowed,
        )
        .unwrap_or_else(|| "post_sort asc".to_string());

        let belong_subtree = self.belong_subtree_condition(ctx, &query).await?;
        let page_result = self
            .post_repo
            .select_page(
                ctx,
                &query,
                belong_subtree,
                current as u64,
                size as u64,
                &order_by,
            )
            .await?;
        if page_result.total <= 0 {
            return Ok(TableDataInfo::empty());
        }
        tracing::info!(
            current,
            size,
            total = page_result.total,
            "岗位分页查询成功"
        );
        Ok(page_result)
    }

    /// 按查询条件取全部岗位(导出用),与 page 共用同一套 filter/order。
    #[tracing::instrument(skip(self, ctx), fields(page = query.page.page_num, size = query.page.page_size))]
    pub async fn list_all(
        &self,
        ctx: &DataContext<'_>,
        query: PostPageQuery,
    ) -> AppResult<Vec<Post>> {
        let allowed = [
            ("postId", "post_id"),
            ("postCode", "post_code"),
            ("postSort", "post_sort"),
            ("createTime", "create_time"),
        ];
        let order_by = build_order_by(
            &query.page.order_by_column,
            &query.page.is_asc,
            &allowed,
        )
        .unwrap_or_else(|| "post_sort asc".to_string());

        let belong_subtree = self.belong_subtree_condition(ctx, &query).await?;
        let list = self
            .post_repo
            .select_list(ctx, &query, belong_subtree, &order_by)
            .await?;
        Ok(list)
    }

    #[tracing::instrument(skip(self, ctx), fields(post_id = id))]
    pub async fn get_by_id(
        &self,
        ctx: &DataContext<'_>,
        id: i64,
    ) -> AppResult<Post> {
        let post =
            self.post_repo
                .select_by_id(ctx, &id)
                .await?
                .ok_or_else(|| {
                    AppError::NotFound(format!("岗位不存在 (id={})", id))
                })?;
        Ok(post)
    }

    #[tracing::instrument(skip(self, ctx, bo, current_user), fields(operator = %current_user.0.username))]
    pub async fn insert_by_bo(
        &self,
        ctx: &DataContext<'_>,
        bo: PostCreateBo,
        current_user: &CurrentUser,
    ) -> AppResult<()> {
        if self
            .post_repo
            .select_by_post_code(ctx, &bo.post_code)
            .await?
            .is_some()
        {
            return Err(AppError::BadRequest(format!(
                "岗位编码已存在: {}",
                bo.post_code
            )));
        }
        if self
            .post_repo
            .select_by_post_name(ctx, &bo.post_name)
            .await?
            .is_some()
        {
            return Err(AppError::BadRequest(format!(
                "岗位名称已存在: {}",
                bo.post_name
            )));
        }

        let post = Post::from_create_bo(bo)
            .map_err(|e| AppError::Internal(format!("创建岗位失败: {}", e)))?;
        // 审计五列由漏斗 insert 按 ctx 的 principal 盖章
        self.post_repo.insert(ctx, &post).await?;
        tracing::info!(
            post_code = ?post.post_code,
            operator = %current_user.0.username,
            "创建岗位成功"
        );
        Ok(())
    }

    #[tracing::instrument(skip(self, ctx, bo, current_user), fields(operator = %current_user.0.username, post_id = ?bo.post_id))]
    pub async fn update_by_bo(
        &self,
        ctx: &DataContext<'_>,
        bo: PostUpdateBo,
        current_user: &CurrentUser,
    ) -> AppResult<()> {
        let post_id = bo.post_id.ok_or_else(|| {
            AppError::BadRequest("更新岗位失败: 岗位ID为空".to_string())
        })?;
        let mut post = self
            .post_repo
            .select_by_id(ctx, &post_id)
            .await?
            .ok_or_else(|| {
                AppError::NotFound(format!(
                    "更新岗位失败: 岗位不存在 (id={})",
                    post_id
                ))
            })?;

        if let Some(existing) = self
            .post_repo
            .select_by_post_code(ctx, &bo.post_code)
            .await?
            && existing.post_id != Some(post_id)
        {
            return Err(AppError::BadRequest(format!(
                "岗位编码已存在: {}",
                bo.post_code
            )));
        }
        if let Some(existing) = self
            .post_repo
            .select_by_post_name(ctx, &bo.post_name)
            .await?
            && existing.post_id != Some(post_id)
        {
            return Err(AppError::BadRequest(format!(
                "岗位名称已存在: {}",
                bo.post_name
            )));
        }

        post.from_update_bo(bo);
        // update_* 审计由漏斗 update 盖章;已删行 0 行 → NotFound
        // (写路径无行级范围,Java 侧 post 写路径本无探针)
        self.post_repo.update_by_id(ctx, &post, &post_id).await?;
        tracing::info!(
            post_id = post_id,
            operator = %current_user.0.username,
            "更新岗位成功"
        );
        Ok(())
    }

    #[tracing::instrument(skip(self, ctx), fields(post_id = id))]
    pub async fn delete_by_id(
        &self,
        ctx: &DataContext<'_>,
        id: i64,
    ) -> AppResult<()> {
        self.post_repo.delete_by_id(ctx, &id).await?;
        tracing::info!(post_id = id, "删除岗位成功");
        Ok(())
    }

    #[tracing::instrument(skip(self, ctx), fields(dept_id = dept_id))]
    pub async fn count_by_dept(
        &self,
        ctx: &DataContext<'_>,
        dept_id: i64,
    ) -> AppResult<u64> {
        let count = self.post_repo.count_by_dept_id(ctx, dept_id).await?;
        Ok(count)
    }
}
