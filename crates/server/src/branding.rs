//! Host-based branding resolution for partner co-brand mirror.
//!
//! Partner configurations are bundled at compile time from
//! `config/partners/*.json` via `include_dir!`. The first entry
//! (`oxpulse.json`, sorted lexicographically first by filename) is treated as
//! the default and is returned for any host not in the index.

use std::collections::HashMap;
use std::sync::LazyLock;

use axum::http::header::{CONTENT_TYPE, HeaderValue};
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use include_dir::{include_dir, Dir};
use serde::{Deserialize, Serialize};

static PARTNERS_DIR: Dir<'_> =
    include_dir!("$CARGO_MANIFEST_DIR/../../config/partners");

/// Logo URLs for light and dark themes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogoConfig {
    pub light: String,
    pub dark: String,
}

/// Brand colour palette.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Colors {
    pub primary: String,
    pub secondary: String,
    pub accent: Option<String>,
}

/// Affiliate / VPN call-to-action block.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AffiliateConfig {
    pub vpn_cta_url: String,
    pub vpn_cta_text_ru: String,
    pub vpn_cta_text_en: String,
}

/// Legal / entity information for the partner.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LegalConfig {
    pub partner_entity: String,
    pub partner_country: String,
    pub partner_contact: String,
}

/// Full branding configuration for one partner.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrandingConfig {
    pub partner_id: String,
    /// Hostnames that should resolve to this config.
    pub domains: Vec<String>,
    pub display_name: String,
    pub description: String,
    pub logo: LogoConfig,
    pub favicon: String,
    pub og_image: String,
    pub colors: Colors,
    /// Arbitrary copy strings, e.g. `hero_title_ru`, `hero_title_en`.
    pub copy: HashMap<String, String>,
    pub affiliate: Option<AffiliateConfig>,
    pub legal: Option<LegalConfig>,
}

/// All partner configs, parsed once at startup from the bundled JSON files.
/// Files are sorted by name so that `oxpulse.json` is always index 0 (default).
/// Malformed JSON is a programmer error — panic loudly with the file path so
/// it is caught before the service ever handles a request (see `init()`).
static BRANDINGS: LazyLock<Vec<BrandingConfig>> = LazyLock::new(|| {
    let mut files: Vec<_> = PARTNERS_DIR.files().collect();
    files.sort_by_key(|f| f.path());

    files
        .into_iter()
        .map(|f| {
            serde_json::from_slice::<BrandingConfig>(f.contents())
                .unwrap_or_else(|e| panic!("malformed partner config {}: {e}", f.path().display()))
        })
        .collect()
});

/// Lowercase-hostname → index into `BRANDINGS`.
static HOST_INDEX: LazyLock<HashMap<String, usize>> = LazyLock::new(|| {
    let mut map = HashMap::new();
    for (idx, cfg) in BRANDINGS.iter().enumerate() {
        for domain in &cfg.domains {
            map.insert(domain.to_lowercase(), idx);
        }
    }
    map
});

/// Force both LazyLocks to initialize at startup.
///
/// Must be called from `main()` before the server starts listening so that
/// any `panic!` from malformed partner JSON fires at startup, not inside a
/// request handler. Logs how many partners were loaded.
pub fn init() {
    LazyLock::force(&BRANDINGS);
    LazyLock::force(&HOST_INDEX);
    assert!(
        !BRANDINGS.is_empty(),
        "BRANDINGS must be non-empty — no partner JSON files found in config/partners/"
    );
    tracing::info!(count = BRANDINGS.len(), "branding: loaded partners");
}

/// Resolve branding by HTTP `Host` (or `X-Forwarded-Host`) value.
///
/// Strip the port before calling this function (the handler already does
/// `split(':').next()`). Lookup is case-insensitive. Returns the default
/// OxPulse config for any unknown or empty host.
///
/// Non-empty invariant is guaranteed by `init()` called at startup.
pub fn resolve_by_host(host: &str) -> &'static BrandingConfig {
    let key = host.to_lowercase();
    let idx = HOST_INDEX.get(&key).copied().unwrap_or(0);
    &BRANDINGS[idx]
}

/// HTTP handler for `GET /api/branding`.
///
/// Serializes the static `&BrandingConfig` directly — no clone per request.
pub async fn handler(headers: HeaderMap) -> impl IntoResponse {
    let host = headers
        .get("x-forwarded-host")
        .or_else(|| headers.get("host"))
        .and_then(|h| h.to_str().ok())
        .unwrap_or("")
        .split(':') // strip :port
        .next()
        .unwrap_or("");
    let cfg = resolve_by_host(host);
    let body = match serde_json::to_vec(cfg) {
        Ok(b) => b,
        Err(e) => {
            tracing::error!(error = %e, "branding: serialization failed");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                [(CONTENT_TYPE, HeaderValue::from_static("application/json"))],
                br#"{"error":"serialization failed"}"#.to_vec(),
            )
                .into_response();
        }
    };
    (
        StatusCode::OK,
        [(CONTENT_TYPE, HeaderValue::from_static("application/json"))],
        body,
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_known_host_returns_matching_config() {
        let cfg = resolve_by_host("oxpulse.chat");
        assert_eq!(cfg.partner_id, "oxpulse");
    }

    #[test]
    fn resolve_unknown_host_returns_default() {
        let cfg = resolve_by_host("random.example.com");
        assert_eq!(cfg.partner_id, "oxpulse");
    }

    #[test]
    fn resolve_is_case_insensitive() {
        let cfg = resolve_by_host("OxPulse.Chat");
        assert_eq!(cfg.partner_id, "oxpulse");
    }

    #[test]
    fn empty_host_returns_default() {
        let cfg = resolve_by_host("");
        assert_eq!(cfg.partner_id, "oxpulse");
    }

    /// Proves resolve_by_host uses exact match, not suffix match.
    /// A malicious subdomain like "oxpulse.chat.evil.com" must not hijack
    /// the default branding via a naive `ends_with` refactor.
    /// Checks the index directly: the key must not be present at all.
    #[test]
    fn suffix_subdomain_does_not_match_real_domain() {
        // Force init so HOST_INDEX is populated.
        LazyLock::force(&HOST_INDEX);
        assert!(
            HOST_INDEX.get("oxpulse.chat.evil.com").is_none(),
            "suffix match would allow host hijacking"
        );
    }
}
