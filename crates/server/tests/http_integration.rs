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
use oxpulse_chat::turn_pool::TurnPool;
use oxpulse_signaling::Rooms;
use sqlx::postgres::PgPoolOptions;
use sqlx::Row;
use std::fs;
use std::time::Duration;
use tempfile::tempdir;

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

/// Regression test for the `source`/`data` bind-pairing bug in analytics ingest.
///
/// Commit f36ddae (7 days before 2026-04-10) added the `source` column to
/// `call_events` and updated the INSERT to 6 placeholders, but replaced
/// `.bind(&event.data)` with `.bind(&batch.source)` instead of appending it,
/// leaving only 5 bindings for 6 placeholders. Combined with `let _ = ...await`
/// swallowing the error, every insert silently failed for a week.
///
/// This test asserts end-to-end that all six fields — in particular
/// `data["referrer"]` — actually land in Postgres, so a future re-break is
/// caught instead of silently dropping rows.
#[tokio::test]
async fn analytics_insert_persists_all_fields() {
    let db_url = match std::env::var("TEST_DATABASE_URL") {
        Ok(v) if !v.is_empty() => v,
        _ => {
            eprintln!("skipping analytics_insert_persists_all_fields: TEST_DATABASE_URL not set");
            return;
        }
    };

    let pool = PgPoolOptions::new()
        .max_connections(2)
        .connect(&db_url)
        .await
        .expect("connect to TEST_DATABASE_URL");

    oxpulse_chat::migrate::run(&pool).await;

    sqlx::query("TRUNCATE call_events")
        .execute(&pool)
        .await
        .expect("truncate call_events");

    let dir = tempdir().unwrap();
    // Must match the body in `spa_fallback_and_static_files` — the router
    // caches `index.html` in a `OnceLock<String>`, so whichever test runs
    // first wins the cache. Keeping the content identical removes any
    // latent ordering dependency if SPA assertions tighten later.
    fs::write(
        dir.path().join("index.html"),
        "<html><head><title>OxPulse</title></head></html>",
    )
    .unwrap();

    let state = AppState {
        pool: Some(pool.clone()),
        ..test_state()
    };
    let app = build_router(state, dir.path().to_str().unwrap());
    let server = TestServer::new(app);

    let body = serde_json::json!({
        "did": "test-device-1",
        "src": "oxpulse.chat",
        "events": [
            { "e": "page_view", "r": null, "d": { "referrer": "t.me" } },
            { "e": "room_created", "r": "TEST-0001", "d": {} }
        ]
    });

    let response = server.post("/api/event").json(&body).await;
    assert_eq!(
        response.status_code(),
        axum::http::StatusCode::NO_CONTENT,
        "expected 204 No Content, got {}: {}",
        response.status_code(),
        response.text()
    );

    let rows = sqlx::query(
        "SELECT event_type, room_id, source, data FROM call_events ORDER BY event_type",
    )
    .fetch_all(&pool)
    .await
    .expect("fetch call_events rows");

    assert_eq!(
        rows.len(),
        2,
        "expected 2 rows inserted, got {} — the .bind() pairing is probably broken again",
        rows.len()
    );

    // Row[0] — page_view — THIS is the row whose `data` field the old bug
    // silently dropped. If `data["referrer"]` is missing, the regression is back.
    let ev0: String = rows[0].try_get("event_type").unwrap();
    assert_eq!(ev0, "page_view");
    let src0: String = rows[0].try_get("source").unwrap();
    assert_eq!(src0, "oxpulse.chat", "source column must persist batch.src");
    let data0: serde_json::Value = rows[0].try_get("data").unwrap();
    assert_eq!(
        data0.get("referrer").and_then(|v| v.as_str()),
        Some("t.me"),
        "data.referrer must round-trip through insert — regression of the \
         .bind(&event.data) drop bug (commit f36ddae)"
    );

    // Row[1] — room_created — confirms room_id binding still works.
    let ev1: String = rows[1].try_get("event_type").unwrap();
    assert_eq!(ev1, "room_created");
    let room1: Option<String> = rows[1].try_get("room_id").unwrap();
    assert_eq!(room1, Some("TEST-0001".to_string()));
    let src1: String = rows[1].try_get("source").unwrap();
    assert_eq!(src1, "oxpulse.chat");
}

/// Live end-to-end regression test for the room-link preview bug.
///
/// For 7 days `https://oxpulse.chat/{roomId}` returned `404` with no
/// content-type, so Telegram/iMessage link previewers rendered the URL as
/// a "file" instead of a rich card. Commit a0f4a4a made unknown paths
/// fall back to `200 text/html` serving the SvelteKit `index.html`, which
/// ships with full Open Graph meta tags.
///
/// This test hits the LIVE production URL (not a mock router) to assert
/// the fix is actually deployed on the real stack, not just green in unit
/// tests. Gated on `E2E_BASE_URL` so CI environments without outbound
/// network simply skip.
#[tokio::test]
async fn room_link_preview_returns_html_not_404() {
    let base_url = match std::env::var("E2E_BASE_URL") {
        Ok(v) if !v.is_empty() => v,
        _ => {
            eprintln!("skipping room_link_preview_returns_html_not_404: E2E_BASE_URL not set");
            return;
        }
    };

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .expect("build reqwest client");

    let url = format!("{}/TQFA-9412", base_url.trim_end_matches('/'));
    let res = client
        .get(&url)
        .send()
        .await
        .unwrap_or_else(|e| panic!("GET {url} failed: {e}"));

    let status = res.status();
    assert_eq!(
        status.as_u16(),
        200,
        "expected 200, got {status} for {url} — SPA fallback regression?"
    );

    let ct = res
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .unwrap_or_else(|| {
            panic!("content-type header missing on {url} — was 404 without type the bug")
        })
        .to_str()
        .expect("content-type is not valid ASCII")
        .to_lowercase();
    assert!(
        ct.starts_with("text/html"),
        "content-type must start with text/html for link previewers, got: {ct}"
    );

    let body = res.text().await.expect("read response body");
    assert!(
        body.contains("property=\"og:title\""),
        "body must contain og:title meta tag for Telegram/iMessage card rendering — \
         not found in {} bytes of body",
        body.len()
    );
    assert!(
        body.contains("property=\"og:image\""),
        "body must contain og:image meta tag for Telegram/iMessage card rendering — \
         not found in {} bytes of body",
        body.len()
    );
}
