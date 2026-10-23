//! DataScope → ScopeFilter 的直译 resolver + 跨请求 TTL 缓存(设计 7.2)。
//!
//! **缓存归属**:请求内的单次解析缓存(OnceCell)归 app-data 的 ContextCore;
//! **跨请求**缓存归 resolver —— 只有这里知道一次解析要查多少库表
//! (sys_role + sys_role_dept + sys_dept 子树展开),也只有这里能以
//! user_id 为键跨请求去重。键只有 user_id:部门调动的陈旧窗口由
//! [`CachedScopeResolver::invalidate`](user_service 转部门定点失效)
//! 与 TTL(60s)兜底;角色数据范围变更的主动失效见
//! [`CachedScopeResolver::invalidate_all`](role 写路径全清)。
//!
//! **递归防御(设计 7.2 ⚠️ 递归陷阱)**:resolver 持
//! **自有 unscoped 通道**([`CachedScopeResolver::ds`],resolver-free 快照),
//! 取数一律经它的 `system_context()` —— 与用户 ctx 结构上无关,递归不可能。
//! 行为回归钉:`tests/scope_recursion_test.rs`(陷阱 resolver 计数断言)。

use std::{
    collections::HashMap,
    sync::{Arc, OnceLock, RwLock},
    time::{Duration, Instant},
};

use app_data::{
    DataError, DataSource, Principal, ScopeFilter, ScopeResolver,
    sea_orm::DbErr,
};

use super::{data_scope::DataScope, service::PermissionService};

/// 缓存 TTL:60s —— 主动失效(`invalidate`/`invalidate_all`,接线于
/// role/user 写路径)之外的兜底:未接线路径(如未来新增的写点)、
/// 失效与并发读之间的窗口,最长 60s 后自动收敛。
const SCOPE_TTL: Duration = Duration::from_secs(60);

/// 容量上限:1024,超限**全清**(简单防泄漏)。不做 LRU:数据范围缓存
/// 冷启动一次查库即可逐条重建,清空代价可接受,复杂度不值得。
const CACHE_CAPACITY: usize = 1024;

/// 缓存值:(过滤器, 写入时刻)。
type CacheMap = HashMap<i64, (ScopeFilter, Instant)>;

/// DataScope → ScopeFilter 的 1:1 直译。独立的纯函数:PermissionService
/// 是具体类型无法 stub,映射语义在此逐分支单测。
pub(crate) fn map_scope(ds: &DataScope) -> ScopeFilter {
    match ds {
        DataScope::All => ScopeFilter::All,
        DataScope::Custom(dept_ids) => ScopeFilter::DeptIn(dept_ids.clone()),
        DataScope::SelfOnly(user_id) => ScopeFilter::SelfOnly(*user_id),
        DataScope::CustomOrSelf { dept_ids, user_id } => {
            ScopeFilter::DeptInOrSelf {
                depts: dept_ids.clone(),
                user_id: *user_id,
            }
        }
    }
}

/// 读缓存:命中且未过期才返回。过期条目不主动清除 —— 读路径为清几条
/// 过期项去拿写锁会把读请求串行化,得不偿失;过期项由覆盖或超限全清回收。
fn cache_get(
    cache: &CacheMap,
    ttl: Duration,
    now: Instant,
    user_id: i64,
) -> Option<ScopeFilter> {
    let (sf, at) = cache.get(&user_id)?;
    (now.duration_since(*at) < ttl).then(|| sf.clone())
}

/// 写缓存:已达容量且写入的是**新键** → 全清(防泄漏);已有键刷新
/// 原地覆盖,不触发全清 —— 常用用户的续期不该把整表缓存清掉。
fn cache_put(
    cache: &mut CacheMap,
    capacity: usize,
    now: Instant,
    user_id: i64,
    sf: ScopeFilter,
) {
    if cache.len() >= capacity && !cache.contains_key(&user_id) {
        cache.clear();
    }
    cache.insert(user_id, (sf, now));
}

/// 失效单个用户的缓存条目(不存在则 no-op,幂等)。
fn cache_invalidate(cache: &mut CacheMap, user_id: i64) {
    cache.remove(&user_id);
}

/// 全清缓存(幂等)。与超限全清同一容量语义:冷启动逐条重建的
/// 代价可接受,不做按 role 反查受影响用户的反向索引。
fn cache_invalidate_all(cache: &mut CacheMap) {
    cache.clear();
}

/// system 侧的 [`ScopeResolver`] 实现:包 `Arc<PermissionService>` 加
/// 60s TTL 跨请求缓存。由装配层(admin/app.rs)经
/// `DataSource::with_resolver(Arc::new(...))` 注入 app-data —— 这是
/// 设计 7.2 的依赖反转点。
pub struct CachedScopeResolver {
    inner: Arc<PermissionService>,
    /// **resolver 自有 unscoped 通道**(设计 7.2):resolver-free 降级快照(`DataSource::from_connection`,
    /// 与 `Infrastructure::data_source()` 未装配语义同源,包同一连接池)。
    /// resolve 未命中缓存的取数一律经它的 `system_context()`:
    /// ① 通道上没有 resolver 可触发;② system_context 的 principal=None
    /// → ScopeFilter 恒 All(`ContextCore::scope_filter` 的 `(None, _)`
    /// 分支)—— 双重结构保证「解析范围 → 查库」不会再回到解析,
    /// 递归不可能,与任何在途用户 ctx 无关。
    /// 刻意不复用装配后带 resolver 的 `infra.data_source()`:那同样安全
    /// (system_context 永不触发解析),但自有快照让独立性**可见、可
    /// grep**(tests/scope_recursion_test.rs 钉住两重保证)。
    ds: Arc<DataSource>,
    ttl: Duration,
    // std RwLock(而非 tokio RwLock):两个关键区都是纯
    // HashMap 操作,从不跨 await 持锁 —— tokio 锁在此零收益;而
    // invalidate 是同步签名(调用方直接同步调用),tokio
    // 写锁的 await 化会与该签名冲突(blocking_write 在异步上下文里
    // 直接 panic,更不可用)。
    cache: RwLock<CacheMap>,
}

impl CachedScopeResolver {
    pub fn new(ds: Arc<DataSource>, perm: Arc<PermissionService>) -> Self {
        Self {
            inner: perm,
            ds,
            ttl: SCOPE_TTL,
            cache: RwLock::new(CacheMap::new()),
        }
    }

    /// 主动失效某用户的已缓存范围。用户级变更的失效点:
    /// user_service 的 authRole 分配/转部门、role_service 的 authUser
    /// 批量授予/取消(user_id 已知,定点失效);未覆盖的路径只有 60s
    /// TTL 兜底。
    pub fn invalidate(&self, user_id: i64) {
        cache_invalidate(&mut self.lock_write(), user_id);
    }

    /// 主动失效**全部**已缓存范围。角色级变更(data_scope /
    /// role_dept / 删角色 / 停启用)的失效点 —— 缓存按 user_id 键,无法从
    /// role_id 反查受影响用户,取 Java `@CacheEvict(allEntries = true)`
    /// 语义(SysRoleServiceImpl:433 批量删除自用),全清重建。
    pub fn invalidate_all(&self) {
        cache_invalidate_all(&mut self.lock_write());
    }

    fn lock_read(&self) -> std::sync::RwLockReadGuard<'_, CacheMap> {
        // 关键区无可 panic 源(键 i64、值全为 Clone 数据);即使外部
        // panic 令锁中毒,也取回数据继续 —— 缓存问题不得放大成整站 5xx
        self.cache.read().unwrap_or_else(|e| e.into_inner())
    }

    fn lock_write(&self) -> std::sync::RwLockWriteGuard<'_, CacheMap> {
        self.cache.write().unwrap_or_else(|e| e.into_inner())
    }
}

/// 失效槽:ServiceContainer 与 role/user 服务**共享**的「后补注入」句柄。
///
/// 构造环:resolver 依赖 ServiceContainer 里的 PermissionService,而
/// 服务又在 ServiceContainer::new 里构造 —— 服务构造时 resolver 还不
/// 存在,只能先发空槽、装配层(admin/app.rs)建好 resolver 后经
/// [`ServiceContainer::set_scope_cache`](crate::app::ServiceContainer::set_scope_cache)
/// 一次性后补(OnceLock 形状对齐 `Infrastructure::set_data_source`,见
/// core/src/infra/mod.rs 的注释)。刻意是 `Arc<OnceLock<…>>` 而非容器
/// 私有 OnceLock:失效调用点在 role/user 服务的方法里,必须与容器看到
/// **同一格槽** —— set 一次,所有持有者即刻可见。
pub type ScopeCacheSlot = Arc<OnceLock<Arc<CachedScopeResolver>>>;

/// 造一格空槽(构造期默认):所有失效调用 no-op,直到装配层后补真句柄。
pub fn empty_scope_cache_slot() -> ScopeCacheSlot {
    Arc::new(OnceLock::new())
}

/// 槽位便捷调用:全清。未装配(单测/降级路径)时 no-op —— 失效钩子
/// 缺席只回退到 60s TTL 兜底,不得 panic。
pub(crate) fn slot_invalidate_all(slot: &ScopeCacheSlot) {
    if let Some(resolver) = slot.get() {
        resolver.invalidate_all();
    }
}

/// 槽位便捷调用:定点失效某用户。未装配时 no-op,语义同上。
pub(crate) fn slot_invalidate(slot: &ScopeCacheSlot, user_id: i64) {
    if let Some(resolver) = slot.get() {
        resolver.invalidate(user_id);
    }
}

#[async_trait::async_trait]
impl ScopeResolver for CachedScopeResolver {
    async fn resolve(&self, p: &Principal) -> Result<ScopeFilter, DataError> {
        // 1) 命中且未过期:直接返回,不触库(读锁不跨 await)
        if let Some(sf) =
            cache_get(&self.lock_read(), self.ttl, Instant::now(), p.user_id)
        {
            return Ok(sf);
        }
        // 2) 未命中 → 经 **resolver 自有 unscoped 通道**取数(设计 7.2):
        //    `self.ds` 是 resolver-free
        //    快照,system_context 的 principal=None → ScopeFilter 恒 All
        //    —— 查库不会再次触发解析,递归在结构上不可能,与任何在途
        //    用户 ctx 无关(行为钉:tests/scope_recursion_test.rs)。
        //    **锁外执行**,不跨请求串行化。
        let ctx = self.ds.system_context();
        let scope = self
            .inner
            .get_data_scope(&ctx, p.user_id, p.dept_id)
            .await
            .map_err(|e| {
                // 裁定:AppError 不是 DbErr,不能走 From 直转;包成
                // Backend(DbErr::Custom)—— "底层错误透传"语义最贴近,
                // localizer 已能翻译 Custom。失败不落缓存,下次 resolve
                // 重试取数。
                DataError::Backend(DbErr::Custom(format!(
                    "数据范围解析失败: {e}"
                )))
            })?;
        let sf = map_scope(&scope);
        // 3) 写回缓存(容量 1024 超限全清防泄漏)。失败路径已在上面 ?
        //    提前返回,不会把错误结果写进缓存
        cache_put(
            &mut self.lock_write(),
            CACHE_CAPACITY,
            Instant::now(),
            p.user_id,
            sf.clone(),
        );
        Ok(sf)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- map_scope:四个分支逐一直译 ----

    #[test]
    fn maps_all_four_variants() {
        // Custom 用 [2,1]:直译不排序不重排,顺序保持原样
        // (combine_scopes 已保证有序,这里钉住"不二次加工")。
        let cases = [
            (DataScope::All, ScopeFilter::All),
            (
                DataScope::Custom(vec![2, 1]),
                ScopeFilter::DeptIn(vec![2, 1]),
            ),
            (DataScope::SelfOnly(7), ScopeFilter::SelfOnly(7)),
            (
                DataScope::CustomOrSelf {
                    dept_ids: vec![3],
                    user_id: 9,
                },
                ScopeFilter::DeptInOrSelf {
                    depts: vec![3],
                    user_id: 9,
                },
            ),
        ];
        for (ds, want) in cases {
            assert_eq!(map_scope(&ds), want);
        }
    }

    // ---- cache_get:命中 / 未命中 / 过期 ----

    #[test]
    fn cache_get_returns_fresh_entry_and_misses_other_users() {
        let now = Instant::now();
        let mut m = CacheMap::new();
        m.insert(7, (ScopeFilter::SelfOnly(7), now));
        assert_eq!(
            cache_get(&m, SCOPE_TTL, now, 7),
            Some(ScopeFilter::SelfOnly(7))
        );
        assert_eq!(cache_get(&m, SCOPE_TTL, now, 8), None);
    }

    #[test]
    fn cache_get_treats_expired_entry_as_miss() {
        // 时间作为参数注入,不用 sleep:边界(恰好 ttl)即过期,确定性成立
        let now = Instant::now();
        let mut m = CacheMap::new();
        m.insert(7, (ScopeFilter::All, now));
        assert_eq!(
            cache_get(&m, SCOPE_TTL, now + Duration::from_secs(59), 7),
            Some(ScopeFilter::All),
            "59s < 60s:仍在 TTL 内,必须命中"
        );
        assert_eq!(
            cache_get(&m, SCOPE_TTL, now + SCOPE_TTL, 7),
            None,
            "恰好到 ttl 视为过期"
        );
    }

    // ---- cache_put:覆盖 / 超限全清 ----

    #[test]
    fn cache_put_refreshes_existing_key_in_place() {
        let now = Instant::now();
        let mut m = CacheMap::new();
        cache_put(&mut m, CACHE_CAPACITY, now, 7, ScopeFilter::All);
        cache_put(&mut m, CACHE_CAPACITY, now, 7, ScopeFilter::SelfOnly(7));
        assert_eq!(
            cache_get(&m, SCOPE_TTL, now, 7),
            Some(ScopeFilter::SelfOnly(7))
        );
    }

    #[test]
    fn cache_put_clears_all_on_overflow_with_new_key() {
        let now = Instant::now();
        let mut m: CacheMap = (0..CACHE_CAPACITY as i64)
            .map(|uid| (uid, (ScopeFilter::All, now)))
            .collect();
        assert_eq!(m.len(), CACHE_CAPACITY);
        // 第 1025 个**新**用户 → 全清,只剩新条目
        cache_put(
            &mut m,
            CACHE_CAPACITY,
            now,
            999_999,
            ScopeFilter::SelfOnly(999_999),
        );
        assert_eq!(m.len(), 1, "超限全清后只应有新写入的条目");
        assert_eq!(
            cache_get(&m, SCOPE_TTL, now, 999_999),
            Some(ScopeFilter::SelfOnly(999_999))
        );
        assert_eq!(cache_get(&m, SCOPE_TTL, now, 0), None, "旧条目不得幸存");
    }

    #[test]
    fn cache_put_at_capacity_does_not_clear_when_refreshing() {
        let now = Instant::now();
        let mut m: CacheMap = (0..CACHE_CAPACITY as i64)
            .map(|uid| (uid, (ScopeFilter::All, now)))
            .collect();
        // 已满,但刷新的是已有键 → 不触发全清
        cache_put(&mut m, CACHE_CAPACITY, now, 0, ScopeFilter::SelfOnly(0));
        assert_eq!(m.len(), CACHE_CAPACITY);
        assert_eq!(cache_get(&m, SCOPE_TTL, now, 1), Some(ScopeFilter::All));
    }

    // ---- cache_invalidate:定点失效 / 幂等 ----

    #[test]
    fn cache_invalidate_removes_only_target_and_is_idempotent() {
        let now = Instant::now();
        let mut m = CacheMap::new();
        m.insert(1, (ScopeFilter::All, now));
        m.insert(2, (ScopeFilter::All, now));
        cache_invalidate(&mut m, 1);
        assert!(!m.contains_key(&1), "目标用户被清");
        assert!(m.contains_key(&2), "他人缓存不受影响");
        // 不存在的键:幂等,不 panic
        cache_invalidate(&mut m, 42);
    }

    // ---- cache_invalidate_all:全清 / 幂等 ----

    #[test]
    fn cache_invalidate_all_clears_every_entry_and_is_idempotent() {
        let now = Instant::now();
        let mut m = CacheMap::new();
        m.insert(1, (ScopeFilter::All, now));
        m.insert(2, (ScopeFilter::SelfOnly(2), now));
        cache_invalidate_all(&mut m);
        assert!(m.is_empty(), "全清后不得有幸存条目");
        // 空表:幂等,不 panic
        cache_invalidate_all(&mut m);
    }

    // ---- 失效槽(ScopeCacheSlot):空槽 no-op / 装配后分发 ----

    #[tokio::test]
    async fn slot_helpers_noop_on_empty_and_dispatch_when_set() {
        let empty: ScopeCacheSlot = empty_scope_cache_slot();
        // 空槽(未装配/测试降级路径):no-op,不得 panic
        slot_invalidate_all(&empty);
        slot_invalidate(&empty, 7);

        let db = app_data::testing::sqlite_memory_empty().await;
        let resolver = CachedScopeResolver::new(
            Arc::new(db),
            Arc::new(PermissionService::for_test()),
        );
        let now = Instant::now();
        resolver.lock_write().insert(7, (ScopeFilter::All, now));
        resolver
            .lock_write()
            .insert(8, (ScopeFilter::SelfOnly(8), now));
        let slot: ScopeCacheSlot = Arc::new(OnceLock::new());
        assert!(slot.set(Arc::new(resolver)).is_ok(), "测试槽装配只此一次");
        slot_invalidate(&slot, 7);
        assert!(
            !slot.get().unwrap().lock_read().contains_key(&7),
            "定点失效必须清目标用户"
        );
        assert!(
            slot.get().unwrap().lock_read().contains_key(&8),
            "定点失效不得伤及他人"
        );
        slot_invalidate_all(&slot);
        assert!(
            slot.get().unwrap().lock_read().is_empty(),
            "全清必须清空所有条目"
        );
    }

    // ---- 结构级:invalidate_all 后缓存清空、下次 resolve 重查 ----

    #[tokio::test]
    async fn invalidate_all_empties_cache_so_next_resolve_requeries() {
        use crate::menu::model::menu::SUPER_ADMIN_USER_ID;
        let r = resolver_over_empty_sqlite().await;
        let p = Principal {
            user_id: SUPER_ADMIN_USER_ID,
            dept_id: None,
        };
        r.resolve(&p).await.unwrap();
        assert_eq!(r.lock_read().len(), 1, "resolve 后条目入缓存");
        r.invalidate_all();
        assert_eq!(r.lock_read().len(), 0, "invalidate_all 后缓存必须为空");
        // 重新 resolve:走取数路径并写回缓存(superadmin 恒 All)
        assert_eq!(r.resolve(&p).await.unwrap(), ScopeFilter::All);
        assert_eq!(r.lock_read().len(), 1, "重查后条目重新入缓存");
    }

    // ---- 结构级:真实 PermissionService + 内存 SQLite(不 mock;
    //      缓存逻辑用上面的纯函数测;这里只钉错误包装与装配路径)----

    async fn resolver_over_empty_sqlite() -> CachedScopeResolver {
        // resolver 自有 unscoped 通道:resolver-free 快照(生产装配同款)
        let db = app_data::testing::sqlite_memory_empty().await;
        CachedScopeResolver::new(
            Arc::new(db),
            Arc::new(PermissionService::for_test()),
        )
    }

    #[tokio::test]
    async fn new_defaults_to_60s_ttl() {
        let r = resolver_over_empty_sqlite().await;
        assert_eq!(r.ttl, Duration::from_secs(60));
    }

    #[tokio::test]
    async fn resolve_maps_super_admin_fast_path_all() {
        use crate::menu::model::menu::SUPER_ADMIN_USER_ID;
        let r = resolver_over_empty_sqlite().await;
        let p = Principal {
            user_id: SUPER_ADMIN_USER_ID,
            dept_id: None,
        };
        // 两次:第一次走取数+写缓存,第二次走命中分支;结果同为 All
        assert_eq!(r.resolve(&p).await.unwrap(), ScopeFilter::All);
        assert_eq!(r.resolve(&p).await.unwrap(), ScopeFilter::All);
        // invalidate 后缓存清空,再 resolve 重新取数(superadmin 仍 All)
        r.invalidate(SUPER_ADMIN_USER_ID);
        assert_eq!(r.resolve(&p).await.unwrap(), ScopeFilter::All);
    }

    #[tokio::test]
    async fn resolve_wraps_backend_errors_and_does_not_cache_them() {
        let r = resolver_over_empty_sqlite().await;
        // 空库无 sys_role 表 → get_data_scope 报错 → 包成
        // DataError::Backend(DbErr::Custom("数据范围解析失败: …"))
        let p = Principal {
            user_id: 12_345,
            dept_id: Some(10),
        };
        let e = r.resolve(&p).await.unwrap_err();
        assert!(
            matches!(e, DataError::Backend(_)),
            "AppError 必须按裁定包成 Backend,实际: {e:?}"
        );
        assert!(
            format!("{e:?}").contains("数据范围解析失败"),
            "包装文案按裁定,实际: {e:?}"
        );
        // 错误不落缓存:若错误被当成过滤器缓存,第二次会 Ok —— 必须仍失败
        assert!(r.resolve(&p).await.is_err());
    }
}
