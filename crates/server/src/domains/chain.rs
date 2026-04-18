//! Pure mirror-chain construction logic — no HTTP, no global state.
//!
//! Extracted so it can be unit-tested with arbitrary config slices without
//! touching the global `BRANDINGS` static.

use std::collections::HashSet;

use crate::branding::BrandingConfig;

/// Build `(primary, mirrors)` from a host string and a config slice.
///
/// - `host` must already be lowercased and have any port stripped.
/// - Same-partner mirrors are listed before cross-partner ones.
/// - `localhost` is excluded from mirrors (dev-only domain).
/// - Duplicates are removed, preserving first-seen order.
pub fn build_mirror_chain(host: &str, configs: &[BrandingConfig]) -> (String, Vec<String>) {
    // Resolve current partner — default to index 0 (oxpulse) if not found.
    let current_idx = configs
        .iter()
        .position(|c| c.domains.iter().any(|d| d.to_lowercase() == host))
        .unwrap_or(0);

    let current_partner = &configs[current_idx];

    let primary = if host.is_empty() {
        current_partner.domains.first().cloned().unwrap_or_default()
    } else {
        host.to_string()
    };

    // Same-partner mirrors first (excluding primary), then cross-partner.
    let same: Vec<String> = current_partner
        .domains
        .iter()
        .filter(|d| d.to_lowercase() != primary.to_lowercase())
        .cloned()
        .collect();

    let mut other: Vec<String> = Vec::new();
    for cfg in configs {
        if cfg.partner_id == current_partner.partner_id {
            continue;
        }
        for d in &cfg.domains {
            if d.to_lowercase() != primary.to_lowercase() {
                other.push(d.clone());
            }
        }
    }

    // De-duplicate preserving order.
    let mut seen: HashSet<String> = HashSet::new();
    seen.insert(primary.to_lowercase());
    let mut mirrors: Vec<String> = same
        .into_iter()
        .chain(other)
        .filter(|d| seen.insert(d.to_lowercase()))
        .collect();

    // Exclude localhost — dev-only, never a useful production mirror.
    mirrors.retain(|d| d != "localhost");

    (primary, mirrors)
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;
    use crate::branding::{BrandingConfig, Colors, LogoConfig};

    fn make_cfg(partner_id: &str, domains: &[&str]) -> BrandingConfig {
        BrandingConfig {
            partner_id: partner_id.to_string(),
            domains: domains.iter().map(|d| d.to_string()).collect(),
            display_name: partner_id.to_string(),
            description: String::new(),
            logo: LogoConfig {
                light: String::new(),
                dark: String::new(),
            },
            favicon: String::new(),
            og_image: String::new(),
            colors: Colors {
                primary: String::new(),
                secondary: String::new(),
                accent: None,
            },
            copy: HashMap::new(),
            affiliate: None,
            legal: None,
            co_brand_partner: None,
            canonical_override: None,
        }
    }

    fn test_configs() -> Vec<BrandingConfig> {
        vec![
            make_cfg(
                "oxpulse",
                &["oxpulse.chat", "www.oxpulse.chat", "localhost"],
            ),
            make_cfg("piter", &["call.piter.now"]),
            make_cfg(
                "rvpn",
                &["call.rvpn.online", "call1.rvpn.online", "call2.rvpn.online"],
            ),
        ]
    }

    #[test]
    fn primary_is_current_host_when_provided() {
        let cfgs = test_configs();
        let (primary, mirrors) = build_mirror_chain("call.rvpn.online", &cfgs);
        assert_eq!(primary, "call.rvpn.online");
        assert!(mirrors.contains(&"call1.rvpn.online".to_string()));
        assert!(mirrors.contains(&"call2.rvpn.online".to_string()));
        assert!(mirrors.contains(&"call.piter.now".to_string()));
        assert!(mirrors.contains(&"oxpulse.chat".to_string()));
    }

    #[test]
    fn same_partner_mirrors_listed_first() {
        let cfgs = test_configs();
        let (_primary, mirrors) = build_mirror_chain("call.rvpn.online", &cfgs);
        let pos_call1 = mirrors
            .iter()
            .position(|d| d == "call1.rvpn.online")
            .expect("call1.rvpn.online missing");
        let pos_call2 = mirrors
            .iter()
            .position(|d| d == "call2.rvpn.online")
            .expect("call2.rvpn.online missing");
        let pos_piter = mirrors
            .iter()
            .position(|d| d == "call.piter.now")
            .expect("call.piter.now missing");
        let pos_oxpulse = mirrors
            .iter()
            .position(|d| d == "oxpulse.chat")
            .expect("oxpulse.chat missing");
        assert!(pos_call1 < pos_piter);
        assert!(pos_call2 < pos_piter);
        assert!(pos_call1 < pos_oxpulse);
        assert!(pos_call2 < pos_oxpulse);
    }

    #[test]
    fn current_host_excluded_from_mirrors() {
        let cfgs = test_configs();
        let (primary, mirrors) = build_mirror_chain("call.rvpn.online", &cfgs);
        assert!(!mirrors.contains(&primary));
    }

    #[test]
    fn localhost_excluded() {
        let cfgs = test_configs();
        let (_primary, mirrors) = build_mirror_chain("oxpulse.chat", &cfgs);
        assert!(!mirrors.iter().any(|d| d == "localhost"));
    }

    #[test]
    fn empty_host_falls_back_to_default_partner_first_domain() {
        let cfgs = test_configs();
        let (primary, _mirrors) = build_mirror_chain("", &cfgs);
        assert_eq!(primary, "oxpulse.chat");
    }

    #[test]
    fn duplicates_removed() {
        let cfgs = vec![
            make_cfg("oxpulse", &["oxpulse.chat", "shared.example.com"]),
            make_cfg("other", &["other.example.com", "shared.example.com"]),
        ];
        let (_primary, mirrors) = build_mirror_chain("oxpulse.chat", &cfgs);
        let count = mirrors
            .iter()
            .filter(|d| d.as_str() == "shared.example.com")
            .count();
        assert_eq!(count, 1);
    }
}
