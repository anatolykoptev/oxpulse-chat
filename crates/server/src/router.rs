use std::time::Duration;

use axum::extract::ws::WebSocketUpgrade;
use axum::extract::{Path, State};
use axum::http::header::{CACHE_CONTROL, CONTENT_SECURITY_POLICY, CONTENT_TYPE, X_FRAME_OPTIONS};
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use tower_http::services::ServeDir;
use tower_http::set_header::SetResponseHeaderLayer;

#[derive(Clone)]
pub struct AppState {
    pub rooms: oxpulse_signaling::Rooms,
    pub turn_secret: String,
    pub turn_urls: Vec<String>,
    pub stun_urls: Vec<String>,
    pub pool: Option<sqlx::PgPool>,
    pub turn_pool: crate::turn_pool::TurnPool,
    pub metrics: std::sync::Arc<crate::metrics::Metrics>,
    /// If empty, /metrics returns 401 for all requests (endpoint disabled).
    pub metrics_token: String,
}

static SPA_INDEX: std::sync::OnceLock<String> = std::sync::OnceLock::new();

pub fn build_router(state: AppState, room_assets_dir: &str) -> Router {
    let immutable_dir = ServeDir::new(format!("{room_assets_dir}/_app/immutable"));
    let fonts_dir = ServeDir::new(format!("{room_assets_dir}/fonts"));
    // SPA fallback: unknown paths (e.g. /{roomId}) must serve index.html with
    // status 200 so the SvelteKit client router can take over AND link
    // previewers (Telegram/iMessage) see a valid HTML page with OG tags.
    // tower-http's ServeDir::not_found_service preserves 404 even when the
    // fallback resolves, so we use an axum handler via ServeDir::fallback
    // which does honor the handler's status code.
    let index_html_path = format!("{room_assets_dir}/index.html");
    match std::fs::read_to_string(&index_html_path) {
        Ok(body) => {
            SPA_INDEX.set(body).ok();
        }
        Err(e) => {
            tracing::warn!(
                path = %index_html_path,
                error = %e,
                "SPA index.html not found — fallback handler will serve a placeholder. \
                 This is expected in tests that pass a synthetic room_assets_dir; \
                 in production this must exist."
            );
        }
    }
    let static_dir = ServeDir::new(room_assets_dir).fallback(
        axum::handler::HandlerWithoutStateExt::into_service(spa_fallback),
    );

    let immutable =
        Router::new()
            .fallback_service(immutable_dir)
            .layer(SetResponseHeaderLayer::overriding(
                CACHE_CONTROL,
                HeaderValue::from_static("public, max-age=31536000, immutable"),
            ));

    let fonts =
        Router::new()
            .fallback_service(fonts_dir)
            .layer(SetResponseHeaderLayer::overriding(
                CACHE_CONTROL,
                HeaderValue::from_static("public, max-age=31536000, immutable"),
            ));

    Router::new()
        .route("/ws/call/{room_id}", get(ws_call))
        .route("/api/turn-credentials", post(turn_credentials))
        .route("/api/event", post(crate::analytics::ingest))
        .route("/api/health", get(health))
        .route("/metrics", get(metrics_handler))
        .route("/api/branding", get(crate::branding::handler))
        .route("/api/domains", get(crate::domains::handler))
        .route(
            "/api/partner/register",
            post(crate::partner_registry::handler),
        )
        // `/` serves the root SPA index — must go through `spa_fallback`
        // so __BRANDING_*__ placeholders are rendered per-host. Without this
        // explicit route, ServeDir would serve the raw index.html file with
        // unrendered placeholders.
        .route("/", get(spa_fallback))
        .nest("/_app/immutable", immutable)
        .nest("/fonts", fonts)
        .fallback_service(static_dir)
        .layer(SetResponseHeaderLayer::overriding(
            X_FRAME_OPTIONS,
            HeaderValue::from_static("DENY"),
        ))
        .layer(SetResponseHeaderLayer::overriding(
            CONTENT_SECURITY_POLICY,
            HeaderValue::from_static("frame-ancestors 'none'"),
        ))
        .with_state(state)
}

async fn spa_fallback(req_headers: HeaderMap) -> impl IntoResponse {
    let host = crate::branding::extract_host(&req_headers);
    let cfg = crate::branding::resolve_by_host(&host);
    let template = SPA_INDEX
        .get()
        .cloned()
        .unwrap_or_else(|| "<!doctype html><html><body>OxPulse</body></html>".to_string());
    // TODO(perf): cache rendered variants per host if /api/latency-p99 regresses
    let body = crate::branding::render_index(&template, cfg);
    let mut resp_headers = HeaderMap::new();
    resp_headers.insert(
        CONTENT_TYPE,
        HeaderValue::from_static("text/html; charset=utf-8"),
    );
    (StatusCode::OK, resp_headers, body)
}

async fn ws_call(
    ws: WebSocketUpgrade,
    Path(room_id): Path<String>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    oxpulse_signaling::ws_call_handler(ws, Path(room_id), State(state.rooms)).await
}

async fn turn_credentials(State(state): State<AppState>) -> axum::response::Response {
    if state.turn_secret.is_empty() {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({"error": "TURN not configured"})),
        )
            .into_response();
    }
    // Prefer the dynamic pool when at least one server is healthy. Fall
    // back to the static `turn_urls` list (backward compat) when the pool
    // is empty OR every server is currently unhealthy.
    let mut healthy = state.turn_pool.healthy();
    let turn_urls: Vec<String> = if !healthy.is_empty() {
        healthy.sort_by_key(|s| s.cfg.priority);
        healthy.iter().map(|s| s.cfg.url.clone()).collect()
    } else {
        state.turn_urls.clone()
    };
    let creds = oxpulse_turn::generate_credentials(
        &state.turn_secret,
        "chat-user",
        Duration::from_secs(86400),
        &turn_urls,
        &state.stun_urls,
    );
    (StatusCode::OK, Json(creds)).into_response()
}

async fn health() -> &'static str {
    "ok"
}

async fn metrics_handler(headers: HeaderMap, State(state): State<AppState>) -> axum::response::Response {
    if state.metrics_token.is_empty() {
        return (StatusCode::UNAUTHORIZED, "").into_response();
    }
    let provided = headers
        .get("x-internal-token")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    if !constant_time_eq(provided.as_bytes(), state.metrics_token.as_bytes()) {
        return (StatusCode::UNAUTHORIZED, "").into_response();
    }
    use prometheus::Encoder;
    let enc = prometheus::TextEncoder::new();
    let mut buf = Vec::new();
    if enc.encode(&state.metrics.registry.gather(), &mut buf).is_err() {
        return (StatusCode::INTERNAL_SERVER_ERROR, "encode failed").into_response();
    }
    (
        StatusCode::OK,
        [(CONTENT_TYPE, HeaderValue::from_static("text/plain; version=0.0.4"))],
        String::from_utf8(buf).unwrap_or_default(),
    )
        .into_response()
}

/// Length-aware constant-time byte-slice equality. Prevents timing
/// side-channel leaking the valid token shape to an attacker probing
/// /metrics with varied guesses.
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b.iter()).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

#[cfg(test)]
mod metrics_handler_tests {
    use super::constant_time_eq;

    #[test]
    fn constant_time_eq_matches() {
        assert!(constant_time_eq(b"abc123", b"abc123"));
    }
    #[test]
    fn constant_time_eq_rejects_different() {
        assert!(!constant_time_eq(b"abc123", b"abc124"));
    }
    #[test]
    fn constant_time_eq_rejects_different_len() {
        assert!(!constant_time_eq(b"abc", b"abcd"));
    }
    #[test]
    fn constant_time_eq_empty() {
        assert!(constant_time_eq(b"", b""));
        assert!(!constant_time_eq(b"", b"x"));
    }
}
