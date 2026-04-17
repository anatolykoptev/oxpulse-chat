mod common;

use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use tokio_tungstenite::tungstenite::Message;

use common::{connect, join_and_read, TestApp};

#[tokio::test]
async fn test_health_endpoint() {
    let app = TestApp::spawn().await;

    let resp = reqwest::get(&app.http_url("/api/health")).await.unwrap();
    assert_eq!(resp.status(), 200);
    assert_eq!(resp.text().await.unwrap(), "ok");
}

#[tokio::test]
async fn test_join_room_first_is_impolite() {
    let app = TestApp::spawn().await;
    let (mut tx, mut rx) = connect(&app.ws_url("room1")).await;
    let resp = join_and_read(&mut tx, &mut rx).await;
    assert_eq!(resp["type"], "joined");
    assert_eq!(resp["polite"], false);
}

#[tokio::test]
async fn test_join_room_second_is_polite() {
    let app = TestApp::spawn().await;

    let (mut tx1, mut rx1) = connect(&app.ws_url("room2")).await;
    let _ = join_and_read(&mut tx1, &mut rx1).await;

    let (mut tx2, mut rx2) = connect(&app.ws_url("room2")).await;
    let resp = join_and_read(&mut tx2, &mut rx2).await;
    assert_eq!(resp["type"], "joined");
    assert_eq!(resp["polite"], true);
}

#[tokio::test]
async fn test_signal_relay() {
    let app = TestApp::spawn().await;
    let url = app.ws_url("relay-room");

    let (mut tx_a, mut rx_a) = connect(&url).await;
    let _ = join_and_read(&mut tx_a, &mut rx_a).await;

    let (mut tx_b, mut rx_b) = connect(&url).await;
    let _ = join_and_read(&mut tx_b, &mut rx_b).await;

    let signal = r#"{"type":"signal","payload":{"sdp":"test-offer"}}"#;
    tx_a.send(Message::Text(signal.into())).await.unwrap();

    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    let mut got_signal = false;
    while tokio::time::Instant::now() < deadline {
        let msg = tokio::time::timeout(Duration::from_secs(2), rx_b.next()).await;
        match msg {
            Ok(Some(Ok(m))) => {
                let val: serde_json::Value = serde_json::from_str(m.to_text().unwrap()).unwrap();
                if val["type"] == "signal" {
                    assert_eq!(val["payload"]["sdp"], "test-offer");
                    got_signal = true;
                    break;
                }
            }
            _ => break,
        }
    }
    assert!(got_signal, "peer B did not receive the relayed signal");
}

#[tokio::test]
async fn test_room_full_rejection() {
    let app = TestApp::spawn().await;
    let url = app.ws_url("full-room");

    let (mut tx1, mut rx1) = connect(&url).await;
    let _ = join_and_read(&mut tx1, &mut rx1).await;

    let (mut tx2, mut rx2) = connect(&url).await;
    let _ = join_and_read(&mut tx2, &mut rx2).await;

    let (mut tx3, mut rx3) = connect(&url).await;
    tx3.send(Message::Text(r#"{"type":"join"}"#.into()))
        .await
        .unwrap();

    let msg = tokio::time::timeout(Duration::from_secs(5), rx3.next())
        .await
        .expect("timeout")
        .expect("stream ended")
        .expect("ws error");

    let val: serde_json::Value = serde_json::from_str(msg.to_text().unwrap()).unwrap();
    assert_eq!(val["type"], "error");
}

#[tokio::test]
async fn test_peer_left_notification() {
    let app = TestApp::spawn().await;
    let url = app.ws_url("leave-room");

    let (mut tx_a, mut rx_a) = connect(&url).await;
    let _ = join_and_read(&mut tx_a, &mut rx_a).await;

    let (mut tx_b, mut rx_b) = connect(&url).await;
    let _ = join_and_read(&mut tx_b, &mut rx_b).await;

    tx_a.send(Message::Text(r#"{"type":"leave"}"#.into()))
        .await
        .unwrap();
    drop(tx_a);

    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    let mut got_left = false;
    while tokio::time::Instant::now() < deadline {
        let msg = tokio::time::timeout(Duration::from_secs(2), rx_b.next()).await;
        match msg {
            Ok(Some(Ok(m))) => {
                if let Ok(val) = serde_json::from_str::<serde_json::Value>(m.to_text().unwrap()) {
                    if val["type"] == "peer_left" {
                        got_left = true;
                        break;
                    }
                }
            }
            _ => break,
        }
    }
    assert!(got_left, "peer B did not receive peer_left notification");
}

#[tokio::test]
async fn test_turn_credentials_endpoint() {
    let app = TestApp::spawn().await;

    let client = reqwest::Client::new();
    let resp = client
        .post(app.http_url("/api/turn-credentials"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(body.get("username").is_some());
    assert!(body.get("credential").is_some());
    assert!(body.get("ttl").is_some());
    assert!(body.get("ice_servers").is_some());

    let ice = body["ice_servers"].as_array().unwrap();
    assert!(!ice.is_empty());
}
