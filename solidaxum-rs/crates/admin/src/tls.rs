//! HTTPS(首装增强 #1,2026-10-28 决策:自签默认 3650 天)。
//!
//! 动机:内网部署出于安全合规要求 HTTPS 形态(内网无外部攻击面,
//! 也无法签发合法证书)。三种模式:`off`(默认,纯 HTTP)/`self-signed`
//! (安装向导生成自签证书)/`import`(导入客户自有证书)。
//!
//! TLS 终结在**进程内**(axum-server + rustls),不引入 nginx —— 安装包
//! 的产品形态是自包含单进程。装载器与运行时共用一份实现,`--check-config`
//! 预检的就是启动时真实会走的路径。

use std::{path::Path, sync::Arc};

use anyhow::Context;

/// 自签证书有效期(天)。内网没有续期运维,短有效期等于埋雷。
pub const SELF_SIGNED_VALIDITY_DAYS: i64 = 3650;

/// 生成自签证书,返回 `(cert_pem, key_pem)`。
///
/// SAN/CN 都填 `cn`(服务器 IP 或主机名)—— 自签证书浏览器必然告警
/// (客户需手动信任),但名称对上至少能让部分客户端显示正确的主机名。
pub fn generate_self_signed(cn: &str) -> anyhow::Result<(String, String)> {
    let mut params = rcgen::CertificateParams::new(vec![cn.to_string()])
        .map_err(|e| anyhow::anyhow!("构造证书参数失败: {e}"))?;
    params.distinguished_name.push(
        rcgen::DnType::CommonName,
        rcgen::DnValue::Utf8String(cn.to_string()),
    );
    // 生效时间前移 1 天,容忍目标机时钟略快
    params.not_before = time::OffsetDateTime::now_utc()
        .saturating_add(time::Duration::days(-1));
    params.not_after = time::OffsetDateTime::now_utc()
        .saturating_add(time::Duration::days(SELF_SIGNED_VALIDITY_DAYS));

    let key_pair = rcgen::KeyPair::generate()
        .map_err(|e| anyhow::anyhow!("生成密钥对失败: {e}"))?;
    let cert = params
        .self_signed(&key_pair)
        .map_err(|e| anyhow::anyhow!("自签证书生成失败: {e}"))?;
    Ok((cert.pem(), key_pair.serialize_pem()))
}

/// 从 PEM 文件装载证书与私钥,构建 rustls 服务端配置。
///
/// 证书/密钥不配对、文件缺失、格式错误都会在这里以带上下文的错误
/// 暴露 —— 运行时启动 fail-fast(不静默降级回 HTTP,避免客户误以为
/// 还在加密通道),`--check-config` 复用同一函数做安装期预检。
pub fn build_server_config(
    cert_path: &Path,
    key_path: &Path,
) -> anyhow::Result<rustls::ServerConfig> {
    let certs = load_certs(cert_path)?;
    let key = load_private_key(key_path)?;

    // 显式指定提供方:本 crate 依赖图里 rustls 同时带 aws-lc-rs(sea-orm)
    // 与 ring(rcgen)两个 feature,进程级默认无法自动判定 —— 用
    // `ServerConfig::builder()` 会 panic。固定 aws-lc-rs,与 sea-orm 栈一致。
    rustls::ServerConfig::builder_with_provider(
        rustls::crypto::aws_lc_rs::default_provider().into(),
    )
    .with_safe_default_protocol_versions()
    .map_err(|e| anyhow::anyhow!("协商 TLS 协议版本失败: {e}"))?
    .with_no_client_auth()
    .with_single_cert(certs, key)
    .map_err(|e| {
        anyhow::anyhow!(
            "证书与私钥不配对或不受支持(cert={:?}, key={:?}): {e}",
            cert_path,
            key_path
        )
    })
}

fn load_certs(
    path: &Path,
) -> anyhow::Result<Vec<rustls::pki_types::CertificateDer<'static>>> {
    let f = std::fs::File::open(path)
        .with_context(|| format!("打开证书文件失败: {:?}", path))?;
    let mut r = std::io::BufReader::new(f);
    let certs: Result<Vec<_>, _> = rustls_pemfile::certs(&mut r).collect();
    let certs = certs
        .map_err(|e| anyhow::anyhow!("解析证书 PEM 失败({:?}): {e}", path))?;
    if certs.is_empty() {
        anyhow::bail!("证书文件不含任何 PEM 证书: {:?}", path);
    }
    Ok(certs)
}

fn load_private_key(
    path: &Path,
) -> anyhow::Result<rustls::pki_types::PrivateKeyDer<'static>> {
    let f = std::fs::File::open(path)
        .with_context(|| format!("打开私钥文件失败: {:?}", path))?;
    let mut r = std::io::BufReader::new(f);
    rustls_pemfile::private_key(&mut r)
        .map_err(|e| anyhow::anyhow!("解析私钥 PEM 失败({:?}): {e}", path))?
        .ok_or_else(|| anyhow::anyhow!("私钥文件不含 PEM 私钥: {:?}", path))
}

/// HTTPS 服务启动(TLS 分支唯一入口):由 axum_server 独占绑定监听地址。
///
/// 调用方(main.rs 的 HTTP 分支)若预先绑定过同一地址的 tokio listener,
/// 这里会在 serve 时 EADDRINUSE —— 单属主约定:TLS 分支持有 axum_server
/// 的绑定,HTTP 分支持有 tokio listener,二者互斥。
pub async fn serve_https(
    app: axum::Router,
    addr: String,
    cfg: rustls::ServerConfig,
) -> anyhow::Result<()> {
    axum_server::bind_rustls(
        resolve_addr(&addr)?,
        axum_server::tls_rustls::RustlsConfig::from_config(Arc::new(cfg)),
    )
    .serve(app.into_make_service())
    .await?;
    Ok(())
}

fn resolve_addr(addr: &str) -> anyhow::Result<std::net::SocketAddr> {
    use std::net::ToSocketAddrs;
    addr.to_socket_addrs()
        .with_context(|| format!("解析监听地址失败: {addr}"))?
        .next()
        .ok_or_else(|| anyhow::anyhow!("监听地址无可用解析结果: {addr}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_self_signed_passes_the_runtime_loader() {
        // 生成 → 落盘 → 装载,即 --write-config 与启动路径的闭环
        let (cert, key) = generate_self_signed("192.168.1.10").unwrap();
        let dir = std::env::temp_dir().join(format!(
            "solidaxum-tls-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let cert_p = dir.join("server-cert.pem");
        let key_p = dir.join("server-key.pem");
        std::fs::write(&cert_p, &cert).unwrap();
        std::fs::write(&key_p, &key).unwrap();

        let cfg = build_server_config(&cert_p, &key_p);
        assert!(cfg.is_ok(), "自签产物必须能被运行时装载器接受");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn mismatched_key_is_rejected_with_context() {
        // 证书 A 配密钥 B —— 必须报错而不是 panic
        let (cert_a, _key_a) = generate_self_signed("host-a").unwrap();
        let (_cert_b, key_b) = generate_self_signed("host-b").unwrap();
        let dir = std::env::temp_dir().join(format!(
            "solidaxum-tls-mismatch-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let cert_p = dir.join("a-cert.pem");
        let key_p = dir.join("b-key.pem");
        std::fs::write(&cert_p, cert_a).unwrap();
        std::fs::write(&key_p, key_b).unwrap();

        let err = build_server_config(&cert_p, &key_p).unwrap_err();
        assert!(
            err.to_string().contains("不配对")
                || err.to_string().contains("不受支持"),
            "错误信息应说明不配对: {err}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn missing_files_fail_fast_with_paths() {
        let err = build_server_config(
            Path::new("/nonexistent/cert.pem"),
            Path::new("/nonexistent/key.pem"),
        )
        .unwrap_err();
        assert!(
            err.to_string().contains("cert.pem"),
            "错误信息应带文件路径: {err}"
        );
    }

    #[test]
    fn invalid_pem_is_rejected_with_context() {
        // 垃圾内容不能 panic,也不能被当作「无证书」放过
        let dir = std::env::temp_dir().join(format!(
            "solidaxum-tls-invalid-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let cert_p = dir.join("bad-cert.pem");
        let key_p = dir.join("bad-key.pem");
        std::fs::write(&cert_p, "not a pem at all").unwrap();
        std::fs::write(
            &key_p,
            "-----BEGIN PRIVATE KEY-----\ngarbage\n-----END PRIVATE KEY-----\n",
        )
        .unwrap();

        let err = build_server_config(&cert_p, &key_p).unwrap_err();
        assert!(
            err.to_string().contains("PEM"),
            "错误信息应说明 PEM 解析失败: {err}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}

/// 启动路径回归:真实起 axum_server TLS 监听并完成 TCP+TLS+HTTP 握手。
/// loader 单测只验证证书装载,验证不了「进程真的能起、端口真的能连」。
#[cfg(test)]
mod startup_tests {
    use std::sync::Arc;

    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        time::{Duration, sleep, timeout},
    };

    use super::*;

    /// 自签证书不可能被系统信任,客户端校验器直接放行(仅测试用)。
    #[derive(Debug)]
    struct NoVerify;

    impl rustls::client::danger::ServerCertVerifier for NoVerify {
        fn verify_server_cert(
            &self,
            _end_entity: &rustls::pki_types::CertificateDer<'_>,
            _intermediates: &[rustls::pki_types::CertificateDer<'_>],
            _server_name: &rustls::pki_types::ServerName<'_>,
            _ocsp_response: &[u8],
            _now: rustls::pki_types::UnixTime,
        ) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error>
        {
            Ok(rustls::client::danger::ServerCertVerified::assertion())
        }

        fn verify_tls12_signature(
            &self,
            _message: &[u8],
            _cert: &rustls::pki_types::CertificateDer<'_>,
            _dss: &rustls::DigitallySignedStruct,
        ) -> Result<
            rustls::client::danger::HandshakeSignatureValid,
            rustls::Error,
        > {
            Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
        }

        fn verify_tls13_signature(
            &self,
            _message: &[u8],
            _cert: &rustls::pki_types::CertificateDer<'_>,
            _dss: &rustls::DigitallySignedStruct,
        ) -> Result<
            rustls::client::danger::HandshakeSignatureValid,
            rustls::Error,
        > {
            Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
        }

        fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
            vec![
                rustls::SignatureScheme::ECDSA_NISTP256_SHA256,
                rustls::SignatureScheme::ECDSA_NISTP384_SHA384,
                rustls::SignatureScheme::ED25519,
                rustls::SignatureScheme::RSA_PKCS1_SHA256,
                rustls::SignatureScheme::RSA_PKCS1_SHA384,
                rustls::SignatureScheme::RSA_PKCS1_SHA512,
                rustls::SignatureScheme::RSA_PSS_SHA256,
                rustls::SignatureScheme::RSA_PSS_SHA384,
                rustls::SignatureScheme::RSA_PSS_SHA512,
            ]
        }
    }

    fn danger_client_config() -> rustls::ClientConfig {
        rustls::ClientConfig::builder_with_provider(
            rustls::crypto::aws_lc_rs::default_provider().into(),
        )
        .with_safe_default_protocol_versions()
        .unwrap()
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(NoVerify))
        .with_no_client_auth()
    }

    /// TCP 连接 + TLS 握手 + HTTP/1.1 GET,返回完整响应文本。
    async fn tls_get_pong(
        addr: std::net::SocketAddr,
    ) -> anyhow::Result<String> {
        let tcp = tokio::net::TcpStream::connect(addr).await?;
        let connector =
            tokio_rustls::TlsConnector::from(Arc::new(danger_client_config()));
        let server_name =
            rustls::pki_types::ServerName::try_from("127.0.0.1".to_string())
                .unwrap();
        let mut tls = connector.connect(server_name, tcp).await?;
        tls.write_all(
            b"GET /ping HTTP/1.1

Host: 127.0.0.1

Connection: close



",
        )
        .await?;
        let mut buf = Vec::new();
        tls.read_to_end(&mut buf).await?;
        Ok(String::from_utf8_lossy(&buf).into_owned())
    }

    #[tokio::test]
    async fn https_startup_accepts_tls_connection() {
        // 生成 → 落盘 → 装载,与 --write-config + 启动路径同一闭环
        let (cert, key) = generate_self_signed("127.0.0.1").unwrap();
        let dir = std::env::temp_dir().join(format!(
            "solidaxum-tls-startup-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let cert_p = dir.join("server-cert.pem");
        let key_p = dir.join("server-key.pem");
        std::fs::write(&cert_p, &cert).unwrap();
        std::fs::write(&key_p, key).unwrap();
        let rustls_cfg = build_server_config(&cert_p, &key_p).unwrap();

        let app = axum::Router::new()
            .route("/ping", axum::routing::get(|| async { "pong" }));

        // 选一个可用的 loopback 临时端口(拿到即放,留给 axum_server 绑)
        let probe = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = probe.local_addr().unwrap();
        drop(probe);
        let addr_str = addr.to_string();

        // 与 main.rs TLS 分支同一条 serve 路径
        let server = tokio::spawn(serve_https(app, addr_str, rustls_cfg));

        // 轮询直到 TLS GET 拿到 200 pong;server 若提前退出(EADDRINUSE
        // 等启动失败)立刻带出错误,而不是傻等到超时。
        let mut body = String::new();
        for _ in 0..20 {
            if server.is_finished() {
                let err = server.await.unwrap().unwrap_err();
                panic!("HTTPS 服务提前退出(启动失败): {err:?}");
            }
            match timeout(Duration::from_secs(1), tls_get_pong(addr)).await {
                Ok(Ok(resp))
                    if resp.contains("200") && resp.contains("pong") =>
                {
                    body = resp;
                    break;
                }
                _ => sleep(Duration::from_millis(50)).await,
            }
        }
        assert!(!body.is_empty(), "启动后必须能通过 TCP+TLS 拿到 /ping 响应");

        server.abort();
        let _ = server.await;
        let _ = std::fs::remove_dir_all(&dir);
    }
}
