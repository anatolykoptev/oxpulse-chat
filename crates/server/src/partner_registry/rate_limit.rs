//! Per-IP sliding-window rate limit for `/api/partner/register`.
//!
//! In-memory `HashMap<IpAddr, Vec<Instant>>` — small, process-local,
//! forgets state on restart. MVP: upgrade to a proper limiter (e.g.
//! `tower-governor`) when throughput metrics demand it.

use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::{Arc, LazyLock, Mutex};
use std::time::{Duration, Instant};

const RATE_LIMIT_WINDOW: Duration = Duration::from_secs(60);
const RATE_LIMIT_MAX: usize = 10;

#[derive(Default)]
struct RateWindow {
    hits: Vec<Instant>,
}

static RATE_LIMITER: LazyLock<Arc<Mutex<HashMap<IpAddr, RateWindow>>>> =
    LazyLock::new(|| Arc::new(Mutex::new(HashMap::new())));

/// Returns true if the request should proceed; false if the caller hit
/// the per-IP limit within the current window. Non-blocking.
pub fn check(ip: IpAddr) -> bool {
    let now = Instant::now();
    let cutoff = now - RATE_LIMIT_WINDOW;
    let mut guard = match RATE_LIMITER.lock() {
        Ok(g) => g,
        Err(e) => {
            tracing::warn!(error = %e, "partner_registry: rate limiter mutex poisoned");
            return true;
        }
    };
    let entry = guard.entry(ip).or_default();
    entry.hits.retain(|t| *t >= cutoff);
    if entry.hits.len() >= RATE_LIMIT_MAX {
        return false;
    }
    entry.hits.push(now);
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allows_under_threshold_then_blocks() {
        let ip: IpAddr = "192.0.2.1".parse().unwrap();
        for _ in 0..RATE_LIMIT_MAX {
            assert!(check(ip));
        }
        assert!(!check(ip));
    }
}
