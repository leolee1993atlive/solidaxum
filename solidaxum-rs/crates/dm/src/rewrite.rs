//! SQL 改写。当前仅一项:upsert。
//!
//! 实机实测达梦接受 MySQL 方言的反引号标识符、`LIMIT ? OFFSET ?`、`?` 占位符、
//! 多值 `VALUES`,唯独不认 `ON DUPLICATE KEY UPDATE`。
//!
//! 改写范围刻意只覆盖基座实际用到的形态(`OnConflict::column(<主键>)`)。
//! 遇到未覆盖形态一律报错,不静默放行 ——
//! 静默生成语义错误的 SQL 比直接失败危险得多。

use std::borrow::Cow;

use crate::DmError;

const MARKER: &str = "ON DUPLICATE KEY UPDATE";

/// ASCII 大小写不敏感的子串查找,返回**原串**的字节偏移。
///
/// 直接在原串上按字节比对,不分配副本 —— 这既避开了本模块最容易出错的地方
/// (用大写副本的偏移去切原串,遇上不保长的 Unicode 大写映射就会错位),
/// 也免掉了「为找一个关键字而复制整条 SQL」的开销:全项目只有 2 条语句含 upsert,
/// 其余每条查询都要白白付这份复制成本。
///
/// `needle` 只含 ASCII。UTF-8 的续字节恒 ≥ 0x80,不可能与 ASCII 字节相等,
/// 因此匹配位置必然落在字符边界上,可以安全地用来切片。
fn find_ascii_ci(
    haystack: &str,
    needle: &str,
    from_end: bool,
) -> Option<usize> {
    let (h, n) = (haystack.as_bytes(), needle.as_bytes());
    if n.is_empty() || h.len() < n.len() {
        return None;
    }
    let mut windows = h.windows(n.len());
    if from_end {
        windows.rposition(|w| w.eq_ignore_ascii_case(n))
    } else {
        windows.position(|w| w.eq_ignore_ascii_case(n))
    }
}

pub fn rewrite(sql: &str) -> Result<Cow<'_, str>, DmError> {
    let Some(pos) = find_ascii_ci(sql, MARKER, false) else {
        return Ok(Cow::Borrowed(sql));
    };
    Ok(Cow::Owned(build_merge(sql, pos)?))
}

fn build_merge(sql: &str, marker_pos: usize) -> Result<String, DmError> {
    let insert_part = sql[..marker_pos].trim_end();
    let update_part = sql[marker_pos + MARKER.len()..].trim();

    // 解析 INSERT INTO <表> (<列>) VALUES (<占位符>)
    let values_pos = find_ascii_ci(insert_part, " VALUES", true)
        .expect("INSERT 语句必然含 VALUES");
    let head = &insert_part[..values_pos];
    let open = head.find('(').expect("INSERT 必然含列清单");
    let table = head["INSERT INTO ".len()..open].trim();
    let columns: Vec<&str> = head[open + 1..head.rfind(')').unwrap()]
        .split(',')
        .map(str::trim)
        .collect();

    // VALUES 子句必须是单行、且整行都是 `?` 占位符。下面重建 USING 子句时按
    // 「每列一个 ?」生成,内联字面量会被静默丢弃、参数个数随之对不上;多行 VALUES
    // 更是超出单行 MERGE 的表达能力。SeaORM 的 MysqlQueryBuilder 总是产出占位符,
    // 所以这是防御性守卫 —— 与本模块「未覆盖形态一律报错」的立场一致。
    let values_clause = insert_part[values_pos + " VALUES".len()..].trim();
    let placeholders = values_clause
        .strip_prefix('(')
        .and_then(|s| s.strip_suffix(')'))
        .filter(|inner| inner.split(',').all(|v| v.trim() == "?"));
    let Some(placeholders) = placeholders else {
        return Err(DmError::UnsupportedUpsert(format!(
            "VALUES 子句必须是单行且全部为 `?` 占位符,实际为 {values_clause}。SQL: {sql}"
        )));
    };
    let placeholder_count = placeholders.split(',').count();
    if placeholder_count != columns.len() {
        return Err(DmError::UnsupportedUpsert(format!(
            "VALUES 的占位符个数 {placeholder_count} 与列数 {} 不符。SQL: {sql}",
            columns.len()
        )));
    }

    // MERGE 的 USING 子句:把占位符列成一行虚拟表
    let using_cols: Vec<String> =
        columns.iter().map(|c| format!("? AS {c}")).collect();
    // 冲突键取第一列 —— 基座的两处 on_conflict 都是 OnConflict::column(主键),
    // 而主键在 INSERT 列清单中位列第一。
    let key = columns[0];
    let update_cols: Vec<String> = columns[1..]
        .iter()
        .map(|c| format!("t.{c} = s.{c}"))
        .collect();
    let insert_cols = columns.join(", ");
    let insert_vals: Vec<String> =
        columns.iter().map(|c| format!("s.{c}")).collect();

    // ---- 校验:守住"不静默放行"的承诺 ----
    //
    // 上面的 update_cols 是按"更新全部非键列"重建的。基座现有的几处 upsert 都成立,
    // 但必须校验而非假设 —— 若将来有人加一个只更新子集的 OnConflict,
    // 静默多更新列不会报错(占位符数量恰好对得上),会成为极难排查的数据错误。
    let norm = |c: &str| c.trim().trim_matches('`').to_ascii_lowercase();

    let declared: std::collections::BTreeSet<String> = update_part
        .split(',')
        .filter_map(|assign| assign.split('=').next())
        .map(norm)
        .filter(|c| !c.is_empty())
        .collect();
    let expected: std::collections::BTreeSet<String> =
        columns[1..].iter().map(|c| norm(c)).collect();

    if declared != expected {
        return Err(DmError::UnsupportedUpsert(format!(
            "仅支持「更新全部非键列」的 upsert。实际更新列 {declared:?},期望 {expected:?}。SQL: {sql}"
        )));
    }

    // 冲突键取 INSERT 列清单首位,依赖"主键排在第一列"。同样校验而非假设。
    if !norm(key).eq("id") {
        return Err(DmError::UnsupportedUpsert(format!(
            "冲突键推断为 {key},不是预期的主键列 id。SQL: {sql}"
        )));
    }

    // 若达梦拒绝不带 FROM 的 SELECT(Oracle 语法要求 `FROM DUAL`),
    // 把 USING 子句改为 `(SELECT ... FROM DUAL)`。Step 5 的集成测试会暴露这一点。
    Ok(format!(
        "MERGE INTO {table} t USING (SELECT {}) s ON (t.{key} = s.{key}) \
         WHEN MATCHED THEN UPDATE SET {} \
         WHEN NOT MATCHED THEN INSERT ({insert_cols}) VALUES ({})",
        using_cols.join(", "),
        update_cols.join(", "),
        insert_vals.join(", "),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn passes_through_sql_without_upsert() {
        let sql = "SELECT `id` FROM `sys_user` WHERE `del_flag` = '0' LIMIT ? OFFSET ?";
        assert_eq!(rewrite(sql).unwrap(), sql, "无 upsert 的 SQL 必须原样通过");
    }

    #[test]
    fn rewrites_single_column_upsert_to_merge() {
        let sql = "INSERT INTO `flow_definition` (`id`, `flow_name`) VALUES (?, ?) \
                   ON DUPLICATE KEY UPDATE `flow_name` = VALUES(`flow_name`)";
        let out = rewrite(sql).unwrap();
        assert!(out.contains("MERGE INTO"), "应改写为 MERGE INTO: {out}");
        assert!(
            !out.contains("ON DUPLICATE KEY"),
            "不应残留 MySQL 语法: {out}"
        );
    }

    #[test]
    fn is_case_insensitive() {
        let sql = "INSERT INTO `t` (`id`, `v`) VALUES (?, ?) \
                   on duplicate key update `v` = VALUES(`v`)";
        assert!(rewrite(sql).unwrap().contains("MERGE INTO"));
    }

    /// marker 之前出现非 ASCII 标识符时,偏移量不得错位。
    ///
    /// `to_uppercase()` 是 Unicode 感知的、**不保长**:`ı`(U+0131,2 字节)的大写
    /// 是 `I`(1 字节)。若用大写副本的偏移量去切原串,marker 之前每出现一个这样的
    /// 字符就错位一字节 —— 轻则切出语义错误的 SQL,重则落在非字符边界上 panic。
    #[test]
    fn does_not_shift_offsets_on_non_ascii_identifiers() {
        let sql = "INSERT INTO `tı` (`id`, `vı`) VALUES (?, ?) \
                   ON DUPLICATE KEY UPDATE `vı` = VALUES(`vı`)";
        let out = rewrite(sql).expect("非 ASCII 标识符不应导致改写失败");
        assert!(out.contains("MERGE INTO `tı`"), "表名应原样保留: {out}");
        assert!(out.contains("t.`vı` = s.`vı`"), "列名应原样保留: {out}");
        assert!(!out.contains("ON DUPLICATE"), "不应残留 MySQL 语法: {out}");
    }

    /// VALUES 里出现非占位符(内联字面量)必须报错。
    ///
    /// 改写把 USING 子句重建为「每列一个 `?`」,内联字面量会被静默丢弃、
    /// 参数个数随之对不上。SeaORM 的 MysqlQueryBuilder 总是产出占位符,
    /// 所以这是一道防御性守卫 —— 与本模块「未覆盖形态一律报错」的立场一致。
    #[test]
    fn rejects_inline_literals_in_values() {
        let sql = "INSERT INTO `t` (`id`, `v`) VALUES (?, '达梦') \
                   ON DUPLICATE KEY UPDATE `v` = VALUES(`v`)";
        let err = rewrite(sql).expect_err("非占位符 VALUES 必须报错");
        assert!(
            err.to_string().contains("占位符"),
            "错误信息应指明限制: {err}"
        );
    }

    /// 多行 VALUES 同样超出单行 MERGE 的表达能力,必须报错而非改写出错误语义。
    #[test]
    fn rejects_multi_row_values() {
        let sql = "INSERT INTO `t` (`id`, `v`) VALUES (?, ?), (?, ?) \
                   ON DUPLICATE KEY UPDATE `v` = VALUES(`v`)";
        let err = rewrite(sql).expect_err("多行 VALUES 必须报错");
        assert!(
            err.to_string().contains("占位符"),
            "错误信息应指明限制: {err}"
        );
    }

    /// 只更新部分列 —— 计划刻意未覆盖的形态。必须报错,不得静默按"全非键列"改写,
    /// 否则会多更新列且因占位符数量恰好对得上而不报错。
    #[test]
    fn rejects_partial_column_upsert() {
        let sql = "INSERT INTO `t` (`id`, `a`, `b`) VALUES (?, ?, ?) \
                   ON DUPLICATE KEY UPDATE `a` = VALUES(`a`)";
        let err = rewrite(sql).expect_err("未覆盖形态必须报错");
        assert!(
            err.to_string().contains("全部非键列"),
            "错误信息应指明限制: {err}"
        );
    }
}
