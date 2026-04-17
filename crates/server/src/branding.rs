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
static BRANDINGS: LazyLock<Vec<BrandingConfig>> = LazyLock::new(|| {
    let mut files: Vec<_> = PARTNERS_DIR.files().collect();
    files.sort_by_key(|f| f.path());

    files
        .into_iter()
        .filter_map(|f| {
            let bytes = f.contents();
            match serde_json::from_slice::<BrandingConfig>(bytes) {
                Ok(cfg) => Some(cfg),
                Err(e) => {
                    tracing::warn!(
                        file = ?f.path(),
                        error = %e,
                        "failed to parse partner config — skipping"
                    );
                    None
                }
            }
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

/// Resolve branding by HTTP `Host` (or `X-Forwarded-Host`) value.
///
/// Strip the port before calling this function (the handler already does
/// `split(':').next()`). Lookup is case-insensitive. Returns the default
/// OxPulse config for any unknown or empty host.
pub fn resolve_by_host(host: &str) -> &'static BrandingConfig {
    assert!(!BRANDINGS.is_empty(), "BRANDINGS must be non-empty — no partner JSON files found in config/partners/");

    let key = host.to_lowercase();
    let idx = HOST_INDEX.get(&key).copied().unwrap_or(0);
    &BRANDINGS[idx]
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
}
