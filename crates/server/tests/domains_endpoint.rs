//! Integration tests for GET /api/domains.

use axum_test::TestServer;
use oxpulse_chat::router::{build_router, AppState};
use oxpulse_signaling::Rooms;

fn test_state() -> AppState {
    AppState {
        rooms: Rooms::new(),
        turn_secret: String::new(),
        turn_urls: vec![],
        stun_urls: vec![],
        pool: None,
    }
}

fn test_server() -> TestServer {
    let app = build_router(test_state(), "/nonexistent");
    TestServer::new(app)
}

#[tokio::test]
async fn domains_rvpn_host_returns_correct_primary_and_mirrors() {
    let server = test_server();

    let response = server
        .get("/api/domains")
        .add_header(
            axum::http::HeaderName::from_static("host"),
            axum::http::HeaderValue::from_static("call.rvpn.online"),
        )
        .await;

    response.assert_status_ok();
    let body: serde_json::Value = response.json();
    assert_eq!(body["primary"], "call.rvpn.online");
    assert_eq!(body["config_version"], 1);

    let mirrors = body["mirrors"].as_array().expect("mirrors must be array");
    let mirror_strs: Vec<&str> = mirrors
        .iter()
        .filter_map(|v| v.as_str())
        .collect();

    assert!(
        mirror_strs.contains(&"call1.rvpn.online"),
        "mirrors must include call1.rvpn.online"
    );
    assert!(
        mirror_strs.contains(&"call2.rvpn.online"),
        "mirrors must include call2.rvpn.online"
    );
    assert!(
        !mirror_strs.contains(&"call.rvpn.online"),
        "primary must not appear in mirrors"
    );
}

#[tokio::test]
async fn domains_oxpulse_host_returns_oxpulse_primary() {
    let server = test_server();

    let response = server
        .get("/api/domains")
        .add_header(
            axum::http::HeaderName::from_static("x-forwarded-host"),
            axum::http::HeaderValue::from_static("oxpulse.chat"),
        )
        .await;

    response.assert_status_ok();
    let body: serde_json::Value = response.json();
    assert_eq!(body["primary"], "oxpulse.chat");
    let mirrors = body["mirrors"].as_array().expect("mirrors must be array");
    assert!(
        !mirrors.iter().any(|v| v.as_str() == Some("localhost")),
        "localhost must not appear in mirrors"
    );
}
