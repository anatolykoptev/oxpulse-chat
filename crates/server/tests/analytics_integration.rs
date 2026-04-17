//! Analytics DB integration test — POST /api/event end-to-end.
//!
//! Regression for the `source`/`data` bind-pairing bug (commit f36ddae):
//! the INSERT had 6 placeholders but only 5 bindings, silently dropping
//! every event row for a week. Gated on `TEST_DATABASE_URL`.

mod common;

use axum_test::TestServer;
use oxpulse_chat::router::{build_router, AppState};
use sqlx::postgres::PgPoolOptions;
use sqlx::Row;

/// Live end-to-end regression test — all six INSERT fields must land in
/// Postgres, in particular `data["referrer"]` which the old bug dropped.
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

    // Must match the body in `spa_fallback_and_static_files` — the router
    // caches `index.html` in a `OnceLock<String>`, so whichever test runs
    // first wins the cache. Keeping the content identical removes any
    // latent ordering dependency if SPA assertions tighten later.
    let dir = common::spa_tempdir();

    let state = AppState {
        pool: Some(pool.clone()),
        ..common::base_state()
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
