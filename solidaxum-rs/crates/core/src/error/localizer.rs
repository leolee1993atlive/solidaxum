use std::{ops::Deref, sync::LazyLock};

// sqlx 错误类型经受限 re-export 的三别名供给(纯数据枚举,不能执行;
// 整模块转出被 trybuild 守卫禁止)。DbErr / RuntimeErr 本就在
// error::* 受限面里(oss_config_repo 先例)。
use app_data::sea_orm::{
    DbErr, MySqlDatabaseError, PgDatabaseError, RuntimeErr, SqlxError,
};
use regex::Regex;

pub fn translate_db_error(err: &DbErr) -> String {
    let raw = err.to_string();
    let clean_raw = raw.strip_prefix("Custom Error: ").unwrap_or(&raw);
    if let Some(translated) = try_translate_by_code(err) {
        return translated;
    }
    translate_raw(clean_raw)
}

enum DbCode {
    MySql { number: u16, msg: String },
    Pg { state: String, msg: String },
}

fn extract_db_code(err: &DbErr) -> Option<DbCode> {
    let sqlx_err: &SqlxError = match err {
        DbErr::Exec(RuntimeErr::SqlxError(e))
        | DbErr::Query(RuntimeErr::SqlxError(e))
        | DbErr::Conn(RuntimeErr::SqlxError(e)) => e.deref(),
        _ => return None,
    };
    let SqlxError::Database(db_err) = sqlx_err else {
        return None;
    };

    if let Some(my) = db_err.try_downcast_ref::<MySqlDatabaseError>() {
        return Some(DbCode::MySql {
            number: my.number(),
            msg: my.message().to_string(),
        });
    }
    if let Some(pg) = db_err.try_downcast_ref::<PgDatabaseError>() {
        return Some(DbCode::Pg {
            state: pg.code().to_string(),
            msg: pg.message().to_string(),
        });
    }
    None
}

fn try_translate_by_code(err: &DbErr) -> Option<String> {
    match extract_db_code(err)? {
        DbCode::MySql { number, msg } => {
            let title = match number {
                1146 => "表不存在",
                1054 => "字段不存在",
                1064 => "语法错误",
                1045 => "访问被拒绝",
                1049 => "数据库不存在",
                1062 => "数据重复",
                1452 => "外键约束失败",
                _ => return None,
            };
            Some(format!("{}：{}", title, msg))
        }
        DbCode::Pg { state, msg } => {
            let title = match state.as_str() {
                "42P01" => "表不存在",
                "42703" => "字段不存在",
                "42601" => "语法错误",
                "28P01" => "访问被拒绝",
                "23505" => "数据重复",
                "23503" => "外键约束失败",
                _ => return None,
            };
            Some(format!("{}：{}", title, msg))
        }
    }
}

fn join_title_body(title: &str, body: Option<&str>) -> String {
    match body.map(str::trim).filter(|s| !s.is_empty()) {
        Some(b) => format!("{} - {}", title, b),
        None => title.to_string(),
    }
}

pub fn translate_raw(raw: &str) -> String {
    if let Some(caps) = CANNOT_COMPARE_RE.captures(raw) {
        let title = format!(
            "绑定错误：无法比较类型 {} 和类型 {} 的值",
            &caps[1], &caps[2]
        );
        return join_title_body(&title, caps.get(3).map(|m| m.as_str()));
    }
    if let Some(caps) = REF_COLUMN_RE.captures(raw) {
        let title =
            format!("绑定错误：引用的字段 '{}' 在 FROM 子句中未找到", &caps[1]);
        return join_title_body(&title, caps.get(2).map(|m| m.as_str()));
    }
    if let Some(caps) = BINDER_ERROR_RE.captures(raw) {
        return format!("绑定错误：{}", &caps[1]);
    }
    if let Some(caps) = CATALOG_TABLE_RE.captures(raw) {
        let title = format!("目录错误：表 '{}' 不存在", &caps[1]);
        return join_title_body(&title, caps.get(2).map(|m| m.as_str()));
    }
    if let Some(caps) = CATALOG_COLUMN_RE.captures(raw) {
        let title = format!("目录错误：字段 '{}' 不存在", &caps[1]);
        return join_title_body(&title, caps.get(2).map(|m| m.as_str()));
    }
    if let Some(caps) = CATALOG_ERROR_RE.captures(raw) {
        return format!("目录错误：{}", &caps[1]);
    }
    if let Some(caps) = SYNTAX_ERROR_RE.captures(raw) {
        return format!("语法错误：{}", &caps[1]);
    }
    if let Some(caps) = INVALID_INPUT_RE.captures(raw) {
        return format!("输入错误：{}", &caps[1]);
    }
    if let Some(caps) = CONVERSION_ERROR_RE.captures(raw) {
        return format!("类型转换错误：{}", &caps[1]);
    }
    if let Some(caps) = TABLE_NOT_FOUND_RE.captures(raw) {
        let title = format!("表 '{}' 不存在", &caps[1]);
        return join_title_body(&title, caps.get(2).map(|m| m.as_str()));
    }
    if let Some(caps) = COLUMN_NOT_FOUND_RE.captures(raw) {
        let title = format!("字段 '{}' 不存在", &caps[1]);
        return join_title_body(&title, caps.get(2).map(|m| m.as_str()));
    }
    raw.to_string()
}

static BINDER_ERROR_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?s)^Binder Error:\s*(.+)$").expect("常量正则模式必合法")
});
static CANNOT_COMPARE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?s)^Binder Error:\s*Cannot compare values of type (\w+) and type (\w+)(?:\s*-\s*(.*))?$",
    )
    .expect("常量正则模式必合法")
});
static CATALOG_TABLE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"(?s)^Catalog Error:\s*Table with name ['"]([^'"]+)['"] does not exist!(?:\s*-\s*(.*))?$"#,
    )
    .expect("常量正则模式必合法")
});
static CATALOG_COLUMN_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"(?s)^Catalog Error:\s*Column with name ['"]([^'"]+)['"] does not exist!(?:\s*-\s*(.*))?$"#,
    )
    .expect("常量正则模式必合法")
});
static REF_COLUMN_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"(?s)^Binder Error:\s*Referenced column ['"]([^'"]+)['"] not found in FROM clause!?(?:\s*-\s*(.*))?$"#,
    )
    .expect("常量正则模式必合法")
});
static CATALOG_ERROR_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?s)^Catalog Error:\s*(.+)$").expect("常量正则模式必合法")
});
static SYNTAX_ERROR_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?s)^Syntax Error:\s*(.+)$").expect("常量正则模式必合法")
});
static INVALID_INPUT_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?s)^Invalid Input Error:\s*(.+)$")
        .expect("常量正则模式必合法")
});
static CONVERSION_ERROR_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?s)^Conversion Error:\s*(.+)$").expect("常量正则模式必合法")
});
static TABLE_NOT_FOUND_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"(?s)^[Tt]able with name ['"]([^'"]+)['"] does not exist!?(?:\s*-\s*(.*))?$"#,
    )
    .expect("常量正则模式必合法")
});
static COLUMN_NOT_FOUND_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"(?s)^[Cc]olumn with name ['"]([^'"]+)['"] does not exist!?(?:\s*-\s*(.*))?$"#,
    )
    .expect("常量正则模式必合法")
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_translate_cannot_compare_error() {
        let input = "Binder Error: Cannot compare values of type TIMESTAMP and type VARCHAR - an explicit cast is required LINE 21: AND t1.in_date >= t2.dept_date";
        let result = translate_raw(input);
        assert_eq!(
            result,
            "绑定错误：无法比较类型 TIMESTAMP 和类型 VARCHAR 的值 - an explicit cast is required LINE 21: AND t1.in_date >= t2.dept_date"
        );
    }

    #[test]
    fn test_translate_catalog_table() {
        let input = "Catalog Error: Table with name 'users' does not exist!";
        let result = translate_raw(input);
        assert_eq!(result, "目录错误：表 'users' 不存在");
    }

    #[test]
    fn test_translate_syntax_error() {
        let input = "Syntax Error: syntax error at or near \"FROM\"";
        let result = translate_raw(input);
        assert!(result.starts_with("语法错误："));
        assert!(result.contains("FROM"));
    }

    #[test]
    fn test_translate_unknown_error() {
        let input = "Some random error that doesn't match any pattern";
        let result = translate_raw(input);
        assert_eq!(result, input);
    }

    #[test]
    fn test_translate_table_not_found_without_prefix() {
        let input = "Table with name 'orders' does not exist!";
        let result = translate_raw(input);
        assert_eq!(result, "表 'orders' 不存在");
    }

    #[test]
    fn test_translate_generic_binder_error() {
        let input = "Binder Error: some other binder issue";
        let result = translate_raw(input);
        assert_eq!(result, "绑定错误：some other binder issue");
    }

    #[test]
    fn test_translate_ref_column_error() {
        let input =
            "Binder Error: Referenced column 'foo' not found in FROM clause!";
        let result = translate_raw(input);
        assert_eq!(result, "绑定错误：引用的字段 'foo' 在 FROM 子句中未找到");
    }

    #[test]
    fn test_translate_db_error_public_api() {
        let db_err = DbErr::Custom(
            "Syntax Error: syntax error at or near \"FROM\"".to_string(),
        );
        let result = translate_db_error(&db_err);
        assert_eq!(result, "语法错误：syntax error at or near \"FROM\"");
    }

    #[test]
    fn test_translate_db_error_unknown() {
        let db_err = DbErr::Custom("Some unknown error".to_string());
        let result = translate_db_error(&db_err);
        assert_eq!(result, "Some unknown error");
    }
}
