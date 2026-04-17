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
pub(crate) static BRANDINGS: LazyLock<Vec<BrandingConfig>> = LazyLock::new(|| {
    let mut files: Vec<_> = PARTNERS_DIR
        .files()
        .filter(|f| f.path().extension().and_then(|e| e.to_str()) == Some("json"))
        .collect();
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
pub(crate) static HOST_INDEX: LazyLock<HashMap<String, usize>> = LazyLock::new(|| {
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

/// Returns a reference to all loaded partner configurations.
///
/// Useful for building full domain lists (e.g. for `/api/domains`).
pub fn all_configs() -> &'static [BrandingConfig] {
    &BRANDINGS
}

pub use render::render_index;
pub use handler::handler;

pub(crate) mod render;
pub(crate) mod handler;
#[cfg(test)]
mod tests;
