//! Per-IP rate limiting for HTTP API endpoints (Task 4.1).
//!
//! Uses [`governor`] with a [`DashMapStateStore`] so limiter state is
//! shared across concurrent requests without a mutex. Each rate-limited
//! endpoint gets its own [`KeyedLimiter`] built via [`make_limiter`] and
//! attached as a per-route layer via [`axum::middleware::from_fn_with_state`],
//! so other routes (`/api/health`, `/metrics`, WebSocket, static files)
//! stay unaffected.
//!
//! Client IP extraction prefers the first entry of `X-Forwarded-For`
//! (the closest known hop when a reverse proxy is in front), then falls
//! back to `ConnectInfo<SocketAddr>` from the axum connection. If the
//! `ConnectInfo` extension is missing (e.g. an axum-test harness that
//! does not install it), the middleware allows the request through
//! rather than hard-blocking — failing open is safer than locking tests
//! out of the surface they need to exercise.

use std::net::{IpAddr, SocketAddr};
use std::num::NonZeroU32;
use std::sync::Arc;

use axum::body::Body;
use axum::extract::{ConnectInfo, State};
use axum::http::{HeaderMap, Request, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::Json;
use governor::clock::DefaultClock;
use governor::state::keyed::DashMapStateStore;
use governor::{Quota, RateLimiter};

/// Keyed rate limiter shared across request handlers.
///
/// `Arc` so it can be cloned into the axum state without duplicating the
/// underlying DashMap of per-IP token buckets.
pub type KeyedLimiter = Arc<RateLimiter<IpAddr, DashMapStateStore<IpAddr>, DefaultClock>>;

/// Build a keyed rate limiter with `per_minute` sustained requests per IP
/// and a burst capacity of `ceil(per_minute / 2)` (at least 1).
///
/// The half-quota burst allows a short-lived spike (e.g. page load firing
/// two `/api/event` posts back-to-back) without permanently blocking the
/// caller after one bad second.
pub fn make_limiter(per_minute: u32) -> KeyedLimiter {
    let rate = NonZeroU32::new(per_minute.max(1))
        .expect("per_minute.max(1) is always >= 1, NonZeroU32 cannot fail");
    let burst = NonZeroU32::new(per_minute.div_ceil(2).max(1))
        .expect("div_ceil(2).max(1) is always >= 1, NonZeroU32 cannot fail");
    Arc::new(RateLimiter::keyed(
        Quota::per_minute(rate).allow_burst(burst),
    ))
}

/// Extract the client IP from the request.
///
/// Precedence:
///   1. `X-Forwarded-For` first comma-separated entry (closest upstream).
///   2. `ConnectInfo<SocketAddr>` peer address from the axum listener.
///   3. `None` — caller should fail open in that case.
fn extract_client_ip(headers: &HeaderMap, peer: Option<IpAddr>) -> Option<IpAddr> {
    if let Some(xff) = headers.get("x-forwarded-for").and_then(|v| v.to_str().ok()) {
        if let Some(first) = xff.split(',').next().and_then(|s| s.trim().parse().ok()) {
            return Some(first);
        }
    }
    peer
}

/// Axum middleware: enforce the per-IP quota on every request that
/// traverses this layer.
///
/// On `Ok(())` from `check_key`, the request proceeds normally.
/// On `Err(NotUntil)`, responds `429 Too Many Requests` with a
/// `Retry-After` header (seconds) and a JSON body `{"error":
/// "rate_limited", "retry_after_secs": N}`.
///
/// `Retry-After` is derived from the quota's replenish interval rather
/// than `NotUntil::wait_time_from`, which would require a governor
/// `Clock` instance. The replenish interval is the correct conservative
/// hint: it is the minimum wait before at least one token is restored,
/// and for `Quota::per_minute(N)` it is exactly `60/N` seconds.
pub async fn rate_limit_middleware(
    State(limiter): State<KeyedLimiter>,
    request: Request<Body>,
    next: Next,
) -> Response {
    let peer = request
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map(|ci| ci.0.ip());

    // Fail-open when no IP source is available (e.g. test harness without
    // `into_make_service_with_connect_info`). Production always has a peer.
    let Some(ip) = extract_client_ip(request.headers(), peer) else {
        return next.run(request).await;
    };

    match limiter.check_key(&ip) {
        Ok(()) => next.run(request).await,
        Err(not_until) => {
            // Replenish interval = 60 / per_minute seconds; round up to
            // at least 1 s so clients don't hot-loop.
            let retry_after = not_until
                .quota()
                .replenish_interval()
                .as_secs()
                .max(1);

            tracing::warn!(%ip, retry_after, "api rate limit exceeded");

            (
                StatusCode::TOO_MANY_REQUESTS,
                [(axum::http::header::RETRY_AFTER, retry_after.to_string())],
                Json(serde_json::json!({
                    "error": "rate_limited",
                    "retry_after_secs": retry_after,
                })),
            )
                .into_response()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;

    #[test]
    fn make_limiter_respects_quota() {
        // Build a 2/min limiter directly so we can assert exact burst=1
        // behaviour without fighting the div_ceil(2).max(1) rounding that
        // `make_limiter` applies. This test exercises the Quota plumbing
        // end-to-end: 2/min with burst=1 means exactly one hit is allowed
        // before the token bucket is empty.
        let quota = Quota::per_minute(NonZeroU32::new(2).unwrap())
            .allow_burst(NonZeroU32::new(1).unwrap());
        let limiter: KeyedLimiter = Arc::new(RateLimiter::keyed(quota));
        let ip: IpAddr = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1));

        assert!(limiter.check_key(&ip).is_ok(), "first hit must pass");
        assert!(
            limiter.check_key(&ip).is_err(),
            "second hit must be rate limited (burst exhausted)"
        );
    }

    #[test]
    fn burst_allows_short_spike() {
        // make_limiter(10) -> quota 10/min, burst = ceil(10/2) = 5.
        // Five tight-loop hits from the same IP must all succeed; the
        // sixth must be rejected.
        let limiter = make_limiter(10);
        let ip: IpAddr = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1));

        for i in 0..5 {
            assert!(
                limiter.check_key(&ip).is_ok(),
                "burst hit #{i} must pass (burst cap = 5)"
            );
        }
        assert!(
            limiter.check_key(&ip).is_err(),
            "6th tight-loop hit must be rejected (burst exhausted)"
        );
    }

    #[test]
    fn extract_client_ip_prefers_xff_first_hop() {
        let mut h = HeaderMap::new();
        h.insert(
            "x-forwarded-for",
            "203.0.113.7, 10.0.0.1, 10.0.0.2".parse().unwrap(),
        );
        let peer = Some(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)));
        let ip = extract_client_ip(&h, peer).expect("XFF should yield an IP");
        assert_eq!(ip, "203.0.113.7".parse::<IpAddr>().unwrap());
    }

    #[test]
    fn extract_client_ip_falls_back_to_peer() {
        let h = HeaderMap::new();
        let peer = Some(IpAddr::V4(Ipv4Addr::new(198, 51, 100, 42)));
        let ip = extract_client_ip(&h, peer).expect("peer fallback should yield an IP");
        assert_eq!(ip, "198.51.100.42".parse::<IpAddr>().unwrap());
    }

    #[test]
    fn extract_client_ip_returns_none_when_no_source() {
        let h = HeaderMap::new();
        assert_eq!(extract_client_ip(&h, None), None);
    }

    #[test]
    fn extract_client_ip_ignores_malformed_xff() {
        let mut h = HeaderMap::new();
        h.insert("x-forwarded-for", "not-an-ip".parse().unwrap());
        let peer = Some(IpAddr::V4(Ipv4Addr::new(10, 0, 0, 7)));
        let ip = extract_client_ip(&h, peer).expect("should fall back to peer");
        assert_eq!(ip, "10.0.0.7".parse::<IpAddr>().unwrap());
    }
}
