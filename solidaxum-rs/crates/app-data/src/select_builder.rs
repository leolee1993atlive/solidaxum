//! select 逃生舱:保留 SeaORM 完整构造力,策略照施加(设计 6.4)。
//!
//! [`SelectQuery`] 由 `DataContext::select` / `TxnContext::select` 构造(入口在
//! context.rs),字段全私有、不暴露执行器 —— 执行只发生在 [`SelectQuery::all`] /
//! [`SelectQuery::one`],策略(软删除 + 行级范围三态)在执行前施加完毕。
use crate::{
    context::ContextCore,
    count::CountMode,
    error::DataError,
    page::{Page, Paging},
    policy::DataPolicy,
    principal::{ScopeFilter, scope_condition},
    sea_orm::{ColumnTrait, Condition, QueryFilter},
};

pub struct SelectQuery<'a, E: DataPolicy> {
    core: &'a ContextCore<'a>,
    sel: sea_orm::Select<E>,
    include_deleted: bool,
    decl: ScopeDecl,
    count_mode: CountMode,
}

/// 行级范围的三态声明(设计 6.4)。
enum ScopeDecl {
    /// CONSTRAINED=true 的默认:范围落在驱动实体自身的列上。
    Driver,
    /// join 落点声明:闭包在 `constrain_via::<E2>()` 调用点捕获 E2 的列
    /// (Copy 单元枚举),执行期拿到 ScopeFilter 再合成条件。
    Via(ScopeFn),
    /// 显式豁免(`without_row_scope`)。名字扎眼、可 grep,不做静默默认。
    None,
}

/// Via 的闭包形态(独立别名,免 type_complexity)。
type ScopeFn = Box<
    dyn FnOnce(&ScopeFilter) -> Result<Option<Condition>, DataError> + Send,
>;

impl<'a, E> SelectQuery<'a, E>
where
    E: DataPolicy,
{
    pub(crate) fn new(
        core: &'a ContextCore<'a>,
        sel: sea_orm::Select<E>,
    ) -> Self {
        Self {
            core,
            sel,
            include_deleted: false,
            decl: ScopeDecl::Driver,
            count_mode: core.count_mode(),
        }
    }

    /// `page()` 的 total 改为**全量精确计数**。
    ///
    /// 默认口径是上限计数(1 万):不到上限时本就是精确值,所以这行字只在
    /// 结果集**确实可能过万、且确实要精确数**时才需要 —— 典型是与第三方数据
    /// 对账。代价与结果集大小成正比(20 万行实测 PG 7.9 ms / MySQL 47.7 ms)。
    pub fn exact_total(mut self) -> Self {
        self.count_mode = CountMode::Exact;
        self
    }

    /// 换一个计数上限(默认 [`crate::count::DEFAULT_COUNT_CAP`] = 1 万)。
    /// 上限越低越省,代价是越容易触顶、总数越早变成"≥ 上限"。
    pub fn total_cap(mut self, cap: u64) -> Self {
        self.count_mode = CountMode::Capped(cap);
        self
    }

    /// 单次放行软删除(设计 7.3)。只豁免软删除,行级范围照施加。
    pub fn include_deleted(mut self) -> Self {
        self.include_deleted = true;
        self
    }

    /// 行级范围落在**声明的** E2 实体列上(join 落点声明,设计 6.4)。
    ///
    /// **只适用于 `CONSTRAINED = false` 的驱动实体。** 驱动自身受约束时,本声明
    /// 会顶掉驱动的行级范围(三态只走一条分支),`apply()` 因此对该组合 fail-closed
    /// 报 [`DataError::Unsupported`] —— 见那里的说明。
    ///
    /// 只负责合成带表限定前缀的条件,不校验调用方真的 join 了 E2 ——
    /// 没 join 会在执行期被后端拒绝(fail-closed,不会静默放行全表)。
    pub fn constrain_via<E2: DataPolicy>(mut self) -> Self {
        let (d, o) = (E2::dept_column(), E2::owner_column());
        self.decl =
            ScopeDecl::Via(Box::new(move |sf| scope_condition(d, o, sf)));
        self
    }

    /// 显式豁免行级范围。软删除照施加;豁免必须写出这行字才能过审。
    pub fn without_row_scope(mut self) -> Self {
        self.decl = ScopeDecl::None;
        self
    }

    /// 施加策略,返回可直接执行的 Select。吃掉 self —— 调用方再也拿不回
    /// 未施加策略的查询。
    async fn apply(self) -> Result<sea_orm::Select<E>, DataError> {
        Ok(self.apply_with_scope().await?.0)
    }

    /// 与 [`Self::apply`] 同源,额外报告是否真的挂上了行级范围条件。
    /// 分页的整表估算只对"行集就是整张表"成立,范围一旦施加就必须拒绝
    /// (与 [`crate::context::ContextCore::page`] 的 `!scoped` 同口径)。
    async fn apply_with_scope(
        self,
    ) -> Result<(sea_orm::Select<E>, bool), DataError> {
        apply_policy(self.core, self.sel, self.include_deleted, self.decl).await
    }

    // apply() 吃掉 self,所以 core 必须**先**取出来 —— 写成
    // `self.apply().await?.all(self.core)` 是 use-after-move。
    pub async fn all(self) -> Result<Vec<E::Model>, DataError> {
        let core = self.core;
        Ok(self.apply().await?.all(core).await?)
    }

    pub async fn one(self) -> Result<Option<E::Model>, DataError> {
        let core = self.core;
        Ok(self.apply().await?.one(core).await?)
    }

    /// 逃生舱分页(设计 6.4 ORDER BY 裁定):排序不走窄 API 参数,
    /// 调用方在 Select 构件上 `order_by_*`;策略注入(软删除 + 三态范围)与
    /// 窄 `ctx.page` 同源。RuoYi 的 order_by_column/is_asc 白名单换算留在
    /// handler 层(转成 Column,替换字符串拼接)。
    ///
    /// total 走**上限计数**(默认 1 万,见 [`crate::count`]):不到上限时就是
    /// 精确值,超出时返回上限并置 `Page::total_capped`。要精确总数加一句
    /// [`Self::exact_total`]。
    pub async fn page(
        self,
        page_num: i64,
        page_size: i64,
    ) -> Result<Page<E::Model>, DataError>
    where
        E::Model: Send + Sync,
    {
        use crate::sea_orm::{PaginatorTrait, QueryTrait};
        let paging = Paging::new(page_num, page_size);
        // apply() 消耗 self —— 先取 core 引用与口径开关,再拿施加完策略的 Select
        let core = self.core;
        let (mode, include_deleted) = (self.count_mode, self.include_deleted);
        // 目录估算的基线只在这两档能重建:Via 持的是 FnOnce,复制不出第二份
        // (且 join 落点下"整表估算"本就不成立)。认不出就不给估算。
        let baseline = match &self.decl {
            ScopeDecl::Driver => Some(ScopeDecl::Driver),
            ScopeDecl::None => Some(ScopeDecl::None),
            ScopeDecl::Via(_) => Option::None,
        };
        let (sel, row_scoped) = self.apply_with_scope().await?;

        let (total, total_capped) =
            crate::count::count_total(core, sel.clone().into_query(), mode)
                .await?;
        let paginator = sel.clone().paginate(core, paging.limit());
        let rows = paginator.fetch_page(paging.page() - 1).await?;

        // 门控在 approx_when_unfiltered 内部(见该函数的早退):调用侧
        // 无条件调用,无法用 if 跳过。行级范围一旦施加,目录估算(不认
        // WHERE)给的是整表行数,与本次结果集无关 —— 必须抑制。
        // 窄 API(`ContextCore::page`)的 `!scoped` 同口径。
        let total_approx = approx_when_unfiltered(
            core,
            sel,
            include_deleted,
            baseline,
            total_capped,
            row_scoped,
        )
        .await?;

        Ok(Page {
            rows,
            total,
            total_capped,
            total_approx,
        })
    }

    /// 计数:策略(软删除 + 三态范围)与 `all`/`page` 同源施加,只发一条
    /// COUNT —— 不要用 `page(1, 1).total` 代替,那会多发一条 SELECT 并
    /// 白取一行(12 处调用点曾如此)。
    pub async fn count(self) -> Result<u64, DataError>
    where
        E::Model: Send + Sync,
    {
        use crate::sea_orm::PaginatorTrait;
        let core = self.core;
        let sel = self.apply().await?;
        Ok(sel.count(core).await?)
    }

    /// 只 build 不执行,给 SQL 断言用(设计 9.1 "最有价值,不连库"的那一类)。
    /// 返回 String,不构成执行能力 —— 所以可以经 `test-util` 给业务 crate 用:
    /// 38 个实体的 WHERE 穷举断言就靠这个缝。
    #[cfg(any(test, feature = "test-util"))]
    pub async fn into_sql(
        self,
        backend: sea_orm::DbBackend,
    ) -> Result<String, DataError> {
        use crate::sea_orm::QueryTrait;
        Ok(self.apply().await?.build(backend).to_string())
    }
}

/// 策略施加的本体(软删除 + 行级范围三态)。抽成自由函数是为了能对
/// **同一实体的空查询**再跑一遍,给 [`approx_when_unfiltered`] 造基线。
async fn apply_policy<E>(
    core: &ContextCore<'_>,
    mut sel: sea_orm::Select<E>,
    include_deleted: bool,
    decl: ScopeDecl,
) -> Result<(sea_orm::Select<E>, bool), DataError>
where
    E: DataPolicy,
{
    // 软删除:除非单次放行,驱动实体声明了就过滤(与 CONSTRAINED 无关)
    if !include_deleted && let Some((col, alive)) = E::soft_delete() {
        sel = sel.filter(col.eq(alive));
    }
    // 行级范围是否真的合成出条件(ScopeFilter::All → 否)。分页估算的
    // 抑制判据;软删除不算 —— 它不改变"是不是整张表"的语义(目录估算
    // 同样按整表算)。
    let mut row_scoped = false;
    // 行级范围三态。
    //
    // ScopeFilter **只在真正要用的两条分支里解析** —— 与 find_by_id / page
    // 同一口径。不需要范围的形态(without_row_scope、不受约束驱动)不该白付
    // 一次 ScopeResolver 查库(设计 7.2 的实现要查 sys_role_dept 并展开科室树),
    // 也不该在 DataSource 尚未装 resolver 时被判成 Unsupported。
    match decl {
        ScopeDecl::Driver if E::CONSTRAINED => {
            let sf = core.scope_filter().await?.clone();
            let (d, o) = (E::dept_column(), E::owner_column());
            if let Some(c) = scope_condition(d, o, &sf)? {
                sel = sel.filter(c);
                row_scoped = true;
            }
        }
        ScopeDecl::Driver => {
            // 驱动不受约束且未声明落点:fail-closed(设计 6.4)
            // table_name 收 &self(entity/base_entity.rs:35),
            // **不能写 E::table_name()**
            return Err(DataError::Unsupported(format!(
                "{} 作驱动且 CONSTRAINED=false:必须 constrain_via::<E>() \
                 或 without_row_scope()",
                E::default().table_name()
            )));
        }
        // 受约束驱动 + constrain_via 是非法组合,fail-closed 拒绝。
        //
        // 三态只会走一条分支,所以 Via 会把驱动**自身**的行级范围整条顶掉:
        // 生成的 WHERE 里只剩 E2 的条件。若调用方确实 join 了 E2(constrain_via
        // 本就是为 join 而设),SQL 能正常执行,而驱动行不再受调用者部门限制
        // —— 静默的跨部门泄漏,没有任何报错。
        //
        // 设计 6.4 只把 constrain_via 定义在「驱动实体不受约束」那一档,
        // 这里照此拒绝,不做「自作主张 AND 两侧」的补救 —— 那等于替调用方
        // 猜意图,而猜错的方向是放宽权限。
        ScopeDecl::Via(_) if E::CONSTRAINED => {
            return Err(DataError::Unsupported(format!(
                "{} 自身 CONSTRAINED=true,不能再 constrain_via::<_>():\
                 落点声明会顶掉驱动自身的行级范围。受约束驱动的范围已自动施加,\
                 删掉这次 constrain_via 即可(设计 6.4 只把它定义在不受约束的驱动上)",
                E::default().table_name()
            )));
        }
        ScopeDecl::Via(f) => {
            let sf = core.scope_filter().await?.clone();
            if let Some(c) = f(&sf)? {
                sel = sel.filter(c);
                row_scoped = true;
            }
        }
        ScopeDecl::None => {}
    }
    Ok((sel, row_scoped))
}

/// 是否允许进入表级估算的**基线比对**(真正的用户过滤条件由下游
/// `row_set_key` 再判一次)。目录估算不认 WHERE,只有"计数触顶 +
/// 未施加行级范围 + 有可重建基线"三者同时成立时,查询的行集才可能
/// 等于整张表。抽成纯函数只为把谓词语义钉在真值表上;门控本身在
/// [`approx_when_unfiltered`] 内部早退,调用侧无法用 if 跳过。
fn should_estimate(
    total_capped: bool,
    row_scoped: bool,
    baseline_available: bool,
) -> bool {
    total_capped && !row_scoped && baseline_available
}

/// 计数触顶后,决定要不要给表级估算行数。
///
/// 判据是**这个查询选出的行集是否就是整张表**(去掉软删除后)。目录估算不认
/// WHERE,查询一旦带了条件或行级范围,整表的数就跟本次结果无关,给了是误导。
///
/// **门控在本函数内部**(`should_estimate` 早退),不在调用侧 ——
/// `SelectQuery::page` 无条件调用,无法用一个 `if` 跳过抑制。要绕过只能
/// 改本函数内部的门控或调用点传入的 `total_capped`/`row_scoped`。
///
/// 过了门控之后,判定方式不是让调用方声明(声明就会有人忘、忘了不报错),
/// 而是**把同样的策略施加在一个空查询上,拿两边的行集指纹比对**:
/// `Statement` 的 PartialEq 覆盖 SQL 文本与绑定值,逐位相等才算"没加条件"。
/// 任何不确定的情形一律不给估算。
async fn approx_when_unfiltered<E>(
    core: &ContextCore<'_>,
    sel: sea_orm::Select<E>,
    include_deleted: bool,
    baseline: Option<ScopeDecl>,
    total_capped: bool,
    row_scoped: bool,
) -> Result<Option<i64>, DataError>
where
    E: DataPolicy,
{
    use sea_orm::ConnectionTrait;

    use crate::sea_orm::QueryTrait;
    let baseline_available = baseline.is_some();
    if !should_estimate(total_capped, row_scoped, baseline_available) {
        return Ok(None);
    }
    let Some(decl) = baseline else {
        return Ok(None);
    };
    let backend = core.get_database_backend();
    let (base, _) =
        apply_policy::<E>(core, E::find(), include_deleted, decl).await?;
    // scope_filter 在 core 里是 OnceCell 缓存的,这第二遍不会再查一次库。
    if crate::count::row_set_key(backend, base.into_query())
        != crate::count::row_set_key(backend, sel.into_query())
    {
        return Ok(None);
    }
    crate::count::approx_row_count(core, E::default().table_name()).await
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use crate::{
        principal::{Principal, ScopeFilter},
        sea_orm::EntityTrait,
        testing::{self, FixedResolver, sandbox},
    };

    use super::{ScopeDecl, apply_policy, should_estimate};

    /// 谓词语义锚:三个条件按真值表组合。谓词里的 `!row_scoped`(或
    /// `baseline_available`)被去掉,对应行就会从 false 变 true,本测试即失败。
    ///
    /// 本测试**不证明 `page` 的接线**:它只钉谓词语义。真正的抑制门控
    /// 位于 [`approx_when_unfiltered`](super::approx_when_unfiltered) 内部早退,
    /// `page` 侧无条件调用;
    /// SQLite 无统计目录,端到端也无法观察估算值。
    #[test]
    fn should_estimate_truth_table() {
        // capped + scoped:整表估算与结果集无关 → 禁止
        assert!(!should_estimate(true, true, true));
        assert!(!should_estimate(true, true, false));
        // capped + 未施加范围 + 有基线 → 允许(All/unscoped 的路径)
        assert!(should_estimate(true, false, true));
        // 未触顶:total 已是精确值 → 不需要估算
        assert!(!should_estimate(false, false, true));
        // 无基线(如 Via 的 FnOnce 无法重建)→ 禁止
        assert!(!should_estimate(true, false, false));
    }

    /// 回归锚:分页的整表估算抑制判据是"是否真的挂上了行级范围"。
    /// 受辖实体 + DeptIn 必须报 true;All / 显式豁免报 false。
    ///
    /// 为什么不直接断 `total_approx` —— SQLite 没有统计目录,
    /// `approx_row_count` 对任何查询都返回 None,`page()` 的端到端断言
    /// 区分不出"抑制了"与"问过但退化成 None"(见 context.rs 的
    /// `escape_hatch_scoped_page_has_no_whole_table_approx`)。
    #[tokio::test]
    async fn apply_policy_reports_row_scope_only_when_condition_attached() {
        let ds = testing::sqlite_memory().await;
        let principal = Principal {
            user_id: 1,
            dept_id: Some(10),
        };

        let dept_in = ds.derive_with_resolver(Arc::new(FixedResolver(
            ScopeFilter::DeptIn(vec![10]),
        )));
        let ctx = dept_in.context(principal.clone());
        let (_, row_scoped) = apply_policy(
            ctx.core(),
            sandbox::Entity::find(),
            false,
            ScopeDecl::Driver,
        )
        .await
        .unwrap();
        assert!(row_scoped, "DeptIn 必须报告已施加行级范围(估算须抑制)");

        let all =
            ds.derive_with_resolver(Arc::new(FixedResolver(ScopeFilter::All)));
        let ctx = all.context(principal);
        let (_, row_scoped) = apply_policy(
            ctx.core(),
            sandbox::Entity::find(),
            false,
            ScopeDecl::Driver,
        )
        .await
        .unwrap();
        assert!(!row_scoped, "All 不挂范围条件,整表估算仍可用");

        let (_, row_scoped) = apply_policy(
            ctx.core(),
            sandbox::Entity::find(),
            false,
            ScopeDecl::None,
        )
        .await
        .unwrap();
        assert!(!row_scoped, "显式豁免不施加范围");
    }
}
