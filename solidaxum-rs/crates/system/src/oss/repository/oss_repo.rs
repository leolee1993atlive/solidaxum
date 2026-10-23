//! oss 仓储(**全豁免模块**)。
//!
//! 漏斗公共契约(执行面/错误)见 `app_data` 模块文档。**方法级裁定(Java 基线逐方法核对)**:现网
//! `SysOssMapper.java` **零 `@DataPermission`**(接口体为空,全走
//! BaseMapperPlus)—— 文件记录是全局资源(上传/下载/列表按 oss_id 定位,
//! 可见性不经数据范围)。统一锚:**SysOssMapper 无 @DataPermission ——
//! 文件记录非行级**。由此每个读方法显式 `without_row_scope()`;sys_oss
//! 无软删列(policy.rs 如实),删除是物理删(Java 侧同为物理删)。
//!
//! | 本文件方法 | Java 锚(SysOssController/ServiceImpl → SysOssMapper) | @DataPermission | 漏斗路径 |
//! |---|---|---|---|
//! | `select_by_id` | download(Controller:92)→ `getById`(SysOssServiceImpl:187,**@Cacheable(SYS_OSS):186**)→ selectVoById | ❌ | `without_row_scope()` |
//! | `select_by_ids` | listByIds(Controller:63)→ `listByIds`(ServiceImpl:85)→ selectByIds | ❌ | `without_row_scope()` |
//! | `insert` | upload(Controller:78)→ `upload`(ServiceImpl:233)→ ossMapper.insert | — | `ctx.insert`(审计五列漏斗盖) |
//! | `delete_by_id`(物理) | remove(Controller:105)→ `deleteWithValidByIds`(ServiceImpl:307)→ deleteByIds | — | `ctx.delete`(WHERE=主键,0 行 → NotFound) |
//! | `select_page` | list(Controller:51)→ `queryPageList`(ServiceImpl:70)→ selectVoPage | ❌ | `without_row_scope()`(逃生舱分页,total 全量口径) |
//!
//! **缓存互锚**:Java `getById` 挂 `@Cacheable(SYS_OSS)`(ServiceImpl:186)
//! —— Rust 侧未移植该 oss-id 缓存(translation 自有整行读路径),
//! 未命中的落库读即本文件 `select_by_id`(豁免口径);后续若补缓存,
//! evict/put 留在 service 层,与 dict/config 的旁路对位同款。

use app_core::{
    model::entity::BaseEntity, repo::str_trim_or_empty, vo::page::TableDataInfo,
};
use app_data::{
    DataContext, DataError, DataSource,
    sea_orm::{
        self, ActiveModelBehavior,
        ActiveValue::{NotSet, Set},
        ColumnTrait, DateTimeUtc, DeriveEntityModel, DerivePrimaryKey,
        DeriveRelation, EntityTrait, EnumIter, Order, PrimaryKeyTrait,
        QueryFilter, QueryOrder,
    },
};

use crate::oss::model::oss::{OssQueryBo, SysOss};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "sys_oss")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub oss_id: i64,
    pub file_name: String,
    pub original_name: String,
    pub file_suffix: String,
    pub url: String,
    pub ext1: Option<String>,
    pub create_dept: Option<i64>,
    pub create_by: Option<i64>,
    pub create_time: Option<DateTimeUtc>,
    pub update_by: Option<i64>,
    pub update_time: Option<DateTimeUtc>,
    pub service: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

impl From<&Model> for SysOss {
    fn from(m: &Model) -> Self {
        SysOss {
            oss_id: Some(m.oss_id),
            file_name: m.file_name.clone(),
            original_name: m.original_name.clone(),
            file_suffix: m.file_suffix.clone(),
            url: m.url.clone(),
            service: m.service.clone(),
            ext1: m.ext1.clone(),
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

/// SysOss → ActiveModel。审计五列只搬运领域层已持有的值;**写入时由
/// 漏斗统一盖章**(insert 全盖、update 盖 update_*),领域层不再手工盖章。
fn oss_to_active_model(oss: &SysOss) -> ActiveModel {
    ActiveModel {
        oss_id: match oss.oss_id {
            Some(id) => Set(id),
            None => NotSet,
        },
        file_name: Set(oss.file_name.clone()),
        original_name: Set(oss.original_name.clone()),
        file_suffix: Set(oss.file_suffix.clone()),
        url: Set(oss.url.clone()),
        ext1: Set(oss.ext1.clone()),
        create_dept: Set(oss.base_entity.create_dept),
        create_by: Set(oss.base_entity.create_by),
        create_time: Set(oss.base_entity.create_time),
        update_by: Set(oss.base_entity.update_by),
        update_time: Set(oss.base_entity.update_time),
        service: Set(oss.service.clone()),
    }
}

#[derive(Clone)]
pub struct OssRepository {
    /// 装配期快照:方法只转发调用方 ctx(豁免读不解析 resolver,
    /// 降级快照无影响)。
    ds: std::sync::Arc<DataSource>,
}

impl OssRepository {
    pub fn new(ds: std::sync::Arc<DataSource>) -> Self {
        Self { ds }
    }

    /// 测试/工具桥接面:system_context。生产零消费,notice/client 同形。
    pub fn system_context(&self) -> DataContext<'_> {
        self.ds.system_context()
    }

    /// 按主键查文件记录。
    ///
    /// 豁免裁定:SysOssMapper 无 @DataPermission —— 文件记录非行级
    /// (Java 锚 getById SysOssServiceImpl:187 → selectVoById;
    /// @Cacheable(SYS_OSS) 的缓存互锚见模块头)。
    pub async fn select_by_id(
        &self,
        ctx: &DataContext<'_>,
        oss_id: &i64,
    ) -> Result<Option<SysOss>, DataError> {
        let model = ctx
            .select(Entity::find_by_id(*oss_id))
            .without_row_scope()
            .one()
            .await?;
        Ok(model.as_ref().map(SysOss::from))
    }

    /// 按 id 集查文件记录(oss_id 倒序)。
    ///
    /// 豁免裁定:SysOssMapper 无 @DataPermission —— 文件记录非行级
    /// (Java 锚 listByIds SysOssServiceImpl:85 → selectByIds)。
    pub async fn select_by_ids(
        &self,
        ctx: &DataContext<'_>,
        oss_ids: &[i64],
    ) -> Result<Vec<SysOss>, DataError> {
        let models = ctx
            .select(
                Entity::find()
                    .filter(Column::OssId.is_in(oss_ids.to_vec()))
                    .order_by(Column::OssId, Order::Desc),
            )
            .without_row_scope()
            .all()
            .await?;
        Ok(models.iter().map(SysOss::from).collect())
    }

    /// 新增文件记录。审计五列由漏斗 insert 统一盖章(principal 取自 ctx)
    /// (Java 锚 upload SysOssServiceImpl:233 的落库半边)。
    pub async fn insert(
        &self,
        ctx: &DataContext<'_>,
        oss: &SysOss,
    ) -> Result<(), DataError> {
        let active = oss_to_active_model(oss);
        ctx.insert_returning_nothing(active).await?;
        Ok(())
    }

    /// 物理删除文件记录(sys_oss 无 @TableLogic,Java 侧同为物理删)。
    /// 漏斗 delete:WHERE = 主键,0 行 → NotFound(Java 锚
    /// deleteWithValidByIds SysOssServiceImpl:307)。
    pub async fn delete_by_id(
        &self,
        ctx: &DataContext<'_>,
        oss_id: &i64,
    ) -> Result<(), DataError> {
        ctx.delete::<Entity>(*oss_id).await
    }

    /// 分页查询文件记录列表。
    ///
    /// 豁免裁定:SysOssMapper 无 @DataPermission —— 文件记录非行级
    /// (Java 锚 queryPageList SysOssServiceImpl:70 → selectVoPage,
    /// buildQueryWrapper:165;排序在 Select 构件上,total 为全量口径)。
    pub async fn select_page(
        &self,
        ctx: &DataContext<'_>,
        query: &OssQueryBo,
        page_no: u64,
        page_size: u64,
        order_by: &str,
    ) -> Result<TableDataInfo<SysOss>, DataError> {
        let file_name = str_trim_or_empty(&query.file_name);
        let original_name = str_trim_or_empty(&query.original_name);
        let file_suffix = str_trim_or_empty(&query.file_suffix);
        let service = str_trim_or_empty(&query.service);

        let mut select = Entity::find();

        if !file_name.is_empty() {
            select = select.filter(Column::FileName.contains(&file_name));
        }
        if !original_name.is_empty() {
            select =
                select.filter(Column::OriginalName.contains(&original_name));
        }
        if !file_suffix.is_empty() {
            select = select.filter(Column::FileSuffix.eq(&file_suffix));
        }
        if !service.is_empty() {
            select = select.filter(Column::Service.eq(&service));
        }

        let (order_col, order_dir) = parse_order(order_by);
        select = select.order_by(order_col, order_dir);

        let page = ctx
            .select(select)
            .without_row_scope()
            .page(page_no as i64, page_size as i64)
            .await?;

        let rows: Vec<SysOss> = page.rows.iter().map(SysOss::from).collect();

        Ok(TableDataInfo::from_page_rows(&page, rows))
    }
}

fn parse_order(order_by: &str) -> (Column, Order) {
    let mut parts = order_by.split_whitespace();
    let col = match parts.next().unwrap_or("oss_id") {
        "oss_id" => Column::OssId,
        "file_name" => Column::FileName,
        "create_time" => Column::CreateTime,
        _ => Column::OssId,
    };
    let desc = !parts.next().is_some_and(|d| d.eq_ignore_ascii_case("asc"));
    if desc {
        (col, Order::Desc)
    } else {
        (col, Order::Asc)
    }
}
