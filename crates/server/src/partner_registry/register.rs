//! Registration business logic — SQL transaction + validity checks.
//!
//! `register()` looks up a token by its sha256 hash, enforces "not used
//! / not revoked / not expired / partner matches", flips the row to
//! used, and returns the fully-assembled `RegistrationOk` body.

use std::net::IpAddr;

use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use super::creds::{load_reality_from_env, short_random_hex};
use super::error::RegistrationError;

/// Reality (VLESS / xray) credentials returned to the edge-node on success.
#[derive(Debug, Clone, Serialize)]
pub struct RealityCreds {
    pub reality_uuid: String,
    pub reality_public_key: String,
    pub reality_short_id: String,
    pub reality_server_name: String,
}

/// Successful registration response body.
#[derive(Debug, Clone, Serialize)]
pub struct RegistrationOk {
    pub node_id: String,
    pub backend_endpoint: String,
    #[serde(flatten)]
    pub reality: RealityCreds,
    pub turn_secret: String,
    pub turns_subdomain: String,
    pub config_version: u64,
}

/// Request body for `POST /api/partner/register`.
#[derive(Debug, Deserialize)]
pub struct RegisterRequest {
    pub partner_id: String,
    pub domain: String,
    pub token: String,
    /// Edge-node's self-reported public IP. Used when no trusted
    /// `X-Forwarded-For` header is present.
    #[serde(default)]
    pub public_ip: Option<IpAddr>,
}

type TokenRow = (
    Uuid,
    String,
    chrono::DateTime<chrono::Utc>,
    Option<chrono::DateTime<chrono::Utc>>,
    Option<chrono::DateTime<chrono::Utc>>,
);

/// Look up a token by its sha256 hash, enforce validity, flip the row to
/// "used", and persist the edge-node in `partner_nodes` — all in one
/// transaction. Returns the response body including the assigned
/// `turns_subdomain`.
pub async fn register(
    pool: &PgPool,
    partner_id: &str,
    domain: &str,
    token: &str,
    public_ip: IpAddr,
) -> Result<RegistrationOk, RegistrationError> {
    let token_hash = super::creds::hash_token(token);

    let reality = load_reality_from_env()?;
    let turn_secret =
        std::env::var("TURN_SECRET").map_err(|_| RegistrationError::TurnNotConfigured)?;
    if turn_secret.is_empty() {
        return Err(RegistrationError::TurnNotConfigured);
    }

    let mut tx = pool.begin().await?;

    let row: Option<TokenRow> = sqlx::query_as(
        "SELECT token_id, partner_id, expires_at, used_at, revoked_at \
         FROM partner_tokens \
         WHERE token_hash = $1 \
         FOR UPDATE",
    )
    .bind(&token_hash)
    .fetch_optional(&mut *tx)
    .await?;

    let Some((token_id, db_partner, expires_at, used_at, revoked_at)) = row else {
        return Err(RegistrationError::TokenNotFound);
    };

    if revoked_at.is_some() {
        return Err(RegistrationError::TokenRevoked);
    }
    if used_at.is_some() {
        return Err(RegistrationError::TokenAlreadyUsed);
    }
    if expires_at <= chrono::Utc::now() {
        return Err(RegistrationError::TokenExpired);
    }
    if db_partner != partner_id {
        return Err(RegistrationError::PartnerMismatch);
    }

    let node_id = format!("{partner_id}-{}", short_random_hex(6));
    let turns_subdomain = format!(
        "api-{}",
        node_id.split('-').last().unwrap_or("000000")
    );

    sqlx::query(
        "UPDATE partner_tokens \
         SET used_at = NOW(), used_from_ip = $1, node_id = $2 \
         WHERE token_id = $3",
    )
    .bind(public_ip.to_string())
    .bind(&node_id)
    .bind(token_id)
    .execute(&mut *tx)
    .await?;

    let turns_subdomain: String = sqlx::query_scalar(
        "INSERT INTO partner_nodes \
             (node_id, partner_id, domain, turns_subdomain, public_ip) \
         VALUES ($1, $2, $3, $4, $5) \
         ON CONFLICT (partner_id, domain) \
             DO UPDATE SET last_seen_at = NOW() \
         RETURNING turns_subdomain",
    )
    .bind(&node_id)
    .bind(partner_id)
    .bind(domain)
    .bind(&turns_subdomain)
    .bind(public_ip.to_string())
    .fetch_one(&mut *tx)
    .await?;

    tx.commit().await?;

    let backend_endpoint = std::env::var("PARTNER_BACKEND_ENDPOINT")
        .unwrap_or_else(|_| "reality://krolik-server:5349".to_string());

    Ok(RegistrationOk {
        node_id,
        backend_endpoint,
        reality,
        turn_secret,
        turns_subdomain,
        config_version: crate::domains::CONFIG_VERSION,
    })
}
