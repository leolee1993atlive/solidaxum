#![cfg(feature = "test-util")]
//! `update_where` 判例:单语句条件 UPDATE 的漏斗形态 —— 恢复转办三条件
//! 单语句(先写者保)的 CAS 平价。规则:WHERE = 调用方条件 AND 软删存活
//! (声明了软删的实体);ActiveModel 里的主键字段被忽略(cond 是唯一 WHERE
//! 来源,主键不进 SET);不盖审计(Set 什么落什么,update_* 由调用方手工
//! 携带);受辖实体 fail-closed;0 行 → Ok(0)(批量语义无 NotFound 约定)。
//!
//! 与 soft_delete_test.rs 同款:不依赖 sea-orm crate 本身,脚手架与实体
//! 全部来自 `app_data::testing` + 受限 re-export。判例要断言已删行/主键
//! 的实际落库状态,读侧逃生舱 `include_deleted()` 可见墓碑行
//! (task_user_repo_test.rs 先例),无需另造旁路视图实体。
mod audit_soft {
    use app_data::sea_orm::{self, *};

    /// 豁免(CONSTRAINED=false)+ 软删 + 审计列的最小实体 —— testing.rs
    /// 的现成实体没有这个组合(sandbox 有审计列但受辖;soft_dict 豁免带
    /// 软删但无审计列),而"审计不盖"判例需要一个**声明了 audit_columns**
    /// 的豁免实体来钉:若 update_where 盖章,调用方的 Set 值会被顶掉。
    #[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
    #[sea_orm(table_name = "audit_soft")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        pub id: i64,
        pub payload: String,
        pub del_flag: String,
        pub update_by: Option<i64>,
        pub update_time: Option<DateTimeUtc>,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

impl app_data::DataPolicy for audit_soft::Entity {
    const CONSTRAINED: bool = false;
    fn dept_column() -> Option<audit_soft::Column> {
        None
    }
    fn owner_column() -> Option<audit_soft::Column> {
        None
    }
    fn soft_delete() -> Option<(audit_soft::Column, &'static str)> {
        Some((audit_soft::Column::DelFlag, "0"))
    }
    fn audit_columns() -> Option<app_data::AuditColumns<audit_soft::ActiveModel>>
    {
        use app_data::sea_orm::ActiveValue::Set;
        Some(app_data::AuditColumns {
            update_by: Some(|m: &mut audit_soft::ActiveModel, v: i64| {
                m.update_by = Set(Some(v))
            }),
            update_time: Some(
                |m: &mut audit_soft::ActiveModel,
                 v: app_data::sea_orm::DateTimeUtc| {
                    m.update_time = Set(Some(v))
                },
            ),
            ..Default::default()
        })
    }
}

use std::sync::Arc;

use app_data::{
    DataContext, DataError, Principal, ScopeFilter,
    sea_orm::{ActiveValue::Set, ColumnTrait, Condition, EntityTrait},
    testing::{self, FixedResolver},
};

async fn ds() -> app_data::DataSource {
    let ds = testing::sqlite_memory_empty().await;
    ds.create_table(audit_soft::Entity)
        .await
        .expect("建 audit_soft 表");
    ds
}

/// 种子:经漏斗 insert(审计列按声明盖章),del_flag 取显式值 ——
/// 已删行("2")与存活行("0")都种得出来(同 testing::seed 处置)。
async fn seed(ctx: &DataContext<'_>, rows: &[(i64, &str)]) {
    for (id, flag) in rows {
        ctx.insert(audit_soft::ActiveModel {
            id: Set(*id),
            payload: Set(format!("p{id}")),
            del_flag: Set((*flag).to_string()),
            ..Default::default()
        })
        .await
        .expect("种子插入");
    }
}

/// 全量读(含墓碑行):读侧逃生舱 include_deleted。
async fn all_rows(ctx: &DataContext<'_>) -> Vec<(i64, String, String)> {
    let rows = ctx
        .select(audit_soft::Entity::find())
        .without_row_scope()
        .include_deleted()
        .all()
        .await
        .expect("全量读");
    rows.into_iter()
        .map(|m| (m.id, m.payload, m.del_flag))
        .collect()
}

/// ① 豁免 + 软删实体:WHERE = cond AND 软删存活 —— 命中行按 ActiveModel
/// 的 Set 列更新,返回存活命中数;条件外的存活行与已删行均原样。
#[tokio::test]
async fn update_where_hits_alive_rows_matching_cond_and_returns_count() {
    let ds = ds().await;
    let ctx = ds.system_context();
    seed(&ctx, &[(1, "0"), (2, "0"), (3, "0"), (4, "2")]).await;

    let n = ctx
        .update_where::<audit_soft::Entity>(
            Condition::all().add(audit_soft::Column::Id.gte(2)),
            audit_soft::ActiveModel {
                payload: Set("X".to_string()),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(n, 2, "cond 命中的三行中两行存活(已删的 id=4 被 alive 过滤)");

    assert_eq!(
        all_rows(&ctx).await,
        vec![
            (1, "p1".to_string(), "0".to_string()),
            (2, "X".to_string(), "0".to_string()),
            (3, "X".to_string(), "0".to_string()),
            (4, "p4".to_string(), "2".to_string()),
        ],
        "条件外的存活行(id=1)与已删行(id=4,payload/del_flag)原样"
    );
}

/// ② 条件不匹配 → Ok(0):批量语义无 NotFound 约定(转办"select 已不见
/// 行"的后置场景,旧单语句 rows_affected=0 平价)。
#[tokio::test]
async fn update_where_without_matching_rows_returns_zero() {
    let ds = ds().await;
    let ctx = ds.system_context();
    seed(&ctx, &[(1, "0")]).await;

    let n = ctx
        .update_where::<audit_soft::Entity>(
            Condition::all().add(audit_soft::Column::Id.eq(99)),
            audit_soft::ActiveModel {
                payload: Set("X".to_string()),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(n, 0, "零行命中是合法批量结果");
    assert_eq!(
        all_rows(&ctx).await,
        vec![(1, "p1".to_string(), "0".to_string())],
        "被拒范围外行原样"
    );
}

/// ③ 受辖实体(CONSTRAINED=true)fail-closed:批量条件更新同样不得把
/// 行级范围绕成"条件里自己拼"。文案判别守卫归属(与 delete_where/
/// soft_delete_where 同风格,文案必须说"受辖实体")。
#[tokio::test]
async fn update_where_on_constrained_entity_fails_closed() {
    let ds = testing::sqlite_memory().await;
    testing::seed(&ds, vec![(1, 10, 1, "0"), (2, 99, 2, "0")]).await;
    let ds = ds
        .with_resolver(Arc::new(FixedResolver(ScopeFilter::DeptIn(vec![10]))));
    let ctx = ds.context(Principal {
        user_id: 1,
        dept_id: Some(10),
    });

    let err = ctx
        .update_where::<testing::sandbox::Entity>(
            Condition::all().add(testing::sandbox::Column::DeptId.eq(10)),
            testing::sandbox::ActiveModel {
                del_flag: Set("2".to_string()),
                ..Default::default()
            },
        )
        .await
        .unwrap_err();
    let DataError::Unsupported(msg) = err else {
        panic!("受辖形态必须 Unsupported,实际: {err:?}")
    };
    assert!(
        msg.contains("受辖实体"),
        "必须由 CONSTRAINED 守卫拦截,实际文案: {msg}"
    );
    // 行必须原样:守卫在执行前短路,不得产生副作用
    assert!(
        ds.system_context()
            .find_by_id::<testing::sandbox::Entity>(1)
            .await
            .is_ok()
    );
}

/// ④ ActiveModel 里的主键字段被忽略:cond 是唯一 WHERE 来源,主键不进
/// SET —— Set 了别的 id 也不会改写行 id,更不会凭空造行。
#[tokio::test]
async fn update_where_ignores_primary_key_fields_in_set() {
    let ds = ds().await;
    let ctx = ds.system_context();
    seed(&ctx, &[(1, "0")]).await;

    let n = ctx
        .update_where::<audit_soft::Entity>(
            Condition::all().add(audit_soft::Column::Id.eq(1)),
            audit_soft::ActiveModel {
                id: Set(999), // 主键 Set 值必须被忽略
                payload: Set("X".to_string()),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(n, 1);

    let rows = all_rows(&ctx).await;
    assert_eq!(
        rows,
        vec![(1, "X".to_string(), "0".to_string())],
        "行 id 仍是 1(未被 Set 的 999 改写),也没有凭空多出 id=999 的行"
    );
}

/// ⑤ TxnContext 面同语义:闭包内可用,返回计数;闭包 Err → 更新随事务
/// 整体回滚。
#[tokio::test]
async fn update_where_inside_transaction_commits_and_rolls_back() {
    let ds = ds().await;
    let ctx = ds.system_context();
    seed(&ctx, &[(1, "0"), (2, "0")]).await;

    // 提交分支:闭包内条件更新,返回计数;提交后可见
    let n = ctx
        .transaction(|tx| {
            Box::pin(async move {
                tx.update_where::<audit_soft::Entity>(
                    Condition::all().add(audit_soft::Column::Id.eq(1)),
                    audit_soft::ActiveModel {
                        payload: Set("C".to_string()),
                        ..Default::default()
                    },
                )
                .await
            })
        })
        .await
        .unwrap();
    assert_eq!(n, 1, "事务内返回存活命中数");

    // 回滚分支:更新后报错 → 不落库
    let _ = ctx
        .transaction(|tx| {
            Box::pin(async move {
                tx.update_where::<audit_soft::Entity>(
                    Condition::all().add(audit_soft::Column::Id.eq(2)),
                    audit_soft::ActiveModel {
                        payload: Set("R".to_string()),
                        ..Default::default()
                    },
                )
                .await?;
                Err::<(), DataError>(DataError::NotFound)
            })
        })
        .await;

    assert_eq!(
        all_rows(&ctx).await,
        vec![
            (1, "C".to_string(), "0".to_string()),
            (2, "p2".to_string(), "0".to_string()),
        ],
        "提交分支落库;回滚分支的更新必须被撤销"
    );
}

/// ⑥ 审计不盖:Set 什么落什么 —— update_where 不经 audit_columns 盖章,
/// update_* 由调用方手工携带(与 soft_delete_where 的"不盖审计"同口径;
/// category 删除的 tombstone 惯用法恰好依赖审计列随 ActiveModel 携带)。
/// 用声明了 audit_columns 的豁免实体钉:若漏斗盖章,调用方的 Set 值会被
/// principal(42)顶掉;AM 里 NotSet 的 update_time 也必须保持种子值。
#[tokio::test]
async fn update_where_does_not_stamp_audit_columns() {
    let ds = ds().await;
    // 有 principal 的上下文(42):insert 会盖章,update_where 不许盖
    let ctx = ds.context(Principal {
        user_id: 42,
        dept_id: Some(10),
    });
    let seeded = ctx
        .insert(audit_soft::ActiveModel {
            id: Set(1),
            payload: Set("p1".to_string()),
            del_flag: Set("0".to_string()),
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(seeded.update_by, Some(42), "前置:insert 照盖(对照组)");

    let n = ctx
        .update_where::<audit_soft::Entity>(
            Condition::all().add(audit_soft::Column::Id.eq(1)),
            audit_soft::ActiveModel {
                payload: Set("X".to_string()),
                update_by: Set(Some(55)), // 调用方手工携带的审计值
                ..Default::default()      // update_time NotSet → 保持
            },
        )
        .await
        .unwrap();
    assert_eq!(n, 1);

    let row = ctx
        .select(audit_soft::Entity::find_by_id(1))
        .without_row_scope()
        .one()
        .await
        .unwrap()
        .expect("行还在");
    assert_eq!(
        row.update_by,
        Some(55),
        "调用方的 Set 值落库,不被 principal 盖章顶掉"
    );
    assert_eq!(
        row.update_time, seeded.update_time,
        "NotSet 的审计列保持原值(update_where 不盖时间)"
    );
}

/// ⑦ 空 SET 守卫(补充判例,同 update_query 的动机):仅主键 Set 的
/// ActiveModel → Unsupported。没有守卫时 vendor 的 is_noop 分支会对空 SET
/// 静默 Ok(0) —— 连同 WHERE 一起跳过,条件 UPDATE 变成无声 no-op。
#[tokio::test]
async fn update_where_with_empty_set_is_rejected_not_silent_ok() {
    let ds = ds().await;
    let ctx = ds.system_context();
    seed(&ctx, &[(1, "0")]).await;

    let err = ctx
        .update_where::<audit_soft::Entity>(
            Condition::all().add(audit_soft::Column::Id.eq(1)),
            audit_soft::ActiveModel {
                id: Set(1), // 只有主键,无任何非主键 Set
                ..Default::default()
            },
        )
        .await
        .unwrap_err();
    assert!(
        matches!(err, DataError::Unsupported(_)),
        "空 SET 更新必须被拒,实际: {err:?}"
    );
    assert_eq!(
        all_rows(&ctx).await,
        vec![(1, "p1".to_string(), "0".to_string())],
        "被拒的 update_where 不得产生副作用"
    );
}
