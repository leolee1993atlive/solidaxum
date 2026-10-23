//! gen_table_column 仓储(**全豁免**)。
//!
//! 漏斗公共契约(执行面/错误)见 `app_data` 模块文档。裁定与
//! gen_table_repo 同锚:现网 `GenTableColumnMapper.java` 类级标注
//! `@InterceptorIgnore(dataPermission = "true")`(零 `@DataPermission`
//! 方法注解)—— **GenTableColumnMapper 类级 @InterceptorIgnore
//! (dataPermission) —— 代码生成元数据非行级**。由此每个读方法显式
//! `without_row_scope()`;gen_table_column 的 DataPolicy 落
//! CONSTRAINED=false,无软删列。
//!
//! | 本文件方法 | Java 锚(GenTableServiceImpl → GenTableColumnMapper) | @DataPermission | 漏斗路径 |
//! |---|---|---|---|
//! | `select_by_table_id` | selectDbTableColumnsByName / synchDb 的现有列读 | ❌(类级 Ignore) | `without_row_scope()` |
//! | `insert_batch` | importGenTable / synchDb 的列批量插 | — | `ctx.insert_many`(审计五列漏斗盖) |
//! | `update_batch` | updateGenTable 的列批量改 | — | 逐行 `ctx.update`(审计盖 update_*,0 行 → NotFound) |
//! | `delete_by_table_ids` | deleteGenTableByIds / synchDb 的级联清列 | — | `ctx.delete_where`(豁免实体;0 行 → Ok) |
//! | `delete_by_ids` | 按列主键集删(当前生产无消费,保留对位) | — | `ctx.delete_where`(同上) |
//!
//! **批量原语拆分**:`insert_many` 是单语句批量插(单条多值 INSERT,
//! 不回读);逐行(update)与整批
//! (insert_many/delete_where)的选择与原语义逐点一致。

use app_core::model::entity::BaseEntity;
use app_data::{
    DataContext, DataError,
    sea_orm::{
        self, ActiveModelBehavior, ActiveValue, ColumnTrait, Condition,
        DateTimeUtc, DeriveEntityModel, DerivePrimaryKey, DeriveRelation,
        EntityTrait, EnumIter, PrimaryKeyTrait, QueryFilter, QueryOrder,
    },
};

use crate::codegen::model::gen_column::GenTableColumn;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "gen_table_column")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub column_id: i64,
    pub table_id: i64,
    pub column_name: String,
    pub column_comment: Option<String>,
    pub column_type: String,
    pub java_type: String,
    pub java_field: String,
    pub is_pk: String,
    pub is_increment: String,
    pub is_required: String,
    pub is_insert: String,
    pub is_edit: String,
    pub is_list: String,
    pub is_query: String,
    pub query_type: String,
    pub html_type: String,
    pub dict_type: Option<String>,
    pub sort: i32,
    pub create_dept: Option<i64>,
    pub create_by: Option<i64>,
    pub create_time: Option<DateTimeUtc>,
    pub update_by: Option<i64>,
    pub update_time: Option<DateTimeUtc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

impl From<&Model> for GenTableColumn {
    fn from(m: &Model) -> Self {
        GenTableColumn {
            column_id: m.column_id,
            table_id: m.table_id,
            column_name: m.column_name.clone(),
            column_comment: m.column_comment.clone(),
            column_type: m.column_type.clone(),
            java_type: m.java_type.clone(),
            java_field: m.java_field.clone(),
            is_pk: m.is_pk.clone(),
            is_increment: m.is_increment.clone(),
            is_required: m.is_required.clone(),
            is_insert: m.is_insert.clone(),
            is_edit: m.is_edit.clone(),
            is_list: m.is_list.clone(),
            is_query: m.is_query.clone(),
            query_type: m.query_type.clone(),
            html_type: m.html_type.clone(),
            dict_type: m.dict_type.clone(),
            sort: m.sort,
            cap_java_field: None,
            super_column: None,
            usable_column: None,
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

/// GenTableColumn → ActiveModel。审计五列只搬运领域层已持有的值;
/// **写入时由漏斗统一盖章**(insert 全盖、update 盖 update_*)。
fn gen_column_to_active_model(column: &GenTableColumn) -> ActiveModel {
    ActiveModel {
        column_id: ActiveValue::Set(column.column_id),
        table_id: ActiveValue::Set(column.table_id),
        column_name: ActiveValue::Set(column.column_name.clone()),
        column_comment: ActiveValue::Set(column.column_comment.clone()),
        column_type: ActiveValue::Set(column.column_type.clone()),
        java_type: ActiveValue::Set(column.java_type.clone()),
        java_field: ActiveValue::Set(column.java_field.clone()),
        is_pk: ActiveValue::Set(column.is_pk.clone()),
        is_increment: ActiveValue::Set(column.is_increment.clone()),
        is_required: ActiveValue::Set(column.is_required.clone()),
        is_insert: ActiveValue::Set(column.is_insert.clone()),
        is_edit: ActiveValue::Set(column.is_edit.clone()),
        is_list: ActiveValue::Set(column.is_list.clone()),
        is_query: ActiveValue::Set(column.is_query.clone()),
        query_type: ActiveValue::Set(column.query_type.clone()),
        html_type: ActiveValue::Set(column.html_type.clone()),
        dict_type: ActiveValue::Set(column.dict_type.clone()),
        sort: ActiveValue::Set(column.sort),
        create_dept: ActiveValue::Set(column.base_entity.create_dept),
        create_by: ActiveValue::Set(column.base_entity.create_by),
        create_time: ActiveValue::Set(column.base_entity.create_time),
        update_by: ActiveValue::Set(column.base_entity.update_by),
        update_time: ActiveValue::Set(column.base_entity.update_time),
    }
}

#[derive(Clone, Default)]
pub struct GenColumnRepository {
    // 无状态仓储(dept 同型):执行面全在调用方传入的 ctx。
}

impl GenColumnRepository {
    pub fn new() -> Self {
        Self::default()
    }

    /// 按表查全部列(sort / column_id 升序)。
    ///
    /// 豁免裁定:GenTableColumnMapper 类级 @InterceptorIgnore
    /// (dataPermission)—— 代码生成元数据非行级。
    pub async fn select_by_table_id(
        &self,
        ctx: &DataContext<'_>,
        table_id: i64,
    ) -> Result<Vec<GenTableColumn>, DataError> {
        let models = ctx
            .select(
                Entity::find()
                    .filter(Column::TableId.eq(table_id))
                    .order_by_asc(Column::Sort)
                    .order_by_asc(Column::ColumnId),
            )
            .without_row_scope()
            .all()
            .await?;
        Ok(models.iter().map(|m| m.into()).collect())
    }

    /// 批量插入列(原 insert_many 单语句)。`ctx.insert_many` 单条多值
    /// INSERT,不回读;审计五列漏斗盖。
    pub async fn insert_batch(
        &self,
        ctx: &DataContext<'_>,
        columns: &[GenTableColumn],
    ) -> Result<(), DataError> {
        ctx.insert_many(
            columns.iter().map(gen_column_to_active_model).collect(),
        )
        .await?;
        Ok(())
    }

    /// 批量更新列。逐行 `ctx.update`:审计盖 update_*,WHERE = 主键
    /// (豁免实体),0 行 → NotFound。
    pub async fn update_batch(
        &self,
        ctx: &DataContext<'_>,
        columns: &[GenTableColumn],
    ) -> Result<(), DataError> {
        for col in columns {
            ctx.update(gen_column_to_active_model(col)).await?;
        }
        Ok(())
    }

    /// 按表主键集级联清列(物理删,无软删列)。
    /// 豁免实体 → `ctx.delete_where`;0 行 → Ok,批量语义无 NotFound 约定。
    pub async fn delete_by_table_ids(
        &self,
        ctx: &DataContext<'_>,
        table_ids: &[i64],
    ) -> Result<(), DataError> {
        if table_ids.is_empty() {
            return Ok(());
        }
        ctx.delete_where::<Entity>(
            Condition::all()
                .add(Column::TableId.is_in(table_ids.iter().copied())),
        )
        .await?;
        Ok(())
    }

    /// 按列主键集物理删。裁定同 [`Self::delete_by_table_ids`]。
    pub async fn delete_by_ids(
        &self,
        ctx: &DataContext<'_>,
        column_ids: &[i64],
    ) -> Result<(), DataError> {
        if column_ids.is_empty() {
            return Ok(());
        }
        ctx.delete_where::<Entity>(
            Condition::all()
                .add(Column::ColumnId.is_in(column_ids.iter().copied())),
        )
        .await?;
        Ok(())
    }
}
