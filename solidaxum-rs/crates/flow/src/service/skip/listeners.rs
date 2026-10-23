use app_core::error::AppResult;

use crate::{
    listener::{
        ListenerRegistry, trigger::trigger_listeners, types::ListenerVariable,
    },
    model::{definition::Definition, node::Node},
};

/// 依次触发定义级、节点级监听器。
///
/// 触发顺序是契约,`engine_test::listener_fire_order_is_start_assignment_finish_create`
/// 在守着它。
pub(crate) async fn fire_def_and_node(
    registry: &ListenerRegistry,
    definition: &Definition,
    node: &Node,
    event: &str,
    lv: &mut ListenerVariable,
) -> AppResult<()> {
    trigger_listeners(
        registry,
        definition.listener_type.as_deref(),
        definition.listener_path.as_deref(),
        event,
        lv,
    )
    .await?;
    trigger_listeners(
        registry,
        node.listener_type.as_deref(),
        node.listener_path.as_deref(),
        event,
        lv,
    )
    .await
}
