use std::time::Duration;

use axum::extract::ws::WebSocketUpgrade;
use axum::extract::{ConnectInfo, Path, State};
use axum::http::header::{CACHE_CONTROL, CONTENT_SECURITY_POLICY, CONTENT_TYPE, X_FRAME_OPTIONS};
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::middleware::from_fn_with_state;
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use tower_http::services::ServeDir;
use tower_http::set_header::SetResponseHeaderLayer;

use crate::rate_limit::{make_limiter, rate_limit_middleware};

#[derive(Clone)]
pub struct AppState {
    pub rooms: oxpulse_signaling::Rooms,
    pub turn_secret: String,
    pub turn_urls: Vec<String>,
    pub stun_urls: Vec<String>,
    pub pool: Option<sqlx::PgPool>,
    pub turn_pool: crate::turn_pool::TurnPool,
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

    // Per-IP rate limiters (Task 4.1): one bucket-map per endpoint so a
    // /api/event flood cannot starve /api/turn-credentials and vice-versa.
    // Built once per router so state is shared across every request.
    let turn_credentials_limiter = make_limiter(30);
    let event_limiter = make_limiter(60);

    Router::new()
        .route("/ws/call/{room_id}", get(ws_call))
        .route(
            "/api/turn-credentials",
            post(turn_credentials).layer(from_fn_with_state(
                turn_credentials_limiter,
                rate_limit_middleware,
            )),
        )
        .route(
            "/api/event",
            post(crate::analytics::ingest).layer(from_fn_with_state(
                event_limiter,
                rate_limit_middleware,
            )),
        )
        .route("/api/health", get(health))
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
    headers: axum::http::HeaderMap,
    connect_info: ConnectInfo<std::net::SocketAddr>,
) -> impl IntoResponse {
    oxpulse_signaling::ws_call_handler(
        ws,
        Path(room_id),
        State(state.rooms),
        headers,
        connect_info,
    )
    .await
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
