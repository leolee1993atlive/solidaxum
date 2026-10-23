use std::{
    collections::HashMap,
    io::{Cursor, Write},
    sync::Arc,
};

use app_core::{
    error::{AppError, AppResult},
    util::snowflake,
};
use rust_embed::RustEmbed;
use tera::{Context, Tera};
use zip::ZipWriter;

use crate::codegen::model::{gen_column::GenTableColumn, gen_table::GenTable};

/// 编译期内嵌模板文件,运行时不依赖文件系统
#[derive(RustEmbed)]
#[folder = "src/codegen/templates"]
struct Templates;

// ── Tera 过滤器 ─────────────────────────────────────────────────────

/// 将 Java 类型名映射为 Rust 类型名
fn rust_type_filter(
    value: &tera::Value,
    _: &HashMap<String, tera::Value>,
) -> tera::Result<tera::Value> {
    let java_type = value.as_str().unwrap_or("String");
    let rust_type = match java_type {
        "String" => "String",
        "Long" => "i64",
        "Integer" => "i32",
        "Boolean" => "bool",
        "Double" => "f64",
        "BigDecimal" => "rust_decimal::Decimal",
        "LocalDateTime" => "chrono::DateTime<chrono::Utc>",
        _ => "String",
    };
    Ok(tera::Value::String(rust_type.to_string()))
}

/// 将 snake_case 转为 PascalCase(用于 SeaORM Column 枚举名)
fn pascal_case_filter(
    value: &tera::Value,
    _: &HashMap<String, tera::Value>,
) -> tera::Result<tera::Value> {
    let s = value.as_str().unwrap_or("");
    let pascal: String = s
        .split('_')
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                Some(first) => {
                    first.to_uppercase().collect::<String>() + chars.as_str()
                }
                None => String::new(),
            }
        })
        .collect();
    Ok(tera::Value::String(pascal))
}

#[derive(Clone)]
pub struct CodeGenEngine {
    tera: Arc<Tera>,
}

impl CodeGenEngine {
    pub fn new() -> AppResult<Self> {
        // 模板在编译期内嵌进二进制,运行时直接从内存加载
        let mut tera = Tera::default();
        for name in Templates::iter() {
            if let Some(file) = Templates::get(name.as_ref()) {
                let content = std::str::from_utf8(&file.data).map_err(|e| {
                    AppError::Internal(format!("模板 UTF-8 解码失败: {}", e))
                })?;
                tera.add_raw_template(&name, content).map_err(|e| {
                    AppError::Internal(format!("模板注册失败: {}", e))
                })?;
            }
        }
        tera.register_filter("rust_type", rust_type_filter);
        tera.register_filter("pascal", pascal_case_filter);
        Ok(Self {
            tera: Arc::new(tera),
        })
    }

    pub fn build_context(
        &self,
        table: &GenTable,
        columns: &[GenTableColumn],
    ) -> Context {
        let mut ctx = Context::new();
        ctx.insert("package_name", &table.package_name);
        ctx.insert("module_name", &table.module_name);
        ctx.insert("business_name", &table.business_name);
        ctx.insert("class_name", &table.class_name);
        ctx.insert("function_name", &table.function_name);
        ctx.insert("function_author", &table.function_author);
        ctx.insert("table_name", &table.table_name);
        ctx.insert("table_comment", &table.table_comment);
        ctx.insert("tpl_category", &table.tpl_category);
        ctx.insert("is_tree", &table.is_tree());
        ctx.insert("is_crud", &table.is_crud());

        let list_columns: Vec<&GenTableColumn> =
            columns.iter().filter(|c| c.is_list()).collect();
        let query_columns: Vec<&GenTableColumn> =
            columns.iter().filter(|c| c.is_query()).collect();
        let insert_columns: Vec<&GenTableColumn> = columns
            .iter()
            .filter(|c| c.is_insert() && !c.is_pk())
            .collect();
        let edit_columns: Vec<&GenTableColumn> =
            columns.iter().filter(|c| c.is_edit()).collect();

        ctx.insert("columns", columns);
        ctx.insert("list_columns", &list_columns);
        ctx.insert("query_columns", &query_columns);
        ctx.insert("insert_columns", &insert_columns);
        ctx.insert("edit_columns", &edit_columns);
        ctx.insert(
            "datetime",
            &chrono::Utc::now().format("%Y-%m-%d").to_string(),
        );

        ctx.insert("tree_code", &table.tree_code);
        ctx.insert("tree_parent_code", &table.tree_parent_code);
        ctx.insert("tree_name", &table.tree_name);

        // 菜单 SQL 所需变量
        let permission_prefix =
            format!("{}:{}", table.module_name, table.business_name);
        ctx.insert("permission_prefix", &permission_prefix);
        ctx.insert("parent_menu_id", &table.parent_menu_id.unwrap_or(0));

        let menu_ids: Vec<i64> = match &table.menu_ids {
            Some(ids) if ids.len() >= 6 => ids.clone(),
            _ => (0..6).map(|_| snowflake::next_id()).collect(),
        };
        ctx.insert("menu_ids", &menu_ids);

        // ── 模板渲染所需的派生变量 ───────────────────────────────────

        // business_name_cap: 首字母大写,用于 Vue 函数名如 listPost
        let business_name_cap = {
            let mut chars = table.business_name.chars();
            match chars.next() {
                Some(c) => {
                    c.to_uppercase().collect::<String>() + chars.as_str()
                }
                None => String::new(),
            }
        };
        ctx.insert("business_name_cap", &business_name_cap);

        // dicts: 逗号分隔的带引号字典类型,用于 Vue useDict
        let dict_set: Vec<String> = {
            let mut seen = std::collections::HashSet::new();
            columns
                .iter()
                .filter_map(|c| c.dict_type.clone())
                .filter(|d| !d.is_empty())
                .filter(|d| seen.insert(d.clone()))
                .map(|d| format!("'{}'", d))
                .collect()
        };
        ctx.insert("dicts", &dict_set.join(","));

        // pk_field / pk_field_js: 主键字段名(snake_case / camelCase)
        let pk_col: Option<&GenTableColumn> = table
            .pk_column
            .as_ref()
            .or_else(|| columns.iter().find(|c| c.is_pk()))
            .or_else(|| columns.first());
        let pk_field =
            pk_col.map(|c| c.column_name.clone()).unwrap_or_default();
        let pk_field_js =
            pk_col.map(|c| c.java_field.clone()).unwrap_or_default();
        ctx.insert("pk_field", &pk_field);
        ctx.insert("pk_field_js", &pk_field_js);
        ctx.insert("pk_column", &pk_col);

        ctx
    }

    pub fn render(
        &self,
        template_name: &str,
        context: &Context,
    ) -> AppResult<String> {
        self.tera.render(template_name, context).map_err(|e| {
            AppError::Internal(format!("Template render error: {}", e))
        })
    }

    pub fn template_names(
        &self,
        tpl_category: &str,
        frontend_type: &str,
    ) -> Vec<String> {
        let rust_templates = [
            "rust/model.rs.tera",
            "rust/repository.rs.tera",
            "rust/service.rs.tera",
            "rust/handler.rs.tera",
            "rust/route.rs.tera",
        ];

        let frontend_templates: &[&str] = match frontend_type {
            "svelte" => &[
                "svelte/+page.svelte.tera",
                "svelte/+page.ts.tera",
                "svelte/types.ts.tera",
            ],
            _ => {
                &["vue/index.vue.tera", "vue/api.ts.tera", "vue/types.ts.tera"]
            }
        };

        let tree_templates: &[&str] = if tpl_category == "tree" {
            match frontend_type {
                "svelte" => &["svelte/+page-tree.svelte.tera"],
                _ => &["vue/index-tree.vue.tera"],
            }
        } else {
            &[]
        };

        let mut all: Vec<String> = Vec::new();
        all.extend(rust_templates.iter().map(|s| s.to_string()));
        all.extend(frontend_templates.iter().map(|s| s.to_string()));
        all.extend(tree_templates.iter().map(|s| s.to_string()));
        all.push("sql/menu.sql.tera".to_string());
        all
    }

    pub fn preview_code(
        &self,
        table: &GenTable,
        columns: &[GenTableColumn],
    ) -> AppResult<HashMap<String, String>> {
        let ctx = self.build_context(table, columns);
        let names =
            self.template_names(&table.tpl_category, &table.frontend_type);
        let mut result = HashMap::new();

        for template_name in &names {
            let rendered = self.render(template_name, &ctx)?;
            let output_path = template_to_output_path(template_name, table);
            result.insert(output_path, rendered);
        }

        Ok(result)
    }

    /// 将单张表的渲染结果写入已有的 ZipWriter(不调用 finish)
    fn write_table_files<W: std::io::Write + std::io::Seek>(
        &self,
        zip: &mut zip::ZipWriter<W>,
        options: zip::write::FileOptions<()>,
        table: &GenTable,
        columns: &[GenTableColumn],
    ) -> AppResult<()> {
        let ctx = self.build_context(table, columns);
        let names =
            self.template_names(&table.tpl_category, &table.frontend_type);

        for template_name in &names {
            let rendered = self.render(template_name, &ctx)?;
            let output_path = template_to_output_path(template_name, table);
            zip.start_file(&output_path, options)
                .map_err(|e| AppError::Internal(format!("ZIP error: {}", e)))?;
            zip.write_all(rendered.as_bytes()).map_err(|e| {
                AppError::Internal(format!("ZIP write error: {}", e))
            })?;
        }

        Ok(())
    }

    pub fn download_code(
        &self,
        table: &GenTable,
        columns: &[GenTableColumn],
    ) -> AppResult<Vec<u8>> {
        let mut buffer = Cursor::new(Vec::new());
        {
            let mut zip = ZipWriter::new(&mut buffer);
            let options = zip::write::FileOptions::<()>::default()
                .compression_method(zip::CompressionMethod::Deflated);

            self.write_table_files(&mut zip, options, table, columns)?;

            zip.finish().map_err(|e| {
                AppError::Internal(format!("ZIP finish error: {}", e))
            })?;
        }

        Ok(buffer.into_inner())
    }

    /// 批量生成多张表的代码,写入同一个 zip 归档
    pub fn batch_download_code(
        &self,
        tables: &[(&GenTable, &[GenTableColumn])],
    ) -> AppResult<Vec<u8>> {
        let mut buffer = Cursor::new(Vec::new());
        {
            let mut zip = ZipWriter::new(&mut buffer);
            let options = zip::write::FileOptions::<()>::default()
                .compression_method(zip::CompressionMethod::Deflated);

            for (table, columns) in tables {
                self.write_table_files(&mut zip, options, table, columns)?;
            }

            zip.finish().map_err(|e| {
                AppError::Internal(format!("ZIP finish error: {}", e))
            })?;
        }

        Ok(buffer.into_inner())
    }
}

pub fn template_to_output_path(
    template_name: &str,
    table: &GenTable,
) -> String {
    let class_name = &table.class_name;
    let business_name = &table.business_name;
    let module_name = &table.module_name;

    match template_name {
        "rust/model.rs.tera" => {
            format!(
                "src/{}/{}/{}.rs",
                module_name, business_name, business_name
            )
        }
        "rust/repository.rs.tera" => format!(
            "src/{}/{}/{}_repo.rs",
            module_name, business_name, business_name
        ),
        "rust/service.rs.tera" => format!(
            "src/{}/{}/{}_service.rs",
            module_name, business_name, business_name
        ),
        "rust/handler.rs.tera" => format!(
            "src/{}/{}/{}_handler.rs",
            module_name, business_name, business_name
        ),
        "rust/route.rs.tera" => format!(
            "src/{}/{}/{}_route.rs",
            module_name, business_name, business_name
        ),
        "vue/index.vue.tera" => {
            format!("src/views/{}/{}/index.vue", module_name, business_name)
        }
        "vue/api.ts.tera" => {
            format!("src/api/{}/{}/index.ts", module_name, business_name)
        }
        "vue/types.ts.tera" => {
            format!("src/api/{}/{}/types.ts", module_name, business_name)
        }
        "svelte/+page.svelte.tera" => {
            format!("src/routes/{}/{}/+page.svelte", module_name, business_name)
        }
        "svelte/+page.ts.tera" => {
            format!("src/routes/{}/{}/+page.ts", module_name, business_name)
        }
        "svelte/types.ts.tera" => {
            format!("src/lib/types/{}.ts", business_name)
        }
        "sql/menu.sql.tera" => format!("sql/{}/menu.sql", class_name),
        _ => template_name
            .strip_suffix(".tera")
            .unwrap_or(template_name)
            .to_string(),
    }
}
