//! Host-based branding resolution for partner co-brand mirror.
//!
//! Partner configurations are bundled at compile time from
//! `config/partners/*.json` via `include_dir!`. The first entry
//! (`oxpulse.json`, sorted lexicographically first by filename) is treated as
//! the default and is returned for any host not in the index.

use std::collections::HashMap;
use std::sync::LazyLock;

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

/// Substitutes `__BRANDING_*__` placeholders in an HTML template.
///
/// Called by the SPA fallback handler in router.rs for every SPA route so
/// that crawlers (Telegram, iMessage, Twitter) see partner-specific OG tags
/// without executing JS. Values injected into the JSON-LD block must be
/// JSON-safe (no `"` or `\`); partner configs are hand-authored so this is
/// acceptable without escaping logic — add an escape layer if configs become
/// user-controlled.
pub fn render_index(template: &str, cfg: &BrandingConfig) -> String {
    template
        .replace("__BRANDING_SITE_NAME__", &cfg.display_name)
        .replace("__BRANDING_TITLE__", &cfg.display_name)
        .replace("__BRANDING_DESCRIPTION__", &cfg.description)
        .replace("__BRANDING_CANONICAL__", &primary_canonical(cfg))
        .replace("__BRANDING_OG_URL__", &primary_canonical(cfg))
        .replace("__BRANDING_OG_IMAGE__", &absolute_asset_url(cfg, &cfg.og_image))
        .replace("__BRANDING_FAVICON__", &cfg.favicon)
        .replace("__BRANDING_PARTNER_ID__", &cfg.partner_id)
}

fn primary_canonical(cfg: &BrandingConfig) -> String {
    match cfg.domains.first() {
        Some(d) => format!("https://{}/", d),
        None => "/".to_string(),
    }
}

fn absolute_asset_url(cfg: &BrandingConfig, path: &str) -> String {
    if path.starts_with("http://") || path.starts_with("https://") {
        return path.to_string();
    }
    match cfg.domains.first() {
        Some(d) => format!("https://{}{}", d, path),
        None => path.to_string(),
    }
}

#[cfg(test)]
#[path = "branding_tests.rs"]
mod tests;
