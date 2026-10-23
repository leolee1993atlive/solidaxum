//! MySQL 建表脚本 → SQLite / PostgreSQL 方言 转换器(文本级,不连库)。
//!
//! ```text
//! cargo run --bin mysql2x -- sqlite   ../../sql/solidaxum_init.sql ../../sql/sqlite/solidaxum_init.sql
//! cargo run --bin mysql2x -- postgres ../../sql/bl_mcdms_init.sql      ../../sql/postgresql/bl_mcdms_init.sql
//! ```
//!
//! 与 mysql2dm 同一思路:drop_orphaned_lines(含 REPLACE INTO / ALTER TABLE /
//! UPDATE 放行)与 unescape_mysql_literals 原样复用,再按目标方言转换:
//! - 通用:去 CREATE DATABASE / USE / SET FOREIGN_KEY_CHECKS;表尾
//!   `) ENGINE=...;` → `);`;表内 KEY/UNIQUE KEY → 表后 CREATE [UNIQUE] INDEX;
//!   去 USING BTREE
//! - sqlite:内联/表 COMMENT 直接丢弃;AUTO_INCREMENT 列 →
//!   `INTEGER PRIMARY KEY AUTOINCREMENT` 并删表级 PRIMARY KEY 行;
//!   REPLACE INTO → INSERT OR REPLACE INTO;NOW() → CURRENT_TIMESTAMP;
//!   CONCAT(a, b) → (a || b);保留反引号
//! - postgres:字符串字面量外去反引号;COMMENT → COMMENT ON COLUMN/TABLE;
//!   tinyint → SMALLINT、datetime → TIMESTAMP、*text → TEXT、*blob → BYTEA;
//!   `bigint NOT NULL AUTO_INCREMENT` → `BIGSERIAL NOT NULL`(表级主键保留);
//!   REPLACE INTO → INSERT INTO;NOW()/CONCAT() 原样

use std::error::Error;
use std::fs;

#[derive(Clone, Copy, PartialEq)]
enum Mode {
    Sqlite,
    Postgres,
}

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = std::env::args().collect();
    let (mode, src, dst) = match (args.get(1), args.get(2), args.get(3)) {
        (Some(m), Some(s), Some(d)) => (m.as_str(), s.as_str(), d.as_str()),
        _ => {
            eprintln!("用法: mysql2x <sqlite|postgres> <mysql.sql> <输出.sql>");
            std::process::exit(2);
        }
    };
    let mode = match mode {
        "sqlite" => Mode::Sqlite,
        "postgres" => Mode::Postgres,
        other => {
            eprintln!("未知目标方言: {other}(仅支持 sqlite|postgres)");
            std::process::exit(2);
        }
    };
    let input = fs::read_to_string(src)?;
    let (filtered, dropped) = drop_orphaned_lines(&input);
    report_dropped(&dropped, src);
    let unescaped = unescape_mysql_literals(&filtered);
    // 字符串外的整体改写:pg 去反引号;sqlite 改写 NOW()/CONCAT();
    // 两种模式都要处理 MySQL 0x 十六进制字面量(规格外形态,sys_notice 种子)
    let pre = match mode {
        Mode::Postgres => convert_hex_literals(&remove_backticks(&unescaped), mode),
        Mode::Sqlite => {
            convert_hex_literals(&convert_concat(&replace_now(&unescaped)), mode)
        }
    };
    let output = gen_header(mode, src, dst) + &convert_ddl(mode, &pre);
    fs::write(dst, output)?;
    eprintln!("已写出 {dst}");
    Ok(())
}

/// 生成文件头:声明产物来源与重新生成命令。
fn gen_header(mode: Mode, src: &str, dst: &str) -> String {
    let (s, d) = (file_name(src), file_name(dst));
    let (m, dir) = match mode {
        Mode::Sqlite => ("sqlite", "sqlite"),
        Mode::Postgres => ("postgres", "postgresql"),
    };
    // PG 把 datetime 映射成 TIMESTAMPTZ(见 map_types_pg),而种子里的时间
    // 字面量不带时区,PG 会按会话 TimeZone 去解释它们。固定成 UTC,保证同
    // 一份脚本在任何服务器上导入都得到相同的时刻。
    let tz = match mode {
        Mode::Postgres => "SET TimeZone = 'UTC';\n\n",
        Mode::Sqlite => "",
    };
    format!(
        "-- 本文件由 tools/dm-probe 生成,勿手改\n\
         -- 源文件: sql/{s}\n\
         -- 重新生成(在 tools/dm-probe 目录下执行): cargo run --bin mysql2x -- {m} ../../sql/{s} ../../sql/{dir}/{d}\n\n{tz}"
    )
}

/// 取路径的文件名部分(仅用于生成头展示)。
fn file_name(p: &str) -> String {
    std::path::Path::new(p)
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| p.to_string())
}

/// 第一遍(先于反转义):剔除孤儿行(同 mysql2dm,含 REPLACE INTO /
/// ALTER TABLE / UPDATE 放行)。
/// 规则:CREATE TABLE 块与 INSERT/REPLACE...VALUES 体之外的行,只保留
/// 空行、注释、以及 DROP/CREATE/INSERT/REPLACE/COMMENT/ALTER/UPDATE
/// 开头的语句行。
///
/// 返回 (过滤后文本, 被丢弃的行);丢弃是有损的,调用方必须把第二项报给
/// 用户。历史教训:MySQL 版曾有两处 INSERT 语句头缺失(flow_instance 与
/// sys_oper_log),孤儿体被这里静默吃掉,pg/sqlite/dm 三个方言因此少了两
/// 张表的数据且无任何提示,直到有人在真实 PG 上数行才发现。
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
        } else if t.starts_with("CREATE DATABASE")
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

/// 去掉字符串字面量里的 MySQL 反斜杠转义;字面量外原样保留(同 mysql2dm)。
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

/// postgres:字符串字面量外去反引号(字符串单引号界定、'' 双写、可跨行)。
fn remove_backticks(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    let mut in_string = false;
    while let Some(c) = chars.next() {
        if in_string {
            out.push(c);
            if c == '\'' {
                if chars.peek() == Some(&'\'') {
                    out.push(chars.next().unwrap());
                } else {
                    in_string = false;
                }
            }
        } else if c == '\'' {
            in_string = true;
            out.push(c);
        } else if c != '`' {
            out.push(c);
        }
    }
    out
}

/// sqlite:字符串外、按词边界把 NOW() 改写为 CURRENT_TIMESTAMP。
fn replace_now(input: &str) -> String {
    let b = input.as_bytes();
    let mut out = String::new();
    let mut copied = 0; // 已复制到 out 的字节位置
    let mut i = 0;
    let mut in_string = false;
    while i < b.len() {
        let c = b[i];
        if in_string {
            if c == b'\'' {
                if b.get(i + 1) == Some(&b'\'') {
                    i += 2;
                    continue;
                }
                in_string = false;
            }
            i += 1;
            continue;
        }
        if c == b'\'' {
            in_string = true;
            i += 1;
            continue;
        }
        if c == b'N'
            && b[i..].starts_with(b"NOW()")
            && (i == 0 || !is_ident_byte(b[i - 1]))
        {
            out.push_str(&input[copied..i]);
            out.push_str("CURRENT_TIMESTAMP");
            i += 5;
            copied = i;
            continue;
        }
        i += 1;
    }
    out.push_str(&input[copied..]);
    out
}

/// sqlite:字符串外把 CONCAT(a, b, ...) 改写为 (a || b || ...)
/// (按顶层逗号拆分参数;本仓仅一处两参调用)。
fn convert_concat(input: &str) -> String {
    let b = input.as_bytes();
    let mut out = String::new();
    let mut copied = 0;
    let mut i = 0;
    let mut in_string = false;
    while i < b.len() {
        let c = b[i];
        if in_string {
            if c == b'\'' {
                if b.get(i + 1) == Some(&b'\'') {
                    i += 2;
                    continue;
                }
                in_string = false;
            }
            i += 1;
            continue;
        }
        if c == b'\'' {
            in_string = true;
            i += 1;
            continue;
        }
        if c == b'C'
            && b[i..].starts_with(b"CONCAT(")
            && (i == 0 || !is_ident_byte(b[i - 1]))
        {
            // 找配对右括号(字符串感知)
            let open = i + 6; // '(' 的位置
            let mut depth = 0;
            let mut j = open;
            let mut in_s = false;
            let close = loop {
                let d = b[j];
                if in_s {
                    if d == b'\'' {
                        if b.get(j + 1) == Some(&b'\'') {
                            j += 2;
                            continue;
                        }
                        in_s = false;
                    }
                } else if d == b'\'' {
                    in_s = true;
                } else if d == b'(' {
                    depth += 1;
                } else if d == b')' {
                    depth -= 1;
                    if depth == 0 {
                        break j;
                    }
                }
                j += 1;
            };
            let args = split_top_level(&input[open + 1..close]);
            out.push_str(&input[copied..i]);
            out.push('(');
            out.push_str(&args.join(" || "));
            out.push(')');
            i = close + 1;
            copied = i;
            continue;
        }
        i += 1;
    }
    out.push_str(&input[copied..]);
    out
}

/// 按顶层逗号拆分参数列表(括号深度 0、字符串外)。
fn split_top_level(s: &str) -> Vec<String> {
    let b = s.as_bytes();
    let mut parts = Vec::new();
    let mut depth = 0;
    let mut in_s = false;
    let mut start = 0;
    let mut i = 0;
    while i < b.len() {
        let c = b[i];
        if in_s {
            if c == b'\'' {
                if b.get(i + 1) == Some(&b'\'') {
                    i += 2;
                    continue;
                }
                in_s = false;
            }
        } else if c == b'\'' {
            in_s = true;
        } else if c == b'(' {
            depth += 1;
        } else if c == b')' {
            depth -= 1;
        } else if c == b',' && depth == 0 {
            parts.push(s[start..i].trim().to_string());
            start = i + 1;
        }
        i += 1;
    }
    parts.push(s[start..].trim().to_string());
    parts
}

/// 字符串外把 MySQL `0x<hex>` 十六进制字面量改写为目标方言 blob 字面量:
/// sqlite → `x'<hex>'`;postgres → `'\x<hex>'`(bytea 十六进制输入格式)。
/// 规格外形态说明:仅 sys_notice 种子两处(公告内容 longblob),dm 侧原生支持
/// 0x 字面量故不处理。
fn convert_hex_literals(input: &str, mode: Mode) -> String {
    let b = input.as_bytes();
    let mut out = String::new();
    let mut copied = 0;
    let mut i = 0;
    let mut in_string = false;
    let mut n = 0;
    while i < b.len() {
        let c = b[i];
        if in_string {
            if c == b'\'' {
                if b.get(i + 1) == Some(&b'\'') {
                    i += 2;
                    continue;
                }
                in_string = false;
            }
            i += 1;
            continue;
        }
        if c == b'\'' {
            in_string = true;
            i += 1;
            continue;
        }
        // 0x 后至少一位十六进制,前字符非标识符(避免误伤标识符内 0x)
        if c == b'0'
            && b.get(i + 1) == Some(&b'x')
            && b.get(i + 2).is_some_and(|d| d.is_ascii_hexdigit())
            && (i == 0 || !is_ident_byte(b[i - 1]))
        {
            let mut end = i + 2;
            while b.get(end).is_some_and(|d| d.is_ascii_hexdigit()) {
                end += 1;
            }
            let hex = &input[i + 2..end];
            out.push_str(&input[copied..i]);
            match mode {
                Mode::Sqlite => out.push_str(&format!("x'{hex}'")),
                Mode::Postgres => out.push_str(&format!("'\\x{hex}'")),
            }
            n += 1;
            i = end;
            copied = i;
            continue;
        }
        i += 1;
    }
    out.push_str(&input[copied..]);
    if n > 0 {
        eprintln!("注意:改写 {n} 处 MySQL 0x 十六进制字面量(blob 种子)");
    }
    out
}

fn is_ident_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

fn convert_ddl(mode: Mode, input: &str) -> String {
    let mut out: Vec<String> = Vec::new();
    let mut table: Option<String> = None; // 当前 CREATE TABLE 的表名
    let mut pending: Vec<String> = Vec::new(); // 表后追加语句(注释/索引)
    let mut last_col_line: Option<usize> = None; // out 中最后一列定义的行号
    let mut auto_col: Option<String> = None; // sqlite: 本表 AUTO_INCREMENT 列名

    for raw in input.lines() {
        let line = raw.trim_end();
        let trimmed = line.trim();

        // 库级指令:sqlite/pg 均不需要
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
                // 表注释:pg → COMMENT ON TABLE;sqlite 无 COMMENT ON,丢弃
                if mode == Mode::Postgres {
                    if let Some(comment) = extract_table_comment(trimmed) {
                        pending.push(format!("COMMENT ON TABLE {t} IS '{comment}';"));
                    }
                }
                out.extend(pending.drain(..));
                out.push(String::new());
                table = None;
                last_col_line = None;
                auto_col = None;
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
            // sqlite:AUTO_INCREMENT 列已内联 INTEGER PRIMARY KEY,
            // 删掉表级 PRIMARY KEY (col) 行
            if mode == Mode::Sqlite {
                if let Some(col) = &auto_col {
                    if trimmed.starts_with("PRIMARY KEY") && trimmed.contains(col.as_str()) {
                        continue;
                    }
                }
            }
            // PRIMARY KEY ... USING BTREE → 去 USING BTREE
            let mut l = line.to_string();
            if l.contains("USING BTREE") {
                l = l.replace(" USING BTREE", "");
            }
            // 列尾 COMMENT '...':pg → COMMENT ON COLUMN;sqlite 直接丢弃
            if let Some(pos) = find_column_comment(&l) {
                if mode == Mode::Postgres {
                    let comment = &l
                        [pos + COLUMN_COMMENT_PREFIX..l.trim_end_matches(',').trim_end().len() - 1];
                    let col =
                        ident_token(l.trim_start().trim_start_matches('`')).to_string();
                    pending.push(format!("COMMENT ON COLUMN {t}.{col} IS '{comment}';"));
                }
                l = l[..pos].trim_end().to_string();
                // 补回被吃掉的行尾逗号
                if line.trim_end().ends_with(',') {
                    l.push(',');
                }
            }
            // AUTO_INCREMENT
            if l.contains("AUTO_INCREMENT") {
                match mode {
                    Mode::Sqlite => {
                        if auto_col.is_some() {
                            panic!("规格外形态:表 {t} 出现第二个 AUTO_INCREMENT 列");
                        }
                        let (newl, col) = sqlite_autoincrement_line(&l);
                        auto_col = Some(col);
                        l = newl;
                    }
                    Mode::Postgres => {
                        if !l.contains("bigint NOT NULL AUTO_INCREMENT") {
                            panic!("规格外形态:表 {t} 的 AUTO_INCREMENT 形态非预期: {l}");
                        }
                        l = l.replace(
                            "bigint NOT NULL AUTO_INCREMENT",
                            "BIGSERIAL NOT NULL",
                        );
                    }
                }
            }
            // 规格外形态说明:dc_case_analysis.current_marker 是 MySQL 生成列,
            // PG 不支持 VIRTUAL 关键字,改写为 STORED(语义等价,落盘存储)
            if mode == Mode::Postgres && l.contains("VIRTUAL") {
                eprintln!("注意:表 {t} 含生成列 VIRTUAL,已改写为 STORED");
                l = l.replace(" VIRTUAL", " STORED");
            }
            // 类型映射
            l = match mode {
                Mode::Sqlite => map_types_sqlite(&l),
                Mode::Postgres => map_types_pg(&l),
            };
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

        // REPLACE INTO:sqlite → INSERT OR REPLACE;pg 全新 schema 无冲突 → INSERT
        if trimmed.starts_with("REPLACE INTO") {
            let kw = match mode {
                Mode::Sqlite => "INSERT OR REPLACE INTO",
                Mode::Postgres => "INSERT INTO",
            };
            out.push(line.replacen("REPLACE INTO", kw, 1));
            continue;
        }
        // ALTER TABLE 增量(业务文件 sys_dept.dept_code)
        if trimmed.starts_with("ALTER TABLE") {
            convert_alter(mode, trimmed, &mut out);
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

/// sqlite:`` `job_log_id` bigint NOT NULL AUTO_INCREMENT, ``
///     → `` `job_log_id` INTEGER PRIMARY KEY AUTOINCREMENT, ``
/// 返回(新行, 列名)。
fn sqlite_autoincrement_line(line: &str) -> (String, String) {
    let indent: String = line.chars().take_while(|c| c.is_whitespace()).collect();
    let rest = line.trim_start();
    let name_end = rest.find(char::is_whitespace).unwrap_or(rest.len());
    let name_token = &rest[..name_end];
    let comma = if rest.trim_end().ends_with(',') { "," } else { "" };
    (
        format!("{indent}{name_token} INTEGER PRIMARY KEY AUTOINCREMENT{comma}"),
        name_token.trim_matches('`').to_string(),
    )
}

/// ALTER TABLE 两种形态(业务文件 sys_dept.dept_code 增量):
/// - ADD COLUMN ... COMMENT '...':剥内联 COMMENT;
///   pg 追加 COMMENT ON COLUMN,sqlite 无 COMMENT ON 直接丢弃
/// - ADD UNIQUE INDEX name (cols) → CREATE UNIQUE INDEX name ON x (cols)
/// 其余形态原样放行并告警(本仓暂不存在)。
fn convert_alter(mode: Mode, line: &str, out: &mut Vec<String>) {
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
            if mode == Mode::Postgres {
                out.push(format!("COMMENT ON COLUMN {table}.{col} IS '{comment}';"));
            }
        } else {
            out.push(stmt);
        }
        return;
    }
    eprintln!("警告:规格外 ALTER TABLE 形态,原样放行: {line}");
    out.push(line.to_string());
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

/// 找列定义里的 ` COMMENT '` 起始位置(不在字符串内 —— 列注释只出现在行尾)。
fn find_column_comment(line: &str) -> Option<usize> {
    line.find(" COMMENT '")
}

/// " COMMENT '" 的长度(含前导空格与开引号)。
const COLUMN_COMMENT_PREFIX: usize = 10;

/// sqlite:类型亲和性宽松,只归并 *text/*blob;其余(含反引号)原样。
fn map_types_sqlite(line: &str) -> String {
    let mut l = line.to_string();
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

/// postgres:tinyint → SMALLINT;datetime → TIMESTAMPTZ;*text → TEXT;*blob → BYTEA。
///
/// datetime 必须落到 TIMESTAMPTZ 而不是 TIMESTAMP:实体基类 BaseEntity 的
/// create_time / update_time 是 Option<DateTime<Utc>>,sqlx 的 PG 驱动只认
/// TIMESTAMPTZ,配 TIMESTAMP 会在解码时报 "mismatched types" —— 因为
/// BaseEntity 被所有实体继承,等于所有业务表在 PG 上都读不了。sqlx 的
/// MySQL 驱动对 DATETIME 宽容,所以这个错只在 PG 上暴露。
/// 无时区字面量按会话 TimeZone 解释,gen_header 已固定 UTC。
fn map_types_pg(line: &str) -> String {
    let mut l = line.to_string();
    for pat in ["tinyint(1)", "tinyint(4)", "tinyint", "TINYINT"] {
        if l.contains(pat) {
            l = l.replacen(pat, "SMALLINT", 1);
            break;
        }
    }
    for (from, to) in [
        ("datetime", "TIMESTAMPTZ"),
        ("DATETIME", "TIMESTAMPTZ"),
        // char(n) → varchar(n)。PG 的 char(n) 是 bpchar,而 sqlx 把字符串
        // 参数按 text 绑定;bpchar = text 会把类型转换加到**列**上
        // (Filter: ((del_flag)::text = ...)),该列上的索引就退化成过滤器,
        // 只能全扫。本仓 53 个 char 列全是 char(1),转 varchar 无尾空格语义
        // 差异。前导空格是防撞:" char(" 不会匹配 " varchar(" 里的 char。
        (" char(", " varchar("),
        ("longtext", "TEXT"),
        ("LONGTEXT", "TEXT"),
        ("mediumtext", "TEXT"),
        ("MEDIUMTEXT", "TEXT"),
        ("tinytext", "TEXT"),
        ("TINYTEXT", "TEXT"),
        ("longblob", "BYTEA"),
        ("LONGBLOB", "BYTEA"),
        ("mediumblob", "BYTEA"),
        ("MEDIUMBLOB", "BYTEA"),
    ] {
        if l.contains(from) {
            l = l.replacen(from, to, 1);
        }
    }
    l
}
