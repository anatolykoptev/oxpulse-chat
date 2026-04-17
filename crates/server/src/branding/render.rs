//! HTML template rendering: substitutes `__BRANDING_*__` placeholders.
//!
//! Called by the SPA fallback handler in router.rs for every SPA route so
//! that crawlers (Telegram, iMessage, Twitter) see partner-specific OG tags
//! without executing JS. Values injected into the JSON-LD block must be
//! JSON-safe (no `"` or `\`); partner configs are hand-authored so this is
//! acceptable without escaping logic — add an escape layer if configs become
//! user-controlled.

use super::BrandingConfig;

/// Substitutes `__BRANDING_*__` placeholders in an HTML template.
///
/// `__BRANDING_JSON__` is replaced with the full `BrandingConfig` serialized
/// as JSON so the client-side store can bootstrap synchronously from the
/// server-rendered page and avoid a tab-title flash on partner domains.
///
/// # Panics
/// Never panics: `BrandingConfig` is always serializable (all fields are
/// primitive types — no maps with non-string keys, no floats). If for some
/// reason serialization fails, a safe `{}` is substituted.
pub fn render_index(template: &str, cfg: &BrandingConfig) -> String {
    let branding_json =
        serde_json::to_string(cfg).unwrap_or_else(|_| "{}".to_string());
    template
        .replace("__BRANDING_SITE_NAME__", &cfg.display_name)
        .replace("__BRANDING_TITLE__", &cfg.display_name)
        .replace("__BRANDING_DESCRIPTION__", &cfg.description)
        .replace("__BRANDING_CANONICAL__", &primary_canonical(cfg))
        .replace("__BRANDING_OG_URL__", &primary_canonical(cfg))
        .replace("__BRANDING_OG_IMAGE__", &absolute_asset_url(cfg, &cfg.og_image))
        .replace("__BRANDING_FAVICON__", &cfg.favicon)
        .replace("__BRANDING_PARTNER_ID__", &cfg.partner_id)
        .replace(
            "__BRANDING_CO_BRAND_PARTNER__",
            cfg.co_brand_partner.as_deref().unwrap_or(""),
        )
        .replace("__BRANDING_JSON__", &branding_json)
}

pub(crate) fn primary_canonical(cfg: &BrandingConfig) -> String {
    if let Some(ref c) = cfg.canonical_override {
        return c.clone();
    }
    match cfg.domains.first() {
        Some(d) => format!("https://{}/", d),
        None => "/".to_string(),
    }
}

pub(crate) fn absolute_asset_url(cfg: &BrandingConfig, path: &str) -> String {
    if path.starts_with("http://") || path.starts_with("https://") {
        return path.to_string();
    }
    match cfg.domains.first() {
        Some(d) => format!("https://{}{}", d, path),
        None => path.to_string(),
    }
}
