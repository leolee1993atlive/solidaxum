//! ODBC 结果集到中间表示的映射。
//!
//! 当前一律以文本取值(`get_text`)再交给 SeaORM 按目标类型解析 ——
//! 实机实测雪花 ID 19 位 i64 经文本往返无精度损失。
//! 若后续出现性能瓶颈,再改为按列类型走 `get_data` 的二进制路径。

use odbc_api::Cursor;
use sea_query::Value;

use crate::DmError;

/// 中间表示。由 `vendor/sea-orm/src/driver/dm.rs` 包成 `QueryResultRow::Dm`。
/// 本 crate 不依赖 sea-orm,故不直接产出 `QueryResult`。
/// **按 SELECT 列序保存**,不能换成 `BTreeMap`。
///
/// SeaORM 的 `try_get_by_index` 与 `into_tuple::<(A, B)>()` 按列序取值,而
/// `BTreeMap` 按列名字典序排列。两者不一致时会静默取到**另一列**,且文本中间
/// 表示常常「恰好」能解析成功(字符串列尤其如此),错得无声无息。
///
/// 列数是一条 SELECT 的宽度(几个到几十个),按名查找线性扫描即可,
/// 这个规模下比有序表更快,也顺带保住了同名列(BTreeMap 会把它们合并成一个)。
#[derive(Debug, Default, Clone, PartialEq)]
pub struct DmRow {
    columns: Vec<(String, Value)>,
}

impl DmRow {
    pub fn from_ordered(columns: Vec<(String, Value)>) -> Self {
        Self { columns }
    }

    /// 按列名取值。同名列取第一个,与常见驱动的行为一致。
    pub fn get(&self, name: &str) -> Option<&Value> {
        self.columns.iter().find(|(n, _)| n == name).map(|(_, v)| v)
    }

    /// 按 SELECT 列序取值(0 基)。
    pub fn get_by_index(&self, idx: usize) -> Option<&Value> {
        self.columns.get(idx).map(|(_, v)| v)
    }

    /// 列名,按 SELECT 列序。
    pub fn keys(&self) -> impl Iterator<Item = &String> {
        self.columns.iter().map(|(n, _)| n)
    }
}

pub fn cursor_to_rows(mut cursor: impl Cursor) -> Result<Vec<DmRow>, DmError> {
    let col_count = cursor.num_result_cols()? as u16;
    let mut names = Vec::with_capacity(col_count as usize);
    for i in 1..=col_count {
        names.push(cursor.col_name(i)?);
    }

    let mut rows = Vec::new();
    while let Some(mut row) = cursor.next_row()? {
        let mut values = Vec::with_capacity(names.len());
        for (idx, name) in names.iter().enumerate() {
            let v = match read_cell(&mut row, (idx + 1) as u16)? {
                Some(text) => Value::String(Some(text)),
                None => Value::String(None),
            };
            values.push((name.clone(), v));
        }
        rows.push(DmRow::from_ordered(values));
    }
    Ok(rows)
}

/// 读取一个文本单元格。Windows 上 odbc-api 走宽字符 API,必须用
/// `get_wide_text` 取回 —— `get_text`(SQL_C_CHAR)会被驱动转成系统 ANSI
/// (中文 Windows 即 GBK),再按 UTF-8 解码就是乱码。
/// Linux/unixODBC 走窄字符 API,`get_text` 直出 UTF-8(麒麟实测中文往返正确)。
#[cfg(target_os = "windows")]
fn read_cell(
    row: &mut odbc_api::CursorRow<'_>,
    idx: u16,
) -> Result<Option<String>, DmError> {
    let mut buf: Vec<u16> = Vec::new();
    Ok(row
        .get_wide_text(idx, &mut buf)?
        .then(|| String::from_utf16_lossy(&buf)))
}

/// Linux/unixODBC 窄字符路径。
#[cfg(not(target_os = "windows"))]
fn read_cell(
    row: &mut odbc_api::CursorRow<'_>,
    idx: u16,
) -> Result<Option<String>, DmError> {
    let mut buf: Vec<u8> = Vec::new();
    Ok(row
        .get_text(idx, &mut buf)?
        .then(|| String::from_utf8_lossy(&buf).into_owned()))
}

/// 排空游标(execute 路径只关心 row_count,不取行)。
/// 供 vendor/sea-orm 的达梦 driver 使用 —— 它不直接依赖 odbc-api,
/// 无法调用 `Cursor::next_row`。
pub fn drain_cursor(mut cursor: impl Cursor) -> Result<(), DmError> {
    while cursor.next_row()?.is_some() {}
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 按下标取值必须follow SELECT 的列序,不能是列名的字典序。
    ///
    /// SeaORM 的 `try_get_by_index` 与 `into_tuple::<(A, B)>()` 都按列序取值。
    /// 若中间表示按列名排序,取到的就是**另一列** —— 而文本中间表示往往「恰好」
    /// 能解析成功(字符串列尤其如此),错得无声无息。
    #[test]
    fn index_access_follows_select_order_not_column_name_order() {
        // 列名故意逆字典序:按名排序会把 `a` 排到 `z` 前面,与 SELECT 顺序相反
        let row = DmRow::from_ordered(vec![
            (
                "z_first".to_owned(),
                Value::String(Some("第一列".to_owned())),
            ),
            (
                "a_second".to_owned(),
                Value::String(Some("第二列".to_owned())),
            ),
        ]);

        assert_eq!(
            row.get_by_index(0),
            Some(&Value::String(Some("第一列".to_owned()))),
            "下标 0 应取 SELECT 的第一列"
        );
        assert_eq!(
            row.get_by_index(1),
            Some(&Value::String(Some("第二列".to_owned()))),
            "下标 1 应取 SELECT 的第二列"
        );
    }

    /// 列名迭代同样要保持 SELECT 序 —— `QueryResult::column_names()` 依赖它。
    #[test]
    fn column_names_keep_select_order() {
        let row = DmRow::from_ordered(vec![
            ("z_first".to_owned(), Value::String(None)),
            ("a_second".to_owned(), Value::String(None)),
        ]);
        let names: Vec<&str> = row.keys().map(String::as_str).collect();
        assert_eq!(names, vec!["z_first", "a_second"]);
    }

    /// 按名取值不受顺序影响。
    #[test]
    fn name_access_still_works() {
        let row = DmRow::from_ordered(vec![
            ("z_first".to_owned(), Value::String(Some("v".to_owned()))),
            ("a_second".to_owned(), Value::String(None)),
        ]);
        assert_eq!(
            row.get("z_first"),
            Some(&Value::String(Some("v".to_owned())))
        );
        assert_eq!(row.get("a_second"), Some(&Value::String(None)));
        assert_eq!(row.get("不存在"), None);
    }
}
