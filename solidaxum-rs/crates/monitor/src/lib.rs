pub mod cache;
pub mod logininfo;
pub mod middleware;
pub mod online;
pub mod operlog;
pub mod policy;

use app_core::infra::Infrastructure;

#[derive(Clone)]
pub struct MonitorServices {
    pub oper_log_service: crate::operlog::service::OperLogService,
    pub login_info_service: crate::logininfo::service::LoginInfoService,
    pub online_user_service: crate::online::service::OnlineUserService,
}

impl std::fmt::Debug for MonitorServices {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MonitorServices").finish()
    }
}

impl MonitorServices {
    pub fn new(infra: &Infrastructure) -> Self {
        Self {
            oper_log_service: crate::operlog::service::OperLogService::new(
                infra,
            ),
            login_info_service:
                crate::logininfo::service::LoginInfoService::new(infra),
            online_user_service: crate::online::service::OnlineUserService::new(
                infra,
            ),
        }
    }
}

pub struct OperLogState {
    pub services: MonitorServices,
    /// handler 请求期建 ctx 的 DataSource(装配后与 infra.data_source()
    /// 同一 Arc,带 resolver;全豁免读不解析 resolver,降级快照无影响)。
    pub ds: std::sync::Arc<app_data::DataSource>,
    /// 下游产品注册的补充路径名规则。基座自身留空。
    pub title_rules: crate::middleware::oper_log::TitleRules,
}

impl std::fmt::Debug for OperLogState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OperLogState")
            .field("services", &self.services)
            .finish()
    }
}
