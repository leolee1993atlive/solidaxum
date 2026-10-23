use serde::{Deserialize, Deserializer, Serialize};
use validator::Validate;

const DEFAULT_PAGE_SIZE: i64 = 10;
const MAX_PAGE_SIZE: i64 = 200;

pub fn deserialize_opt_i64_from_str<'de, D>(
    deserializer: D,
) -> Result<Option<i64>, D::Error>
where
    D: Deserializer<'de>,
{
    let opt = Option::<String>::deserialize(deserializer)?;
    match opt {
        None => Ok(None),
        Some(s) if s.trim().is_empty() => Ok(None),
        Some(s) => s
            .trim()
            .parse::<i64>()
            .map(Some)
            .map_err(serde::de::Error::custom),
    }
}

#[derive(Debug, Clone, Default, Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct PageQuery {
    #[serde(default, deserialize_with = "deserialize_opt_i64_from_str")]
    #[validate(range(min = 1, max = 1000))]
    pub page_num: Option<i64>,
    #[serde(default, deserialize_with = "deserialize_opt_i64_from_str")]
    #[validate(range(min = 1, max = 200))]
    pub page_size: Option<i64>,
    pub order_by_column: Option<String>,
    pub is_asc: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TableDataInfo<T> {
    pub code: i32,
    pub msg: String,
    pub rows: Vec<T>,
    pub total: i64,
    /// `total` 触到计数上限被截断:真实总数 **≥ total**,前端可渲染成
    /// “10000+”。默认口径下业务表到不了上限,这里恒为 false、total 就是精确值。
    ///
    /// 两个新字段都**只增不改**:老前端不认识它们会直接忽略,`total` 的
    /// 含义与位置一字未动,分页组件照常工作。
    #[serde(default)]
    pub total_capped: bool,
    /// 触顶时的表级估算行数(不认 WHERE、不精确)。仅供展示 “约 20 万”,
    /// **不得**参与页数计算。不适用时为 null。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_approx: Option<i64>,
}

impl<T> TableDataInfo<T> {
    pub fn new(rows: Vec<T>, total: i64) -> Self {
        Self {
            code: 200,
            msg: "查询成功".to_string(),
            rows,
            total,
            total_capped: false,
            total_approx: None,
        }
    }

    /// 带计数口径的构造(上限计数触顶时用)。常规路径走 [`Self::new`]。
    pub fn with_count_meta(
        rows: Vec<T>,
        total: i64,
        total_capped: bool,
        total_approx: Option<i64>,
    ) -> Self {
        Self {
            total_capped,
            total_approx,
            ..Self::new(rows, total)
        }
    }

    /// 行已被另行映射(Model → Vo)时,从原 `Page` 取回 total 与计数口径。
    ///
    /// **仓储把 Model 映射成 Vo 之后交付分页结果,只能走这个口子。** 手写
    /// `TableDataInfo::new(mapped_rows, page.total)` 会把 `total_capped` /
    /// `total_approx` 静默丢掉 —— 前端于是拿到一个被上限截断、却看着像精确值
    /// 的 total。不报错、不告警,只是数字是错的:20 万行 sys_user 上实测过
    /// 这个形状 —— `total=10000` 配 `totalCapped=false`,自相矛盾却无人报警。
    pub fn from_page_rows<S>(page: &app_data::Page<S>, rows: Vec<T>) -> Self {
        Self::with_count_meta(
            rows,
            page.total,
            page.total_capped,
            page.total_approx,
        )
    }

    /// 同一批结果换一种行类型:total 与计数口径原样继承。
    ///
    /// 用在**服务层已经拿到 `TableDataInfo<Model>`、handler 再映射成 Vo** 的那一段
    /// (oss 就是这个形状)。与 [`Self::from_page_rows`] 同一目的,只是上游是
    /// 另一个 `TableDataInfo` 而不是 `Page`。
    pub fn with_rows<U>(&self, rows: Vec<U>) -> TableDataInfo<U> {
        TableDataInfo::with_count_meta(
            rows,
            self.total,
            self.total_capped,
            self.total_approx,
        )
    }

    pub fn empty() -> Self {
        Self {
            code: 200,
            msg: "查询成功".to_string(),
            rows: Vec::new(),
            total: 0,
            total_capped: false,
            total_approx: None,
        }
    }
}

pub type PageResult<T> = TableDataInfo<T>;

pub fn normalize_paging(q: &PageQuery) -> (i64, i64, i64) {
    let mut current = q.page_num.unwrap_or(1);
    let mut size = q.page_size.unwrap_or(DEFAULT_PAGE_SIZE);

    if current < 1 {
        current = 1;
    }
    if size < 1 {
        size = DEFAULT_PAGE_SIZE;
    }
    if size > MAX_PAGE_SIZE {
        size = MAX_PAGE_SIZE;
    }

    let offset = (current - 1) * size;
    (current, size, offset)
}

pub fn build_order_by(
    order_by_column: &Option<String>,
    is_asc: &Option<String>,
    allowed: &[(&str, &str)],
) -> Option<String> {
    let column_key = order_by_column.as_deref()?.trim();
    if column_key.is_empty() {
        return None;
    }

    let column = allowed
        .iter()
        .find_map(|(key, col)| (*key == column_key).then_some(*col))?;

    let dir = match is_asc.as_deref().map(|s| s.to_ascii_lowercase()) {
        Some(ref s) if s == "asc" || s == "ascending" => "asc",
        Some(ref s) if s == "desc" || s == "descending" => "desc",
        _ => "asc",
    };

    Some(format!("{column} {dir}"))
}

pub fn sanitize_sort(
    sort: &Option<String>,
    order: &Option<String>,
    allowed: &[(&str, &str)],
) -> Option<String> {
    let sort = sort.as_deref()?.trim();
    if sort.is_empty() {
        return None;
    }
    let column = allowed
        .iter()
        .find_map(|(key, col)| (*key == sort).then_some(*col))?;
    let dir = match order.as_deref().map(|s| s.to_ascii_lowercase()) {
        Some(ref s) if s == "asc" => "asc",
        Some(ref s) if s == "desc" => "desc",
        _ => "desc",
    };
    Some(format!("{column} {dir}"))
}

/// 设计 12:逐字段核对 —— code 200 / msg 查询成功 / rows / total,
/// 与 TableDataInfo::new 同源,不另写一份字段。计数口径(截断标记 + 估算值)
/// 一并透传,不在这里丢掉。
impl<T> From<app_data::Page<T>> for TableDataInfo<T> {
    fn from(p: app_data::Page<T>) -> Self {
        Self::with_count_meta(p.rows, p.total, p.total_capped, p.total_approx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_from_converts_with_ruoyi_envelope() {
        // 设计 12:逐字段核对 —— code 200 / msg 查询成功 / rows / total
        let p = app_data::Page::exact(vec![1i64, 2, 3], 42);
        let t = TableDataInfo::from(p);
        assert_eq!(t.code, 200);
        assert_eq!(t.msg, "查询成功");
        assert_eq!(t.rows, vec![1, 2, 3]);
        assert_eq!(t.total, 42);
        // 未触顶的常规轮次:total 就是精确值,两个新字段不改变任何含义
        assert!(!t.total_capped);
        assert_eq!(t.total_approx, None);
    }

    /// 上限计数触顶时,截断标记与估算值必须原样透传到 VO ——
    /// 在 From 里丢掉的话,前端只能看到一个"恰好等于 10000"的总数,
    /// 分不清是真有 1 万条还是被截断了。
    #[test]
    fn page_from_carries_count_metadata() {
        let p = app_data::Page {
            rows: vec![1i64],
            total: 10_000,
            total_capped: true,
            total_approx: Some(200_002),
        };
        let t = TableDataInfo::from(p);
        assert_eq!(t.total, 10_000);
        assert!(t.total_capped);
        assert_eq!(t.total_approx, Some(200_002));
    }

    /// 老前端按 `total` 取数、按 rows 渲染,新字段只增不改:
    /// 未触顶时 totalApprox 根本不出现在 JSON 里(skip_serializing_if)。
    #[test]
    fn serialized_shape_stays_backward_compatible() {
        let t = TableDataInfo::new(vec![1i64], 3);
        let j = serde_json::to_value(&t).unwrap();
        assert_eq!(j["code"], 200);
        assert_eq!(j["total"], 3);
        assert_eq!(j["totalCapped"], false);
        assert!(j.get("totalApprox").is_none());

        let t =
            TableDataInfo::with_count_meta(vec![1i64], 10_000, true, Some(9));
        let j = serde_json::to_value(&t).unwrap();
        assert_eq!(j["totalCapped"], true);
        assert_eq!(j["totalApprox"], 9);
    }
}
