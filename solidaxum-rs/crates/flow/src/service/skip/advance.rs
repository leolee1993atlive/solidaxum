use app_core::error::{AppError, AppResult};

use crate::{
    engine::{graph, runtime::pending_join_branches, transition::PathWayData},
    enums::{NodeType, SkipType},
    model::node::Node,
    service::skip::context::SkipContext,
};

/// 流转的去向。
pub(crate) enum Advance {
    /// 目标是汇聚网关,但仍有前驱分支未到达：本分支就此结束,不创建下游任务。
    JoinPending,
    /// 正常推进到 `next_nodes`。
    To {
        path_way: PathWayData,
        next_nodes: Vec<Node>,
    },
}

/// 决定这次流转往哪走。**只读**——不写任何一张表。
///
/// 在**事务外**执行(守卫提升,形态论证见 task_service 文件头)——不持
/// 事务句柄,活跃任务快照由 `SkipContext::load` 预读;其 BadRequest 文案
/// (无法找到/无法确定下一节点)在闭包外以原文案抛出。
pub(crate) async fn resolve_next(
    sctx: &SkipContext,
    resolver: &dyn crate::resolver::UserResolver,
) -> AppResult<Advance> {
    let mut path_way = PathWayData {
        def_id: sctx.task.definition_id,
        ins_id: Some(sctx.task.instance_id),
        skip_type: Some(SkipType::Pass.as_str().to_string()),
        ..Default::default()
    };

    let next_node = graph::get_next_node(
        &sctx.now_node,
        sctx.params.node_code.as_deref(),
        SkipType::Pass.as_str(),
        &mut path_way,
        &sctx.all_skips,
        &sctx.all_nodes,
    )
    .ok_or_else(|| AppError::BadRequest("无法找到下一节点".to_string()))?;

    // 汇聚网关 AND-join 检测(load 时的活跃任务快照)
    let is_join_gateway = NodeType::from_i8(next_node.node_type)
        .map(|t| t.is_gate_way())
        .unwrap_or(false)
        && sctx
            .all_skips
            .iter()
            .filter(|s| s.next_node_code == next_node.node_code)
            .count()
            > 1;

    if is_join_gateway
        && pending_join_branches(
            &sctx.now_node.node_code,
            &next_node.node_code,
            &sctx.active_tasks,
            &sctx.all_skips,
        )
    {
        return Ok(Advance::JoinPending);
    }

    // 预热 hasRole 角色快照(条件里有 hasRole 时才现查;事务外纯读)
    let spel_ctx = crate::expression::condition::prefetch_spel_context(
        resolver,
        &sctx.all_skips,
        &sctx.variables,
    )
    .await;
    let next_nodes = graph::get_next_by_check_gateway(
        &next_node,
        &sctx.variables,
        &mut path_way,
        &sctx.all_skips,
        &sctx.all_nodes,
        &spel_ctx,
    )?;

    if next_nodes.is_empty() {
        return Err(AppError::BadRequest("无法确定下一节点".to_string()));
    }

    path_way.target_nodes = next_nodes.clone();

    Ok(Advance::To {
        path_way,
        next_nodes,
    })
}
