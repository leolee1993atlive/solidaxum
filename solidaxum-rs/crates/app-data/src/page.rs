//! 分页请求/结果类型。字段语义对齐 app_core::vo::page(后者是对齐 RuoYi 的
//! 响应结构);app-data 不能依赖 app-core,这 ~20 行钳制逻辑是有意的副本,
//! app-core 侧做 `From<Page<T>> for TableDataInfo<T>` 时逐字段核对。

const DEFAULT_PAGE_SIZE: u64 = 10;
const MAX_PAGE_SIZE: u64 = 200;

/// 已钳制的分页参数。page 从 1 起。
///
/// 字段私有:**只允许经 [`Paging::new`] 构造**,两条不变式由构造器统一保证:
///
/// 1. `size ∈ 1..=200`(过小取默认 10,过大钳到 200)—— 分页器内部对
///    `page_size == 0` 是 `assert!`,size ≥ 1 是硬前提;
/// 2. `(page - 1) * size ≤ u64::MAX` —— `offset()` 与分页器的
///    `page_size * page`(executor/paginator.rs)都是裸 u64 乘法,page_num
///    是 HTTP 可控值,不钳就在 debug 下 panic、release 下回绕。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Paging {
    page: u64,
    size: u64,
}

impl Paging {
    pub fn new(page_num: i64, page_size: i64) -> Self {
        let raw = if page_num < 1 { 1 } else { page_num as u64 };
        let size = if page_size < 1 {
            DEFAULT_PAGE_SIZE
        } else {
            (page_size as u64).min(MAX_PAGE_SIZE)
        };
        // 不变量 2:先把 0 起页码钳到 u64::MAX / size,再加回 1。
        // (raw - 1) 不会下溢(raw ≥ 1);saturating_add 兜住 size == 1 时
        // u64::MAX + 1 的角例 —— 此时任何 raw 都本就满足不变量,不钳。
        let page = (raw - 1).min(u64::MAX / size).saturating_add(1);
        Self { page, size }
    }

    /// 页号(1 起),已按不变式 2 钳制。
    pub fn page(&self) -> u64 {
        self.page
    }

    /// 页大小(1..=200)。
    pub fn size(&self) -> u64 {
        self.size
    }

    /// OFFSET 值 = (page - 1) * size。由构造器不变式保证不溢出。
    pub fn offset(&self) -> u64 {
        (self.page - 1) * self.size
    }

    pub fn limit(&self) -> u64 {
        self.size
    }
}

/// 分页结果。`rows` + `total`;HTTP 包装(code/msg)是 app-core 的事。
#[derive(Debug, Clone)]
pub struct Page<T> {
    pub rows: Vec<T>,
    pub total: i64,
    /// `total` 是否触到计数上限被截断 —— true 时真实总数 **≥ total**
    /// (前端据此渲染 “10000+”)。默认口径 [`CountMode::Capped`] 下,
    /// 结果集不到上限时恒为 false、`total` 就是精确值;要精确值请显式走
    /// `page_exact()` / `.exact_total()`。
    ///
    /// [`CountMode::Capped`]: crate::count::CountMode::Capped
    pub total_capped: bool,
    /// 触顶时的**表级**估算行数(`pg_class.reltuples` /
    /// `information_schema.tables.table_rows`),用于把 “10000+” 换成
    /// “约 20 万”。
    ///
    /// 三条限制,写在类型上省得下游误用:
    /// 1. 只在 `total_capped` 为 true 时才可能有值;
    /// 2. **不认 WHERE** —— 它是整张表的量级,所以只在查询无用户过滤条件时给出;
    /// 3. **不精确**(MySQL 实测偏差 1.4%),只进展示位,不得参与页数计算。
    pub total_approx: Option<i64>,
}

impl<T> Page<T> {
    /// 精确计数的结果(未触顶)。
    pub fn exact(rows: Vec<T>, total: i64) -> Self {
        Self {
            rows,
            total,
            total_capped: false,
            total_approx: None,
        }
    }

    /// 只换行类型(Model → Vo 的映射),计数口径整体保留。
    ///
    /// 手写 `TableDataInfo::new(mapped_rows, page.total)` 会把 `total_capped`
    /// / `total_approx` 悄悄丢掉 —— 前端于是拿到一个被上限截断、却看着像
    /// 精确值的 total。不报错、不告警,只是数字是错的。
    ///
    /// 映射能消费 Model 时用这个;只能借用(`.iter().map(Vo::from)`)时用
    /// app-core 侧的 `TableDataInfo::from_page_rows(&page, rows)`,同一目的。
    pub fn map_rows<U>(self, f: impl FnMut(T) -> U) -> Page<U> {
        // 解构而不是 `..self`:将来给 Page 加字段时,这里会**编译不过**,
        // 逼着新字段也跟着搬 —— 正是本方法存在的理由。
        let Page {
            rows,
            total,
            total_capped,
            total_approx,
        } = self;
        Page {
            rows: rows.into_iter().map(f).collect(),
            total,
            total_capped,
            total_approx,
        }
    }

    /// 同一批结果换一种行类型,计数口径整体保留 —— 行已在别处映射好时用
    /// (`page.rows.iter().map(Vo::from).collect()` 这种借用式映射)。
    pub fn with_rows<U>(&self, rows: Vec<U>) -> Page<U> {
        Page {
            rows,
            total: self.total,
            total_capped: self.total_capped,
            total_approx: self.total_approx,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paging_clamps_to_design_defaults() {
        let p = Paging::new(0, 0);
        assert_eq!((p.page(), p.size()), (1, 10)); // 默认 1/10
        let p = Paging::new(3, 999);
        assert_eq!(p.size(), 200); // 上限 200
        let p = Paging::new(2, 20);
        assert_eq!((p.page(), p.size()), (2, 20)); // 常规值原样通过
    }

    #[test]
    fn paging_offset_is_zero_based() {
        assert_eq!(Paging::new(3, 20).offset(), 40);
    }

    /// page_num 是 HTTP 可控值:i64::MAX 若不钳,(page-1)*size
    /// 在 debug 下 panic、release 下回绕,分页器内部的 page_size * page 同理。
    /// 这里钉住钳制后的饱和值:(page-1) 恰好等于 u64::MAX / size。
    #[test]
    fn paging_huge_page_saturates_without_overflow() {
        let p = Paging::new(i64::MAX, 10);
        assert_eq!(p.page(), u64::MAX / 10 + 1);
        assert_eq!(p.offset(), (u64::MAX / 10) * 10);
        // size = 1 的角例:任何页号都满足不变式,不得误钳/溢出
        let p = Paging::new(i64::MAX, 1);
        assert_eq!(p.page(), i64::MAX as u64);
        assert_eq!(p.offset(), i64::MAX as u64 - 1);
    }
}
