mod common;

use std::{collections::HashSet, sync::Arc};

use app_core::{model::entity::BaseEntity, vo::page::PageQuery};
use app_data::DataSource;
use common::setup_db;
use system::codegen::{
    model::{
        gen_column::GenTableColumn,
        gen_table::{GenTable, GenTableQueryBo},
    },
    repository::gen_table_repo::GenTableRepository,
    service::{
        db_meta::{RawColumnInfo, resolve_html_type, resolve_java_type},
        gen_table_service::{build_gen_table_from_meta, merge_columns},
    },
};

fn sample_table(table_id: i64, table_name: &str, comment: &str) -> GenTable {
    GenTable {
        table_id,
        data_name: "master".to_string(),
        table_name: table_name.to_string(),
        table_comment: comment.to_string(),
        class_name: "SysUser".to_string(),
        tpl_category: "crud".to_string(),
        frontend_type: "svelte".to_string(),
        package_name: "crates.system".to_string(),
        module_name: "system".to_string(),
        business_name: "user".to_string(),
        function_name: "用户".to_string(),
        function_author: "RuoYi".to_string(),
        gen_type: "0".to_string(),
        gen_path: "/".to_string(),
        options: None,
        remark: None,
        pk_column: None,
        columns: None,
        tree_code: None,
        tree_parent_code: None,
        tree_name: None,
        menu_ids: None,
        parent_menu_id: None,
        parent_menu_name: None,
        enable_export: None,
        enable_status: None,
        status_field: None,
        enable_unique: None,
        unique_fields: None,
        enable_sort: None,
        sort_field: None,
        tree_root_value: None,
        tree_ancestors_field: None,
        tree_order_field: None,
        base_entity: BaseEntity {
            create_dept: None,
            create_by: None,
            create_time: None,
            update_by: None,
            update_time: None,
        },
    }
}

// ── DB-level pagination ───────────────────────────────────

fn empty_gen_query() -> GenTableQueryBo {
    GenTableQueryBo {
        table_name: None,
        table_comment: None,
        data_name: None,
        begin_time: None,
        end_time: None,
        page: PageQuery {
            page_num: Some(1),
            page_size: Some(10),
            order_by_column: None,
            is_asc: None,
        },
    }
}

fn gen_repo_system(ds: DataSource) -> (Arc<DataSource>, GenTableRepository) {
    (Arc::new(ds), GenTableRepository::new())
}

#[tokio::test]
async fn test_select_page_db_pagination() {
    let (ds, repo) = gen_repo_system(setup_db().await);
    let ctx = ds.system_context();

    repo.insert(&ctx, &sample_table(9101, "sys_alpha", "A表"))
        .await
        .unwrap();
    repo.insert(&ctx, &sample_table(9102, "sys_beta", "B表"))
        .await
        .unwrap();
    repo.insert(&ctx, &sample_table(9103, "sys_gamma", "G表"))
        .await
        .unwrap();

    // 第一页: page=1, size=2
    let (rows1, total) = repo
        .select_page(&ctx, &empty_gen_query(), 1, 2)
        .await
        .unwrap();
    assert_eq!(total, 3);
    assert_eq!(rows1.len(), 2);

    // 第二页: page=2, size=2
    let (rows2, _) = repo
        .select_page(&ctx, &empty_gen_query(), 2, 2)
        .await
        .unwrap();
    assert_eq!(rows2.len(), 1);

    // 两页合起来覆盖全部 3 行
    let names: HashSet<String> = rows1
        .iter()
        .chain(rows2.iter())
        .map(|r| r.table_name.clone())
        .collect();
    assert!(names.contains("sys_alpha"));
    assert!(names.contains("sys_beta"));
    assert!(names.contains("sys_gamma"));
}

#[tokio::test]
async fn test_select_page_with_filter() {
    let (ds, repo) = gen_repo_system(setup_db().await);
    let ctx = ds.system_context();

    repo.insert(&ctx, &sample_table(9101, "sys_alpha", "A表"))
        .await
        .unwrap();
    repo.insert(&ctx, &sample_table(9102, "sys_beta", "B表"))
        .await
        .unwrap();

    let mut query = empty_gen_query();
    query.table_name = Some("alpha".to_string());
    let (rows, total) = repo.select_page(&ctx, &query, 1, 10).await.unwrap();
    assert_eq!(total, 1);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].table_name, "sys_alpha");
}

// ── import fills table_comment + respects config ──────────

#[test]
fn test_build_gen_table_fills_comment_and_config() {
    let table = build_gen_table_from_meta(
        "sys_user",
        "用户表（增强版）",
        "master",
        "crates.system",
        "vue",
        "RuoYi",
    );
    assert_eq!(table.table_name, "sys_user");
    assert_eq!(table.table_comment, "用户表（增强版）");
    assert_eq!(table.frontend_type, "vue");
    assert_eq!(table.package_name, "crates.system");
    assert_eq!(table.function_author, "RuoYi");
}

#[test]
fn test_build_gen_table_function_name_strips_parens() {
    let table = build_gen_table_from_meta(
        "sys_user",
        "用户表（增强版）",
        "master",
        "crates.system",
        "vue",
        "RuoYi",
    );
    assert_eq!(table.function_name, "用户表");
}

#[test]
fn test_build_gen_table_function_name_ascii_parens() {
    let table = build_gen_table_from_meta(
        "sys_user",
        "User table (enhanced)",
        "master",
        "crates.system",
        "vue",
        "RuoYi",
    );
    assert_eq!(table.function_name, "User table");
}

#[test]
fn test_build_gen_table_function_name_fallback_to_class_name() {
    let table = build_gen_table_from_meta(
        "sys_user",
        "",
        "master",
        "crates.system",
        "vue",
        "RuoYi",
    );
    // 评论为空时回退到 class_name
    assert_eq!(table.function_name, table.class_name);
    assert_eq!(table.class_name, "User");
}

// ── html_type 名称后缀覆盖应在类型默认值之后生效 ───────────

#[test]
fn html_type_name_overrides_apply() {
    use system::codegen::service::db_meta::resolve_html_type;

    // varchar *_type 后缀 → select（而非 input）
    assert_eq!(
        resolve_html_type("varchar(20)", 20, 0, "category_type"),
        "select"
    );

    // varchar *_image 后缀 → imageUpload（而非 input）
    assert_eq!(
        resolve_html_type("varchar(255)", 255, 0, "cover_image"),
        "imageUpload"
    );

    // varchar *_file 后缀 → fileUpload（而非 input）
    assert_eq!(
        resolve_html_type("varchar(255)", 255, 0, "attach_file"),
        "fileUpload"
    );

    // text content → editor
    assert_eq!(resolve_html_type("text", 0, 0, "content"), "editor");

    // varchar *_status 后缀 → radio
    assert_eq!(
        resolve_html_type("varchar(2)", 2, 0, "audit_status"),
        "radio"
    );

    // varchar *_sex 后缀 → select
    assert_eq!(resolve_html_type("varchar(2)", 2, 0, "user_sex"), "select");

    // varchar *_remark 后缀 → textarea
    assert_eq!(
        resolve_html_type("varchar(500)", 500, 0, "biz_remark"),
        "textarea"
    );

    // datetime → datetime（类型默认，无名称覆盖）
    assert_eq!(
        resolve_html_type("datetime", 0, 0, "create_time"),
        "datetime"
    );

    // int *_sort 后缀 → inputNumber
    assert_eq!(
        resolve_html_type("int(11)", 11, 0, "display_sort"),
        "inputNumber"
    );

    // 普通 varchar（无名称匹配）→ input（类型默认）
    assert_eq!(resolve_html_type("varchar(64)", 64, 0, "title"), "input");

    // text 无名称匹配 → textarea（类型默认）
    assert_eq!(resolve_html_type("text", 0, 0, "raw_data"), "textarea");
}

#[test]
fn html_type_boolean_still_switch() {
    use system::codegen::service::db_meta::resolve_html_type;
    // tinyint(1) status → switch（布尔）
    assert_eq!(resolve_html_type("tinyint(1)", 1, 0, "status"), "switch");
    // tinyint(1) is_enabled → switch
    assert_eq!(
        resolve_html_type("tinyint(1)", 1, 0, "is_enabled"),
        "switch"
    );
}

// ── synchDb preserves user-edited column attributes ────────

fn make_column(
    column_id: i64,
    table_id: i64,
    name: &str,
    comment: &str,
    col_type: &str,
) -> GenTableColumn {
    GenTableColumn {
        column_id,
        table_id,
        column_name: name.to_string(),
        column_comment: Some(comment.to_string()),
        column_type: col_type.to_string(),
        java_type: "String".to_string(),
        java_field: name.to_string(),
        is_pk: "0".to_string(),
        is_increment: "0".to_string(),
        is_required: "0".to_string(),
        is_insert: "1".to_string(),
        is_edit: "1".to_string(),
        is_list: "1".to_string(),
        is_query: "0".to_string(),
        query_type: "EQ".to_string(),
        html_type: "input".to_string(),
        dict_type: None,
        sort: 1,
        cap_java_field: None,
        super_column: None,
        usable_column: None,
        base_entity: BaseEntity {
            create_dept: None,
            create_by: None,
            create_time: None,
            update_by: None,
            update_time: None,
        },
    }
}

#[test]
fn merge_columns_preserves_user_edits() {
    // 已存在列带有用户编辑
    let mut existing = make_column(1001, 500, "title", "旧注释", "varchar(32)");
    existing.is_required = "0".to_string();
    existing.html_type = "select".to_string();
    existing.is_insert = "0".to_string();
    existing.is_edit = "0".to_string();
    existing.is_query = "0".to_string();
    existing.query_type = "LIKE".to_string();
    existing.dict_type = Some("sys_yes_no".to_string());

    // raw 列（同名，DB 未变更）
    let raw = vec![RawColumnInfo {
        column_name: "title".to_string(),
        column_comment: Some("标题".to_string()),
        column_type: "varchar(64)".to_string(),
        data_type: "varchar".to_string(),
        is_pk: false,
        is_auto_increment: false,
        is_nullable: false,
        ordinal_position: 1,
    }];

    let merged = merge_columns(&[existing], &raw, 500);

    assert_eq!(merged.len(), 1);
    let col = &merged[0];

    // 用户编辑字段应保留（7 个字段全部与 raw 派生值不同，才能证明保留）
    assert_eq!(col.is_required, "0", "is_required 应保留用户编辑");
    assert_eq!(col.html_type, "select", "html_type 应保留");
    assert_eq!(col.is_insert, "0", "is_insert 应保留");
    assert_eq!(col.is_edit, "0", "is_edit 应保留用户编辑");
    assert_eq!(col.is_query, "0", "is_query 应保留用户编辑");
    assert_eq!(col.query_type, "LIKE", "query_type 应保留");
    assert_eq!(
        col.dict_type.as_deref(),
        Some("sys_yes_no"),
        "dict_type 应保留"
    );

    // 结构字段应来自 raw
    assert_eq!(
        col.column_comment.as_deref(),
        Some("标题"),
        "column_comment 应来自 raw"
    );
    assert_eq!(col.column_type, "varchar(64)", "column_type 应来自 raw");

    // column_id 应保留
    assert_eq!(col.column_id, 1001, "column_id 应保留");
}

#[test]
fn merge_columns_drops_stale_and_adds_new() {
    // existing 有列 A, B
    let existing = vec![
        make_column(1, 700, "col_a", "A列", "varchar(32)"),
        make_column(2, 700, "col_b", "B列", "varchar(32)"),
    ];

    // raw 只有 B, C（A 被删除，C 是新列）
    let raw = vec![
        RawColumnInfo {
            column_name: "col_b".to_string(),
            column_comment: Some("B列更新".to_string()),
            column_type: "varchar(64)".to_string(),
            data_type: "varchar".to_string(),
            is_pk: false,
            is_auto_increment: false,
            is_nullable: false,
            ordinal_position: 1,
        },
        RawColumnInfo {
            column_name: "col_c".to_string(),
            column_comment: Some("C列".to_string()),
            column_type: "int".to_string(),
            data_type: "int".to_string(),
            is_pk: false,
            is_auto_increment: false,
            is_nullable: true,
            ordinal_position: 2,
        },
    ];

    let merged = merge_columns(&existing, &raw, 700);

    assert_eq!(merged.len(), 2, "应包含 B 和 C，A 被丢弃");

    let names: Vec<&str> =
        merged.iter().map(|c| c.column_name.as_str()).collect();
    assert!(names.contains(&"col_b"), "col_b 应存在");
    assert!(names.contains(&"col_c"), "col_c 应存在");
    assert!(!names.contains(&"col_a"), "col_a 应被丢弃");

    let col_b = merged.iter().find(|c| c.column_name == "col_b").unwrap();
    assert_eq!(col_b.column_id, 2, "col_b 的 column_id 应保留");
    assert_eq!(
        col_b.column_comment.as_deref(),
        Some("B列更新"),
        "col_b 的 comment 应来自 raw"
    );

    let col_c = merged.iter().find(|c| c.column_name == "col_c").unwrap();
    assert_eq!(col_c.column_type, "int", "col_c 的类型应来自 raw");
    assert_eq!(col_c.is_required, "0", "col_c 新列应为 nullable 默认");
}

#[test]
fn merge_columns_preserves_query_type_unconditionally() {
    // 即使不是 is_list 列，query_type 也应保留
    let mut existing = make_column(3, 800, "status", "状态", "varchar(2)");
    existing.is_list = "0".to_string(); // 不是列表列
    existing.query_type = "NE".to_string();

    let raw = vec![RawColumnInfo {
        column_name: "status".to_string(),
        column_comment: Some("状态".to_string()),
        column_type: "varchar(2)".to_string(),
        data_type: "varchar".to_string(),
        is_pk: false,
        is_auto_increment: false,
        is_nullable: true,
        ordinal_position: 1,
    }];

    let merged = merge_columns(&[existing], &raw, 800);

    assert_eq!(merged.len(), 1);
    assert_eq!(
        merged[0].query_type, "NE",
        "query_type 应无条件保留（不再限于 is_list）"
    );
}

// ── batchGenCode writes single valid zip ───────────────────

fn sample_table_with_columns(
    table_name: &str,
    class_name: &str,
    business_name: &str,
    table_id: i64,
) -> (GenTable, Vec<GenTableColumn>) {
    let table = GenTable {
        table_id,
        data_name: "master".to_string(),
        table_name: table_name.to_string(),
        table_comment: table_name.to_string(),
        class_name: class_name.to_string(),
        tpl_category: "crud".to_string(),
        frontend_type: "vue".to_string(),
        package_name: "crates.system".to_string(),
        module_name: "system".to_string(),
        business_name: business_name.to_string(),
        function_name: class_name.to_string(),
        function_author: "RuoYi".to_string(),
        gen_type: "0".to_string(),
        gen_path: "/".to_string(),
        options: None,
        remark: None,
        pk_column: None,
        columns: None,
        tree_code: None,
        tree_parent_code: None,
        tree_name: None,
        menu_ids: None,
        parent_menu_id: None,
        parent_menu_name: None,
        enable_export: None,
        enable_status: None,
        status_field: None,
        enable_unique: None,
        unique_fields: None,
        enable_sort: None,
        sort_field: None,
        tree_root_value: None,
        tree_ancestors_field: None,
        tree_order_field: None,
        base_entity: BaseEntity {
            create_dept: None,
            create_by: None,
            create_time: None,
            update_by: None,
            update_time: None,
        },
    };
    let mut id_col = make_column(1, table_id, "id", "主键", "bigint");
    id_col.is_pk = "1".to_string();
    id_col.is_insert = "0".to_string();
    id_col.java_type = "Long".to_string();

    let mut name_col = make_column(2, table_id, "name", "名称", "varchar(64)");
    name_col.is_required = "1".to_string();

    let mut remark_col =
        make_column(3, table_id, "remark", "备注", "varchar(255)");
    remark_col.is_insert = "0".to_string();

    let columns = vec![id_col, name_col, remark_col];
    (table, columns)
}

#[test]
fn batch_download_produces_valid_zip() {
    use std::io::{Cursor, Read};

    use system::codegen::service::code_gen::CodeGenEngine;
    use zip::ZipArchive;

    let engine = CodeGenEngine::new().expect("engine should init");

    let (table1, cols1) =
        sample_table_with_columns("sys_post", "Post", "post", 7001);
    let (table2, cols2) =
        sample_table_with_columns("sys_notice", "Notice", "notice", 7002);

    let bytes = engine
        .batch_download_code(&[(&table1, &cols1), (&table2, &cols2)])
        .expect("batch download should succeed");

    // 验证 zip 有效且可解析
    let reader = Cursor::new(bytes);
    let mut archive = ZipArchive::new(reader).expect("zip should be valid");
    assert!(!archive.is_empty(), "zip should contain files");

    // 验证至少能读取一个文件
    let mut file = archive.by_index(0).expect("first file should be readable");
    let mut contents = String::new();
    file.read_to_string(&mut contents)
        .expect("should read file");
    assert!(!contents.is_empty());
}

// ── rust-embed 编译期内嵌模板 ──────────────────────────────

#[test]
fn engine_init_works_without_filesystem() {
    // CodeGenEngine::new() 不依赖运行时 CWD,模板在编译期内嵌
    let engine = system::codegen::service::code_gen::CodeGenEngine::new();
    assert!(
        engine.is_ok(),
        "CodeGenEngine should init from embedded templates"
    );
}

// ── 真实模板渲染 ───────────────────────────────────────────

#[test]
fn render_produces_real_output() {
    use system::codegen::service::code_gen::CodeGenEngine;

    let engine = CodeGenEngine::new().unwrap();
    let (table, columns) =
        sample_table_with_columns("sys_post", "Post", "post", 7001);
    let files = engine.preview_code(&table, &columns).unwrap();

    // Rust 模型: 真实结构体 + 字段
    let model_path = files
        .keys()
        .find(|k| k.ends_with("/post.rs"))
        .expect("model should be written to src/system/post/post.rs");
    let model = files.get(model_path).expect("model should exist");
    assert!(
        model.contains("pub struct Post"),
        "model should define Post struct"
    );
    assert!(
        model.lines().count() > 15,
        "model should be substantial, not a stub"
    );

    // 主键由 snowflake 生成,非插入字段初始化为 None
    assert!(
        model.contains("id: Some(snowflake::next_id()),"),
        "from_create_bo should initialize PK from snowflake"
    );
    assert!(
        model.contains("remark: None,"),
        "from_create_bo should initialize non-insertable fields with None"
    );

    // UpdateBo 主键类型跟随列类型
    assert!(
        model.contains("pub id: Option<i64>,"),
        "UpdateBo PK type should be Option<i64>"
    );

    // from_update_bo 中 required 字段 unwrap,Option 字段直接赋值
    assert!(
        model.contains(
            "self.name = bo.name.unwrap_or_else(|| self.name.clone());"
        ),
        "from_update_bo should unwrap required fields"
    );
    assert!(
        model.contains("self.remark = bo.remark;"),
        "from_update_bo should assign Option fields directly"
    );

    // Rust 仓库: SeaORM 实体，且从 snake_case 模型模块导入
    let repo = files
        .values()
        .find(|v| v.contains("DeriveEntityModel"))
        .expect("repository should have SeaORM entity");
    assert!(
        repo.lines().count() > 15,
        "repository should be substantial"
    );
    assert!(
        repo.contains("use super::post::Post::"),
        "repository should import model from snake_case module"
    );

    // Rust 服务：业务逻辑 + 同样模型导入
    let service = files
        .values()
        .find(|v| v.contains("pub struct PostService"))
        .expect("service should exist");
    assert!(
        service.contains("use super::post::Post::"),
        "service should import model from snake_case module"
    );

    // Rust handler: Axum 处理器 + 权限守卫，且从 snake_case 模型模块导入
    let handler = files
        .values()
        .find(|v| v.contains("require_permission"))
        .expect("handler should have permission guards");
    assert!(
        handler.contains("R::"),
        "handler should return R<T> wrapper"
    );
    assert!(
        handler.contains("use super::post::Post::"),
        "handler should import model from snake_case module"
    );

    // Vue api.ts: 五个 CRUD 函数
    let api = files
        .values()
        .find(|v| v.contains("export function listPost"))
        .expect("api.ts should exist");
    assert!(
        api.matches("export").count() >= 5,
        "api should have at least 5 exported functions"
    );

    // Vue types.ts: 接口 + 字段
    let types = files
        .values()
        .find(|v| v.contains("interface Post"))
        .expect("types.ts should exist");
    assert!(types.lines().count() > 8, "types.ts should be substantial");

    // 菜单 SQL 仍然工作
    let sql = files
        .values()
        .find(|v| v.contains("sys_menu"))
        .expect("menu SQL should exist");
    assert!(sql.contains("insert"), "SQL should have INSERT");
}

#[test]
fn render_includes_menu_sql() {
    use system::codegen::service::code_gen::CodeGenEngine;

    let engine = CodeGenEngine::new().unwrap();
    let (mut table, columns) =
        sample_table_with_columns("sys_post", "Post", "post", 7001);
    table.function_name = "岗位".to_string();
    let files = engine.preview_code(&table, &columns).unwrap();

    let menu_sql = files
        .values()
        .find(|v| v.contains("sys_menu") && v.contains("insert"));
    assert!(
        menu_sql.is_some(),
        "should generate menu SQL with sys_menu inserts"
    );

    let sql = menu_sql.unwrap();
    assert!(
        sql.contains("system:post:list"),
        "should contain permission prefix"
    );
    assert!(sql.contains("岗位"), "should contain function_name");
    assert!(
        sql.contains("system:post:query"),
        "should contain query button permission"
    );
    assert!(
        sql.contains("system:post:export"),
        "should contain export button permission"
    );
}

// ============================================================
// 补充:resolve_java_type / db_meta 边界
// ============================================================

#[test]
fn resolve_java_type_varchar_returns_string() {
    assert_eq!(resolve_java_type("varchar", 255, 0, "name"), "String");
    assert_eq!(resolve_java_type("VARCHAR(50)", 50, 0, "code"), "String");
    assert_eq!(resolve_java_type("text", 0, 0, "body"), "String");
    assert_eq!(resolve_java_type("char", 10, 0, "flag"), "String");
}

#[test]
fn resolve_java_type_bigint_returns_long() {
    assert_eq!(resolve_java_type("bigint", 0, 0, "id"), "Long");
    assert_eq!(resolve_java_type("int8", 0, 0, "uid"), "Long");
}

#[test]
fn resolve_java_type_int_returns_integer() {
    assert_eq!(resolve_java_type("int", 0, 0, "age"), "Integer");
    assert_eq!(resolve_java_type("integer", 0, 0, "x"), "Integer");
    assert_eq!(resolve_java_type("smallint", 0, 0, "y"), "Integer");
}

#[test]
fn resolve_java_type_decimal_with_scale_returns_bigdecimal() {
    assert_eq!(resolve_java_type("decimal", 10, 2, "amount"), "BigDecimal");
    assert_eq!(resolve_java_type("numeric", 10, 4, "price"), "BigDecimal");
}

#[test]
fn resolve_java_type_decimal_without_scale_uses_integer_type() {
    // column_scale=0 -> 走 resolve_integer_type 分支(按长度选 Integer/Long)
    let t = resolve_java_type("decimal", 10, 0, "count");
    // resolve_integer_type 长度 <= 10 时返回 Integer, > 10 时返回 Long
    assert!(
        t == "Integer" || t == "Long",
        "decimal 无 scale 应退化为整数类型, got {t}"
    );
}

#[test]
fn resolve_java_type_datetime_returns_localdatetime() {
    assert_eq!(
        resolve_java_type("datetime", 0, 0, "create_time"),
        "LocalDateTime"
    );
    assert_eq!(resolve_java_type("timestamp", 0, 0, "ts"), "LocalDateTime");
    assert_eq!(resolve_java_type("date", 0, 0, "d"), "LocalDateTime");
}

#[test]
fn resolve_java_type_float_returns_double() {
    assert_eq!(resolve_java_type("float", 0, 0, "score"), "Double");
    assert_eq!(resolve_java_type("double", 0, 0, "ratio"), "Double");
}

#[test]
fn resolve_java_type_boolean_column_name_heuristic() {
    // is_xxx / has_xxx 列名即使类型是 tinyint 也应识别为 Boolean
    assert_eq!(resolve_java_type("tinyint", 1, 0, "is_active"), "Boolean");
    assert_eq!(resolve_java_type("tinyint", 1, 0, "has_perm"), "Boolean");
}

// ── D-T3: codegen 引擎 bug 特征化测试 ──────────────────────────────

#[test]
fn batch_gen_code_paths_do_not_conflict() {
    // 两表不同 business_name 应生成不重叠的输出路径
    let table_a = sample_table(1, "sys_user", "用户表");
    let table_b = sample_table(2, "sys_role", "角色表");
    // table_b 的 business_name 修改以模拟不同表
    let mut table_b = table_b;
    table_b.business_name = "role".to_string();
    table_b.class_name = "SysRole".to_string();

    let paths_a: Vec<String> = [
        "rust/model.rs.tera",
        "rust/repository.rs.tera",
        "rust/service.rs.tera",
        "vue/index.vue.tera",
    ]
    .iter()
    .map(|name| {
        system::codegen::service::code_gen::template_to_output_path(
            name, &table_a,
        )
    })
    .collect();
    let paths_b: Vec<String> = [
        "rust/model.rs.tera",
        "rust/repository.rs.tera",
        "rust/service.rs.tera",
        "vue/index.vue.tera",
    ]
    .iter()
    .map(|name| {
        system::codegen::service::code_gen::template_to_output_path(
            name, &table_b,
        )
    })
    .collect();

    let set_a: HashSet<&str> = paths_a.iter().map(|s| s.as_str()).collect();
    let set_b: HashSet<&str> = paths_b.iter().map(|s| s.as_str()).collect();
    assert!(
        set_a.intersection(&set_b).next().is_none(),
        "batchGenCode 多表 zip 不应有路径冲突: A={:?}, B={:?}",
        paths_a,
        paths_b
    );
}

#[test]
fn merge_columns_preserves_user_edits_v2() {
    // synchDb 合并时保留用户手改字段(html_type/is_required/is_insert/is_edit)
    let existing = vec![GenTableColumn {
        column_id: 1001,
        table_id: 1,
        column_name: "status".to_string(),
        column_comment: Some("状态".to_string()),
        column_type: "char(1)".to_string(),
        java_type: "String".to_string(),
        java_field: "status".to_string(),
        is_pk: "0".to_string(),
        is_increment: "0".to_string(),
        is_required: "1".to_string(), // 用户手改
        is_insert: "0".to_string(),   // 用户手改
        is_edit: "1".to_string(),
        is_list: "1".to_string(),
        is_query: "1".to_string(),
        query_type: "EQ".to_string(),
        html_type: "radio".to_string(), // 用户手改为 radio
        dict_type: Some("sys_normal_disable".to_string()),
        sort: 1,
        cap_java_field: None,
        super_column: None,
        usable_column: None,
        base_entity: empty_base_entity(),
    }];

    let raw = vec![RawColumnInfo {
        column_name: "status".to_string(),
        column_comment: Some("状态".to_string()),
        column_type: "char(1)".to_string(),
        data_type: "char".to_string(),
        is_pk: false,
        is_auto_increment: false,
        is_nullable: true,
        ordinal_position: 1,
    }];

    let merged = merge_columns(&existing, &raw, 1);
    assert_eq!(merged.len(), 1);
    let col = &merged[0];
    assert_eq!(col.html_type, "radio", "手改 html_type 应保留");
    assert_eq!(col.is_required, "1", "手改 is_required 应保留");
    assert_eq!(col.is_insert, "0", "手改 is_insert 应保留");
    assert_eq!(
        col.dict_type.as_deref(),
        Some("sys_normal_disable"),
        "dict_type 应保留"
    );
}

#[test]
fn resolve_html_type_correct_mappings() {
    // 验证关键 html_type 推断与 Java 一致
    assert_eq!(
        resolve_html_type("datetime", 0, 0, "create_time"),
        "datetime"
    );
    assert_eq!(
        resolve_html_type("char", 1, 0, "status"),
        "switch",
        "char(1)+status→boolean→switch"
    );
    assert_eq!(
        resolve_html_type("varchar", 2, 0, "status"),
        "radio",
        "varchar(2)+status→radio"
    );
    assert_eq!(resolve_html_type("tinyint", 1, 0, "is_enabled"), "switch");
    assert_eq!(resolve_html_type("varchar", 255, 0, "remark"), "textarea");
    assert_eq!(resolve_html_type("varchar", 100, 0, "xxx_id"), "input");
    assert_eq!(resolve_html_type("varchar", 10, 0, "user_type"), "select");
    assert_eq!(
        resolve_html_type("varchar", 255, 0, "avatar"),
        "imageUpload"
    );
    assert_eq!(
        resolve_html_type("bigint", 0, 0, "order_num"),
        "inputNumber"
    );
}

fn empty_base_entity() -> BaseEntity {
    BaseEntity {
        create_dept: None,
        create_by: None,
        create_time: None,
        update_by: None,
        update_time: None,
    }
}
