//! flow_skip 仓储(**全豁免模块**)。
//!
//! 统一锚:**warm-flow/ruoyi-workflow Mapper 零 @DataPermission —— 工作流
//! 数据非行级**(普查,warm-flow 全仓 grep 计数 = 0),每个读方法
//! 显式 `without_row_scope()`;DataPolicy 见 `crate::policy`(豁免 +
//! soft_delete=(DelFlag,"0") + 审计列 None —— A 组类型不匹配,手工盖)。
//!
//! | 本文件方法 | Java 锚 | @DataPermission | 漏斗路径 |
//! |---|---|---|---|
//! | `select_by_definition_id` | `DefServiceImpl.getAllDataDefinition:167`(`skipService().getByDefId`)/ `getFlowCombineNoDef:177` | ❌ | `without_row_scope()` |
//! | `select_by_definition_id_and_now_node_code` | task_service 按钮计算(`SkipService` 按出边查询同语义) | ❌ | `without_row_scope()` |
//! | `select_by_id` | `FlowNodeConfServiceImpl.updateSkipCondition:74` 的 `getById` 存在性守卫(对齐 Java `SkipService.getById`) | ❌ | `without_row_scope()` |
//! | `update_skip_condition` | `FlowNodeConfServiceImpl.updateSkipCondition:81`(显式 set,置空生效) | ❌ | `ctx.update_where`(WHERE = id + 软删存活) |
//! | `insert_batch` | `DefServiceImpl.saveDef:148` `skipService().saveBatch(allSkips)` / `copyDef` 链 `:278` | — | `tx.insert_many`(单条多值 INSERT,不回读) |
//! | `soft_delete_by_definition_id` | `DefServiceImpl.saveDef:133` `skipService().remove(...)` + `@TableLogic(value="0", delval="1")`(FlowSkip.java:73) | — | `tx.soft_delete_where`(WHERE = definition_id + 存活;不盖审计,同现状) |
//!
//! **删除一律逻辑删**:`remove(...)` 在 `@TableLogic` 实体上是逻辑删,
//! `delete_many` 硬删与 Java 锚不符(详见 node_repo.rs 头注同款说明),
//! 故本文件只提供 `soft_delete_by_definition_id`。

use app_data::{
    DataContext, DataError, TxnContext,
    sea_orm::{
        ActiveValue::{NotSet, Set},
        ColumnTrait, Condition, EntityTrait, QueryFilter,
    },
};

use crate::{
    model::skip::Skip,
    repository::entity::flow_skip::{self, Column, Entity},
};

fn skip_to_active_model(skip: &Skip) -> flow_skip::ActiveModel {
    flow_skip::ActiveModel {
        id: match skip.id {
            Some(id) => Set(id),
            None => NotSet,
        },
        definition_id: Set(skip.definition_id),
        now_node_code: Set(skip.now_node_code.clone()),
        now_node_type: Set(skip.now_node_type),
        next_node_code: Set(skip.next_node_code.clone()),
        next_node_type: Set(skip.next_node_type),
        // node_id 为 #[sea_orm(ignore)] 字段，ActiveModel 不含此列，不参与 INSERT/UPDATE
        skip_name: Set(skip.skip_name.clone()),
        skip_type: Set(skip.skip_type.clone()),
        skip_condition: Set(skip.skip_condition.clone()),
        coordinate: Set(skip.coordinate.clone()),
        create_time: Set(skip.create_time),
        create_by: Set(skip.create_by.clone()),
        update_time: Set(skip.update_time),
        update_by: Set(skip.update_by.clone()),
        del_flag: Set(skip.del_flag.clone()),
        tenant_id: Set(skip.tenant_id.clone()),
    }
}

/// 流程跳转数据访问(无状态关联函数,执行面见文件头审计表)
pub struct SkipRepository;

impl SkipRepository {
    pub async fn select_by_definition_id(
        ctx: &DataContext<'_>,
        definition_id: i64,
    ) -> Result<Vec<Skip>, DataError> {
        let models = ctx
            .select(
                Entity::find().filter(Column::DefinitionId.eq(definition_id)),
            )
            .without_row_scope()
            .all()
            .await?;
        Ok(models.iter().map(Skip::from).collect())
    }

    pub async fn select_by_definition_id_and_now_node_code(
        ctx: &DataContext<'_>,
        definition_id: i64,
        now_node_code: &str,
    ) -> Result<Vec<Skip>, DataError> {
        let models = ctx
            .select(
                Entity::find()
                    .filter(Column::DefinitionId.eq(definition_id))
                    .filter(Column::NowNodeCode.eq(now_node_code)),
            )
            .without_row_scope()
            .all()
            .await?;
        Ok(models.iter().map(Skip::from).collect())
    }

    /// 按主键查询单条跳线(nodeConf 单点更新的存在性守卫用)。
    pub async fn select_by_id(
        ctx: &DataContext<'_>,
        id: i64,
    ) -> Result<Option<Skip>, DataError> {
        let model = ctx
            .select(Entity::find_by_id(id))
            .without_row_scope()
            .one()
            .await?;
        Ok(model.as_ref().map(Skip::from))
    }

    /// 单点更新连线跳转条件(对齐参考实现 FlowNodeConfServiceImpl
    /// .updateSkipCondition:显式 set,Set(None) 置空 = 无条件通过)。
    /// 连线不存在 → 0 行,由 service 层判 404。
    pub async fn update_skip_condition(
        ctx: &DataContext<'_>,
        skip_id: i64,
        skip_condition: Option<String>,
    ) -> Result<u64, DataError> {
        ctx.update_where::<Entity>(
            Condition::all().add(Column::Id.eq(skip_id)),
            flow_skip::ActiveModel {
                skip_condition: Set(skip_condition),
                ..Default::default()
            },
        )
        .await
    }

    /// 批量插入(事务路径:save_def / copy_def / save_def_json 的写闭包内)。
    /// `tx.insert_many` 单条多值 INSERT,不回读。
    pub async fn insert_batch(
        tx: &TxnContext<'_>,
        skips: &[Skip],
    ) -> Result<(), DataError> {
        tx.insert_many(skips.iter().map(skip_to_active_model).collect())
            .await?;
        Ok(())
    }

    /// 按定义软删全部跳转(事务路径,同 node_repo 同名函数)。
    pub async fn soft_delete_by_definition_id(
        tx: &TxnContext<'_>,
        definition_id: i64,
    ) -> Result<u64, DataError> {
        tx.soft_delete_where::<Entity>(
            Condition::all().add(Column::DefinitionId.eq(definition_id)),
            "1",
        )
        .await
    }
}
