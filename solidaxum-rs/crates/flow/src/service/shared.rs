//! `task_service` 与 `instance_service` 共用的流程状态自由函数。
//!
//! 这四个函数原本在两个 service 里各存在一份逐字相同的副本。

use std::sync::Arc;

use app_core::error::AppResult;
use app_data::TxnContext;

use crate::{
    enums::{FlowStatus, NodeType, SkipType},
    model::{instance::Instance, node::Node, skip::Skip},
    notifier::FlowNotifier,
    resolver::BusinessStatusCallback,
};

/// 申请人节点的 `node_code`。
///
/// 定义对齐 Java `FlwCommonServiceImpl.applyNodeCode`：开始节点出边指向的
/// **第一个中间节点**（`getFirstBetweenNode`）。用于判断一次退回是否退到了
/// 申请人手上 —— Java 仅在退回目标是该节点时才把业务状态置为 `back`
/// （`WorkflowGlobalListener:209-215`），退到中间节点时流程仍在审批中。
pub(crate) fn apply_node_code(
    all_nodes: &[Node],
    all_skips: &[Skip],
) -> Option<String> {
    let start = all_nodes.iter().find(|n| {
        NodeType::from_i8(n.node_type).is_some_and(|t| t.is_start())
    })?;
    let first_skip = all_skips
        .iter()
        .find(|s| s.now_node_code == start.node_code)?;
    all_nodes
        .iter()
        .find(|n| n.node_code == first_skip.next_node_code)
        .map(|n| n.node_code.clone())
}

/// 根据节点类型和跳转类型推导任务流程状态
pub(crate) fn set_flow_status(node_type: i8, skip_type: &str) -> String {
    match NodeType::from_i8(node_type) {
        Some(NodeType::Start) => FlowStatus::ToBeSubmit.as_str().to_string(),
        Some(NodeType::End) => FlowStatus::Finished.as_str().to_string(),
        _ if skip_type == SkipType::Reject.as_str() => {
            FlowStatus::Reject.as_str().to_string()
        }
        _ => FlowStatus::Approval.as_str().to_string(),
    }
}

/// 流程终态（warm-flow 数字 `flow_status`）到**业务状态**的映射。
///
/// 返回值直接写入业务表的 status 列，因此**必须落在
/// `wf_business_status` 字典的取值集合内**（对齐 Java `BusinessStatusEnum`：
/// cancel / draft / waiting / finish / invalid / back / termination）。
/// 返回字典外的值 = 前端 dict-tag 翻译不到 = 页面显示原码值。
pub(crate) fn final_status_label(flow_status: &str) -> Option<&str> {
    match flow_status {
        // Finished
        "8" => Some("finish"),
        // Terminate —— 终止不是完成,Java 用 TERMINATION
        "4" => Some("termination"),
        // Reject / TaskBack —— Java 用 BACK,字典里没有 "reject"
        "9" | "11" => Some("back"),
        // Nullify / Invalid
        "5" | "10" => Some("invalid"),
        // Cancel / Retrieve（撤销、撤回）
        "6" | "7" => Some("cancel"),
        _ => None,
    }
}

#[cfg(test)]
mod final_status_label_tests {
    use super::final_status_label;

    /// `wf_business_status` 字典的全部取值（sql/seed_dict_wf.sql，对齐 Java
    /// BusinessStatusEnum）。映射产出必须是它的子集，否则前端翻译不到。
    const WF_BUSINESS_STATUS: &[&str] = &[
        "cancel",
        "draft",
        "waiting",
        "finish",
        "invalid",
        "back",
        "termination",
    ];

    #[test]
    fn every_mapped_status_exists_in_wf_business_status_dict() {
        // 覆盖 FlowStatus 全部取值(0..=13),任何有映射的都必须在字典里。
        for code in 0..=13 {
            let code = code.to_string();
            if let Some(status) = final_status_label(&code) {
                assert!(
                    WF_BUSINESS_STATUS.contains(&status),
                    "flow_status={code} 映射出的业务状态 {status:?} 不在 \
                     wf_business_status 字典内,前端会显示原码值"
                );
            }
        }
    }

    #[test]
    fn terminate_is_not_reported_as_finish() {
        assert_eq!(final_status_label("4"), Some("termination"));
        assert_eq!(final_status_label("8"), Some("finish"));
    }

    #[test]
    fn reject_maps_to_back_not_reject() {
        // 历史上这里返回 "reject",而字典里只有 "back" → 请假页显示原码。
        assert_eq!(final_status_label("9"), Some("back"));
        assert_eq!(final_status_label("11"), Some("back"));
    }

    #[test]
    fn revoke_and_cancel_map_to_cancel() {
        assert_eq!(final_status_label("6"), Some("cancel")); // Cancel
        assert_eq!(final_status_label("7"), Some("cancel")); // Retrieve(撤销)
    }

    #[test]
    fn in_progress_statuses_have_no_business_status() {
        // 审批中/待提交等非终态不应回写业务表,否则会把进行中的单据改成终态。
        for code in ["0", "1", "2", "3", "12", "13"] {
            assert_eq!(
                final_status_label(code),
                None,
                "flow_status={code} 非终态,不应产出业务状态"
            );
        }
    }
}

#[cfg(test)]
mod apply_node_code_tests {
    use super::apply_node_code;
    use crate::{
        enums::NodeType,
        model::{node::Node, skip::Skip},
    };

    fn mk_node(node_type: NodeType, code: &str) -> Node {
        Node {
            id: None,
            node_type: node_type as i8,
            definition_id: 1,
            node_code: code.to_string(),
            node_name: Some(code.to_string()),
            permission_flag: None,
            node_ratio: None,
            coordinate: None,
            any_node_skip: None,
            listener_type: None,
            listener_path: None,
            form_custom: None,
            form_path: None,
            version: "1".to_string(),
            create_time: None,
            create_by: None,
            update_time: None,
            update_by: None,
            ext: None,
            del_flag: None,
            tenant_id: None,
        }
    }

    fn mk_skip(now: &str, next: &str) -> Skip {
        Skip {
            id: None,
            definition_id: 1,
            now_node_code: now.to_string(),
            now_node_type: None,
            next_node_code: next.to_string(),
            next_node_type: None,
            node_id: None,
            skip_name: None,
            skip_type: Some("PASS".to_string()),
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

    /// start → apply → audit → end 的四节点流程。
    fn four_node_flow() -> (Vec<Node>, Vec<Skip>) {
        let nodes = vec![
            mk_node(NodeType::Start, "start"),
            mk_node(NodeType::Between, "apply"),
            mk_node(NodeType::Between, "audit"),
            mk_node(NodeType::End, "end"),
        ];
        let skips = vec![
            mk_skip("start", "apply"),
            mk_skip("apply", "audit"),
            mk_skip("audit", "end"),
        ];
        (nodes, skips)
    }

    #[test]
    fn returns_first_node_after_start() {
        let (nodes, skips) = four_node_flow();
        assert_eq!(
            apply_node_code(&nodes, &skips).as_deref(),
            Some("apply"),
            "申请人节点应是开始节点出边指向的第一个中间节点"
        );
    }

    #[test]
    fn is_not_the_start_node_itself() {
        // 若误取开始节点,退回判定会永远不成立(退回目标不可能是 start)。
        let (nodes, skips) = four_node_flow();
        assert_ne!(apply_node_code(&nodes, &skips).as_deref(), Some("start"));
    }

    #[test]
    fn is_not_an_intermediate_node() {
        // audit 是中间审批节点:退回到它时流程仍在审批中,业务状态不应变 back。
        let (nodes, skips) = four_node_flow();
        assert_ne!(apply_node_code(&nodes, &skips).as_deref(), Some("audit"));
    }

    #[test]
    fn none_when_start_node_missing() {
        let (nodes, skips) = four_node_flow();
        let without_start: Vec<Node> = nodes
            .into_iter()
            .filter(|n| n.node_code != "start")
            .collect();
        assert_eq!(apply_node_code(&without_start, &skips), None);
    }

    #[test]
    fn none_when_start_has_no_outgoing_edge() {
        let (nodes, skips) = four_node_flow();
        let without_first: Vec<Skip> = skips
            .into_iter()
            .filter(|s| s.now_node_code != "start")
            .collect();
        assert_eq!(apply_node_code(&nodes, &without_first), None);
    }
}

/// 在调用方事务内通知业务终态回写。失败向上传播以回滚整笔流程操作(原子化)。
///
/// 事务句柄为 `&TxnContext<'_>`;instance_service(revoke/termination/
/// invalid)与 task_service/skip 链的写闭包统一接线。
pub(crate) async fn notify_business_status(
    callback: &dyn BusinessStatusCallback,
    txn: &TxnContext<'_>,
    flow_code: &str,
    instance: &Instance,
    status: &str,
) -> AppResult<()> {
    if instance.business_id.is_empty() {
        return Ok(());
    }
    callback
        .on_finished(txn, flow_code, &instance.business_id, status)
        .await
}

/// 新待办通知:任务创建后(事务提交成功)调用。失败仅 warn 不阻塞流程
/// (对齐旧系统消息推送的"尽力而为"语义, 但旧系统失败后无迹可查,
/// 这里至少有日志)。
pub(crate) async fn notify_new_tasks(
    notifier: &Arc<dyn FlowNotifier>,
    briefs: &[(String, Vec<String>)],
) {
    for (node_name, perms) in briefs {
        let ids: Vec<i64> =
            perms.iter().filter_map(|p| p.parse().ok()).collect();
        if ids.is_empty() {
            continue;
        }
        let msg = format!("您有新的待办任务:{node_name}");
        if let Err(e) = notifier
            .notify_users(&ids, &msg, Some("/task/taskWaiting"))
            .await
        {
            tracing::warn!(error = %e, node = %node_name, "新待办通知发送失败(已忽略)");
        }
    }
}

#[cfg(test)]
mod notify_business_status_tests {
    //! 判例:**回调与流程操作同事务** —— on_finished Err → 整笔回滚,
    //! 流程状态不落。钉的正是接口语义:回调收 `&TxnContext<'_>`,与流程
    //! 写入共用一个 `DataContext::transaction` 闭包,闭包 Err ⇒ rollback。
    use app_core::error::{AppError, AppResult};
    use app_data::{
        DataError, TxnContext,
        sea_orm::{ActiveValue::Set, EntityTrait},
    };

    use super::{Instance, notify_business_status};
    use crate::{
        repository::entity::{flow_definition, flow_instance},
        resolver::{BusinessStatusCallback, NoopBusinessCallback},
    };

    /// 业务回写恒失败的回调桩:模拟业务表写库错误(真实实现如
    /// LeaveStatusCallback 在写库失败时同样向上传播 Err)。
    struct ErrCallback;

    #[async_trait::async_trait]
    impl BusinessStatusCallback for ErrCallback {
        async fn on_finished(
            &self,
            _txn: &TxnContext<'_>,
            _flow_code: &str,
            _business_id: &str,
            _status: &str,
        ) -> AppResult<()> {
            Err(AppError::Internal("模拟业务回写失败".to_string()))
        }
    }

    fn mk_instance(id: i64, business_id: &str) -> Instance {
        Instance {
            id: Some(id),
            definition_id: 100,
            business_id: business_id.to_string(),
            node_type: 1,
            node_code: "audit".to_string(),
            node_name: None,
            variable: None,
            flow_status: "2".to_string(),
            activity_status: 1,
            def_json: None,
            create_time: None,
            create_by: None,
            update_time: None,
            update_by: None,
            ext: None,
            del_flag: Some("0".to_string()),
            tenant_id: None,
        }
    }

    fn to_active_model(m: &Instance) -> flow_instance::ActiveModel {
        flow_instance::ActiveModel {
            id: Set(m.id.unwrap()),
            definition_id: Set(m.definition_id),
            business_id: Set(m.business_id.clone()),
            node_type: Set(m.node_type),
            node_code: Set(m.node_code.clone()),
            node_name: Set(m.node_name.clone()),
            variable: Set(m.variable.clone()),
            flow_status: Set(m.flow_status.clone()),
            activity_status: Set(m.activity_status),
            def_json: Set(m.def_json.clone()),
            create_time: Set(m.create_time),
            create_by: Set(m.create_by.clone()),
            update_time: Set(m.update_time),
            update_by: Set(m.update_by.clone()),
            ext: Set(m.ext.clone()),
            del_flag: Set(m.del_flag.clone()),
            tenant_id: Set(m.tenant_id.clone()),
        }
    }

    async fn count_instances(ctx: &app_data::DataContext<'_>) -> usize {
        ctx.select(flow_instance::Entity::find())
            .without_row_scope()
            .all()
            .await
            .unwrap()
            .len()
    }

    /// 建表脚手架:flow_instance 的 CREATE TABLE 带指向 flow_definition 的
    /// 外键,sqlx 默认 PRAGMA foreign_keys=ON —— 只建子表会在 insert 时
    /// 报 "no such table: main.flow_definition"(definition_test 同款处置)。
    /// 同时种子一条 id=100 的定义,满足 mk_instance 的 definition_id 外键。
    async fn setup_ds() -> app_data::DataSource {
        let ds = app_data::testing::sqlite_memory_empty().await;
        ds.create_table(flow_definition::Entity).await.unwrap();
        ds.create_table(flow_instance::Entity).await.unwrap();
        ds.system_context()
            .insert(flow_definition::ActiveModel {
                id: Set(100),
                flow_code: Set("leave1".to_string()),
                flow_name: Set("判例流程".to_string()),
                model_value: Set("{}".to_string()),
                category: Set(None),
                version: Set("1".to_string()),
                is_publish: Set(1),
                form_custom: Set(None),
                form_path: Set(None),
                activity_status: Set(1),
                listener_type: Set(None),
                listener_path: Set(None),
                ext: Set(None),
                create_time: Set(None),
                create_by: Set(None),
                update_time: Set(None),
                update_by: Set(None),
                del_flag: Set(Some("0".to_string())),
                tenant_id: Set(None),
            })
            .await
            .unwrap();
        ds
    }

    /// 判例:回调 Err → 整笔回滚,流程状态不落。
    #[tokio::test]
    async fn callback_err_rolls_back_whole_transaction() {
        let ds = setup_ds().await;
        let ctx = ds.system_context();
        let instance = mk_instance(1, "42");

        let result: Result<(), DataError> = ctx
            .transaction(|tx| {
                // owned 值进闭包(HRTB 不收借用切片)。
                let instance = instance.clone();
                Box::pin(async move {
                    let callback = ErrCallback;
                    // 流程写入与业务回写在同一事务闭包内。
                    tx.insert(to_active_model(&instance)).await?;
                    // 调用方语义:回调 Err 经 `?` 向上传播 → 闭包 Err → 回滚。
                    notify_business_status(
                        &callback, tx, "leave1", &instance, "finish",
                    )
                    .await
                    .map_err(|e| {
                        DataError::Unsupported(format!("业务回写失败:{e}"))
                    })?;
                    Ok(())
                })
            })
            .await;

        assert!(result.is_err(), "回调 Err 必须使整笔事务失败");
        assert_eq!(
            count_instances(&ctx).await,
            0,
            "回调 Err 回滚整笔事务后,流程状态不应落库"
        );
    }

    /// 对照组:回调 Ok → 事务提交,流程写入落库(证明回滚断言有效性)。
    #[tokio::test]
    async fn callback_ok_commits_flow_writes() {
        let ds = setup_ds().await;
        let ctx = ds.system_context();
        let instance = mk_instance(1, "42");

        let result: Result<(), DataError> = ctx
            .transaction(|tx| {
                let instance = instance.clone();
                Box::pin(async move {
                    let callback = NoopBusinessCallback;
                    tx.insert(to_active_model(&instance)).await?;
                    notify_business_status(
                        &callback, tx, "leave1", &instance, "finish",
                    )
                    .await
                    .map_err(|e| {
                        DataError::Unsupported(format!("业务回写失败:{e}"))
                    })?;
                    Ok(())
                })
            })
            .await;

        assert!(result.is_ok(), "回调 Ok 应正常提交:{result:?}");
        assert_eq!(count_instances(&ctx).await, 1, "提交后流程状态应落库");
    }
}
