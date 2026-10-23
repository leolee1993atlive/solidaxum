use std::sync::Arc;

use app_core::middleware::extractors::CurrentUser;

use crate::{
    listener::ListenerRegistry,
    notifier::{FlowNotifier, NoopFlowNotifier},
    permission::{DefaultPermissionHandler, PermissionHandler},
    resolver::{
        BusinessStatusCallback, HandlerSelectService, NoopBusinessCallback,
        NoopHandlerSelect, NoopResolver, UserResolver,
    },
    service::{
        category_service::CategoryService,
        definition_service::{DefinitionService, OperatorContext},
        instance_service::InstanceService,
        leave_service::LeaveService,
        spel_service::SpelService,
        task_service::TaskService,
        variable_service::VariableService,
    },
};

pub mod category_handler;
pub mod chart_handler;
pub mod definition_handler;
pub mod designer_handler;
pub mod instance_handler;
pub mod leave_handler;
pub mod node_conf_handler;
pub mod spel_handler;
pub mod task_handler;
pub mod variable_handler;
pub mod warm_flow_ui_handler;

/// flow crate 独立状态（不依赖 system::AppState）
#[derive(Clone)]
pub struct FlowState {
    /// 装配层注入的 DataSource(admin 经 `infra.data_source()` 传入,
    /// handler 请求期建 `DataContext`)。flow 11 实体全豁免(policy.rs),
    /// 读侧全走 `without_row_scope()` 不解析范围,无 resolver 的
    /// fail-closed 不触发。
    pub ds: Arc<app_data::DataSource>,
    pub def_service: DefinitionService,
    pub ins_service: InstanceService,
    pub task_service: TaskService,
    pub category_service: CategoryService,
    pub spel_service: SpelService,
    pub variable_service: VariableService,
    pub leave_service: LeaveService,
    pub listener_registry: Arc<ListenerRegistry>,
    pub resolver: Arc<dyn UserResolver>,
    pub business_callback: Arc<dyn BusinessStatusCallback>,
    pub notifier: Arc<dyn FlowNotifier>,
    pub handler_select: Arc<dyn HandlerSelectService>,
}

impl FlowState {
    pub fn new(ds: Arc<app_data::DataSource>) -> Self {
        let registry = Arc::new(ListenerRegistry::new());
        let permission: Arc<dyn PermissionHandler> =
            Arc::new(DefaultPermissionHandler);
        let resolver: Arc<dyn UserResolver> = Arc::new(NoopResolver);
        let business_callback: Arc<dyn BusinessStatusCallback> =
            Arc::new(NoopBusinessCallback);
        Self::with_resolver_callback_and_notifier(
            ds,
            registry,
            permission,
            resolver,
            business_callback,
            Arc::new(NoopFlowNotifier),
        )
    }

    pub fn with_registry_and_permission(
        ds: Arc<app_data::DataSource>,
        registry: Arc<ListenerRegistry>,
        permission: Arc<dyn PermissionHandler>,
    ) -> Self {
        let resolver: Arc<dyn UserResolver> = Arc::new(NoopResolver);
        let business_callback: Arc<dyn BusinessStatusCallback> =
            Arc::new(NoopBusinessCallback);
        Self::with_resolver_callback_and_notifier(
            ds,
            registry,
            permission,
            resolver,
            business_callback,
            Arc::new(NoopFlowNotifier),
        )
    }

    pub fn with_resolver_and_callback(
        ds: Arc<app_data::DataSource>,
        registry: Arc<ListenerRegistry>,
        permission: Arc<dyn PermissionHandler>,
        resolver: Arc<dyn UserResolver>,
        business_callback: Arc<dyn BusinessStatusCallback>,
    ) -> Self {
        Self::with_resolver_callback_and_notifier(
            ds,
            registry,
            permission,
            resolver,
            business_callback,
            Arc::new(NoopFlowNotifier),
        )
    }

    /// 装配入口:执行面经请求期 `DataContext`(handler 按登录用户构造)。
    pub fn with_resolver_callback_and_notifier(
        ds: Arc<app_data::DataSource>,
        registry: Arc<ListenerRegistry>,
        permission: Arc<dyn PermissionHandler>,
        resolver: Arc<dyn UserResolver>,
        business_callback: Arc<dyn BusinessStatusCallback>,
        notifier: Arc<dyn FlowNotifier>,
    ) -> Self {
        let ins_service = InstanceService::new(
            registry.clone(),
            permission.clone(),
            resolver.clone(),
            business_callback.clone(),
            notifier.clone(),
        );
        Self {
            ds,
            def_service: DefinitionService::new(),
            ins_service: ins_service.clone(),
            task_service: TaskService::new(
                registry.clone(),
                permission.clone(),
                resolver.clone(),
                business_callback.clone(),
                notifier.clone(),
            ),
            category_service: CategoryService::new(),
            spel_service: SpelService::new(),
            variable_service: VariableService::new(),
            leave_service: LeaveService::new(ins_service),
            listener_registry: registry,
            resolver,
            business_callback,
            notifier,
            handler_select: Arc::new(NoopHandlerSelect),
        }
    }
}

/// 从 CurrentUser 构建 OperatorContext
pub(crate) fn to_ctx(user: &CurrentUser) -> OperatorContext {
    OperatorContext {
        user_id: user.get_user_id().to_string(),
        tenant_id: None, // P2: 从 LoginUser 获取租户 ID
    }
}

/// 请求期数据上下文(handler 驱动,登录用户进 Principal)。
///
/// flow 11 实体全豁免(crate::policy):读侧全走 `without_row_scope()`,
/// 写侧不受行级范围 —— 即使装配的 DataSource 带 resolver 也不触发解析
/// (只有真的解析 ScopeFilter 才会,而豁免实体永远不解析)。
pub(crate) fn user_ctx<'a>(
    state: &'a FlowState,
    user: &CurrentUser,
) -> app_data::DataContext<'a> {
    state.ds.context(app_data::Principal::from(&user.0))
}
