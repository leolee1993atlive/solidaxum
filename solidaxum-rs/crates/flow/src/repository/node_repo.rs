//! flow_node 仓储(**全豁免模块**)。
//!
//! 统一锚:**warm-flow/ruoyi-workflow Mapper 零 @DataPermission —— 工作流
//! 数据非行级**(普查,warm-flow 全仓 grep 计数 = 0),每个读方法
//! 显式 `without_row_scope()`;DataPolicy 见 `crate::policy`(豁免 +
//! soft_delete=(DelFlag,"0") + 审计列 None —— A 组类型不匹配,手工盖)。
//!
//! | 本文件方法 | Java 锚 | @DataPermission | 漏斗路径 |
//! |---|---|---|---|
//! | `select_by_definition_id` | `DefServiceImpl.getAllDataDefinition:165`(`nodeService().getByDefId`)/ `getFlowCombineNoDef:176` | ❌ | `without_row_scope()` |
//! | `select_by_definition_id_and_node_code` | 工具面(生产零调用;`NodeService.getNode` 同语义) | ❌ | `without_row_scope()` |
//! | `update_permission_flag` | `FlowNodeConfServiceImpl.updatePermissionFlag:62`(显式 set,置空生效;整图重建 save-json 之外的单点运维面) | ❌ | `ctx.update_where`(WHERE = definition_id + node_code + 软删存活) |
//! | `insert_batch` | `DefServiceImpl.saveDef:147` `nodeService().saveBatch(allNodes)` / `copyDef` 链 `:275` | — | `tx.insert_many`(单条多值 INSERT,不回读) |
//! | `soft_delete_by_definition_id` | `DefServiceImpl.saveDef:132` `nodeService().remove(...)` + `@TableLogic(value="0", delval="1")`(FlowNode.java:81) | — | `tx.soft_delete_where`(WHERE = definition_id + 存活;不盖审计,同现状) |
//!
//! **删除一律逻辑删**:`DefServiceImpl.saveDef:132` 的 `remove(...)` 在
//! `@TableLogic` 实体上是**逻辑删**(UPDATE del_flag='1'),`delete_many`
//! 硬删与 Java 锚不符,故本文件只提供 `soft_delete_by_definition_id`
//! (读侧本就全滤 del_flag='0',逻辑删的行为差异仅是墓碑行不再被
//! 物理清除)。

use app_data::{
    DataContext, DataError, TxnContext,
    sea_orm::{
        ActiveValue::{NotSet, Set},
        ColumnTrait, Condition, EntityTrait, QueryFilter,
    },
};

use crate::{
    model::node::Node,
    repository::entity::flow_node::{self, Column, Entity},
};

fn node_to_active_model(node: &Node) -> flow_node::ActiveModel {
    flow_node::ActiveModel {
        id: match node.id {
            Some(id) => Set(id),
            None => NotSet,
        },
        node_type: Set(node.node_type),
        definition_id: Set(node.definition_id),
        node_code: Set(node.node_code.clone()),
        node_name: Set(node.node_name.clone()),
        permission_flag: Set(node.permission_flag.clone()),
        node_ratio: Set(node.node_ratio.clone()),
        coordinate: Set(node.coordinate.clone()),
        any_node_skip: Set(node.any_node_skip.clone()),
        listener_type: Set(node.listener_type.clone()),
        listener_path: Set(node.listener_path.clone()),
        form_custom: Set(node.form_custom.clone()),
        form_path: Set(node.form_path.clone()),
        version: Set(node.version.clone()),
        create_time: Set(node.create_time),
        create_by: Set(node.create_by.clone()),
        update_time: Set(node.update_time),
        update_by: Set(node.update_by.clone()),
        ext: Set(node.ext.clone()),
        del_flag: Set(node.del_flag.clone()),
        tenant_id: Set(node.tenant_id.clone()),
    }
}

/// 流程节点数据访问(无状态关联函数,执行面见文件头审计表)
pub struct NodeRepository;

impl NodeRepository {
    pub async fn select_by_definition_id(
        ctx: &DataContext<'_>,
        definition_id: i64,
    ) -> Result<Vec<Node>, DataError> {
        let models = ctx
            .select(
                Entity::find().filter(Column::DefinitionId.eq(definition_id)),
            )
            .without_row_scope()
            .all()
            .await?;
        Ok(models.iter().map(Node::from).collect())
    }

    pub async fn select_by_definition_id_and_node_code(
        ctx: &DataContext<'_>,
        definition_id: i64,
        node_code: &str,
    ) -> Result<Option<Node>, DataError> {
        let model = ctx
            .select(
                Entity::find()
                    .filter(Column::DefinitionId.eq(definition_id))
                    .filter(Column::NodeCode.eq(node_code)),
            )
            .without_row_scope()
            .one()
            .await?;
        Ok(model.as_ref().map(Node::from))
    }

    /// 单点更新节点办理人权限标识(对齐参考实现 FlowNodeConfServiceImpl
    /// .updatePermissionFlag 的显式 set:支持 Set(None) 置空,这是
    /// 整图重建 save-json 之外的实施运维面)。作用于已发布定义的存量行,
    /// 不动定义版本。节点不存在 → 0 行,由 service 层判 404。
    pub async fn update_permission_flag(
        ctx: &DataContext<'_>,
        definition_id: i64,
        node_code: &str,
        permission_flag: Option<String>,
    ) -> Result<u64, DataError> {
        ctx.update_where::<Entity>(
            Condition::all()
                .add(Column::DefinitionId.eq(definition_id))
                .add(Column::NodeCode.eq(node_code)),
            flow_node::ActiveModel {
                permission_flag: Set(permission_flag),
                ..Default::default()
            },
        )
        .await
    }

    /// 批量插入(事务路径:save_def / copy_def / save_def_json 的写闭包内)。
    /// `tx.insert_many` 单条多值 INSERT,不回读。
    pub async fn insert_batch(
        tx: &TxnContext<'_>,
        nodes: &[Node],
    ) -> Result<(), DataError> {
        tx.insert_many(nodes.iter().map(node_to_active_model).collect())
            .await?;
        Ok(())
    }

    /// 按定义软删全部节点(事务路径:save_def 更新分支 / delete_def(s) /
    /// save_def_json 的写闭包内)。`soft_delete_where` 自带存活过滤
    /// (@TableLogic 平价),0 行 → Ok(0)(批量语义无 NotFound 约定)。
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
