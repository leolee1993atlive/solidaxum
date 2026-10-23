use std::sync::Arc;

use axum::{
    Router,
    extract::DefaultBodyLimit,
    routing::{delete, get, post, put},
};

use crate::{
    app::AppState,
    oss::handler::oss_handler::{
        change_oss_config_status, create_oss_config, delete_oss,
        delete_oss_config, download_oss, get_oss_config_by_id, list_oss,
        list_oss_by_ids, list_oss_config, update_oss_config, upload_oss,
    },
};

/// # 参数
/// - `upload_body_limit`: 上传单文件大小上限(字节),来自 `config.oss.upload_body_limit`。
///   仅作用于上传接口(axum 默认 DefaultBodyLimit 为 2MB,不足以承载图片/文件上传)。
///   对齐 RuoYi `spring.servlet.multipart.max-file-size`。
pub fn oss_route(upload_body_limit: usize) -> Router<Arc<AppState>> {
    Router::new()
        .route("/resource/oss/list", get(list_oss))
        .route("/resource/oss/listByIds/{ossIds}", get(list_oss_by_ids))
        // 单独放宽 body 上限: 上传接口按配置,其余 OSS 接口仍是默认 2MB。
        .route(
            "/resource/oss/upload",
            post(upload_oss).layer(DefaultBodyLimit::max(upload_body_limit)),
        )
        .route("/resource/oss/download/{ossId}", get(download_oss))
        .route("/resource/oss/{ossIds}", delete(delete_oss))
        .route("/resource/oss/config/list", get(list_oss_config))
        .route(
            "/resource/oss/config",
            post(create_oss_config).put(update_oss_config),
        )
        .route(
            "/resource/oss/config/changeStatus",
            put(change_oss_config_status),
        )
        // GET 详情与 DELETE 批量共用同一路径段(axum 0.8 不允许同位置不同
        // 参数名的两个捕获,故合并;Path 提取按位置而非名字)。
        .route(
            "/resource/oss/config/{ossConfigId}",
            get(get_oss_config_by_id).delete(delete_oss_config),
        )
}

/// local 存储静态直链: 公开访问, 挂在鉴权层之外(参照 designer_public_route)。
pub fn oss_public_route() -> Router<Arc<AppState>> {
    Router::new().route(
        "/oss/file/{config_key}/{*key}",
        get(crate::oss::handler::local_file_handler::serve_local_file),
    )
}
