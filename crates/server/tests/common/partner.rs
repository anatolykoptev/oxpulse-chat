//! Partner-registration test helpers — DB setup, token seeding, env setup.
//!
//! Shared between `partner_registration.rs` (happy path + replay) and
//! `partner_registration_reject.rs` (error-path coverage).

#![allow(dead_code)]

use axum_test::TestServer;
use oxpulse_chat::partner_registry::hash_token;
use oxpulse_chat::router::{build_router, AppState};
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;

pub const REALITY_PUBLIC_KEY: &str = "test-reality-public-key";
pub const REALITY_SHORT_ID: &str = "deadbeef";
pub const TURN_SECRET: &str = "test-turn-secret-for-registration";

pub fn set_env() {
    std::env::set_var(
        "PARTNER_REALITY_UUID",
        "00000000-0000-4000-8000-000000000001",
    );
    std::env::set_var("PARTNER_REALITY_PUBLIC_KEY", REALITY_PUBLIC_KEY);
    std::env::set_var("PARTNER_REALITY_SHORT_ID", REALITY_SHORT_ID);
    std::env::set_var("PARTNER_REALITY_SERVER_NAME", "www.samsung.com");
    std::env::set_var("TURN_SECRET", TURN_SECRET);
}

pub async fn setup_pool() -> Option<PgPool> {
    let db_url = match std::env::var("TEST_DATABASE_URL") {
        Ok(v) if !v.is_empty() => v,
        _ => return None,
    };
    let pool = PgPoolOptions::new()
        .max_connections(2)
        .connect(&db_url)
        .await
        .expect("connect to TEST_DATABASE_URL");
    oxpulse_chat::migrate::run(&pool).await;
    sqlx::query("TRUNCATE partner_tokens, partner_nodes")
        .execute(&pool)
        .await
        .expect("truncate partner_tokens, partner_nodes");
    Some(pool)
}

pub async fn seed_token(
    pool: &PgPool,
    partner: &str,
    raw: &str,
    expires_at: chrono::DateTime<chrono::Utc>,
) {
    sqlx::query(
        "INSERT INTO partner_tokens (partner_id, token_hash, expires_at) \
         VALUES ($1, $2, $3)",
    )
    .bind(partner)
    .bind(hash_token(raw))
    .bind(expires_at)
    .execute(pool)
    .await
    .expect("seed token");
}

pub fn build_server(pool: PgPool) -> TestServer {
    let state = AppState {
        pool: Some(pool),
        ..super::base_state()
    };
    let app = build_router(state, "/nonexistent");
    TestServer::new(app)
}
