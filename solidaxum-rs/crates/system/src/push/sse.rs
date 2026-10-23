use std::{convert::Infallible, sync::Arc, time::Duration};

use app_core::{
    middleware::extractors::CurrentUser, vo::response::stringify_ids,
};
use axum::{
    extract::State,
    response::sse::{Event, KeepAlive, Sse},
};
use tokio::sync::broadcast;
use tokio_stream::{Stream, StreamExt, wrappers::BroadcastStream};

use crate::{app::AppState, push::message::PushMessage};

#[derive(Clone)]
pub struct PushHub {
    tx: broadcast::Sender<PushMessage>,
}

impl PushHub {
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(1024);
        Self { tx }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<PushMessage> {
        self.tx.subscribe()
    }

    pub fn push_to_user(&self, user_id: i64, msg: PushMessage) {
        let mut msg = msg;
        msg.to_user_id = Some(user_id);
        let _ = self.tx.send(msg);
    }

    pub fn push_all(&self, msg: PushMessage) {
        let _ = self.tx.send(msg);
    }

    pub fn try_push_all(
        &self,
        msg: PushMessage,
    ) -> Result<usize, Box<broadcast::error::SendError<PushMessage>>> {
        self.tx.send(msg).map_err(Box::new)
    }
}

impl Default for PushHub {
    fn default() -> Self {
        Self::new()
    }
}

pub async fn sse_handler(
    State(state): State<Arc<AppState>>,
    CurrentUser(login_user): CurrentUser,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let user_id = login_user.user_id;
    let rx = state.services.push_hub.subscribe();
    let mut stream = BroadcastStream::new(rx);

    let stream = async_stream::stream! {
        let mut heartbeat = tokio::time::interval(Duration::from_secs(30));
        loop {
            tokio::select! {
                Some(Ok(msg)) = stream.next() => {
                    if msg.to_user_id == Some(user_id) || msg.to_user_id.is_none() {
                        let data = encode_push_message(&msg);
                        yield Ok(Event::default().data(data));
                    }
                }
                _ = heartbeat.tick() => {
                    yield Ok(Event::default().comment("ping"));
                }
            }
        }
    };

    Sse::new(stream).keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(30))
            .text("ping"),
    )
}

/// 序列化推送消息 —— 与 HTTP 信封同一套 ID 规则(`message_id` 是雪花通知 id)。
fn encode_push_message(msg: &PushMessage) -> String {
    match serde_json::to_value(msg) {
        Ok(mut value) => {
            stringify_ids(&mut value, None);
            value.to_string()
        }
        Err(_) => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_message_id_is_string() {
        let msg = PushMessage::notice(
            1_761_300_000_000_000_001,
            "测试".to_string(),
            serde_json::json!({ "noticeId": 42_i64 }),
            None,
        );
        let json: serde_json::Value =
            serde_json::from_str(&encode_push_message(&msg)).unwrap();
        assert_eq!(json["messageId"], "1761300000000000001");
        assert_eq!(json["data"]["noticeId"], "42");
        assert!(json["timestamp"].is_number(), "timestamp 不是 ID: {json}");
    }
}
