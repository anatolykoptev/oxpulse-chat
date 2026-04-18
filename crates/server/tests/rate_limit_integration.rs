//! Integration test for Task 4.1 — per-IP rate limit on `/api/turn-credentials`.
//!
//! `TestApp::spawn()` wires the router behind `axum::serve(listener,
//! router.into_make_service_with_connect_info::<SocketAddr>())`, so the
//! `rate_limit_middleware` sees a real `ConnectInfo<SocketAddr>` and
//! keys the limiter on `127.0.0.1`.
//!
//! `/api/turn-credentials` is built with `make_limiter(30)` → 30/min
//! sustained, burst = 15. A tight loop of 35 requests from one IP must
//! therefore produce at least a handful of 429s (exact count depends on
//! how fast the token bucket replenishes during the loop), and at
//! least one 429 must carry a `Retry-After` header.
//!
//! Note: with `turn_secret = "test-secret"`, the handler returns 200
//! with a TURN credential body. With `turn_secret` empty it returns
//! 503, which would mask the rate limit behaviour — `TestApp::spawn`
//! already sets it, so we just reuse that.

mod common;

use common::TestApp;

#[tokio::test]
async fn api_rate_limit_kicks_in_on_burst() {
    let app = TestApp::spawn().await;
    let url = app.http_url("/api/turn-credentials");
    let client = reqwest::Client::new();

    let mut ok_count = 0usize;
    let mut too_many_count = 0usize;
    let mut retry_after_seen = false;

    for _ in 0..35 {
        let resp = client
            .post(&url)
            .send()
            .await
            .expect("POST /api/turn-credentials failed");
        match resp.status().as_u16() {
            200 => ok_count += 1,
            429 => {
                too_many_count += 1;
                if resp.headers().contains_key(reqwest::header::RETRY_AFTER) {
                    retry_after_seen = true;
                }
            }
            other => panic!("unexpected status {other}; expected 200 or 429"),
        }
    }

    assert!(
        ok_count > 0,
        "at least one request must pass; got ok={ok_count}, 429={too_many_count}"
    );
    assert!(
        too_many_count >= 5,
        "expected ≥5 of 35 burst requests to be rate-limited (429); got ok={ok_count}, 429={too_many_count}"
    );
    assert!(
        retry_after_seen,
        "at least one 429 response must carry a Retry-After header"
    );
}
