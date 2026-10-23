//! dict_data 仓储(**全豁免模块**)。
//!
//! 漏斗公共契约(执行面/错误)见 `app_data` 模块文档。**方法级裁定(Java 基线逐方法核对)**:现网
//! `SysDictDataMapper.java` **零 `@DataPermission`**(接口只有 default
//! `selectDictDataByType`,其余走 BaseMapperPlus)—— 字典数据是全局
//! 功能定义,数据权限过滤对它无业务含义。统一锚:**SysDictDataMapper
//! 无 @DataPermission —— 字典数据非行级**。由此每个读方法显式
//! `without_row_scope()`;sys_dict_data 的 DataPolicy 落
//! CONSTRAINED=false,无软删列,漏斗对读只剩"显式豁免"这一个动作可做。
//!
//! | 本文件方法 | Java 锚(SysDictDataController/ServiceImpl → SysDictDataMapper) | @DataPermission | 漏斗路径 |
//! |---|---|---|---|
//! | `select_by_type` | `/dict/data/type/{t}`(Controller:86)→ SysDictTypeServiceImpl `selectDictDataByType`(ServiceImpl:108,`@Cacheable(SYS_DICT)`:106 缓存路径)→ Mapper default `selectDictDataByType`(eq + orderByAsc) | ❌ | `without_row_scope()` |
//! | `select_all` | export(Controller:62)→ `selectDictDataList`(SysDictDataServiceImpl:56)→ `selectVoList` | ❌ | `without_row_scope()` |
//! | `exists_by_type_and_value` | `checkDictDataUnique`(SysDictDataServiceImpl:157)→ `lambda().exists()` | ❌ | `without_row_scope()`(唯一性须全量) |
//! | `exists_by_type` | `deleteDictTypeByIds` 的删除守卫(SysDictTypeServiceImpl:147)→ `lambda().eq(...).exists()` | ❌ | `without_row_scope()`(守卫须真实计数) |
//! | `select_by_id` | getInfo(Controller:75)→ `selectDictDataById`(SysDictDataServiceImpl:100) → selectVoById | ❌ | `without_row_scope()` |
//! | `select_page` | list(Controller:49)→ `selectPageDictDataList`(SysDictDataServiceImpl:43) → selectVoPage | ❌ | `without_row_scope()`(逃生舱分页,total 全量口径) |
//! | `insert` | add(Controller:104)→ `insertDictData`(SysDictDataServiceImpl:124)(写路径,`@CachePut(SYS_DICT)`) | — | `ctx.insert`(审计五列漏斗盖) |
//! | `update_by_id` | edit(Controller:122)→ `updateDictData`(SysDictDataServiceImpl:141)(写路径,`@CachePut(SYS_DICT)`) | — | `ctx.update`(豁免实体;审计盖 update_*,0 行 → NotFound) |
//! | `delete_by_id`(物理) | remove(Controller:139)→ `deleteDictDataByIds`(SysDictDataServiceImpl:110) → deleteByIds | — | `ctx.delete`(WHERE=主键,0 行 → NotFound) |
//! | `update_dict_type_in_txn` | SysDictTypeServiceImpl `updateDictType` 的级联半边(ServiceImpl:192-196,`UPDATE sys_dict_data SET dict_type` + `@Transactional`) | —(写) | 读-改-写逐行 `tx.update`(dept `update_children_ancestors_in_txn` 同款) |
//!
//! **缓存互锚**:Java `selectDictDataByType` 挂 `@Cacheable(SYS_DICT)`
//! (ServiceImpl:106)—— Rust 侧 dict_data_service 的 `sys_dict:{t}` Redis
//! 旁路与它对位,缓存未命中才落库,落库读即本文件 `select_by_type`
//! (豁免口径);写路径的缓存失效(evict/put)在 service 层,与漏斗无关。

use app_core::{
    model::entity::BaseEntity, repo::str_trim_or_empty, vo::page::TableDataInfo,
};
use app_data::{
    DataContext, DataError, DataSource, TxnContext,
    sea_orm::{
        self, ActiveModelBehavior,
        ActiveValue::{NotSet, Set},
        ColumnTrait, DateTimeUtc, DeriveEntityModel, DerivePrimaryKey,
        DeriveRelation, EntityTrait, EnumIter, Order, PrimaryKeyTrait,
        QueryFilter, QueryOrder,
    },
};

use crate::dict::model::dict_data::{
    DictData, DictDataListVo, DictDataPageQuery,
};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "sys_dict_data")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub dict_code: i64,
    pub dict_sort: i32,
    pub dict_label: String,
    pub dict_value: String,
    pub dict_type: String,
    pub css_class: Option<String>,
    pub list_class: Option<String>,
    pub is_default: String,
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

impl From<&Model> for DictData {
    fn from(m: &Model) -> Self {
        DictData {
            dict_code: Some(m.dict_code),
            dict_sort: m.dict_sort,
            dict_label: m.dict_label.clone(),
            dict_value: m.dict_value.clone(),
            dict_type: m.dict_type.clone(),
            css_class: m.css_class.clone(),
            list_class: m.list_class.clone(),
            is_default: m.is_default.clone(),
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

/// DictData → ActiveModel。审计五列只搬运领域层已持有的值;**写入时由
/// 漏斗统一盖章**(insert 全盖、update 盖 update_*),领域层不再手工盖章。
fn dict_data_to_active_model(dict_data: &DictData) -> ActiveModel {
    ActiveModel {
        dict_code: match dict_data.dict_code {
            Some(id) => Set(id),
            None => NotSet,
        },
        dict_sort: Set(dict_data.dict_sort),
        dict_label: Set(dict_data.dict_label.clone()),
        dict_value: Set(dict_data.dict_value.clone()),
        dict_type: Set(dict_data.dict_type.clone()),
        css_class: Set(dict_data.css_class.clone()),
        list_class: Set(dict_data.list_class.clone()),
        is_default: Set(dict_data.is_default.clone()),
        create_dept: Set(dict_data.base_entity.create_dept),
        create_by: Set(dict_data.base_entity.create_by),
        create_time: Set(dict_data.base_entity.create_time),
        update_by: Set(dict_data.base_entity.update_by),
        update_time: Set(dict_data.base_entity.update_time),
        remark: Set(dict_data.remark.clone()),
    }
}

#[derive(Clone)]
pub struct DictDataRepository {
    /// 装配期快照:方法只转发调用方 ctx(豁免读不解析 resolver,
    /// 降级快照无影响)。
    ds: std::sync::Arc<DataSource>,
}

impl DictDataRepository {
    pub fn new(ds: std::sync::Arc<DataSource>) -> Self {
        Self { ds }
    }

    /// 测试/工具桥接面:system_context。生产零消费,dict_type_repo 同形。
    pub fn system_context(&self) -> DataContext<'_> {
        self.ds.system_context()
    }

    /// 根据字典类型查询全部数据 (按 dict_sort 升序)。
    ///
    /// 豁免裁定:SysDictDataMapper 无 @DataPermission —— 字典数据非行级
    /// (Java 锚 selectDictDataByType Mapper default 方法;`@Cacheable
    /// (SYS_DICT)` 的落库半边,缓存未命中才走到这里)。
    pub async fn select_by_type(
        &self,
        ctx: &DataContext<'_>,
        dict_type: &str,
    ) -> Result<Vec<DictData>, DataError> {
        let models = ctx
            .select(
                Entity::find()
                    .filter(Column::DictType.eq(dict_type))
                    .order_by(Column::DictSort, Order::Asc),
            )
            .without_row_scope()
            .all()
            .await?;
        Ok(models.iter().map(DictData::from).collect())
    }

    /// 全量查询(供 export;按 dict_sort 升序)。
    ///
    /// 豁免裁定:SysDictDataMapper 无 @DataPermission —— 字典数据非行级
    /// (Java 锚 export → selectDictDataList → selectVoList)。
    pub async fn select_all(
        &self,
        ctx: &DataContext<'_>,
    ) -> Result<Vec<DictData>, DataError> {
        let models = ctx
            .select(Entity::find().order_by(Column::DictSort, Order::Asc))
            .without_row_scope()
            .all()
            .await?;
        Ok(models.iter().map(DictData::from).collect())
    }

    /// 校验 (dict_type, dict_value) 是否已存在(对齐 Java checkDictDataUnique)。
    /// `exclude_code` 用于更新时排除自身(传当前 dict_code)。
    ///
    /// 豁免裁定:SysDictDataMapper 无 @DataPermission —— 字典数据非行级
    /// (唯一性校验须全量;exists 语义以 `.one()` 终端实现,免 COUNT)。
    pub async fn exists_by_type_and_value(
        &self,
        ctx: &DataContext<'_>,
        dict_type: &str,
        dict_value: &str,
        exclude_code: Option<i64>,
    ) -> Result<bool, DataError> {
        let mut select = Entity::find()
            .filter(Column::DictType.eq(dict_type))
            .filter(Column::DictValue.eq(dict_value));
        if let Some(code) = exclude_code {
            select = select.filter(Column::DictCode.ne(code));
        }
        let model = ctx.select(select).without_row_scope().one().await?;
        Ok(model.is_some())
    }

    /// 字典类型下是否已有数据(dict_type 删除守卫)。
    ///
    /// 豁免裁定:SysDictDataMapper 无 @DataPermission —— 字典数据非行级
    /// (Java 锚 deleteDictTypeByIds 的守卫 SysDictTypeServiceImpl:147 →
    /// `lambda().eq(...).exists()`;守卫须真实计数,不得被范围砍小)。
    pub async fn exists_by_type(
        &self,
        ctx: &DataContext<'_>,
        dict_type: &str,
    ) -> Result<bool, DataError> {
        let model = ctx
            .select(Entity::find().filter(Column::DictType.eq(dict_type)))
            .without_row_scope()
            .one()
            .await?;
        Ok(model.is_some())
    }

    /// 按主键查详情。
    ///
    /// 豁免裁定:SysDictDataMapper 无 @DataPermission —— 字典数据非行级
    /// (Java 锚 selectDictDataById → selectVoById)。
    pub async fn select_by_id(
        &self,
        ctx: &DataContext<'_>,
        dict_code: &i64,
    ) -> Result<Option<DictData>, DataError> {
        let model = ctx
            .select(Entity::find_by_id(*dict_code))
            .without_row_scope()
            .one()
            .await?;
        Ok(model.as_ref().map(DictData::from))
    }

    /// 分页查询字典数据列表。
    ///
    /// 豁免裁定:SysDictDataMapper 无 @DataPermission —— 字典数据非行级
    /// (Java 锚 list → selectPageDictDataList → selectVoPage;排序在
    /// Select 构件上,total 为全量口径)。
    pub async fn select_page(
        &self,
        ctx: &DataContext<'_>,
        query: &DictDataPageQuery,
        page_no: u64,
        page_size: u64,
        order_by: &str,
    ) -> Result<TableDataInfo<DictDataListVo>, DataError> {
        let dict_type = str_trim_or_empty(&query.dict_type);
        let dict_label = str_trim_or_empty(&query.dict_label);

        let mut select = Entity::find();

        if !dict_type.is_empty() {
            select = select.filter(Column::DictType.eq(&dict_type));
        }
        if !dict_label.is_empty() {
            select = select.filter(Column::DictLabel.contains(&dict_label));
        }

        let (order_col, order_dir) = parse_order(order_by);
        select = select.order_by(order_col, order_dir);

        let page = ctx
            .select(select)
            .without_row_scope()
            .page(page_no as i64, page_size as i64)
            .await?;

        let rows: Vec<DictDataListVo> = page
            .rows
            .iter()
            .map(DictData::from)
            .map(|d: DictData| d.to_list_vo())
            .collect();

        Ok(TableDataInfo::from_page_rows(&page, rows))
    }

    /// 新增字典数据。审计五列由漏斗 insert 统一盖章(principal 取自 ctx)
    /// (Java 锚 insertDictData,`@CachePut(SYS_DICT)` 写缓存在 service 层)。
    pub async fn insert(
        &self,
        ctx: &DataContext<'_>,
        dict_data: &DictData,
    ) -> Result<(), DataError> {
        let active = dict_data_to_active_model(dict_data);
        ctx.insert_returning_nothing(active).await?;
        Ok(())
    }

    /// 按主键更新。漏斗 update:审计盖 update_*(create_* 搬运领域层
    /// 原值),WHERE = 主键(豁免实体),0 行 → NotFound(原"先查后改"
    /// 预检随漏斗 WHERE 删除,service 侧先读拼领域对象,语义相同)。
    pub async fn update_by_id(
        &self,
        ctx: &DataContext<'_>,
        dict_data: &DictData,
        dict_code: &i64,
    ) -> Result<(), DataError> {
        let mut active = dict_data_to_active_model(dict_data);
        active.dict_code = Set(*dict_code);
        ctx.update(active).await?;
        Ok(())
    }

    /// 物理删除字典数据(sys_dict_data 无 @TableLogic,Java 侧同为
    /// 物理删)。漏斗 delete:WHERE = 主键,0 行 → NotFound。
    pub async fn delete_by_id(
        &self,
        ctx: &DataContext<'_>,
        dict_code: &i64,
    ) -> Result<(), DataError> {
        ctx.delete::<Entity>(*dict_code).await
    }

    /// 级联改写 sys_dict_data.dict_type(dict_type 改名的跟随半边)。
    ///
    /// 原 `UPDATE sys_dict_data SET dict_type = ? WHERE dict_type = ?`
    /// (Java SysDictTypeServiceImpl.updateDictType ServiceImpl:192-196
    /// 的 lambda update;`@Transactional` ServiceImpl:192 平价 —— 生产
    /// 调用点在 dict_type_service 的事务闭包内,与自身 update 同事务,
    /// dept `update_children_ancestors_in_txn` 同款)。逃生舱无按条件
    /// 批量 SET 原语:读-改-写逐行 update,每行审计盖 update_*
    /// (principal 取自 tx);豁免读(字典数据非行级)。同名早退。
    pub async fn update_dict_type_in_txn(
        tx: &TxnContext<'_>,
        old_dict_type: &str,
        new_dict_type: &str,
    ) -> Result<(), DataError> {
        if old_dict_type == new_dict_type {
            return Ok(());
        }
        let rows = tx
            .select(Entity::find().filter(Column::DictType.eq(old_dict_type)))
            .without_row_scope()
            .all()
            .await?;
        for row in rows {
            let active = ActiveModel {
                dict_code: Set(row.dict_code),
                dict_type: Set(new_dict_type.to_string()),
                ..Default::default()
            };
            tx.update(active).await?;
        }
        Ok(())
    }
}

fn parse_order(order_by: &str) -> (Column, Order) {
    let mut parts = order_by.split_whitespace();
    let col = match parts.next().unwrap_or("dict_sort") {
        "dict_code" => Column::DictCode,
        "dict_sort" => Column::DictSort,
        "dict_label" => Column::DictLabel,
        "dict_value" => Column::DictValue,
        "dict_type" => Column::DictType,
        "create_time" => Column::CreateTime,
        _ => Column::DictSort,
    };
    let desc = !parts.next().is_some_and(|d| d.eq_ignore_ascii_case("asc"));
    if desc {
        (col, Order::Desc)
    } else {
        (col, Order::Asc)
    }
}
