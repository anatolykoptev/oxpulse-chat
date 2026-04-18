// Each test binary includes only the helpers it uses; items used by other
// binaries appear dead to that binary's linter pass. Suppress globally for
// this shared helper module.
#![allow(dead_code)]

pub mod partner;

use std::net::SocketAddr;
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::Message;

/// Minimal AppState with empty TURN config — shared by axum-test integration tests.
pub fn base_state() -> oxpulse_chat::router::AppState {
    oxpulse_chat::router::AppState {
        rooms: oxpulse_signaling::Rooms::new(),
        turn_secret: String::new(),
        turn_urls: vec![],
        stun_urls: vec![],
        pool: None,
        turn_pool: oxpulse_chat::turn_pool::TurnPool::empty(),
    }
}

/// Create a tempdir containing a minimal `index.html` for router tests that
/// exercise the SPA fallback. The returned `TempDir` must be kept alive for
/// the duration of the test — dropping it deletes the directory.
pub fn spa_tempdir() -> tempfile::TempDir {
    let dir = tempfile::TempDir::new().expect("create tempdir");
    std::fs::write(
        dir.path().join("index.html"),
        "<html><head><title>OxPulse</title></head></html>",
    )
    .expect("write index.html");
    dir
}

pub type WsSink = futures_util::stream::SplitSink<
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
    Message,
>;

pub type WsStream = futures_util::stream::SplitStream<
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
>;

pub struct TestApp {
    pub addr: SocketAddr,
}

impl TestApp {
    pub async fn spawn() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let state = oxpulse_chat::router::AppState {
            rooms: oxpulse_signaling::Rooms::new(),
            turn_secret: "test-secret".into(),
            turn_urls: vec!["turn:test:3478".into()],
            stun_urls: vec!["stun:stun.l.google.com:19302".into()],
            pool: None,
            turn_pool: oxpulse_chat::turn_pool::TurnPool::empty(),
        };

        let router = oxpulse_chat::router::build_router(state, "/nonexistent");

        tokio::spawn(async move {
            axum::serve(listener, router).await.unwrap();
        });

        TestApp { addr }
    }

    pub fn ws_url(&self, room_id: &str) -> String {
        format!("ws://{}/ws/call/{}", self.addr, room_id)
    }

    pub fn http_url(&self, path: &str) -> String {
        format!("http://{}{}", self.addr, path)
    }
}

/// Connect to a WebSocket URL, returning split sink and stream.
pub async fn connect(url: &str) -> (WsSink, WsStream) {
    let (ws, _) = tokio_tungstenite::connect_async(url).await.unwrap();
    ws.split()
}

/// Send join message and return the parsed JSON response.
pub async fn join_and_read(ws: &mut WsSink, rx: &mut WsStream) -> serde_json::Value {
    ws.send(Message::Text(r#"{"type":"join"}"#.into()))
        .await
        .unwrap();
    let msg = tokio::time::timeout(Duration::from_secs(5), rx.next())
        .await
        .expect("timeout waiting for response")
        .expect("stream ended")
        .expect("ws error");
    serde_json::from_str(msg.to_text().unwrap()).unwrap()
}
