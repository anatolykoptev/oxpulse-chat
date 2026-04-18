use std::net::{IpAddr, Ipv4Addr};
use std::sync::LazyLock;

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{ConnectInfo, Path, State};
use axum::http::HeaderMap;
use axum::response::IntoResponse;
use futures_util::{SinkExt, StreamExt};

use crate::rate_limit::JoinLimiter;
use crate::rooms::TaggedSignal;
use crate::types::{ClientMsg, ServerMsg};
use crate::Rooms;

/// Join timeout: client must send `{"type":"join"}` within this window.
const JOIN_TIMEOUT_SECS: u64 = 5;

/// Minimum/maximum inclusive length for a room id.
const MIN_ROOM_LEN: usize = 3;
const MAX_ROOM_LEN: usize = 32;

/// Process-global sliding-window join limiter (30 joins / 60s / IP).
/// Intentionally shared across all `ws_call_handler` invocations so we rate
/// limit per source IP regardless of which room is being joined.
static JOIN_LIMITER: LazyLock<JoinLimiter> = LazyLock::new(JoinLimiter::new);

/// Validate a room id against the hardened slug regex
/// `^[A-Za-z0-9][A-Za-z0-9-]{2,31}$` — permissive enough for both
/// `ABCD-1234` product codes and ad-hoc user slugs but strict enough
/// to bound entropy and forbid leading dashes, whitespace, and
/// path-ish characters.
///
/// Hand-rolled to avoid pulling in the `regex` crate for a single
/// pattern.
pub fn validate_room_id(room_id: &str) -> bool {
    let len = room_id.len();
    if !(MIN_ROOM_LEN..=MAX_ROOM_LEN).contains(&len) {
        return false;
    }
    let mut chars = room_id.chars();
    // First char: alphanumeric only (no leading dash).
    let Some(first) = chars.next() else {
        return false;
    };
    if !first.is_ascii_alphanumeric() {
        return false;
    }
    // Remaining chars: alphanumeric or '-'.
    chars.all(|c| c.is_ascii_alphanumeric() || c == '-')
}

/// Extract the client IP for rate-limit keying.
///
/// Priority:
///   1. `X-Real-IP` (set by Caddy upstream in prod).
///   2. `X-Forwarded-For` first hop.
///   3. `ConnectInfo<SocketAddr>` peer address.
///   4. `127.0.0.1` as a safe fallback — rate limit still applies on loopback.
///
/// No trust toggle is needed here because in dev / tests the ConnectInfo
/// path IS the real remote, and in prod Caddy always terminates upstream
/// and sets `X-Real-IP`.
pub(crate) fn extract_client_ip(headers: &HeaderMap, peer: Option<IpAddr>) -> IpAddr {
    if let Some(xri) = headers
        .get("x-real-ip")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.trim().parse().ok())
    {
        return xri;
    }
    if let Some(xff) = headers.get("x-forwarded-for").and_then(|v| v.to_str().ok()) {
        if let Some(first) = xff.split(',').next().and_then(|s| s.trim().parse().ok()) {
            return first;
        }
    }
    peer.unwrap_or(IpAddr::V4(Ipv4Addr::LOCALHOST))
}

pub async fn ws_call_handler(
    ws: WebSocketUpgrade,
    Path(room_id): Path<String>,
    State(rooms): State<Rooms>,
    headers: HeaderMap,
    ConnectInfo(peer_addr): ConnectInfo<std::net::SocketAddr>,
) -> impl IntoResponse {
    let client_ip = extract_client_ip(&headers, Some(peer_addr.ip()));
    ws.on_upgrade(move |socket| handle_call_ws(socket, room_id, rooms, client_ip))
}

async fn handle_call_ws(socket: WebSocket, room_id: String, rooms: Rooms, client_ip: IpAddr) {
    let metrics = rooms.metrics.clone();
    metrics.on_ws_connect();
    let _disconnect = DisconnectGuard { metrics: metrics.clone() };

    let (mut sink, mut stream) = socket.split();

    // Entropy guard: reject malformed room ids before even waiting for the
    // join message. Keeps log noise down and prevents trivial spray attacks
    // that probe random paths.
    if !validate_room_id(&room_id) {
        let msg = ServerMsg::Error {
            message: "invalid room id".into(),
        };
        let _ = sink.send(msg.to_text()).await;
        return;
    }

    // Per-IP sliding-window rate limit. Enforced BEFORE `wait_for_join` so
    // we never hold a 5s timer open for an IP that's already over quota.
    if !JOIN_LIMITER.check(client_ip) {
        let msg = ServerMsg::Error {
            message: "rate limit exceeded".into(),
        };
        let _ = sink.send(msg.to_text()).await;
        return;
    }

    if !wait_for_join(&mut stream).await {
        metrics.on_ws_handshake_failed();
        metrics.on_ws_join_err();
        let msg = ServerMsg::Error {
            message: "expected join message".into(),
        };
        let _ = sink.send(msg.to_text()).await;
        return;
    }

    let (tx, polite, peer_id) = match rooms.join(&room_id) {
        Some(triple) => triple,
        None => {
            metrics.on_ws_handshake_failed();
            metrics.on_ws_join_err();
            let msg = ServerMsg::Error {
                message: "Room is full".into(),
            };
            let _ = sink.send(msg.to_text()).await;
            return;
        }
    };

    if sink
        .send(ServerMsg::Joined { polite }.to_text())
        .await
        .is_err()
    {
        metrics.on_ws_handshake_failed();
        metrics.on_ws_join_err();
        rooms.leave(&room_id);
        return;
    }
    metrics.on_ws_join_ok();
    tracing::info!(room_id = %room_id, polite, peer_id, "call_peer_joined");

    // Subscribe to broadcast BEFORE sending PeerJoined so nothing is missed.
    let mut rx = tx.subscribe();
    let (fwd_tx, mut fwd_rx) = tokio::sync::mpsc::channel::<Message>(64);

    let send_task = tokio::spawn(async move {
        while let Some(msg) = fwd_rx.recv().await {
            if sink.send(msg).await.is_err() {
                break;
            }
        }
    });

    let fwd_tx2 = fwd_tx.clone();
    let recv_bcast = tokio::spawn(async move {
        while let Ok(tagged) = rx.recv().await {
            if tagged.from == peer_id {
                continue;
            }
            if fwd_tx2
                .send(Message::Text(tagged.payload.into()))
                .await
                .is_err()
            {
                break;
            }
        }
    });

    // Broadcast PeerJoined so the impolite peer knows to send a fresh offer.
    if polite {
        rooms.mark_connected(&room_id);
        let peer_joined = serde_json::to_string(&ServerMsg::PeerJoined)
            .unwrap_or_else(|_| r#"{"type":"error","message":"serialization failed"}"#.to_string());
        let _ = tx.send(TaggedSignal {
            from: peer_id,
            payload: peer_joined,
        });
    }

    process_messages(&mut stream, &tx, &fwd_tx, &room_id, peer_id).await;

    let connected_at = rooms.connected_at(&room_id);
    rooms.leave(&room_id);
    let peer_left = serde_json::to_string(&ServerMsg::PeerLeft)
        .unwrap_or_else(|_| r#"{"type":"error","message":"serialization failed"}"#.to_string());
    let _ = tx.send(TaggedSignal {
        from: peer_id,
        payload: peer_left,
    });
    recv_bcast.abort();
    drop(fwd_tx);
    let _ = send_task.await;

    if connected_at > 0 && rooms.try_mark_ended(&room_id) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        let duration_secs = (now - connected_at).max(0) as u64;
        metrics.on_call_ended(duration_secs as f64);
        tracing::info!(room_id = %room_id, duration_secs, "call_ended");
    } else {
        tracing::info!(room_id = %room_id, "call_peer_left");
    }
}

/// RAII guard — fires `on_ws_disconnect` when dropped (covers every exit path
/// including panics and early returns before `Joined`).
struct DisconnectGuard {
    metrics: std::sync::Arc<dyn crate::metrics::SignalingMetrics>,
}

impl Drop for DisconnectGuard {
    fn drop(&mut self) {
        self.metrics.on_ws_disconnect();
    }
}

async fn process_messages(
    stream: &mut futures_util::stream::SplitStream<WebSocket>,
    tx: &tokio::sync::broadcast::Sender<TaggedSignal>,
    fwd_tx: &tokio::sync::mpsc::Sender<Message>,
    room_id: &str,
    peer_id: u64,
) {
    while let Some(Ok(msg)) = stream.next().await {
        let text = match msg {
            Message::Text(t) => t.to_string(),
            Message::Pong(_) | Message::Ping(_) => continue,
            Message::Close(_) => break,
            _ => continue,
        };
        match serde_json::from_str::<ClientMsg>(&text) {
            Ok(ClientMsg::Signal { payload }) => {
                let sig_type = payload
                    .get("type")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown");
                tracing::info!(room_id = %room_id, peer_id, sig_type, "signal_relay");
                let out =
                    serde_json::to_string(&ServerMsg::Signal { payload }).unwrap_or_else(|_| {
                        r#"{"type":"error","message":"serialization failed"}"#.to_string()
                    });
                let _ = tx.send(TaggedSignal {
                    from: peer_id,
                    payload: out,
                });
            }
            Ok(ClientMsg::Leave) => break,
            Ok(ClientMsg::Ping) => {
                let _ = fwd_tx.send(ServerMsg::Pong.to_text()).await;
            }
            Ok(ClientMsg::Join) => {} // duplicate join, ignore
            Err(_) => {
                let err = ServerMsg::Error {
                    message: "invalid message".into(),
                };
                if fwd_tx.send(err.to_text()).await.is_err() {
                    break;
                }
            }
        }
    }
}

/// Wait for the first text message to be a Join. Returns false on timeout/error.
async fn wait_for_join(stream: &mut futures_util::stream::SplitStream<WebSocket>) -> bool {
    let timeout = std::time::Duration::from_secs(JOIN_TIMEOUT_SECS);
    let result = tokio::time::timeout(timeout, async {
        while let Some(Ok(msg)) = stream.next().await {
            let text = match msg {
                Message::Text(t) => t.to_string(),
                Message::Pong(_) | Message::Ping(_) => continue,
                Message::Close(_) => return false,
                _ => continue,
            };
            return matches!(
                serde_json::from_str::<ClientMsg>(&text),
                Ok(ClientMsg::Join)
            );
        }
        false
    })
    .await;
    result.unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_room_id_accepts_product_code() {
        assert!(validate_room_id("ABCD-1234"));
    }

    #[test]
    fn validate_room_id_accepts_custom_slug() {
        assert!(validate_room_id("room-foo-42"));
    }

    #[test]
    fn validate_room_id_accepts_min_length_alnum() {
        assert!(validate_room_id("test1"));
        assert!(validate_room_id("abc")); // exactly 3 chars
    }

    #[test]
    fn validate_room_id_accepts_max_length() {
        // exactly 32 chars, alphanumeric.
        assert!(validate_room_id("a123456789012345678901234567890b"));
    }

    #[test]
    fn validate_room_id_rejects_empty() {
        assert!(!validate_room_id(""));
    }

    #[test]
    fn validate_room_id_rejects_leading_dash() {
        assert!(!validate_room_id("-lead"));
    }

    #[test]
    fn validate_room_id_rejects_whitespace() {
        assert!(!validate_room_id("with space"));
    }

    #[test]
    fn validate_room_id_rejects_too_short() {
        assert!(!validate_room_id("ab")); // 2 chars
    }

    #[test]
    fn validate_room_id_rejects_too_long() {
        // 33 chars — one over the 32-char cap.
        assert!(!validate_room_id("toolongXXXXXXXXXXXXXXXXXXXXXXXXXX"));
    }

    #[test]
    fn validate_room_id_rejects_slash_and_dot() {
        assert!(!validate_room_id("a/b/c"));
        assert!(!validate_room_id("a.b.c"));
    }

    #[test]
    fn extract_client_ip_prefers_x_real_ip() {
        let mut h = HeaderMap::new();
        h.insert("x-real-ip", "203.0.113.7".parse().unwrap());
        h.insert(
            "x-forwarded-for",
            "198.51.100.1, 10.0.0.1".parse().unwrap(),
        );
        let peer = Some(IpAddr::V4("127.0.0.1".parse().unwrap()));
        let ip = extract_client_ip(&h, peer);
        assert_eq!(ip, "203.0.113.7".parse::<IpAddr>().unwrap());
    }

    #[test]
    fn extract_client_ip_falls_back_to_xff() {
        let mut h = HeaderMap::new();
        h.insert(
            "x-forwarded-for",
            "198.51.100.1, 10.0.0.1".parse().unwrap(),
        );
        let peer = Some(IpAddr::V4("127.0.0.1".parse().unwrap()));
        let ip = extract_client_ip(&h, peer);
        assert_eq!(ip, "198.51.100.1".parse::<IpAddr>().unwrap());
    }

    #[test]
    fn extract_client_ip_falls_back_to_peer() {
        let h = HeaderMap::new();
        let peer = Some(IpAddr::V4("192.0.2.99".parse().unwrap()));
        let ip = extract_client_ip(&h, peer);
        assert_eq!(ip, "192.0.2.99".parse::<IpAddr>().unwrap());
    }

    #[test]
    fn extract_client_ip_defaults_to_localhost() {
        let h = HeaderMap::new();
        let ip = extract_client_ip(&h, None);
        assert_eq!(ip, IpAddr::V4(Ipv4Addr::LOCALHOST));
    }
}
