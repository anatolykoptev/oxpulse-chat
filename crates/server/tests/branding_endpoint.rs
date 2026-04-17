//! Integration tests for GET /api/branding — host-based branding resolution.

use axum_test::TestServer;
use oxpulse_chat::router::{build_router, AppState};
use oxpulse_chat::turn_pool::TurnPool;
use oxpulse_signaling::Rooms;

fn test_state() -> AppState {
    AppState {
        rooms: Rooms::new(),
        turn_secret: String::new(),
        turn_urls: vec![],
        stun_urls: vec![],
        pool: None,
        turn_pool: TurnPool::new(vec![]),
    }
}

fn test_server() -> TestServer {
    let app = build_router(test_state(), "/nonexistent");
    TestServer::new(app)
}

#[tokio::test]
async fn branding_known_host_returns_matching_config() {
    let server = test_server();

    let response = server
        .get("/api/branding")
        .add_header(
            axum::http::HeaderName::from_static("host"),
            axum::http::HeaderValue::from_static("oxpulse.chat"),
        )
        .await;

    response.assert_status_ok();
    let body: serde_json::Value = response.json();
    assert_eq!(body["partner_id"], "oxpulse");
}

#[tokio::test]
async fn branding_unknown_host_returns_default() {
    let server = test_server();

    let response = server
        .get("/api/branding")
        .add_header(
            axum::http::HeaderName::from_static("host"),
            axum::http::HeaderValue::from_static("unknown.example"),
        )
        .await;

    response.assert_status_ok();
    let body: serde_json::Value = response.json();
    assert_eq!(body["partner_id"], "oxpulse");
}

#[tokio::test]
async fn branding_x_forwarded_host_takes_priority() {
    let server = test_server();

    let response = server
        .get("/api/branding")
        .add_header(
            axum::http::HeaderName::from_static("x-forwarded-host"),
            axum::http::HeaderValue::from_static("oxpulse.chat"),
        )
        .add_header(
            axum::http::HeaderName::from_static("host"),
            axum::http::HeaderValue::from_static("some-proxy.internal"),
        )
        .await;

    response.assert_status_ok();
    let body: serde_json::Value = response.json();
    assert_eq!(body["partner_id"], "oxpulse");
}
