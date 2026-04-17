//! Integration tests for POST /api/turn-credentials — Task 2.4.
//!
//! The handler must:
//! 1. Serve only the healthy subset of the dynamic `turn_pool`, sorted by
//!    priority ascending, when at least one pool server is healthy.
//! 2. Fall back to the static `turn_urls` list (backward compat) when the
//!    pool is empty OR every pool server is currently unhealthy.
//! 3. Return `503` when `turn_secret` is unset (unchanged from Task 2.3).

use std::fs;
use std::sync::atomic::Ordering;

use axum_test::TestServer;
use oxpulse_chat::config::TurnServerCfg;
use oxpulse_chat::router::{build_router, AppState};
use oxpulse_chat::turn_pool::TurnPool;
use oxpulse_signaling::Rooms;
use tempfile::tempdir;

fn base_state() -> AppState {
    AppState {
        rooms: Rooms::new(),
        turn_secret: String::new(),
        turn_urls: vec![],
        stun_urls: vec![],
        pool: None,
        turn_pool: TurnPool::new(vec![]),
    }
}

/// Collect every ICE url from the handler response body.
fn extract_urls(body: &serde_json::Value) -> Vec<String> {
    body["ice_servers"]
        .as_array()
        .expect("ice_servers must be array")
        .iter()
        .flat_map(|s| {
            s["urls"]
                .as_array()
                .map(|arr| {
                    arr.iter()
                        .filter_map(|u| u.as_str().map(String::from))
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default()
        })
        .collect()
}

#[tokio::test]
async fn filters_unhealthy_servers() {
    let pool = TurnPool::new(vec![
        TurnServerCfg {
            url: "turn:healthy.example:3478".into(),
            region: "ru-msk".into(),
            priority: 0,
        },
        TurnServerCfg {
            url: "turn:unhealthy.example:3478".into(),
            region: "de-fra".into(),
            priority: 1,
        },
    ]);
    // Flip the second server to unhealthy directly (bypass the probe loop).
    pool.all()[1].healthy.store(false, Ordering::Relaxed);

    let dir = tempdir().unwrap();
    fs::write(
        dir.path().join("index.html"),
        "<html><head><title>OxPulse</title></head></html>",
    )
    .unwrap();

    let state = AppState {
        turn_secret: "test-secret".into(),
        turn_urls: vec!["turn:fallback.example:3478".into()],
        turn_pool: pool,
        ..base_state()
    };
    let app = build_router(state, dir.path().to_str().unwrap());
    let server = TestServer::new(app);

    let response = server.post("/api/turn-credentials").await;
    response.assert_status_ok();
    let body: serde_json::Value = response.json();
    let urls = extract_urls(&body);

    assert!(
        urls.contains(&"turn:healthy.example:3478".to_string()),
        "healthy TURN URL must appear in ice_servers, got: {urls:?}"
    );
    assert!(
        !urls.contains(&"turn:unhealthy.example:3478".to_string()),
        "unhealthy TURN URL must be filtered out, got: {urls:?}"
    );
    assert!(
        !urls.contains(&"turn:fallback.example:3478".to_string()),
        "static fallback must NOT be used while at least one pool server is healthy, got: {urls:?}"
    );
}

#[tokio::test]
async fn falls_back_when_no_healthy_servers() {
    let pool = TurnPool::new(vec![TurnServerCfg {
        url: "turn:dead.example:3478".into(),
        region: "ru-msk".into(),
        priority: 0,
    }]);
    pool.all()[0].healthy.store(false, Ordering::Relaxed);

    let dir = tempdir().unwrap();
    fs::write(
        dir.path().join("index.html"),
        "<html><head><title>OxPulse</title></head></html>",
    )
    .unwrap();

    let state = AppState {
        turn_secret: "test-secret".into(),
        turn_urls: vec!["turn:fallback.example:3478".into()],
        turn_pool: pool,
        ..base_state()
    };
    let app = build_router(state, dir.path().to_str().unwrap());
    let server = TestServer::new(app);

    let response = server.post("/api/turn-credentials").await;
    response.assert_status_ok();
    let body: serde_json::Value = response.json();
    let urls = extract_urls(&body);

    assert!(
        urls.contains(&"turn:fallback.example:3478".to_string()),
        "fallback TURN URL must be used when pool has no healthy servers, got: {urls:?}"
    );
    assert!(
        !urls.contains(&"turn:dead.example:3478".to_string()),
        "dead pool server must not appear in ice_servers, got: {urls:?}"
    );
}

#[tokio::test]
async fn empty_pool_falls_back_to_static_urls() {
    // Zero-server pool should behave identically to "all unhealthy".
    let dir = tempdir().unwrap();
    fs::write(
        dir.path().join("index.html"),
        "<html><head><title>OxPulse</title></head></html>",
    )
    .unwrap();

    let state = AppState {
        turn_secret: "test-secret".into(),
        turn_urls: vec!["turn:fallback.example:3478".into()],
        turn_pool: TurnPool::new(vec![]),
        ..base_state()
    };
    let app = build_router(state, dir.path().to_str().unwrap());
    let server = TestServer::new(app);

    let response = server.post("/api/turn-credentials").await;
    response.assert_status_ok();
    let body: serde_json::Value = response.json();
    let urls = extract_urls(&body);

    assert!(
        urls.contains(&"turn:fallback.example:3478".to_string()),
        "empty pool must fall back to static turn_urls, got: {urls:?}"
    );
}

#[tokio::test]
async fn sorts_healthy_servers_by_priority_ascending() {
    // Insert out-of-order to prove the handler sorts rather than relying on
    // configuration order.
    let pool = TurnPool::new(vec![
        TurnServerCfg {
            url: "turn:low-prio.example:3478".into(),
            region: "de-fra".into(),
            priority: 5,
        },
        TurnServerCfg {
            url: "turn:high-prio.example:3478".into(),
            region: "ru-msk".into(),
            priority: 0,
        },
    ]);

    let dir = tempdir().unwrap();
    fs::write(
        dir.path().join("index.html"),
        "<html><head><title>OxPulse</title></head></html>",
    )
    .unwrap();

    let state = AppState {
        turn_secret: "test-secret".into(),
        turn_urls: vec![],
        turn_pool: pool,
        ..base_state()
    };
    let app = build_router(state, dir.path().to_str().unwrap());
    let server = TestServer::new(app);

    let response = server.post("/api/turn-credentials").await;
    response.assert_status_ok();
    let body: serde_json::Value = response.json();
    let urls = extract_urls(&body);

    let hi_idx = urls
        .iter()
        .position(|u| u == "turn:high-prio.example:3478")
        .expect("high-prio URL must appear");
    let lo_idx = urls
        .iter()
        .position(|u| u == "turn:low-prio.example:3478")
        .expect("low-prio URL must appear");
    assert!(
        hi_idx < lo_idx,
        "priority=0 must appear before priority=5, got urls: {urls:?}"
    );
}
