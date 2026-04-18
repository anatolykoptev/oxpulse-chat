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
    pub metrics: std::sync::Arc<crate::metrics::Metrics>,
    /// If empty, /metrics returns 401 for all requests (endpoint disabled).
    pub metrics_token: String,
    /// Lowercased region hints that force `iceTransportPolicy: "relay"`
    /// when a client's geo hint prefix-matches. See Task 4.3.
    pub force_relay_regions: Vec<String>,
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

/// Extract a lowercased geo hint from client headers.
///
/// Prefers `X-Client-Region` (set by our edge) over `CF-IPCountry`
/// (Cloudflare-provided). Returns `None` for missing OR empty-string
/// header values so downstream code can treat "no hint" uniformly.
fn geo_hint(headers: &HeaderMap) -> Option<String> {
    headers
        .get("x-client-region")
        .or_else(|| headers.get("cf-ipcountry"))
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_ascii_lowercase())
        .filter(|s| !s.is_empty())
}

async fn turn_credentials(
    headers: HeaderMap,
    State(state): State<AppState>,
) -> axum::response::Response {
    let start = std::time::Instant::now();
    let hint = geo_hint(&headers);
    let resp = turn_credentials_inner(&state, hint.as_deref());
    state
        .metrics
        .turn_cred_latency_seconds
        .observe(start.elapsed().as_secs_f64());
    if resp.status() == StatusCode::OK {
        state.metrics.turn_creds_issued_total.inc();
    }
    resp
}

fn turn_credentials_inner(state: &AppState, hint: Option<&str>) -> axum::response::Response {
    if state.turn_secret.is_empty() {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({"error": "TURN not configured"})),
        )
            .into_response();
    }
    // Prefer the dynamic pool when at least one server is healthy. Fall
    // back to the static `turn_urls` list (backward compat) when the pool
    // is empty OR every pool server is currently unhealthy.
    let healthy = state.turn_pool.healthy();
    let turn_urls: Vec<String> = if !healthy.is_empty() {
        order_by_geo(healthy, hint)
            .into_iter()
            .map(|s| s.cfg.url.clone())
            .collect()
    } else {
        state.turn_urls.clone()
    };
    // Task 4.3: decide `iceTransportPolicy` based on relay availability and
    // configured force-relay regions. Compute BEFORE the struct construction
    // because we need to know whether a relay is actually reachable.
    let no_relay_available = turn_urls.is_empty();
    let policy = decide_ice_transport_policy(no_relay_available, hint, &state.force_relay_regions);
    let creds = oxpulse_turn::TurnCredentials {
        ice_transport_policy: policy,
        ..oxpulse_turn::generate_credentials(
            &state.turn_secret,
            "chat-user",
            Duration::from_secs(86400),
            &turn_urls,
            &state.stun_urls,
        )
    };
    (StatusCode::OK, Json(creds)).into_response()
}

/// Pure decision function for `iceTransportPolicy`. Factored out so it can
/// be unit-tested without spinning up the HTTP harness.
///
/// Rules (Task 4.3):
/// - If no relay server is available at all → `"all"` (forcing relay would
///   break the call — we always degrade open rather than break).
/// - Else if the client's `hint` prefix-matches any entry in
///   `force_regions` → `"relay"` (operator wants to hide client IPs for
///   this region).
/// - Else → `"all"` (default, preserves existing behaviour).
///
/// Matching is case-insensitive on both sides. `force_regions` entries are
/// already lowercased by `Config::from_env`; the `hint` is lowercased by
/// `geo_hint`. We still defensively `to_ascii_lowercase` the hint here so
/// callers can't pass a mixed-case value by accident.
pub fn decide_ice_transport_policy(
    no_relay_available: bool,
    hint: Option<&str>,
    force_regions: &[String],
) -> &'static str {
    if no_relay_available {
        return "all";
    }
    let Some(hint) = hint else { return "all" };
    let hint_lc = hint.to_ascii_lowercase();
    if force_regions.iter().any(|r| hint_lc.starts_with(r.as_str())) {
        "relay"
    } else {
        "all"
    }
}

/// Reorder a healthy TURN pool by region prefix match against an optional
/// geo hint. Servers whose `cfg.region` (lowercased) starts with the hint
/// come first (priority-ascending within that group), followed by the
/// remainder (also priority-ascending). With `None`/no-match, this is
/// equivalent to a simple priority sort.
fn order_by_geo(
    mut healthy: Vec<std::sync::Arc<crate::turn_pool::TurnServer>>,
    hint: Option<&str>,
) -> Vec<std::sync::Arc<crate::turn_pool::TurnServer>> {
    // Stable sort by priority first — this handles the None/no-match case
    // and also acts as the tie-break ordering within partition groups.
    healthy.sort_by_key(|s| s.cfg.priority);
    let Some(hint) = hint else {
        return healthy;
    };
    let (matched, rest): (Vec<_>, Vec<_>) = healthy
        .into_iter()
        .partition(|s| s.cfg.region.to_ascii_lowercase().starts_with(hint));
    matched.into_iter().chain(rest).collect()
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

#[cfg(test)]
mod geo_hint_tests {
    use super::{geo_hint, order_by_geo};
    use crate::config::TurnServerCfg;
    use crate::turn_pool::TurnServer;
    use axum::http::{HeaderMap, HeaderValue};
    use std::sync::atomic::{AtomicBool, AtomicU32};
    use std::sync::Arc;

    fn hdrs(pairs: &[(&'static str, &str)]) -> HeaderMap {
        let mut h = HeaderMap::new();
        for (k, v) in pairs {
            h.insert(*k, HeaderValue::from_str(v).unwrap());
        }
        h
    }

    #[test]
    fn geo_hint_prefers_x_client_region() {
        let h = hdrs(&[("x-client-region", "RU-MSK"), ("cf-ipcountry", "DE")]);
        assert_eq!(geo_hint(&h).as_deref(), Some("ru-msk"));
    }

    #[test]
    fn geo_hint_falls_back_to_cf_ipcountry() {
        let h = hdrs(&[("cf-ipcountry", "RU")]);
        assert_eq!(geo_hint(&h).as_deref(), Some("ru"));
    }

    #[test]
    fn geo_hint_none_without_headers() {
        let h = HeaderMap::new();
        assert_eq!(geo_hint(&h), None);
    }

    #[test]
    fn geo_hint_none_for_empty_header() {
        let h = hdrs(&[("x-client-region", "")]);
        assert_eq!(geo_hint(&h), None);
        let h = hdrs(&[("cf-ipcountry", "")]);
        assert_eq!(geo_hint(&h), None);
    }

    #[test]
    fn geo_hint_lowercases() {
        let h = hdrs(&[("x-client-region", "Ru-SpB")]);
        assert_eq!(geo_hint(&h).as_deref(), Some("ru-spb"));
    }

    // --- order_by_geo coverage (exercises the reordering logic used by
    // turn_credentials_inner). The integration test in
    // tests/turn_credentials.rs covers the end-to-end HTTP path.

    fn srv(region: &str, priority: i32, url: &str) -> Arc<TurnServer> {
        Arc::new(TurnServer {
            cfg: TurnServerCfg {
                url: url.to_string(),
                region: region.to_string(),
                priority,
            },
            healthy: AtomicBool::new(true),
            consecutive_failures: AtomicU32::new(0),
            last_rtt_ms: AtomicU32::new(0),
        })
    }

    #[test]
    fn order_by_geo_prefers_prefix_match_then_priority() {
        let pool = vec![
            srv("ru-spb", 0, "turn:ru-spb"),
            srv("de-fra", 0, "turn:de-fra"),
            srv("ru-msk", 5, "turn:ru-msk"),
        ];
        let ordered = order_by_geo(pool, Some("ru"));
        let urls: Vec<_> = ordered.iter().map(|s| s.cfg.url.clone()).collect();
        assert_eq!(urls, vec!["turn:ru-spb", "turn:ru-msk", "turn:de-fra"]);
    }

    #[test]
    fn order_by_geo_no_hint_sorts_by_priority() {
        let pool = vec![
            srv("de-fra", 5, "turn:de-fra"),
            srv("ru-msk", 0, "turn:ru-msk"),
        ];
        let ordered = order_by_geo(pool, None);
        let urls: Vec<_> = ordered.iter().map(|s| s.cfg.url.clone()).collect();
        assert_eq!(urls, vec!["turn:ru-msk", "turn:de-fra"]);
    }

    #[test]
    fn order_by_geo_unknown_hint_sorts_by_priority() {
        let pool = vec![
            srv("de-fra", 5, "turn:de-fra"),
            srv("ru-msk", 0, "turn:ru-msk"),
        ];
        let ordered = order_by_geo(pool, Some("unknown"));
        let urls: Vec<_> = ordered.iter().map(|s| s.cfg.url.clone()).collect();
        assert_eq!(urls, vec!["turn:ru-msk", "turn:de-fra"]);
    }
}

#[cfg(test)]
mod ice_policy_tests {
    use super::decide_ice_transport_policy;

    #[test]
    fn decide_policy_defaults_to_all() {
        // Pool empty, no force regions → "all".
        assert_eq!(decide_ice_transport_policy(true, None, &[]), "all");
    }

    #[test]
    fn decide_policy_returns_all_when_pool_empty_even_with_force() {
        // No relay available at all — forcing relay would break the client,
        // so we degrade open to "all" even when the hint matches the list.
        let force = vec!["ru".to_string()];
        assert_eq!(
            decide_ice_transport_policy(true, Some("ru-spb"), &force),
            "all"
        );
    }

    #[test]
    fn decide_policy_returns_relay_on_match() {
        // Pool populated, hint prefix-matches a force entry.
        let force = vec!["ru".to_string()];
        assert_eq!(
            decide_ice_transport_policy(false, Some("ru-spb"), &force),
            "relay"
        );
    }

    #[test]
    fn decide_policy_case_insensitive_match() {
        // Hint arrives uppercase but we lowercase internally; force_regions
        // are already lowercased by Config::from_env.
        let force = vec!["ru-msk".to_string()];
        assert_eq!(
            decide_ice_transport_policy(false, Some("RU-MSK"), &force),
            "relay"
        );
    }

    #[test]
    fn decide_policy_no_match_returns_all() {
        let force = vec!["ru".to_string()];
        assert_eq!(
            decide_ice_transport_policy(false, Some("de-fra"), &force),
            "all"
        );
    }

    #[test]
    fn decide_policy_none_hint_returns_all() {
        // No hint from client → can't decide to force relay for them.
        let force = vec!["ru".to_string()];
        assert_eq!(decide_ice_transport_policy(false, None, &force), "all");
    }
}
