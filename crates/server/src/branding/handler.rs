//! HTTP handler for `GET /api/branding`.
//!
//! Separated from the data and resolution logic so that the HTTP
//! surface lives in its own concern boundary.

use axum::http::header::CONTENT_TYPE;
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::IntoResponse;

/// Resolves the partner branding config for the requesting host and returns
/// it as JSON. Uses `X-Forwarded-Host` with a fallback to `Host`.
pub async fn handler(headers: HeaderMap) -> impl IntoResponse {
    let host = headers
        .get("x-forwarded-host")
        .or_else(|| headers.get("host"))
        .and_then(|h| h.to_str().ok())
        .unwrap_or("")
        .split(':')
        .next()
        .unwrap_or("");
    let cfg = crate::branding::resolve_by_host(host);
    match serde_json::to_vec(cfg) {
        Ok(body) => (
            StatusCode::OK,
            [(CONTENT_TYPE, HeaderValue::from_static("application/json"))],
            body,
        )
            .into_response(),
        Err(e) => {
            tracing::error!(error = %e, "branding: serialization failed");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                [(CONTENT_TYPE, HeaderValue::from_static("application/json"))],
                br#"{"error":"serialization failed"}"#.to_vec(),
            )
                .into_response()
        }
    }
}
