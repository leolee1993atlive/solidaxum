//! post 仓储。
//!
//! 漏斗公共契约(执行面/错误)见 `app_data` 模块文档。**方法级豁免裁定(Java 基线逐方法核对)**:
//!
//! | 本文件方法 | Java 锚(SysPostServiceImpl → SysPostMapper) | @DataPermission | 漏斗路径 |
//! |---|---|---|---|
//! | `select_page` | `selectPagePostList` → Mapper:31(带) | ✅ | `ctx.select(..).page()` 自动施加 |
//! | `select_list` | `selectPostList` → `selectVoList`(基方法) | ❌ | `without_row_scope()`(见下偏差注) |
//! | `select_all` | `selectPostAll` → `lambda().voList()` | ❌ | `without_row_scope()` |
//! | `select_by_id` | `selectPostById` → `selectVoById` | ❌ | `without_row_scope()` |
//! | `select_by_post_code` | `checkPostCodeUnique` → `exists()` | ❌ | `without_row_scope()` |
//! | `select_by_post_name` | `checkPostNameUnique` → `exists()` | ❌ | `without_row_scope()` |
//! | `count_by_dept_id` | `countPostByDeptId` → `count()` | ❌ | `without_row_scope()` |
//! | `select_by_user_id` | `selectPostsByUserId` → Mapper:73(无注解) | ❌ | `without_row_scope()` |
//! | `insert`/`update_by_id`/`delete_by_id` | `insertPost`/`updatePost`/`deletePostByIds`(写路径) | ❌ | `ctx.insert`;update/软删走 `ctx.update_without_row_scope`(见下段) |
//!
//! **偏差注(select_list/导出)**:Mapper 里带注解的 `selectPostList(Wrapper)`
//! 在现网 Java 基线**零调用**(死代码);导出走 service `selectPostList` →
//! `selectVoList` 基方法,无注解、无范围改写。按"以现网行为为准"裁定豁免。
//!
//! **写路径不受辖**:Java 写方法无注解(拦截器
//! 只改写查询),且 SysPostController **完全没有**数据范围探针 —— post
//! 的写路径在 Java 侧本就不受辖,本仓照此平价:update/软删统一走
//! `update_without_row_scope`(与 user/role/dept 同口径;user/role/dept
//! 的越权拦截由 handler 的 `check_*_data_scope` 探针承担,post 无探针
//! 可补,Java 平价即为不受辖)。
//!
//! **子树过滤的两侧**:belongDeptId 有双重职责,拆开看 ——
//! - ①数据范围收窄(belongDeptId 子树 IN 当权限使):漏斗的 DeptAndBelow
//!   (CustomOrSelf/Custom)在 resolver 解析期已展开部门树,经 sys_post 的
//!   DataPolicy(dept=DeptId, owner=CreateBy)施加;
//! - ②belongDeptId 的**业务搜索过滤**(Java buildQueryWrapper:93-102,
//!   与 @DataPermission 并存、正交):service 层经 dept 子树展开成
//!   `Condition` 注入 `select_page`/`select_list` —— 漏斗管"能看哪些行",
//!   业务条件管"想搜哪个子树",SQL 里 AND 叠加。

use app_core::{
    model::{entity::BaseEntity, status::DelFlag},
    repo::str_trim_or_empty,
    vo::page::TableDataInfo,
};
use app_data::{
    DataContext, DataError,
    sea_orm::{
        self, ActiveModelBehavior,
        ActiveValue::{NotSet, Set},
        ColumnTrait, Condition, DateTimeUtc, DeriveEntityModel,
        DerivePrimaryKey, DeriveRelation, EntityTrait, EnumIter, JoinType,
        Order, PrimaryKeyTrait, QueryFilter, QueryOrder, QuerySelect, Select,
    },
};

use crate::{
    post::model::post::{Post, PostListVo, PostPageQuery},
    relation::user_post,
};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "sys_post")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub post_id: i64,
    pub dept_id: i64,
    pub post_code: String,
    pub post_category: Option<String>,
    pub post_name: String,
    pub post_sort: i32,
    pub status: String,
    /// 删除标志（0代表存在 1代表删除）。对齐 Java SysPost @TableLogic delFlag。
    pub del_flag: String,
    pub create_dept: Option<i64>,
    pub create_by: Option<i64>,
    pub create_time: Option<DateTimeUtc>,
    pub update_by: Option<i64>,
    pub update_time: Option<DateTimeUtc>,
    pub remark: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

impl From<&Model> for Post {
    fn from(m: &Model) -> Self {
        Post {
            post_id: Some(m.post_id),
            dept_id: m.dept_id,
            post_code: m.post_code.clone(),
            post_category: m.post_category.clone(),
            post_name: m.post_name.clone(),
            post_sort: m.post_sort,
            status: m.status.clone(),
            del_flag: m.del_flag.clone(),
            remark: m.remark.clone(),
            base_entity: BaseEntity {
                create_dept: m.create_dept,
                create_by: m.create_by,
                create_time: m.create_time,
                update_by: m.update_by,
                update_time: m.update_time,
            },
        }
    }
}

/// Post → ActiveModel。审计五列(create_dept/create_by/create_time/
/// update_by/update_time)只搬运领域层已持有的值;**写入时由漏斗统一盖章**
/// (insert 全盖、update 盖 update_*),领域层不再手工盖章。
fn post_to_active_model(post: &Post) -> ActiveModel {
    ActiveModel {
        post_id: match post.post_id {
            Some(id) => Set(id),
            None => NotSet,
        },
        dept_id: Set(post.dept_id),
        post_code: Set(post.post_code.clone()),
        post_category: Set(post.post_category.clone()),
        post_name: Set(post.post_name.clone()),
        post_sort: Set(post.post_sort),
        status: Set(post.status.clone()),
        del_flag: Set(post.del_flag.clone()),
        create_dept: Set(post.base_entity.create_dept),
        create_by: Set(post.base_entity.create_by),
        create_time: Set(post.base_entity.create_time),
        update_by: Set(post.base_entity.update_by),
        update_time: Set(post.base_entity.update_time),
        remark: Set(post.remark.clone()),
    }
}

#[derive(Clone, Default)]
pub struct PostRepository {
    // 无状态仓储:执行面全在调用方传入的 ctx(用户请求
    // 由 handler 请求期从 `infra.data_source()` 构造,那里才带 resolver,
    // 见 app.rs 装配顺序注释;测试经自有 `ds.system_context()` 构造 ctx)。
}

impl PostRepository {
    pub fn new() -> Self {
        Self::default()
    }

    /// 全部岗位(下拉框用)。
    ///
    /// 豁免裁定:Java `SysPostServiceImpl.selectPostAll` → `lambda().voList()`,
    /// 无 @DataPermission → 显式 `without_row_scope()`(全员可见);软删过滤
    /// 照施加(对齐 @TableLogic)。
    pub async fn select_all(
        &self,
        ctx: &DataContext<'_>,
    ) -> Result<Vec<Post>, DataError> {
        let models = ctx
            .select(
                Entity::find()
                    .order_by(Column::PostSort, Order::Asc)
                    .order_by(Column::PostId, Order::Asc),
            )
            .without_row_scope()
            .all()
            .await?;
        Ok(models.iter().map(Post::from).collect())
    }

    /// 按主键查详情。
    ///
    /// 豁免裁定:Java `selectPostById` → `selectVoById`,无 @DataPermission
    /// → `without_row_scope()`;软删过滤照施加。
    pub async fn select_by_id(
        &self,
        ctx: &DataContext<'_>,
        post_id: &i64,
    ) -> Result<Option<Post>, DataError> {
        let model = ctx
            .select(Entity::find_by_id(*post_id))
            .without_row_scope()
            .one()
            .await?;
        Ok(model.as_ref().map(Post::from))
    }

    /// 按岗位编码查(唯一性校验用)。
    ///
    /// 豁免裁定:Java `checkPostCodeUnique` → `exists()`,无
    /// @DataPermission(唯一性校验必须跨部门全量,否则同名岗位在别部门
    /// 会被漏判)→ `without_row_scope()`。
    pub async fn select_by_post_code(
        &self,
        ctx: &DataContext<'_>,
        post_code: &str,
    ) -> Result<Option<Post>, DataError> {
        let model = ctx
            .select(Entity::find().filter(Column::PostCode.eq(post_code)))
            .without_row_scope()
            .one()
            .await?;
        Ok(model.as_ref().map(Post::from))
    }

    /// 按岗位名称查(唯一性校验用)。
    ///
    /// 豁免裁定:Java `checkPostNameUnique` → `exists()`,无
    /// @DataPermission → `without_row_scope()`(理由同上)。
    pub async fn select_by_post_name(
        &self,
        ctx: &DataContext<'_>,
        post_name: &str,
    ) -> Result<Option<Post>, DataError> {
        let model = ctx
            .select(Entity::find().filter(Column::PostName.eq(post_name)))
            .without_row_scope()
            .one()
            .await?;
        Ok(model.as_ref().map(Post::from))
    }

    /// 新增岗位。审计五列由漏斗 insert 统一盖章(principal 取自 ctx),
    /// 领域层传入的审计值会被覆盖。
    pub async fn insert(
        &self,
        ctx: &DataContext<'_>,
        post: &Post,
    ) -> Result<(), DataError> {
        let active = post_to_active_model(post);
        ctx.insert_returning_nothing(active).await?;
        Ok(())
    }

    /// 按主键更新。
    /// **写路径不受行级范围**:Java SysPostMapper 的写
    /// 方法无 @DataPermission,且 SysPostController **完全没有**数据范围
    /// 探针 —— post 的写路径在 Java 侧本就不受辖,本仓照此平价。
    /// WHERE = 主键 + 软删存活,0 行 → NotFound;审计盖 update_*。
    pub async fn update_by_id(
        &self,
        ctx: &DataContext<'_>,
        post: &Post,
        post_id: &i64,
    ) -> Result<(), DataError> {
        let mut active = post_to_active_model(post);
        active.post_id = Set(*post_id);
        ctx.update_without_row_scope(active).await?;
        Ok(())
    }

    /// 软删岗位(del_flag 置 '1',对齐 Java deleteById 的 @TableLogic)。
    /// **写路径不受行级范围**:Java SysPostMapper 的写
    /// 方法无 @DataPermission,且 SysPostController **完全没有**数据范围
    /// 探针 —— post 的写路径在 Java 侧本就不受辖,本仓照此平价。
    /// WHERE = 主键 + 软删存活,0 行 → NotFound;审计盖 update_*。
    pub async fn delete_by_id(
        &self,
        ctx: &DataContext<'_>,
        post_id: &i64,
    ) -> Result<(), DataError> {
        let active = ActiveModel {
            post_id: Set(*post_id),
            del_flag: Set(DelFlag::Deleted.as_str().to_string()),
            ..Default::default()
        };
        ctx.update_without_row_scope(active).await?;
        Ok(())
    }

    /// 岗位列表的过滤条件。**`select_page`(列表)与 `select_list`(导出)共用**。
    ///
    /// 部门过滤两条,互斥(对齐 Java buildQueryWrapper:93-102):
    /// - `dept_id` 精确匹配(单部门搜索,优先);
    /// - `belong_dept_id` 子树 IN —— 由 service 层展开成 `Condition` 注入
    ///   (`belong_subtree` 参数;dept_id 已给定时 service 传 None)。
    fn apply_filters(
        mut select: Select<Entity>,
        query: &PostPageQuery,
        belong_subtree: Option<Condition>,
    ) -> Select<Entity> {
        let post_code = str_trim_or_empty(&query.post_code);
        let post_name = str_trim_or_empty(&query.post_name);
        let post_category = str_trim_or_empty(&query.post_category);
        let status = str_trim_or_empty(&query.status);

        if !post_code.is_empty() {
            select = select.filter(Column::PostCode.contains(&post_code));
        }
        if !post_name.is_empty() {
            select = select.filter(Column::PostName.contains(&post_name));
        }
        if !post_category.is_empty() {
            select =
                select.filter(Column::PostCategory.contains(&post_category));
        }
        // 部门过滤:dept_id 精确匹配(单部门搜索,优先)
        if let Some(dept_id) = query.dept_id {
            select = select.filter(Column::DeptId.eq(dept_id));
        }
        // 部门过滤:belongDeptId 子树 IN(业务搜索条件,service 层展开注入;
        // 与漏斗行级范围正交 —— SQL 里 AND 叠加,见模块头)
        if let Some(cond) = belong_subtree {
            select = select.filter(cond);
        }
        if !status.is_empty() {
            select = select.filter(Column::Status.eq(&status));
        }

        select
    }

    /// 分页查询岗位列表。
    ///
    /// **范围施加点**:Java `selectPagePostList`(SysPostMapper.java:31-37
    /// 带 @DataPermission:deptName=dept_id, userName=create_by)→ 本方法走
    /// `ctx.select(..).page()`,漏斗按 sys_post 的 DataPolicy 自动施加;
    /// 排序在 Select 构件上,逃生舱分页保持 ORDER BY(设计 6.4 裁定)。
    /// `belong_subtree` 是 belongDeptId 子树的业务搜索条件,与范围叠加。
    pub async fn select_page(
        &self,
        ctx: &DataContext<'_>,
        query: &PostPageQuery,
        belong_subtree: Option<Condition>,
        page_no: u64,
        page_size: u64,
        order_by: &str,
    ) -> Result<TableDataInfo<PostListVo>, DataError> {
        let mut select =
            Self::apply_filters(Entity::find(), query, belong_subtree);

        let (order_col, order_dir) = parse_order(order_by);
        select = select.order_by(order_col, order_dir);

        let page = ctx
            .select(select)
            .page(page_no as i64, page_size as i64)
            .await?;

        let rows: Vec<PostListVo> = page
            .rows
            .iter()
            .map(Post::from)
            .map(|p: Post| p.to_list_vo())
            .collect();

        Ok(TableDataInfo::from_page_rows(&page, rows))
    }

    /// 按过滤条件查询全部岗位(不分页,与 select_page 共用同一套 filter)。
    ///
    /// 豁免裁定(导出):**现网 Java 走 service `selectPostList` →
    /// `selectVoList` 基方法,无 @DataPermission**(Mapper 里带注解的
    /// `selectPostList(Wrapper)` 零调用)→ `without_row_scope()`。
    /// `belong_subtree` 是 belongDeptId 子树的业务搜索条件,照常叠加
    /// (Java 里 page 与 export 共用 buildQueryWrapper)。
    pub async fn select_list(
        &self,
        ctx: &DataContext<'_>,
        query: &PostPageQuery,
        belong_subtree: Option<Condition>,
        order_by: &str,
    ) -> Result<Vec<Post>, DataError> {
        let mut select =
            Self::apply_filters(Entity::find(), query, belong_subtree);

        let (order_col, order_dir) = parse_order(order_by);
        select = select.order_by(order_col, order_dir);

        let models = ctx.select(select).without_row_scope().all().await?;
        Ok(models.iter().map(Post::from).collect())
    }

    /// 按部门统计岗位数(dept_service 删除守卫用)。
    ///
    /// 豁免裁定:Java `countPostByDeptId` → `count()`,无 @DataPermission
    /// → `without_row_scope()`(守卫需要真实计数,不能被范围砍小)。
    pub async fn count_by_dept_id(
        &self,
        ctx: &DataContext<'_>,
        dept_id: i64,
    ) -> Result<u64, DataError> {
        ctx.select(Entity::find().filter(Column::DeptId.eq(dept_id)))
            .without_row_scope()
            .count()
            .await
    }

    /// 查询用户关联的岗位列表 (JOIN sys_user_post)
    ///
    /// 豁免裁定:Java `selectPostsByUserId`(SysPostMapper.java:73-79)无
    /// @DataPermission(个人中心的岗位组是"我的岗位"语义)→
    /// `without_row_scope()`;软删过滤由漏斗照施。
    ///
    /// 单跳 INNER JOIN,无显式外键,沿用调用点构造法:
    /// `Entity::belongs_to(..).from(..).to(..).into()` 生成 RelationDef,
    /// JOIN 类型由 `.join(JoinType::InnerJoin, ..)` 显式指定。
    pub async fn select_by_user_id(
        &self,
        ctx: &DataContext<'_>,
        user_id: i64,
    ) -> Result<Vec<Post>, DataError> {
        let select = Entity::find()
            .join(
                JoinType::InnerJoin,
                // post ⟕ user_post:ON post.post_id = user_post.post_id
                Entity::belongs_to(user_post::Entity)
                    .from(Column::PostId)
                    .to(user_post::Column::PostId)
                    .into(),
            )
            .filter(user_post::Column::UserId.eq(user_id))
            .order_by(Column::PostSort, Order::Asc);
        let models = ctx.select(select).without_row_scope().all().await?;
        Ok(models.iter().map(Post::from).collect())
    }
}

fn parse_order(order_by: &str) -> (Column, Order) {
    let mut parts = order_by.split_whitespace();
    let col = match parts.next().unwrap_or("post_id") {
        "post_id" => Column::PostId,
        "post_code" => Column::PostCode,
        "post_sort" => Column::PostSort,
        "create_time" => Column::CreateTime,
        _ => Column::PostId,
    };
    let desc = !parts.next().is_some_and(|d| d.eq_ignore_ascii_case("asc"));
    if desc {
        (col, Order::Desc)
    } else {
        (col, Order::Asc)
    }
}
