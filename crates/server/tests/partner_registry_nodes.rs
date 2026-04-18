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

    // Malformed domain: leading dot.
    let r3 = server
        .post("/api/partner/register")
        .json(&serde_json::json!({
            "partner_id": "rvpn",
            "domain": ".foo.com",
            "token": "any",
            "public_ip": "198.51.100.42",
        }))
        .await;
    assert_eq!(r3.status_code().as_u16(), 400);
    let v3: serde_json::Value = r3.json();
    assert_eq!(v3["code"], "bad_domain");

    // Malformed domain: consecutive dots.
    let r4 = server
        .post("/api/partner/register")
        .json(&serde_json::json!({
            "partner_id": "rvpn",
            "domain": "foo..com",
            "token": "any",
            "public_ip": "198.51.100.42",
        }))
        .await;
    assert_eq!(r4.status_code().as_u16(), 400);
    let v4: serde_json::Value = r4.json();
    assert_eq!(v4["code"], "bad_domain");
}

#[tokio::test]
async fn re_registration_reuses_node_id_and_subdomain() {
    p::set_env();
    let Some(pool) = p::setup_pool().await else {
        eprintln!("skipping: TEST_DATABASE_URL not set");
        return;
    };
    // Seed TWO tokens for same partner.
    let raw1 = "ptkn_reg1";
    let raw2 = "ptkn_reg2";
    let exp = chrono::Utc::now() + chrono::Duration::hours(1);
    p::seed_token(&pool, "rvpn", raw1, exp).await;
    p::seed_token(&pool, "rvpn", raw2, exp).await;
    let server = p::build_server(pool.clone());
    let body1 = serde_json::json!({"partner_id": "rvpn", "domain": "call1.rvpn.online", "token": raw1, "public_ip": "198.51.100.1"});
    let body2 = serde_json::json!({"partner_id": "rvpn", "domain": "call1.rvpn.online", "token": raw2, "public_ip": "198.51.100.2"});
    let r1 = server.post("/api/partner/register").json(&body1).await;
    assert_eq!(r1.status_code().as_u16(), 201);
    let v1: serde_json::Value = r1.json();
    let r2 = server.post("/api/partner/register").json(&body2).await;
    assert_eq!(r2.status_code().as_u16(), 201);
    let v2: serde_json::Value = r2.json();
    assert_eq!(v1["node_id"], v2["node_id"], "node_id must be stable across re-registration");
    assert_eq!(v1["turns_subdomain"], v2["turns_subdomain"], "turns_subdomain must be stable");
    // Verify partner_nodes has exactly one row with these canonical values.
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM partner_nodes WHERE partner_id = $1 AND domain = $2")
        .bind("rvpn").bind("call1.rvpn.online").fetch_one(&pool).await.unwrap();
    assert_eq!(count, 1);
    // Verify the partner_nodes row has the canonical node_id matching what was returned.
    let db_node: String = sqlx::query_scalar("SELECT node_id FROM partner_nodes WHERE partner_id = $1 AND domain = $2")
        .bind("rvpn").bind("call1.rvpn.online").fetch_one(&pool).await.unwrap();
    assert_eq!(db_node, v1["node_id"].as_str().unwrap());
}
