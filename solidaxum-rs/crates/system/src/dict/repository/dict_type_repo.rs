//! dict_type 仓储(**全豁免模块**)。
//!
//! 漏斗公共契约(执行面/错误)见 `app_data` 模块文档。**方法级裁定(Java 基线逐方法核对)**:现网
//! `SysDictTypeMapper.java` **零 `@DataPermission`**(接口体为空,连自定义
//! 方法都没有,全走 BaseMapperPlus)—— 字典类型是全局功能定义,数据
//! 权限过滤对它无业务含义。统一锚:**SysDictTypeMapper 无
//! @DataPermission —— 字典数据非行级**。由此每个读方法显式
//! `without_row_scope()`(豁免必须写出这行字才能过审,不做静默默认);
//! sys_dict_type 的 DataPolicy 落 CONSTRAINED=false,无软删列,
//! 漏斗对读只剩"显式豁免"这一个动作可做。
//!
//! | 本文件方法 | Java 锚(SysDictTypeController/ServiceImpl → SysDictTypeMapper) | @DataPermission | 漏斗路径 |
//! |---|---|---|---|
//! | `select_all` | optionselect/export(Controller:145/:58)→ `selectDictTypeAll`(ServiceImpl:96)→ `selectVoList` 基方法 | ❌ | `without_row_scope()` |
//! | `select_by_id` | getInfo(Controller:71)→ `selectDictTypeById`(ServiceImpl:120)→ selectVoById | ❌ | `without_row_scope()` |
//! | `select_by_type` | `selectDictTypeByType`(ServiceImpl:132,`@Cacheable(SYS_DICT_TYPE)`:130 缓存路径;唯一性校验 checkDictTypeUnique:216 也走它) | ❌ | `without_row_scope()`(唯一性须全量) |
//! | `select_page` | list(Controller:45)→ `selectPageDictTypeList`(ServiceImpl:58)→ selectVoPage | ❌ | `without_row_scope()`(逃生舱分页,total 全量口径) |
//! | `insert` | add(Controller:85)→ `insertDictType`(ServiceImpl:174,写路径) | — | `ctx.insert`(审计五列漏斗盖) |
//! | `update_by_id` | edit(Controller:103)→ `updateDictType`(ServiceImpl:193,写路径) | — | `ctx.update`(豁免实体无行级范围;审计盖 update_*,0 行 → NotFound) |
//! | `update_by_id_in_txn` | 同上(`@Transactional` ServiceImpl:192 的自身 update 半边,生产路径在 service 的事务闭包内) | — | `tx.update`(dept `update_by_id_in_txn` 同款) |
//! | `delete_by_id`(物理) | remove(Controller:120)→ `deleteDictTypeByIds`(ServiceImpl:142)→ deleteByIds | — | `ctx.delete`(WHERE=主键,0 行 → NotFound) |
//!
//! **写路径说明**:Java 写方法本就无注解(拦截器只改写查询),sys_dict_type
//! 是豁免实体(CONSTRAINED=false),漏斗 update/delete 对它不施加行级范围
//! (WHERE = 主键),审计五列照盖(insert 全盖、update 盖 update_*)—— 与
//! Java 直连行为逐点一致,无收紧也无放松。0 行 → NotFound 由漏斗承担,
//! update_by_id 不做"先查后改"预检。
//!
//! **缓存互锚**:Java `selectDictTypeByType` 挂 `@Cacheable(SYS_DICT_TYPE)`
//! (ServiceImpl:132)—— Rust 侧 dict_type_service 的 `sys_dict_type:{t}`
//! Redis 旁路与它对位,缓存未命中才落库,落库读即本文件 `select_by_type`
//! (豁免口径);写路径的缓存失效(evict)在 service 层,与漏斗无关。

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

use crate::dict::model::dict_type::{
    DictType, DictTypeListVo, DictTypePageQuery,
};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "sys_dict_type")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub dict_id: i64,
    pub dict_name: String,
    pub dict_type: String,
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

impl From<&Model> for DictType {
    fn from(m: &Model) -> Self {
        DictType {
            dict_id: Some(m.dict_id),
            dict_name: m.dict_name.clone(),
            dict_type: m.dict_type.clone(),
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

/// DictType → ActiveModel。审计五列(create_dept/create_by/create_time/
/// update_by/update_time)只搬运领域层已持有的值;**写入时由漏斗统一盖章**
/// (insert 全盖、update 盖 update_*),领域层不再手工盖章。
fn dict_type_to_active_model(dict_type: &DictType) -> ActiveModel {
    ActiveModel {
        dict_id: match dict_type.dict_id {
            Some(id) => Set(id),
            None => NotSet,
        },
        dict_name: Set(dict_type.dict_name.clone()),
        dict_type: Set(dict_type.dict_type.clone()),
        create_dept: Set(dict_type.base_entity.create_dept),
        create_by: Set(dict_type.base_entity.create_by),
        create_time: Set(dict_type.base_entity.create_time),
        update_by: Set(dict_type.base_entity.update_by),
        update_time: Set(dict_type.base_entity.update_time),
        remark: Set(dict_type.remark.clone()),
    }
}

#[derive(Clone)]
pub struct DictTypeRepository {
    /// 装配期快照:方法只转发调用方 ctx(装配期 data_source 多半还是降级
    /// 快照,不影响豁免读 —— without_row_scope 连 resolver 都不解析)。
    ds: std::sync::Arc<DataSource>,
}

impl DictTypeRepository {
    pub fn new(ds: std::sync::Arc<DataSource>) -> Self {
        Self { ds }
    }

    /// 测试/工具桥接面:system_context(principal=None → ScopeFilter 恒
    /// All)。生产零消费(handler 一律请求期构造用户 ctx),menu/post
    /// 仓储同形保留。
    pub fn system_context(&self) -> DataContext<'_> {
        self.ds.system_context()
    }

    /// 全量字典类型(optionselect / export 用)。
    ///
    /// 豁免裁定:SysDictTypeMapper 无 @DataPermission —— 字典数据非行级
    /// (Java 锚 selectDictTypeAll ServiceImpl:96 → selectVoList)。
    pub async fn select_all(
        &self,
        ctx: &DataContext<'_>,
    ) -> Result<Vec<DictType>, DataError> {
        let models = ctx
            .select(Entity::find().order_by(Column::DictId, Order::Asc))
            .without_row_scope()
            .all()
            .await?;
        Ok(models.iter().map(DictType::from).collect())
    }

    /// 按主键查详情。
    ///
    /// 豁免裁定:SysDictTypeMapper 无 @DataPermission —— 字典数据非行级
    /// (Java 锚 selectDictTypeById ServiceImpl:120 → selectVoById)。
    pub async fn select_by_id(
        &self,
        ctx: &DataContext<'_>,
        dict_id: &i64,
    ) -> Result<Option<DictType>, DataError> {
        let model = ctx
            .select(Entity::find_by_id(*dict_id))
            .without_row_scope()
            .one()
            .await?;
        Ok(model.as_ref().map(DictType::from))
    }

    /// 按字典类型标识查(缓存未命中的落库读 + 唯一性校验)。
    ///
    /// 豁免裁定:SysDictTypeMapper 无 @DataPermission —— 字典数据非行级
    /// (Java 锚 selectDictTypeByType ServiceImpl:132,`@Cacheable
    /// (SYS_DICT_TYPE)` 的落库半边;唯一性 checkDictTypeUnique:213 共用,
    /// 唯一性须全量)。
    pub async fn select_by_type(
        &self,
        ctx: &DataContext<'_>,
        dict_type: &str,
    ) -> Result<Option<DictType>, DataError> {
        let model = ctx
            .select(Entity::find().filter(Column::DictType.eq(dict_type)))
            .without_row_scope()
            .one()
            .await?;
        Ok(model.as_ref().map(DictType::from))
    }

    /// 分页查询字典类型列表。
    ///
    /// 豁免裁定:SysDictTypeMapper 无 @DataPermission —— 字典数据非行级
    /// (Java 锚 list Controller:45 → selectPageDictTypeList
    /// ServiceImpl:58 → selectVoPage;排序在 Select 构件上,逃生舱
    /// 分页保持 ORDER BY,total 为全量口径)。
    pub async fn select_page(
        &self,
        ctx: &DataContext<'_>,
        query: &DictTypePageQuery,
        page_no: u64,
        page_size: u64,
        order_by: &str,
    ) -> Result<TableDataInfo<DictTypeListVo>, DataError> {
        let dict_name = str_trim_or_empty(&query.dict_name);
        let dict_type = str_trim_or_empty(&query.dict_type);

        let mut select = Entity::find();

        if !dict_name.is_empty() {
            select = select.filter(Column::DictName.contains(&dict_name));
        }
        if !dict_type.is_empty() {
            select = select.filter(Column::DictType.contains(&dict_type));
        }

        let (order_col, order_dir) = parse_order(order_by);
        select = select.order_by(order_col, order_dir);

        let page = ctx
            .select(select)
            .without_row_scope()
            .page(page_no as i64, page_size as i64)
            .await?;

        let rows: Vec<DictTypeListVo> = page
            .rows
            .iter()
            .map(DictType::from)
            .map(|d: DictType| d.to_list_vo())
            .collect();

        Ok(TableDataInfo::from_page_rows(&page, rows))
    }

    /// 新增字典类型。审计五列由漏斗 insert 统一盖章(principal 取自 ctx),
    /// 领域层传入的审计值会被覆盖(Java 锚 insertDictType ServiceImpl:174
    /// 前半,`@CachePut(SYS_DICT)` 写缓存在 service 层)。
    pub async fn insert(
        &self,
        ctx: &DataContext<'_>,
        dict_type: &DictType,
    ) -> Result<(), DataError> {
        let active = dict_type_to_active_model(dict_type);
        ctx.insert_returning_nothing(active).await?;
        Ok(())
    }

    /// 按主键更新。漏斗 update:审计盖 update_*(create_* 搬运领域层原值),
    /// WHERE = 主键(sys_dict_type 豁免实体,无软删/行级范围),0 行 →
    /// NotFound(Java 锚 updateDictType ServiceImpl:193;原"先查后改"预检
    /// 随漏斗 WHERE 删除 —— service 侧为拼领域对象仍需先读,读不到时
    /// NotFound 语义相同)。
    pub async fn update_by_id(
        &self,
        ctx: &DataContext<'_>,
        dict_type: &DictType,
        dict_id: &i64,
    ) -> Result<(), DataError> {
        let mut active = dict_type_to_active_model(dict_type);
        active.dict_id = Set(*dict_id);
        ctx.update(active).await?;
        Ok(())
    }

    /// [`Self::update_by_id`] 的事务内变体(关联函数:执行面全在 tx,
    /// 无仓储状态参与,dept `update_by_id_in_txn` 同款)。生产路径在
    /// dict_type_service 改名分支的 `ctx.transaction` 闭包内 —— 自身
    /// update 与 sys_dict_data 级联必须同事务(Java `updateDictType` 带
    /// @Transactional,ServiceImpl:192)。`pub`:repo 集成测试要驱动
    /// 事务闭包内的自身 update 半边(menu `delete_role_menus_by_menu_ids`
    /// 同口径)。
    pub async fn update_by_id_in_txn(
        tx: &app_data::TxnContext<'_>,
        dict_type: &DictType,
    ) -> Result<(), DataError> {
        let active = dict_type_to_active_model(dict_type);
        tx.update(active).await?;
        Ok(())
    }

    /// 物理删除字典类型(Java 锚 deleteDictTypeByIds ServiceImpl:142 →
    /// deleteByIds;sys_dict_type 无 @TableLogic,Java 侧同为物理删)。
    /// 漏斗 delete:WHERE = 主键,0 行 → NotFound(生产调用点在 service
    /// 删除守卫通过之后,守卫语义见 dict_data_repo `exists_by_type`)。
    pub async fn delete_by_id(
        &self,
        ctx: &DataContext<'_>,
        dict_id: &i64,
    ) -> Result<(), DataError> {
        ctx.delete::<Entity>(*dict_id).await
    }
}

fn parse_order(order_by: &str) -> (Column, Order) {
    let mut parts = order_by.split_whitespace();
    let col = match parts.next().unwrap_or("dict_id") {
        "dict_id" => Column::DictId,
        "dict_name" => Column::DictName,
        "dict_type" => Column::DictType,
        "create_time" => Column::CreateTime,
        _ => Column::DictId,
    };
    let desc = !parts.next().is_some_and(|d| d.eq_ignore_ascii_case("asc"));
    if desc {
        (col, Order::Desc)
    } else {
        (col, Order::Asc)
    }
}
