//! user 仓储。
//!
//! 漏斗公共契约(执行面/错误)见 `app_data` 模块文档。**方法级裁定(Java 基线逐方法核对)**:
//!
//! | 本文件方法 | Java 锚(SysUserServiceImpl → SysUserMapper) | @DataPermission | 漏斗路径 |
//! |---|---|---|---|
//! | `select_page` | `selectPageUserList`(Mapper.java:37-43) | ✅(dept_id/create_by) | `ctx.select(..).page()` 自动施加 |
//! | `select_list`(导出) | `selectUserExportList`(Mapper.java:66-85) | ✅ | `ctx.select` 自动(**与 post 相反**:user 的导出在 Java 带注解) |
//! | `select_all`(变体合并) | `selectUserList(Wrapper)`(Mapper.java:51-57) | ✅ | `ctx.select` 自动 |
//! | `select_option_by_ids_and_dept` | `selectUserByIds` → `selectUserList`(ServiceImpl:175-184) | ✅ | `ctx.select` 自动 |
//! | `select_by_id` | `selectUserById` → `selectVoById`(ServiceImpl:166) | ❌ | `without_row_scope()` |
//! | `select_by_user_name` | `selectUserByUserName` → `lambda().voOne()`(ServiceImpl:143) | ❌ | `without_row_scope()`(登录路径无身份) |
//! | `select_by_phone_number` | `checkPhoneUnique` → `lambda().exists()`(ServiceImpl:239) | ❌ | `without_row_scope()`(唯一性须跨部门全量) |
//! | `select_by_email` | `checkEmailUnique` → `lambda().exists()`(ServiceImpl:253) | ❌ | `without_row_scope()`(同上) |
//! | `count_normal_by_dept_id` | dept 删除守卫的 lambda 计数(无 mapper 注解锚) | ❌ | `without_row_scope()`(守卫需真实计数) |
//! | `insert` | `insertUser` → mapper.insert(写路径) | — | `ctx.insert`(审计五列漏斗盖) |
//! | `update_by_id` | `updateById`(**Mapper.java:158-163 本身带注解**) | ✅ | `ctx.update_without_row_scope`(见表下段) |
//! | `update_by_id_without_row_scope` | Controller:72/:98 `DataPermissionHelper.ignore` → `updateById` | ignore 显式豁免 | `ctx.update_without_row_scope`(与 `update_by_id` 行为等价,保留命名只为 profile 自服务 Java 锚可读,见表下段) |
//! | `update_status` | `updateUserStatus` → `lambda().updateCount()`(ServiceImpl:366) | ❌ | `ctx.update_without_row_scope` 单行(见表下段) |
//! | `select_list_by_dept_id` | `selectUserListByDept` → `lambda().voList()`(ServiceImpl:555) | ❌ | `without_row_scope()`(Java 无注解,对齐放宽) |
//! | `select_page_by_dept_id` | 同上语义的分页变体(Java 无独立锚) | ❌ | `without_row_scope()` |
//! | `insert_user_roles` | `insertUserRole` → userRoleMapper(SysUserRoleMapper 无注解) | ❌ | `ctx.insert_many`(关联表 DataPolicy 全 None) |
//! | `insert_user_posts` | `insertUserPost` → userPostMapper(ServiceImpl:442) | ❌ | `ctx.insert_many`(同上) |
//! | `count_users_by_post_id` | 岗位关联计数(userPostMapper lambda) | ❌ | `ctx.select` + `count()` |
//! | `select_role_ids_by_user_id` | userRoleMapper 的 id 投影查询 | ❌ | `ctx.select`(ORDER BY 在构件上保持) |
//! | `select_post_ids_by_user_id` | userPostMapper 同族 | ❌ | 同上 |
//!
//! **写路径统一不受辖**:全部 update 走
//! `ctx.update_without_row_scope`,越权拦截由 handler 的
//! `check_user_data_scope` 探针承担 —— 对齐 Java 的实际拦截模型
//! (controller 显式探针 + 查询方法带注解,写方法本身不带范围)。
//! `update_by_id_without_row_scope` 因此与 `update_by_id` 行为等价,
//! 保留独立命名只为让 profile 自服务路径的 Java 锚
//! (DataPermissionHelper.ignore)在调用点仍然可读。
//!
//! **变体合并**:`select_all_with_scope(&scope)` 并入 `select_all(ctx)` ——
//! 范围由漏斗按调用方 ctx 施加:handler 传用户 ctx(受辖),system_context
//! 调用方得全量(principal=None → ScopeFilter 恒 All),语义恰好
//! 两全。`apply_user_data_scope`/手工软删过滤随合并删除。
//!
//! **关联表批量删的落点**:`delete_user_roles_by_user_id` /
//! `delete_user_posts_by_user_id` 走漏斗的 `delete_where::<E>(Condition)`
//! 窄原语(仅豁免实体、无软删列,fail-closed 拒受辖)—— 生产路径是
//! user_service 的 `update_user_posts`/`insert_user_auth` 在
//! `ctx.transaction` 闭包内 delete-then-insert。插入侧 `ctx.insert_many`:
//! 关联表 DataPolicy 全 None → 无审计盖章、无范围语义。
//!
//! **写路径审计移交**:user_to_active_model 只搬运领域层已持有的审计值,
//! 写入时由漏斗统一盖章(insert 全盖、update 盖 update_*),领域层/服务层
//! 不再手工盖章(user_id 雪花仍是应用侧生成,不经漏斗)。

use app_core::{
    model::{
        entity::BaseEntity,
        status::{DelFlag, Status},
    },
    repo::str_trim_or_empty,
    vo::page::TableDataInfo,
};
use app_data::{
    DataContext, DataError, TxnContext,
    sea_orm::{
        self, ActiveModelBehavior,
        ActiveValue::{NotSet, Set},
        ColumnTrait, DateTimeUtc, DeriveEntityModel, DerivePrimaryKey,
        DeriveRelation, EntityTrait, EnumIter, Order, PrimaryKeyTrait,
        QueryFilter, QueryOrder, Select,
    },
};

use crate::{
    relation::{user_post, user_role},
    user::{
        handler::user_handler::UserPageQuery,
        model::user::{User, UserListVo},
    },
};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "sys_user")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub user_id: i64,
    pub dept_id: Option<i64>,
    pub user_name: String,
    pub nick_name: String,
    pub user_type: Option<String>,
    pub email: Option<String>,
    #[sea_orm(column_name = "phone_number")]
    pub phone_number: Option<String>,
    pub gender: Option<String>,
    pub avatar: Option<i64>,
    pub password: String,
    pub status: String,
    pub del_flag: String,
    pub login_ip: Option<String>,
    pub login_date: Option<DateTimeUtc>,
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

impl From<&Model> for User {
    fn from(m: &Model) -> Self {
        User {
            user_id: Some(m.user_id),
            dept_id: m.dept_id,
            user_name: m.user_name.clone(),
            nick_name: m.nick_name.clone(),
            password: m.password.clone(),
            user_type: m.user_type.clone(),
            email: m.email.clone(),
            phone_number: m.phone_number.clone(),
            gender: m.gender.clone(),
            avatar: m.avatar,
            status: m.status.clone(),
            del_flag: Some(m.del_flag.clone()),
            login_ip: m.login_ip.clone(),
            login_date: m.login_date,
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

/// User → ActiveModel。审计五列(create_dept/create_by/create_time/
/// update_by/update_time)只搬运领域层已持有的值;**写入时由漏斗统一盖章**
/// (insert 全盖、update 盖 update_*),领域层不再手工盖章 —— 服务层传入的
/// 审计值会被漏斗覆盖(principal 取自 ctx)。
fn user_to_active_model(user: &User) -> ActiveModel {
    ActiveModel {
        user_id: match user.user_id {
            Some(id) => Set(id),
            None => NotSet,
        },
        dept_id: Set(user.dept_id),
        user_name: Set(user.user_name.clone()),
        nick_name: Set(user.nick_name.clone()),
        user_type: Set(user.user_type.clone()),
        email: Set(user.email.clone()),
        phone_number: Set(user.phone_number.clone()),
        gender: Set(user.gender.clone()),
        avatar: Set(user.avatar),
        password: Set(user.password.clone()),
        status: Set(user.status.clone()),
        del_flag: Set(user
            .del_flag
            .clone()
            .unwrap_or_else(|| DelFlag::Alive.as_str().to_string())),
        login_ip: Set(user.login_ip.clone()),
        login_date: Set(user.login_date),
        create_dept: Set(user.base_entity.create_dept),
        create_by: Set(user.base_entity.create_by),
        create_time: Set(user.base_entity.create_time),
        update_by: Set(user.base_entity.update_by),
        update_time: Set(user.base_entity.update_time),
        remark: Set(user.remark.clone()),
    }
}

/// user 列表/导出共用的筛选链。抽出来是为了让 select_page 与 select_list
/// 不会漂移(导出漏一个筛选条件 = 导出的内容和屏幕上看到的不一样)。
/// 部门/软删/行级范围不在其中 —— 那三样由漏斗施加(dept_id 精确搜索与
/// 行级范围在 SQL 里 AND 叠加,正交)。
fn apply_user_filters(
    mut select: Select<Entity>,
    query: &UserPageQuery,
) -> Select<Entity> {
    let user_name = str_trim_or_empty(&query.user_name);
    let nick_name = str_trim_or_empty(&query.nick_name);
    let status = str_trim_or_empty(&query.status);

    if !user_name.is_empty() {
        select = select.filter(Column::UserName.contains(&user_name));
    }
    if !nick_name.is_empty() {
        select = select.filter(Column::NickName.contains(&nick_name));
    }
    if !status.is_empty() {
        select = select.filter(Column::Status.eq(&status));
    }
    if let Some(ref phone) = query.phone_number {
        let phone = phone.trim();
        if !phone.is_empty() {
            select = select.filter(Column::PhoneNumber.contains(phone));
        }
    }
    if let Some(dept_id) = query.dept_id {
        select = select.filter(Column::DeptId.eq(dept_id));
    }
    if let Some(ref begin) = query.begin_time {
        let begin = begin.trim();
        if !begin.is_empty()
            && let Ok(dt) = chrono::NaiveDateTime::parse_from_str(
                begin,
                "%Y-%m-%d %H:%M:%S",
            )
        {
            select = select.filter(Column::CreateTime.gte(dt.and_utc()));
        }
    }
    if let Some(ref end) = query.end_time {
        let end = end.trim();
        if !end.is_empty()
            && let Ok(dt) =
                chrono::NaiveDateTime::parse_from_str(end, "%Y-%m-%d %H:%M:%S")
        {
            select = select.filter(Column::CreateTime.lte(dt.and_utc()));
        }
    }
    select
}

#[derive(Clone, Default)]
pub struct UserRepository {
    // 无状态仓储:执行面全在调用方传入的 ctx(用户请求
    // 由 handler 请求期从 `infra.data_source()` 构造,那里才带 resolver,
    // 见 app.rs 装配顺序注释;测试经自有 `ds.system_context()` 构造 ctx)。
}

impl UserRepository {
    pub fn new() -> Self {
        Self::default()
    }

    /// 查询用户列表(变体合并:原 `select_all_with_scope(&scope)` 并入)。
    ///
    /// 范围施加点:Java `SysUserServiceImpl` 经 `selectUserList(Wrapper)`
    /// (SysUserMapper.java:51-57 带 @DataPermission:deptName=dept_id,
    /// userName=create_by)→ 本方法走 `ctx.select`,漏斗按 sys_user 的
    /// DataPolicy 自动施加;system_context 调用方得全量。
    pub async fn select_all(
        &self,
        ctx: &DataContext<'_>,
    ) -> Result<Vec<User>, DataError> {
        let models = ctx.select(Entity::find()).all().await?;
        Ok(models.iter().map(User::from).collect())
    }

    /// 用户下拉选项查询(对齐 Java `selectUserByIds`:status=0 + 可选
    /// dept/userIds)。
    ///
    /// 范围施加点:Java `selectUserByIds`(ServiceImpl:175-184)走的是
    /// **带注解的** `selectUserList` → `ctx.select` 自动施加(与 post 的
    /// 下拉豁免相反,user 的下拉在 Java 是受辖的)。
    pub async fn select_option_by_ids_and_dept(
        &self,
        ctx: &DataContext<'_>,
        user_ids: Option<&[i64]>,
        dept_id: Option<i64>,
    ) -> Result<Vec<User>, DataError> {
        let mut select =
            Entity::find().filter(Column::Status.eq(Status::Normal.as_str()));
        if let Some(dept_id) = dept_id {
            select = select.filter(Column::DeptId.eq(dept_id));
        }
        if let Some(ids) = user_ids.filter(|ids| !ids.is_empty()) {
            select = select.filter(Column::UserId.is_in(ids.to_vec()));
        }
        let models = ctx.select(select).all().await?;
        Ok(models.iter().map(User::from).collect())
    }

    /// 按主键查详情。
    ///
    /// 豁免裁定:Java `selectUserById` → `selectVoById`(ServiceImpl:166),
    /// 无 @DataPermission → `without_row_scope()`;软删过滤照施加。
    pub async fn select_by_id(
        &self,
        ctx: &DataContext<'_>,
        user_id: &i64,
    ) -> Result<Option<User>, DataError> {
        let model = ctx
            .select(Entity::find_by_id(*user_id))
            .without_row_scope()
            .one()
            .await?;
        Ok(model.as_ref().map(User::from))
    }

    /// 按用户名查(登录路径/唯一性校验用)。
    ///
    /// 豁免裁定:Java `selectUserByUserName` → `lambda().voOne()`
    /// (ServiceImpl:143),无 @DataPermission → `without_row_scope()`。
    /// 登录路径没有请求身份,以 system_context 调用,
    /// 本方法只按传入 ctx 执行。
    pub async fn select_by_user_name(
        &self,
        ctx: &DataContext<'_>,
        user_name: &str,
    ) -> Result<Option<User>, DataError> {
        let model = ctx
            .select(Entity::find().filter(Column::UserName.eq(user_name)))
            .without_row_scope()
            .one()
            .await?;
        Ok(model.as_ref().map(User::from))
    }

    /// 统计部门下正常状态的用户数量(用于 dept 删除/停用守卫)。
    ///
    /// 豁免裁定:守卫需要真实计数(Java 侧 lambda 计数,无 @DataPermission
    /// 锚)→ `without_row_scope()`;软删过滤照施加。
    pub async fn count_normal_by_dept_id(
        &self,
        ctx: &DataContext<'_>,
        dept_id: i64,
    ) -> Result<u64, DataError> {
        ctx.select(
            Entity::find()
                .filter(Column::DeptId.eq(dept_id))
                .filter(Column::Status.eq(Status::Normal.as_str())),
        )
        .without_row_scope()
        .count()
        .await
    }

    /// 按手机号查询(排除指定 id 用于唯一性校验)。`exclude_id` 为 None
    /// 时不过滤。
    ///
    /// 豁免裁定:Java `checkPhoneUnique` → `lambda().exists()`
    /// (ServiceImpl:239-248),无 @DataPermission(唯一性校验必须跨部门
    /// 全量,否则同手机号在别部门会被漏判)→ `without_row_scope()`。
    pub async fn exists_by_phone_number(
        &self,
        ctx: &DataContext<'_>,
        phone_number: &str,
        exclude_id: Option<i64>,
    ) -> Result<bool, DataError> {
        let mut q = Entity::find().filter(Column::PhoneNumber.eq(phone_number));
        if let Some(uid) = exclude_id {
            q = q.filter(Column::UserId.ne(uid));
        }
        Ok(ctx.select(q).without_row_scope().count().await? > 0)
    }

    /// 按邮箱查询(排除指定 id 用于唯一性校验)。
    ///
    /// 豁免裁定:Java `checkEmailUnique` → `lambda().exists()`
    /// (ServiceImpl:253-262),无 @DataPermission → `without_row_scope()`
    /// (理由同上)。
    pub async fn exists_by_email(
        &self,
        ctx: &DataContext<'_>,
        email: &str,
        exclude_id: Option<i64>,
    ) -> Result<bool, DataError> {
        let mut q = Entity::find().filter(Column::Email.eq(email));
        if let Some(uid) = exclude_id {
            q = q.filter(Column::UserId.ne(uid));
        }
        Ok(ctx.select(q).without_row_scope().count().await? > 0)
    }

    /// 按用户名判存在(排除指定 id)。`select_by_user_name` 保留给登录与导入
    /// —— 那两条路真要整行。
    pub async fn exists_by_user_name(
        &self,
        ctx: &DataContext<'_>,
        user_name: &str,
        exclude_id: Option<i64>,
    ) -> Result<bool, DataError> {
        let mut q = Entity::find().filter(Column::UserName.eq(user_name));
        if let Some(uid) = exclude_id {
            q = q.filter(Column::UserId.ne(uid));
        }
        Ok(ctx.select(q).without_row_scope().count().await? > 0)
    }

    /// 新增用户。审计五列由漏斗 insert 统一盖章(principal 取自 ctx;
    /// user_id 雪花保持应用侧生成),领域层传入的审计值会被覆盖。
    /// 用户请求路径审计人 = 操作人;登录/注册等无身份路径
    /// 与跨模块调用方传 system_context,审计人 = 0(系统)。
    pub async fn insert(
        &self,
        ctx: &DataContext<'_>,
        user: &User,
    ) -> Result<(), DataError> {
        let active = user_to_active_model(user);
        ctx.insert_returning_nothing(active).await?;
        Ok(())
    }

    /// [`Self::insert`] 的事务内变体(insert_by_bo 外层事务闭包内经
    /// [`UserService`](crate::user::service::user_service) 调用;盖章语义
    /// 与连接路径一致 —— TxnContext 由 `ctx.transaction` 构造,携带同一
    /// principal,审计五列照常由漏斗盖)。
    pub async fn insert_in_txn(
        &self,
        tx: &TxnContext<'_>,
        user: &User,
    ) -> Result<(), DataError> {
        let active = user_to_active_model(user);
        tx.insert_returning_nothing(active).await?;
        Ok(())
    }

    /// 按主键更新。
    ///
    /// **写路径不受行级范围**:Java `SysUserMapper.updateById`
    /// 虽带 @DataPermission(Mapper.java:158-163),但那正是 Java 版踩过不良
    /// 事件的地方 —— 受辖 update 会让「改得动/改不动」取决于数据范围的边界
    /// 抖动,失败态还是静默 0 行。本仓统一走写侧显式豁免,越权拦截交给
    /// Java 对位的 controller 探针 `checkUserDataScope`
    /// (SysUserController.edit:201 / resetPwd:256 / changeStatus:273)。
    /// WHERE = 主键 + 软删存活;已删行 → NotFound。审计盖 update_*
    /// (create_* 搬运领域层原值,即更新不冲掉建单人)。
    pub async fn update_by_id(
        &self,
        ctx: &DataContext<'_>,
        user: &User,
        user_id: &i64,
    ) -> Result<(), DataError> {
        let mut active = user_to_active_model(user);
        active.user_id = Set(*user_id);
        ctx.update_without_row_scope(active).await?;
        Ok(())
    }

    /// [`Self::update_by_id`] 的事务内变体(update_by_bo 外层事务闭包内
    /// 使用;WHERE 语义与盖章行为同连接路径)。
    pub async fn update_by_id_in_txn(
        &self,
        tx: &TxnContext<'_>,
        user: &User,
        user_id: &i64,
    ) -> Result<(), DataError> {
        let mut active = user_to_active_model(user);
        active.user_id = Set(*user_id);
        tx.update_without_row_scope(active).await?;
        Ok(())
    }

    /// 按主键更新(免行级范围)—— profile 自服务路径专用。
    ///
    /// Java 锚:SysProfileController:72(updateUserProfile)/:98(updateUserPwd)
    /// 都把更新包在 `DataPermissionHelper.ignore` 里 —— 自服务写路径的
    /// "本人行"语义由业务 WHERE(user_id = 本人,即主键)表达,不经数据
    /// 范围。对应漏斗的写侧显式豁免原语 `ctx.update_without_row_scope`
    /// (设计 7.4):软删存活、审计盖章(update_* = 操作人)、
    /// 空 SET 守卫、0 行 → NotFound 与 `update_by_id` 完全一致,仅跳过
    /// 行级范围一项。其余写路径不得借用本方法 —— 豁免必须像 Java 的
    /// ignore 一样在调用点写出来。
    pub async fn update_by_id_without_row_scope(
        &self,
        ctx: &DataContext<'_>,
        user: &User,
        user_id: &i64,
    ) -> Result<(), DataError> {
        let mut active = user_to_active_model(user);
        active.user_id = Set(*user_id);
        ctx.update_without_row_scope(active).await?;
        Ok(())
    }

    /// 插入用户-角色关联(原 INSERT INTO sys_user_role)。
    ///
    /// 裁定:关联表 DataPolicy 全 None(CONSTRAINED=false,无软删/审计)
    /// → `ctx.insert_many`;空列表 no-op 保留。
    pub async fn insert_user_roles(
        &self,
        ctx: &DataContext<'_>,
        user_id: i64,
        role_ids: &[i64],
    ) -> Result<(), DataError> {
        if role_ids.is_empty() {
            return Ok(());
        }
        ctx.insert_many(
            role_ids
                .iter()
                .map(|&rid| user_role::ActiveModel {
                    user_id: Set(user_id),
                    role_id: Set(rid),
                })
                .collect(),
        )
        .await?;
        Ok(())
    }

    /// 插入用户-岗位关联(原 INSERT INTO sys_user_post)。裁定同
    /// [`Self::insert_user_roles`]。
    pub async fn insert_user_posts(
        &self,
        ctx: &DataContext<'_>,
        user_id: i64,
        post_ids: &[i64],
    ) -> Result<(), DataError> {
        if post_ids.is_empty() {
            return Ok(());
        }
        ctx.insert_many(
            post_ids
                .iter()
                .map(|&pid| user_post::ActiveModel {
                    user_id: Set(user_id),
                    post_id: Set(pid),
                })
                .collect(),
        )
        .await?;
        Ok(())
    }

    /// 统计某岗位关联的用户数(原 SELECT COUNT(*) FROM sys_user_post)。
    ///
    /// 裁定:关联表读(全 None 豁免)→ `without_row_scope`。
    pub async fn count_users_by_post_id(
        &self,
        ctx: &DataContext<'_>,
        post_id: i64,
    ) -> Result<u64, DataError> {
        ctx.select(
            user_post::Entity::find()
                .filter(user_post::Column::PostId.eq(post_id)),
        )
        .without_row_scope()
        .count()
        .await
    }

    /// 越权可见性探针:对齐 Java
    /// `checkUserDataScope` → `countUserById`(SysUserMapper.java:130-134,
    /// **带 @DataPermission**:dept_id/create_by + @TableLogic)。
    /// 走受辖 `ctx.select`(范围 + 软删过滤都施加),count==0 ⟺ 目标行
    /// 不存在**或**不在调用方数据范围内 —— 读路径(getInfo/authRole)与
    /// 写路径(edit/resetPwd/changeStatus)的越权判定都由它承担。
    pub async fn count_user_by_id(
        &self,
        ctx: &DataContext<'_>,
        user_id: i64,
    ) -> Result<u64, DataError> {
        ctx.select(Entity::find_by_id(user_id)).count().await
    }

    /// 查询用户关联的角色 ID 列表(原 SELECT role_id FROM sys_user_role
    /// WHERE user_id = ? ORDER BY role_id ASC)。
    ///
    /// 裁定:关联表读(全 None 豁免)→ `without_row_scope`;投影查询
    /// (select_only + into_tuple)不在漏斗面,读全模型后取 role_id,
    /// ORDER BY 在 Select 构件上保持升序语义。
    pub async fn select_role_ids_by_user_id(
        &self,
        ctx: &DataContext<'_>,
        user_id: i64,
    ) -> Result<Vec<i64>, DataError> {
        let models = ctx
            .select(
                user_role::Entity::find()
                    .filter(user_role::Column::UserId.eq(user_id))
                    .order_by_asc(user_role::Column::RoleId),
            )
            .without_row_scope()
            .all()
            .await?;
        Ok(models.into_iter().map(|m| m.role_id).collect())
    }

    /// 查询用户关联的岗位 ID 列表(原 SELECT post_id FROM sys_user_post
    /// WHERE user_id = ? ORDER BY post_id ASC)。裁定同
    /// [`Self::select_role_ids_by_user_id`]。
    pub async fn select_post_ids_by_user_id(
        &self,
        ctx: &DataContext<'_>,
        user_id: i64,
    ) -> Result<Vec<i64>, DataError> {
        let models = ctx
            .select(
                user_post::Entity::find()
                    .filter(user_post::Column::UserId.eq(user_id))
                    .order_by_asc(user_post::Column::PostId),
            )
            .without_row_scope()
            .all()
            .await?;
        Ok(models.into_iter().map(|m| m.post_id).collect())
    }

    /// 更新用户状态。
    ///
    /// Java `updateUserStatus`(ServiceImpl:366-372)无 @DataPermission;
    /// 越权拦截在 controller 的 `checkUserDataScope`(SysUserController
    /// .changeStatus:273)。**写路径不受行级范围**(同
    /// `update_by_id` 的说明)。WHERE = 主键 + 软删存活,0 行 → NotFound
    /// (原「0 行 → RecordNotFound」语义保留);审计盖 update_*。
    pub async fn update_status(
        &self,
        ctx: &DataContext<'_>,
        user_id: i64,
        status: &str,
    ) -> Result<(), DataError> {
        let active = ActiveModel {
            user_id: Set(user_id),
            status: Set(status.to_string()),
            ..Default::default()
        };
        ctx.update_without_row_scope(active).await?;
        Ok(())
    }

    /// 按部门 ID 查询用户列表(非分页)。
    ///
    /// 豁免裁定:Java `selectUserListByDept` → `lambda().voList()`
    /// (ServiceImpl:555-560)无 @DataPermission → `without_row_scope()`
    /// (Java 基线无注解,行为对齐放宽)。
    pub async fn select_list_by_dept_id(
        &self,
        ctx: &DataContext<'_>,
        dept_id: i64,
    ) -> Result<Vec<UserListVo>, DataError> {
        let models = ctx
            .select(
                Entity::find()
                    .filter(Column::DeptId.eq(dept_id))
                    .order_by_desc(Column::UserId),
            )
            .without_row_scope()
            .all()
            .await?;

        Ok(models
            .iter()
            .map(User::from)
            .map(|u: User| u.to_list_vo())
            .collect())
    }

    /// 按部门 ID 分页查询用户。裁定同 [`Self::select_list_by_dept_id`]
    /// (Java 无独立分页锚,同走 lambda 语义)→ `without_row_scope()`;
    /// 排序在 Select 构件上,逃生舱分页保持 ORDER BY(设计 6.4 裁定)。
    pub async fn select_page_by_dept_id(
        &self,
        ctx: &DataContext<'_>,
        dept_id: i64,
        page_no: u64,
        page_size: u64,
    ) -> Result<TableDataInfo<UserListVo>, DataError> {
        let select = Entity::find()
            .filter(Column::DeptId.eq(dept_id))
            .order_by(Column::UserId, Order::Desc);

        let page = ctx
            .select(select)
            .without_row_scope()
            .page(page_no as i64, page_size as i64)
            .await?;

        let rows: Vec<UserListVo> = page
            .rows
            .iter()
            .map(User::from)
            .map(|u: User| u.to_list_vo())
            .collect();

        Ok(TableDataInfo::from_page_rows(&page, rows))
    }

    /// 分页查询用户列表。
    ///
    /// 范围施加点:Java `selectPageUserList`(SysUserMapper.java:37-43 带
    /// @DataPermission:deptName=dept_id,userName=create_by)→ 本方法走
    /// `ctx.select(..).page()`,漏斗按 sys_user 的 DataPolicy 自动施加;
    /// 排序在 Select 构件上,逃生舱分页保持 ORDER BY(设计 6.4 裁定)。
    pub async fn select_page(
        &self,
        ctx: &DataContext<'_>,
        query: &UserPageQuery,
        page_no: u64,
        page_size: u64,
        order_by: &str,
    ) -> Result<TableDataInfo<UserListVo>, DataError> {
        let mut select = apply_user_filters(Entity::find(), query);
        let (order_col, order_dir) = parse_order(order_by);
        select = select.order_by(order_col, order_dir);

        let page = ctx
            .select(select)
            .page(page_no as i64, page_size as i64)
            .await?;

        let rows: Vec<UserListVo> = page
            .rows
            .iter()
            .map(User::from)
            .map(|u: User| u.to_list_vo())
            .collect();

        Ok(TableDataInfo::from_page_rows(&page, rows))
    }

    /// 按查询条件取全部用户(导出用),与 select_page 共用同一套 filter/order。
    ///
    /// 范围施加点:Java 导出 `selectUserExportList`(SysUserMapper.java:66-85
    /// 带 @DataPermission,join 限定 d.dept_id/u.create_by)→ `ctx.select`
    /// 自动施加。**与 post 的导出豁免相反** —— user 的导出在 Java 基线
    /// 就是受辖的(见模块头审计表)。
    pub async fn select_list(
        &self,
        ctx: &DataContext<'_>,
        query: &UserPageQuery,
        order_by: &str,
    ) -> Result<Vec<UserListVo>, DataError> {
        let mut select = apply_user_filters(Entity::find(), query);
        let (order_col, order_dir) = parse_order(order_by);
        select = select.order_by(order_col, order_dir);

        let models = ctx.select(select).all().await?;
        Ok(models
            .iter()
            .map(User::from)
            .map(|u: User| u.to_list_vo())
            .collect())
    }

    /// 与 [`Self::select_list`] 同过滤、同受辖面的计数,供导出前的行数闸门
    /// 使用。复用 `apply_user_filters` 并同样走 `ctx.select`(不加
    /// `without_row_scope`)—— 计数必须与随后要取的行**同范围**,否则
    /// 受限管理员会被按全量行数误拒。
    pub async fn count_by_query(
        &self,
        ctx: &DataContext<'_>,
        query: &UserPageQuery,
    ) -> Result<u64, DataError> {
        ctx.select(apply_user_filters(Entity::find(), query))
            .count()
            .await
    }
}

fn parse_order(order_by: &str) -> (Column, Order) {
    let mut parts = order_by.split_whitespace();
    let col = match parts.next().unwrap_or("user_id") {
        "user_id" => Column::UserId,
        "user_name" => Column::UserName,
        "nick_name" => Column::NickName,
        "create_time" => Column::CreateTime,
        _ => Column::UserId,
    };
    let desc = !parts.next().is_some_and(|d| d.eq_ignore_ascii_case("asc"));
    if desc {
        (col, Order::Desc)
    } else {
        (col, Order::Asc)
    }
}
