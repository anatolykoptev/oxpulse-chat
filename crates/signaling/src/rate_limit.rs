//! Per-IP sliding-window rate limiter for WebSocket room joins (Task 4.2).
//!
//! Hand-rolled limiter backed by `DashMap<IpAddr, VecDeque<Instant>>` — no
//! external dep beyond `dashmap` (already a signaling dependency).
//!
//! Window: 60s sliding. Max hits per window: 30. Stale entries are trimmed
//! opportunistically on each `check()` and, every `JANITOR_EVERY` calls,
//! empty buckets are removed from the outer map.
//!
//! This is intentionally narrower than `tower-governor` or `governor` — those
//! are reserved for the API-side limiter (Task 4.1). For our purposes the
//! lock-per-IP granularity of DashMap is enough because each WebSocket
//! handshake only touches its own IP bucket.

use std::collections::VecDeque;
use std::net::IpAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use dashmap::DashMap;

/// How long a recorded join counts toward the per-IP quota.
pub const WINDOW: Duration = Duration::from_secs(60);
/// Max joins allowed per IP within the window.
pub const MAX_JOINS_PER_WINDOW: usize = 30;

/// Run the map-level janitor every this many calls.
const JANITOR_EVERY: u64 = 256;
/// Abuse guard: if the map grows past this size, clear it entirely.
const MAP_SIZE_CAP: usize = 50_000;

/// Sliding-window per-IP join limiter.
#[derive(Default)]
pub struct JoinLimiter {
    buckets: DashMap<IpAddr, VecDeque<Instant>>,
    calls: AtomicU64,
}

impl JoinLimiter {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a join attempt for `ip`. Returns `true` if it is allowed,
    /// `false` if the per-IP quota is exhausted.
    ///
    /// Wall-clock based via `Instant::now()`. For deterministic tests, use
    /// [`JoinLimiter::check_at`].
    pub fn check(&self, ip: IpAddr) -> bool {
        self.check_at(ip, Instant::now())
    }

    /// Same as [`JoinLimiter::check`] but with an explicit `now` — exposed
    /// for deterministic unit tests.
    pub fn check_at(&self, ip: IpAddr, now: Instant) -> bool {
        self.maybe_run_janitor(now);

        let cutoff = now.checked_sub(WINDOW);
        let mut entry = self.buckets.entry(ip).or_default();
        // Drop expired timestamps at the head of the deque (they are oldest
        // since we always push_back in monotonic order).
        if let Some(cutoff) = cutoff {
            while entry.front().is_some_and(|t| *t < cutoff) {
                entry.pop_front();
            }
        }

        if entry.len() >= MAX_JOINS_PER_WINDOW {
            return false;
        }
        entry.push_back(now);
        true
    }

    fn maybe_run_janitor(&self, now: Instant) {
        let n = self.calls.fetch_add(1, Ordering::Relaxed);
        if !n.is_multiple_of(JANITOR_EVERY) {
            return;
        }
        // Abuse guard: full clear on overflow so a spray attack cannot
        // exhaust memory.
        if self.buckets.len() >= MAP_SIZE_CAP {
            self.buckets.clear();
            return;
        }
        let cutoff = now.checked_sub(WINDOW);
        if let Some(cutoff) = cutoff {
            self.buckets.retain(|_, hits| {
                while hits.front().is_some_and(|t| *t < cutoff) {
                    hits.pop_front();
                }
                !hits.is_empty()
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ip(s: &str) -> IpAddr {
        s.parse().unwrap()
    }

    #[test]
    fn allows_up_to_max_then_blocks() {
        let limiter = JoinLimiter::new();
        let addr = ip("198.51.100.1");
        for i in 0..MAX_JOINS_PER_WINDOW {
            assert!(limiter.check(addr), "call {i} should be allowed");
        }
        assert!(!limiter.check(addr), "call N+1 must be blocked");
    }

    #[test]
    fn allows_thirty_joins_within_one_second() {
        let limiter = JoinLimiter::new();
        let addr = ip("198.51.100.2");
        let start = Instant::now();
        let step = Duration::from_millis(10);
        for i in 0..MAX_JOINS_PER_WINDOW {
            let t = start + step * (i as u32);
            assert!(
                limiter.check_at(addr, t),
                "call {i} in-window should be allowed"
            );
        }
        let t_blocked = start + step * (MAX_JOINS_PER_WINDOW as u32);
        assert!(
            !limiter.check_at(addr, t_blocked),
            "31st call must be blocked"
        );
    }

    #[test]
    fn allows_again_after_window_elapses() {
        let limiter = JoinLimiter::new();
        let addr = ip("198.51.100.3");
        let t0 = Instant::now();
        for i in 0..MAX_JOINS_PER_WINDOW {
            assert!(limiter.check_at(addr, t0 + Duration::from_millis(i as u64)));
        }
        assert!(!limiter.check_at(addr, t0 + Duration::from_millis(1)));
        // Jump past the 60-second window — the old entries age out.
        let later = t0 + WINDOW + Duration::from_secs(1);
        assert!(
            limiter.check_at(addr, later),
            "after window, new joins should be allowed"
        );
    }

    #[test]
    fn independent_ips_have_independent_buckets() {
        let limiter = JoinLimiter::new();
        let a = ip("198.51.100.4");
        let b = ip("198.51.100.5");
        let t = Instant::now();
        for _ in 0..MAX_JOINS_PER_WINDOW {
            assert!(limiter.check_at(a, t));
        }
        assert!(!limiter.check_at(a, t));
        // `b` should still be fully unused.
        assert!(limiter.check_at(b, t));
    }
}
