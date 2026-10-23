//! 流程办理人重解析补偿(outbox)的持久化与接口层(参考实现,
//! 管理端查询端点 D4)。
//!
//! 设计锚:用户/角色/部门变更在**业务事务内**向 `flow_assignee_sync`
//! 写一行(outbox 模式),后台 worker(`admin::assignee_sync_worker`)
//! 消费后对运行中流程实例重解析办理人并 diff 增删 `flow_user` ——
//! 替代旧 Java 系统脆弱的 `@Async` 事件链(事件丢了无人知晓,
//! outbox 行丢了 = 事务本身失败,天然可靠)。
//!
//! **无 Java 对应物**:本表是参考实现自有设计(RuoYi-Vue-Plus 没有
//! 补偿任务表),参照 sys_job_log 的豁免先例 —— 队列数据非行级,
//! DataPolicy 落 CONSTRAINED=false、审计列全 None(时间戳由本模块
//! 手工盖,与 flow 引擎表 A 组裁定同因:`Option<NaiveDateTime>` 与
//! 漏斗盖章签名不匹配)。

pub mod handler;
pub mod model;
pub mod repository;
pub mod route;
pub mod service;
