use std::{collections::HashMap, sync::Arc};

use admin::flow_resolver::{LeaveStatusCallback, SystemUserResolver};
use app_core::util::snowflake;
use app_data::{DataError, DataSource, sea_orm::ActiveValue::Set};
use flow::{
    repository::{entity::test_leave, leave_repo},
    resolver::{BusinessStatusCallback, HandlerSelectService, UserResolver},
};
use serde_json::json;

/// DataSource 持库,`create_table` 按实体建表;回调经
/// `system_context().transaction` 闭包拿到 `&TxnContext<'_>`(唯一事务
/// 句柄),断言读回走 `leave_repo::select_by_id(&DataContext)`。
async fn setup_leave_ds() -> Arc<DataSource> {
    let ds = Arc::new(app_data::testing::sqlite_memory_empty().await);
    ds.create_table(test_leave::Entity).await.unwrap();
    ds
}

/// 种子一条请假单(test_leave 无审计/软删列,plain insert 原样落库)。
async fn seed_leave(ds: &DataSource, model: &test_leave::Model) {
    ds.system_context()
        .insert(test_leave::ActiveModel {
            id: Set(model.id),
            leave_type: Set(model.leave_type.clone()),
            remark: Set(model.remark.clone()),
            status: Set(model.status.clone()),
            apply_code: Set(model.apply_code.clone()),
            start_date: Set(model.start_date),
            end_date: Set(model.end_date),
            leave_days: Set(model.leave_days),
            create_dept: Set(model.create_dept),
            create_time: Set(model.create_time),
            create_by: Set(model.create_by),
            update_time: Set(model.update_time),
            update_by: Set(model.update_by),
        })
        .await
        .unwrap();
}

/// 为 SystemUserResolver 建最小库:仅需 sys_user_role / sys_user_post 两张关联表
/// (resolve_designators 经由 user_ids_by_role_ids / user_ids_by_post_ids 查它们)。
/// 连接经 `sqlite_memory_empty`(门控 ds),裸 DDL/种子经
/// `testkit::execute_raw(&ds, …)`;resolver 构造收 `Arc<DataSource>`。
async fn setup_resolver_ds() -> Arc<DataSource> {
    let ds = app_data::testing::sqlite_memory_empty().await;
    app_data::testkit::execute_raw(
        &ds,
        "CREATE TABLE sys_user_role (user_id INTEGER NOT NULL, role_id INTEGER NOT NULL)",
    )
    .await
    .unwrap();
    app_data::testkit::execute_raw(
        &ds,
        "CREATE TABLE sys_user_post (user_id INTEGER NOT NULL, post_id INTEGER NOT NULL)",
    )
    .await
    .unwrap();
    // role 800 → users {1001, 1002};post 900 → users {1002, 1003}
    app_data::testkit::execute_raw(
        &ds,
        "INSERT INTO sys_user_role (user_id, role_id) VALUES \
         (1001, 800), (1002, 800), (1002, 801)",
    )
    .await
    .unwrap();
    app_data::testkit::execute_raw(
        &ds,
        "INSERT INTO sys_user_post (user_id, post_id) VALUES \
         (1002, 900), (1003, 900)",
    )
    .await
    .unwrap();
    Arc::new(ds)
}

/// flow_resolver.rs:46 — `SELECT DISTINCT user_id FROM sys_user_role WHERE role_id IN ({})`
/// flow_resolver.rs:92 — `SELECT DISTINCT user_id FROM sys_user_post WHERE post_id IN ({})`
/// 二者经由 SystemUserResolver::resolve_designators 触发(designator 前缀 role:/post:)。
/// 断言当前手写 SQL 行为:DISTINCT 去重 + 多来源并集去重排序。
#[tokio::test]
async fn system_user_resolver_resolves_role_and_post_designators() {
    let ds = setup_resolver_ds().await;
    let resolver = SystemUserResolver::new(ds);

    // role:800 → users {1001, 1002}
    let ids = resolver
        .resolve_designators(&["role:800".to_string()])
        .await;
    assert_eq!(ids, vec![1001, 1002], "role:800 应解析为 1001,1002(升序)");

    // post:900 → users {1002, 1003}
    let ids = resolver
        .resolve_designators(&["post:900".to_string()])
        .await;
    assert_eq!(ids, vec![1002, 1003], "post:900 应解析为 1002,1003(升序)");

    // 并集:role:800 ∪ post:900 = {1001,1002,1003}(去重排序)
    let ids = resolver
        .resolve_designators(&["role:800".to_string(), "post:900".to_string()])
        .await;
    assert_eq!(
        ids,
        vec![1001, 1002, 1003],
        "role+post 并集应去重排序为 1001,1002,1003"
    );
}

/// 不存在/非法 designator 应安静跳过,不报错、不影响结果。
#[tokio::test]
async fn system_user_resolver_skips_unknown_designators() {
    let ds = setup_resolver_ds().await;
    let resolver = SystemUserResolver::new(ds);

    // role:999 无关联 → 空结果
    let ids = resolver
        .resolve_designators(&["role:999".to_string()])
        .await;
    assert!(ids.is_empty(), "无关联的 role 应返回空");

    // user: 直接 id 透传(不经关联表)
    let ids = resolver
        .resolve_designators(&["user:555".to_string()])
        .await;
    assert_eq!(ids, vec![555], "user: 前缀应直接透传 user_id");
}

/// 全量系统表库(deptrole 交集需要 sys_user / sys_user_role / sys_dept)。
/// 种子:user 301(dept 101, role 201)、302(dept 102)、901(dept 101)。
async fn setup_full_ds() -> Arc<DataSource> {
    let ds = app_data::testing::sqlite_memory_empty().await;
    app_data::testkit::run_script(&ds, app_data::testkit::SYSTEM_TABLE_DDL)
        .await
        .expect("DDL 执行失败");
    app_data::testkit::run_script(&ds, app_data::testkit::SYSTEM_SEED)
        .await
        .expect("seed 执行失败");
    Arc::new(ds)
}

/// deptrole:{deptId}:{roleId} = 科室 ∩ 角色 交集(对齐 Java `deptRole:` 惰性
/// token 的下游解析 selectUsersByRoleIdAndDeptIds)。
#[tokio::test]
async fn system_user_resolver_resolves_deptrole_designator() {
    let ds = setup_full_ds().await;
    let resolver = SystemUserResolver::new(ds);

    // dept 101 ∩ role 201 → user 301(901 同在 dept 101 但无 role 201)
    let ids = resolver
        .resolve_designators(&["deptrole:101:201".to_string()])
        .await;
    assert_eq!(ids, vec![301], "deptrole:101:201 应只命中 user 301");

    // 科室不匹配 → 空(user 302 在 dept 102 但无 role 201)
    assert!(
        resolver
            .resolve_designators(&["deptrole:102:201".to_string()])
            .await
            .is_empty()
    );
    // 角色不匹配 → 空(dept 101 无人持 role 200)
    assert!(
        resolver
            .resolve_designators(&["deptrole:101:200".to_string()])
            .await
            .is_empty()
    );

    // 与其他设计符并集(去重排序)
    let ids = resolver
        .resolve_designators(&[
            "deptrole:101:201".to_string(),
            "user:555".to_string(),
        ])
        .await;
    assert_eq!(ids, vec![301, 555]);
}

/// user_role_ids:hasRole 预热快照的数据源(user_id → role_ids)。
#[tokio::test]
async fn system_user_resolver_user_role_ids() {
    let ds = setup_full_ds().await;
    let resolver = SystemUserResolver::new(ds);

    let m = resolver.user_role_ids(&[301, 302, 999]).await;
    assert_eq!(m.get(&301), Some(&vec![201]), "user 301 应持 role 201");
    assert!(
        !m.contains_key(&302) || m[&302].is_empty(),
        "user 302 无角色"
    );
    assert!(!m.contains_key(&999), "不存在的用户不应有记录");
}

/// deptrolekey:{deptId}:{roleKey}:roleKey 翻译成 roleId 后按 deptrole
/// 交集解析(跨环境通用,角色 ID 各环境不同 key 不变;
/// 对齐参考实现 Flow.deptRoleKeys,查不到的 key 静默跳过)。
/// 种子:role 201 key=common;user 301(dept 101, role 201)。
#[tokio::test]
async fn system_user_resolver_resolves_deptrolekey_designator() {
    let ds = setup_full_ds().await;
    let resolver = SystemUserResolver::new(ds);

    // dept 101 ∩ role key=common(→201) → user 301
    let ids = resolver
        .resolve_designators(&["deptrolekey:101:common".to_string()])
        .await;
    assert_eq!(ids, vec![301], "deptrolekey:101:common 应命中 user 301");

    // 查不到的 roleKey 静默跳过 → 空(配合 orElse 兜底)
    assert!(
        resolver
            .resolve_designators(&["deptrolekey:101:nonexist".to_string()])
            .await
            .is_empty()
    );
}

/// user_role_keys:hasRoleKeys 预热快照的数据源(user_id → role_keys)。
#[tokio::test]
async fn system_user_resolver_user_role_keys() {
    let ds = setup_full_ds().await;
    let resolver = SystemUserResolver::new(ds);

    let m = resolver.user_role_keys(&[301, 302]).await;
    assert_eq!(
        m.get(&301).map(|v| v.as_slice()),
        Some(&["common".to_string()][..]),
        "user 301 应持 roleKey common"
    );
    assert!(
        !m.contains_key(&302) || m[&302].is_empty(),
        "user 302 无角色"
    );
}

#[tokio::test]
async fn leave_callback_returns_business_info() {
    let ds = setup_leave_ds().await;
    seed_leave(
        &ds,
        &test_leave::Model {
            id: 100,
            leave_type: "事假".into(),
            remark: None,
            status: Some("waiting".into()),
            apply_code: Some("AP001".into()),
            start_date: None,
            end_date: None,
            leave_days: Some(2),
            create_dept: None,
            create_time: None,
            create_by: None,
            update_time: None,
            update_by: None,
        },
    )
    .await;

    let callback = LeaveStatusCallback::new(ds.clone());
    let info = callback
        .get_business_info("100")
        .await
        .unwrap()
        .expect("应有业务信息");
    assert_eq!(info.code.as_deref(), Some("AP001"));
    assert!(info.title.as_deref().unwrap_or("").contains("事假"));
}

#[tokio::test]
async fn leave_callback_on_finished_updates_status() {
    let ds = setup_leave_ds().await;
    let model = test_leave::Model {
        id: snowflake::next_id(),
        leave_type: "事假".into(),
        remark: None,
        status: Some("waiting".into()),
        apply_code: None,
        start_date: None,
        end_date: None,
        leave_days: Some(1),
        create_dept: None,
        create_time: None,
        create_by: None,
        update_time: None,
        update_by: None,
    };
    let id = model.id;
    seed_leave(&ds, &model).await;

    let callback = LeaveStatusCallback::new(ds.clone());
    // 原子化:回调在调用方事务内执行,失败应可回滚整笔流程操作。
    // 事务句柄唯一来源 = DataContext::transaction 闭包(owned 值进闭包)。
    let business_id = id.to_string();
    ds.system_context()
        .transaction(|tx| {
            let callback = callback.clone();
            Box::pin(async move {
                callback
                    .on_finished(tx, "leave1", &business_id, "finish")
                    .await
                    .map_err(|e| DataError::Unsupported(e.to_string()))
            })
        })
        .await
        .unwrap();
    let updated = leave_repo::select_by_id(&ds.system_context(), id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(updated.status.as_deref(), Some("finish"));
}

#[tokio::test]
async fn leave_callback_on_started_sets_waiting_in_txn() {
    let ds = setup_leave_ds().await;
    let model = test_leave::Model {
        id: snowflake::next_id(),
        leave_type: "事假".into(),
        remark: None,
        status: Some("draft".into()),
        apply_code: None,
        start_date: None,
        end_date: None,
        leave_days: Some(1),
        create_dept: None,
        create_time: None,
        create_by: None,
        update_time: None,
        update_by: None,
    };
    let id = model.id;
    seed_leave(&ds, &model).await;

    let callback = LeaveStatusCallback::new(ds.clone());
    let business_id = id.to_string();
    ds.system_context()
        .transaction(|tx| {
            let callback = callback.clone();
            Box::pin(async move {
                callback
                    .on_started(tx, "leave1", &business_id)
                    .await
                    .map_err(|e| DataError::Unsupported(e.to_string()))
            })
        })
        .await
        .unwrap();
    let updated = leave_repo::select_by_id(&ds.system_context(), id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(updated.status.as_deref(), Some("waiting"));
}

/// 回调对所有流程触发,必须能按业务隔离:非请假业务安静跳过(Ok),否则会把
/// 无关流程的启动/办理整笔回滚掉。隔离分两层:(1)flowCode 守卫——非 leave*
/// 流程直接跳过(主,对齐 Java flowCode 路由);(2)businessId 兜底——flowCode
/// 匹配但 businessId 非数字或查无请假记录时也跳过(辅)。
#[tokio::test]
async fn leave_callback_skips_non_leave_business() {
    let ds = setup_leave_ds().await;
    // 预置一条真实请假记录,用于验证"flowCode 不匹配时即便记录存在也不写"。
    let leave_id = snowflake::next_id();
    seed_leave(
        &ds,
        &test_leave::Model {
            id: leave_id,
            leave_type: "事假".into(),
            remark: None,
            status: Some("waiting".into()),
            apply_code: None,
            start_date: None,
            end_date: None,
            leave_days: Some(1),
            create_dept: None,
            create_time: None,
            create_by: None,
            update_time: None,
            update_by: None,
        },
    )
    .await;

    let callback = LeaveStatusCallback::new(ds.clone());
    let leave_id_str = leave_id.to_string();
    ds.system_context()
        .transaction(|tx| {
            let callback = callback.clone();
            Box::pin(async move {
                // (1)flowCode 守卫:非 leave 流程(如纠纷处理)即便 businessId
                //     指向真实请假记录,也必须跳过、status 不变。
                callback
                    .on_started(tx, "dispute1", &leave_id_str)
                    .await
                    .expect("非 leave flowCode 应安静跳过");
                callback
                    .on_finished(tx, "case_task", &leave_id_str, "finish")
                    .await
                    .expect("非 leave flowCode 应安静跳过");

                // (2)businessId 兜底:flowCode 匹配但 businessId 非数字或
                //     查无记录,跳过。
                callback
                    .on_started(tx, "leave1", "task_contract_biz_1")
                    .await
                    .expect("非数字 businessId 应安静跳过");
                callback
                    .on_finished(tx, "leave1", "999999", "finish")
                    .await
                    .expect("缺失请假记录应安静跳过");
                Ok(())
            })
        })
        .await
        .unwrap();

    // 预置记录的 status 应保持 "waiting"(被守卫挡住,未被回写)。
    let unchanged = leave_repo::select_by_id(&ds.system_context(), leave_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        unchanged.status.as_deref(),
        Some("waiting"),
        "flowCode 守卫应阻止非 leave 流程回写请假记录"
    );
}

// ─── 判例:选人弹窗三型在受限 resolver 下照全量 ─────────────────────
//
// adapter 查询全部显式 without_row_scope(flow_resolver.rs 文件头
// 审计表:名称回显/办理人解析锚不受辖;弹窗列表锚受辖但本仓路由公开、
// 无会话 Principal 可施,显式豁免维持全量,偏差披露)。DataSource 即便
// 装上 SelfOnly(7) 的受限 resolver,弹窗三型也必须照全量返回(豁免不
// 解析 resolver);分页 total 口径 = 过滤(软删)后总数。

/// 用 testkit 全量种子建库(4 user / 3 role / 5 dept,全部 del_flag='0'),
/// 装上 SelfOnly(7) 的受限 resolver 再交给选人服务。
/// 直接经 `app_data::testkit` 的 DDL/种子(不经 admin::testkit —— 那是
/// test-util 门控模块,这里让判例在两套测试门下都可跑)。
async fn setup_handler_select_ds() -> Arc<DataSource> {
    let ds = app_data::testing::sqlite_memory_empty().await;
    app_data::testkit::run_script(&ds, app_data::testkit::SYSTEM_TABLE_DDL)
        .await
        .expect("DDL 执行失败");
    app_data::testkit::run_script(&ds, app_data::testkit::SYSTEM_SEED)
        .await
        .expect("seed 执行失败");
    Arc::new(ds.derive_with_resolver(Arc::new(
        app_data::testing::FixedResolver(app_data::ScopeFilter::SelfOnly(7)),
    )))
}

/// 三型(user/role/dept)在受限 resolver 下照全量;total 为软删过滤后总数。
#[tokio::test]
async fn handler_select_returns_full_rows_under_restricted_resolver() {
    let ds = setup_handler_select_ds().await;
    let svc = admin::flow_resolver::SystemHandlerSelectService::new(ds);

    // user 型:种子 4 个存活用户;storage_id 带 user: 前缀
    let r = svc.get_handlers("user", None, 1, 10).await.unwrap();
    assert_eq!(r.total, 4, "user 型 total 应为存活全量(4),实际 {}", r.total);
    assert_eq!(r.rows.len(), 4);
    assert!(r.rows.iter().all(|row| row.storage_id.starts_with("user:")));

    // role 型:种子 3 个存活角色;role_sort 升序
    let r = svc.get_handlers("role", None, 1, 10).await.unwrap();
    assert_eq!(r.total, 3, "role 型 total 应为存活全量(3)");
    assert_eq!(r.rows.len(), 3);
    assert!(r.rows.iter().all(|row| row.storage_id.starts_with("role:")));

    // dept 型:种子 5 个存活部门;order_num 升序
    let r = svc.get_handlers("dept", None, 1, 10).await.unwrap();
    assert_eq!(r.total, 5, "dept 型 total 应为存活全量(5)");
    assert_eq!(r.rows.len(), 5);
    assert!(r.rows.iter().all(|row| row.storage_id.starts_with("dept:")));
    // order_num 升序:首个应为 100(总部,order_num=0)
    assert_eq!(r.rows[0].handler_code, "100");
}

/// 分页 total 口径:total 与页码无关(过滤后总数),页行数按 size 切。
#[tokio::test]
async fn handler_select_page_total_matches_pre_migration_semantics() {
    let ds = setup_handler_select_ds().await;
    let svc = admin::flow_resolver::SystemHandlerSelectService::new(ds);

    // 4 个用户,page_size=3:第 1 页 3 行、total=4;第 2 页 1 行、total 仍 4
    let p1 = svc.get_handlers("user", None, 1, 3).await.unwrap();
    assert_eq!(p1.rows.len(), 3);
    assert_eq!(p1.total, 4);
    let p2 = svc.get_handlers("user", None, 2, 3).await.unwrap();
    assert_eq!(p2.rows.len(), 1);
    assert_eq!(p2.total, 4, "total 与页码无关(迁移前 count() 同口径)");

    // status 过滤参与 total:种子用户全部 status='0',传 '1' 应 0 行 0 total
    let r = svc.get_handlers("user", Some("1"), 1, 10).await.unwrap();
    assert_eq!(r.total, 0);
    assert!(r.rows.is_empty());

    // user 型附带左侧部门树:种子 5 部门全部 status='0',根 = 总部(100,
    // 其 parent_id=0 不在结果集),树下应含全部部门 id
    let r = svc.get_handlers("user", None, 1, 10).await.unwrap();
    assert_eq!(r.tree_selections.len(), 1, "唯一根节点 = 总部");
    let root = &r.tree_selections[0];
    assert_eq!(root.get("id").and_then(|v| v.as_str()), Some("100"));
}

// ─── D2 实例级办理人解析(sameStep/preStep/preStepApper/preStepParent/initiator) ───
//
// 语义基准:Java FlwTaskAssigneeServiceImpl.java:523-583(逐行核实)。
// 有意差异(代码注释登记):解析不到历史任务时 Java 抛 ServiceException,
// Rust 返回空 + warn(解析无错误通道);preStep 回退发起人的条件 Rust 泛化为
// 所有节点(Java 限定 ae_rep 源节点)。

/// D2 最小库:sys_user / sys_role / sys_user_role 经 SYSTEM_TABLE_DDL,
/// flow_his_task / flow_user 经实体建表(create_table)。
///
/// 种子拓扑:
/// - 角色 R1=11(parent_id=12) / R2=12(无上级)
/// - user 201(dept 10, role 11) / 202(dept 10, role 12) / 203(dept 20, role 12)
///   / 204(dept 10, 无角色)
async fn setup_instance_ds() -> Arc<DataSource> {
    let ds = app_data::testing::sqlite_memory_empty().await;
    app_data::testkit::run_script(&ds, app_data::testkit::SYSTEM_TABLE_DDL)
        .await
        .expect("DDL 执行失败");
    app_data::testkit::execute_raw(
        &ds,
        "INSERT INTO sys_role (role_id, role_name, role_key, role_sort, \
         data_scope, menu_check_strictly, dept_check_strictly, status, \
         del_flag, parent_id) VALUES \
         (11, '科室角色', 'r1', 0, '1', 1, 1, '0', '0', 12), \
         (12, '上级角色', 'r2', 1, '1', 1, 1, '0', '0', NULL)",
    )
    .await
    .unwrap();
    app_data::testkit::execute_raw(
        &ds,
        "INSERT INTO sys_user (user_id, dept_id, user_name, nick_name, \
         password, status, del_flag) VALUES \
         (201, 10, 'u201', '用户201', 'x', '0', '0'), \
         (202, 10, 'u202', '用户202', 'x', '0', '0'), \
         (203, 20, 'u203', '用户203', 'x', '0', '0'), \
         (204, 10, 'u204', '用户204', 'x', '0', '0')",
    )
    .await
    .unwrap();
    app_data::testkit::execute_raw(
        &ds,
        "INSERT INTO sys_user_role (user_id, role_id) VALUES \
         (201, 11), (202, 12), (203, 12)",
    )
    .await
    .unwrap();
    // 实体衍生表带外键约束(flow_his_task → flow_definition/flow_instance,
    // flow_user → flow_task),父表必须先建
    ds.create_table(flow::repository::entity::flow_definition::Entity)
        .await
        .unwrap();
    ds.create_table(flow::repository::entity::flow_instance::Entity)
        .await
        .unwrap();
    ds.create_table(flow::repository::entity::flow_task::Entity)
        .await
        .unwrap();
    ds.create_table(flow::repository::entity::flow_his_task::Entity)
        .await
        .unwrap();
    ds.create_table(flow::repository::entity::flow_user::Entity)
        .await
        .unwrap();
    // 外键父行:his_task → flow_definition/flow_instance,flow_user.associated
    // → flow_task;实例 1~4 与各 seed 用到的 task id 全部预置
    app_data::testkit::execute_raw(
        &ds,
        "INSERT INTO flow_definition (id, flow_code, flow_name, model_value, \
         version, is_publish, activity_status) VALUES (1, 'd2', 'D2 流程', '{}', \
         '1', 1, 1)",
    )
    .await
    .unwrap();
    app_data::testkit::execute_raw(
        &ds,
        "INSERT INTO flow_instance (id, definition_id, business_id, node_type, \
         node_code, flow_status, activity_status) VALUES \
         (1, 1, 'b1', 1, 'n', '1', 1), (2, 1, 'b2', 1, 'n', '1', 1), \
         (3, 1, 'b3', 1, 'n', '1', 1), (4, 1, 'b4', 1, 'n', '1', 1)",
    )
    .await
    .unwrap();
    app_data::testkit::execute_raw(
        &ds,
        "INSERT INTO flow_task (id, definition_id, instance_id, node_code, \
         node_type, flow_status) VALUES \
         (99, 1, 1, 'n', 1, '1'), (100, 1, 1, 'n', 1, '1'), \
         (101, 1, 1, 'n', 1, '1'), (200, 1, 2, 'n', 1, '1'), \
         (201, 1, 2, 'n', 1, '1'), (202, 1, 2, 'n', 1, '1'), \
         (203, 1, 2, 'n', 1, '1'), (204, 1, 2, 'n', 1, '1'), \
         (205, 1, 2, 'n', 1, '1'), (206, 1, 2, 'n', 1, '1'), \
         (207, 1, 2, 'n', 1, '1'), \
         (210, 1, 3, 'n', 1, '1'), (211, 1, 3, 'n', 1, '1'), \
         (300, 1, 4, 'n', 1, '1')",
    )
    .await
    .unwrap();
    Arc::new(ds)
}

/// 实例 1:同节点两条历史(id 10 早 / 11 晚)+ 一条别的节点更早历史(id 9)。
async fn seed_same_step(ds: &DataSource) {
    app_data::testkit::execute_raw(
        ds,
        "INSERT INTO flow_his_task (id, definition_id, instance_id, task_id, \
         node_code, cooperate_type, skip_type, flow_status, del_flag) VALUES \
         (9, 1, 1, 99, 'other_node', 1, 'PASS', 'PASS', '0'), \
         (10, 1, 1, 100, 'ae_dept_audit', 1, 'PASS', 'PASS', '0'), \
         (11, 1, 1, 101, 'ae_dept_audit', 1, 'PASS', 'PASS', '0')",
    )
    .await
    .unwrap();
    app_data::testkit::execute_raw(
        ds,
        "INSERT INTO flow_user (id, type, processed_by, associated, del_flag) \
         VALUES (1, '1', '201', 100, '0'), (2, '1', '202', 101, '0')",
    )
    .await
    .unwrap();
}

/// sameStep:{nodeCode}:该实例该节点**最早** his_task(id 升序第一条)的
/// flow_user 办理人(对齐 Java FlwTaskAssigneeServiceImpl:523-536)。
#[tokio::test]
async fn same_step_returns_earliest_his_task_users() {
    let ds = setup_instance_ds().await;
    seed_same_step(&ds).await;
    let r = SystemUserResolver::new(ds);
    let ctx = flow::resolver::InstanceAssigneeContext {
        instance_id: 1,
        target_node_code: "ae_ana".into(),
        variable: HashMap::new(),
        current: None,
    };
    let out = r
        .resolve_instance_designators(&["sameStep:ae_dept_audit".into()], &ctx)
        .await;
    assert_eq!(out, vec![201], "sameStep 取最早历史任务的办理人");
}

/// 实例 2:目标节点匹配 + skip_type 过滤 + id 最大取胜 + 逗号集合匹配 +
/// 匹配但无 flow_user 时回退 initiator 变量。
async fn seed_pre_step(ds: &DataSource) {
    app_data::testkit::execute_raw(
        ds,
        "INSERT INTO flow_his_task (id, definition_id, instance_id, task_id, \
         target_node_code, cooperate_type, skip_type, flow_status, del_flag) \
         VALUES \
         (20, 1, 2, 200, 'ae_area_audit', 1, 'PASS', 'PASS', '0'), \
         (21, 1, 2, 201, 'ae_area_audit', 1, 'PASS', 'PASS', '0'), \
         (22, 1, 2, 202, 'ae_area_audit', 1, 'REJECT', 'BACK', '0'), \
         (23, 1, 2, 203, 'other_node', 1, 'PASS', 'PASS', '0'), \
         (24, 1, 2, 204, 'ae_ana,ae_area_audit', 1, 'PASS', 'PASS', '0'), \
         (25, 1, 2, 205, 'empty_users', 1, 'PASS', 'PASS', '0'), \
         (26, 1, 2, 206, 'ae_ana,ae_ana', 1, 'PASS', 'PASS', '0')",
    )
    .await
    .unwrap();
    // task 200 → 203 / 201 → 204;202(REJECT)与 204(逗号多值集合不等)无
    // user;205 故意无 flow_user(回退 initiator 用)
    app_data::testkit::execute_raw(
        ds,
        "INSERT INTO flow_user (id, type, processed_by, associated, del_flag) \
         VALUES (10, '1', '203', 200, '0'), (11, '1', '204', 201, '0'), \
         (12, '1', '207', 206, '0'), (13, '1', '208', 207, '1')",
    )
    .await
    .unwrap();
    // 走查实测回归:任务完成即被引擎逻辑删办理人行(skip/persist.rs),
    // preStep 查的恰是这种历史任务 —— 软删行必须仍可解析。
    app_data::testkit::execute_raw(
        ds,
        "INSERT INTO flow_his_task (id, definition_id, instance_id, task_id, \
         target_node_code, cooperate_type, skip_type, flow_status, del_flag) \
         VALUES (27, 1, 2, 207, 'ae_soft_deleted', 1, 'PASS', 'PASS', '0')",
    )
    .await
    .unwrap();
}

/// preStep:{nodeCode}:target_node_code 匹配(逗号拆分集合相等/单值相等)且
/// skip_type=PASS 的 **id 最大** his_task 的 flow_user;空则回退
/// variable['initiator'](Java 限定 ae_rep 节点,Rust 泛化为所有节点 ——
/// 有意差异,见实现处注释)。
#[tokio::test]
async fn pre_step_returns_latest_passing_task_users_with_initiator_fallback() {
    let ds = setup_instance_ds().await;
    seed_pre_step(&ds).await;
    let r = SystemUserResolver::new(ds);
    let ctx = flow::resolver::InstanceAssigneeContext {
        instance_id: 2,
        target_node_code: "ae_ana".into(),
        variable: HashMap::from([("initiator".to_string(), json!(777))]),
        current: None,
    };

    // id 最大且 PASS:21 → flow_user 204(REJECT 的 22 被过滤)
    let out = r
        .resolve_instance_designators(&["preStep:ae_area_audit".into()], &ctx)
        .await;
    assert_eq!(out, vec![204], "preStep 取 id 最大的 PASS 历史任务办理人");

    // 逗号集合不等('ae_ana,ae_area_audit' ≠ {ae_ana})→ 不匹配;
    // 'ae_ana,ae_ana' 拆分去重后集合相等 → 命中(id 26 → 207)
    let out = r
        .resolve_instance_designators(&["preStep:ae_ana".into()], &ctx)
        .await;
    assert_eq!(
        out,
        vec![207],
        "逗号拆分集合与 [node_code] 相等才匹配('ae_ana,ae_ana' 命中)"
    );

    // 匹配但无 flow_user → 回退 initiator 变量
    let out = r
        .resolve_instance_designators(&["preStep:empty_users".into()], &ctx)
        .await;
    assert_eq!(out, vec![777], "无 flow_user 时回退 variable['initiator']");

    // 无匹配历史 → 同样回退 initiator 变量
    let out = r
        .resolve_instance_designators(&["preStep:no_such_node".into()], &ctx)
        .await;
    assert_eq!(out, vec![777], "无匹配历史任务时回退 variable['initiator']");

    // 办理人行已软删(任务完成的引擎常态)→ 仍须解析出办理人,
    // 不得因默认软删存活过滤恒空而回退 initiator
    let out = r
        .resolve_instance_designators(&["preStep:ae_soft_deleted".into()], &ctx)
        .await;
    assert_eq!(out, vec![208], "软删的办理人行仍可解析(preStep)");
}

/// 实例 3:approver 与 flow_user 并存,preStepApper 只取 approver。
async fn seed_pre_step_apper(ds: &DataSource) {
    app_data::testkit::execute_raw(
        ds,
        "INSERT INTO flow_his_task (id, definition_id, instance_id, task_id, \
         target_node_code, approver, cooperate_type, skip_type, flow_status, \
         del_flag) VALUES \
         (30, 1, 3, 210, 'ae_apper_node', '205', 1, 'PASS', 'PASS', '0'), \
         (31, 1, 3, 211, 'ae_apper_node', '206', 1, 'PASS', 'PASS', '0')",
    )
    .await
    .unwrap();
    app_data::testkit::execute_raw(
        ds,
        "INSERT INTO flow_user (id, type, processed_by, associated, del_flag) \
         VALUES (20, '1', '206', 211, '0')",
    )
    .await
    .unwrap();
}

/// preStepApper:{nodeCode}:同 preStep 找 his_task,但只取 approver 字段
/// (一人),不看 flow_user。
#[tokio::test]
async fn pre_step_apper_returns_only_approver() {
    let ds = setup_instance_ds().await;
    seed_pre_step_apper(&ds).await;
    let r = SystemUserResolver::new(ds);
    let ctx = flow::resolver::InstanceAssigneeContext {
        instance_id: 3,
        target_node_code: "ae_ana".into(),
        variable: HashMap::new(),
        current: None,
    };
    // id 最大 = 31,approver=206;id=30 的 approver=205 因非最大而不取
    let out = r
        .resolve_instance_designators(
            &["preStepApper:ae_apper_node".into()],
            &ctx,
        )
        .await;
    assert_eq!(
        out,
        vec![206],
        "preStepApper 取 id 最大 his_task 的 approver"
    );
}

/// 实例 4:preStepParent = preStep 办理人(201)的上级角色用户 ∩ 同部门。
/// 201 的角色 R1=11 → 上级 R2=12;R2 用户 202(dept 10)/ 203(dept 20),
/// 与 201 同 dept(10)的只有 202。
async fn seed_pre_step_parent(ds: &DataSource) {
    app_data::testkit::execute_raw(
        ds,
        "INSERT INTO flow_his_task (id, definition_id, instance_id, task_id, \
         target_node_code, cooperate_type, skip_type, flow_status, del_flag) \
         VALUES (40, 1, 4, 300, 'p_node', 1, 'PASS', 'PASS', '0')",
    )
    .await
    .unwrap();
    app_data::testkit::execute_raw(
        ds,
        "INSERT INTO flow_user (id, type, processed_by, associated, del_flag) \
         VALUES (30, '1', '201', 300, '0')",
    )
    .await
    .unwrap();
}

/// preStepParent:{nodeCode}:preStep 办理人的上级角色(sys_role.parent_id)
/// ∩ 同部门用户(对齐旧 SysUserServiceImpl.java:908-929)。
#[tokio::test]
async fn pre_step_parent_returns_parent_role_users_same_dept() {
    let ds = setup_instance_ds().await;
    seed_pre_step_parent(&ds).await;
    let r = SystemUserResolver::new(ds);
    let ctx = flow::resolver::InstanceAssigneeContext {
        instance_id: 4,
        target_node_code: "ae_ana".into(),
        variable: HashMap::new(),
        current: None,
    };
    let out = r
        .resolve_instance_designators(&["preStepParent:p_node".into()], &ctx)
        .await;
    assert_eq!(
        out,
        vec![202],
        "上级角色 R2 用户中仅 202 与办理人 201 同部门(10)"
    );
}

/// initiator 读变量(数字/字符串皆可);initiatorParent = initiator 的
/// 上级角色 ∩ 同部门(201 → R1 → R2 ∩ dept 10 → 202)。
#[tokio::test]
async fn initiator_reads_variable_and_parent_variant() {
    let ds = setup_instance_ds().await;
    let r = SystemUserResolver::new(ds);
    let ctx = flow::resolver::InstanceAssigneeContext {
        instance_id: 0,
        target_node_code: "ae_ana".into(),
        variable: HashMap::from([("initiator".to_string(), json!(201))]),
        current: None,
    };
    let out = r
        .resolve_instance_designators(&["initiator".into()], &ctx)
        .await;
    assert_eq!(out, vec![201]);
    let out = r
        .resolve_instance_designators(&["initiatorParent".into()], &ctx)
        .await;
    assert_eq!(out, vec![202], "initiatorParent 展开上级角色 ∩ 同部门");

    // 字符串形态的数字同样可读
    let ctx = flow::resolver::InstanceAssigneeContext {
        instance_id: 0,
        target_node_code: "ae_ana".into(),
        variable: HashMap::from([("initiator".to_string(), json!("201"))]),
        current: None,
    };
    let out = r
        .resolve_instance_designators(&["initiator".into()], &ctx)
        .await;
    assert_eq!(out, vec![201], "initiator 字符串数字同样解析");
}

/// 有意差异:Java 解析不到历史任务抛 ServiceException;Rust 返回空 +
/// warn(解析面无错误通道,见计划 Global Constraints 登记)。
#[tokio::test]
async fn missing_history_returns_empty_not_error() {
    let ds = setup_instance_ds().await;
    let r = SystemUserResolver::new(ds);
    let ctx = flow::resolver::InstanceAssigneeContext {
        instance_id: 999,
        target_node_code: "ae_ana".into(),
        variable: HashMap::new(),
        current: None,
    };
    assert!(
        r.resolve_instance_designators(&["sameStep:any".into()], &ctx)
            .await
            .is_empty()
    );
    assert!(
        r.resolve_instance_designators(&["preStep:any".into()], &ctx)
            .await
            .is_empty(),
        "preStep 无历史且无 initiator 变量时应为空,不回退出别的值"
    );
    assert!(
        r.resolve_instance_designators(&["preStepApper:any".into()], &ctx)
            .await
            .is_empty()
    );
}

/// D14 回归:preStep 引用**在途 his 行的 target**(= 本次 PASS 推进的
/// 目标节点)时,skip 事务内该行尚未提交,resolver 独立连接查不到 →
/// 须直接消费上下文快照,不落库。(此前该场景恒走 initiator 回退,
/// 部门主管被派给发起人而非组长办理人。sameStep「最早行」语义在
/// 返工回路下与快照不合,维持纯库查,不在此列。)
#[tokio::test]
async fn in_flight_current_step_resolves_from_context() {
    let ds = setup_instance_ds().await;
    let r = SystemUserResolver::new(ds);
    let ctx = flow::resolver::InstanceAssigneeContext {
        // instance 999 无任何 his 行:库查必然落空,只能靠 current 命中
        instance_id: 999,
        // 正在为 ae_dept_mgr 解析办理人;在途行 target = ae_dept_mgr
        target_node_code: "ae_dept_mgr".into(),
        variable: HashMap::new(),
        current: Some(flow::resolver::CurrentStepContext {
            node_code: "ae_leader".into(),
            skip_target_node_code: "ae_dept_mgr".into(),
            task_id: 42,
            handlers: vec![201, 202],
            approver: Some(201),
        }),
    };
    let out = r
        .resolve_instance_designators(&["preStep:ae_dept_mgr".into()], &ctx)
        .await;
    assert_eq!(
        out,
        vec![201, 202],
        "preStep 命中在途行 target 应取上下文 handlers"
    );
    let out = r
        .resolve_instance_designators(
            &["preStepApper:ae_dept_mgr".into()],
            &ctx,
        )
        .await;
    assert_eq!(
        out,
        vec![201],
        "preStepApper 命中在途行 target 应取上下文 approver"
    );
    // 引用别的节点:不命中在途行,照旧走库查(该实例无 his → 空)
    let out = r
        .resolve_instance_designators(&["preStep:ae_other".into()], &ctx)
        .await;
    assert!(
        out.is_empty(),
        "未命中在途行的设计符仍走库查,不应泄漏当前步数据"
    );
    // sameStep 不消费在途快照(语义=最早已提交行):该实例无 his → 空
    let out = r
        .resolve_instance_designators(&["sameStep:ae_leader".into()], &ctx)
        .await;
    assert!(out.is_empty(), "sameStep 维持纯库查语义");
}
