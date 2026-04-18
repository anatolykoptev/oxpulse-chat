//! HTTP surface for `POST /api/partner/register`.
//!
//! Owns the request/response wire format concerns: JSON validation,
//! per-IP rate limiting, client IP extraction from X-Forwarded-For.
//! Business logic lives in `register::register()`, rate limiting in
//! `rate_limit::check()`.

use std::net::IpAddr;

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use axum::Json;

use super::rate_limit;
use super::register::{register, RegisterRequest};
use crate::router::AppState;

/// Extract the client IP. Trusts the first `X-Forwarded-For` hop (Caddy
/// in front in prod). Falls back to the body-provided `public_ip`.
///
/// We intentionally do not use `ConnectInfo<SocketAddr>` here: the server
/// binary uses plain `axum::serve(listener, app)` without the connect-info
/// wrapper, so the extension would be absent anyway. Install.sh always
/// sends `public_ip` in the body (best-effort from `curl ifconfig.me`)
/// so direct requests still have a usable source IP for logging and
/// rate-limit keying.
fn client_ip(headers: &HeaderMap, body_ip: Option<IpAddr>) -> Option<IpAddr> {
    if let Some(v) = headers.get("x-forwarded-for").and_then(|h| h.to_str().ok()) {
        if let Some(first) = v.split(',').next().map(str::trim) {
            if let Ok(ip) = first.parse::<IpAddr>() {
                return Some(ip);
            }
        }
    }
    body_ip
}

/// Axum handler for `POST /api/partner/register`.
///
/// Extractor ordering note: in axum 0.8 the body extractor (`Json<T>`)
/// must be the last argument because it consumes the request.
pub async fn handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<RegisterRequest>,
) -> axum::response::Response {
    let Some(pool) = state.pool.as_ref() else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({
                "error": "database not configured",
                "code": "db_unavailable",
            })),
        )
            .into_response();
    };

    let Some(ip) = client_ip(&headers, body.public_ip) else {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": "could not determine client IP",
                "code": "missing_ip",
            })),
        )
            .into_response();
    };

    if !rate_limit::check(ip) {
        tracing::warn!(%ip, "partner_registry: rate limit exceeded");
        return (
            StatusCode::TOO_MANY_REQUESTS,
            Json(serde_json::json!({
                "error": "rate limit exceeded",
                "code": "rate_limited",
            })),
        )
            .into_response();
    }

    if body.partner_id.is_empty() || body.partner_id.len() > 64 {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": "partner_id must be 1..=64 chars",
                "code": "bad_partner_id",
            })),
        )
            .into_response();
    }
    if body.token.is_empty() || body.token.len() > 256 {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": "token must be 1..=256 chars",
                "code": "bad_token",
            })),
        )
            .into_response();
    }

    match register(pool, &body.partner_id, &body.domain, &body.token, ip).await {
        Ok(ok) => {
            tracing::info!(
                partner_id = %body.partner_id,
                node_id = %ok.node_id,
                %ip,
                "partner_registry: node registered"
            );
            (StatusCode::CREATED, Json(ok)).into_response()
        }
        Err(e) => {
            tracing::warn!(
                partner_id = %body.partner_id,
                code = e.code(),
                error = %e,
                %ip,
                "partner_registry: registration rejected"
            );
            (
                e.status(),
                Json(serde_json::json!({
                    "error": e.to_string(),
                    "code": e.code(),
                })),
            )
                .into_response()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn client_ip_prefers_xff() {
        let mut h = HeaderMap::new();
        h.insert("x-forwarded-for", "203.0.113.42, 10.0.0.1".parse().unwrap());
        let ip = client_ip(&h, Some("127.0.0.1".parse().unwrap()));
        assert_eq!(ip, Some("203.0.113.42".parse().unwrap()));
    }

    #[test]
    fn client_ip_falls_back_to_body() {
        let h = HeaderMap::new();
        let ip = client_ip(&h, Some("198.51.100.7".parse().unwrap()));
        assert_eq!(ip, Some("198.51.100.7".parse().unwrap()));
    }

    #[test]
    fn client_ip_returns_none_when_no_source() {
        let h = HeaderMap::new();
        assert_eq!(client_ip(&h, None), None);
    }
}
