//! 全仓护栏:分页总数的**计数口径**不得在交付途中被丢掉。
//!
//! # 这条护栏在防什么
//!
//! `app_data::Page` 的 total 默认走**上限计数**(1 万):不到上限时是精确值,
//! 超出时 total 等于上限并置 `total_capped`,`total_approx` 再补一个量级。
//! 仓储/服务层普遍要把 Model 映射成 Vo,历史写法是
//!
//! ```ignore
//! let rows: Vec<Vo> = page.rows.iter().map(Vo::from).collect();
//! Ok(TableDataInfo::new(rows, page.total))   // ← 两个新字段在这里蒸发
//! ```
//!
//! 这样交付出去的 JSON 是 `total=10000` 配 `totalCapped=false` —— **自相矛盾,
//! 却不报错、不告警**,前端只能把它当精确总数显示。20 万行 sys_user 上实测
//! 到的正是这个形状(`/system/user/list` 返回 total=10000/totalCapped=false),
//! 本护栏就是那次实测的产物。
//!
//! 正确写法二选一:
//!   - `TableDataInfo::from_page_rows(&page, rows)`(行已借用式映射好)
//!   - `PageResult::from(page)` / `page.map_rows(..)`(能消费 Model 时)
//!
//! # 判据
//!
//! 扫全仓 `.rs`,揪出 `TableDataInfo::new(` / `PageResult::new(` 的实参里
//! 出现 `.total` 的写法 —— 那必然是从某个 `Page`/`TableDataInfo` 里
//! **只抠了 total**,把旁边两个字段留在了原地。

use std::{
    fs,
    path::{Path, PathBuf},
};

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for e in entries.flatten() {
        let p = e.path();
        if p.is_dir() {
            // vendor 是 sea-orm 的 fork,不受本仓约定管辖
            if p.file_name()
                .is_some_and(|n| n == "vendor" || n == "target")
            {
                continue;
            }
            walk(&p, out);
        } else if p.extension().is_some_and(|x| x == "rs") {
            out.push(p);
        }
    }
}

#[test]
fn no_call_site_builds_a_page_vo_from_total_alone() {
    // CARGO_MANIFEST_DIR = crates/core;往上两级到仓库根
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("仓库根")
        .join("crates");

    let mut files = Vec::new();
    walk(&root, &mut files);
    assert!(!files.is_empty(), "扫描目录为空,护栏本身失效了");

    let mut offenders = Vec::new();
    for f in &files {
        let Ok(src) = fs::read_to_string(f) else {
            continue;
        };
        for (i, line) in src.lines().enumerate() {
            // 注释里引用这个反例是**故意的**(几处文档说明就是在讲它为什么错),
            // 只判真代码。
            if line.trim_start().starts_with("//") {
                continue;
            }
            let is_ctor = line.contains("TableDataInfo::new(")
                || line.contains("PageResult::new(");
            // `.total)` / `.total as` —— 从别处抠出的 total 当第二个实参
            if is_ctor && line.contains(".total") {
                offenders.push(format!(
                    "{}:{}: {}",
                    f.display(),
                    i + 1,
                    line.trim()
                ));
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "这些调用点只搬了 total,把 total_capped / total_approx 丢在了原地。\n\
         改用 TableDataInfo::from_page_rows(&page, rows) 或 PageResult::from(page):\n  {}",
        offenders.join("\n  ")
    );
}
