#![cfg(feature = "test-util")]
//! `soft_delete_where` 判例:Java `@TableLogic` 的批量写路径平价 ——
//! WHERE = 调用方条件 AND 软删存活(已删行不重复计数);仅豁免实体
//! (CONSTRAINED=false)可用,受辖实体与无软删列实体均 fail-closed。
//!
//! 与 runtime_write_test.rs 同款:不依赖 sea-orm crate,脚手架与实体来自
//! `app_data::testing`。判例②/⑤要断言 del_flag 的**实际落库值**(含非存活
//! 行),而漏斗读侧一律施存活过滤,故文件内定义一个同表、无软删声明的
//! 校验用实体作旁路视图 —— 全部件来自受限 re-export,证业务侧拼得出。
use std::sync::Arc;

use app_data::{
    DataContext, DataError, DataPolicy, Principal, ScopeFilter,
    sea_orm::{
        ActiveValue::Set, ColumnTrait, Condition, EntityTrait, QueryOrder,
    },
    testing::{self, FixedResolver},
};

/// 校验用旁路视图:与 `testing::soft_dict` 同表,但 `soft_delete() = None` ——
/// 读侧不施存活过滤,能读到已删行的实际 del_flag 值。
/// `CONSTRAINED = false`,读取显式 `without_row_scope()`。
mod soft_dict_raw {
    use app_data::sea_orm::{self, *};

    #[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
    #[sea_orm(table_name = "soft_dict")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        pub id: i64,
        pub del_flag: String,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

impl DataPolicy for soft_dict_raw::Entity {
    const CONSTRAINED: bool = false;
    fn dept_column() -> Option<soft_dict_raw::Column> {
        None
    }
    fn owner_column() -> Option<soft_dict_raw::Column> {
        None
    }
    fn soft_delete() -> Option<(soft_dict_raw::Column, &'static str)> {
        None
    }
    fn audit_columns()
    -> Option<app_data::AuditColumns<soft_dict_raw::ActiveModel>> {
        None
    }
}

/// 种子面:经漏斗 insert(soft_dict 无审计列可盖),del_flag 取显式值 ——
/// 已删行("2")与存活行("0")都种得出来,不需要后门(同 testing::seed 处置)。
async fn seed_soft_dict(ctx: &DataContext<'_>, rows: &[(i64, &str)]) {
    for (id, flag) in rows {
        ctx.insert(testing::soft_dict::ActiveModel {
            id: Set(*id),
            del_flag: Set((*flag).to_string()),
        })
        .await
        .expect("种子插入");
    }
}

/// 旁路视图全量读(不过滤存活),返回 (id, del_flag) 供落库值断言。
async fn raw_flags(ctx: &DataContext<'_>) -> Vec<(i64, String)> {
    let rows = ctx
        .select(
            soft_dict_raw::Entity::find()
                .order_by_asc(soft_dict_raw::Column::Id),
        )
        .without_row_scope()
        .all()
        .await
        .expect("旁路视图全量读");
    rows.into_iter().map(|m| (m.id, m.del_flag)).collect()
}

/// ① 豁免 + 软删实体:WHERE = cond AND 软删存活 —— 命中行置 dead 值,
/// 返回存活命中数;条件外的存活行原样。
#[tokio::test]
async fn soft_delete_where_hits_alive_rows_matching_cond_and_returns_count() {
    let ds = testing::sqlite_memory().await;
    let ctx = ds.system_context();
    seed_soft_dict(&ctx, &[(1, "0"), (2, "0"), (3, "0")]).await;

    let n = ctx
        .soft_delete_where::<testing::soft_dict::Entity>(
            Condition::all().add(testing::soft_dict::Column::Id.gte(2)),
            "2",
        )
        .await
        .unwrap();
    assert_eq!(n, 2, "返回存活命中数(cond 命中的两行)");

    assert_eq!(
        raw_flags(&ctx).await,
        vec![
            (1, "0".to_string()),
            (2, "2".to_string()),
            (3, "2".to_string())
        ],
        "cond 命中行置 dead 值,条件外的存活行(id=1)原样"
    );
}

/// ② 已删行不重复计数:alive 过滤是 `@TableLogic` 平价 —— 已删行既不计入
/// 返回数,也不被新的 dead 值重写。
#[tokio::test]
async fn soft_delete_where_does_not_recount_already_dead_rows() {
    let ds = testing::sqlite_memory().await;
    let ctx = ds.system_context();
    seed_soft_dict(&ctx, &[(1, "0"), (2, "2")]).await;

    let n = ctx
        .soft_delete_where::<testing::soft_dict::Entity>(
            Condition::all().add(testing::soft_dict::Column::Id.gte(1)),
            "9", // 刻意换个新 dead 值:已删行若被重写,落库值会变成 "9"
        )
        .await
        .unwrap();
    assert_eq!(n, 1, "已删行不计入存活命中数");

    assert_eq!(
        raw_flags(&ctx).await,
        vec![(1, "9".to_string()), (2, "2".to_string())],
        "已删行的原 dead 值必须原样保留,不被重写"
    );
}

/// ③ 受辖实体(CONSTRAINED=true)fail-closed:批量软删同样不得把行级范围
/// 绕成"条件里自己拼"。文案判别守卫归属(第一个守卫说"受辖实体")。
#[tokio::test]
async fn soft_delete_where_on_constrained_entity_fails_closed() {
    let ds = testing::sqlite_memory().await;
    testing::seed(&ds, vec![(1, 10, 1, "0"), (2, 99, 2, "0")]).await;
    let ds = ds
        .with_resolver(Arc::new(FixedResolver(ScopeFilter::DeptIn(vec![10]))));
    let ctx = ds.context(Principal {
        user_id: 1,
        dept_id: Some(10),
    });

    let err = ctx
        .soft_delete_where::<testing::sandbox::Entity>(
            Condition::all().add(testing::sandbox::Column::DeptId.eq(10)),
            "2",
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

/// ④ 无软删列实体 fail-closed:没有 @TableLogic 可对齐,批量写删语义无
/// 落点(物理删走 delete_where)。文案判别守卫归属(第二个守卫说"软删除列")。
#[tokio::test]
async fn soft_delete_where_without_soft_delete_column_fails_closed() {
    let ds = testing::sqlite_memory().await;
    let ctx = ds.system_context();
    ctx.insert(testing::dict_entry::ActiveModel {
        id: Set(1),
        label: Set("原值".into()),
        remark: Set("备注".into()),
    })
    .await
    .unwrap();

    let err = ctx
        .soft_delete_where::<testing::dict_entry::Entity>(
            Condition::all().add(testing::dict_entry::Column::Id.eq(1)),
            "2",
        )
        .await
        .unwrap_err();
    let DataError::Unsupported(msg) = err else {
        panic!("无软删列形态必须 Unsupported,实际: {err:?}")
    };
    assert!(
        msg.contains("软删除列"),
        "必须由软删守卫拦截而非 CONSTRAINED 守卫,实际文案: {msg}"
    );
    // 行必须原样
    let row = ctx
        .find_by_id::<testing::dict_entry::Entity>(1)
        .await
        .unwrap();
    assert_eq!(row.label, "原值");
}

/// ⑤ dead 值原样落库:'1' 与 '2' 均可,漏斗不解释、不校验 dead 的取值
/// (RuoYi 惯例 "2",warm-flow 侧业务亦用 "1" 等值 —— 取值是调用方的事)。
#[tokio::test]
async fn soft_delete_where_stores_dead_value_verbatim() {
    let ds = testing::sqlite_memory().await;
    let ctx = ds.system_context();
    seed_soft_dict(&ctx, &[(1, "0"), (2, "0")]).await;

    let n = ctx
        .soft_delete_where::<testing::soft_dict::Entity>(
            Condition::all().add(testing::soft_dict::Column::Id.eq(1)),
            "1",
        )
        .await
        .unwrap();
    assert_eq!(n, 1);
    let n = ctx
        .soft_delete_where::<testing::soft_dict::Entity>(
            Condition::all().add(testing::soft_dict::Column::Id.eq(2)),
            "2",
        )
        .await
        .unwrap();
    assert_eq!(n, 1);

    assert_eq!(
        raw_flags(&ctx).await,
        vec![(1, "1".to_string()), (2, "2".to_string())],
        "dead 值必须原样落库,'1' 与 '2' 均可"
    );
}

/// ⑥ TxnContext 面同语义:闭包内可用;闭包 Err → 软删随事务整体回滚。
#[tokio::test]
async fn soft_delete_where_inside_transaction_commits_and_rolls_back() {
    let ds = testing::sqlite_memory().await;
    let ctx = ds.system_context();
    seed_soft_dict(&ctx, &[(1, "0"), (2, "0")]).await;

    // 提交分支:闭包内软删,返回计数;提交后落库值可见
    let n = ctx
        .transaction(|tx| {
            Box::pin(async move {
                let n = tx
                    .soft_delete_where::<testing::soft_dict::Entity>(
                        Condition::all()
                            .add(testing::soft_dict::Column::Id.eq(1)),
                        "2",
                    )
                    .await?;
                Ok::<u64, DataError>(n)
            })
        })
        .await
        .unwrap();
    assert_eq!(n, 1, "事务内返回存活命中数");

    // 回滚分支:软删后报错 → 不落库
    let _ = ctx
        .transaction(|tx| {
            Box::pin(async move {
                tx.soft_delete_where::<testing::soft_dict::Entity>(
                    Condition::all().add(testing::soft_dict::Column::Id.eq(2)),
                    "2",
                )
                .await?;
                Err::<(), DataError>(DataError::NotFound)
            })
        })
        .await;

    assert_eq!(
        raw_flags(&ctx).await,
        vec![(1, "2".to_string()), (2, "0".to_string())],
        "提交分支落库;回滚分支的软删必须被撤销"
    );
}
