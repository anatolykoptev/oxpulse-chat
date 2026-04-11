use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;

use crate::config::TurnServerCfg;

/// A single TURN server tracked by the pool.
///
/// `healthy` starts `true` (optimistic) and is flipped to `false` by the
/// probe task (Task 2.3) after `unhealthy_after_fails` consecutive failures.
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
/// Task 2.1 provides only the container and accessors. Task 2.2 adds the
/// STUN probe primitive and Task 2.3 wires a background task that calls
/// into the probe and updates `healthy` / `consecutive_failures` /
/// `last_rtt_ms`. Task 2.6 will replace the inner `Arc<Vec<_>>` with an
/// `ArcSwap` for hot-reload.
#[derive(Clone)]
pub struct TurnPool {
    servers: Arc<Vec<Arc<TurnServer>>>,
}

impl TurnPool {
    pub fn new(cfgs: Vec<TurnServerCfg>) -> Self {
        let servers = cfgs
            .into_iter()
            .map(|cfg| {
                Arc::new(TurnServer {
                    cfg,
                    healthy: AtomicBool::new(true),
                    consecutive_failures: AtomicU32::new(0),
                    last_rtt_ms: AtomicU32::new(0),
                })
            })
            .collect::<Vec<_>>();
        Self {
            servers: Arc::new(servers),
        }
    }

    /// All servers currently flagged healthy. Caller decides ordering.
    pub fn healthy(&self) -> Vec<Arc<TurnServer>> {
        self.servers
            .iter()
            .filter(|s| s.is_healthy())
            .cloned()
            .collect()
    }

    /// Every configured server regardless of health. Useful for /metrics
    /// and the probe loop.
    pub fn all(&self) -> Vec<Arc<TurnServer>> {
        self.servers.iter().cloned().collect()
    }

    pub fn len(&self) -> usize {
        self.servers.len()
    }
    pub fn is_empty(&self) -> bool {
        self.servers.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::TurnServerCfg;

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
}
