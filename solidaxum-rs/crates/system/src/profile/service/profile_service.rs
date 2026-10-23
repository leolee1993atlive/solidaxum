//! profile 服务(无独立 repo,复用 user/post/dept 仓储)。
//!
//! **方法级 Java 审计表**(源:`SysProfileController` /
//! `SysUserServiceImpl` / `SysPostMapper`;个人中心是自服务路径,
//! 读自己、改自己):
//!
//! | 本文件路径 | Java 锚 | @DataPermission | 处置 |
//! |---|---|---|---|
//! | `get_profile` → `user_repo.select_by_id` | Controller:44 `selectUserById` → ServiceImpl:166 `selectVoById` | ❌ | 豁免(user_repo `without_row_scope`) |
//! | `get_profile` → `role_repo.select_roles_by_user_id` | ServiceImpl:198 `selectUserRoleGroup` → roleMapper join(Mapper:89 无注解) | ❌ | 豁免(role_repo `without_row_scope`;ctx 只承载软删过滤) |
//! | `get_profile` → `post_repo.select_by_user_id` | ServiceImpl:213 `selectUserPostGroup` → SysPostMapper:73 join | ❌ | 豁免(post_repo 已裁) |
//! | `get_profile` → `dept_repo.select_by_id` | 部门名翻译(Java ProfileUserVo 经 @Translation 按 deptId 查) | ❌ | 豁免(dept_repo 已裁) |
//! | `update_profile` 唯一性 | Controller:66/:69 `checkPhoneUnique`/`checkEmailUnique`(ServiceImpl:239/:253) | ❌ | 豁免(唯一性须跨部门全量,user_repo 已裁) |
//! | `update_profile` 更新 | Controller:72 `DataPermissionHelper.ignore` → ServiceImpl:386 lambda 按 PK | ignore 显式豁免 | `update_by_id_without_row_scope`(对齐 ignore;软删/审计/0 行 NotFound 照旧,设计 7.4 修正) |
//! | `update_password` 读 | Controller:90 `selectUserById` | ❌ | 豁免(同上) |
//! | `update_password` 更新 | Controller:98 `DataPermissionHelper.ignore` → ServiceImpl:405 lambda 按 PK | ignore 显式豁免 | 同 `update_profile` |
//!
//! **读路径全豁免的语义根据**:个人中心查的是"自己的行"——经显式
//! user_id 条件定位,不是行级范围要回答的"哪些行可见"问题;Java 侧
//! 这些方法也均无注解,两者一致。

use app_core::{
    error::{AppError, AppResult},
    infra::Infrastructure,
    util::crypto_util::hash_password,
};
use app_data::DataContext;

use crate::{
    dept::repository::dept_repo::DeptRepository,
    post::repository::post_repo::PostRepository,
    role::repository::role_repo::RoleRepository,
    translation::service::translation_service::TranslationService,
    user::{
        model::user::{ProfileUserVo, ProfileVo, SysUserProfileBo},
        repository::user_repo::UserRepository,
    },
};

#[derive(Clone)]
pub struct ProfileService {
    user_repo: UserRepository,
    role_repo: RoleRepository,
    post_repo: PostRepository,
    dept_repo: DeptRepository,
    translation_service: TranslationService,
}

impl ProfileService {
    /// 装配期只取 DataSource 快照(未带 resolver 亦可):本服务全部方法只
    /// **转发** ctx,用户请求的 ctx 由 handler 请求期从 `infra.data_source()`
    /// 构造(见 app.rs 装配顺序注释)。
    pub fn new(infra: &Infrastructure) -> Self {
        Self {
            user_repo: UserRepository::new(),
            role_repo: RoleRepository::new(),
            post_repo: PostRepository::new(),
            dept_repo: DeptRepository::new(),
            translation_service: TranslationService::new(infra),
        }
    }

    /// 获取个人中心信息(读路径全豁免,Java 锚见模块头审计表)。
    pub async fn get_profile(
        &self,
        ctx: &DataContext<'_>,
        user_id: i64,
    ) -> AppResult<ProfileVo> {
        // Java selectVoById 无注解(豁免),ctx 只承载软删过滤。
        let user = self
            .user_repo
            .select_by_id(ctx, &user_id)
            .await?
            .ok_or_else(|| {
                AppError::NotFound(format!("用户不存在 (id={})", user_id))
            })?;

        // 角色: role_group = "角色名1,角色名2"
        // select_roles_by_user_id 对齐 Java 豁免方法 selectRolesByUserId
        // (SysRoleMapper:89,join 无注解),ctx 只承载软删过滤。
        let roles =
            self.role_repo.select_roles_by_user_id(ctx, user_id).await?;
        let role_group = roles
            .iter()
            .map(|r| r.role_name.clone())
            .collect::<Vec<_>>()
            .join(",");

        // 岗位: post_group = "岗位名1,岗位名2"
        // select_by_user_id 对齐 Java 豁免方法 selectPostsByUserId
        // (SysPostMapper:73,join 无注解),ctx 只承载软删过滤。
        let posts = self.post_repo.select_by_user_id(ctx, user_id).await?;
        let post_group = posts
            .iter()
            .map(|p| p.post_name.clone())
            .collect::<Vec<_>>()
            .join(",");

        // 部门名称(Java 侧经 @Translation 按 deptId 查,无注解路径)
        let dept_name = if let Some(dept_id) = user.dept_id {
            self.dept_repo
                .select_by_id(ctx, &dept_id)
                .await?
                .map(|d| d.dept_name)
        } else {
            None
        };

        // 头像 URL(翻译读豁免,
        // ctx 只承载连接 —— @Translation(OSS_ID_TO_URL) 无注解路径)
        let avatar_url = self
            .translation_service
            .oss_id_to_url_single(ctx, user.avatar)
            .await?;

        let profile_user = ProfileUserVo {
            user_id: user.user_id.unwrap_or_default(),
            dept_id: user.dept_id,
            user_name: user.user_name.clone(),
            nick_name: user.nick_name.clone(),
            user_type: user.user_type.clone(),
            email: user.email.clone(),
            phone_number: user.phone_number.clone(),
            gender: user.gender.clone(),
            avatar: user.avatar,
            avatar_url,
            login_ip: user.login_ip.clone(),
            login_date: user.login_date,
            create_time: user.base_entity.create_time,
            dept_name,
        };

        Ok(ProfileVo {
            user: profile_user,
            role_group,
            post_group,
        })
    }

    /// 更新个人中心信息(审计人 = 操作人本人)。
    ///
    /// Java 基线:Controller:72 把 `updateUserProfile`(ServiceImpl:386,
    /// lambda 按 user_id PK 更新)包在 `DataPermissionHelper.ignore` 里 ——
    /// 自服务写路径不经数据范围,"本人行"语义由业务 WHERE(user_id = 本人,
    /// 即主键)表达。本侧对齐走漏斗的写侧显式豁免原语
    /// `update_by_id_without_row_scope`(设计 7.4):SelfOnly(含无任何
    /// 角色,combine_scopes 空集 → SelfOnly)操作者改自己资料不再因
    /// `create_by ≠ 本人`(建单人是管理员)0 行 → NotFound,Java 经 ignore
    /// 恒成功的行为平价恢复。软删存活、审计盖章、已删用户 0 行 → NotFound
    /// 照旧 —— Java 的 ignore 不影响 @TableLogic,两者只豁免范围一项。
    ///
    /// 审计:Java 的 lambda 更新无实体,MyBatis-Plus 不回填 update_by
    /// (Java 侧本就保留旧值);本侧盖为操作人 ——
    /// 漏斗 update 按 ctx principal 盖 `update_by = login_user.user_id`。
    pub async fn update_profile(
        &self,
        ctx: &DataContext<'_>,
        user_id: i64,
        bo: SysUserProfileBo,
    ) -> AppResult<()> {
        // 详情/唯一性在 Java 侧均无注解(豁免),ctx 只承载软删过滤。
        let mut user = self
            .user_repo
            .select_by_id(ctx, &user_id)
            .await?
            .ok_or_else(|| AppError::NotFound("用户不存在".to_string()))?;

        // 手机/邮箱唯一性(排除当前用户)
        if let Some(ref phone) = bo.phone_number
            && !phone.trim().is_empty()
            && self
                .user_repo
                .exists_by_phone_number(ctx, phone, Some(user_id))
                .await?
        {
            return Err(AppError::Conflict(format!(
                "手机号已存在: '{}'",
                phone
            )));
        }
        if let Some(ref email) = bo.email
            && !email.trim().is_empty()
            && self
                .user_repo
                .exists_by_email(ctx, email, Some(user_id))
                .await?
        {
            return Err(AppError::Conflict(format!("邮箱已存在: '{}'", email)));
        }

        if let Some(nick_name) = bo.nick_name {
            user.nick_name = nick_name;
        }
        if let Some(email) = bo.email {
            user.email = Some(email);
        }
        if let Some(phone) = bo.phone_number {
            user.phone_number = Some(phone);
        }
        if let Some(gender) = bo.gender {
            user.gender = Some(gender);
        }
        if let Some(avatar) = bo.avatar {
            user.avatar = Some(avatar);
        }

        // Controller:72 `DataPermissionHelper.ignore`:自服务写路径的"本人行"
        // 语义由主键(user_id = 本人)表达,不经数据范围 —— 走写侧显式豁免
        // 原语(软删存活/审计盖章/0 行 NotFound 照旧)。update_by/update_time
        // 由漏斗按 ctx principal 盖章(= 操作人)。
        self.user_repo
            .update_by_id_without_row_scope(ctx, &user, &user_id)
            .await?;
        Ok(())
    }

    /// 修改密码(审计人 = 操作人本人;范围/审计语义同
    /// [`Self::update_profile`] 的注释,Java 锚 Controller:98 +
    /// ServiceImpl:405)。
    pub async fn update_password(
        &self,
        ctx: &DataContext<'_>,
        user_id: i64,
        old_password: &str,
        new_password: &str,
    ) -> AppResult<()> {
        let mut user = self
            .user_repo
            .select_by_id(ctx, &user_id)
            .await?
            .ok_or_else(|| AppError::NotFound("用户不存在".to_string()))?;

        if !bcrypt::verify(old_password, &user.password)
            .map_err(|e| AppError::Internal(e.to_string()))?
        {
            return Err(AppError::BadRequest("旧密码不正确".to_string()));
        }
        if old_password == new_password {
            return Err(AppError::BadRequest(
                "新密码不能与旧密码相同".to_string(),
            ));
        }

        let new_hash = hash_password(new_password)?;
        user.password = new_hash;

        // Controller:98 `DataPermissionHelper.ignore`(同 update_profile 的
        // Controller:72 处置):写侧显式豁免行级范围,软删/审计照旧;
        // update_by/update_time 由漏斗按 ctx principal 盖章(= 操作人)。
        self.user_repo
            .update_by_id_without_row_scope(ctx, &user, &user_id)
            .await?;
        Ok(())
    }
}
