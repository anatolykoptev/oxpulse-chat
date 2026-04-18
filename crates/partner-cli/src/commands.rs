//! Subcommand implementations. Each one is small, synchronous print-side
//! logic + one sqlx call. Kept in one file because the subcommands share
//! a minimal schema and are easier to eyeball as a group (<200 lines).

use anyhow::{bail, Context, Result};
use rand::RngCore;
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use sqlx::Row;
use uuid::Uuid;

const ISSUE_TOKEN_WARNING: &str =
    "!! This is the ONLY time this raw token will be shown. Copy it now. !!";

/// Verify that the server's migrations have been applied by probing for the
/// `partner_tokens` table. Gives a clear error message if the table is absent.
///
/// The CLI does NOT maintain its own schema copy — run the server once
/// (`cargo run -p oxpulse-chat`) to apply migrations before using the CLI.
pub async fn check_schema(pool: &PgPool) -> Result<()> {
    sqlx::query("SELECT 1 FROM partner_tokens LIMIT 1")
        .execute(pool)
        .await
        .map_err(|e| anyhow::anyhow!(
            "partner_tokens table not found — run `cargo run -p oxpulse-chat` once to apply migrations ({e})"
        ))?;
    Ok(())
}

fn hash_token(raw: &str) -> String {
    let mut h = Sha256::new();
    h.update(raw.as_bytes());
    format!("{:x}", h.finalize())
}

fn generate_raw_token() -> String {
    let mut buf = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut buf);
    let hex: String = buf.iter().map(|b| format!("{b:02x}")).collect();
    format!("ptkn_{hex}")
}

/// Parses a human duration like "30d" / "48h" / "2w". Returns seconds.
fn parse_duration(s: &str) -> Result<i64> {
    let d = humantime::parse_duration(s).with_context(|| format!("invalid duration: {s}"))?;
    Ok(d.as_secs() as i64)
}

pub async fn issue_token(pool: &PgPool, partner: &str, valid_for: &str) -> Result<()> {
    if partner.is_empty() || partner.len() > 64 {
        bail!("partner must be 1..=64 chars");
    }
    let secs = parse_duration(valid_for)?;
    let raw = generate_raw_token();
    let token_hash = hash_token(&raw);
    let expires_at = chrono::Utc::now() + chrono::Duration::seconds(secs);

    let token_id: Uuid = sqlx::query_scalar(
        "INSERT INTO partner_tokens (partner_id, token_hash, expires_at) \
         VALUES ($1, $2, $3) RETURNING token_id",
    )
    .bind(partner)
    .bind(&token_hash)
    .bind(expires_at)
    .fetch_one(pool)
    .await
    .context("insert partner_tokens row")?;

    println!("{ISSUE_TOKEN_WARNING}");
    println!("token_id  : {token_id}");
    println!("partner   : {partner}");
    println!("expires_at: {expires_at}");
    println!("raw token : {raw}");
    Ok(())
}

pub async fn list_tokens(
    pool: &PgPool,
    partner: Option<&str>,
    include_used: bool,
    include_revoked: bool,
) -> Result<()> {
    // Bind positional indices: $1 is always partner ("" matches all when
    // we short-circuit via OR); fixed parameter list avoids Postgres'
    // "could not determine data type of parameter" when a bind is absent.
    let used_clause = if include_used {
        ""
    } else {
        " AND used_at IS NULL"
    };
    let revoked_clause = if include_revoked {
        ""
    } else {
        " AND revoked_at IS NULL"
    };
    let partner_filter = partner.unwrap_or("");
    let sql = format!(
        "SELECT token_id, partner_id, expires_at, used_at, revoked_at, node_id \
         FROM partner_tokens \
         WHERE ($1 = '' OR partner_id = $1){used_clause}{revoked_clause} \
         ORDER BY created_at DESC LIMIT 200"
    );
    let rows = sqlx::query(&sql)
        .bind(partner_filter)
        .fetch_all(pool)
        .await
        .context("select partner_tokens")?;
    println!(
        "{:<36}  {:<12}  {:<20}  {:<20}  {:<10}  node_id",
        "token_id", "partner_id", "expires_at", "used_at", "revoked"
    );
    for r in rows {
        let tid: Uuid = r.try_get("token_id")?;
        let pid: String = r.try_get("partner_id")?;
        let exp: chrono::DateTime<chrono::Utc> = r.try_get("expires_at")?;
        let used: Option<chrono::DateTime<chrono::Utc>> = r.try_get("used_at")?;
        let rev: Option<chrono::DateTime<chrono::Utc>> = r.try_get("revoked_at")?;
        let nid: Option<String> = r.try_get("node_id")?;
        println!(
            "{tid}  {pid:<12}  {:<20}  {:<20}  {:<10}  {}",
            exp.format("%Y-%m-%d %H:%M:%S"),
            used.map(|u| u.format("%Y-%m-%d %H:%M:%S").to_string())
                .unwrap_or_else(|| "-".into()),
            if rev.is_some() { "yes" } else { "no" },
            nid.unwrap_or_else(|| "-".into())
        );
    }
    Ok(())
}

pub async fn revoke_token(pool: &PgPool, token_id: &str) -> Result<()> {
    let uuid = Uuid::parse_str(token_id).context("token_id must be a UUID")?;
    let res = sqlx::query(
        "UPDATE partner_tokens SET revoked_at = NOW() \
         WHERE token_id = $1 AND revoked_at IS NULL",
    )
    .bind(uuid)
    .execute(pool)
    .await
    .context("revoke token")?;
    if res.rows_affected() == 0 {
        bail!("token not found or already revoked: {token_id}");
    }
    println!("revoked: {token_id}");
    Ok(())
}

pub async fn list_nodes(pool: &PgPool, partner: Option<&str>) -> Result<()> {
    let partner_filter = partner.unwrap_or("");
    let sql = "SELECT node_id, partner_id, used_at, used_from_ip FROM partner_tokens \
               WHERE used_at IS NOT NULL AND ($1 = '' OR partner_id = $1) \
               ORDER BY used_at DESC LIMIT 200";
    let rows = sqlx::query(sql)
        .bind(partner_filter)
        .fetch_all(pool)
        .await
        .context("select nodes")?;
    println!(
        "{:<24}  {:<12}  {:<20}  ip",
        "node_id", "partner_id", "registered_at"
    );
    for r in rows {
        let nid: Option<String> = r.try_get("node_id")?;
        let pid: String = r.try_get("partner_id")?;
        let used: Option<chrono::DateTime<chrono::Utc>> = r.try_get("used_at")?;
        let ip: Option<String> = r.try_get("used_from_ip")?;
        println!(
            "{:<24}  {pid:<12}  {:<20}  {}",
            nid.unwrap_or_else(|| "-".into()),
            used.map(|u| u.format("%Y-%m-%d %H:%M:%S").to_string())
                .unwrap_or_else(|| "-".into()),
            ip.unwrap_or_else(|| "-".into())
        );
    }
    Ok(())
}

pub async fn deactivate_node(_pool: &PgPool, _node_id: &str) -> anyhow::Result<()> {
    anyhow::bail!(
        "deactivate-node not yet implemented — tracking in-progress.\n\
        Workaround: UPDATE partner_tokens SET revoked_at = NOW() WHERE node_id = $1;\n\
        (See follow-up: add `deactivated_at` column + proper CLI path.)"
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Golden-value parity test — must match server's partner_registry::creds::hash_token.
    /// REFERENCE: sha256("test-token-fixed")
    /// If this fails, the CLI's hash_token drifted from the server's copy. Fix in lockstep.
    #[test]
    fn hash_token_matches_server_reference() {
        assert_eq!(
            hash_token("test-token-fixed"),
            "f227298136580b1377d03ef38f996e39bc442f9d1afd48069ea842af5d54cd97"
        );
    }
}
