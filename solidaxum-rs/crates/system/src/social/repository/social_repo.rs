//! social 仓储(**全豁免模块**)。
//!
//! 漏斗公共契约(执行面/错误)见 `app_data` 模块文档。**方法级裁定(Java 基线逐方法核对)**:现网
//! `SysSocialMapper.java` **零 `@DataPermission`**(接口体为空,全走
//! BaseMapperPlus)—— 三方绑定在 Java 侧本就不受数据权限约束。统一锚:
//! **SysSocialMapper 无 @DataPermission —— 三方绑定非行级**(policy.rs
//! 落 CONSTRAINED=false;表虽有 user_id 列,那是绑定目标不是
//! 权限归属,勿"见列加回" owner_column)。由此每个读方法显式
//! `without_row_scope()`;sys_social 的软删列 del_flag 由漏斗照施。
//!
//! | 本文件方法 | Java 锚(Controller/ServiceImpl → SysSocialMapper) | @DataPermission | 漏斗路径 |
//! |---|---|---|---|
//! | `select_by_user_id` | SysSocialController.list(:37)→ `queryListByUserId`(SysSocialServiceImpl:60)→ `lambda().eq(userId).voList()` | ❌ | `without_row_scope()`(软删过滤漏斗照施) |
//! | `select_by_id` | `queryById`(SysSocialServiceImpl:34)→ selectVoById | ❌ | `without_row_scope()` |
//! | `soft_delete_by_id_for_user` | AuthController.unlockSocial(:157)→ `deleteWithValidById`(SysSocialServiceImpl:116)→ deleteById | — | 先读验属(豁免读:id + user_id)+ del_flag 一次 `ctx.update`(client 软删同款;0 行 → NotFound) |
//!
//! **软删形状说明**:Java SysSocial 实体无 @TableLogic 字段,deleteById
//! 是物理删;Rust 侧是软删 del_flag='2'(schema 有 del_flag 列),
//! 形状差异保持未改(与 notice create_by_name 同口径的如实锚定)。
//! 0 行 → NotFound 对齐 Java unlockSocial 的
//! `return rows ? R.ok() : R.fail`(AuthController:161)—— 静默 Ok
//! 会把"没删到"吞掉。

use app_core::model::status::DelFlag;
use app_data::{
    DataContext, DataError, DataSource,
    sea_orm::{
        self, ActiveModelBehavior, ActiveValue::Set, ColumnTrait, DateTimeUtc,
        DeriveEntityModel, DerivePrimaryKey, DeriveRelation, EntityTrait,
        EnumIter, Order, PrimaryKeyTrait, QueryFilter, QueryOrder,
    },
};

use crate::social::model::social::Social;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "sys_social")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: i64,
    pub user_id: i64,
    pub auth_id: Option<String>,
    pub source: Option<String>,
    pub open_id: Option<String>,
    pub user_name: Option<String>,
    pub nick_name: Option<String>,
    pub avatar: Option<String>,
    pub access_token: Option<String>,
    pub expire_in: Option<i32>,
    pub refresh_token: Option<String>,
    pub create_time: Option<DateTimeUtc>,
    pub del_flag: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

impl From<&Model> for Social {
    fn from(m: &Model) -> Self {
        Social {
            id: Some(m.id),
            user_id: m.user_id,
            auth_id: m.auth_id.clone(),
            source: m.source.clone(),
            open_id: m.open_id.clone(),
            user_name: m.user_name.clone(),
            nick_name: m.nick_name.clone(),
            avatar: m.avatar.clone(),
            access_token: m.access_token.clone(),
            expire_in: m.expire_in,
            refresh_token: m.refresh_token.clone(),
            create_time: m.create_time,
            del_flag: Some(m.del_flag.clone()),
        }
    }
}

#[derive(Clone)]
pub struct SocialRepository {
    /// 装配期快照:方法只转发调用方 ctx(豁免读不解析 resolver,
    /// 降级快照无影响)。
    ds: std::sync::Arc<DataSource>,
}

impl SocialRepository {
    pub fn new(ds: std::sync::Arc<DataSource>) -> Self {
        Self { ds }
    }

    /// 测试/工具桥接面:system_context。生产零消费,notice/client 同形。
    pub fn system_context(&self) -> DataContext<'_> {
        self.ds.system_context()
    }

    /// 查询用户绑定的三方账号列表(按创建时间倒序)。
    ///
    /// 豁免裁定:SysSocialMapper 无 @DataPermission —— 三方绑定非行级
    /// (Java 锚 queryListByUserId SysSocialServiceImpl:60;软删过滤
    /// 由漏斗照施)。
    pub async fn select_by_user_id(
        &self,
        ctx: &DataContext<'_>,
        user_id: i64,
    ) -> Result<Vec<Social>, DataError> {
        let models = ctx
            .select(
                Entity::find()
                    .filter(Column::UserId.eq(user_id))
                    .order_by(Column::CreateTime, Order::Desc),
            )
            .without_row_scope()
            .all()
            .await?;
        Ok(models.iter().map(Social::from).collect())
    }

    /// 按主键查绑定详情。
    ///
    /// 豁免裁定:SysSocialMapper 无 @DataPermission —— 三方绑定非行级
    /// (Java 锚 queryById SysSocialServiceImpl:34;软删过滤由漏斗照施)。
    pub async fn select_by_id(
        &self,
        ctx: &DataContext<'_>,
        id: i64,
    ) -> Result<Option<Social>, DataError> {
        let model = ctx
            .select(Entity::find_by_id(id))
            .without_row_scope()
            .one()
            .await?;
        Ok(model.as_ref().map(Social::from))
    }

    /// 解绑本人的三方账号(软删 del_flag='2')。
    ///
    /// "本人行"语义由先读半边的 user_id 条件表达(Java
    /// DataPermissionHelper.ignore 的同款形状,profile 自服务写先例);
    /// 后续 `ctx.update` 的 WHERE = 主键(豁免实体),软删存活过滤由
    /// 漏斗照施 → 已删行/不存在 → NotFound(对齐 Java unlockSocial 的
    /// R.fail 分支,AuthController:161)。审计:sys_social 只有
    /// create_time 列,无 update_* 可盖(policy.rs 如实声明)。
    pub async fn soft_delete_by_id_for_user(
        &self,
        ctx: &DataContext<'_>,
        id: i64,
        user_id: i64,
    ) -> Result<(), DataError> {
        let model = ctx
            .select(
                Entity::find()
                    .filter(Column::Id.eq(id))
                    .filter(Column::UserId.eq(user_id)),
            )
            .without_row_scope()
            .one()
            .await?
            .ok_or(DataError::NotFound)?;
        let active = ActiveModel {
            id: Set(model.id),
            del_flag: Set(DelFlag::Deleted.as_str().to_string()),
            ..Default::default()
        };
        ctx.update(active).await?;
        Ok(())
    }
}
