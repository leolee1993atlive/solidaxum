//! 办理人重解析补偿服务(assignee_resync)的 TDD 证据与行为钉。
//!
//!参考实现 消费侧:worker 取出 flow_assignee_sync 行后,本服务对
//! 运行中实例重解析办理人并 diff 增删 flow_user。对齐语义:
//! - 运行中实例:del_flag='0' AND node_type BETWEEN 0 AND 1 AND
//!   flow_status IN ('1','13')(Approval/Pending,Java runningStatus 同口径);
//! - 任务级跳过:flow_user 存在 type IN ('2','3')(转办/委派)即整任务
//!   跳过(对齐 Java assigneeHandler:1401-1406);
//! - 事件过滤:事件波及用户与 diff(adds ∪ removes)无交集则不应用,
//!   有交集则应用**完整** diff(对齐 Java 语义);
//! - 软删死值照抄 user_repo 既有 soft_delete 配置(del_flag='1',
//!   `@TableLogic(value="0", delval="1")`)。

use std::{collections::HashMap, sync::Arc};

use app_data::sea_orm::ActiveValue::Set;
use async_trait::async_trait;
use flow::{
    repository::{
        entity::{
            flow_definition, flow_his_task, flow_instance, flow_node,
            flow_task, flow_user,
        },
        user_repo::UserRepository,
    },
    resolver::UserResolver,
    service::assignee_resync::{
        AssigneeResyncService, ResyncFilter, ResyncReport,
    },
};

/// 假 resolver:设计符 → 用户 ID 的固定映射(只实现补偿链路用到的面)。
struct FakeResolver {
    designators: HashMap<String, Vec<i64>>,
}

#[async_trait]
impl UserResolver for FakeResolver {
    async fn user_names(&self, _ids: &[i64]) -> HashMap<i64, String> {
        HashMap::new()
    }

    async fn resolve_designators(&self, designators: &[String]) -> Vec<i64> {
        designators
            .iter()
            .flat_map(|d| self.designators.get(d).cloned().unwrap_or_default())
            .collect()
    }
}

fn resolver(map: &[(&str, &[i64])]) -> Arc<dyn UserResolver> {
    Arc::new(FakeResolver {
        designators: map
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_vec()))
            .collect(),
    })
}

async fn setup_ds() -> app_data::DataSource {
    let ds = app_data::testing::sqlite_memory_empty().await;
    ds.create_table(flow_definition::Entity).await.unwrap();
    ds.create_table(flow_node::Entity).await.unwrap();
    ds.create_table(flow_instance::Entity).await.unwrap();
    ds.create_table(flow_task::Entity).await.unwrap();
    ds.create_table(flow_his_task::Entity).await.unwrap();
    ds.create_table(flow_user::Entity).await.unwrap();
    ds
}

/// 造一个运行中实例:definition 1 → node n1(perm) → instance → task,
/// task 下挂既有办理人 users: [(type, processed_by)]。
#[allow(clippy::too_many_arguments)]
async fn seed_running_instance(
    ctx: &app_data::DataContext<'_>,
    instance_id: i64,
    task_id: i64,
    node_code: &str,
    perm: Option<&str>,
    flow_status: &str,
    users: &[(i64, &str, &str)],
) {
    ctx.insert(flow_definition::ActiveModel {
        id: Set(instance_id * 1000),
        flow_code: Set(format!("code_{}", instance_id * 1000)),
        flow_name: Set(format!("流程{}", instance_id)),
        model_value: Set("CLASSICS".to_string()),
        version: Set("1".to_string()),
        is_publish: Set(1),
        activity_status: Set(1),
        del_flag: Set(Some("0".to_string())),
        ..Default::default()
    })
    .await
    .unwrap();
    ctx.insert(flow_node::ActiveModel {
        id: Set(instance_id * 1000 + 1),
        node_type: Set(1),
        definition_id: Set(instance_id * 1000),
        node_code: Set(node_code.to_string()),
        node_name: Set(Some(node_code.to_string())),
        permission_flag: Set(perm.map(str::to_string)),
        version: Set("1".to_string()),
        del_flag: Set(Some("0".to_string())),
        ..Default::default()
    })
    .await
    .unwrap();
    ctx.insert(flow_instance::ActiveModel {
        id: Set(instance_id),
        definition_id: Set(instance_id * 1000),
        business_id: Set(format!("biz_{instance_id}")),
        node_type: Set(1),
        node_code: Set(node_code.to_string()),
        variable: Set(Some("{}".to_string())),
        flow_status: Set(flow_status.to_string()),
        activity_status: Set(1),
        del_flag: Set(Some("0".to_string())),
        ..Default::default()
    })
    .await
    .unwrap();
    ctx.insert(flow_task::ActiveModel {
        id: Set(task_id),
        definition_id: Set(instance_id * 1000),
        instance_id: Set(instance_id),
        node_code: Set(node_code.to_string()),
        node_name: Set(Some(node_code.to_string())),
        node_type: Set(1),
        flow_status: Set("1".to_string()),
        del_flag: Set(Some("0".to_string())),
        ..Default::default()
    })
    .await
    .unwrap();
    for (i, (uid, ty, handler)) in users.iter().enumerate() {
        ctx.insert(flow_user::ActiveModel {
            id: Set(task_id * 100 + *uid + i as i64),
            type_: Set(ty.to_string()),
            processed_by: Set(Some(handler.to_string())),
            associated: Set(task_id),
            del_flag: Set(Some("0".to_string())),
            ..Default::default()
        })
        .await
        .unwrap();
    }
}

async fn active_handlers(
    ctx: &app_data::DataContext<'_>,
    task_id: i64,
) -> Vec<String> {
    let all = UserRepository::select_all_by_task_id(ctx, task_id)
        .await
        .unwrap();
    let mut handlers: Vec<String> = all
        .iter()
        .filter(|u| u.del_flag.as_deref() == Some("0"))
        .filter_map(|u| u.processed_by.clone())
        .collect();
    handlers.sort();
    handlers
}

#[tokio::test]
async fn resync_diffs_and_applies_flow_user_changes() {
    let ds = setup_ds().await;
    let ctx = ds.system_context();
    // 实例 1 运行中(flow_status='1'),节点 n1 办理人角色 role:5;
    // 现有办理人 201,假解析出 [202, 203]
    seed_running_instance(
        &ctx,
        1,
        100,
        "n1",
        Some("role:5"),
        "1",
        &[(1, "1", "201")],
    )
    .await;

    let svc = AssigneeResyncService::new(resolver(&[("role:5", &[202, 203])]));
    let report: ResyncReport = svc
        .resync(&ctx, ResyncFilter::default())
        .await
        .expect("resync 必须成功");

    assert_eq!(report.instances, 1);
    assert_eq!(report.tasks_touched, 1);
    assert_eq!(report.users_added, 2);
    assert_eq!(report.users_removed, 1);
    assert_eq!(report.tasks_skipped_manual, 0);

    // 201 软删(死值 '1' 与引擎约定一致),202/203 新增
    assert_eq!(
        active_handlers(&ctx, 100).await,
        vec!["202".to_string(), "203".to_string()]
    );
}

#[tokio::test]
async fn resync_skips_task_with_transfer_or_depute() {
    let ds = setup_ds().await;
    let ctx = ds.system_context();
    // 任务含 type='2'(转办)办理人 → 整任务跳过,不 diff
    seed_running_instance(
        &ctx,
        1,
        100,
        "n1",
        Some("role:5"),
        "1",
        &[(1, "1", "201"), (2, "2", "210")],
    )
    .await;

    let svc = AssigneeResyncService::new(resolver(&[("role:5", &[202])]));
    let report = svc
        .resync(&ctx, ResyncFilter::default())
        .await
        .expect("resync 必须成功");

    assert_eq!(report.tasks_skipped_manual, 1);
    assert_eq!(report.tasks_touched, 0);
    assert_eq!(report.users_added, 0);
    assert_eq!(report.users_removed, 0);
    // 办理人集合原样不动(含转办行 210 —— 跳过即零变更)
    assert_eq!(
        active_handlers(&ctx, 100).await,
        vec!["201".to_string(), "210".to_string()]
    );
}

#[tokio::test]
async fn resync_with_event_filter_only_applies_when_intersecting() {
    let ds = setup_ds().await;
    let ctx = ds.system_context();
    seed_running_instance(
        &ctx,
        1,
        100,
        "n1",
        Some("role:5"),
        "1",
        &[(1, "1", "201")],
    )
    .await;
    let svc = AssigneeResyncService::new(resolver(&[("role:5", &[202, 203])]));

    // 事件用户 [299] 与 diff{+202,+203,-201} 无交集 → 不应用
    let report = svc
        .resync(
            &ctx,
            ResyncFilter {
                instance_id: None,
                event_user_ids: Some(vec![299]),
            },
        )
        .await
        .unwrap();
    assert_eq!(report.users_added, 0);
    assert_eq!(report.users_removed, 0);
    assert_eq!(report.tasks_touched, 0);
    assert_eq!(
        active_handlers(&ctx, 100).await,
        vec!["201".to_string()],
        "无交集时办理人不得变动"
    );

    // 事件用户 [201] 命中 removes → 应用**完整** diff(对齐 Java 语义)
    let report = svc
        .resync(
            &ctx,
            ResyncFilter {
                instance_id: None,
                event_user_ids: Some(vec![201]),
            },
        )
        .await
        .unwrap();
    assert_eq!(report.users_added, 2);
    assert_eq!(report.users_removed, 1);
    assert_eq!(
        active_handlers(&ctx, 100).await,
        vec!["202".to_string(), "203".to_string()]
    );
}

#[tokio::test]
async fn resync_respects_instance_filter() {
    let ds = setup_ds().await;
    let ctx = ds.system_context();
    seed_running_instance(
        &ctx,
        1,
        100,
        "n1",
        Some("role:5"),
        "1",
        &[(1, "1", "201")],
    )
    .await;
    seed_running_instance(
        &ctx,
        2,
        200,
        "n1",
        Some("role:5"),
        "1",
        &[(1, "1", "301")],
    )
    .await;

    let svc = AssigneeResyncService::new(resolver(&[("role:5", &[202])]));
    let report = svc
        .resync(
            &ctx,
            ResyncFilter {
                instance_id: Some(1),
                event_user_ids: None,
            },
        )
        .await
        .unwrap();

    assert_eq!(report.instances, 1, "instance 过滤只处理目标实例");
    assert_eq!(active_handlers(&ctx, 100).await, vec!["202".to_string()]);
    assert_eq!(
        active_handlers(&ctx, 200).await,
        vec!["301".to_string()],
        "实例 2 不得被波及"
    );
}

#[tokio::test]
async fn resync_ignores_finished_instances() {
    let ds = setup_ds().await;
    let ctx = ds.system_context();
    // 已完结(flow_status='8')实例不参与重解析
    seed_running_instance(
        &ctx,
        1,
        100,
        "n1",
        Some("role:5"),
        "8",
        &[(1, "1", "201")],
    )
    .await;

    let svc = AssigneeResyncService::new(resolver(&[("role:5", &[202])]));
    let report = svc.resync(&ctx, ResyncFilter::default()).await.unwrap();

    assert_eq!(report.instances, 0);
    assert_eq!(
        active_handlers(&ctx, 100).await,
        vec!["201".to_string()],
        "完结实例的办理人不得变动"
    );
}
