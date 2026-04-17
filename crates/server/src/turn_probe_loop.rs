//! Background worker that probes TURN servers in a [`TurnPool`] and
//! updates their health atomics. Split from [`crate::turn_pool`] so the
//! pool module stays focused on the state container — this file owns
//! the I/O concerns (DNS resolution, periodic ticking, STUN probe call).

use std::sync::atomic::Ordering;
use std::time::Duration;

use crate::turn_pool::TurnPool;

/// Parse `turn:host:port?transport=udp` → first resolved SocketAddr.
/// Returns None (with a tracing::warn) on any parse or resolution failure.
pub async fn parse_host_port(url: &str) -> Option<std::net::SocketAddr> {
    let rest = url
        .strip_prefix("turns:")
        .or_else(|| url.strip_prefix("turn:"))
        .unwrap_or(url);
    let host_port = rest.split('?').next().unwrap_or(rest);
    match tokio::net::lookup_host(host_port).await {
        Ok(mut addrs) => addrs.next().or_else(|| {
            tracing::warn!(url, "parse_host_port: DNS returned no addresses");
            None
        }),
        Err(e) => {
            tracing::warn!(url, error = %e, "parse_host_port: DNS lookup failed");
            None
        }
    }
}

impl TurnPool {
    /// Spawn a background task that probes each server on `interval` and
    /// updates `healthy` / `consecutive_failures` / `last_rtt_ms`. Logs on
    /// state transitions only (turn_server_up / turn_server_down).
    pub fn start_probe_task(
        &self,
        interval: Duration,
        unhealthy_after: u32,
    ) -> tokio::task::JoinHandle<()> {
        let servers = self.servers.clone();
        let handle = tokio::spawn(async move {
            let mut tick = tokio::time::interval(interval);
            loop {
                tick.tick().await;
                for server in servers.iter() {
                    let addr = match parse_host_port(&server.cfg.url).await {
                        Some(a) => a,
                        None => {
                            // DNS / URL parse failure counts as a probe failure so that
                            // servers with stale DNS entries eventually fall out of the
                            // healthy set instead of being silently rotated forever.
                            let fails = server
                                .consecutive_failures
                                .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
                                + 1;
                            if fails >= unhealthy_after
                                && server
                                    .healthy
                                    .swap(false, std::sync::atomic::Ordering::Relaxed)
                            {
                                tracing::warn!(
                                    region = %server.cfg.region,
                                    url = %server.cfg.url,
                                    consecutive_failures = fails,
                                    "turn_server_down_dns_unresolved"
                                );
                            }
                            continue;
                        }
                    };
                    match crate::turn_probe::probe(addr, Duration::from_secs(3)).await {
                        Ok(rtt) => {
                            server.consecutive_failures.store(0, Ordering::Relaxed);
                            server.last_rtt_ms.store(rtt, Ordering::Relaxed);
                            if !server.healthy.swap(true, Ordering::Relaxed) {
                                tracing::info!(
                                    region = %server.cfg.region,
                                    url = %server.cfg.url,
                                    rtt_ms = rtt,
                                    "turn_server_up"
                                );
                            }
                        }
                        Err(e) => {
                            let fails =
                                server.consecutive_failures.fetch_add(1, Ordering::Relaxed) + 1;
                            if fails >= unhealthy_after
                                && server.healthy.swap(false, Ordering::Relaxed)
                            {
                                tracing::warn!(
                                    region = %server.cfg.region,
                                    url = %server.cfg.url,
                                    consecutive_failures = fails,
                                    error = %e,
                                    "turn_server_down"
                                );
                            }
                        }
                    }
                }
            }
        });
        handle
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::TurnServerCfg;

    // Minimal STUN reply builder for the probe-loop tests. Mirrors the
    // 20-byte header the `turn_probe` client sends, echoing the 12-byte
    // transaction ID so the probe accepts it as a matching response.
    const STUN_BINDING_SUCCESS: u16 = 0x0101;
    const STUN_MAGIC_COOKIE: u32 = 0x2112_A442;

    fn stun_success_reply(req: &[u8]) -> [u8; 20] {
        let mut resp = [0u8; 20];
        resp[0..2].copy_from_slice(&STUN_BINDING_SUCCESS.to_be_bytes());
        resp[4..8].copy_from_slice(&STUN_MAGIC_COOKIE.to_be_bytes());
        resp[8..20].copy_from_slice(&req[8..20]);
        resp
    }

    fn cfg(region: &str, priority: i32, url: &str) -> TurnServerCfg {
        TurnServerCfg {
            url: url.to_string(),
            region: region.to_string(),
            priority,
        }
    }

    #[tokio::test]
    async fn probe_loop_flips_healthy_to_false_after_consecutive_failures() {
        // Silent UDP socket: probe times out every tick.
        let silent = tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let addr = silent.local_addr().unwrap();
        let _guard = silent;

        let pool = TurnPool::new(vec![cfg(
            "ru-msk",
            0,
            &format!("turn:{}:{}", addr.ip(), addr.port()),
        )]);
        pool.start_probe_task(Duration::from_millis(50), 2);

        // With 3s probe timeout, 2 failures accrue at ~3s cadence; allow 8s.
        let deadline = std::time::Instant::now() + Duration::from_secs(8);
        let server = pool.all()[0].clone();
        while server.is_healthy() {
            if std::time::Instant::now() >= deadline {
                panic!(
                    "server not marked unhealthy within 8s; failures={}",
                    server.consecutive_failures.load(Ordering::Relaxed)
                );
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        assert!(server.consecutive_failures.load(Ordering::Relaxed) >= 2);
    }

    #[tokio::test]
    async fn probe_loop_records_rtt_and_keeps_server_healthy_on_success() {
        // Mock STUN server that echoes every request with Binding-Success.
        let mock = tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let addr = mock.local_addr().unwrap();

        tokio::spawn(async move {
            let mut buf = [0u8; 1500];
            while let Ok((_, peer)) = mock.recv_from(&mut buf).await {
                let _ = mock.send_to(&stun_success_reply(&buf), peer).await;
            }
        });

        let pool = TurnPool::new(vec![cfg(
            "ru-msk",
            0,
            &format!("turn:{}:{}", addr.ip(), addr.port()),
        )]);
        pool.start_probe_task(Duration::from_millis(50), 3);

        let deadline = std::time::Instant::now() + Duration::from_secs(3);
        let server = pool.all()[0].clone();
        loop {
            tokio::time::sleep(Duration::from_millis(150)).await;
            if server.is_healthy() && server.consecutive_failures.load(Ordering::Relaxed) == 0 {
                break;
            }
            if std::time::Instant::now() >= deadline {
                panic!(
                    "server not healthy after probes; failures={}",
                    server.consecutive_failures.load(Ordering::Relaxed)
                );
            }
        }
        assert!(server.is_healthy());
        assert_eq!(server.consecutive_failures.load(Ordering::Relaxed), 0);
    }
}
