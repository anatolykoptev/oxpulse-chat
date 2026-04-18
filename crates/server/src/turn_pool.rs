//! In-memory TURN server pool with per-server health flags.
//!
//! This module owns ONLY the state container: `TurnServer`, `TurnPool`,
//! accessors (`healthy`, `all`, `len`). The background probe worker that
//! mutates those atomics lives in [`crate::turn_probe_loop`] to keep this
//! file focused on data rather than I/O.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;

use arc_swap::ArcSwap;

use crate::config::TurnServerCfg;

/// A single TURN server tracked by the pool.
///
/// `healthy` starts `true` (optimistic) and is flipped to `false` by the
/// probe task after `unhealthy_after_fails` consecutive failures.
pub struct TurnServer {
    pub cfg: TurnServerCfg,
    pub healthy: AtomicBool,
    pub consecutive_failures: AtomicU32,
    pub last_rtt_ms: AtomicU32,
}

impl TurnServer {
    pub fn region(&self) -> &str {
        &self.cfg.region
    }
    pub fn url(&self) -> &str {
        &self.cfg.url
    }
    pub fn priority(&self) -> i32 {
        self.cfg.priority
    }
    pub fn is_healthy(&self) -> bool {
        self.healthy.load(Ordering::Relaxed)
    }
    pub fn last_rtt_ms(&self) -> u32 {
        self.last_rtt_ms.load(Ordering::Relaxed)
    }
}

/// A thread-safe collection of TURN servers with per-server health flags.
///
/// The inner storage is an [`ArcSwap`] over `Vec<Arc<TurnServer>>` so the
/// list can be atomically replaced by `reload` (Task 2.6) without blocking
/// concurrent readers. Each `load()` returns a consistent snapshot of the
/// pool — readers never see a torn mix of old and new entries.
#[derive(Clone)]
pub struct TurnPool {
    pub(crate) servers: Arc<ArcSwap<Vec<Arc<TurnServer>>>>,
}

impl Default for TurnPool {
    fn default() -> Self {
        Self::empty()
    }
}

impl TurnPool {
    /// Construct an empty pool with no servers — useful for tests and for
    /// environments where TURN is off.
    pub fn empty() -> Self {
        Self::new(Vec::new())
    }

    pub fn new(cfgs: Vec<TurnServerCfg>) -> Self {
        let servers = build_servers(cfgs);
        Self {
            servers: Arc::new(ArcSwap::from_pointee(servers)),
        }
    }

    /// All servers currently flagged healthy. Caller decides ordering.
    pub fn healthy(&self) -> Vec<Arc<TurnServer>> {
        self.servers
            .load()
            .iter()
            .filter(|s| s.is_healthy())
            .cloned()
            .collect()
    }

    /// Every configured server regardless of health. Useful for /metrics
    /// and the probe loop.
    pub fn all(&self) -> Vec<Arc<TurnServer>> {
        self.servers.load().iter().cloned().collect()
    }

    pub fn len(&self) -> usize {
        self.servers.load().len()
    }
    pub fn is_empty(&self) -> bool {
        self.servers.load().is_empty()
    }

    /// Atomically replace the pool with a freshly-built list from
    /// `new_cfgs`. For each entry whose `url` is present in both the old
    /// and new sets we preserve `healthy` / `consecutive_failures` /
    /// `last_rtt_ms` so an otherwise-unchanged server does not lose its
    /// probe history across a hot-reload.
    ///
    /// Returns `(added, removed, preserved)` URL counts for logging.
    pub fn reload(&self, new_cfgs: Vec<TurnServerCfg>) -> (usize, usize, usize) {
        let old = self.servers.load_full();
        let by_url: HashMap<&str, &Arc<TurnServer>> =
            old.iter().map(|s| (s.cfg.url.as_str(), s)).collect();

        let mut preserved = 0usize;
        let mut added = 0usize;
        let new_vec: Vec<Arc<TurnServer>> = new_cfgs
            .into_iter()
            .map(|cfg| match by_url.get(cfg.url.as_str()) {
                Some(existing) => {
                    preserved += 1;
                    Arc::new(TurnServer {
                        cfg,
                        healthy: AtomicBool::new(existing.healthy.load(Ordering::Relaxed)),
                        consecutive_failures: AtomicU32::new(
                            existing.consecutive_failures.load(Ordering::Relaxed),
                        ),
                        last_rtt_ms: AtomicU32::new(existing.last_rtt_ms.load(Ordering::Relaxed)),
                    })
                }
                None => {
                    added += 1;
                    Arc::new(TurnServer {
                        cfg,
                        healthy: AtomicBool::new(true),
                        consecutive_failures: AtomicU32::new(0),
                        last_rtt_ms: AtomicU32::new(0),
                    })
                }
            })
            .collect();

        let removed = old.len().saturating_sub(preserved);
        self.servers.store(Arc::new(new_vec));
        (added, removed, preserved)
    }
}

/// Build fresh optimistic-healthy `TurnServer` instances from configs.
fn build_servers(cfgs: Vec<TurnServerCfg>) -> Vec<Arc<TurnServer>> {
    cfgs.into_iter()
        .map(|cfg| {
            Arc::new(TurnServer {
                cfg,
                healthy: AtomicBool::new(true),
                consecutive_failures: AtomicU32::new(0),
                last_rtt_ms: AtomicU32::new(0),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg(region: &str, priority: i32, url: &str) -> TurnServerCfg {
        TurnServerCfg {
            url: url.to_string(),
            region: region.to_string(),
            priority,
        }
    }

    #[test]
    fn new_pool_marks_all_servers_healthy() {
        let pool = TurnPool::new(vec![
            cfg("ru-msk", 0, "turn:host1:3478"),
            cfg("de-fra", 1, "turn:host2:3478"),
        ]);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.healthy().len(), 2);
    }

    #[test]
    fn flipping_healthy_to_false_removes_from_healthy_list() {
        let pool = TurnPool::new(vec![
            cfg("ru-msk", 0, "turn:host1:3478"),
            cfg("de-fra", 1, "turn:host2:3478"),
        ]);
        let all = pool.all();
        all[0].healthy.store(false, Ordering::Relaxed);
        let healthy = pool.healthy();
        assert_eq!(healthy.len(), 1);
        assert_eq!(healthy[0].region(), "de-fra");
    }

    #[test]
    fn empty_pool() {
        let pool = TurnPool::new(vec![]);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert!(pool.healthy().is_empty());
    }

    #[test]
    fn reload_preserves_healthy_flag_for_matched_urls() {
        let pool = TurnPool::new(vec![cfg("ru-msk", 0, "turn:hostA:3478")]);
        // Simulate probe loop marking A unhealthy.
        pool.all()[0].healthy.store(false, Ordering::Relaxed);

        let (added, removed, preserved) = pool.reload(vec![
            cfg("ru-msk", 0, "turn:hostA:3478"),
            cfg("de-fra", 1, "turn:hostB:3478"),
        ]);
        assert_eq!((added, removed, preserved), (1, 0, 1));

        let all = pool.all();
        assert_eq!(all.len(), 2);
        let a = all.iter().find(|s| s.url() == "turn:hostA:3478").unwrap();
        let b = all.iter().find(|s| s.url() == "turn:hostB:3478").unwrap();
        assert!(!a.is_healthy(), "A unhealthy flag must be preserved");
        assert!(b.is_healthy(), "B starts optimistic-true");
    }

    #[test]
    fn reload_drops_absent_urls() {
        let pool = TurnPool::new(vec![
            cfg("ru-msk", 0, "turn:hostA:3478"),
            cfg("de-fra", 1, "turn:hostB:3478"),
        ]);
        let (added, removed, preserved) =
            pool.reload(vec![cfg("ru-msk", 0, "turn:hostA:3478")]);
        assert_eq!((added, removed, preserved), (0, 1, 1));

        let all = pool.all();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].url(), "turn:hostA:3478");
    }

    #[test]
    fn reload_adds_new_urls() {
        let pool = TurnPool::new(vec![cfg("ru-msk", 0, "turn:hostA:3478")]);
        let (added, removed, preserved) = pool.reload(vec![
            cfg("ru-msk", 0, "turn:hostA:3478"),
            cfg("de-fra", 1, "turn:hostB:3478"),
        ]);
        assert_eq!((added, removed, preserved), (1, 0, 1));
        let urls: Vec<String> = pool.all().iter().map(|s| s.url().to_string()).collect();
        assert!(urls.contains(&"turn:hostA:3478".to_string()));
        assert!(urls.contains(&"turn:hostB:3478".to_string()));
    }

    #[test]
    fn reload_preserves_consecutive_failures() {
        let pool = TurnPool::new(vec![cfg("ru-msk", 0, "turn:hostA:3478")]);
        pool.all()[0]
            .consecutive_failures
            .store(5, Ordering::Relaxed);
        pool.all()[0].last_rtt_ms.store(42, Ordering::Relaxed);

        let (_, _, preserved) =
            pool.reload(vec![cfg("ru-msk", 0, "turn:hostA:3478")]);
        assert_eq!(preserved, 1);

        let a = &pool.all()[0];
        assert_eq!(a.consecutive_failures.load(Ordering::Relaxed), 5);
        assert_eq!(a.last_rtt_ms.load(Ordering::Relaxed), 42);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn reload_is_atomic() {
        let pool = TurnPool::new(vec![
            cfg("r1", 0, "turn:a:1"),
            cfg("r2", 1, "turn:b:1"),
            cfg("r3", 2, "turn:c:1"),
        ]);
        let cfg_small = vec![cfg("r1", 0, "turn:a:1")];
        let cfg_big = vec![
            cfg("r1", 0, "turn:a:1"),
            cfg("r2", 1, "turn:b:1"),
            cfg("r3", 2, "turn:c:1"),
        ];
        let valid_lens = [1usize, 3usize];

        let stop = Arc::new(AtomicBool::new(false));
        let mut readers = Vec::new();
        for _ in 0..10 {
            let pool = pool.clone();
            let stop = stop.clone();
            readers.push(tokio::spawn(async move {
                while !stop.load(Ordering::Relaxed) {
                    let v = pool.all();
                    assert!(
                        valid_lens.contains(&v.len()),
                        "torn read: len={}",
                        v.len()
                    );
                    // Every entry must be a real Arc.
                    for s in &v {
                        let _ = s.url();
                    }
                }
            }));
        }

        for _ in 0..200 {
            pool.reload(cfg_small.clone());
            tokio::task::yield_now().await;
            pool.reload(cfg_big.clone());
            tokio::task::yield_now().await;
        }
        stop.store(true, Ordering::Relaxed);
        for r in readers {
            r.await.unwrap();
        }
    }
}
