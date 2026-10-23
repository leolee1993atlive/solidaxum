//! local 本地存储契约测试: 上传 → 静态直链 → 鉴权下载 → 删除 全链路,
//! 以及同名文件共存、危险类型 attachment、路径穿越防护。

#![cfg(feature = "test-util")]

use admin::testkit::{TestApp, api_uri, assert_ok, code, user_with_perms};
use app_core::model::login_user::LoginUser;
use axum::{
    body::Body,
    http::{HeaderMap, Request, StatusCode, header},
};
use base64::{Engine, engine::general_purpose::STANDARD as BASE64};
use http_body_util::BodyExt;
use serde_json::Value;
use tower::ServiceExt;

fn temp_root(tag: &str) -> std::path::PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("oss-local-it-{tag}-{nanos}"));
    std::fs::create_dir_all(&dir).expect("创建临时目录失败");
    dir
}

/// 插入 local 配置(status='Y' 成为默认) + 一个 S3 配置(验证非 local 404)。
async fn seed_oss_configs(app: &TestApp, root: &std::path::Path) {
    let endpoint =
        format!("file://{}", root.display().to_string().replace('\\', "/"));
    let ds = app.state.infra.data_source();
    app_data::testkit::execute_raw(
        &ds,
        &format!(
            "INSERT INTO sys_oss_config (oss_config_id, config_key, access_key, secret_key, bucket_name, prefix, endpoint, domain_url, is_https, region, status, access_policy, create_by) VALUES \
             (9901, 'local', '', '', 'default', '', '{endpoint}', '', 'N', '', 'Y', '1', 1), \
             (9902, 'miniocfg', 'x', 'x', 'b', '', '127.0.0.1:9000', '', 'N', '', 'N', '1', 1)"
        ),
    )
    .await
    .expect("插入 OSS 配置失败");
}

/// multipart 上传(对照 contract_system_test.rs 用户导入的构造方式)。
async fn upload(
    app: &TestApp,
    user: &LoginUser,
    file_name: &str,
    content_type: &str,
    data: &[u8],
) -> (StatusCode, Value) {
    let boundary = "----oss_local_test_boundary";
    let mut body = Vec::new();
    body.extend_from_slice(
        format!(
            "--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"{file_name}\"\r\nContent-Type: {content_type}\r\n\r\n"
        )
        .as_bytes(),
    );
    body.extend_from_slice(data);
    body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());
    let payload = serde_json::to_vec(user).expect("序列化 LoginUser 失败");
    let req = Request::builder()
        .method("POST")
        .uri(api_uri("/resource/oss/upload"))
        .header("x-test-login-user", BASE64.encode(&payload))
        .header(
            header::CONTENT_TYPE,
            format!("multipart/form-data; boundary={boundary}"),
        )
        .body(Body::from(body))
        .expect("构建请求失败");
    let resp = app.router.clone().oneshot(req).await.expect("路由处理失败");
    let status = resp.status();
    let bytes = resp
        .into_body()
        .collect()
        .await
        .expect("读取响应体失败")
        .to_bytes();
    let v: Value = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap_or(Value::Null)
    };
    (status, v)
}

/// 不带鉴权的公开 GET(静态直链场景)。
async fn get_public(
    app: &TestApp,
    uri: &str,
) -> (StatusCode, HeaderMap, Vec<u8>) {
    let req = Request::builder()
        .method("GET")
        .uri(api_uri(uri))
        .body(Body::empty())
        .expect("构建请求失败");
    let resp = app.router.clone().oneshot(req).await.expect("路由处理失败");
    let status = resp.status();
    let headers = resp.headers().clone();
    let bytes = resp
        .into_body()
        .collect()
        .await
        .expect("读取响应体失败")
        .to_bytes()
        .to_vec();
    (status, headers, bytes)
}

/// 带鉴权的裸 GET(下载场景, 需要响应头而非 JSON 信封)。
async fn get_authed(
    app: &TestApp,
    uri: &str,
    user: &LoginUser,
) -> (StatusCode, HeaderMap, Vec<u8>) {
    let payload = serde_json::to_vec(user).expect("序列化 LoginUser 失败");
    let req = Request::builder()
        .method("GET")
        .uri(api_uri(uri))
        .header("x-test-login-user", BASE64.encode(&payload))
        .body(Body::empty())
        .expect("构建请求失败");
    let resp = app.router.clone().oneshot(req).await.expect("路由处理失败");
    let status = resp.status();
    let headers = resp.headers().clone();
    let bytes = resp
        .into_body()
        .collect()
        .await
        .expect("读取响应体失败")
        .to_bytes()
        .to_vec();
    (status, headers, bytes)
}

#[tokio::test]
async fn local_upload_serve_download_delete_full_flow() {
    let app = TestApp::new().await;
    let root = temp_root("flow");
    seed_oss_configs(&app, &root).await;
    let uploader = user_with_perms(&["system:oss:upload"]);
    let png = b"fake-png-bytes-123";

    // 1. 上传 → url 为 /api/oss/file/local/<uuid>.png, originalName 保留
    let (status, body) =
        upload(&app, &uploader, "report-final.png", "image/png", png).await;
    assert_ok(status, &body);
    let url = body
        .pointer("/data/url")
        .and_then(Value::as_str)
        .expect("url");
    assert!(url.starts_with("/api/oss/file/local/"), "url 形态: {url}");
    assert!(url.ends_with(".png"), "url 后缀: {url}");
    assert_eq!(
        body.pointer("/data/originalName").and_then(Value::as_str),
        Some("report-final.png")
    );
    // ossId 经响应信封 stringify_ids 字符串化(is_id_key 命中 "Id" 后缀,
    // 且雪花 ID 超出 JS 安全整数), 故用 as_str; 对照 contract_system_test
    // 的 as_id() helper 同源。
    let oss_id = body
        .pointer("/data/ossId")
        .and_then(Value::as_str)
        .expect("ossId");

    // 2. 静态直链: 不带鉴权 200, content-type 正确, inline(无 attachment), 长缓存
    let (status, headers, bytes) = get_public(&app, url).await;
    assert_eq!(status, StatusCode::OK, "静态直链应 200");
    assert_eq!(
        headers
            .get(header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok()),
        Some("image/png")
    );
    assert!(
        headers.get(header::CONTENT_DISPOSITION).is_none(),
        "图片应 inline"
    );
    assert_eq!(
        headers
            .get(header::X_CONTENT_TYPE_OPTIONS)
            .and_then(|v| v.to_str().ok()),
        Some("nosniff")
    );
    assert_eq!(
        headers
            .get(header::CACHE_CONTROL)
            .and_then(|v| v.to_str().ok()),
        Some("public, max-age=31536000, immutable")
    );
    assert_eq!(bytes, png);

    // 3. 鉴权下载: Content-Disposition 用 original_name
    let downloader = user_with_perms(&["system:oss:download"]);
    let (status, headers, bytes) = get_authed(
        &app,
        &format!("/resource/oss/download/{oss_id}"),
        &downloader,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let disposition = headers
        .get(header::CONTENT_DISPOSITION)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert!(
        disposition.contains("report-final.png"),
        "下载文件名: {disposition}"
    );
    assert_eq!(bytes, png);

    // 4. 删除 → 静态直链 404, 物理文件已删
    let file_path = root.join("default").join(url.rsplit('/').next().unwrap());
    assert!(
        file_path.exists(),
        "删除前文件应存在: {}",
        file_path.display()
    );
    let remover = user_with_perms(&["system:oss:remove"]);
    let (status, body) = app
        .delete(&format!("/resource/oss/{oss_id}"), Some(&remover))
        .await;
    assert_ok(status, &body);
    assert!(!file_path.exists(), "删除后物理文件应移除");
    let (status, _, _) = get_public(&app, url).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn local_static_route_forces_attachment_for_html() {
    let app = TestApp::new().await;
    let root = temp_root("xss");
    seed_oss_configs(&app, &root).await;
    let uploader = user_with_perms(&["system:oss:upload"]);
    let (status, body) = upload(
        &app,
        &uploader,
        "page.html",
        "text/html",
        b"<script>alert(1)</script>",
    )
    .await;
    assert_ok(status, &body);
    let url = body
        .pointer("/data/url")
        .and_then(Value::as_str)
        .expect("url");

    let (status, headers, _) = get_public(&app, url).await;
    assert_eq!(status, StatusCode::OK);
    let disposition = headers
        .get(header::CONTENT_DISPOSITION)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert!(
        disposition.starts_with("attachment"),
        "html 应强制下载: {disposition}"
    );
}

#[tokio::test]
async fn local_static_route_rejects_traversal_and_non_local_config() {
    let app = TestApp::new().await;
    let root = temp_root("guard");
    seed_oss_configs(&app, &root).await;

    // 路径穿越(%2e%2e%2f 解码后为 ../)
    let (status, _, _) =
        get_public(&app, "/oss/file/local/..%2f..%2fetc%2fpasswd").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    // 不存在的文件
    let (status, _, _) =
        get_public(&app, "/oss/file/local/0192a1b2c3d4.png").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    // 非 local 配置(S3) → local_file_path 返回 None → 404
    let (status, _, _) =
        get_public(&app, "/oss/file/miniocfg/0192a1b2c3d4.png").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    // 不存在的配置
    let (status, _, _) =
        get_public(&app, "/oss/file/nosuch/0192a1b2c3d4.png").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn local_same_name_files_coexist_with_distinct_urls() {
    let app = TestApp::new().await;
    let root = temp_root("samename");
    seed_oss_configs(&app, &root).await;
    let uploader = user_with_perms(&["system:oss:upload"]);

    let (status, body1) =
        upload(&app, &uploader, "report-final.png", "image/png", b"v1").await;
    assert_ok(status, &body1);
    let (status, body2) =
        upload(&app, &uploader, "report-final.png", "image/png", b"v2").await;
    assert_ok(status, &body2);

    // 同名文件: 存储名不同(UUID), 显示名相同 —— sys_oss 表既有能力
    let name1 = body1
        .pointer("/data/fileName")
        .and_then(Value::as_str)
        .unwrap();
    let name2 = body2
        .pointer("/data/fileName")
        .and_then(Value::as_str)
        .unwrap();
    assert_ne!(name1, name2, "同名文件存储名应不同");
    assert_eq!(
        body1.pointer("/data/originalName").and_then(Value::as_str),
        body2.pointer("/data/originalName").and_then(Value::as_str),
    );

    let url1 = body1.pointer("/data/url").and_then(Value::as_str).unwrap();
    let url2 = body2.pointer("/data/url").and_then(Value::as_str).unwrap();
    let (s1, _, b1) = get_public(&app, url1).await;
    let (s2, _, b2) = get_public(&app, url2).await;
    assert_eq!((s1, b1.as_slice()), (StatusCode::OK, b"v1".as_slice()));
    assert_eq!((s2, b2.as_slice()), (StatusCode::OK, b"v2".as_slice()));
}

#[tokio::test]
async fn local_config_rejects_private_access_policy() {
    let app = TestApp::new().await;
    let adder = user_with_perms(&["system:ossConfig:add"]);
    let editor =
        user_with_perms(&["system:ossConfig:edit", "system:ossConfig:list"]);

    // (a) local(file://) + accessPolicy=0(私有) → 400 拒绝, 且不落库。
    // accessKey 等字段有 min=1 校验, 不能传空串(否则被参数校验抢先拒绝,
    // 测不到本守卫)。
    let (status, body) = app
        .post(
            "/resource/oss/config",
            serde_json::json!({
                "configKey": "localpriv",
                "accessKey": "x",
                "secretKey": "x",
                "bucketName": "default",
                "endpoint": "file://./data/oss-priv",
                "accessPolicy": "0"
            }),
            Some(&adder),
        )
        .await;
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "local 私有配置应 400: {body}"
    );
    assert_ne!(code(&body), 200, "信封 code 应非 200: {body}");
    let msg = body.pointer("/msg").and_then(Value::as_str).unwrap_or("");
    assert!(
        msg.contains("accessPolicy"),
        "错误信息应点明 accessPolicy: {msg}"
    );
    // 未落库: 配置列表仍为空(测试库种子无 sys_oss_config 行)
    let (status, body) =
        app.get("/resource/oss/config/list", Some(&editor)).await;
    assert_ok(status, &body);
    assert_eq!(
        body.pointer("/data/total").and_then(Value::as_i64),
        Some(0),
        "被拒绝的配置不应落库: {body}"
    );

    // (b) 对照组: 同样 file:// 但 accessPolicy=1(公开) → 接受
    let (status, body) = app
        .post(
            "/resource/oss/config",
            serde_json::json!({
                "configKey": "localpub",
                "accessKey": "x",
                "secretKey": "x",
                "bucketName": "default",
                "endpoint": "file://./data/oss-pub",
                "accessPolicy": "1"
            }),
            Some(&adder),
        )
        .await;
    assert_ok(status, &body);

    // (c) PUT 更新路径: 把已存在的 local 配置改为私有 → 同样拒绝且不生效
    let (status, body) =
        app.get("/resource/oss/config/list", Some(&editor)).await;
    assert_ok(status, &body);
    // ossConfigId 出站恒为字符串(stringify_ids), 兼容数字形态; 入站 BO
    // 是 Option<i64>, 还原为数字传回。
    let cfg_id: i64 = body
        .pointer("/data/rows/0/ossConfigId")
        .and_then(|v| {
            v.as_i64()
                .or_else(|| v.as_str().and_then(|s| s.parse().ok()))
        })
        .expect("ossConfigId");
    let (status, body) = app
        .put(
            "/resource/oss/config",
            serde_json::json!({
                "ossConfigId": cfg_id,
                "configKey": "localpub",
                "accessKey": "x",
                "secretKey": "x",
                "bucketName": "default",
                "endpoint": "file://./data/oss-pub",
                "accessPolicy": "0"
            }),
            Some(&editor),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "更新为私有应 400: {body}");
    assert_ne!(code(&body), 200, "信封 code 应非 200: {body}");
    let (status, body) =
        app.get("/resource/oss/config/list", Some(&editor)).await;
    assert_ok(status, &body);
    assert_eq!(
        body.pointer("/data/rows/0/accessPolicy")
            .and_then(Value::as_str),
        Some("1"),
        "被拒绝的更新不应生效: {body}"
    );
}

/// 回归测试: 上传体 > oper_log 中间件的 64KB body 上限不应解析失败。
///
/// 背景: OperLogMiddleware 曾用 `to_bytes(body, 65536)` 读取所有非 GET 请求体,
/// 超限时 `.unwrap_or_default()` 把 body 静默截成空,导致下游 `Multipart` 解析
/// 失败,返回 `读取上传文件失败: Error parsing multipart/form-data request`。
/// 修复后 multipart 请求跳过 body 读取,此处用 70KB(>64KB) PNG 字节流验证。
#[tokio::test]
async fn upload_larger_than_operlog_cap_succeeds() {
    let app = TestApp::new().await;
    let root = temp_root("big");
    seed_oss_configs(&app, &root).await;
    let uploader = user_with_perms(&["system:oss:upload"]);

    // 70_000 字节,刻意超过 oper_log 的 65536 上限。
    let big_png = vec![0u8; 70_000];
    let (status, body) =
        upload(&app, &uploader, "big-avatar.png", "image/png", &big_png).await;
    assert_ok(status, &body);
    let oss_id = body
        .pointer("/data/ossId")
        .and_then(Value::as_str)
        .expect("ossId 应存在");
    assert!(!oss_id.is_empty(), "ossId 不应为空: {body}");
}

/// 回归测试: 上传体超过 axum 默认 2MB body 上限应成功。
///
/// 背景: axum 默认 `DefaultBodyLimit` 为 2 MiB,个人中心头像 / 文件管理
/// 上传的图片常常超过该值,导致 body 被截断、`Multipart` 解析失败,报
/// `读取文件内容失败: Error parsing multipart/form-data request`(500)。
/// 修复在 `/resource/oss/upload` 路由单独放宽到 10MB(对齐 RuoYi
/// `spring.servlet.multipart.max-file-size: 10MB`),此处用 3MB 验证。
#[tokio::test]
async fn upload_larger_than_default_body_limit_succeeds() {
    let app = TestApp::new().await;
    let root = temp_root("mb");
    seed_oss_configs(&app, &root).await;
    let uploader = user_with_perms(&["system:oss:upload"]);

    // 3 MiB,超过 axum 默认 DefaultBodyLimit(2 MiB),但低于放宽后的 10MB。
    let big_png = vec![0u8; 3 * 1024 * 1024];
    let (status, body) =
        upload(&app, &uploader, "big-photo.png", "image/png", &big_png).await;
    assert_ok(status, &body);
    let oss_id = body
        .pointer("/data/ossId")
        .and_then(Value::as_str)
        .expect("ossId 应存在");
    assert!(!oss_id.is_empty(), "ossId 不应为空: {body}");
}
