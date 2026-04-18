//! Integration tests for the /metrics endpoint (Task 3.1).
//!
//! Verifies the token-gated contract:
//!   - AppState::metrics_token empty  → 401 (endpoint effectively disabled)
//!   - wrong X-Internal-Token         → 401
//!   - correct X-Internal-Token       → 200, Prometheus text format body

mod common;

use axum_test::TestServer;
use oxpulse_chat::router::{build_router, AppState};

fn make_server(token: &str) -> TestServer {
    let mut state: AppState = common::base_state();
    state.metrics_token = token.to_string();
    let dir = common::spa_tempdir();
    let app = build_router(state, dir.path().to_str().unwrap());
    // Leak tempdir — must outlive the server for the test.
    std::mem::forget(dir);
    TestServer::new(app)
}

#[tokio::test]
async fn metrics_requires_configured_token() {
    let server = make_server("");
    let resp = server.get("/metrics").await;
    resp.assert_status(axum::http::StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn metrics_rejects_wrong_token() {
    let server = make_server("secret-abc");
    let resp = server
        .get("/metrics")
        .add_header("x-internal-token", "wrong-token")
        .await;
    resp.assert_status(axum::http::StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn metrics_serves_prometheus_text_on_valid_token() {
    let server = make_server("valid-xyz");
    let resp = server
        .get("/metrics")
        .add_header("x-internal-token", "valid-xyz")
        .await;
    resp.assert_status_ok();
    let ct = resp.header("content-type");
    assert!(
        ct.to_str().unwrap().starts_with("text/plain"),
        "expected text/plain, got {ct:?}"
    );
    let body = resp.text();
    for name in [
        "rooms_active",
        "ws_connects_total",
        "turn_servers_healthy",
        "turn_creds_issued_total",
        "turn_cred_latency_seconds",
    ] {
        assert!(body.contains(name), "missing metric {name} in body: {body}");
    }
}
