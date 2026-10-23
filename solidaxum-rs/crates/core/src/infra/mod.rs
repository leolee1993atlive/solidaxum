use std::sync::{Arc, OnceLock};

use deadpool_redis::Pool;

#[derive(Clone)]
pub struct Infrastructure {
    pub pool: Arc<ConnectionPool>,
    pub captcha_enabled: bool,
    pub token_ttl: u64,
    /// 装配期一次性注入的 DataSource(私有:crate 外只能经 [`Self::new`] +
    /// [`Self::set_data_source`] 组装,不得用字面量绕过)。
    ///
    /// 刻意是 `Arc` + 后补,而非构造参数:resolver(`CachedScopeResolver`)
    /// 依赖 `ServiceContainer` 里的 `PermissionService`,而 ServiceContainer
    /// 又依赖本 infra —— Infrastructure 构造时 resolver 还不存在,只能
    /// services 起来后再注入,`OnceLock` 正是「一次性后补」的形状。
    /// `Clone` 会携带已装配的同一 Arc(AppState/testkit 依赖这一点)。
    ///
    /// core 不持有连接 —— 装配层(admin app.rs/testkit.rs)自建
    /// `DataSource::connect` 后经 `derive_with_resolver` 派生带 resolver
    /// 的主 ds 注入这里。
    ds: OnceLock<Arc<app_data::DataSource>>,
}

impl Infrastructure {
    /// 唯一构造器(零连接;ds 随后由装配层 [`Self::set_data_source`] 注入)。
    /// 私有字段 ds 不出现在字段面,crate 外无法再用字面量构造 —— 刻意。
    pub fn new(
        pool: Arc<ConnectionPool>,
        captcha_enabled: bool,
        token_ttl: u64,
    ) -> Self {
        Self {
            pool,
            captcha_enabled,
            token_ttl,
            ds: OnceLock::new(),
        }
    }

    /// 装配后访问器:返回存储的同一 Arc(指针稳定,resolver 的跨请求缓存
    /// 跟着句柄走)。未装配 → **panic**(装配契约):生产代码零路径在
    /// 未装配状态调它(app.rs/testkit 的读序有断言锚定)—— 契约违反
    /// 直接炸比静默给无范围句柄(行级范围 fail-closed Unsupported、
    /// 系统路径却照常跑)更诚实。
    pub fn data_source(&self) -> Arc<app_data::DataSource> {
        self.ds
            .get()
            .expect(
                "Infrastructure.data_source() 在 set_data_source 之前被调用:\
                 装配层必须在构造 services 前后注入 DataSource(装配契约)",
            )
            .clone()
    }

    /// 装配期一次性注入(OnceLock:首次成功,之后恒 false)。
    /// 返回 bool 而非 panic:被拒只说明重复装配(编程错误),首次句柄仍
    /// 在生效,炸进程不如让启动日志可见。
    pub fn set_data_source(&self, ds: Arc<app_data::DataSource>) -> bool {
        self.ds.set(ds).is_ok()
    }
}

#[derive(Clone)]
pub struct ConnectionPool {
    pub redis_pool: Pool,
}

#[cfg(test)]
mod tests {
    use std::panic::AssertUnwindSafe;

    use app_data::{
        DataError, Principal, ScopeFilter,
        testing::{self, sandbox_row},
    };

    use super::*;

    /// 惰性 Redis 池(不真正连接)—— 与 admin testkit 同一处置。
    fn lazy_redis_pool() -> Arc<ConnectionPool> {
        let pool = deadpool_redis::Config::from_url("redis://127.0.0.1:6379")
            .create_pool(Some(deadpool_redis::Runtime::Tokio1))
            .expect("创建惰性 redis 池失败");
        Arc::new(ConnectionPool { redis_pool: pool })
    }

    fn infra_bare() -> Infrastructure {
        Infrastructure::new(lazy_redis_pool(), false, 1800)
    }

    /// 装配契约:未装配时 `data_source()` 必须 panic —— 静默降级会把
    /// 「漏装配」伪装成「行级范围全部 Unsupported」的运行期怪病,
    /// 不如启动即炸。
    #[tokio::test]
    async fn data_source_before_assembly_panics() {
        let infra = infra_bare();
        let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
            let _ = infra.data_source();
        }));
        assert!(
            result.is_err(),
            "未装配的 data_source() 必须 panic(装配契约),不得静默降级"
        );
    }

    /// 装配后返回存储的同一 Arc(指针稳定);二次 set 被拒(OnceLock),
    /// 且被拒后仍返回首次句柄。
    #[tokio::test]
    async fn set_then_get_returns_same_arc_and_rejects_second() {
        let infra = infra_bare();
        // 模拟生产装配形态:ds0(resolver-less unscoped 通道)派生出带
        // resolver 的主 ds —— 装配层唯一形状
        let ds0 = Arc::new(testing::sqlite_memory().await);
        let ds = Arc::new(ds0.derive_with_resolver(Arc::new(
            testing::FixedResolver(ScopeFilter::All),
        )));
        assert!(infra.set_data_source(ds.clone()));
        assert!(
            Arc::ptr_eq(&infra.data_source(), &ds),
            "装配后必须返回同一 Arc"
        );
        // 带 resolver 的语义证词:装配后的句柄 context() 可解析(与未装配
        // 的 panic 契约相对);system_context 真查 sqlite(sqlite_memory 已
        // 建表,种行 + 读回)
        let assembled = infra.data_source();
        assembled
            .system_context()
            .insert(sandbox_row(1, Some(10), Some(1)))
            .await
            .unwrap();
        let row = assembled
            .system_context()
            .find_by_id::<testing::sandbox::Entity>(1)
            .await
            .unwrap();
        assert_eq!(row.id, 1);
        assembled
            .context(Principal {
                user_id: 1,
                dept_id: None,
            })
            .scope_filter()
            .await
            .unwrap();

        let second = Arc::new(testing::sqlite_memory().await);
        assert!(
            !infra.set_data_source(second),
            "二次装配必须被 OnceLock 拒绝"
        );
        assert!(
            Arc::ptr_eq(&infra.data_source(), &ds),
            "被拒后仍返回首次装配的句柄"
        );
    }

    /// AppState/testkit 的用法是 set 之后再 clone 进容器 —— Clone 必须携带
    /// 已装配的同一 Arc,否则克隆体会 panic(装配凭空消失)。
    #[tokio::test]
    async fn clone_carries_assembled_arc() {
        let infra = infra_bare();
        let ds = Arc::new(testing::sqlite_memory().await);
        assert!(infra.set_data_source(ds.clone()));
        let clone = infra.clone();
        assert!(
            Arc::ptr_eq(&clone.data_source(), &ds),
            "克隆体必须携带已装配的同一 Arc"
        );
    }

    /// 母体(ds0)不受派生污染:装配后 infra 只见派生体,而 ds0 仍是
    /// resolver-less 的 unscoped 通道(context() fail-closed)—— 装配层
    /// 鸡蛋环破解的结构性证词(细节判例在 app-data context.rs)。
    #[tokio::test]
    async fn assembled_ds_leaves_mother_unscoped() {
        let infra = infra_bare();
        let ds0 = Arc::new(testing::sqlite_memory().await);
        let err = ds0
            .context(Principal {
                user_id: 1,
                dept_id: None,
            })
            .scope_filter()
            .await
            .unwrap_err();
        assert!(
            matches!(err, DataError::Unsupported(_)),
            "母体必须保持 unscoped(fail-closed),实际: {err:?}"
        );
        assert!(infra.set_data_source(Arc::new(ds0.derive_with_resolver(
            Arc::new(testing::FixedResolver(ScopeFilter::All))
        ))));
        infra
            .data_source()
            .context(Principal {
                user_id: 1,
                dept_id: None,
            })
            .scope_filter()
            .await
            .unwrap();
    }
}
