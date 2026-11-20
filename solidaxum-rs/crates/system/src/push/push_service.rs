//! 通用推送编排:SSE 优先 + IM/短信/APP 渠道扇出。
//!
//! 设计要点:
//! 1. **SSE 先发**:每个收件人一条 `PushMessage::workflow`(雪花消息 ID),
//!    不依赖外部网络,站内消息永远先到;
//! 2. **渠道降级**:钉钉/企微/飞书按 `sys_message_channel` 配置发送,短信/
//!    APP 只经注册表;未配置/未注册一律写 `SKIPPED` 日志,不向上抛错;
//! 3. **平台 ID 回填**:`sys_user.*_user_id` 为空且有手机号时,按手机号查
//!    平台 userid/openid 并回写,下一次直接命中;
//! 4. **10s 去重**:同一 `(biz_type, biz_id, 用户集合)` 窗口内只发一次,
//!    防止事件重试/并发重复通知;映射表按窗口懒清理,大小有界;
//! 5. **失败隔离**:任何渠道失败只落 `sys_message_send_log`,不影响业务
//!    事务,也不影响其他渠道。
//!
//! 事件钩子必须在业务事务提交后调用(`tokio::spawn` 异步扇出),HTTP
//! 渠道慢请求不得阻塞业务响应。

use std::{
    collections::{HashMap, hash_map::DefaultHasher},
    hash::{Hash, Hasher},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use app_core::{
    infra::Infrastructure, serde::encrypt_field::FieldEncryptor,
    util::snowflake,
};
use app_data::{DataContext, DataSource};
use serde::Serialize;
use serde_json::json;

use crate::{
    push::{
        channel_config_service::resolve_config,
        channel_senders,
        channels::{self, Channel, SendOutcome, SendRequest},
        message::PushMessage,
        repository::sys_message_channel::ChannelConfig,
        sse::PushHub,
    },
    user::{model::user::User, repository::user_repo::UserRepository},
};

/// 去重窗口(同一事件 10s 内重复触发只发一次)。
const DEDUPE_WINDOW: Duration = Duration::from_secs(10);
/// 去重表软上限:超过即按窗口清理,避免进程长期运行无界增长。
const DEDUPE_PRUNE_THRESHOLD: usize = 512;

/// 单渠道发送结果。
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChannelPushResult {
    pub channel: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skip_reason: Option<String>,
}

/// 一次编排的汇总结果(事件钩子可忽略,测试/调用方用于观测)。
#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PushResult {
    /// 命中 10s 去重窗口,未实际发送。
    pub deduped: bool,
    /// SSE 实际推送的用户数。
    pub sse_sent: usize,
    /// 渠道成功数。
    pub success: usize,
    /// 渠道失败数。
    pub failed: usize,
    /// 渠道跳过数。
    pub skipped: usize,
    pub channels: Vec<ChannelPushResult>,
}

/// 通用推送服务(持 `DataSource` + 共享 `PushHub`)。
#[derive(Clone)]
pub struct PushService {
    ds: Arc<DataSource>,
    hub: PushHub,
    encryptor: Option<Arc<dyn FieldEncryptor>>,
    dedupe: Arc<Mutex<HashMap<String, Instant>>>,
}

impl PushService {
    pub fn new(infra: &Infrastructure, hub: PushHub) -> Self {
        Self {
            ds: infra.data_source(),
            hub,
            encryptor: infra.field_encryptor(),
            dedupe: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// 供同 crate/事件钩子读取数据源(如结案后补查案件/处理项)。
    pub fn data_source(&self) -> Arc<DataSource> {
        self.ds.clone()
    }

    /// 通用通知:SSE 逐用户先发,随后按 WECHAT → DINGTALK → FEISHU →
    /// SMS → APP 顺序扇出渠道。
    ///
    /// 返回计数而非错误:通知失败只影响日志,绝不影响业务事务。
    pub async fn notify(
        &self,
        biz_type: &str,
        biz_id: i64,
        user_ids: &[i64],
        content: &str,
        path: Option<&str>,
    ) -> PushResult {
        let mut ids = user_ids.to_vec();
        ids.sort_unstable();
        ids.dedup();
        if ids.is_empty() {
            return PushResult::default();
        }
        let key = dedupe_key(biz_type, biz_id, &ids);
        if self.is_duplicate(key) {
            return PushResult {
                deduped: true,
                ..Default::default()
            };
        }

        let ctx = self.ds.system_context();
        let biz_id_str = biz_id.to_string();
        let batch_id = biz_id_str.clone();
        let mut result = PushResult::default();

        // 1. SSE 优先(站内消息不依赖外部渠道可用性)
        for &uid in &ids {
            self.hub.push_to_user(
                uid,
                PushMessage::workflow(
                    snowflake::next_id(),
                    content.to_string(),
                    path.map(str::to_string),
                ),
            );
        }
        result.sse_sent = ids.len();
        write_sse_log(
            &ctx,
            biz_type,
            Some(&biz_id_str),
            Some(&batch_id),
            &ids,
            content,
        )
        .await;

        // 2. 用户行只读一次(平台 ID/手机号/用户名都从这里取)
        let users = match UserRepository::new()
            .select_by_ids_unscoped(&ctx, &ids)
            .await
        {
            Ok(rows) => rows,
            Err(e) => {
                tracing::warn!(error = %e, "推送读取用户行失败,渠道降级为空收件人");
                Vec::new()
            }
        };

        // 3. IM 渠道
        for channel in [Channel::Wechat, Channel::Dingtalk, Channel::Feishu] {
            let outcome = self
                .dispatch_im(
                    &ctx,
                    channel,
                    &users,
                    &ids,
                    content,
                    biz_type,
                    &biz_id_str,
                    &batch_id,
                )
                .await;
            Self::record(&mut result, channel.as_str(), outcome);
        }

        // 4. 短信(手机号) / APP(用户名)
        let phones: Vec<String> = users
            .iter()
            .filter_map(|u| u.phone_number.clone())
            .map(|p| p.trim().to_string())
            .filter(|p| !p.is_empty())
            .collect();
        let sms_outcome = channels::dispatch(
            &ctx,
            Channel::Sms,
            SendRequest {
                biz_type,
                config: None,
                user_ids: &ids,
                content,
                content_map: Some(&json!({ "text": content })),
                platform_ids: &phones,
                batch_id: &batch_id,
                biz_id: &biz_id_str,
            },
        )
        .await
        .unwrap_or_else(|e| {
            tracing::warn!(error = %e, "短信通道日志写入失败");
            SendOutcome::Skipped
        });
        Self::record(&mut result, Channel::Sms.as_str(), sms_outcome);

        let names: Vec<String> = users
            .iter()
            .map(|u| u.user_name.clone())
            .filter(|n| !n.trim().is_empty())
            .collect();
        let app_outcome = channels::dispatch(
            &ctx,
            Channel::App,
            SendRequest {
                biz_type,
                config: None,
                user_ids: &ids,
                content,
                content_map: None,
                platform_ids: &names,
                batch_id: &batch_id,
                biz_id: &biz_id_str,
            },
        )
        .await
        .unwrap_or_else(|e| {
            tracing::warn!(error = %e, "APP 通道日志写入失败");
            SendOutcome::Skipped
        });
        Self::record(&mut result, Channel::App.as_str(), app_outcome);

        result
    }

    /// IM 单渠道:解析配置 → 平台 ID(缺则手机号回填)→ dispatch。
    #[allow(clippy::too_many_arguments)]
    async fn dispatch_im(
        &self,
        ctx: &DataContext<'_>,
        channel: Channel,
        users: &[User],
        user_ids: &[i64],
        content: &str,
        biz_type: &str,
        biz_id: &str,
        batch_id: &str,
    ) -> SendOutcome {
        let channel_name = channel.as_str();
        let (cfg, _source) =
            match resolve_config(ctx, self.encryptor.as_ref(), channel_name)
                .await
            {
                Ok(v) => v,
                Err(e) => {
                    tracing::warn!(
                        channel = channel_name,
                        error = %e,
                        "渠道配置解析失败,按未配置降级"
                    );
                    (ChannelConfig::default(), "none".to_string())
                }
            };

        let configured = cfg.enabled && cfg.is_configured();
        let mut platform_ids = if configured {
            self.resolve_platform_ids(ctx, channel, &cfg, users).await
        } else {
            Vec::new()
        };
        platform_ids.retain(|id| !id.trim().is_empty());
        platform_ids.dedup();

        channels::dispatch(
            ctx,
            channel,
            SendRequest {
                biz_type,
                config: configured.then_some(&cfg),
                user_ids,
                content,
                content_map: None,
                platform_ids: &platform_ids,
                batch_id,
                biz_id,
            },
        )
        .await
        .unwrap_or_else(|e| {
            tracing::warn!(
                channel = channel_name,
                error = %e,
                "渠道发送日志写入失败"
            );
            SendOutcome::Skipped
        })
    }

    /// 取平台 ID;缺失且有手机号时按平台接口反查并回写 `sys_user`。
    async fn resolve_platform_ids(
        &self,
        ctx: &DataContext<'_>,
        channel: Channel,
        cfg: &ChannelConfig,
        users: &[User],
    ) -> Vec<String> {
        let mut ids = Vec::new();
        let mut missing = Vec::new();
        for user in users {
            match platform_id_of(channel, user) {
                Some(id) if !id.trim().is_empty() => ids.push(id),
                _ => {
                    if let Some(phone) = user
                        .phone_number
                        .as_deref()
                        .map(str::trim)
                        .filter(|p| !p.is_empty())
                    {
                        missing.push((user.user_id.unwrap_or_default(), phone));
                    }
                }
            }
        }
        if missing.is_empty() {
            return ids;
        }

        // token 取一次,逐手机号反查;失败只跳过回填,不影响其余渠道。
        let token = match channel {
            Channel::Dingtalk => channel_senders::dingtalk_token(cfg).await,
            Channel::Wechat => channel_senders::wechat_token(cfg).await,
            Channel::Feishu => channel_senders::feishu_token(cfg).await,
            _ => return ids,
        };
        let token = match token {
            Ok(t) => t,
            Err(e) => {
                tracing::warn!(
                    channel = channel.as_str(),
                    error = %e,
                    "取平台 token 失败,跳过手机号回填"
                );
                return ids;
            }
        };

        let repo = UserRepository::new();
        for (user_id, phone) in missing {
            let found = match channel {
                Channel::Dingtalk => {
                    channel_senders::dingtalk_userid_by_mobile(
                        cfg, &token, phone,
                    )
                    .await
                }
                Channel::Wechat => {
                    channel_senders::wechat_userid_by_mobile(cfg, &token, phone)
                        .await
                }
                Channel::Feishu => {
                    channel_senders::feishu_userid_by_mobile(cfg, &token, phone)
                        .await
                }
                _ => Ok(None),
            };
            match found {
                Ok(Some(platform_id)) => {
                    if let Err(e) = repo
                        .update_platform_user_id(
                            ctx,
                            user_id,
                            channel.as_str(),
                            &platform_id,
                        )
                        .await
                    {
                        tracing::warn!(
                            user_id,
                            channel = channel.as_str(),
                            error = %e,
                            "平台 ID 回写失败"
                        );
                    }
                    ids.push(platform_id);
                }
                Ok(None) => {}
                Err(e) => {
                    tracing::warn!(
                        user_id,
                        channel = channel.as_str(),
                        error = %e,
                        "手机号反查平台 ID 失败"
                    );
                }
            }
        }
        ids
    }

    fn record(result: &mut PushResult, channel: &str, outcome: SendOutcome) {
        let status = match outcome {
            SendOutcome::Success => {
                result.success += 1;
                "SUCCESS"
            }
            SendOutcome::Failed => {
                result.failed += 1;
                "FAILED"
            }
            SendOutcome::Skipped => {
                result.skipped += 1;
                "SKIPPED"
            }
        };
        result.channels.push(ChannelPushResult {
            channel: channel.to_string(),
            status: status.to_string(),
            skip_reason: None,
        });
    }

    /// 返回 true 表示命中 10s 去重窗口。
    fn is_duplicate(&self, key: String) -> bool {
        let now = Instant::now();
        let Ok(mut map) = self.dedupe.lock() else {
            // 锁中毒不阻断通知:宁可重复发,也不静默丢消息。
            return false;
        };
        if map.len() > DEDUPE_PRUNE_THRESHOLD {
            map.retain(|_, at| now.duration_since(*at) < DEDUPE_WINDOW);
        }
        if let Some(at) = map.get(&key)
            && now.duration_since(*at) < DEDUPE_WINDOW
        {
            return true;
        }
        map.insert(key, now);
        false
    }
}

fn platform_id_of(channel: Channel, user: &User) -> Option<String> {
    match channel {
        Channel::Dingtalk => user.dingtalk_user_id.clone(),
        Channel::Wechat => user.wechat_user_id.clone(),
        Channel::Feishu => user.feishu_user_id.clone(),
        _ => None,
    }
}

fn dedupe_key(biz_type: &str, biz_id: i64, user_ids: &[i64]) -> String {
    let mut hasher = DefaultHasher::new();
    user_ids.hash(&mut hasher);
    format!("{biz_type}:{biz_id}:{:x}", hasher.finish())
}

/// 写一条 SSE 成功日志(事件通知/流程通知共用)。
/// `batch_id` 与渠道发送行对齐(同批推送可聚合);流程通知无批次传 None。
pub async fn write_sse_log(
    ctx: &DataContext<'_>,
    biz_type: &str,
    biz_id: Option<&str>,
    batch_id: Option<&str>,
    user_ids: &[i64],
    content: &str,
) {
    channels::write_log(
        ctx, "SSE", biz_type, user_ids, content, "SUCCESS", None, None, None,
        batch_id, biz_id,
    )
    .await;
}
