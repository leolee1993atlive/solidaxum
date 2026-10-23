//! 通用 SQL 执行器:连接串 + 一条 SQL,结果以 TSV 打印;
//! 或 `@文件` 模式:单连接按序执行整份脚本,逐条报错,末尾汇总。
//!
//! ```text
//! cargo run --bin dm-sql -- "DSN=DM8;UID=SYSDBA;PWD=xxx" "SELECT 1"
//! DM_ODBC="..." cargo run --bin dm-sql -- "SELECT 1"
//! DM_ODBC="..." cargo run --bin dm-sql -- @script.sql
//! ```

use odbc_api::{Connection, ConnectionOptions, Cursor, Environment, ResultSetMetadata};
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    let mut args = std::env::args().skip(1);
    let (conn_str, sql) = match (args.next(), args.next()) {
        (Some(a), Some(b)) => (a, b),
        (Some(sql), None) => (std::env::var("DM_ODBC")?, sql),
        _ => {
            eprintln!("用法: dm-sql \"<连接串>\" \"<SQL|@文件>\"  或  DM_ODBC=.. dm-sql \"<SQL|@文件>\"");
            std::process::exit(2);
        }
    };

    let env = Environment::new()?;
    let conn = env.connect_with_connection_string(&conn_str, ConnectionOptions::default())?;

    if let Some(path) = sql.strip_prefix('@') {
        return run_file(&conn, path);
    }
    run_one(&conn, &sql)
}

fn run_one(conn: &Connection<'_>, sql: &str) -> Result<(), Box<dyn Error>> {
    match conn.execute(sql, (), None)? {
        Some(mut cursor) => {
            let cols = cursor.num_result_cols()? as u16;
            let mut names = Vec::new();
            for i in 1..=cols {
                names.push(cursor.col_name(i)?);
            }
            println!("{}", names.join("\t"));
            let mut buf: Vec<u16> = Vec::new();
            let mut n = 0usize;
            while let Some(mut row) = cursor.next_row()? {
                let mut fields = Vec::new();
                for i in 1..=cols {
                    buf.clear();
                    if row.get_wide_text(i, &mut buf)? {
                        fields.push(String::from_utf16_lossy(&buf));
                    } else {
                        fields.push("NULL".into());
                    }
                }
                println!("{}", fields.join("\t"));
                n += 1;
            }
            eprintln!("-- {n} 行");
        }
        None => eprintln!("-- 无结果集"),
    }
    Ok(())
}

/// 逐条执行脚本:语句按"行尾分号"切分(脚本不含存储过程/触发器)。
/// 失败的语句记录后继续,末尾汇总并以非零码退出。
fn run_file(conn: &Connection<'_>, path: &str) -> Result<(), Box<dyn Error>> {
    let text = std::fs::read_to_string(path)?;
    let mut stmt = String::new();
    let mut ok = 0usize;
    let mut failed: Vec<(String, String)> = Vec::new();

    for line in text.lines() {
        let t = line.trim();
        if stmt.is_empty() && (t.is_empty() || t.starts_with("--")) {
            continue;
        }
        stmt.push_str(line);
        stmt.push('\n');
        if t.ends_with(';') {
            let sql = stmt.trim().trim_end_matches(';').to_string();
            stmt.clear();
            if sql.is_empty() {
                continue;
            }
            match conn.execute(&sql, (), None) {
                Ok(mut cursor) => {
                    // 有结果集的语句(SELECT/COMMENT 元数据查询)排空
                    if let Some(mut c) = cursor.take() {
                        while c.next_row()?.is_some() {}
                    }
                    ok += 1;
                }
                Err(e) => {
                    let head: String = sql.chars().take(120).collect();
                    failed.push((head, e.to_string()));
                }
            }
        }
    }
    if !stmt.trim().is_empty() {
        failed.push(("(末尾未以分号结束的残留)".into(), stmt.trim().chars().take(120).collect()));
    }

    eprintln!("-- 成功 {ok} 条,失败 {} 条", failed.len());
    for (i, (sql, err)) in failed.iter().enumerate() {
        eprintln!("[失败 {}] {}", i + 1, sql.replace('\n', " "));
        eprintln!("    {err}");
    }
    if failed.is_empty() {
        Ok(())
    } else {
        Err(format!("{} 条语句执行失败", failed.len()).into())
    }
}
