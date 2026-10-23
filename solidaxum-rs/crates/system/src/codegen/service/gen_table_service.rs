use std::{collections::HashMap, sync::Arc};

use app_core::{
    error::{AppError, AppResult},
    model::entity::BaseEntity,
    util::snowflake,
    vo::page::normalize_paging,
};
use app_data::DataContext;

use crate::codegen::{
    model::{
        gen_column::{GenColumnUpdateBo, GenTableColumn},
        gen_table::{
            DbTableQueryBo, DbTableVo, GenTable, GenTableQueryBo,
            GenTableUpdateBo,
        },
    },
    repository::{
        gen_column_repo::GenColumnRepository,
        gen_table_repo::GenTableRepository,
    },
    service::db_meta::{
        DbMetaReader, RawColumnInfo, resolve_html_type, resolve_java_type,
    },
};

#[derive(Clone)]
pub struct GenTableService {
    /// 元数据读取经 app-data 漏斗(设计 6.3 meta 行)。system_context 语义:
    /// 代码生成是管理功能,不解析行级范围。这里捕获的恒为装配后句柄
    /// (set_data_source 先于容器构造,未装配即 panic);且
    /// gen_table/gen_column 落 CONSTRAINED=false,漏斗只在 CONSTRAINED
    /// 时解析 resolver —— 全豁免读下 resolver 在不在句柄上,语义无差别。
    /// 仓储无状态(执行面全在调用方 ctx),本字段只剩元数据读取一个消费者
    /// —— 即设计 6.3 的 meta 行,`system_context` 三处
    /// (list_db_tables / import_tables / synch_db)。
    ds: Arc<app_data::DataSource>,
    gen_table_repo: GenTableRepository,
    gen_column_repo: GenColumnRepository,
    package_name: String,
    frontend_type: String,
    author: String,
}

impl GenTableService {
    pub fn new(
        ds: Arc<app_data::DataSource>,
        package_name: String,
        frontend_type: String,
        author: String,
    ) -> Self {
        Self {
            gen_table_repo: GenTableRepository::new(),
            gen_column_repo: GenColumnRepository::new(),
            ds,
            package_name,
            frontend_type,
            author,
        }
    }

    pub async fn get_detail(
        &self,
        ctx: &DataContext<'_>,
        table_id: i64,
    ) -> AppResult<(GenTable, Vec<GenTableColumn>)> {
        let table = self
            .gen_table_repo
            .select_by_id(ctx, table_id)
            .await?
            .ok_or(AppError::NotFound(format!(
                "gen_table {} not found",
                table_id
            )))?;
        let columns = self
            .gen_column_repo
            .select_by_table_id(ctx, table_id)
            .await?;
        Ok((table, columns))
    }

    pub async fn list_columns(
        &self,
        ctx: &DataContext<'_>,
        table_id: i64,
    ) -> AppResult<Vec<GenTableColumn>> {
        Ok(self
            .gen_column_repo
            .select_by_table_id(ctx, table_id)
            .await?)
    }

    pub async fn list_page(
        &self,
        ctx: &DataContext<'_>,
        query: &GenTableQueryBo,
    ) -> AppResult<(Vec<GenTable>, u64)> {
        let (current, size, _offset) = normalize_paging(&query.page);
        Ok(self
            .gen_table_repo
            .select_page(ctx, query, current as u64, size as u64)
            .await?)
    }

    pub async fn list_db_tables(
        &self,
        ctx: &DataContext<'_>,
        query: &DbTableQueryBo,
    ) -> AppResult<(Vec<DbTableVo>, u64)> {
        // 设计 6.3 meta 行:库表元数据读走 system_context
        let meta_ctx = self.ds.system_context();
        let reader = DbMetaReader::new(&meta_ctx);
        let data_name = query.data_name.as_deref().unwrap_or("master");
        let imported = self
            .gen_table_repo
            .select_imported_table_names(ctx, data_name)
            .await?;
        let (_, limit, offset) = normalize_paging(&query.page);
        reader
            .list_tables(
                data_name,
                query.table_name.as_deref(),
                query.table_comment.as_deref(),
                &imported,
                offset as u64,
                limit as u64,
            )
            .await
    }

    pub async fn import_tables(
        &self,
        ctx: &DataContext<'_>,
        table_names: &[String],
        data_name: &str,
    ) -> AppResult<()> {
        // 设计 6.3 meta 行:库表元数据读走 system_context
        let meta_ctx = self.ds.system_context();
        let reader = DbMetaReader::new(&meta_ctx);

        for table_name in table_names {
            let raw_columns =
                reader.list_columns(data_name, table_name).await?;
            if raw_columns.is_empty() {
                return Err(AppError::NotFound(format!(
                    "表 {} 不存在或无列信息",
                    table_name
                )));
            }

            let table_comment =
                reader.get_table_comment(data_name, table_name).await?;

            let gen_table = build_gen_table_from_meta(
                table_name,
                &table_comment,
                data_name,
                &self.package_name,
                &self.frontend_type,
                &self.author,
            );

            // 审计五列由漏斗按 ctx principal 盖章(导入人)
            self.gen_table_repo.insert(ctx, &gen_table).await?;

            let columns: Vec<GenTableColumn> = raw_columns
                .iter()
                .enumerate()
                .map(|(idx, raw)| {
                    init_column_from_raw(raw, gen_table.table_id, idx + 1)
                })
                .collect();

            self.gen_column_repo.insert_batch(ctx, &columns).await?;
        }

        Ok(())
    }

    pub async fn update(
        &self,
        ctx: &DataContext<'_>,
        bo: GenTableUpdateBo,
    ) -> AppResult<()> {
        if bo.tpl_category == "tree" {
            if bo.tree_code.as_deref().filter(|s| !s.is_empty()).is_none() {
                return Err(AppError::BadRequest(
                    "树编码字段不能为空".to_string(),
                ));
            }
            if bo
                .tree_parent_code
                .as_deref()
                .filter(|s| !s.is_empty())
                .is_none()
            {
                return Err(AppError::BadRequest(
                    "树父编码字段不能为空".to_string(),
                ));
            }
            if bo.tree_name.as_deref().filter(|s| !s.is_empty()).is_none() {
                return Err(AppError::BadRequest(
                    "树名称字段不能为空".to_string(),
                ));
            }
        }

        let options = build_options_json(&bo);

        // 注意:BO 不带 create_* 与 data_name,update 是全列 Set ——
        // 这些列随领域层原值(空)写回,保持既有形状不改
        // 语义(update_* 改由漏斗按 principal 盖章,0 行 → NotFound)。
        let table = GenTable {
            table_id: bo.table_id,
            data_name: String::new(),
            table_name: bo.table_name,
            table_comment: bo.table_comment,
            class_name: bo.class_name,
            tpl_category: bo.tpl_category,
            frontend_type: bo.frontend_type,
            package_name: bo.package_name,
            module_name: bo.module_name,
            business_name: bo.business_name,
            function_name: bo.function_name,
            function_author: bo.function_author,
            gen_type: bo.gen_type.unwrap_or_else(|| "0".to_string()),
            gen_path: bo.gen_path.unwrap_or_else(|| "/".to_string()),
            options: Some(options),
            remark: bo.remark,
            pk_column: None,
            columns: None,
            tree_code: bo.tree_code,
            tree_parent_code: bo.tree_parent_code,
            tree_name: bo.tree_name,
            menu_ids: None,
            parent_menu_id: bo.parent_menu_id,
            parent_menu_name: bo.parent_menu_name,
            enable_export: bo.enable_export,
            enable_status: bo.enable_status,
            status_field: bo.status_field,
            enable_unique: bo.enable_unique,
            unique_fields: bo.unique_fields,
            enable_sort: bo.enable_sort,
            sort_field: bo.sort_field,
            tree_root_value: bo.tree_root_value,
            tree_ancestors_field: bo.tree_ancestors_field,
            tree_order_field: bo.tree_order_field,
            base_entity: BaseEntity {
                create_dept: None,
                create_by: None,
                create_time: None,
                update_by: None,
                update_time: Some(chrono::Utc::now()),
            },
        };
        self.gen_table_repo.update_by_id(ctx, &table).await?;

        if let Some(ref columns) = bo.columns {
            let normalized: Vec<GenTableColumn> = columns
                .iter()
                .map(|c| {
                    let mut col = column_bo_to_entity(c);
                    if !col.is_dict_column() {
                        col.dict_type = None;
                    }
                    col
                })
                .collect();
            self.gen_column_repo.update_batch(ctx, &normalized).await?;
        }

        Ok(())
    }

    pub async fn delete_by_ids(
        &self,
        ctx: &DataContext<'_>,
        ids: &[i64],
    ) -> AppResult<()> {
        self.gen_column_repo.delete_by_table_ids(ctx, ids).await?;
        self.gen_table_repo.delete_by_ids(ctx, ids).await?;
        Ok(())
    }

    pub async fn synch_db(
        &self,
        ctx: &DataContext<'_>,
        table_id: i64,
    ) -> AppResult<()> {
        let table = self
            .gen_table_repo
            .select_by_id(ctx, table_id)
            .await?
            .ok_or(AppError::NotFound("业务表不存在".to_string()))?;
        let existing_columns = self
            .gen_column_repo
            .select_by_table_id(ctx, table_id)
            .await?;
        // 设计 6.3 meta 行:库表元数据读走 system_context
        let meta_ctx = self.ds.system_context();
        let reader = DbMetaReader::new(&meta_ctx);

        let raw_columns = reader
            .list_columns(&table.data_name, &table.table_name)
            .await?;
        if raw_columns.is_empty() {
            return Err(AppError::NotFound(
                "同步数据失败，原表结构不存在".to_string(),
            ));
        }

        let existing_map: HashMap<String, GenTableColumn> = existing_columns
            .into_iter()
            .map(|c| (c.column_name.clone(), c))
            .collect();

        let save_columns =
            merge_columns_map(&existing_map, &raw_columns, table_id);

        self.gen_column_repo
            .delete_by_table_ids(ctx, &[table_id])
            .await?;
        self.gen_column_repo
            .insert_batch(ctx, &save_columns)
            .await?;

        Ok(())
    }
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

/// 将 DB 最新列元数据与现有 GenTableColumn 合并:
/// 已存在列保留用户编辑字段(is_required/html_type/is_insert/is_edit/is_query/dict_type/query_type),
/// 结构字段(column_comment/column_type/java_type/java_field/is_pk/is_increment/is_list/sort)从 raw 重新派生。
/// 其中 is_list 不保留用户编辑，始终根据列名重新计算（结构性属性）。
/// DB 已无的陈旧列不出现在返回值中。新列以默认值插入。
pub fn merge_columns(
    existing: &[GenTableColumn],
    raw_columns: &[RawColumnInfo],
    table_id: i64,
) -> Vec<GenTableColumn> {
    let existing_map: HashMap<String, GenTableColumn> = existing
        .iter()
        .map(|c| (c.column_name.clone(), c.clone()))
        .collect();
    merge_columns_map(&existing_map, raw_columns, table_id)
}

fn merge_columns_map(
    existing_map: &HashMap<String, GenTableColumn>,
    raw_columns: &[RawColumnInfo],
    table_id: i64,
) -> Vec<GenTableColumn> {
    let mut save_columns = Vec::with_capacity(raw_columns.len());
    for (idx, raw) in raw_columns.iter().enumerate() {
        let mut column = init_column_from_raw(raw, table_id, idx + 1);

        if let Some(prev) = existing_map.get(&column.column_name) {
            // 保留 column_id
            column.column_id = prev.column_id;
            // 保留用户编辑字段
            column.is_required = prev.is_required.clone();
            column.html_type = prev.html_type.clone();
            column.is_insert = prev.is_insert.clone();
            column.is_edit = prev.is_edit.clone();
            column.is_query = prev.is_query.clone();
            column.dict_type = prev.dict_type.clone();
            column.query_type = prev.query_type.clone();
        }
        save_columns.push(column);
    }
    save_columns
}

/// 从表元数据构建 GenTable(纯函数,便于测试)
pub fn build_gen_table_from_meta(
    table_name: &str,
    table_comment: &str,
    data_name: &str,
    package_name: &str,
    frontend_type: &str,
    author: &str,
) -> GenTable {
    let table_id = snowflake::next_id();
    let class_name = convert_class_name(table_name);
    let business_name = get_business_name(table_name);
    let function_name = derive_function_name(table_comment, &class_name);

    GenTable {
        table_id,
        data_name: data_name.to_string(),
        table_name: table_name.to_string(),
        table_comment: table_comment.to_string(),
        class_name,
        tpl_category: "crud".to_string(),
        frontend_type: frontend_type.to_string(),
        package_name: package_name.to_string(),
        module_name: "system".to_string(),
        business_name,
        function_name,
        function_author: author.to_string(),
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
        base_entity: empty_base_entity(),
    }
}

/// 从表注释推导功能名称:去除括号及之后内容;注释为空则回退到 class_name
fn derive_function_name(table_comment: &str, class_name: &str) -> String {
    if table_comment.is_empty() {
        return class_name.to_string();
    }
    let stripped = table_comment
        .split(['(', '\u{ff08}'])
        .next()
        .unwrap_or(table_comment)
        .trim();
    if stripped.is_empty() {
        class_name.to_string()
    } else {
        stripped.to_string()
    }
}

fn column_bo_to_entity(bo: &GenColumnUpdateBo) -> GenTableColumn {
    GenTableColumn {
        column_id: bo.column_id,
        table_id: bo.table_id,
        column_name: bo.column_name.clone(),
        column_comment: bo.column_comment.clone(),
        column_type: bo.column_type.clone(),
        java_type: bo.java_type.clone(),
        java_field: bo.java_field.clone(),
        is_pk: bo.is_pk.clone(),
        is_increment: bo.is_increment.clone(),
        is_required: bo.is_required.clone(),
        is_insert: bo.is_insert.clone(),
        is_edit: bo.is_edit.clone(),
        is_list: bo.is_list.clone(),
        is_query: bo.is_query.clone(),
        query_type: bo.query_type.clone(),
        html_type: bo.html_type.clone(),
        dict_type: bo.dict_type.clone(),
        sort: bo.sort,
        cap_java_field: None,
        super_column: None,
        usable_column: None,
        base_entity: empty_base_entity(),
    }
}

fn build_options_json(bo: &GenTableUpdateBo) -> String {
    let mut map = serde_json::Map::new();
    if let Some(ref v) = bo.tree_code {
        map.insert("treeCode".into(), serde_json::Value::String(v.clone()));
    }
    if let Some(ref v) = bo.tree_parent_code {
        map.insert(
            "treeParentCode".into(),
            serde_json::Value::String(v.clone()),
        );
    }
    if let Some(ref v) = bo.tree_name {
        map.insert("treeName".into(), serde_json::Value::String(v.clone()));
    }
    if let Some(v) = bo.parent_menu_id {
        map.insert("parentMenuId".into(), serde_json::Value::Number(v.into()));
    }
    if let Some(ref v) = bo.parent_menu_name {
        map.insert(
            "parentMenuName".into(),
            serde_json::Value::String(v.clone()),
        );
    }
    if let Some(v) = bo.enable_export {
        map.insert("enableExport".into(), serde_json::Value::Bool(v));
    }
    if let Some(v) = bo.enable_status {
        map.insert("enableStatus".into(), serde_json::Value::Bool(v));
    }
    if let Some(ref v) = bo.status_field {
        map.insert("statusField".into(), serde_json::Value::String(v.clone()));
    }
    if let Some(v) = bo.enable_unique {
        map.insert("enableUnique".into(), serde_json::Value::Bool(v));
    }
    if let Some(ref v) = bo.unique_fields {
        map.insert(
            "uniqueFields".into(),
            serde_json::Value::Array(
                v.iter()
                    .map(|s| serde_json::Value::String(s.clone()))
                    .collect(),
            ),
        );
    }
    if let Some(v) = bo.enable_sort {
        map.insert("enableSort".into(), serde_json::Value::Bool(v));
    }
    if let Some(ref v) = bo.sort_field {
        map.insert("sortField".into(), serde_json::Value::String(v.clone()));
    }
    if let Some(ref v) = bo.tree_root_value {
        map.insert(
            "treeRootValue".into(),
            serde_json::Value::String(v.clone()),
        );
    }
    if let Some(ref v) = bo.tree_ancestors_field {
        map.insert(
            "treeAncestors".into(),
            serde_json::Value::String(v.clone()),
        );
    }
    if let Some(ref v) = bo.tree_order_field {
        map.insert(
            "treeOrderField".into(),
            serde_json::Value::String(v.clone()),
        );
    }
    serde_json::Value::Object(map).to_string()
}

fn init_column_from_raw(
    raw: &RawColumnInfo,
    table_id: i64,
    sort: usize,
) -> GenTableColumn {
    let column_name = raw.column_name.to_lowercase();
    let java_field = to_camel_case(&column_name);
    let column_length = get_column_length(&raw.column_type);
    let column_scale = get_column_scale(&raw.column_type);
    let java_type = resolve_java_type(
        &raw.column_type,
        column_length,
        column_scale,
        &column_name,
    )
    .to_string();
    let html_type = resolve_html_type(
        &raw.column_type,
        column_length,
        column_scale,
        &column_name,
    )
    .to_string();

    const NOT_ADD: &[&str] = &[
        "create_dept",
        "create_by",
        "create_time",
        "del_flag",
        "update_by",
        "update_time",
        "version",
    ];
    const NOT_EDIT: &[&str] = &[
        "create_dept",
        "create_by",
        "create_time",
        "del_flag",
        "update_by",
        "update_time",
        "version",
    ];
    const NOT_LIST: &[&str] = &[
        "create_dept",
        "create_by",
        "create_time",
        "del_flag",
        "update_by",
        "update_time",
        "version",
    ];
    const NOT_QUERY: &[&str] = &[
        "id",
        "create_dept",
        "create_by",
        "create_time",
        "del_flag",
        "update_by",
        "update_time",
        "remark",
        "version",
    ];

    let is_insert = if !NOT_ADD.contains(&column_name.as_str()) && !raw.is_pk {
        "1"
    } else {
        "0"
    };
    let is_edit = if !NOT_EDIT.contains(&column_name.as_str()) {
        "1"
    } else {
        "0"
    };
    let is_list = if !NOT_LIST.contains(&column_name.as_str()) {
        "1"
    } else {
        "0"
    };
    let is_query = if !NOT_QUERY.contains(&column_name.as_str()) && !raw.is_pk {
        "1"
    } else {
        "0"
    };

    let mut query_type = "EQ".to_string();
    if column_name.ends_with("name") {
        query_type = "LIKE".to_string();
    }
    if html_type == "datetime" && is_query == "1" {
        query_type = "BETWEEN".to_string();
    }

    GenTableColumn {
        column_id: snowflake::next_id(),
        table_id,
        column_name: raw.column_name.clone(),
        column_comment: raw.column_comment.clone().filter(|c| !c.is_empty()),
        column_type: raw.column_type.to_lowercase(),
        java_type,
        java_field,
        is_pk: if raw.is_pk { "1" } else { "0" }.to_string(),
        is_increment: if raw.is_auto_increment { "1" } else { "0" }.to_string(),
        is_required: if !raw.is_nullable { "1" } else { "0" }.to_string(),
        is_insert: is_insert.to_string(),
        is_edit: is_edit.to_string(),
        is_list: is_list.to_string(),
        is_query: is_query.to_string(),
        query_type,
        html_type,
        dict_type: None,
        sort: sort as i32,
        cap_java_field: None,
        super_column: None,
        usable_column: None,
        base_entity: empty_base_entity(),
    }
}

fn to_camel_case(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut capitalize = false;
    for (i, ch) in s.chars().enumerate() {
        if ch == '_' {
            capitalize = true;
        } else if capitalize {
            result.push(ch.to_ascii_uppercase());
            capitalize = false;
        } else if i == 0 {
            result.push(ch.to_ascii_lowercase());
        } else {
            result.push(ch);
        }
    }
    result
}

fn convert_class_name(table_name: &str) -> String {
    let stripped = table_name
        .trim_start_matches("sys_")
        .trim_start_matches("gen_");
    let mut result = String::with_capacity(stripped.len());
    let mut capitalize = true;
    for ch in stripped.chars() {
        if ch == '_' {
            capitalize = true;
        } else if capitalize {
            result.push(ch.to_ascii_uppercase());
            capitalize = false;
        } else {
            result.push(ch);
        }
    }
    result
}

fn get_business_name(table_name: &str) -> String {
    if let Some(pos) = table_name.find('_') {
        to_camel_case(&table_name[pos + 1..])
    } else {
        to_camel_case(table_name)
    }
}

fn get_column_length(column_type: &str) -> i32 {
    if let (Some(start), Some(end)) =
        (column_type.find('('), column_type.find(')'))
    {
        let inner = &column_type[start + 1..end];
        let length_str = inner.split(',').next().unwrap_or(inner).trim();
        length_str.parse::<i32>().unwrap_or(0)
    } else {
        0
    }
}

fn get_column_scale(column_type: &str) -> i32 {
    if let (Some(start), Some(end)) =
        (column_type.find('('), column_type.find(')'))
    {
        let inner = &column_type[start + 1..end];
        if let Some(scale_str) = inner.split(',').nth(1) {
            return scale_str.trim().parse::<i32>().unwrap_or(0);
        }
    }
    0
}
