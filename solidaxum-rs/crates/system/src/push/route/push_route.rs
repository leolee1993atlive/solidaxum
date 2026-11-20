use std::sync::Arc;

use axum::{
    Router,
    routing::{get, post, put},
};

use crate::{
    app::AppState,
    push::handler::{
        channel_config_handler as ch, message_log_handler as log,
    },
    push::sse::sse_handler,
};

pub fn push_route() -> Router<Arc<AppState>> {
    Router::new()
        .route("/resource/message", get(sse_handler))
        // 渠道配置(钉钉/企微/飞书;短信/APP 按院方定制,不在此维护)
        .route("/system/messageChannel/list", get(ch::list_channels))
        .route("/system/messageChannel/{channel}", put(ch::save_channel))
        .route(
            "/system/messageChannel/{channel}/test",
            post(ch::test_channel),
        )
        // 发送历史(全局审计流水,只读)
        .route("/system/messageLog/list", get(log::list_send_logs))
}
