//! MySQL 建表脚本 → 达梦方言 转换器(文本级,不连库)。
//!
//! ```text
//! cargo run --bin mysql2dm -- ../../sql/solidaxum_init.sql ../../sql/dm/solidaxum_init.sql
//! ```
//!
//! 规则(与 sql/dm/Readme.md 的差异清单一一对应):
//! - 去 CREATE DATABASE / USE / SET FOREIGN_KEY_CHECKS(达梦单实例单库,schema 即用户)
//! - 列内联 COMMENT / 表尾 COMMENT → COMMENT ON COLUMN / COMMENT ON TABLE
//! - 表尾 ENGINE=... CHARSET=... COLLATE=... 删除
//! - AUTO_INCREMENT → IDENTITY(1,1);tinyint(n) → TINYINT;*text → TEXT;longblob → BLOB
//! - PRIMARY KEY (...) USING BTREE → 去 USING BTREE
//! - 表内 KEY / UNIQUE KEY → 表后 CREATE [UNIQUE] INDEX
//! - INSERT 字符串内的 MySQL 反斜杠转义(\" \\ \n 等)→ 字面字符(引号双写)
//! - REPLACE INTO → INSERT INTO(DM 全新 schema,无冲突);UPDATE 行原样放行
//! - ALTER TABLE x ADD COLUMN ... COMMENT '...' → 剥内联 COMMENT + COMMENT ON COLUMN
//! - ALTER TABLE x ADD UNIQUE INDEX name (cols) → CREATE UNIQUE INDEX

use std::error::Error;
use std::fs;

fn main() -> Result<(), Box<dyn Error>> {
    let (src, dst) = match (std::env::args().nth(1), std::env::args().nth(2)) {
        (Some(s), Some(d)) => (s, d),
        _ => {
            eprintln!("用法: mysql2dm <mysql.sql> <dm.sql>");
            std::process::exit(2);
        }
    };
    let input = fs::read_to_string(&src)?;
    let (filtered, dropped) = drop_orphaned_lines(&input);
    report_dropped(&dropped, &src);
    let unescaped = unescape_mysql_literals(&filtered);
    let output = gen_header(&src, &dst) + &convert_ddl(&unescaped);
    fs::write(&dst, output)?;
    eprintln!("已写出 {dst}");
    Ok(())
}

/// 生成文件头:声明产物来源与重新生成命令。
fn gen_header(src: &str, dst: &str) -> String {
    let (s, d) = (file_name(src), file_name(dst));
    format!(
        "-- 本文件由 tools/dm-probe 生成,勿手改\n\
         -- 源文件: sql/{s}\n\
         -- 重新生成(在 tools/dm-probe 目录下执行): cargo run --bin mysql2dm -- ../../sql/{s} ../../sql/dm/{d}\n\n"
    )
}

/// 取路径的文件名部分(仅用于生成头展示)。
fn file_name(p: &str) -> String {
    std::path::Path::new(p)
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| p.to_string())
}

/// 第一遍(先于反转义):剔除孤儿行。
/// 规则:CREATE TABLE 块与 INSERT/REPLACE...VALUES 体之外的行,只保留
/// 空行、注释、以及 DROP/CREATE/INSERT/REPLACE/COMMENT/ALTER/UPDATE
/// 开头的语句行(ALTER/UPDATE 为业务文件 sys_dept.dept_code 增量)。
///
/// 返回 (过滤后文本, 被丢弃的行);丢弃是有损的,调用方必须把第二项报给
/// 用户。历史教训:源文件曾在清理运行时数据时删了 INSERT 头却留下 VALUES
/// 体(flow_instance 一行、sys_oper_log 92 行含原始 HTTP multipart 载荷),
/// 那些行在任何数据库上都不是合法 SQL,却被这里静默吃掉 —— dm/pg/sqlite
/// 三个方言因此少了两张表的数据且无任何提示。源文件已修,告警留作复发防线。
fn drop_orphaned_lines(input: &str) -> (String, Vec<(usize, String)>) {
    let mut out: Vec<&str> = Vec::new();
    let mut dropped: Vec<(usize, String)> = Vec::new();
    let mut in_create = false;
    let mut in_insert = false;
    for (idx, line) in input.lines().enumerate() {
        let t = line.trim();
        if in_create {
            out.push(line);
            if t.starts_with(") ENGINE=") || t == ");" {
                in_create = false;
            }
            continue;
        }
        if in_insert {
            out.push(line);
            if t.ends_with(';') {
                in_insert = false;
            }
            continue;
        }
        if t.starts_with("CREATE TABLE") {
            in_create = true;
            out.push(line);
        } else if t.starts_with("INSERT INTO") || t.starts_with("REPLACE INTO") {
            in_insert = true;
            out.push(line);
        } else if t.is_empty()
            || t.starts_with("--")
            || t.starts_with("DROP TABLE")
            || t.starts_with("COMMENT ON")
            || t.starts_with("ALTER TABLE")
            || t.starts_with("UPDATE ")
        {
            out.push(line);
        }
        // 其余行:库级指令由 convert_ddl 处理?—— 不,CREATE DATABASE/USE/SET
        // 以这些关键字开头,在 convert_ddl 里被跳过;这里直接放行即可。
        else if t.starts_with("CREATE DATABASE")
            || t.starts_with("USE ")
            || t.starts_with("SET FOREIGN_KEY_CHECKS")
        {
            out.push(line);
        } else {
            // 其它一律视为孤儿行丢弃 —— 但记下来,由 report_dropped 告警
            dropped.push((idx + 1, t.to_string()));
        }
    }
    (out.join("\n") + "\n", dropped)
}

/// 把 drop_orphaned_lines 丢掉的行报给用户。
/// 只告警不中止:孤儿行也可能是源文件里刻意保留的非 SQL 片段,是否可接受
/// 由人判断;但绝不能像以前那样一声不吭。
fn report_dropped(dropped: &[(usize, String)], src: &str) {
    if dropped.is_empty() {
        return;
    }
    eprintln!(
        "警告: 从 {src} 丢弃了 {} 行无法归类的顶层内容(既不在 CREATE TABLE 块内,\n\
         也不在 INSERT/REPLACE ... VALUES 体内)。这些内容不会出现在产物里 ——\n\
         若其中含数据,说明源文件的 INSERT 语句头缺失,请先修源文件再重新生成。",
        dropped.len()
    );
    const MAX: usize = 10;
    for (ln, text) in dropped.iter().take(MAX) {
        let mut s: String = text.chars().take(80).collect();
        if text.chars().count() > 80 {
            s.push('…');
        }
        eprintln!("  源文件第 {ln} 行: {s}");
    }
    if dropped.len() > MAX {
        eprintln!("  ……另有 {} 行未列出", dropped.len() - MAX);
    }
}

/// 去掉字符串字面量里的 MySQL 反斜杠转义;字面量外原样保留。
fn unescape_mysql_literals(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    let mut in_string = false;
    while let Some(c) = chars.next() {
        if in_string {
            match c {
                '\\' => {
                    let n = chars.next().unwrap_or('\\');
                    match n {
                        '\'' => out.push_str("''"), // 引号双写
                        'n' => out.push('\n'),
                        'r' => out.push('\r'),
                        't' => out.push('\t'),
                        '0' => out.push('\0'),
                        'b' => out.push('\u{8}'),
                        'Z' => out.push('\u{1a}'),
                        other => out.push(other), // \\ \" \% \_ 等 → 字面字符
                    }
                }
                '\'' => {
                    if chars.peek() == Some(&'\'') {
                        out.push('\'');
                        out.push('\'');
                        chars.next();
                    } else {
                        out.push('\'');
                        in_string = false;
                    }
                }
                other => out.push(other),
            }
        } else {
            out.push(c);
            if c == '\'' {
                in_string = true;
            }
        }
    }
    out
}

fn convert_ddl(input: &str) -> String {
    let mut out: Vec<String> = Vec::new();
    let mut table: Option<String> = None; // 当前 CREATE TABLE 的表名
    let mut pending: Vec<String> = Vec::new(); // 表后追加语句(注释/索引)
    let mut last_col_line: Option<usize> = None; // out 中最后一列定义的行号

    for raw in input.lines() {
        let line = raw.trim_end();
        let trimmed = line.trim();

        // 库级指令:达梦不需要
        if trimmed.starts_with("CREATE DATABASE")
            || trimmed.starts_with("USE ")
            || trimmed.starts_with("SET FOREIGN_KEY_CHECKS")
        {
            continue;
        }

        if let Some(t) = table.clone() {
            // CREATE TABLE 块内
            if trimmed.starts_with(") ENGINE=") || trimmed == ");" {
                // 收表:去掉最后一列末尾多余的逗号
                if let Some(i) = last_col_line {
                    if out[i].trim_end().ends_with(',') {
                        let len = out[i].trim_end().len() - 1;
                        out[i].truncate(len);
                        out[i] = out[i].trim_end().to_string();
                    }
                }
                out.push(");".into());
                if let Some(comment) = extract_table_comment(trimmed) {
                    pending.push(format!("COMMENT ON TABLE {t} IS '{comment}';"));
                }
                out.extend(pending.drain(..));
                out.push(String::new());
                table = None;
                last_col_line = None;
                continue;
            }
            // 块内注释行:原样保留,不参与列/逗号处理
            if trimmed.starts_with("--") {
                out.push(line.to_string());
                continue;
            }
            // 索引行 → 表后 CREATE INDEX
            if let Some(idx) = convert_key_line(trimmed, &t) {
                pending.push(idx);
                continue;
            }
            // PRIMARY KEY ... USING BTREE → 去 USING BTREE
            let mut l = line.to_string();
            if l.contains("USING BTREE") {
                l = l.replace(" USING BTREE", "");
            }
            // AUTO_INCREMENT → IDENTITY(1,1)
            if l.contains("AUTO_INCREMENT") {
                l = l.replace("AUTO_INCREMENT", "IDENTITY(1,1)");
            }
            // 列尾 COMMENT '...' → COMMENT ON COLUMN
            if let Some(pos) = find_column_comment(&l) {
                let comment = &l[pos + COLUMN_COMMENT_PREFIX..l.trim_end_matches(',').trim_end().len() - 1];
                let col = ident_token(l.trim_start().trim_start_matches('`')).to_string();
                pending.push(format!("COMMENT ON COLUMN {t}.{col} IS '{comment}';"));
                l = l[..pos].trim_end().to_string();
                // 补回被吃掉的行尾逗号
                if line.trim_end().ends_with(',') {
                    l.push(',');
                }
            }
            // 类型映射
            l = map_types(&l);
            last_col_line = Some(out.len());
            out.push(l);
            continue;
        }

        if trimmed.starts_with("CREATE TABLE") {
            let name = ident_token(
                trimmed
                    .trim_start_matches("CREATE TABLE")
                    .trim()
                    .trim_start_matches('`'),
            )
            .to_string();
            table = Some(name);
            out.push(line.to_string());
            continue;
        }

        // REPLACE INTO → INSERT INTO(DM 全新 schema,无冲突)
        if trimmed.starts_with("REPLACE INTO") {
            out.push(line.replacen("REPLACE INTO", "INSERT INTO", 1));
            continue;
        }
        // ALTER TABLE 增量(业务文件 sys_dept.dept_code)
        if trimmed.starts_with("ALTER TABLE") {
            convert_alter(trimmed, &mut out);
            continue;
        }

        out.push(line.to_string());
    }

    let mut s = out.join("\n");
    if !s.ends_with('\n') {
        s.push('\n');
    }
    s
}

/// 从 `) ENGINE=... COMMENT='...';` 提取表注释。
fn extract_table_comment(line: &str) -> Option<String> {
    let marker = "COMMENT='";
    let start = line.find(marker)? + marker.len();
    let end = line[start..].rfind('\'')? + start;
    Some(line[start..end].to_string())
}

/// 表内 KEY / UNIQUE KEY 行 → CREATE INDEX 语句。
/// 索引名兼容反引号(上游 mysqldump 风格)与裸标识符(业务手风格式)。
fn convert_key_line(line: &str, table: &str) -> Option<String> {
    let (unique, rest) = if let Some(r) = line.strip_prefix("UNIQUE KEY") {
        (true, r)
    } else if let Some(r) = line.strip_prefix("KEY") {
        (false, r)
    } else {
        return None;
    };
    // 形如 `idx_name` (`col1`,`col2`), 或 idx_name (col1),
    let rest = rest.trim().trim_start_matches('`');
    let idx_name = ident_token(rest);
    let cols_start = rest.find('(')? + 1;
    let cols_end = rest.rfind(')')?;
    let cols = &rest[cols_start..cols_end];
    let kw = if unique { "CREATE UNIQUE INDEX" } else { "CREATE INDEX" };
    Some(format!("{kw} {idx_name} ON {table} ({cols});"))
}

/// 取标识符前缀(字母/数字/下划线),兼容反引号剥离后的裸标识符。
fn ident_token(s: &str) -> &str {
    let end = s
        .find(|c: char| !(c.is_alphanumeric() || c == '_'))
        .unwrap_or(s.len());
    &s[..end]
}

/// ALTER TABLE 两种形态(业务文件 sys_dept.dept_code 增量):
/// - ADD COLUMN ... COMMENT '...' → 剥内联 COMMENT,追加 COMMENT ON COLUMN
/// - ADD UNIQUE INDEX name (cols) → CREATE UNIQUE INDEX name ON x (cols)
/// 其余形态原样放行并告警(本仓暂不存在)。
fn convert_alter(line: &str, out: &mut Vec<String>) {
    let body = line.trim().trim_end_matches(';').trim_end();
    let rest = body.trim_start_matches("ALTER TABLE").trim_start();
    let table = ident_token(rest);
    let after = rest[table.len()..].trim_start();
    if let Some(a) = after.strip_prefix("ADD UNIQUE INDEX") {
        let a = a.trim_start();
        let name = ident_token(a);
        if let (Some(cs), Some(ce)) = (a.find('('), a.rfind(')')) {
            out.push(format!(
                "CREATE UNIQUE INDEX {name} ON {table} ({});",
                &a[cs + 1..ce]
            ));
            return;
        }
    }
    if let Some(a) = after.strip_prefix("ADD COLUMN") {
        let a = a.trim_start();
        let col = ident_token(a);
        let stmt = format!("ALTER TABLE {table} ADD COLUMN {a};");
        if let Some(pos) = find_column_comment(&stmt) {
            let comment_end = stmt.rfind('\'').unwrap();
            let comment = &stmt[pos + COLUMN_COMMENT_PREFIX..comment_end];
            out.push(format!("{};", stmt[..pos].trim_end()));
            out.push(format!("COMMENT ON COLUMN {table}.{col} IS '{comment}';"));
        } else {
            out.push(stmt);
        }
        return;
    }
    eprintln!("警告:规格外 ALTER TABLE 形态,原样放行: {line}");
    out.push(line.to_string());
}

/// 找列定义里的 ` COMMENT '` 起始位置(不在字符串内 —— 列注释只出现在行尾)。
fn find_column_comment(line: &str) -> Option<usize> {
    line.find(" COMMENT '")
}

/// " COMMENT '" 的长度(含前导空格与开引号)。
const COLUMN_COMMENT_PREFIX: usize = 10;

fn map_types(line: &str) -> String {
    let mut l = line.to_string();
    // tinyint(n) / tinyint → TINYINT(达梦不支持显示宽度)
    for pat in ["tinyint(1)", "tinyint(4)", "tinyint"] {
        if l.contains(pat) {
            l = l.replacen(pat, "TINYINT", 1);
            break;
        }
    }
    for (from, to) in [
        ("longtext", "TEXT"),
        ("LONGTEXT", "TEXT"),
        ("mediumtext", "TEXT"),
        ("MEDIUMTEXT", "TEXT"),
        ("tinytext", "TEXT"),
        ("TINYTEXT", "TEXT"),
        ("longblob", "BLOB"),
        ("LONGBLOB", "BLOB"),
        ("mediumblob", "BLOB"),
        ("MEDIUMBLOB", "BLOB"),
    ] {
        if l.contains(from) {
            l = l.replacen(from, to, 1);
        }
    }
    l
}
