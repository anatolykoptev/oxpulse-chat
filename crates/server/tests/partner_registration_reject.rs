//! Rejection paths for POST /api/partner/register — one test per error
//! code to keep regressions pinpoint-findable.

mod common;

use common::partner as p;
use oxpulse_chat::partner_registry::hash_token;

async fn reject(
    partner_in_token: &str,
    partner_in_body: &str,
    raw: &str,
    seed_expires: chrono::Duration,
    revoke: bool,
    expected_status: u16,
    expected_code: &str,
) {
    p::set_env();
    let Some(pool) = p::setup_pool().await else {
        eprintln!("skipping: TEST_DATABASE_URL not set");
        return;
    };
    if !partner_in_token.is_empty() {
        p::seed_token(
            &pool,
            partner_in_token,
            raw,
            chrono::Utc::now() + seed_expires,
        )
        .await;
        if revoke {
            sqlx::query("UPDATE partner_tokens SET revoked_at = NOW() WHERE token_hash = $1")
                .bind(hash_token(raw))
                .execute(&pool)
                .await
                .unwrap();
        }
    }
    let server = p::build_server(pool);
    let body = serde_json::json!({
        "partner_id": partner_in_body,
        "domain": "call.example.test",
        "token": raw,
        "public_ip": "203.0.113.5",
    });
    let r = server.post("/api/partner/register").json(&body).await;
    assert_eq!(
        r.status_code().as_u16(),
        expected_status,
        "body={}",
        r.text()
    );
    let v: serde_json::Value = r.json();
    assert_eq!(v["code"], expected_code);
}

#[tokio::test]
async fn rejects_expired_token() {
    reject(
        "acme",
        "acme",
        "ptkn_expired",
        chrono::Duration::hours(-1),
        false,
        403,
        "token_expired",
    )
    .await;
}

#[tokio::test]
async fn rejects_revoked_token() {
    reject(
        "acme",
        "acme",
        "ptkn_revoked",
        chrono::Duration::hours(1),
        true,
        403,
        "token_revoked",
    )
    .await;
}

#[tokio::test]
async fn rejects_partner_mismatch() {
    reject(
        "acme",
        "other",
        "ptkn_mismatch",
        chrono::Duration::hours(1),
        false,
        403,
        "partner_mismatch",
    )
    .await;
}

#[tokio::test]
async fn rejects_unknown_token() {
    reject(
        "",
        "acme",
        "ptkn_unknown",
        chrono::Duration::hours(1),
        false,
        403,
        "token_not_found",
    )
    .await;
}
