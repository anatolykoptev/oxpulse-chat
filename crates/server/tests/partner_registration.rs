//! Happy path + replay regression for POST /api/partner/register.
//!
//! Gated on `TEST_DATABASE_URL` (same pattern as analytics_integration.rs).
//! Error-path coverage lives in `partner_registration_reject.rs`.

mod common;

use common::partner as p;
use oxpulse_chat::partner_registry::hash_token;

#[tokio::test]
async fn register_succeeds_once_then_replays_as_conflict() {
    p::set_env();
    let Some(pool) = p::setup_pool().await else {
        eprintln!("skipping: TEST_DATABASE_URL not set");
        return;
    };
    let raw = "ptkn_test_ok";
    p::seed_token(
        &pool,
        "acme",
        raw,
        chrono::Utc::now() + chrono::Duration::hours(1),
    )
    .await;
    let server = p::build_server(pool.clone());

    let body = serde_json::json!({
        "partner_id": "acme",
        "domain": "call.acme.test",
        "token": raw,
        "public_ip": "203.0.113.5",
    });

    let r1 = server.post("/api/partner/register").json(&body).await;
    assert_eq!(r1.status_code().as_u16(), 201, "body={}", r1.text());
    let v1: serde_json::Value = r1.json();
    assert!(
        v1["node_id"].as_str().unwrap().starts_with("acme-"),
        "node_id should be prefixed with partner: {v1}"
    );
    assert_eq!(v1["reality_public_key"], p::REALITY_PUBLIC_KEY);
    assert_eq!(v1["turn_secret"], p::TURN_SECRET);

    let used_at: Option<chrono::DateTime<chrono::Utc>> =
        sqlx::query_scalar("SELECT used_at FROM partner_tokens WHERE token_hash = $1")
            .bind(hash_token(raw))
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(used_at.is_some(), "used_at must be filled after register");

    let r2 = server.post("/api/partner/register").json(&body).await;
    assert_eq!(r2.status_code().as_u16(), 409, "body={}", r2.text());
    let v2: serde_json::Value = r2.json();
    assert_eq!(v2["code"], "token_already_used");
}
