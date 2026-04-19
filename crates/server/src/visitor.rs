//! Anonymous visitor-identity cookie middleware.
//!
//! Sets `ox_vid` — a UUID v4 — on the first SPA response a browser
//! sees, and renews `Max-Age` on subsequent SPA visits so active users
//! never lose the cookie. Provides a stable id for pre-registration
//! product work (engagement flows, anonymous-to-registered correlation)
//! without depending on localStorage (which is cleared by incognito,
//! manual clears, and browser migration).
//!
//! Scope: applied to SPA GETs only. API endpoints, /ws/* WebSocket
//! upgrades, and /metrics are skipped — they either don't serve HTML
//! (no browser cookie jar to update) or are infra-only.

use axum::body::Body;
use axum::extract::Request;
use axum::http::{header, HeaderValue, Method};
use axum::middleware::Next;
use axum::response::Response;

/// Cookie name. Prefix `ox_` to stay consistent with the localStorage
/// key `ox_did` used by the SPA tracker. Distinct name keeps the two
/// identifiers independent — we can correlate them later without
/// guessing which source wrote which.
const COOKIE_NAME: &str = "ox_vid";

/// 400 days — the modern browser cap (RFC 6265bis, Chrome 104+). Firefox
/// lets us go up to ~2 years but there's no point specifying a value the
/// largest market share clamps.
const MAX_AGE_SECS: u64 = 400 * 24 * 3600;

/// Middleware: ensures every SPA-bound response carries `ox_vid`.
/// Existing cookie (even if just received on the request) is respected —
/// we never overwrite a stable id with a fresh one. No cookie present →
/// generate a UUID v4 and attach Set-Cookie.
pub async fn ensure_visitor_cookie(req: Request<Body>, next: Next) -> Response {
    // Skip non-SPA traffic up front so we don't churn header maps on
    // hot paths like /api/event or /ws.
    let skip = !matches!(req.method(), &Method::GET)
        || is_non_spa_path(req.uri().path());

    if skip {
        return next.run(req).await;
    }

    let has_cookie = request_has_vid(&req);
    let mut response = next.run(req).await;

    // Only set on successful SPA responses — avoid attaching cookies to
    // error pages the browser might discard without rendering.
    let status = response.status();
    if !status.is_success() && !status.is_redirection() {
        return response;
    }

    if !has_cookie {
        let vid = uuid::Uuid::new_v4().to_string();
        if let Ok(val) = HeaderValue::from_str(&build_set_cookie(&vid)) {
            response.headers_mut().append(header::SET_COOKIE, val);
        }
    }
    response
}

fn is_non_spa_path(path: &str) -> bool {
    path.starts_with("/api/")
        || path.starts_with("/ws/")
        || path == "/metrics"
        || path == "/api/health"
        || path.starts_with("/_app/")
        || path.starts_with("/fonts/")
}

fn request_has_vid(req: &Request<Body>) -> bool {
    let Some(raw) = req.headers().get(header::COOKIE) else {
        return false;
    };
    let Ok(cookies) = raw.to_str() else {
        return false;
    };
    cookie_has_name(cookies, COOKIE_NAME)
}

fn cookie_has_name(header_value: &str, target: &str) -> bool {
    for part in header_value.split(';') {
        let part = part.trim();
        // Match `name=…` — ignore pure attributes like `Secure` that
        // should not appear in a request Cookie header but we're defensive.
        if let Some(eq) = part.find('=') {
            if &part[..eq] == target && !part[eq + 1..].is_empty() {
                return true;
            }
        }
    }
    false
}

fn build_set_cookie(value: &str) -> String {
    format!(
        "{name}={value}; Max-Age={max_age}; Path=/; Secure; HttpOnly; SameSite=Lax",
        name = COOKIE_NAME,
        value = value,
        max_age = MAX_AGE_SECS,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cookie_has_name_matches_single() {
        assert!(cookie_has_name("ox_vid=abc", "ox_vid"));
    }

    #[test]
    fn cookie_has_name_matches_multi() {
        assert!(cookie_has_name("sess=x; ox_vid=y; foo=z", "ox_vid"));
    }

    #[test]
    fn cookie_has_name_rejects_missing() {
        assert!(!cookie_has_name("sess=x; other=y", "ox_vid"));
    }

    #[test]
    fn cookie_has_name_rejects_empty_value() {
        assert!(!cookie_has_name("ox_vid=", "ox_vid"));
    }

    #[test]
    fn cookie_has_name_rejects_prefix_collision() {
        // ox_vid_extra should NOT match ox_vid.
        assert!(!cookie_has_name("ox_vid_extra=y", "ox_vid"));
    }

    #[test]
    fn set_cookie_contains_required_attributes() {
        let s = build_set_cookie("abc-123");
        assert!(s.contains("ox_vid=abc-123"));
        assert!(s.contains("Max-Age=34560000"));
        assert!(s.contains("Path=/"));
        assert!(s.contains("Secure"));
        assert!(s.contains("HttpOnly"));
        assert!(s.contains("SameSite=Lax"));
    }

    #[test]
    fn is_non_spa_path_skips_api_and_ws() {
        assert!(is_non_spa_path("/api/event"));
        assert!(is_non_spa_path("/api/health"));
        assert!(is_non_spa_path("/ws/call/room-123"));
        assert!(is_non_spa_path("/metrics"));
        assert!(is_non_spa_path("/_app/immutable/x.js"));
        assert!(is_non_spa_path("/fonts/roboto.woff2"));
        assert!(!is_non_spa_path("/"));
        assert!(!is_non_spa_path("/room-abc"));
    }
}
