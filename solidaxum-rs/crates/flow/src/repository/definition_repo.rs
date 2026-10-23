//! flow_definition 仓储(**全豁免模块**)。
//!
//! 漏斗公共契约(执行面/错误)见 `app_data` 模块文档。**方法级裁定(Java 基线逐方法核对)**:
//! warm-flow `FlowDefinitionMapper`(mybatis / mybatis-plus 双实现)与
//! ruoyi-workflow `FlwDefinitionServiceImpl` 的全部 Mapper 零 `@DataPermission`
//! (普查:warm-flow 全仓 grep 计数 = 0,ruoyi-workflow 8 个 Mapper
//! 同口径)—— 工作流数据非行级。统一锚:**warm-flow/ruoyi-workflow Mapper
//! 零 @DataPermission —— 工作流数据非行级**。由此每个读方法显式
//! `without_row_scope()`(豁免必须写出这行字才能过审,不做静默默认);
//! DataPolicy 见 `crate::policy`(CONSTRAINED=false、soft_delete=(DelFlag,"0")、
//! audit_columns=None —— A 组 create_by/update_by 是 `Option<String>`、时间列
//! `Option<DateTime>`,与漏斗盖章签名(i64/DateTimeUtc)不匹配,审计由 service
//! 手工盖,同 Java DataFillHandler 之前的业务显式 set)。
//!
//! | 本文件方法 | Java 锚 | @DataPermission | 漏斗路径 |
//! |---|---|---|---|
//! | `select_by_id` | `FlwDefinitionServiceImpl.publish:122` 前置 getById / warm-flow `DefServiceImpl.getAllDataDefinition:163` | ❌ | `without_row_scope()` |
//! | `select_by_flow_code_and_version` | definition_service 新建版本冲突预检(对齐 `DefServiceImpl.checkAndSave:103` 的存在性检查) | ❌ | `without_row_scope()`(唯一性须全量) |
//! | `select_by_flow_code` | `DefServiceImpl.saveDef:108`(设计器 save-json 取既有定义) | ❌ | `without_row_scope()` |
//! | `select_published_by_flow_code` | `FlwDefinitionServiceImpl.publish:122`(同 flow_code 已发布/过期定义降级) | ❌ | `without_row_scope()` |
//! | `select_def_ids_with_instances` | `FlwDefinitionServiceImpl.publish:122` 的过期判定(有实例 → isPublish=9 Expired) | ❌ | `without_row_scope()`;join 改整行读映射 id(逃生舱无 into_tuple 终端,dept 同款裁定;重复 id 与原 select_only 口径一致,调用方仅 contains) |
//! | `select_page` | `FlwDefinitionServiceImpl.queryList:72` | ❌ | 逃生舱 `page` + `without_row_scope()`(ORDER BY 留构件,设计 6.4 裁定) |
//! | `select_unpublished_page` | `FlwDefinitionServiceImpl.unPublishList:88` | ❌ | 同上 |
//! | `insert` | `DefServiceImpl.checkAndSave:103` / `saveDef:108` 新建分支 | — | `tx.insert`(A 组审计列漏斗不盖,service 手工盖) |
//! | `update_by_id` | `DefServiceImpl.saveDef:108` 更新分支 / unPublish(DefService:170) | — | `ctx.update`(0 行 → NotFound,见下) |
//! | `update_by_id_in_txn` | `FlwDefinitionServiceImpl.publish:122` / `removeDef:188`(同事务多写) | — | `tx.update`(事务路径,语义同上) |
//! | `soft_delete_by_id` | `FlwDefinitionServiceImpl.removeDef:188` + `@TableLogic(value="0", delval="1")`(FlowDefinition.java:76) | — | `ctx.soft_delete_where`(工具面,生产零调用) |
//!
//! **写路径说明**:Java 写方法本就无注解(拦截器只改写查询),flow_definition
//! 是豁免实体(CONSTRAINED=false),漏斗 update 对它不施加行级范围。漏斗
//! 接管带来两点收紧:① update 的 WHERE 追加软删存活 —— 无此过滤时对已删行
//! update 会把 del_flag 覆盖回 ActiveModel 携带的值(形同复活已删定义),
//! 故 0 行 → NotFound;② 0 行语义统一为 `DataError::NotFound`
//! (404「数据不存在」)。

use app_data::{
    DataContext, DataError, TxnContext,
    sea_orm::{
        ActiveValue::{NotSet, Set},
        ColumnTrait, Condition, EntityTrait, JoinType, Order, QueryFilter,
        QueryOrder, QuerySelect, RelationTrait, Select,
    },
};

use crate::{
    enums::PublishStatus,
    model::definition::Definition,
    repository::entity::flow_definition::{self, Column, Entity, Relation},
};

fn apply_filters(
    mut select: Select<Entity>,
    flow_code: Option<&str>,
    flow_name: Option<&str>,
    category: Option<&str>,
) -> Select<Entity> {
    if let Some(code) = flow_code
        && !code.is_empty()
    {
        select = select.filter(Column::FlowCode.contains(code));
    }
    if let Some(name) = flow_name
        && !name.is_empty()
    {
        select = select.filter(Column::FlowName.contains(name));
    }
    if let Some(cat) = category
        && !cat.is_empty()
    {
        select = select.filter(Column::Category.contains(cat));
    }

    select.order_by(Column::CreateTime, Order::Desc)
}

fn def_to_active_model(def: &Definition) -> flow_definition::ActiveModel {
    flow_definition::ActiveModel {
        id: match def.id {
            Some(id) => Set(id),
            None => NotSet,
        },
        flow_code: Set(def.flow_code.clone()),
        flow_name: Set(def.flow_name.clone()),
        model_value: Set(def.model_value.clone()),
        category: Set(def.category.clone()),
        version: Set(def.version.clone()),
        is_publish: Set(def.is_publish),
        form_custom: Set(def.form_custom.clone()),
        form_path: Set(def.form_path.clone()),
        activity_status: Set(def.activity_status),
        listener_type: Set(def.listener_type.clone()),
        listener_path: Set(def.listener_path.clone()),
        ext: Set(def.ext.clone()),
        create_time: Set(def.create_time),
        create_by: Set(def.create_by.clone()),
        update_time: Set(def.update_time),
        update_by: Set(def.update_by.clone()),
        del_flag: Set(def.del_flag.clone()),
        tenant_id: Set(def.tenant_id.clone()),
    }
}

/// 流程定义数据访问(无状态关联函数;连接路径收 `&DataContext`,
/// 事务路径收 `&TxnContext`,审计手工盖 —— 见文件头审计表)
pub struct DefinitionRepository;

impl DefinitionRepository {
    pub async fn select_by_id(
        ctx: &DataContext<'_>,
        id: i64,
    ) -> Result<Option<Definition>, DataError> {
        let model = ctx
            .select(Entity::find_by_id(id))
            .without_row_scope()
            .one()
            .await?;
        Ok(model.as_ref().map(Definition::from))
    }

    pub async fn select_by_flow_code_and_version(
        ctx: &DataContext<'_>,
        flow_code: &str,
        version: &str,
    ) -> Result<Option<Definition>, DataError> {
        let model = ctx
            .select(
                Entity::find()
                    .filter(Column::FlowCode.eq(flow_code))
                    .filter(Column::Version.eq(version)),
            )
            .without_row_scope()
            .one()
            .await?;
        Ok(model.as_ref().map(Definition::from))
    }

    /// 按 flow_code 查询最新一条未删除的定义（不限制版本）
    pub async fn select_by_flow_code(
        ctx: &DataContext<'_>,
        flow_code: &str,
    ) -> Result<Option<Definition>, DataError> {
        let model = ctx
            .select(
                Entity::find()
                    .filter(Column::FlowCode.eq(flow_code))
                    .order_by(Column::CreateTime, Order::Desc),
            )
            .without_row_scope()
            .one()
            .await?;
        Ok(model.as_ref().map(Definition::from))
    }

    /// 查询指定 flow_code 下所有已发布或已过期的定义
    pub async fn select_published_by_flow_code(
        ctx: &DataContext<'_>,
        flow_code: &str,
    ) -> Result<Vec<Definition>, DataError> {
        let models = ctx
            .select(
                Entity::find()
                    .filter(Column::FlowCode.eq(flow_code))
                    .filter(
                        Condition::any()
                            .add(Column::IsPublish.eq(1_i8))
                            .add(Column::IsPublish.eq(9_i8)),
                    ),
            )
            .without_row_scope()
            .all()
            .await?;
        Ok(models.iter().map(Definition::from).collect())
    }

    /// 查询同 flow_code 下是否有运行中实例关联的定义，返回 definition_id 集合
    pub async fn select_def_ids_with_instances(
        ctx: &DataContext<'_>,
        flow_code: &str,
    ) -> Result<Vec<i64>, DataError> {
        let models = ctx
            .select(
                Entity::find()
                    .filter(Column::FlowCode.eq(flow_code))
                    .filter(
                        Condition::any()
                            .add(Column::IsPublish.eq(1_i8))
                            .add(Column::IsPublish.eq(9_i8)),
                    )
                    .join(JoinType::InnerJoin, Relation::Instance.def()),
            )
            .without_row_scope()
            .all()
            .await?;
        Ok(models.iter().map(|m| m.id).collect())
    }

    /// 插入(事务路径:save_def / copy_def / save_def_json 的写闭包内)。
    pub async fn insert(
        tx: &TxnContext<'_>,
        def: &Definition,
    ) -> Result<(), DataError> {
        tx.insert_returning_nothing(def_to_active_model(def))
            .await?;
        Ok(())
    }

    /// 按主键更新(连接路径:unpublish / set_active)。
    pub async fn update_by_id(
        ctx: &DataContext<'_>,
        def: &Definition,
    ) -> Result<(), DataError> {
        ctx.update(def_to_active_model(def)).await?;
        Ok(())
    }

    /// [`Self::update_by_id`] 的事务内变体(save_def / publish /
    /// save_def_json / delete_def(s) 的写闭包内)。
    pub async fn update_by_id_in_txn(
        tx: &TxnContext<'_>,
        def: &Definition,
    ) -> Result<(), DataError> {
        tx.update(def_to_active_model(def)).await?;
        Ok(())
    }

    /// 按主键软删(连接路径,工具面:生产零调用,menu `delete_by_id` 同款
    /// 保留)。`soft_delete_where` 的 WHERE 自带软删存活过滤(@TableLogic
    /// 平价,已删行不重复计数),不盖审计 —— update_* 由调用方手工盖(同现状)。
    pub async fn soft_delete_by_id(
        ctx: &DataContext<'_>,
        id: i64,
    ) -> Result<u64, DataError> {
        ctx.soft_delete_where::<Entity>(
            Condition::all().add(Column::Id.eq(id)),
            "1",
        )
        .await
    }

    pub async fn select_page(
        ctx: &DataContext<'_>,
        flow_code: Option<&str>,
        flow_name: Option<&str>,
        category: Option<&str>,
        page_no: u64,
        page_size: u64,
    ) -> Result<app_data::Page<Definition>, DataError> {
        let select = Entity::find()
            .filter(Column::IsPublish.eq(PublishStatus::Published as i8));
        let select = apply_filters(select, flow_code, flow_name, category);
        let page = ctx
            .select(select)
            .without_row_scope()
            .page(page_no as i64, page_size as i64)
            .await?;
        let rows: Vec<Definition> =
            page.rows.iter().map(Definition::from).collect();
        Ok(page.with_rows(rows))
    }

    /// 查询未发布流程定义分页列表
    pub async fn select_unpublished_page(
        ctx: &DataContext<'_>,
        flow_code: Option<&str>,
        flow_name: Option<&str>,
        category: Option<&str>,
        page_no: u64,
        page_size: u64,
    ) -> Result<app_data::Page<Definition>, DataError> {
        let select = Entity::find()
            .filter(Column::IsPublish.eq(PublishStatus::Unpublished as i8));
        let select = apply_filters(select, flow_code, flow_name, category);
        let page = ctx
            .select(select)
            .without_row_scope()
            .page(page_no as i64, page_size as i64)
            .await?;
        let rows: Vec<Definition> =
            page.rows.iter().map(Definition::from).collect();
        Ok(page.with_rows(rows))
    }
}
