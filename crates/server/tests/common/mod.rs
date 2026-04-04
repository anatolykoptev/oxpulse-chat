use std::net::SocketAddr;
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::Message;

pub type WsSink = futures_util::stream::SplitSink<
    tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >,
    Message,
>;

pub type WsStream = futures_util::stream::SplitStream<
    tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >,
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
