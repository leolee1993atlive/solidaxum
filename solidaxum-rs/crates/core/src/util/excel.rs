use std::io::Cursor;

use calamine::{Reader, Xlsx};
use rust_xlsxwriter::{Format, Workbook};

use crate::error::{AppError, AppResult};

/// Excel 行数据转换 trait
pub trait ExcelRow: Sized {
    /// 表头
    fn headers() -> Vec<&'static str>;
    /// 转换为 Excel 行
    fn to_row(&self) -> Vec<String>;
    /// 从 Excel 记录解析
    fn from_record(record: &[String]) -> AppResult<Self>;
}

/// 单次导出的行数硬上限。
///
/// 导出是全系统唯一不受分页约束的读路径:列表端点一律 `select_page`,
/// 只有 `/export` 按查询条件取全量。xlsx 把每个单元格写成一个 XML 节点、
/// 且整本工作簿在 `save_to_buffer` 前都驻留内存,行数一大就是内存峰值的
/// 主要来源 —— 这道闸拦的就是「不带筛选条件导整张日志表」。
///
/// **超限拒绝而不是截断**:静默少给数据比报错危险得多 —— 用户会拿着
/// 残缺的 xlsx 当完整数据用,且无从察觉。
pub const EXPORT_ROW_LIMIT: usize = 50_000;

/// 导出行数闸门。取数前拿 COUNT 调用它可避免白取整张表;
/// [`ExcelBuilder::export`] 也会在写盘前再调一次兜底。
pub fn guard_export_rows(total: usize) -> AppResult<()> {
    if total > EXPORT_ROW_LIMIT {
        return Err(AppError::BadRequest(format!(
            "导出行数 {} 超过单次上限 {},请缩小查询范围(如限定时间段)后重试",
            total, EXPORT_ROW_LIMIT
        )));
    }
    Ok(())
}

/// Excel 导入导出构建器
pub struct ExcelBuilder<T: ExcelRow> {
    _marker: std::marker::PhantomData<T>,
}

impl<T: ExcelRow> ExcelBuilder<T> {
    /// 导出 Excel 文件到字节缓冲区。
    ///
    /// 行数超过 [`EXPORT_ROW_LIMIT`] 时返回 `BadRequest`。这是兜底闸:
    /// 所有导出端点都收口在此,新增端点自动受辖。数据量大的表还应在
    /// 取数**之前**用 COUNT 拦一道,避免白取(见各 export handler)。
    pub fn export(rows: &[T]) -> AppResult<Vec<u8>> {
        guard_export_rows(rows.len())?;

        let mut workbook = Workbook::new();
        let worksheet = workbook.add_worksheet();
        let header_format = Format::new().set_bold();

        for (col, header) in T::headers().iter().enumerate() {
            worksheet
                .write_string_with_format(
                    0,
                    col as u16,
                    *header,
                    &header_format,
                )
                .map_err(|e| {
                    AppError::Internal(format!("写入表头失败: {}", e))
                })?;
        }

        for (row_idx, row) in rows.iter().enumerate() {
            for (col_idx, value) in row.to_row().iter().enumerate() {
                worksheet
                    .write_string((row_idx + 1) as u32, col_idx as u16, value)
                    .map_err(|e| {
                        AppError::Internal(format!("写入单元格失败: {}", e))
                    })?;
            }
        }

        workbook
            .save_to_buffer()
            .map_err(|e| AppError::Internal(format!("Excel导出失败: {}", e)))
    }

    /// 从字节缓冲区导入 Excel 文件
    pub fn import(data: &[u8]) -> AppResult<Vec<T>> {
        let cursor = Cursor::new(data);
        let mut workbook: Xlsx<Cursor<&[u8]>> =
            calamine::open_workbook_from_rs(cursor).map_err(|e| {
                AppError::BadRequest(format!("Excel解析失败: {}", e))
            })?;

        let sheet_name = workbook
            .sheet_names()
            .first()
            .ok_or_else(|| {
                AppError::BadRequest("Excel文件没有工作表".to_string())
            })?
            .clone();

        let range = workbook.worksheet_range(&sheet_name).map_err(|e| {
            AppError::BadRequest(format!("读取Excel工作表失败: {}", e))
        })?;

        let mut result = Vec::new();
        let mut rows = range.rows();

        // 跳过表头
        if rows.next().is_none() {
            return Ok(result);
        }

        for row in rows {
            let row: &[calamine::Data] = row;
            let record: Vec<String> = row
                .iter()
                .map(|cell: &calamine::Data| cell.to_string())
                .collect();
            result.push(T::from_record(&record)?);
        }

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, PartialEq)]
    struct TestRow {
        id: i64,
        name: String,
    }

    impl ExcelRow for TestRow {
        fn headers() -> Vec<&'static str> {
            vec!["编号", "名称"]
        }

        fn to_row(&self) -> Vec<String> {
            vec![self.id.to_string(), self.name.clone()]
        }

        fn from_record(record: &[String]) -> AppResult<Self> {
            if record.len() < 2 {
                return Err(AppError::BadRequest("数据列数不足".to_string()));
            }
            let id = record[0].parse().map_err(|e| {
                AppError::BadRequest(format!("编号解析失败: {}", e))
            })?;
            Ok(TestRow {
                id,
                name: record[1].clone(),
            })
        }
    }

    #[test]
    fn test_excel_round_trip() {
        let rows = vec![
            TestRow {
                id: 1,
                name: "测试一".to_string(),
            },
            TestRow {
                id: 2,
                name: "测试二".to_string(),
            },
        ];

        let bytes = ExcelBuilder::<TestRow>::export(&rows).unwrap();
        assert!(!bytes.is_empty());

        let imported = ExcelBuilder::<TestRow>::import(&bytes).unwrap();
        assert_eq!(imported, rows);
    }

    #[test]
    fn test_excel_import_empty() {
        let rows: Vec<TestRow> = Vec::new();
        let bytes = ExcelBuilder::<TestRow>::export(&rows).unwrap();
        let imported = ExcelBuilder::<TestRow>::import(&bytes).unwrap();
        assert!(imported.is_empty());
    }

    /// 闸门的边界:恰好等于上限放行,多一行拒绝。
    #[test]
    fn guard_export_rows_boundary() {
        assert!(guard_export_rows(0).is_ok());
        assert!(guard_export_rows(EXPORT_ROW_LIMIT).is_ok());

        let err = guard_export_rows(EXPORT_ROW_LIMIT + 1).unwrap_err();
        assert!(
            matches!(err, AppError::BadRequest(_)),
            "超限应是 BadRequest(400) 而不是 500,前端才能把文案透出来: {err:?}"
        );
        // 文案要带上真实行数与上限,用户才知道该缩到多小。
        let msg = err.to_string();
        assert!(msg.contains(&(EXPORT_ROW_LIMIT + 1).to_string()), "{msg}");
        assert!(msg.contains(&EXPORT_ROW_LIMIT.to_string()), "{msg}");
    }

    /// 兜底闸挂在 `export` 上:任何导出端点都绕不开,且在建工作簿**之前**
    /// 就返回 —— 不会先把 5 万多行写成 xlsx 再抛错。
    #[test]
    fn export_rejects_over_limit() {
        let rows: Vec<TestRow> = (0..=EXPORT_ROW_LIMIT as i64)
            .map(|id| TestRow {
                id,
                name: String::new(),
            })
            .collect();
        assert_eq!(rows.len(), EXPORT_ROW_LIMIT + 1);

        let err = ExcelBuilder::<TestRow>::export(&rows).unwrap_err();
        assert!(matches!(err, AppError::BadRequest(_)), "{err:?}");
    }
}
