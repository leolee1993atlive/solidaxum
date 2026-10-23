//! 流程定义服务 —— definition 链经 app-data 漏斗执行。
//!
//! # 事务形状:守卫提升事务外,写半边单闭包
//!
//! 统一形状:**守卫(纯读)提升事务外,写半边单闭包**(save_def / publish /
//! copy_def / save_def_json / delete_def / delete_defs;unpublish 与
//! set_active 本就无事务,单写)。依据 menu_handler 删除级联的先例
//! (「守卫是纯读,写在事务外与写在事务内可观察行为一致」),而不是随意改写:
//!
//! 1. **错误通道**:闭包返回 `Result<T, DataError>`,而 DataError 只有
//!    NotFound/Conflict/Backend/Unsupported 四变体 —— 「流程编码 X 版本 Y
//!    已存在」「已发布的流程定义不能删除」这类带具体文案的业务错误
//!    (AppError::Conflict/BadRequest,文案是前端契约)塞不进去。守卫提升后
//!    业务错误在闭包外以原文案抛出,闭包内只剩 repo 调用的 DataError,
//!    经 `From<DataError>` 得到正确的 HTTP 语义。
//! 2. **可观察行为等价**:READ COMMITTED 是语句级快照 —— SELECT 守卫放在
//!    事务内同样挡不住「检查与写入之间另一事务提交」的并发
//!    (check-then-act 竞态对同事务语句同样存在,只有唯一约束或
//!    SERIALIZABLE 能防,两侧都没有)。守卫移出事务不改变任何结果。
//! 3. **写原子性不变**:每个方法的全部写语句仍在同一闭包内,闭包 Err →
//!    整体回滚。
//!
//! # 0 行语义与软删接管
//!
//! - 漏斗 `update` 的 WHERE = 主键 + 软删存活(豁免实体无行级范围):
//!   **已删定义对写路径不可见** —— 0 行 → `DataError::NotFound` → 404
//!   「数据不存在」(对齐 Java @TableLogic 写路径语义;触发点是「读后被
//!   并发删除」的竞争窗口)。
//! - `soft_delete_where` 的 WHERE 自带存活过滤:重复软删返回 Ok(0) 不再
//!   计数已删行(Java @TableLogic 平价);消费点本就忽略返回值。
//! - 批插走 `insert_many`(单条多值 INSERT,不回读);空集合 no-op。
//!
//! # 审计盖章
//!
//! A 组实体(warm-flow 引擎表)的 create_by/update_by 是 `Option<String>`、
//! 时间列是 `Option<DateTime>`,与漏斗盖章签名(i64/DateTimeUtc)不匹配
//! (policy.rs 双组裁定)→ `OperatorContext` 保留,审计字段由本服务手工盖。
//! `DataContext` 只负责执行面(连接/事务/软删接管)。

use std::collections::{HashMap, HashSet};

use app_core::{
    error::{AppError, AppResult},
    util::snowflake,
};
use app_data::DataContext;
use chrono::Local;

use crate::{
    enums::{ActivityStatus, NodeType, PublishStatus},
    model::{
        convert::{def_json_to_entities, entities_to_def_json},
        def_json::DefJson,
        definition::Definition,
        flow_combine::FlowCombine,
        node::Node,
        skip::Skip,
    },
    repository::{
        definition_repo::DefinitionRepository, node_repo::NodeRepository,
        skip_repo::SkipRepository,
    },
};

/// 操作上下文：审计字段 + 租户
#[derive(Clone, Debug)]
pub struct OperatorContext {
    pub user_id: String,
    pub tenant_id: Option<String>,
}

impl OperatorContext {
    pub fn now(&self) -> chrono::NaiveDateTime {
        Local::now().naive_local()
    }
}

impl OperatorContext {
    pub fn now_static() -> chrono::NaiveDateTime {
        Local::now().naive_local()
    }
}

/// 无状态服务:执行面全部经请求期 `DataContext`(handler 按登录用户构造),
/// 不再持有数据库连接。
#[derive(Clone, Copy, Debug, Default)]
pub struct DefinitionService;

/// save_def_json 闭包内对定义行的三种写法(读取与分支判定已在事务外完成)。
enum DefWrite {
    /// only_node_skip:只换节点/连边,定义行不动
    Keep,
    Update(Definition),
    Insert(Definition),
}

impl DefinitionService {
    pub fn new() -> Self {
        Self
    }

    /// 保存/更新流程定义（事务保证）
    #[tracing::instrument(skip(self, ctx, json, op))]
    pub async fn save_def(
        &self,
        ctx: &DataContext<'_>,
        json: &DefJson,
        op: &OperatorContext,
    ) -> AppResult<Definition> {
        let combine = def_json_to_entities(json);

        // 校验流程合法性(纯函数,原在事务内,提前不改变行为)
        self.validate_definition(&combine)?;

        let now = op.now();

        // 新建分支的版本冲突预检:纯读守卫,事务外(menu 先例,见文件头)。
        if json.id.is_none()
            && DefinitionRepository::select_by_flow_code_and_version(
                ctx,
                &combine.definition.flow_code,
                &combine.definition.version,
            )
            .await?
            .is_some()
        {
            return Err(AppError::Conflict(format!(
                "流程编码 {} 版本 {} 已存在",
                combine.definition.flow_code, combine.definition.version
            )));
        }

        let existing_id = json.id;
        let def_id = existing_id.unwrap_or_else(snowflake::next_id);

        // 定义行写模型(更新分支盖 update_*,新建分支盖全审计)
        let mut def = combine.definition.clone();
        def.id = Some(def_id);
        if existing_id.is_some() {
            def.update_time = Some(now);
            def.update_by = Some(op.user_id.clone());
        } else {
            def.create_time = Some(now);
            def.create_by = Some(op.user_id.clone());
            def.update_time = Some(now);
            def.update_by = Some(op.user_id.clone());
            def.tenant_id = op.tenant_id.clone();
        }

        // 为节点和跳转分配 ID 和 definition_id(owned 值先进闭包:
        // transaction 的 HRTB 不收借用切片)
        let version = combine.definition.version.clone();
        let tenant_id = op.tenant_id.clone();
        let user_id = op.user_id.clone();

        let mut nodes = combine.all_nodes;
        for node in &mut nodes {
            node.id = Some(snowflake::next_id());
            node.definition_id = def_id;
            node.version = version.clone();
            node.create_time = Some(now);
            node.create_by = Some(user_id.clone());
            node.tenant_id = tenant_id.clone();
        }

        // 构建 node_code -> node_id 映射，用于填充 skip.node_id
        let node_code_map: HashMap<&str, i64> = nodes
            .iter()
            .filter_map(|n| n.id.map(|id| (n.node_code.as_str(), id)))
            .collect();

        let mut skips = combine.all_skips;
        for skip in &mut skips {
            skip.id = Some(snowflake::next_id());
            skip.definition_id = def_id;
            skip.node_id =
                node_code_map.get(skip.now_node_code.as_str()).copied();
            skip.create_time = Some(now);
            skip.create_by = Some(user_id.clone());
            skip.tenant_id = tenant_id.clone();
        }

        ctx.transaction(|tx| {
            Box::pin(async move {
                if existing_id.is_some() {
                    // 更新：先软删除旧节点和跳转
                    NodeRepository::soft_delete_by_definition_id(tx, def_id)
                        .await?;
                    SkipRepository::soft_delete_by_definition_id(tx, def_id)
                        .await?;
                    DefinitionRepository::update_by_id_in_txn(tx, &def).await?;
                } else {
                    DefinitionRepository::insert(tx, &def).await.map_err(
                        |e| {
                            tracing::error!(
                                flow_code = %def.flow_code,
                                version = %def.version,
                                def_id = def_id,
                                error = %e,
                                "插入 flow_definition 失败"
                            );
                            e
                        },
                    )?;
                }

                NodeRepository::insert_batch(tx, &nodes).await.map_err(
                    |e| {
                        tracing::error!(
                            node_count = nodes.len(),
                            error = %e,
                            "插入 flow_node 批量失败"
                        );
                        e
                    },
                )?;
                SkipRepository::insert_batch(tx, &skips).await.map_err(
                    |e| {
                        tracing::error!(
                            skip_count = skips.len(),
                            error = %e,
                            "插入 flow_skip 批量失败"
                        );
                        e
                    },
                )?;
                Ok(())
            })
        })
        .await?;

        DefinitionRepository::select_by_id(ctx, def_id)
            .await?
            .ok_or_else(|| AppError::Internal("保存后查询定义失败".to_string()))
    }

    /// 校验流程定义合法性
    fn validate_definition(&self, combine: &FlowCombine) -> AppResult<()> {
        // 此时无节点可校验。节点完整性由发布兜底 —— `publish` 已检查
        // nodes.is_empty()(见本文件 publish 内),放开这里不会让空流程被发布。
        if combine.all_nodes.is_empty() {
            return Ok(());
        }

        // 1. 必须有且仅有一个开始节点
        let start_count = combine
            .all_nodes
            .iter()
            .filter(|n| n.node_type == NodeType::Start as i8)
            .count();
        if start_count == 0 {
            return Err(AppError::BadRequest(
                "流程定义缺少开始节点".to_string(),
            ));
        }
        if start_count > 1 {
            return Err(AppError::BadRequest(
                "流程定义存在多个开始节点".to_string(),
            ));
        }

        // 2. 节点编码唯一
        let mut codes = HashSet::new();
        for node in &combine.all_nodes {
            if !codes.insert(&node.node_code) {
                return Err(AppError::BadRequest(format!(
                    "节点编码重复: {}",
                    node.node_code
                )));
            }
        }

        // 3. 所有跳转目标节点存在
        let node_codes: HashSet<&str> = combine
            .all_nodes
            .iter()
            .map(|n| n.node_code.as_str())
            .collect();
        for skip in &combine.all_skips {
            if !node_codes.contains(skip.next_node_code.as_str()) {
                return Err(AppError::BadRequest(format!(
                    "跳转目标节点不存在: {} -> {}",
                    skip.now_node_code, skip.next_node_code
                )));
            }
        }

        // 4. 结束节点不能有出边
        let end_codes: Vec<&str> = combine
            .all_nodes
            .iter()
            .filter(|n| n.node_type == NodeType::End as i8)
            .map(|n| n.node_code.as_str())
            .collect();
        for skip in &combine.all_skips {
            if end_codes.contains(&skip.now_node_code.as_str()) {
                return Err(AppError::BadRequest(format!(
                    "结束节点不能有出边: {}",
                    skip.now_node_code
                )));
            }
        }

        // 5. 普通节点不能直接有多条 PASS 或多条 REJECT 出边（必须通过网关）
        let gateway_codes: HashSet<&str> = combine
            .all_nodes
            .iter()
            .filter(|n| {
                NodeType::from_i8(n.node_type)
                    .map(|t| t.is_gate_way())
                    .unwrap_or(false)
            })
            .map(|n| n.node_code.as_str())
            .collect();

        let mut pass_count: HashMap<&str, usize> = HashMap::new();
        let mut reject_count: HashMap<&str, usize> = HashMap::new();
        for skip in &combine.all_skips {
            if gateway_codes.contains(skip.now_node_code.as_str()) {
                continue;
            }
            match skip.skip_type.as_deref() {
                Some("PASS") => {
                    *pass_count
                        .entry(skip.now_node_code.as_str())
                        .or_default() += 1
                }
                Some("REJECT") => {
                    *reject_count
                        .entry(skip.now_node_code.as_str())
                        .or_default() += 1
                }
                _ => {}
            }
        }
        for (code, count) in &pass_count {
            if *count > 1 {
                return Err(AppError::BadRequest(format!(
                    "普通节点 {} 有多条 PASS 出边，请使用互斥网关",
                    code
                )));
            }
        }
        for (code, count) in &reject_count {
            if *count > 1 {
                return Err(AppError::BadRequest(format!(
                    "普通节点 {} 有多条 REJECT 出边，请使用互斥网关",
                    code
                )));
            }
        }

        // 6. 互斥网关出边条件唯一性
        for node in &combine.all_nodes {
            if node.node_type != NodeType::Serial as i8 {
                continue;
            }
            let skips: Vec<&Skip> = combine
                .all_skips
                .iter()
                .filter(|s| s.now_node_code == node.node_code)
                .collect();
            let mut conditions = HashSet::new();
            for skip in &skips {
                if let Some(cond) = &skip.skip_condition
                    && !cond.is_empty()
                    && !conditions.insert(cond.as_str())
                {
                    return Err(AppError::BadRequest(format!(
                        "互斥网关 {} 的出边条件重复: {}",
                        node.node_code, cond
                    )));
                }
            }
        }

        Ok(())
    }

    /// 发布流程定义
    #[tracing::instrument(skip(self, ctx, op), fields(definition_id = id))]
    pub async fn publish(
        &self,
        ctx: &DataContext<'_>,
        id: i64,
        op: &OperatorContext,
    ) -> AppResult<()> {
        // 守卫与降级依据全是纯读,事务外读取(见文件头逐块对照)。
        let def = DefinitionRepository::select_by_id(ctx, id)
            .await?
            .ok_or_else(|| {
                AppError::NotFound(format!("流程定义不存在 (id={})", id))
            })?;

        // 检查是否有节点
        let nodes = NodeRepository::select_by_definition_id(ctx, id).await?;
        if nodes.is_empty() {
            return Err(AppError::BadRequest(
                "流程定义中没有节点，无法发布".to_string(),
            ));
        }

        // 将同 flow_code 的其他已发布定义降级
        let def_ids_with_instances =
            DefinitionRepository::select_def_ids_with_instances(
                ctx,
                &def.flow_code,
            )
            .await?;
        let existing = DefinitionRepository::select_published_by_flow_code(
            ctx,
            &def.flow_code,
        )
        .await?;

        let now = op.now();
        let user_id = op.user_id.clone();

        // 降级写模型与发布写模型先算成 owned 值再进闭包(HRTB 不收借用切片)
        let demote_list: Vec<Definition> = existing
            .into_iter()
            .filter(|other| other.id != Some(id))
            .map(|mut updated| {
                updated.is_publish = if updated
                    .id
                    .map(|oid| def_ids_with_instances.contains(&oid))
                    .unwrap_or(false)
                {
                    PublishStatus::Expired as i8
                } else {
                    PublishStatus::Unpublished as i8
                };
                updated.update_time = Some(now);
                updated.update_by = Some(user_id.clone());
                updated
            })
            .collect();

        let mut published_def = def;
        published_def.is_publish = PublishStatus::Published as i8;
        published_def.update_time = Some(now);
        published_def.update_by = Some(user_id);

        ctx.transaction(|tx| {
            Box::pin(async move {
                for other in &demote_list {
                    DefinitionRepository::update_by_id_in_txn(tx, other)
                        .await?;
                }
                DefinitionRepository::update_by_id_in_txn(tx, &published_def)
                    .await?;
                Ok(())
            })
        })
        .await?;
        Ok(())
    }

    /// 取消发布
    #[tracing::instrument(skip(self, ctx, op), fields(definition_id = id))]
    pub async fn unpublish(
        &self,
        ctx: &DataContext<'_>,
        id: i64,
        op: &OperatorContext,
    ) -> AppResult<()> {
        let def = DefinitionRepository::select_by_id(ctx, id)
            .await?
            .ok_or_else(|| {
                AppError::NotFound(format!("流程定义不存在 (id={})", id))
            })?;
        if def.is_publish != PublishStatus::Published as i8 {
            return Err(AppError::BadRequest(
                "只能取消已发布的流程定义".to_string(),
            ));
        }

        let mut updated = def;
        updated.is_publish = PublishStatus::Unpublished as i8;
        updated.update_time = Some(op.now());
        updated.update_by = Some(op.user_id.clone());
        DefinitionRepository::update_by_id(ctx, &updated).await?;
        Ok(())
    }

    /// 复制定义
    #[tracing::instrument(skip(self, ctx, op), fields(definition_id = id))]
    pub async fn copy_def(
        &self,
        ctx: &DataContext<'_>,
        id: i64,
        op: &OperatorContext,
    ) -> AppResult<Definition> {
        let def = DefinitionRepository::select_by_id(ctx, id)
            .await?
            .ok_or_else(|| {
                AppError::NotFound(format!("流程定义不存在 (id={})", id))
            })?;

        let nodes = NodeRepository::select_by_definition_id(ctx, id).await?;
        let skips = SkipRepository::select_by_definition_id(ctx, id).await?;

        let new_version = Self::get_new_version(&def.version);
        let new_id = snowflake::next_id();
        let now = op.now();

        // 创建新定义
        let new_def = Definition {
            id: Some(new_id),
            flow_name: format!("{}(复制)", def.flow_name),
            version: new_version.clone(),
            is_publish: 0,
            create_time: Some(now),
            create_by: Some(op.user_id.clone()),
            update_time: Some(now),
            update_by: Some(op.user_id.clone()),
            tenant_id: op.tenant_id.clone(),
            ..def.clone()
        };

        // 复制节点
        let new_nodes: Vec<Node> = nodes
            .iter()
            .map(|n| Node {
                id: Some(snowflake::next_id()),
                definition_id: new_id,
                version: new_version.clone(),
                create_time: Some(now),
                create_by: Some(op.user_id.clone()),
                update_time: Some(now),
                update_by: Some(op.user_id.clone()),
                tenant_id: op.tenant_id.clone(),
                ..n.clone()
            })
            .collect();

        // 构建 node_code -> 新 node_id 映射，重映射 skip.node_id
        let node_code_map: HashMap<&str, i64> = new_nodes
            .iter()
            .filter_map(|n| n.id.map(|id| (n.node_code.as_str(), id)))
            .collect();

        let new_skips: Vec<Skip> = skips
            .iter()
            .map(|s| Skip {
                id: Some(snowflake::next_id()),
                definition_id: new_id,
                node_id: node_code_map.get(s.now_node_code.as_str()).copied(),
                create_time: Some(now),
                create_by: Some(op.user_id.clone()),
                update_time: Some(now),
                update_by: Some(op.user_id.clone()),
                tenant_id: op.tenant_id.clone(),
                ..s.clone()
            })
            .collect();

        // 新定义经闭包返回值带回(事务提交才算复制成功)
        let new_def = ctx
            .transaction(|tx| {
                Box::pin(async move {
                    DefinitionRepository::insert(tx, &new_def).await?;
                    NodeRepository::insert_batch(tx, &new_nodes).await?;
                    SkipRepository::insert_batch(tx, &new_skips).await?;
                    Ok(new_def)
                })
            })
            .await?;
        Ok(new_def)
    }

    /// 获取新版本号
    fn get_new_version(version: &str) -> String {
        if let Ok(n) = version.parse::<u32>() {
            (n + 1).to_string()
        } else {
            format!("{}_1", version)
        }
    }

    /// 激活/挂起
    #[tracing::instrument(skip(self, ctx, op), fields(definition_id = id))]
    pub async fn set_active(
        &self,
        ctx: &DataContext<'_>,
        id: i64,
        active: bool,
        op: &OperatorContext,
    ) -> AppResult<()> {
        let mut def = DefinitionRepository::select_by_id(ctx, id)
            .await?
            .ok_or_else(|| {
                AppError::NotFound(format!("流程定义不存在 (id={})", id))
            })?;
        def.activity_status = if active {
            ActivityStatus::Active as i8
        } else {
            ActivityStatus::Suspended as i8
        };
        def.update_time = Some(op.now());
        def.update_by = Some(op.user_id.clone());
        DefinitionRepository::update_by_id(ctx, &def).await?;
        Ok(())
    }

    /// 导出 JSON
    #[tracing::instrument(skip(self, ctx), fields(definition_id = id))]
    pub async fn export_json(
        &self,
        ctx: &DataContext<'_>,
        id: i64,
    ) -> AppResult<DefJson> {
        let def = DefinitionRepository::select_by_id(ctx, id)
            .await?
            .ok_or_else(|| {
                AppError::NotFound(format!("流程定义不存在 (id={})", id))
            })?;
        let nodes = NodeRepository::select_by_definition_id(ctx, id).await?;
        let skips = SkipRepository::select_by_definition_id(ctx, id).await?;
        Ok(entities_to_def_json(&def, &nodes, &skips))
    }

    /// 根据定义 ID 返回 DefJson（供设计器加载）
    pub async fn get_def_json(
        &self,
        ctx: &DataContext<'_>,
        id: i64,
    ) -> AppResult<DefJson> {
        self.export_json(ctx, id).await
    }

    /// 供设计器 save-json API 使用：保存或更新 DefJson
    /// only_node_skip: 仅更新节点和条件，不更新定义基础信息
    #[tracing::instrument(skip(self, ctx, def_json))]
    pub async fn save_def_json(
        &self,
        ctx: &DataContext<'_>,
        def_json: DefJson,
        only_node_skip: bool,
    ) -> AppResult<()> {
        let mut combine = def_json_to_entities(&def_json);

        // 既有定义查询与分支判定:纯读,事务外(见文件头逐块对照)。
        let existing =
            DefinitionRepository::select_by_flow_code(ctx, &def_json.flow_code)
                .await?;

        let (def_write, def_id) = if only_node_skip {
            let existing = existing.ok_or_else(|| {
                AppError::NotFound(format!(
                    "流程定义不存在: {}",
                    def_json.flow_code
                ))
            })?;
            (
                DefWrite::Keep,
                existing.id.expect("id 由雪花生成或查询回填,必有值"),
            )
        } else {
            match existing {
                Some(existing) => {
                    let mut updated = combine.definition.clone();
                    updated.id = existing.id;
                    updated.create_time = existing.create_time;
                    updated.create_by = existing.create_by;
                    updated.update_time = Some(OperatorContext::now_static());
                    (
                        DefWrite::Update(updated),
                        existing.id.expect("id 由雪花生成或查询回填,必有值"),
                    )
                }
                None => {
                    let new_id = snowflake::next_id();
                    let mut new_def = combine.definition.clone();
                    new_def.id = Some(new_id);
                    let now = OperatorContext::now_static();
                    new_def.create_time = Some(now);
                    new_def.update_time = Some(now);
                    (DefWrite::Insert(new_def), new_id)
                }
            }
        };

        for node in &mut combine.all_nodes {
            node.id = Some(snowflake::next_id());
            node.definition_id = def_id;
        }
        // 构建 node_code -> node_id 映射，用于填充 skip.node_id
        let node_code_map: HashMap<&str, i64> = combine
            .all_nodes
            .iter()
            .filter_map(|n| n.id.map(|id| (n.node_code.as_str(), id)))
            .collect();
        for skip in &mut combine.all_skips {
            skip.id = Some(snowflake::next_id());
            skip.definition_id = def_id;
            skip.node_id =
                node_code_map.get(skip.now_node_code.as_str()).copied();
        }

        ctx.transaction(|tx| {
            Box::pin(async move {
                match &def_write {
                    DefWrite::Keep => {}
                    DefWrite::Update(def) => {
                        DefinitionRepository::update_by_id_in_txn(tx, def)
                            .await?;
                    }
                    DefWrite::Insert(def) => {
                        DefinitionRepository::insert(tx, def).await?;
                    }
                }

                // 删除旧节点和条件，重新插入 —— Java `remove(...)` 在
                // @TableLogic 实体上是逻辑删(DefServiceImpl.saveDef:132-133),
                // 此处对齐为软删(见 node_repo 头注)。
                NodeRepository::soft_delete_by_definition_id(tx, def_id)
                    .await?;
                SkipRepository::soft_delete_by_definition_id(tx, def_id)
                    .await?;

                NodeRepository::insert_batch(tx, &combine.all_nodes).await?;
                SkipRepository::insert_batch(tx, &combine.all_skips).await?;
                Ok(())
            })
        })
        .await?;
        Ok(())
    }

    /// 查询定义列表分页
    #[tracing::instrument(skip(self, ctx))]
    pub async fn list_page(
        &self,
        ctx: &DataContext<'_>,
        flow_code: Option<&str>,
        flow_name: Option<&str>,
        category: Option<&str>,
        page_no: u64,
        page_size: u64,
    ) -> AppResult<app_data::Page<Definition>> {
        let result = DefinitionRepository::select_page(
            ctx, flow_code, flow_name, category, page_no, page_size,
        )
        .await?;
        Ok(result)
    }

    /// 查询未发布定义列表分页
    #[tracing::instrument(skip(self, ctx))]
    pub async fn unpublished_list_page(
        &self,
        ctx: &DataContext<'_>,
        flow_code: Option<&str>,
        flow_name: Option<&str>,
        category: Option<&str>,
        page_no: u64,
        page_size: u64,
    ) -> AppResult<app_data::Page<Definition>> {
        let result = DefinitionRepository::select_unpublished_page(
            ctx, flow_code, flow_name, category, page_no, page_size,
        )
        .await?;
        Ok(result)
    }

    /// 查询单个定义
    pub async fn get_by_id(
        &self,
        ctx: &DataContext<'_>,
        id: i64,
    ) -> AppResult<Option<Definition>> {
        Ok(DefinitionRepository::select_by_id(ctx, id).await?)
    }

    /// 节点配置清单(nodeConf 列表查询,对齐参考实现
    /// FlowNodeConfServiceImpl.listNodes → `NodeService.getByDefId`)。
    pub async fn node_conf_nodes(
        &self,
        ctx: &DataContext<'_>,
        definition_id: i64,
    ) -> AppResult<Vec<Node>> {
        Ok(NodeRepository::select_by_definition_id(ctx, definition_id).await?)
    }

    /// 连线配置清单(nodeConf 列表查询,对齐
    /// FlowNodeConfServiceImpl.listSkips → `SkipService.getByDefId`)。
    pub async fn node_conf_skips(
        &self,
        ctx: &DataContext<'_>,
        definition_id: i64,
    ) -> AppResult<Vec<Skip>> {
        Ok(SkipRepository::select_by_definition_id(ctx, definition_id).await?)
    }

    /// 单点更新节点办理人权限标识(对齐参考实现
    /// FlowNodeConfServiceImpl.updatePermissionFlag):先存在性校验
    /// (不存在 → 404),再显式 set 单字段,支持置空。
    pub async fn update_permission_flag(
        &self,
        ctx: &DataContext<'_>,
        definition_id: i64,
        node_code: &str,
        permission_flag: Option<String>,
    ) -> AppResult<()> {
        if NodeRepository::select_by_definition_id_and_node_code(
            ctx,
            definition_id,
            node_code,
        )
        .await?
        .is_none()
        {
            return Err(AppError::NotFound(format!("节点不存在: {node_code}")));
        }
        NodeRepository::update_permission_flag(
            ctx,
            definition_id,
            node_code,
            permission_flag,
        )
        .await?;
        Ok(())
    }

    /// 单点更新连线跳转条件(对齐参考实现
    /// FlowNodeConfServiceImpl.updateSkipCondition):先按 id 存在性校验
    /// (不存在 → 404),再显式 set,置空 = 无条件通过。
    pub async fn update_skip_condition(
        &self,
        ctx: &DataContext<'_>,
        skip_id: i64,
        skip_condition: Option<String>,
    ) -> AppResult<()> {
        if SkipRepository::select_by_id(ctx, skip_id).await?.is_none() {
            return Err(AppError::NotFound(format!("连线不存在: {skip_id}")));
        }
        SkipRepository::update_skip_condition(ctx, skip_id, skip_condition)
            .await?;
        Ok(())
    }

    /// 导入 JSON 定义
    #[tracing::instrument(skip(self, ctx, json, op))]
    pub async fn import_def_json(
        &self,
        ctx: &DataContext<'_>,
        json: &DefJson,
        op: &OperatorContext,
    ) -> AppResult<Definition> {
        // 导入时视为新增,强制清空 ID
        let mut json = json.clone();
        json.id = None;
        self.save_def(ctx, &json, op).await
    }

    /// 根据上传文件名和内容导入定义
    #[tracing::instrument(skip(self, ctx, data, op))]
    pub async fn import_def(
        &self,
        ctx: &DataContext<'_>,
        original_name: &str,
        data: &[u8],
        op: &OperatorContext,
    ) -> AppResult<()> {
        let ext = original_name
            .rsplit_once('.')
            .map(|(_, s)| s.to_ascii_lowercase())
            .unwrap_or_default();

        match ext.as_ref() {
            "json" => {
                let def_json: DefJson =
                    serde_json::from_slice(data).map_err(|e| {
                        AppError::BadRequest(format!("JSON 解析失败: {}", e))
                    })?;
                self.import_def_json(ctx, &def_json, op).await?;
                Ok(())
            }
            "xml" | "zip" => {
                Err(AppError::BadRequest(format!("{} 导入暂未实现", ext)))
            }
            _ => {
                Err(AppError::BadRequest(format!("不支持的文件格式: {}", ext)))
            }
        }
    }

    /// 根据定义 ID 返回 JSON 字符串
    pub async fn export_json_string(
        &self,
        ctx: &DataContext<'_>,
        id: i64,
    ) -> AppResult<String> {
        let json = self.export_json(ctx, id).await?;
        serde_json::to_string(&json)
            .map_err(|e| AppError::Internal(format!("序列化定义失败: {}", e)))
    }

    /// 软删除定义（含节点和跳转）
    #[tracing::instrument(skip(self, ctx, op), fields(definition_id = id))]
    pub async fn delete_def(
        &self,
        ctx: &DataContext<'_>,
        id: i64,
        op: &OperatorContext,
    ) -> AppResult<()> {
        // 存在性与发布态守卫:纯读,事务外(见文件头逐块对照)。
        let def = DefinitionRepository::select_by_id(ctx, id)
            .await?
            .ok_or_else(|| {
                AppError::NotFound(format!("流程定义不存在 (id={})", id))
            })?;
        if def.is_publish == PublishStatus::Published as i8 {
            return Err(AppError::BadRequest(
                "已发布的流程定义不能删除".to_string(),
            ));
        }

        let mut def = def;
        def.del_flag = Some("1".to_string());
        def.update_time = Some(op.now());
        def.update_by = Some(op.user_id.clone());

        ctx.transaction(|tx| {
            Box::pin(async move {
                NodeRepository::soft_delete_by_definition_id(tx, id).await?;
                SkipRepository::soft_delete_by_definition_id(tx, id).await?;
                DefinitionRepository::update_by_id_in_txn(tx, &def).await?;
                Ok(())
            })
        })
        .await?;
        Ok(())
    }

    /// 批量软删除定义（单事务原子操作）
    #[tracing::instrument(skip(self, ctx, op))]
    pub async fn delete_defs(
        &self,
        ctx: &DataContext<'_>,
        ids: &[i64],
        op: &OperatorContext,
    ) -> AppResult<()> {
        // 逐 id 预检(存在性 + 发布态):纯读守卫,事务外;任一不符即拒绝,
        // 文案与原同事务版逐点一致(见文件头逐块对照)。
        let mut defs: Vec<Definition> = Vec::with_capacity(ids.len());
        for id in ids {
            let def = DefinitionRepository::select_by_id(ctx, *id)
                .await?
                .ok_or_else(|| {
                    AppError::NotFound(format!("流程定义不存在 (id={})", id))
                })?;
            if def.is_publish == PublishStatus::Published as i8 {
                return Err(AppError::BadRequest(
                    "已发布的流程定义不能删除".to_string(),
                ));
            }
            defs.push(def);
        }

        let now = op.now();
        let user_id = op.user_id.clone();
        let defs: Vec<Definition> = defs
            .into_iter()
            .map(|mut def| {
                def.del_flag = Some("1".to_string());
                def.update_time = Some(now);
                def.update_by = Some(user_id.clone());
                def
            })
            .collect();

        ctx.transaction(|tx| {
            Box::pin(async move {
                for def in &defs {
                    let id = def.id.expect("id 由雪花生成或查询回填,必有值");
                    NodeRepository::soft_delete_by_definition_id(tx, id)
                        .await?;
                    SkipRepository::soft_delete_by_definition_id(tx, id)
                        .await?;
                    DefinitionRepository::update_by_id_in_txn(tx, def).await?;
                }
                Ok(())
            })
        })
        .await?;
        Ok(())
    }
}
