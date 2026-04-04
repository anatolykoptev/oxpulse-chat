use std::time::Duration;

use axum::extract::ws::WebSocketUpgrade;
use axum::extract::{Path, State};
use axum::http::header::{CACHE_CONTROL, CONTENT_SECURITY_POLICY, X_FRAME_OPTIONS};
use axum::http::{HeaderValue, StatusCode};
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

pub fn build_router(state: AppState, room_assets_dir: &str) -> Router {
    let immutable_dir = ServeDir::new(format!("{room_assets_dir}/_app/immutable"));
    let fonts_dir = ServeDir::new(format!("{room_assets_dir}/fonts"));
    let static_dir = ServeDir::new(room_assets_dir);

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
