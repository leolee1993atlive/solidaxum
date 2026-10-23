//! 分页总数的计数策略:**上限计数**(默认)+ **目录估算**(触顶时的数量级)。
//!
//! # 为什么默认不做全量精确计数
//!
//! 原实现走 `PaginatorTrait::num_items()`,即 `COUNT(*) FROM (整个查询)`,
//! 代价与**结果集大小**成正比 —— 与用户实际要看的那一页无关。本机实测
//! (PG 18 / MySQL 8.4,20 万行 `sys_user`,`del_flag` 单列索引):
//!
//! | 计数方式 | PG | MySQL |
//! |---|---|---|
//! | 全量精确 `COUNT(*)` | 7.9 ms | 47.7 ms |
//! | 上限计数(投影收窄) | 0.6 ms | 3.0 ms |
//!
//! 上限计数把子查询钉上 `LIMIT cap + 1`:
//!
//! - 结果 ≤ cap → **就是精确值**,与旧行为逐位相同;
//! - 结果 > cap → 返回 cap 并置 [`Page::total_capped`],由前端渲染成 “10000+”。
//!
//! 也就是说,行数不到上限的表**没有任何语义变化**,只是快一个数量级。
//!
//! [`Page::total_capped`]: crate::page::Page::total_capped
//!
//! # 为什么必须把子查询的投影收窄成常量
//!
//! 同一条上限计数,子查询选 `1` 还是选原列表,两个引擎的差别完全不同
//! (EXPLAIN ANALYZE,同上环境,cap = 10000):
//!
//! | 子查询投影 | PG | MySQL |
//! |---|---|---|
//! | `SELECT *`(原列表) | 1.2 ms(planner 自行剪枝 → Index Only Scan) | **26.1 ms**(Index lookup,物化 10001 条宽行) |
//! | `SELECT 1` | 0.6 ms | **3.0 ms**(Covering index lookup) |
//!
//! PG 的规划器发现派生表的列没人引用会自己剪掉,**MySQL 不会**:它老老实实
//! 物化 10001 条完整行。所以收窄投影不是锦上添花 —— 少了它,MySQL 上的
//! 上限计数只能从 47.7 ms 降到 26.1 ms(1.8×),有了它才是 3.0 ms(15.7×)。

use sea_orm::{
    ConnectionTrait, DbBackend,
    sea_query::{Expr, SelectStatement},
};

use crate::{context::ContextCore, dialect, error::DataError};

/// 默认计数上限。
///
/// 选 1 万的依据是业务侧的实际分布:自有业务表基本到不了万级,这条路径上
/// 上限**不会触发**,总数仍是精确值;真正会触顶的是患者表 / 医嘱表那类
/// 第三方大表,而那种表要的恰好只是「量级对不对」,不是精确值。
pub const DEFAULT_COUNT_CAP: u64 = 10_000;

/// 分页总数的计数口径。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CountMode {
    /// 上限计数(默认)。总数 ≤ cap 时为精确值;超出时返回 cap 并置 capped 标记。
    Capped(u64),
    /// 全量精确计数。**代价与结果集大小成正比**,只给确实要精确值的场景
    /// (如与第三方数据对账)显式选用。
    Exact,
}

impl Default for CountMode {
    fn default() -> Self {
        Self::Capped(DEFAULT_COUNT_CAP)
    }
}

/// 按口径数总数,返回 `(total, capped)`。
///
/// `sub` 是**已施加完策略**(软删除 + 行级范围)的查询语句 —— 计数与取页
/// 必须同源,否则总数与行对不上。
pub(crate) async fn count_total(
    core: &ContextCore<'_>,
    sub: SelectStatement,
    mode: CountMode,
) -> Result<(i64, bool), DataError> {
    let backend = core.get_database_backend();
    match mode {
        CountMode::Exact => {
            let sub = prepare_sub(backend, sub, None);
            Ok((count_rows(core, sub).await?, false))
        }
        CountMode::Capped(cap) => {
            let sub = prepare_sub(backend, sub, Some(cap));
            let n = count_rows(core, sub).await?;
            // 子查询取的是 cap + 1 行:数到 cap + 1 说明"至少还有一条",
            // 对外只承认 cap,并把"被截断"这件事显式带出去。
            if n as u64 > cap {
                Ok((cap as i64, true))
            } else {
                Ok((n, false))
            }
        }
    }
}

/// 把业务查询整形成"只用来数行"的子查询。
///
/// 三件事都不影响计数结果,但都影响代价:
/// 1. `reset_limit` / `reset_offset` —— 数的是全集,不是某一页(与
///    `num_items()` 同口径);
/// 2. `clear_order_by` —— 前 N 行有几行与顺序无关,留着白排序;
/// 3. 投影收窄成常量 —— 见模块头的 MySQL 8.6× 实测。
fn prepare_sub(
    backend: DbBackend,
    mut sub: SelectStatement,
    cap: Option<u64>,
) -> SelectStatement {
    sub.reset_limit().reset_offset().clear_order_by();
    if projection_can_narrow(&backend.build(&sub).sql) {
        sub.clear_selects().expr(Expr::cust("1"));
    }
    if let Some(cap) = cap {
        sub.limit(cap.saturating_add(1));
    }
    sub
}

/// 子查询的投影能否收窄成常量。
///
/// `COUNT(*) FROM (SELECT 1 FROM t WHERE ..)` 与 `COUNT(*) FROM (SELECT * FROM
/// t WHERE ..)` 恒等 —— **除非子查询带 DISTINCT**:`SELECT DISTINCT 1` 恒为
/// 一行,总数会静默塌成 1。
///
/// sea-query 的 `SelectStatement::distinct` 字段是 `pub(crate)`,没有访问器,
/// 只能看渲染结果。判据是**整条 SQL 的开头**而不是"含有 DISTINCT":列里若
/// 出现字面量 `'DISTINCT ..'`,前面必有引号,不会误判。开头不是 `SELECT `
/// 的形态(例如带 `WITH` 的 CTE)一律**不收窄** —— 认不出就走恒正确的那条
/// 路,慢一点而已,不拿正确性换速度。
fn projection_can_narrow(sql: &str) -> bool {
    let Some(rest) = sql.strip_prefix("SELECT ") else {
        return false;
    };
    // SELECT DISTINCT / SELECT DISTINCT ON (..) / SELECT ALL
    !rest.starts_with("DISTINCT") && !rest.starts_with("ALL ")
}

/// `SELECT COUNT(*) FROM (<sub>) sub_query`,与 `num_items()` 同一形状
/// (含 `try_get::<i64>` —— fork 的 paginator 对四个后端都这么取)。
async fn count_rows(
    core: &ContextCore<'_>,
    sub: SelectStatement,
) -> Result<i64, DataError> {
    let Some(row) = core.query_one(&count_stmt(sub)).await? else {
        return Ok(0);
    };
    Ok(row.try_get::<i64>("", "num_items")?)
}

/// 查询的**行集指纹**:归一化(去掉 limit / offset / order by,投影收窄)后
/// 渲染出的语句。`Statement` 的 PartialEq 覆盖 SQL 文本**与绑定值**,所以
/// 两条查询指纹相等 ⇒ 选出的行集完全相同。
///
/// 用来回答"这个查询除了策略条件外还加了别的过滤吗" —— 拿它跟同样施加了
/// 策略的空查询比一下就知道,不必让调用方自己声明(声明就会有人忘)。
pub(crate) fn row_set_key(
    backend: DbBackend,
    sub: SelectStatement,
) -> sea_orm::Statement {
    backend.build(&prepare_sub(backend, sub, None))
}

fn count_stmt(sub: SelectStatement) -> SelectStatement {
    SelectStatement::new()
        .expr(Expr::cust("COUNT(*) AS num_items"))
        .from_subquery(sub, "sub_query")
        .to_owned()
}

/// 表级估算行数(目录估算)。方言 SQL 见 [`dialect::approx_row_count`]。
///
/// 返回 `None` 的三种情形,调用方一视同仁地退化成"没有估算值":
/// 方言不支持(SQLite / 达梦)、表不存在、统计从未采集过(PG 返回 -1)。
///
/// **它不认 WHERE**:给出的是整张表的量级,不是当前查询条件下的行数。
pub(crate) async fn approx_row_count(
    core: &ContextCore<'_>,
    table_name: &str,
) -> Result<Option<i64>, DataError> {
    let backend: DbBackend = core.get_database_backend();
    let Some(stmt) = dialect::approx_row_count(backend, table_name) else {
        return Ok(None);
    };
    let Some(row) = core.query_one_raw(stmt).await? else {
        return Ok(None);
    };
    // est 可空(MySQL 视图表的 table_rows 为 NULL),故取 Option。
    let est: Option<i64> = row.try_get("", "est")?;
    // PG 14+ 用 -1 表示"从未 ANALYZE",不是"零行"。
    Ok(est.filter(|n| *n >= 0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_mode_is_capped_at_ten_thousand() {
        assert_eq!(CountMode::default(), CountMode::Capped(10_000));
    }

    /// 收窄判据钉死在"开头"上:DISTINCT 一律不收窄(否则总数塌成 1),
    /// 列里的 DISTINCT 字面量不得误判。
    #[test]
    fn projection_narrowing_is_decided_by_statement_head() {
        assert!(projection_can_narrow("SELECT `a`, `b` FROM `t`"));
        assert!(!projection_can_narrow("SELECT DISTINCT `a` FROM `t`"));
        assert!(!projection_can_narrow(
            r#"SELECT DISTINCT ON ("a") "a" FROM "t""#
        ));
        assert!(!projection_can_narrow("SELECT ALL `a` FROM `t`"));
        // 列里的字面量前面必有引号,不会被当成 DISTINCT 修饰符
        assert!(projection_can_narrow("SELECT 'DISTINCT x' FROM `t`"));
        // 认不出的形态(CTE)走恒正确的那条路
        assert!(!projection_can_narrow(
            "WITH `c` AS (SELECT 1) SELECT * FROM `c`"
        ));
    }

    /// 三个方言的计数 SQL **逐字锚定**。
    ///
    /// CI 只有 SQLite,MySQL/PG 的这两条语句在测试里一次也跑不到 ——
    /// 而它们正是分页总数的全部实现。这里把文本钉死,配合 count.rs 模块头
    /// 记录的实机 EXPLAIN,改动时至少不会悄无声息地变形。
    fn capped_sql(backend: DbBackend) -> String {
        use crate::{
            sea_orm::{
                ColumnTrait, EntityTrait, QueryFilter, QueryOrder, QueryTrait,
            },
            testing::sandbox,
        };
        let sel = sandbox::Entity::find()
            .filter(sandbox::Column::DelFlag.eq("0"))
            // 排序必须在计数语句里消失(数前 N 行与顺序无关,留着白排)
            .order_by_desc(sandbox::Column::Id);
        let sub = prepare_sub(backend, sel.into_query(), Some(10_000));
        backend.build(&count_stmt(sub)).sql
    }

    #[test]
    fn capped_count_sql_is_pinned_per_dialect() {
        assert_eq!(
            capped_sql(DbBackend::MySql),
            "SELECT COUNT(*) AS num_items FROM (SELECT 1 FROM `sandbox` \
             WHERE `sandbox`.`del_flag` = ? LIMIT ?) AS `sub_query`"
        );
        assert_eq!(
            capped_sql(DbBackend::Postgres),
            r#"SELECT COUNT(*) AS num_items FROM (SELECT 1 FROM "sandbox" "#
                .to_owned()
                + r#"WHERE "sandbox"."del_flag" = $1 LIMIT $2) AS "sub_query""#
        );
        assert_eq!(
            capped_sql(DbBackend::Sqlite),
            r#"SELECT COUNT(*) AS num_items FROM (SELECT 1 FROM "sandbox" "#
                .to_owned()
                + r#"WHERE "sandbox"."del_flag" = ? LIMIT ?) AS "sub_query""#
        );
    }

    /// 目录估算的方言 SQL 同样锚定 —— 它在 CI 里必然返回 None(SQLite),
    /// 真正跑起来的 PG/MySQL 两条谁也测不到。
    #[test]
    fn approx_row_count_sql_is_pinned_per_dialect() {
        let sql = |b| dialect::approx_row_count(b, "sys_user").map(|s| s.sql);
        assert_eq!(
            sql(DbBackend::Postgres).unwrap(),
            "SELECT reltuples::bigint AS est FROM pg_class \
             WHERE oid = to_regclass($1)"
        );
        assert_eq!(
            sql(DbBackend::MySql).unwrap(),
            "SELECT CAST(table_rows AS SIGNED) AS est \
             FROM information_schema.tables \
             WHERE table_schema = DATABASE() AND table_name = ?"
        );
        assert!(sql(DbBackend::Sqlite).is_none());
        assert!(sql(DbBackend::Dm).is_none(), "达梦无实机验证,不猜 SQL");
    }
}
