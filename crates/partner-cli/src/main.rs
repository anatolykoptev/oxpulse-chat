//! Admin CLI for issuing / listing / revoking partner bootstrap tokens.
//!
//! Wraps the same `partner_tokens` table the server reads. No HTTP — the
//! CLI is intended to run on the OxPulse-backend host with DATABASE_URL
//! pointing at the same postgres as the server.
//!
//! Subcommands:
//! - `issue-token --partner <id> --valid-for <duration>`
//! - `list-tokens [--partner <id>] [--include-used] [--include-revoked]`
//! - `revoke-token <token-id>`
//! - `list-nodes [--partner <id>]`
//! - `deactivate-node <node-id>`  (MVP placeholder)

mod commands;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use sqlx::postgres::PgPoolOptions;

#[derive(Parser)]
#[command(
    name = "partner-cli",
    version,
    about = "OxPulse partner token admin CLI"
)]
struct Cli {
    #[command(subcommand)]
    cmd: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Issue a new bootstrap token. Prints the raw value ONCE.
    IssueToken {
        /// Partner identifier (e.g. `rvpn`, `piter`).
        #[arg(long)]
        partner: String,
        /// Token validity window (e.g. `30d`, `7d`, `48h`).
        #[arg(long, default_value = "30d")]
        valid_for: String,
    },
    /// List tokens. By default only unused, non-revoked tokens.
    ListTokens {
        #[arg(long)]
        partner: Option<String>,
        #[arg(long)]
        include_used: bool,
        #[arg(long)]
        include_revoked: bool,
    },
    /// Revoke a token by its token_id.
    RevokeToken { token_id: String },
    /// List registered nodes (tokens whose used_at is set).
    ListNodes {
        #[arg(long)]
        partner: Option<String>,
    },
    /// Deactivate a node. MVP placeholder — see TODO in handler.
    DeactivateNode { node_id: String },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    let db_url = std::env::var("DATABASE_URL")
        .context("DATABASE_URL env var must be set (set it in your operator environment)")?;
    let pool = PgPoolOptions::new()
        .max_connections(2)
        .connect(&db_url)
        .await
        .context("connecting to DATABASE_URL")?;

    // The server auto-applies migrations at boot. Probe that the table exists
    // and give a clear error if not (instead of a confusing sqlx error).
    commands::check_schema(&pool).await?;

    match cli.cmd {
        Command::IssueToken { partner, valid_for } => {
            commands::issue_token(&pool, &partner, &valid_for).await
        }
        Command::ListTokens {
            partner,
            include_used,
            include_revoked,
        } => commands::list_tokens(&pool, partner.as_deref(), include_used, include_revoked).await,
        Command::RevokeToken { token_id } => commands::revoke_token(&pool, &token_id).await,
        Command::ListNodes { partner } => commands::list_nodes(&pool, partner.as_deref()).await,
        Command::DeactivateNode { node_id } => commands::deactivate_node(&pool, &node_id).await,
    }
}
