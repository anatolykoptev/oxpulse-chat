//! TDD test: /api/partner/register persists a row in partner_nodes and returns
//! a non-empty turns_subdomain.
//!
//! Gated on `TEST_DATABASE_URL` (graceful skip when not set).

mod common;

use common::partner as p;

#[tokio::test]
async fn register_persists_node_and_assigns_turns_subdomain() {
    p::set_env();
    let Some(pool) = p::setup_pool().await else {
        eprintln!("skipping: TEST_DATABASE_URL not set");
        return;
    };

    let raw = "ptkn_nodes_test_01";
    p::seed_token(
        &pool,
        "rvpn",
        raw,
        chrono::Utc::now() + chrono::Duration::hours(1),
    )
    .await;

    let server = p::build_server(pool.clone());

    let body = serde_json::json!({
        "partner_id": "rvpn",
        "domain": "call42.rvpn.online",
        "token": raw,
        "public_ip": "198.51.100.42",
    });

    let r = server.post("/api/partner/register").json(&body).await;
    assert_eq!(r.status_code().as_u16(), 201, "body={}", r.text());

    let v: serde_json::Value = r.json();
    let subdomain = v["turns_subdomain"]
        .as_str()
        .expect("turns_subdomain must be a string");
    assert!(
        !subdomain.is_empty(),
        "turns_subdomain must be non-empty, got: {v}"
    );
    assert!(
        subdomain.starts_with("api-"),
        "turns_subdomain must start with 'api-', got: {subdomain}"
    );

    // Verify the row was persisted in partner_nodes.
    let row: Option<(String, String, String)> = sqlx::query_as(
        "SELECT node_id, domain, turns_subdomain \
         FROM partner_nodes \
         WHERE partner_id = $1 AND domain = $2",
    )
    .bind("rvpn")
    .bind("call42.rvpn.online")
    .fetch_optional(&pool)
    .await
    .expect("query partner_nodes");

    let (db_node_id, db_domain, db_subdomain) = row.expect("partner_nodes row must exist");
    assert_eq!(db_domain, "call42.rvpn.online");
    assert_eq!(db_subdomain, subdomain, "DB turns_subdomain must match response");
    assert!(
        db_node_id.starts_with("rvpn-"),
        "node_id must be prefixed with partner_id, got: {db_node_id}"
    );
}

#[tokio::test]
async fn register_bad_domain_returns_400() {
    p::set_env();
    let Some(pool) = p::setup_pool().await else {
        eprintln!("skipping: TEST_DATABASE_URL not set");
        return;
    };

    let server = p::build_server(pool);

    // Empty domain.
    let r = server
        .post("/api/partner/register")
        .json(&serde_json::json!({
            "partner_id": "rvpn",
            "domain": "",
            "token": "any",
            "public_ip": "198.51.100.42",
        }))
        .await;
    assert_eq!(r.status_code().as_u16(), 400);
    let v: serde_json::Value = r.json();
    assert_eq!(v["code"], "bad_domain");

    // Domain > 253 chars.
    let long_domain = "a".repeat(254);
    let r2 = server
        .post("/api/partner/register")
        .json(&serde_json::json!({
            "partner_id": "rvpn",
            "domain": long_domain,
            "token": "any",
            "public_ip": "198.51.100.42",
        }))
        .await;
    assert_eq!(r2.status_code().as_u16(), 400);
    let v2: serde_json::Value = r2.json();
    assert_eq!(v2["code"], "bad_domain");
}
