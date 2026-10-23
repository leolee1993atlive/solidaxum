use lettre::{
    Message, SmtpTransport, Transport,
    message::{Mailbox, header::ContentType},
    transport::smtp::authentication::Credentials,
};

use crate::error::{AppError, AppResult};

/// 邮件配置
#[derive(Clone)]
pub struct MailConfig {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    pub from_name: String,
    pub from_email: String,
    pub ssl: bool,
}

/// 邮件服务
pub struct MailService {
    config: MailConfig,
}

impl MailService {
    /// 创建邮件服务
    pub fn new(config: MailConfig) -> Self {
        Self { config }
    }

    /// 发送纯文本邮件
    pub async fn send_simple(
        &self,
        to: &[String],
        subject: &str,
        text: &str,
    ) -> AppResult<()> {
        self.send(to, subject, text, false).await
    }

    /// 发送 HTML 邮件
    pub async fn send_html(
        &self,
        to: &[String],
        subject: &str,
        html: &str,
    ) -> AppResult<()> {
        self.send(to, subject, html, true).await
    }

    async fn send(
        &self,
        to: &[String],
        subject: &str,
        body: &str,
        is_html: bool,
    ) -> AppResult<()> {
        let config = self.config.clone();
        let to = to.to_vec();
        let subject = subject.to_string();
        let body = body.to_string();

        tokio::task::spawn_blocking(move || {
            let from: Mailbox =
                format!("{} <{}>", config.from_name, config.from_email)
                    .parse()
                    .map_err(|e| {
                        AppError::BadRequest(format!(
                            "发件人地址格式错误: {}",
                            e
                        ))
                    })?;

            let mut builder = Message::builder().from(from).subject(subject);
            for addr in &to {
                let mailbox: Mailbox = addr.parse().map_err(|e| {
                    AppError::BadRequest(format!("收件人地址格式错误: {}", e))
                })?;
                builder = builder.to(mailbox);
            }

            let message = if is_html {
                builder.header(ContentType::TEXT_HTML).body(body).map_err(
                    |e| AppError::Internal(format!("构建邮件失败: {}", e)),
                )?
            } else {
                builder.body(body).map_err(|e| {
                    AppError::Internal(format!("构建邮件失败: {}", e))
                })?
            };

            let creds = Credentials::new(config.username, config.password);
            let mailer = if config.ssl {
                SmtpTransport::relay(&config.host)
                    .map_err(|e| {
                        AppError::Internal(format!("SMTP配置错误: {}", e))
                    })?
                    .port(config.port)
                    .credentials(creds)
                    .build()
            } else {
                SmtpTransport::relay(&config.host)
                    .map_err(|e| {
                        AppError::Internal(format!("SMTP配置错误: {}", e))
                    })?
                    .port(config.port)
                    .credentials(creds)
                    .tls(lettre::transport::smtp::client::Tls::None)
                    .build()
            };

            mailer.send(&message).map_err(|e| {
                AppError::Internal(format!("邮件发送失败: {}", e))
            })?;

            Ok(())
        })
        .await
        .map_err(|e| AppError::Internal(format!("邮件任务执行失败: {}", e)))?
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> MailConfig {
        MailConfig {
            host: "smtp.example.com".to_string(),
            port: 587,
            username: "user".to_string(),
            password: "pass".to_string(),
            from_name: "测试".to_string(),
            from_email: "test@example.com".to_string(),
            ssl: true,
        }
    }

    #[test]
    fn test_mail_service_new() {
        let service = MailService::new(test_config());
        assert_eq!(service.config.host, "smtp.example.com");
        assert_eq!(service.config.port, 587);
    }
}
