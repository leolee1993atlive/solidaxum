//! 达梦 ODBC 探针 —— 剩余待测项一次跑完。
//!
//! ```text
//! cargo run --release -- "Driver={DM8};Server=127.0.0.1:5237;UID=SYSDBA;PWD=<pwd>"
//! cargo run --release --features narrow -- "..."     # 中文乱码时切窄字符重测
//! ```
//!
//! 连接串也可通过环境变量 `DM_ODBC` 传入,避免密码进 shell 历史。
//!
//! 每项独立执行,失败不中断后续,末尾输出汇总。产出物直接对应 spec 第 10 节的待测清单。

use odbc_api::{Connection, ConnectionOptions, Cursor, Environment, IntoParameter};
use std::error::Error;

type R<T> = Result<T, Box<dyn Error>>;

const TABLE: &str = "dm_probe";
const ZH: &str = "达梦数据库中文往返测试";
/// 典型雪花 ID,19 位。i64 精度丢失时读回值会变。
const SNOWFLAKE: i64 = 1957203961908830209;

fn main() {
    let conn_str = match std::env::args().nth(1).or_else(|| std::env::var("DM_ODBC").ok()) {
        Some(s) => s,
        None => {
            eprintln!("用法: dm-probe \"Driver={{DM8}};Server=<ip>:<port>;UID=SYSDBA;PWD=<pwd>\"");
            eprintln!("或设置环境变量 DM_ODBC");
            std::process::exit(2);
        }
    };

    println!(
        "字符模式: {}",
        if cfg!(feature = "narrow") {
            "narrow(窄字符)"
        } else {
            "wide(宽字符,odbc-api 默认)"
        }
    );

    let env = match Environment::new() {
        Ok(e) => e,
        Err(e) => {
            eprintln!("创建 ODBC Environment 失败: {e}");
            std::process::exit(1);
        }
    };
    let conn = match env.connect_with_connection_string(&conn_str, ConnectionOptions::default()) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("连接失败: {e}");
            eprintln!("先确认 unixODBC 已注册达梦驱动,并用 isql 手工连通。");
            std::process::exit(1);
        }
    };
    println!("连接成功\n");

    let mut results: Vec<(&str, Result<String, String>)> = Vec::new();

    macro_rules! probe {
        ($name:expr, $body:expr) => {
            match $body {
                Ok(note) => {
                    println!("[PASS] {:<16} {}", $name, note);
                    results.push(($name, Ok(note)));
                }
                Err(e) => {
                    let msg = e.to_string().replace('\n', " ");
                    println!("[FAIL] {:<16} {}", $name, msg);
                    results.push(($name, Err(msg)));
                }
            }
        };
    }

    probe!("建表", setup(&conn));
    probe!("裸标识符", t_bare_identifier(&conn));
    probe!("反引号标识符", t_backtick_identifier(&conn));
    probe!("占位符 ?", t_placeholder_qmark(&conn));
    probe!("占位符 :1", t_placeholder_colon(&conn));
    probe!("中文往返", t_chinese_roundtrip(&conn));
    probe!("雪花 ID 精度", t_snowflake_precision(&conn));
    probe!("datetime 字面量", t_datetime_literal(&conn));
    probe!("char(1)", t_char_flag(&conn));
    probe!("批量插入", t_batch_insert(&conn));
    probe!("LIMIT/OFFSET", t_limit_offset(&conn));
    probe!("upsert", t_upsert(&conn));
    probe!("事务回滚", t_transaction_rollback(&conn));

    let passed = results.iter().filter(|(_, r)| r.is_ok()).count();
    println!("\n==== 汇总: {passed}/{} 通过 ====", results.len());
    for (name, r) in results.iter() {
        if let Err(e) = r {
            println!("  FAIL  {name}: {e}");
        }
    }
    if passed < results.len() {
        println!("\n中文往返失败时,先加 --features narrow 重跑再下结论。");
    }
}

/// 取首行首列文本。无结果集或无行时返回 None。
fn first_text(conn: &Connection, sql: &str) -> R<Option<String>> {
    let Some(mut cursor) = conn.execute(sql, (), None)? else {
        return Ok(None);
    };
    let Some(mut row) = cursor.next_row()? else {
        return Ok(None);
    };
    let mut buf = Vec::new();
    row.get_text(1, &mut buf)?;
    Ok(Some(String::from_utf8_lossy(&buf).into_owned()))
}

/// 统计行数。
fn count(conn: &Connection, where_clause: &str) -> R<i64> {
    let sql = format!("SELECT COUNT(*) FROM {TABLE} WHERE {where_clause}");
    let text = first_text(conn, &sql)?.ok_or("COUNT 未返回结果")?;
    Ok(text.trim().parse::<i64>()?)
}

fn setup(conn: &Connection) -> R<String> {
    // 表不存在时 DROP 会报错,忽略。
    let _ = conn.execute(&format!("DROP TABLE {TABLE}"), (), None);
    conn.execute(
        &format!(
            "CREATE TABLE {TABLE} (\
               id BIGINT PRIMARY KEY, \
               txt VARCHAR(200), \
               flag CHAR(1), \
               ts DATETIME)"
        ),
        (),
        None,
    )?;
    Ok("dm_probe 已建".into())
}

fn t_bare_identifier(conn: &Connection) -> R<String> {
    conn.execute(&format!("SELECT id FROM {TABLE}"), (), None)?;
    Ok("裸标识符可命中(CASE_SENSITIVE=N 生效)".into())
}

fn t_backtick_identifier(conn: &Connection) -> R<String> {
    conn.execute(&format!("SELECT id FROM `{TABLE}`"), (), None)?;
    Ok("反引号可用(SeaORM 生成的 SQL 依赖此项)".into())
}

/// `?` 是 SeaORM/sqlx 的 MySQL 风格占位符。此项决定 DmQueryBuilder 的参数生成方式。
fn t_placeholder_qmark(conn: &Connection) -> R<String> {
    conn.execute(
        &format!("INSERT INTO {TABLE} (id, txt) VALUES (?, ?)"),
        (&1i64, &"qmark".into_parameter()),
        None,
    )?;
    let n = count(conn, "id = 1")?;
    if n == 1 {
        Ok("? 可用".into())
    } else {
        Err(format!("插入后 COUNT={n},期望 1").into())
    }
}

/// `:1` 是 Oracle 风格占位符。若只有此项可用,DmQueryBuilder 需改写参数形式。
fn t_placeholder_colon(conn: &Connection) -> R<String> {
    conn.execute(
        &format!("INSERT INTO {TABLE} (id, txt) VALUES (:1, :2)"),
        (&2i64, &"colon".into_parameter()),
        None,
    )?;
    let n = count(conn, "id = 2")?;
    if n == 1 {
        Ok(":1 可用".into())
    } else {
        Err(format!("插入后 COUNT={n},期望 1").into())
    }
}

/// 宽/窄字符的判定项。不一致即为字符集问题,加 --features narrow 重测。
fn t_chinese_roundtrip(conn: &Connection) -> R<String> {
    conn.execute(
        &format!("INSERT INTO {TABLE} (id, txt) VALUES (?, ?)"),
        (&10i64, &ZH.into_parameter()),
        None,
    )?;
    let got = first_text(conn, &format!("SELECT txt FROM {TABLE} WHERE id = 10"))?
        .ok_or("读回无行")?;
    let got = got.trim_end();
    if got == ZH {
        Ok(format!("往返一致: {got}"))
    } else {
        Err(format!(
            "不一致 —— 期望 {ZH:?} 实际 {got:?}(实际字节 {:x?})",
            got.as_bytes()
        )
        .into())
    }
}

/// i64 全精度。JSON 序列化之外,ODBC 绑定层同样可能丢精度。
fn t_snowflake_precision(conn: &Connection) -> R<String> {
    conn.execute(
        &format!("INSERT INTO {TABLE} (id, txt) VALUES (?, ?)"),
        (&SNOWFLAKE, &"snowflake".into_parameter()),
        None,
    )?;
    let got = first_text(conn, &format!("SELECT id FROM {TABLE} WHERE txt = 'snowflake'"))?
        .ok_or("读回无行")?;
    let got = got.trim();
    if got == SNOWFLAKE.to_string() {
        Ok(format!("{got} 无损"))
    } else {
        Err(format!("精度丢失 —— 写入 {SNOWFLAKE} 读回 {got}").into())
    }
}

/// 达梦能否吃 MySQL 风格的 datetime 字面量。
fn t_datetime_literal(conn: &Connection) -> R<String> {
    conn.execute(
        &format!("INSERT INTO {TABLE} (id, ts) VALUES (?, '2026-08-10 12:34:56')"),
        &20i64,
        None,
    )?;
    let got = first_text(conn, &format!("SELECT ts FROM {TABLE} WHERE id = 20"))?
        .ok_or("读回无行")?;
    Ok(format!("接受 'YYYY-MM-DD HH:MM:SS',读回 {}", got.trim()))
}

/// 基座大量使用 char(1) 存 del_flag / status。
fn t_char_flag(conn: &Connection) -> R<String> {
    conn.execute(
        &format!("INSERT INTO {TABLE} (id, flag) VALUES (?, ?)"),
        (&30i64, &"0".into_parameter()),
        None,
    )?;
    let n = count(conn, "flag = '0'")?;
    if n >= 1 {
        Ok("char(1) 比较正常".into())
    } else {
        Err("flag = '0' 未命中,可能存在尾部补空格问题".into())
    }
}

/// SeaORM 的 insert_many 生成此形式。
fn t_batch_insert(conn: &Connection) -> R<String> {
    conn.execute(
        &format!("INSERT INTO {TABLE} (id, txt) VALUES (?, ?), (?, ?), (?, ?)"),
        (
            &41i64,
            &"a".into_parameter(),
            &42i64,
            &"b".into_parameter(),
            &43i64,
            &"c".into_parameter(),
        ),
        None,
    )?;
    let n = count(conn, "id IN (41, 42, 43)")?;
    if n == 3 {
        Ok("多值 VALUES 可用".into())
    } else {
        Err(format!("期望 3 行,实际 {n}").into())
    }
}

/// SeaORM 分页生成 LIMIT ? OFFSET ?。先测字面量,再测参数化。
fn t_limit_offset(conn: &Connection) -> R<String> {
    conn.execute(&format!("SELECT id FROM {TABLE} ORDER BY id LIMIT 2 OFFSET 1"), (), None)?;
    let parameterized = conn
        .execute(
            &format!("SELECT id FROM {TABLE} ORDER BY id LIMIT ? OFFSET ?"),
            (&2i64, &1i64),
            None,
        )
        .is_ok();
    Ok(format!(
        "字面量可用;参数化 LIMIT ? OFFSET ? {}",
        if parameterized { "可用" } else { "不可用(需改写为字面量)" }
    ))
}

/// SeaORM 的 on_conflict 在 MySQL backend 上生成此形式。
fn t_upsert(conn: &Connection) -> R<String> {
    conn.execute(
        &format!(
            "INSERT INTO {TABLE} (id, txt) VALUES (?, ?) \
             ON DUPLICATE KEY UPDATE txt = ?"
        ),
        (&1i64, &"dup".into_parameter(), &"updated".into_parameter()),
        None,
    )?;
    let got = first_text(conn, &format!("SELECT txt FROM {TABLE} WHERE id = 1"))?
        .ok_or("读回无行")?;
    if got.trim() == "updated" {
        Ok("ON DUPLICATE KEY UPDATE 可用".into())
    } else {
        Err(format!("未触发更新,txt = {:?}", got.trim()).into())
    }
}

/// 手动提交模式 + 回滚。事务桥接依赖此项。
fn t_transaction_rollback(conn: &Connection) -> R<String> {
    conn.set_autocommit(false)?;
    let result = (|| -> R<()> {
        conn.execute(
            &format!("INSERT INTO {TABLE} (id, txt) VALUES (?, ?)"),
            (&99i64, &"rollback-me".into_parameter()),
            None,
        )?;
        conn.rollback()?;
        Ok(())
    })();
    // 无论成败都恢复自动提交,避免污染后续。
    let restore = conn.set_autocommit(true);
    result?;
    restore?;

    let n = count(conn, "id = 99")?;
    if n == 0 {
        Ok("rollback 生效".into())
    } else {
        Err("rollback 后数据仍在,手动提交模式可能未生效".into())
    }
}
