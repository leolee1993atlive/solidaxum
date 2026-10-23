//! `TaskService::skip` 的分阶段实现。
//!
//! 流转有且只有三个出口,由 [`advance::Advance`] 与 [`multi_approval::try_halt`]
//! 的返回类型显式表达,编译器保证穷尽处理:
//!
//! 1. **会签阈值未达成** —— 写入部分 his(顺签额外轮转到下一办理人),实例原地不动
//! 2. **汇聚网关仍有前驱分支未到达** —— 本分支就此结束,不创建下游任务
//! 3. **正常推进** —— 删旧任务、建新任务与办理人、更新流程图与实例、写 his

pub(crate) mod advance;
pub(crate) mod context;
pub(crate) mod listeners;
pub(crate) mod multi_approval;
pub(crate) mod persist;

use crate::{
    listener::ListenerRegistry,
    resolver::{BusinessStatusCallback, UserResolver},
};

/// `skip` 真正需要的协作者——`TaskService` 六个字段里的三个。
/// 显式列出来,而不是传 `&TaskService`,这样各阶段函数的依赖一目了然。
///
/// 不含 `permission`:权限校验留在 `TaskService::check_permission`,
/// 那里还有另外 6 个调用方。也不含 `db` / `notifier`——`skip` 全程用调用方的事务。
pub(crate) struct SkipDeps<'a> {
    pub registry: &'a ListenerRegistry,
    pub resolver: &'a dyn UserResolver,
    pub business_callback: &'a dyn BusinessStatusCallback,
}
