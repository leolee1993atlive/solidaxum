use std::collections::HashSet;

use app_data::{DataError, TxnContext};

use crate::{
    model::{skip::Skip, task::Task},
    repository::{task_repo::TaskRepository, user_repo::UserRepository},
};

/// 从 start 出发沿 skip 可达的所有节点（含路径上节点，不含 start 自身）
pub fn successor_node_codes(
    start: &str,
    all_skips: &[Skip],
) -> HashSet<String> {
    let mut seen = HashSet::new();
    let mut stack = vec![start.to_string()];
    while let Some(code) = stack.pop() {
        for s in all_skips.iter().filter(|s| s.now_node_code == code) {
            if seen.insert(s.next_node_code.clone()) {
                stack.push(s.next_node_code.clone());
            }
        }
    }
    seen
}

/// 一票否决:逻辑删 instance 中位于 except_node_code 后继集内的活跃任务
/// (含其 flow_user)。事务句柄为 `&TxnContext<'_>`,读/写全走 `_in_txn`
/// canonical 变体 —— 删除是 `soft_delete_*`(@TableLogic 平价,
/// task_repo/user_repo 文件头裁定),非物理删。
pub async fn one_vote_veto(
    tx: &TxnContext<'_>,
    instance_id: i64,
    except_node_code: &str,
    all_skips: &[Skip],
) -> Result<(), DataError> {
    let successors = successor_node_codes(except_node_code, all_skips);
    let tasks =
        TaskRepository::select_by_instance_id_in_txn(tx, instance_id).await?;
    for task in tasks {
        if task.node_code != except_node_code
            && successors.contains(&task.node_code)
        {
            let tid = task.id.unwrap_or(0);
            UserRepository::soft_delete_by_task_id_in_txn(tx, tid).await?;
            TaskRepository::soft_delete_by_id_in_txn(tx, tid).await?;
        }
    }
    Ok(())
}

/// 汇聚网关 AND-join 检测（对齐 Java TaskServiceImpl.isGenerateNewTask）。
///
/// `arriving_node_code`: 当前到达汇聚网关的来源节点。
/// `join_node_code`: 汇聚网关节点（入边 > 1 才算汇聚）。
/// 返回 true 表示仍有其它前驱分支未到达，本分支不应放行下游任务。
pub fn pending_join_branches(
    arriving_node_code: &str,
    join_node_code: &str,
    active_tasks: &[Task],
    all_skips: &[Skip],
) -> bool {
    let predecessors: Vec<&str> = all_skips
        .iter()
        .filter(|s| s.next_node_code == join_node_code)
        .map(|s| s.now_node_code.as_str())
        .collect();
    if predecessors.len() <= 1 {
        return false; // 非汇聚网关
    }
    active_tasks.iter().any(|t| {
        t.node_code != arriving_node_code
            && predecessors.contains(&t.node_code.as_str())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_task(node_code: &str) -> Task {
        Task {
            id: Some(1),
            definition_id: 1,
            instance_id: 1,
            node_code: node_code.to_string(),
            node_name: None,
            node_type: 1,
            flow_status: "1".to_string(),
            form_custom: None,
            form_path: None,
            create_time: None,
            create_by: None,
            update_time: None,
            update_by: None,
            del_flag: None,
            tenant_id: None,
            permission_list: vec![],
        }
    }

    fn make_skip(from: &str, to: &str) -> Skip {
        Skip {
            id: Some(1),
            definition_id: 1,
            now_node_code: from.to_string(),
            now_node_type: None,
            next_node_code: to.to_string(),
            next_node_type: None,
            node_id: None,
            skip_name: None,
            skip_type: None,
            skip_condition: None,
            coordinate: None,
            create_time: None,
            create_by: None,
            update_time: None,
            update_by: None,
            del_flag: None,
            tenant_id: None,
        }
    }

    #[test]
    fn join_blocks_until_last_branch_arrives() {
        // A→gw, B→gw (gw 为汇聚网关，2 条入边)
        let skips = vec![make_skip("A", "gw"), make_skip("B", "gw")];
        // 当前分支从 A 到达 gw，B 仍有活跃任务 → 应阻塞（还有前驱未到）
        let active = vec![make_task("B")];
        assert!(pending_join_branches("A", "gw", &active, &skips));
    }

    #[test]
    fn join_allows_when_last_branch() {
        let skips = vec![make_skip("A", "gw"), make_skip("B", "gw")];
        // 仅当前分支（A）的任务，无其它前驱活跃 → 放行
        let active: Vec<Task> = vec![];
        assert!(!pending_join_branches("A", "gw", &active, &skips));
    }

    #[test]
    fn non_join_gateway_never_blocks() {
        let skips = vec![make_skip("A", "gw")]; // 单入边
        let active = vec![make_task("X")];
        assert!(!pending_join_branches("A", "gw", &active, &skips));
    }

    #[test]
    fn successor_set_excludes_unrelated_branch() {
        // A -> B -> C, A -> X (parallel side branch)
        let skips = vec![
            make_skip("A", "B"),
            make_skip("B", "C"),
            make_skip("A", "X"),
        ];
        let successors = successor_node_codes("B", &skips);
        assert!(successors.contains("C"));
        assert!(!successors.contains("X"));
    }
}
