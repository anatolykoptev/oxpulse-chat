//! HTTP integration tests for the oxpulse-chat router.
//!
//! Regression test for the SPA fallback: unknown paths like `/TQFA-9412`
//! must return 200 with `text/html` body (so Telegram/iMessage previewers
//! render a link card), AND known static files must still be served
//! normally without being shadowed by the fallback.
//!
//! Both assertions live in a single test because `router::build_router`
//! caches `index.html` in a `OnceLock<String>`, so running separate tests
//! with different temp dirs would race on the cache.

use axum_test::TestServer;
use oxpulse_chat::router::{build_router, AppState};
use oxpulse_signaling::Rooms;
use std::fs;
use tempfile::tempdir;

fn test_state() -> AppState {
    AppState {
        rooms: Rooms::new(),
        turn_secret: String::new(),
        turn_urls: vec![],
        stun_urls: vec![],
        pool: None,
    }
}

#[tokio::test]
async fn spa_fallback_and_static_files() {
    let dir = tempdir().unwrap();
    fs::write(
        dir.path().join("index.html"),
        "<html><head><title>OxPulse</title></head></html>",
    )
    .unwrap();
    fs::write(dir.path().join("robots.txt"), "User-agent: *").unwrap();

    let app = build_router(test_state(), dir.path().to_str().unwrap());
    let server = TestServer::new(app);

    // Unknown path -> SPA fallback (200 + index.html + text/html).
    let response = server.get("/TQFA-9412").await;
    response.assert_status_ok();
    assert!(
        response.text().contains("<title>OxPulse</title>"),
        "SPA fallback body must contain index.html content, got: {}",
        response.text()
    );
    let ct = response.header("content-type");
    let ct_str = ct.to_str().unwrap();
    assert!(
        ct_str.starts_with("text/html"),
        "SPA fallback content-type must start with text/html, got: {ct_str}"
    );

    // Known static file must not be shadowed by the fallback.
    let response = server.get("/robots.txt").await;
    response.assert_status_ok();
    assert_eq!(response.text(), "User-agent: *");
}
