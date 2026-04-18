use super::render::{absolute_asset_url, primary_canonical};
use super::*;

#[test]
fn partner_configs_resolve() {
    assert_eq!(resolve_by_host("call.piter.now").partner_id, "piter");
    assert_eq!(resolve_by_host("call.rvpn.online").partner_id, "rvpn");
    assert_eq!(resolve_by_host("call1.rvpn.online").partner_id, "rvpn");
    assert_eq!(resolve_by_host("oxpulse.chat").partner_id, "oxpulse");
}

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
#[test]
fn suffix_subdomain_does_not_match_real_domain() {
    LazyLock::force(&HOST_INDEX);
    assert!(
        HOST_INDEX.get("oxpulse.chat.evil.com").is_none(),
        "suffix match would allow host hijacking"
    );
}

fn test_cfg() -> BrandingConfig {
    use std::collections::HashMap;
    BrandingConfig {
        partner_id: "testpartner".to_string(),
        domains: vec!["call.example.com".to_string()],
        display_name: "TestPartner".to_string(),
        description: "Secure calls".to_string(),
        logo: LogoConfig {
            light: "/logo-light.svg".to_string(),
            dark: "/logo-dark.svg".to_string(),
        },
        favicon: "/favicon.ico".to_string(),
        og_image: "/og-image.png".to_string(),
        colors: Colors {
            primary: "#0066FF".to_string(),
            secondary: "#1E293B".to_string(),
            accent: None,
        },
        copy: HashMap::new(),
        affiliate: None,
        legal: None,
        co_brand_partner: None,
        canonical_override: None,
    }
}

#[test]
fn render_index_substitutes_all_placeholders() {
    let cfg = test_cfg();
    let html = "<title>__BRANDING_TITLE__</title>\
        <meta name=\"description\" content=\"__BRANDING_DESCRIPTION__\"/>\
        <link rel=\"canonical\" href=\"__BRANDING_CANONICAL__\"/>\
        <meta property=\"og:url\" content=\"__BRANDING_OG_URL__\"/>\
        <meta property=\"og:image\" content=\"__BRANDING_OG_IMAGE__\"/>\
        <meta property=\"og:site_name\" content=\"__BRANDING_SITE_NAME__\"/>\
        <link rel=\"icon\" href=\"__BRANDING_FAVICON__\"/>\
        <meta name=\"partner\" content=\"__BRANDING_PARTNER_ID__\"/>\
        <script id=\"__branding_boot__\" type=\"application/json\">__BRANDING_JSON__</script>";
    let out = render_index(html, &cfg);
    assert!(out.contains("TestPartner"), "title/site_name");
    assert!(out.contains("Secure calls"), "description");
    assert!(
        out.contains("https://call.example.com/"),
        "canonical/og_url"
    );
    assert!(
        out.contains("https://call.example.com/og-image.png"),
        "og_image absolutized"
    );
    assert!(out.contains("/favicon.ico"), "favicon");
    assert!(out.contains("testpartner"), "partner_id");
    assert!(!out.contains("__BRANDING_"), "no leftover placeholders");
}

#[test]
fn render_index_injects_branding_json_script() {
    let cfg = test_cfg();
    let html =
        "<script id=\"__branding_boot__\" type=\"application/json\">__BRANDING_JSON__</script>";
    let out = render_index(html, &cfg);
    // Placeholder must be replaced
    assert!(
        !out.contains("__BRANDING_JSON__"),
        "placeholder must be substituted"
    );
    // Extract the JSON from inside the script tag
    let start = out.find('>').expect("opening tag") + 1;
    let end = out.rfind('<').expect("closing tag");
    let json_str = &out[start..end];
    // Must parse as a valid JSON object
    let parsed: serde_json::Value =
        serde_json::from_str(json_str).expect("injected content must be valid JSON");
    assert!(parsed.is_object(), "must be a JSON object");
    assert_eq!(
        parsed["partner_id"].as_str().unwrap(),
        "testpartner",
        "partner_id must round-trip through JSON"
    );
}

#[test]
fn render_index_leaves_unknown_placeholders_alone() {
    let cfg = test_cfg();
    let html = "<p>__BRANDING_UNKNOWN__ stays</p>";
    let out = render_index(html, &cfg);
    assert!(out.contains("__BRANDING_UNKNOWN__"));
}

#[test]
fn primary_canonical_uses_first_domain() {
    let cfg = test_cfg();
    assert_eq!(primary_canonical(&cfg), "https://call.example.com/");
}

#[test]
fn primary_canonical_no_domains_returns_slash() {
    let mut cfg = test_cfg();
    cfg.domains.clear();
    assert_eq!(primary_canonical(&cfg), "/");
}

#[test]
fn absolute_asset_url_prefixes_relative_paths() {
    let cfg = test_cfg();
    assert_eq!(
        absolute_asset_url(&cfg, "/og-image.png"),
        "https://call.example.com/og-image.png"
    );
}

#[test]
fn absolute_asset_url_passes_through_absolute() {
    let cfg = test_cfg();
    let abs = "https://cdn.example.com/img.png";
    assert_eq!(absolute_asset_url(&cfg, abs), abs);
}

#[test]
fn render_index_includes_co_brand_partner_when_present() {
    let mut cfg = test_cfg();
    cfg.co_brand_partner = Some("RVPN".to_string());
    let html = "<span>__BRANDING_CO_BRAND_PARTNER__</span>";
    let out = render_index(html, &cfg);
    assert!(
        out.contains("RVPN"),
        "co-brand partner name must be injected"
    );
    assert!(
        !out.contains("__BRANDING_CO_BRAND_PARTNER__"),
        "placeholder substituted"
    );
}

#[test]
fn render_index_empty_co_brand_partner_when_absent() {
    let cfg = test_cfg(); // co_brand_partner defaults to None
    let html = "<span>[__BRANDING_CO_BRAND_PARTNER__]</span>";
    let out = render_index(html, &cfg);
    assert_eq!(
        out, "<span>[]</span>",
        "absent co-brand renders as empty string"
    );
}

#[test]
fn primary_canonical_uses_override_when_set() {
    let mut cfg = test_cfg();
    cfg.canonical_override = Some("https://oxpulse.chat/".to_string());
    assert_eq!(primary_canonical(&cfg), "https://oxpulse.chat/");
}

#[test]
fn primary_canonical_override_wins_over_domains() {
    let mut cfg = test_cfg();
    // domains[0] is "call.example.com" from test_cfg()
    cfg.canonical_override = Some("https://oxpulse.chat/".to_string());
    assert_eq!(primary_canonical(&cfg), "https://oxpulse.chat/");
    assert_ne!(primary_canonical(&cfg), "https://call.example.com/");
}

#[test]
fn branding_config_deserializes_without_new_optional_fields() {
    // Back-compat: old configs missing co_brand_partner and canonical_override
    // must still deserialize (serde(default) → None).
    let json = r##"{
        "partner_id": "legacy",
        "domains": ["legacy.example.com"],
        "display_name": "Legacy",
        "description": "legacy",
        "logo": { "light": "/l.svg", "dark": "/d.svg" },
        "favicon": "/f.ico",
        "og_image": "/og.png",
        "colors": { "primary": "#000", "secondary": "#111", "accent": null },
        "copy": {},
        "affiliate": null,
        "legal": null
    }"##;
    let cfg: BrandingConfig = serde_json::from_str(json).expect("legacy JSON must still parse");
    assert!(cfg.co_brand_partner.is_none());
    assert!(cfg.canonical_override.is_none());
}
