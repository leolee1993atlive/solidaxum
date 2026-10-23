// async_trait 展开后的方法自带 `#[must_use]`,与返回类型的 must_use 重复,
// 新版 clippy 的 double_must_use 会因此对每个 async trait 方法报错。
#![allow(clippy::double_must_use)]

//! Job scheduler crate.
//!
//! 建表 SQL（供 DBA 参考，不作为迁移文件）：
//!
//! ```sql
//! -- sys_job
//! CREATE TABLE sys_job (
//!     job_id          BIGINT         NOT NULL PRIMARY KEY,
//!     job_name        VARCHAR(64)    NOT NULL COMMENT '任务名称',
//!     job_group       VARCHAR(64)    NOT NULL DEFAULT 'DEFAULT' COMMENT '任务组名',
//!     invoke_target   VARCHAR(500)   NOT NULL COMMENT '调用目标 handler 名称',
//!     cron_expression VARCHAR(255)   NOT NULL COMMENT 'cron 表达式',
//!     job_params      TEXT           COMMENT '任务参数 (JSON)',
//!     misfire_policy  TINYINT        NOT NULL DEFAULT 1 COMMENT '计划执行错误策略 1=默认 2=立即执行 3=放弃执行',
//!     concurrent      TINYINT        NOT NULL DEFAULT 1 COMMENT '是否并发执行 0=否 1=是',
//!     status          CHAR(1)        NOT NULL DEFAULT '0' COMMENT '任务状态 0=暂停 1=运行',
//!     remark          VARCHAR(500)   COMMENT '备注',
//!     create_by       VARCHAR(64),
//!     update_by       VARCHAR(64),
//!     create_time     DATETIME,
//!     update_time     DATETIME,
//!     del_flag        CHAR(1)        NOT NULL DEFAULT '0'
//! ) COMMENT='定时任务调度表';
//!
//! -- sys_job_log
//! CREATE TABLE sys_job_log (
//!     job_log_id    BIGINT       NOT NULL PRIMARY KEY AUTO_INCREMENT,
//!     job_name      VARCHAR(64)  NOT NULL COMMENT '任务名称',
//!     job_group     VARCHAR(64)  NOT NULL COMMENT '任务组名',
//!     invoke_target VARCHAR(500) NOT NULL COMMENT '调用目标 handler 名称',
//!     job_params    TEXT         COMMENT '任务参数',
//!     job_message   VARCHAR(500) COMMENT '日志信息',
//!     status        CHAR(1)      NOT NULL DEFAULT '0' COMMENT '执行状态 0=失败 1=成功',
//!     exception_info TEXT        COMMENT '异常信息',
//!     start_time    DATETIME     COMMENT '执行开始时间',
//!     stop_time     DATETIME     COMMENT '执行结束时间',
//!     create_time   DATETIME     COMMENT '创建时间'
//! ) COMMENT='定时任务调度日志表';
//! ```

pub mod demo_handlers;
pub mod entity;
pub mod handler_trait;
pub mod job;
pub mod joblog;
pub mod policy;
pub mod scheduler;

use std::sync::Arc;

use app_core::infra::Infrastructure;

use crate::scheduler::SchedulerService;

/// service 错误(anyhow 面)→ `AppError` 的统一映射(两 handler 共用)。
///
/// `DataError` 下沉经 `From<DataError> for AppError` 恢复 HTTP 语义
/// (NotFound → 404 / Conflict → 409 / Backend → DbError 本地化);
/// 其余 anyhow 错误一律 `Internal(e.to_string())`。
pub(crate) fn map_service_err(e: anyhow::Error) -> app_core::error::AppError {
    match e.downcast::<app_data::DataError>() {
        Ok(de) => app_core::error::AppError::from(de),
        Err(e) => app_core::error::AppError::Internal(e.to_string()),
    }
}

#[derive(Clone)]
pub struct JobServices {
    pub job_service: crate::job::service::JobService,
    pub job_log_service: crate::joblog::service::JobLogService,
}

impl std::fmt::Debug for JobServices {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JobServices").finish()
    }
}

impl JobServices {
    pub fn new(infra: &Infrastructure) -> Self {
        Self {
            job_service: crate::job::service::JobService::new(infra),
            job_log_service: crate::joblog::service::JobLogService::new(infra),
        }
    }
}

#[derive(Clone)]
pub struct JobState {
    pub services: JobServices,
    pub scheduler: Arc<SchedulerService>,
}

impl std::fmt::Debug for JobState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JobState").finish()
    }
}
