//! GET /api/domains — returns the primary host and ordered mirror list.
//!
//! Clients use the mirror list to fall back when the primary domain is blocked
//! by RF censorship or otherwise unreachable.

pub mod chain;

use axum::Json;
use axum::http::HeaderMap;
use serde::Serialize;

/// Response shape for GET /api/domains.
///
/// - `primary`: the domain the client currently reached via
///   (`X-Forwarded-Host` / `Host`).
/// - `mirrors`: ordered list of all other known domains suitable for fallback.
///   Same-partner domains come before cross-partner ones.
/// - `config_version`: monotonic counter; clients use this to invalidate their
///   localStorage cache when the partner set changes.
#[derive(Serialize)]
pub struct DomainsResponse {
    pub primary: String,
    pub mirrors: Vec<String>,
    pub config_version: u64,
}

/// Version stamp. Incremented whenever partner JSON files change on disk.
/// Initial value: 1.
pub const CONFIG_VERSION: u64 = 1;

/// Handler for `GET /api/domains`.
pub async fn handler(headers: HeaderMap) -> Json<DomainsResponse> {
    let host = crate::branding::extract_host(&headers);

    let (primary, mirrors) = chain::build_mirror_chain(&host, crate::branding::all_configs());

    Json(DomainsResponse {
        primary,
        mirrors,
        config_version: CONFIG_VERSION,
    })
}
