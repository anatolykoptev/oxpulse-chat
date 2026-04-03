use axum::extract::ws::Message;
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(crate) enum ClientMsg {
    Join,
    Signal { payload: serde_json::Value },
    Leave,
    Ping,
}

#[derive(Serialize, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(crate) enum ServerMsg {
    Joined { polite: bool },
    Signal { payload: serde_json::Value },
    PeerJoined,
    PeerLeft,
    Error { message: String },
    Pong,
}

impl ServerMsg {
    pub(crate) fn to_text(&self) -> Message {
        let json = serde_json::to_string(self)
            .unwrap_or_else(|_| r#"{"type":"error","message":"serialization failed"}"#.to_string());
        Message::Text(json.into())
    }
}
