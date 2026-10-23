//! gen_table 仓储(**全豁免模块**)。
//!
//! 漏斗公共契约(执行面/错误)见 `app_data` 模块文档。**方法级裁定(Java 基线核对)**:现网
//! `GenTableMapper.java` / `GenTableColumnMapper.java` 类级标注
//! `@InterceptorIgnore(dataPermission = "true")`(零 `@DataPermission`
//! 方法注解)—— Java 侧对代码生成元数据**显式关闭**数据权限拦截:
//! 这是开发者工具表,数据权限过滤对它无业务含义。统一锚:
//! **GenTableMapper 类级 @InterceptorIgnore(dataPermission) —— 代码
//! 生成元数据非行级**。由此每个读方法显式 `without_row_scope()`;
//! gen_table 的 DataPolicy 落 CONSTRAINED=false,无软删列。
//!
//! | 本文件方法 | Java 锚(GenController/GenTableServiceImpl → GenTableMapper) | @DataPermission | 漏斗路径 |
//! |---|---|---|---|
//! | `select_by_id` | getInfo(Controller)→ `selectGenTableById`(ServiceImpl)→ selectVoById | ❌(类级 Ignore) | `without_row_scope()` |
//! | `select_list` | 无 Java 对位(Rust 侧导出/调试用全量列表) | ❌(类级 Ignore) | `without_row_scope()` |
//! | `select_page` | list(Controller)→ `selectPageGenTableList`(ServiceImpl)→ selectVoPage | ❌(类级 Ignore) | `without_row_scope()`(逃生舱分页,total 全量口径) |
//! | `select_imported_table_names` | importTable 的已导入排重读(ServiceImpl `selectGenTableNameList` 对位) | ❌(类级 Ignore) | `without_row_scope()`(排重须全量) |
//! | `insert` | importTable(Controller)→ `importGenTable`(ServiceImpl,写路径) | — | `ctx.insert`(审计五列漏斗盖) |
//! | `update_by_id` | edit(Controller)→ `updateGenTable`(ServiceImpl,写路径) | — | `ctx.update`(豁免实体;审计盖 update_*,0 行 → NotFound) |
//! | `delete_by_ids` | remove(Controller)→ `deleteGenTableByIds`(ServiceImpl)→ deleteByIds | — | `ctx.delete_where`(豁免实体按条件批量删;0 行 → Ok,批量语义无 NotFound 约定) |
//!
//! **写路径说明**:gen_table 是豁免实体(CONSTRAINED=false),漏斗
//! update/delete_where 对它不施加行级范围,审计五列照盖(insert 全盖、
//! update 盖 update_*)。update 的 create_* 仍搬运领域层原值(service
//! 侧 BO 不带 create_* 时保持原样,见 service 注释)。

use app_core::model::entity::BaseEntity;
use app_data::{
    DataContext, DataError,
    sea_orm::{
        self, ActiveModelBehavior, ActiveValue, ColumnTrait, Condition,
        DateTimeUtc, DeriveEntityModel, DerivePrimaryKey, DeriveRelation,
        EntityTrait, EnumIter, PrimaryKeyTrait, QueryFilter, QueryOrder,
    },
};

use crate::codegen::model::gen_table::{GenTable, GenTableQueryBo};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "gen_table")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub table_id: i64,
    pub data_name: String,
    pub table_name: String,
    pub table_comment: String,
    pub class_name: String,
    pub tpl_category: String,
    pub frontend_type: String,
    pub package_name: String,
    pub module_name: String,
    pub business_name: String,
    pub function_name: String,
    pub function_author: String,
    pub gen_type: String,
    pub gen_path: String,
    pub options: Option<String>,
    pub remark: Option<String>,
    pub create_dept: Option<i64>,
    pub create_by: Option<i64>,
    pub create_time: Option<DateTimeUtc>,
    pub update_by: Option<i64>,
    pub update_time: Option<DateTimeUtc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

impl From<&Model> for GenTable {
    fn from(m: &Model) -> Self {
        let mut entity = GenTable {
            table_id: m.table_id,
            data_name: m.data_name.clone(),
            table_name: m.table_name.clone(),
            table_comment: m.table_comment.clone(),
            class_name: m.class_name.clone(),
            tpl_category: m.tpl_category.clone(),
            frontend_type: m.frontend_type.clone(),
            package_name: m.package_name.clone(),
            module_name: m.module_name.clone(),
            business_name: m.business_name.clone(),
            function_name: m.function_name.clone(),
            function_author: m.function_author.clone(),
            gen_type: m.gen_type.clone(),
            gen_path: m.gen_path.clone(),
            options: m.options.clone(),
            remark: m.remark.clone(),
            pk_column: None,
            columns: None,
            tree_code: None,
            tree_parent_code: None,
            tree_name: None,
            menu_ids: None,
            parent_menu_id: None,
            parent_menu_name: None,
            enable_export: None,
            enable_status: None,
            status_field: None,
            enable_unique: None,
            unique_fields: None,
            enable_sort: None,
            sort_field: None,
            tree_root_value: None,
            tree_ancestors_field: None,
            tree_order_field: None,
            base_entity: BaseEntity {
                create_dept: m.create_dept,
                create_by: m.create_by,
                create_time: m.create_time,
                update_by: m.update_by,
                update_time: m.update_time,
            },
        };
        if let Some(ref opts_str) = m.options
            && let Ok(map) = serde_json::from_str::<serde_json::Value>(opts_str)
            && let Some(obj) = map.as_object()
        {
            entity.tree_code = obj
                .get("treeCode")
                .and_then(|v| v.as_str().map(String::from));
            entity.tree_parent_code = obj
                .get("treeParentCode")
                .and_then(|v| v.as_str().map(String::from));
            entity.tree_name = obj
                .get("treeName")
                .and_then(|v| v.as_str().map(String::from));
            entity.parent_menu_id =
                obj.get("parentMenuId").and_then(|v| v.as_i64());
            entity.parent_menu_name = obj
                .get("parentMenuName")
                .and_then(|v| v.as_str().map(String::from));
            entity.enable_export =
                obj.get("enableExport").and_then(|v| v.as_bool());
            entity.enable_status =
                obj.get("enableStatus").and_then(|v| v.as_bool());
            entity.status_field = obj
                .get("statusField")
                .and_then(|v| v.as_str().map(String::from));
            entity.enable_unique =
                obj.get("enableUnique").and_then(|v| v.as_bool());
            entity.unique_fields = obj.get("uniqueFields").and_then(|v| {
                v.as_array().map(|a| {
                    a.iter()
                        .filter_map(|x| x.as_str().map(String::from))
                        .collect()
                })
            });
            entity.enable_sort =
                obj.get("enableSort").and_then(|v| v.as_bool());
            entity.sort_field = obj
                .get("sortField")
                .and_then(|v| v.as_str().map(String::from));
            entity.tree_root_value = obj
                .get("treeRootValue")
                .and_then(|v| v.as_str().map(String::from));
            entity.tree_ancestors_field = obj
                .get("treeAncestors")
                .and_then(|v| v.as_str().map(String::from));
            entity.tree_order_field = obj
                .get("treeOrderField")
                .and_then(|v| v.as_str().map(String::from));
        }
        entity
    }
}

/// GenTable → ActiveModel。审计五列只搬运领域层已持有的值;**写入时由
/// 漏斗统一盖章**(insert 全盖、update 盖 update_*),领域层不再手工盖章。
fn gen_table_to_active_model(table: &GenTable) -> ActiveModel {
    ActiveModel {
        table_id: ActiveValue::Set(table.table_id),
        data_name: ActiveValue::Set(table.data_name.clone()),
        table_name: ActiveValue::Set(table.table_name.clone()),
        table_comment: ActiveValue::Set(table.table_comment.clone()),
        class_name: ActiveValue::Set(table.class_name.clone()),
        tpl_category: ActiveValue::Set(table.tpl_category.clone()),
        frontend_type: ActiveValue::Set(table.frontend_type.clone()),
        package_name: ActiveValue::Set(table.package_name.clone()),
        module_name: ActiveValue::Set(table.module_name.clone()),
        business_name: ActiveValue::Set(table.business_name.clone()),
        function_name: ActiveValue::Set(table.function_name.clone()),
        function_author: ActiveValue::Set(table.function_author.clone()),
        gen_type: ActiveValue::Set(table.gen_type.clone()),
        gen_path: ActiveValue::Set(table.gen_path.clone()),
        options: ActiveValue::Set(table.options.clone()),
        remark: ActiveValue::Set(table.remark.clone()),
        create_dept: ActiveValue::Set(table.base_entity.create_dept),
        create_by: ActiveValue::Set(table.base_entity.create_by),
        create_time: ActiveValue::Set(table.base_entity.create_time),
        update_by: ActiveValue::Set(table.base_entity.update_by),
        update_time: ActiveValue::Set(table.base_entity.update_time),
    }
}

#[derive(Clone, Default)]
pub struct GenTableRepository {
    // 无状态仓储(dept 同型):执行面全在调用方传入的 ctx;codegen 侧
    // 无 system_context 生产消费(元数据读的 system_context 在 service
    // 自持的 ds 上,设计 6.3 meta 行),故不留装配期快照与桥接提供者。
}

impl GenTableRepository {
    pub fn new() -> Self {
        Self::default()
    }

    /// 按主键查详情。
    ///
    /// 豁免裁定:GenTableMapper 类级 @InterceptorIgnore(dataPermission)
    /// —— 代码生成元数据非行级(Java 锚 selectGenTableById)。
    pub async fn select_by_id(
        &self,
        ctx: &DataContext<'_>,
        table_id: i64,
    ) -> Result<Option<GenTable>, DataError> {
        let model = ctx
            .select(Entity::find_by_id(table_id))
            .without_row_scope()
            .one()
            .await?;
        Ok(model.as_ref().map(|m| m.into()))
    }

    /// 条件列表读(全量,不分页)。
    ///
    /// 豁免裁定:GenTableMapper 类级 @InterceptorIgnore(dataPermission)
    /// —— 代码生成元数据非行级。
    pub async fn select_list(
        &self,
        ctx: &DataContext<'_>,
        table_name: Option<&str>,
        table_comment: Option<&str>,
        data_name: Option<&str>,
    ) -> Result<Vec<GenTable>, DataError> {
        let mut query = Entity::find();
        if let Some(name) = data_name.filter(|s| !s.is_empty()) {
            query = query.filter(Column::DataName.eq(name));
        }
        if let Some(name) = table_name.filter(|s| !s.is_empty()) {
            query = query.filter(Column::TableName.contains(name));
        }
        if let Some(comment) = table_comment.filter(|s| !s.is_empty()) {
            query = query.filter(Column::TableComment.contains(comment));
        }
        query = query.order_by_desc(Column::UpdateTime);
        let models = ctx.select(query).without_row_scope().all().await?;
        Ok(models.iter().map(|m| m.into()).collect())
    }

    /// 分页查询(数据库层),返回 (当前页数据, 总数)。
    ///
    /// 豁免裁定:GenTableMapper 类级 @InterceptorIgnore(dataPermission)
    /// —— 代码生成元数据非行级(Java 锚 selectPageGenTableList →
    /// selectVoPage;排序在 Select 构件上,total 为全量口径)。
    /// 参数随入漏斗收敛为 (query BO, page_no, page_size)—— 原
    /// (5 过滤项 + offset + limit) 的 too_many_arguments 豁免一并移除。
    pub async fn select_page(
        &self,
        ctx: &DataContext<'_>,
        query: &GenTableQueryBo,
        page_no: u64,
        page_size: u64,
    ) -> Result<(Vec<GenTable>, u64), DataError> {
        let mut select = Entity::find();
        if let Some(name) = query.data_name.as_deref().filter(|s| !s.is_empty())
        {
            select = select.filter(Column::DataName.eq(name));
        }
        if let Some(name) =
            query.table_name.as_deref().filter(|s| !s.is_empty())
        {
            select = select.filter(Column::TableName.contains(name));
        }
        if let Some(comment) =
            query.table_comment.as_deref().filter(|s| !s.is_empty())
        {
            select = select.filter(Column::TableComment.contains(comment));
        }
        if let Some(begin) =
            query.begin_time.as_deref().filter(|s| !s.is_empty())
            && let Ok(dt) = chrono::NaiveDateTime::parse_from_str(
                begin,
                "%Y-%m-%d %H:%M:%S",
            )
        {
            select = select.filter(Column::CreateTime.gte(dt.and_utc()));
        }
        if let Some(end) = query.end_time.as_deref().filter(|s| !s.is_empty())
            && let Ok(dt) =
                chrono::NaiveDateTime::parse_from_str(end, "%Y-%m-%d %H:%M:%S")
        {
            select = select.filter(Column::CreateTime.lte(dt.and_utc()));
        }
        select = select.order_by_desc(Column::UpdateTime);

        let page = ctx
            .select(select)
            .without_row_scope()
            .page(page_no as i64, page_size as i64)
            .await?;

        Ok((
            page.rows.iter().map(|m| m.into()).collect(),
            page.total as u64,
        ))
    }

    /// 新增元数据行。审计五列由漏斗 insert 统一盖章(principal 取自 ctx)
    /// (Java 锚 importGenTable)。
    pub async fn insert(
        &self,
        ctx: &DataContext<'_>,
        table: &GenTable,
    ) -> Result<(), DataError> {
        let active = gen_table_to_active_model(table);
        ctx.insert_returning_nothing(active).await?;
        Ok(())
    }

    /// 按主键更新。漏斗 update:审计盖 update_*(create_* 搬运领域层
    /// 原值),WHERE = 主键(豁免实体),0 行 → NotFound(原
    /// RecordNotUpdated 冒 500 的语义替代;Java 锚 updateGenTable)。
    pub async fn update_by_id(
        &self,
        ctx: &DataContext<'_>,
        table: &GenTable,
    ) -> Result<(), DataError> {
        let active = gen_table_to_active_model(table);
        ctx.update(active).await?;
        Ok(())
    }

    /// 按主键集批量物理删(gen_table 无软删列,Java 侧同为物理删)。
    /// 豁免实体 → `ctx.delete_where`(原 delete_many 对位);0 行 → Ok,
    /// 批量语义无 NotFound 约定(Java 锚 deleteGenTableByIds)。
    pub async fn delete_by_ids(
        &self,
        ctx: &DataContext<'_>,
        ids: &[i64],
    ) -> Result<(), DataError> {
        if ids.is_empty() {
            return Ok(());
        }
        ctx.delete_where::<Entity>(
            Condition::all().add(Column::TableId.is_in(ids.iter().copied())),
        )
        .await?;
        Ok(())
    }

    /// 已导入表名列表(importTable 的排重读)。
    ///
    /// 豁免裁定:GenTableMapper 类级 @InterceptorIgnore(dataPermission)
    /// —— 代码生成元数据非行级(排重须全量;Java 锚 importGenTable 的
    /// 已导入校验半边)。
    pub async fn select_imported_table_names(
        &self,
        ctx: &DataContext<'_>,
        data_name: &str,
    ) -> Result<Vec<String>, DataError> {
        let models = ctx
            .select(Entity::find().filter(Column::DataName.eq(data_name)))
            .without_row_scope()
            .all()
            .await?;
        Ok(models.iter().map(|m| m.table_name.clone()).collect())
    }
}
