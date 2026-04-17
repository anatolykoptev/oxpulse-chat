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
    let static_dir = ServeDir::new(room_assets_dir)
        .fallback(axum::handler::HandlerWithoutStateExt::into_service(spa_fallback));

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
        .route("/api/branding", get(branding_api))
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
    let host = req_headers
        .get("x-forwarded-host")
        .or_else(|| req_headers.get("host"))
        .and_then(|h| h.to_str().ok())
        .unwrap_or("")
        .split(':')
        .next()
        .unwrap_or("");
    let cfg = crate::branding::resolve_by_host(host);
    let template = SPA_INDEX.get().cloned().unwrap_or_else(|| {
        "<!doctype html><html><body>OxPulse</body></html>".to_string()
    });
    // TODO(perf): cache rendered variants per host if /api/latency-p99 regresses
    let body = crate::branding::render_index(&template, cfg);
    let mut resp_headers = HeaderMap::new();
    resp_headers.insert(CONTENT_TYPE, HeaderValue::from_static("text/html; charset=utf-8"));
    (StatusCode::OK, resp_headers, body)
}

/// HTTP handler for `GET /api/branding`.
/// Serializes the static `&BrandingConfig` directly — no clone per request.
async fn branding_api(headers: HeaderMap) -> impl IntoResponse {
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

async fn ws_call(
    ws: WebSocketUpgrade,
    Path(room_id): Path<String>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    oxpulse_signaling::ws_call_handler(ws, Path(room_id), State(state.rooms)).await
}

async fn turn_credentials(State(state): State<AppState>) -> impl IntoResponse {
    if state.turn_secret.is_empty() {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({"error": "TURN not configured"})),
        );
    }
    let creds = oxpulse_turn::generate_credentials(
        &state.turn_secret,
        "chat-user",
        Duration::from_secs(86400),
        &state.turn_urls,
        &state.stun_urls,
    );
    (StatusCode::OK, Json(serde_json::to_value(creds).unwrap()))
}

async fn health() -> &'static str {
    "ok"
}
