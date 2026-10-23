use std::collections::HashMap;

use app_core::error::AppResult;
use app_data::TxnContext;
use serde_json::Value;

// ─── 办理人选择器 ──────────────────────────────────────────────────────────────

/// 设计器选人弹窗里的一行候选记录。
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HandlerRow {
    pub storage_id: String,
    pub handler_code: String,
    pub handler_name: String,
    pub group_name: Option<String>,
    pub create_time: Option<String>,
}

/// 选人弹窗的完整响应（右侧列表 + 左侧部门树）。
#[derive(Debug, Clone, Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HandlerSelectResult {
    pub rows: Vec<HandlerRow>,
    pub total: u64,
    pub tree_selections: Vec<serde_json::Value>,
}

/// 办理人名称回显项（storageId -> 名称）。
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HandlerFeedback {
    pub storage_id: String,
    pub handler_name: String,
}

/// 办理人候选列表服务 trait，由 admin crate 注入真实实现。
#[async_trait::async_trait]
pub trait HandlerSelectService: Send + Sync {
    /// 查询候选列表。
    /// `handler_type`: "user" | "role" | "dept"
    async fn get_handlers(
        &self,
        handler_type: &str,
        status: Option<&str>,
        page_num: u64,
        page_size: u64,
    ) -> AppResult<HandlerSelectResult>;

    /// 根据 storageId 集合回显办理人名称（设计器节点已选权限的名称展示）。
    /// storageId 形如 `role:1` / `dept:101` / `user:5`。返回顺序与入参一致。
    async fn feedback(
        &self,
        storage_ids: &[String],
    ) -> AppResult<Vec<HandlerFeedback>>;
}

/// 默认空实现。
pub struct NoopHandlerSelect;

#[async_trait::async_trait]
impl HandlerSelectService for NoopHandlerSelect {
    async fn get_handlers(
        &self,
        _handler_type: &str,
        _status: Option<&str>,
        _page_num: u64,
        _page_size: u64,
    ) -> AppResult<HandlerSelectResult> {
        Ok(HandlerSelectResult::default())
    }

    async fn feedback(
        &self,
        _storage_ids: &[String],
    ) -> AppResult<Vec<HandlerFeedback>> {
        Ok(Vec::new())
    }
}

/// 业务展示字段（VO 填充用，无副作用）。
#[derive(Debug, Clone, Default)]
pub struct BusinessInfo {
    pub code: Option<String>,
    pub title: Option<String>,
}

/// 在途 skip 的「当前步」快照:正在被办理、his 行在本事务内**即将写入**
/// 的那一步(PASS 推进;退回/计数读不带)。
///
/// `preStep:{nodeCode}` 匹配 `target_node_code == nodeCode` 的最近 PASS
/// 历史行;在途行的 target 即本次推进的目标节点。解析发生在 skip 事务
/// 内,该行尚未提交,resolver 的独立连接查不到 —— 只能误走 initiator
/// 回退(D14)。引擎把内存里现成的当前步数据带进上下文,resolver 以
/// [`Self::skip_target_node_code`] 命中即用,不再落库。
#[derive(Clone, Debug, Default)]
pub struct CurrentStepContext {
    /// 被办理节点的 node_code(在途 his 行的 node_code)
    #[allow(dead_code)]
    pub node_code: String,
    /// 在途 his 行的 target_node_code(= 本次 PASS 推进的目标节点,
    /// `preStep:{nodeCode}` 的命中键)
    pub skip_target_node_code: String,
    /// 该步运行时任务 id(flow_user.associated 锚点;引擎此时尚未软删办理人行)
    #[allow(dead_code)]
    pub task_id: i64,
    /// 该步 flow_user(Approval)的 processed_by
    pub handlers: Vec<i64>,
    /// 实际操作人(his.approver 的等价物,preStepApper 消费)
    pub approver: Option<i64>,
}

/// 实例级办理人解析上下文：sameStep:/preStep:/initiator 等形态需要
/// 实例历史任务与变量快照才能解析。
#[derive(Clone, Debug, Default)]
pub struct InstanceAssigneeContext {
    pub instance_id: i64,
    /// 正在解析办理人的目标节点编码(preStep 语义锚点)
    pub target_node_code: String,
    pub variable: HashMap<String, Value>,
    /// 在途当前步快照;实例启动/补偿重解析等无在途写路径的场景传 None
    /// (引用的 his 行必已提交,照旧走库查)。
    pub current: Option<CurrentStepContext>,
}

/// 用户解析器：将设计器中的设计符/用户 ID 解析为实际办理人。
#[async_trait::async_trait]
pub trait UserResolver: Send + Sync {
    /// 批量查询用户昵称。
    async fn user_names(&self, ids: &[i64]) -> HashMap<i64, String>;

    /// 解析设计符（如 `role:1`、`dept:101`、`user:5`、`post:3`、`deptrole:101:1`）为实际用户 ID 列表。
    async fn resolve_designators(&self, designators: &[String]) -> Vec<i64>;

    /// 批量查询用户拥有的角色 ID（user_id → role_ids），供网关条件
    /// `hasRole` 预热快照。默认空实现：无角色体系时预热出空角色列表，
    /// hasRole 一律判 false。
    async fn user_role_ids(&self, _user_ids: &[i64]) -> HashMap<i64, Vec<i64>> {
        HashMap::new()
    }

    /// 批量查询用户拥有的角色 key（user_id → role_keys），供网关条件
    /// `hasRoleKeys` 预热快照（roleKey 跨环境通用，角色 ID 各环境不同）。
    async fn user_role_keys(
        &self,
        _user_ids: &[i64],
    ) -> HashMap<i64, Vec<String>> {
        HashMap::new()
    }

    /// 解析实例级设计符(sameStep:/preStep:/preStepApper:/preStepParent:/
    /// initiator/initiatorParent)。默认空实现:无实例语义的环境直接丢弃。
    async fn resolve_instance_designators(
        &self,
        _designators: &[String],
        _ctx: &InstanceAssigneeContext,
    ) -> Vec<i64> {
        Vec::new()
    }
}

/// 默认空实现，用于无需系统用户解析的场景。
pub struct NoopResolver;

#[async_trait::async_trait]
impl UserResolver for NoopResolver {
    async fn user_names(&self, _ids: &[i64]) -> HashMap<i64, String> {
        HashMap::new()
    }

    async fn resolve_designators(&self, _designators: &[String]) -> Vec<i64> {
        Vec::new()
    }
}

/// 业务状态回调节口：流程到达终态时通知业务表更新状态。
///
/// `flow_code` 让实现按流程编码区分业务(对齐 Java warm-flow 的
/// `@EventListener(condition="#flowCode.startsWith('leave')")` 路由):
/// 回调应先判断 flow_code 是否属于本业务,不属于即 `Ok(())` 安静跳过,
/// 避免不同业务互相误写。
#[async_trait::async_trait]
pub trait BusinessStatusCallback: Send + Sync {
    /// 业务终态回写。`txn` 为调用方(流程引擎)的事务上下文:回写与流程操作
    /// 同事务,失败应返回 Err 以回滚整笔操作(原子化)。
    /// flow_code: 流程编码,用于区分业务;business_id: 业务主键;
    /// status: 流程终态，如 "finish"/"reject"/"invalid"。
    async fn on_finished(
        &self,
        txn: &TxnContext<'_>,
        flow_code: &str,
        business_id: &str,
        status: &str,
    ) -> AppResult<()>;

    /// 流程启动(提交申请)时回调:业务表状态应置为 "waiting"(对齐 RuoYi
    /// BusinessStatusEnum.WAITING)。在调用方事务内执行,默认无副作用。
    /// flow_code 用于区分业务:非本业务应安静跳过。
    async fn on_started(
        &self,
        txn: &TxnContext<'_>,
        flow_code: &str,
        business_id: &str,
    ) -> AppResult<()> {
        let _ = (txn, flow_code, business_id);
        Ok(())
    }

    /// 读取业务展示字段（VO 填充用，不得产生副作用）。
    async fn get_business_info(
        &self,
        business_id: &str,
    ) -> AppResult<Option<BusinessInfo>> {
        let _ = business_id;
        Ok(None)
    }
}

/// 默认空实现。
pub struct NoopBusinessCallback;

#[async_trait::async_trait]
impl BusinessStatusCallback for NoopBusinessCallback {
    async fn on_finished(
        &self,
        _txn: &TxnContext<'_>,
        _flow_code: &str,
        _business_id: &str,
        _status: &str,
    ) -> AppResult<()> {
        Ok(())
    }
}
