//! resolver 无递归回归(设计 7.2 递归防御的行为钉)。
//!
//! 形状:`CachedScopeResolver` 持**自有 unscoped 通道**(`ds` 字段,
//! resolver-free 降级快照,与 `Infrastructure::data_source()` 未装配语义同源)
//! —— `resolve()` 未命中缓存时经 `self.ds.system_context()` 取数,
//! principal=None → ScopeFilter 恒 All,resolver 永不被触发,递归在结构上
//! 不可能(见下方「递归陷阱」告警)。
//!
//! 本文件用**陷阱 resolver**钉行为:第一次解析放行(供用户 ctx 在途),
//! 之后任何解析直接报错 —— 若 resolver 的内部读绕道任何带 principal 的
//! ctx(且该读受辖),解析会再次发生,resolve 立刻失败。计数器是直接证据。
//! 诚实声明:get_data_scope 路径的三类读(role 列表 / role_dept 对 / dept
//! 子树)当前全部豁免(`without_row_scope`,Java 锚均无 @DataPermission),
//! 因此豁免本身已是第一层防御;本测试钉的是第二层 —— 即便未来某读失去
//! 豁免,通道也必须兜住,而不是靠方法级豁免碰巧不递归。

mod common;

use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use app_data::{
    DataError, DataSource, Principal, ScopeFilter, ScopeResolver,
    sea_orm::DbErr,
};
use common::setup_schema_and_seeds;
use system::{
    dept::repository::dept_repo::DeptRepository,
    permission::{
        scope_resolver::CachedScopeResolver, service::PermissionService,
    },
};

/// 陷阱 resolver:第一次(也是唯一一次合法的)解析放行 All;之后任何
/// 解析都报错 —— 任何把 resolver 内部读带回解析路径的回归都会让
/// resolve 直接失败,而不是静默递归。
struct RecursionTrap {
    calls: AtomicUsize,
}

#[async_trait::async_trait]
impl ScopeResolver for RecursionTrap {
    async fn resolve(&self, _p: &Principal) -> Result<ScopeFilter, DataError> {
        let n = self.calls.fetch_add(1, Ordering::SeqCst);
        if n == 0 {
            Ok(ScopeFilter::All)
        } else {
            Err(DataError::Backend(DbErr::Custom(
                "递归陷阱:resolver 内部读再次触发了范围解析".into(),
            )))
        }
    }
}

/// 本文件的装配形态需要**同一内存库上的多个 DataSource 快照**(用户侧
/// 陷阱通道 + resolver 自有 unscoped 通道),`DataSource` 不 Clone,故持
/// resolver-less 母体(`sqlite_memory_empty` —— 设计 9.4 给装配层测试的
/// 出口,快照经 `derive_with_resolver` 只读派生),建表/种子仍复用
/// common 的同一套。
async fn setup_conn() -> Arc<DataSource> {
    let ds0 = Arc::new(app_data::testing::sqlite_memory_empty().await);
    setup_schema_and_seeds(&ds0).await;
    ds0
}

#[tokio::test]
async fn resolve_internal_reads_never_trigger_scope_resolution() {
    let ds0 = setup_conn().await;

    // 用户侧"陷阱" ds:模拟装配后的 infra.data_source()(带 resolver)。
    let trap = Arc::new(RecursionTrap {
        calls: AtomicUsize::new(0),
    });
    let trap_ds = Arc::new(ds0.clone().derive_with_resolver(trap.clone()));

    // 前置 1:用户 ctx 在途 —— 受辖读(sys_dept 的范围主通道 select_list)
    // 消费第一次(唯一一次合法)解析,计数 = 1。
    let user_ctx = trap_ds.context(Principal {
        user_id: 302,
        dept_id: Some(101),
    });
    let depts = DeptRepository::new()
        .select_list(&user_ctx, &None, &None, &None)
        .await;
    match depts {
        Ok(rows) => assert!(
            !rows.is_empty(),
            "前置:All 范围下受辖读应可见部门(陷阱接线确认)"
        ),
        Err(e) => panic!("前置失败:用户 ctx 的受辖读不该报错,实际 {e:?}"),
    }
    assert_eq!(
        trap.calls.load(Ordering::SeqCst),
        1,
        "前置:用户 ctx 的解析应恰好被消费一次"
    );

    // 主断言:resolver 用自有通道 resolve —— 内部读零新增解析;
    // 若任何内部读绕道带 principal 的受辖路径,陷阱会让 resolve 报错。
    // 自有通道形态:resolver-free 母体本尊(生产装配 admin/app.rs 同款
    // —— derive 只读派生,结构上与用户通道分离)。
    let resolver = CachedScopeResolver::new(
        ds0.clone(),
        Arc::new(PermissionService::for_test()),
    );
    let sf = resolver
        .resolve(&Principal {
            user_id: 302,
            dept_id: Some(101),
        })
        .await
        .expect("resolve 的内部读不得触发任何额外解析(否则陷阱已报错)");
    // 种子(common/mod.rs):user 302 ← role 201(data_scope='2' 自定义,
    // role_dept {101,103})→ DeptIn([101,103])—— 值正确同时证明读到了库。
    assert_eq!(
        sf,
        ScopeFilter::DeptIn(vec![101, 103]),
        "范围值必须来自真实读库且未被任何范围污染"
    );
    assert_eq!(
        trap.calls.load(Ordering::SeqCst),
        1,
        "resolve 的内部读零新增解析(设计 7.2 递归防御)"
    );
}

/// 通道独立性的第二钉:即便有人误把**带 resolver 的 ds**(如装配后的
/// infra.data_source())当作自有通道传入,system_context(principal=None)
/// 也不会触发它 —— ContextCore::scope_filter 的 (None, _) → All 分支
/// 是结构保证,不依赖装配纪律。此测试钉住该分支不被"优化"掉。
#[tokio::test]
async fn owned_channel_with_resolver_attached_still_never_resolves() {
    let ds0 = setup_conn().await;
    let trap = Arc::new(RecursionTrap {
        calls: AtomicUsize::new(0),
    });
    // 误用形态:自有通道槽位塞进了带陷阱 resolver 的 ds
    let miswired = Arc::new(ds0.clone().derive_with_resolver(trap.clone()));
    let resolver = CachedScopeResolver::new(
        miswired,
        Arc::new(PermissionService::for_test()),
    );

    let sf = resolver
        .resolve(&Principal {
            user_id: 302,
            dept_id: Some(101),
        })
        .await
        .expect("system_context 不得触发所挂 resolver(否则陷阱已报错)");
    assert_eq!(sf, ScopeFilter::DeptIn(vec![101, 103]));
    assert_eq!(
        trap.calls.load(Ordering::SeqCst),
        0,
        "principal=None 的 system_context 从不解析,即便 ds 挂了 resolver"
    );
}
