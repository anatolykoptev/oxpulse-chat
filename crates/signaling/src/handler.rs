use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Path, State};
use axum::response::IntoResponse;
use futures_util::{SinkExt, StreamExt};

use crate::rooms::TaggedSignal;
use crate::types::{ClientMsg, ServerMsg};
use crate::Rooms;

/// Join timeout: client must send `{"type":"join"}` within this window.
const JOIN_TIMEOUT_SECS: u64 = 5;

pub async fn ws_call_handler(
    ws: WebSocketUpgrade,
    Path(room_id): Path<String>,
    State(rooms): State<Rooms>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_call_ws(socket, room_id, rooms))
}

async fn handle_call_ws(socket: WebSocket, room_id: String, rooms: Rooms) {
    let (mut sink, mut stream) = socket.split();

    if !wait_for_join(&mut stream).await {
        let msg = ServerMsg::Error {
            message: "expected join message".into(),
        };
        let _ = sink.send(msg.to_text()).await;
        return;
    }

    let (tx, polite, peer_id) = match rooms.join(&room_id) {
        Some(triple) => triple,
        None => {
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
        rooms.leave(&room_id);
        return;
    }
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
        tracing::info!(room_id = %room_id, duration_secs, "call_ended");
    } else {
        tracing::info!(room_id = %room_id, "call_peer_left");
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
