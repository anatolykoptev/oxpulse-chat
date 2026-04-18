# OxPulse Chat — Phase 2 Continuation (Partner Launch)

> **For agentic workers:** Use superpowers:subagent-driven-development to execute.
> Dispatch via `Agent` tool. Model selection per task specified below (Haiku/Sonnet/Opus).
> Checkbox `- [ ]` syntax for tracking.

**Goal:** Finish production hardening for oxpulse-chat partner launch. TurnPool integration, metrics, alerts, abuse protection, load testing, launch checklist.

**Branch:** main. **Workspace:** `$OPERATOR_WORKSPACE`. **Deploy:** `$OPERATOR_DEPLOY`.

**Tech stack:** Rust 1.88 + Axum 0.8 + tower-http 0.6 + SQLx + DashMap + tokio; SvelteKit 5 + TypeScript; PostgreSQL 17; Dozor for monitoring; Prometheus text format for /metrics.

---

## Session bootstrap (for a new Claude Code session)

When opening this plan in a fresh session, do these first:

1. `cd $OPERATOR_WORKSPACE`
2. `git log --oneline main -15` — confirm you see these recent commits in order:
   - `aafedc7` fix(server): don't panic on missing index.html
   - `547a35f` feat(server): TurnPool skeleton with config parsing
   - `e83cfff` feat(server): STUN binding-request health probe
   - `2de3039` docs(postmortem): router 404 Telegram link preview
   - `2bc5dbd` docs(postmortem): analytics silent drop via .bind() off-by-one
   - `9adc7f2` docs(design): TurnPool design doc
   - `b5648ea` docs(slo): define four SLIs for oxpulse-chat
3. `cargo test --workspace 2>&1 | grep "test result"` — all suites green (no regressions from today)
4. Read `docs/design/turn-pool.md` (architectural context for Phase 2.3-2.6)
5. Read `docs/slo/oxpulse-chat.yaml` (metric names required by Phase 3.2)
6. Read the "Status of foundation" section below
7. Invoke skill: `superpowers:subagent-driven-development`
8. Begin with Task 2.3 using the model specified

---

## Status of foundation (as of 2026-04-10)

### Already done (do not re-implement):

**Phase 1 — Stabilization** — all deployed to production:
- Integration test for SPA fallback (`3a98953`)
- Analytics DB integration test (`3917ffb`, `11f1809`)
- Live E2E via reqwest gated on `E2E_BASE_URL` (`e09f2e8`)
- Duplicate `call_connected` guard in `useCall.svelte.ts` (`080808a`)
- `closed` vs `failed` split (`22baf18`)
- `$derived.by` for timerStr (`e560b88`)
- CI extended with web checks + live E2E job (`562633e`)

**Phase 0 — Google-style foundation:**
- TurnPool design doc (`9adc7f2`)
- SLO definitions yaml (`b5648ea`)
- Error budget policy (`2af4f85`)
- Router 404 postmortem (`2de3039`)
- Analytics drop postmortem (`2bc5dbd`)
- Postmortem template (`7adad21`)

**Phase 2 foundation** — compiled but not yet wired into handler:
- `TurnPool`/`TurnServer` types + `parse_turn_servers` env parser (`547a35f`)
- STUN Binding-Request probe primitive in `turn_probe.rs` (`e83cfff`)
- Partner onboarding runbook (`194441c`)

**Phase 5 Task 5.3** — DB-down runbook (`a85d4ee`)

### Current `AppState` in `crates/server/src/router.rs`:
```rust
#[derive(Clone)]
pub struct AppState {
    pub rooms: oxpulse_signaling::Rooms,
    pub turn_secret: String,
    pub turn_urls: Vec<String>,
    pub stun_urls: Vec<String>,
    pub pool: Option<sqlx::PgPool>,
}
```
Task 2.4 adds `pub turn_pool: TurnPool` to this struct.

### Known traps already debugged today (avoid):

- **tower-http 0.6 `ServeDir::not_found_service`** preserves 404 status even when fallback resolves. We worked around with explicit axum handler + `OnceLock<String>` in `router.rs`. Do NOT try to revert to `not_found_service`.
- **`std::fs::read_to_string(...).unwrap_or_else(|e| panic!(...))`** in `build_router` breaks 7 pre-existing integration tests in `call_signaling.rs` that pass a synthetic `/nonexistent` path. Replaced with `match { Err => tracing::warn!(...), Ok(body) => SPA_INDEX.set(body).ok() }`. Do NOT add the panic back.
- **`SPA_INDEX: OnceLock<String>`** is shared across all test `build_router` calls in the same process. Tests that use different index.html bodies will race — all tests in `http_integration.rs` use identical fixture body.
- **`#[allow(dead_code)]`** on `config.rs` fields `turn_servers`, `turn_probe_interval_secs`, `turn_unhealthy_after_fails` — these are consumed starting in Task 2.3. Remove those attributes once Task 2.3 and 2.4 wire them in.

---

## Phase 2 — TurnPool integration (sequential, all touch router.rs/main.rs)

**Estimated:** ~2-3 hours via subagents, sequential (each task depends on the prior).

### Task 2.3: Probe loop + health transitions

**Model:** Sonnet
**Files:** `crates/server/src/turn_pool.rs` (modify), `crates/server/src/main.rs` (modify), `crates/server/src/config.rs` (remove `#[allow(dead_code)]`)
**Depends on:** Tasks 2.1 + 2.2 already landed

- [ ] **Step 1:** Add `parse_host_port` helper to `turn_pool.rs`.

  The function strips the `turn:` / `turns:` scheme prefix, chops any `?transport=…` query string, then resolves the resulting `host:port` string via `tokio::net::lookup_host` and returns the first address.

  ```rust
  /// Parse `turn:host:port?transport=udp` → first resolved SocketAddr.
  /// Returns None (with a tracing::warn) on any parse or resolution failure.
  pub async fn parse_host_port(url: &str) -> Option<std::net::SocketAddr> {
      // Strip scheme prefix: "turn:" or "turns:"
      let rest = url
          .strip_prefix("turns:")
          .or_else(|| url.strip_prefix("turn:"))
          .unwrap_or(url);
      // Drop query string
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
  ```

- [ ] **Step 2:** Add `start_probe_task` to `TurnPool` in `turn_pool.rs`.

  ```rust
  use std::time::Duration;
  use std::sync::atomic::Ordering;

  impl TurnPool {
      /// Spawn a tokio background task that probes every server on `interval`.
      /// After `unhealthy_after` consecutive failures the server is marked unhealthy.
      /// After a success it is marked healthy and consecutive_failures reset to 0.
      pub fn start_probe_task(&self, interval: Duration, unhealthy_after: u32) {
          let servers = self.servers.clone();
          tokio::spawn(async move {
              let mut tick = tokio::time::interval(interval);
              loop {
                  tick.tick().await;
                  for server in servers.iter() {
                      let addr = match parse_host_port(&server.cfg.url).await {
                          Some(a) => a,
                          None => continue,
                      };
                      match crate::turn_probe::probe(addr, Duration::from_secs(3)).await {
                          Ok(rtt) => {
                              server.consecutive_failures.store(0, Ordering::Relaxed);
                              server.last_rtt_ms.store(rtt, Ordering::Relaxed);
                              // Flip true and log only on the first recovery.
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
                              let fails = server
                                  .consecutive_failures
                                  .fetch_add(1, Ordering::Relaxed)
                                  + 1;
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
      }
  }
  ```

- [ ] **Step 3:** Wire probe task in `main.rs` after `AppState` is built (placeholder until Task 2.4 adds `turn_pool` to AppState; for now construct a local pool from config and start it):

  ```rust
  // main.rs — after config is loaded, before serve
  let turn_pool = oxpulse_chat::turn_pool::TurnPool::new(config.turn_servers.clone());
  turn_pool.start_probe_task(
      std::time::Duration::from_secs(config.turn_probe_interval_secs),
      config.turn_unhealthy_after_fails,
  );
  ```

- [ ] **Step 4:** Remove `#[allow(dead_code)]` from `config.rs` fields `turn_servers`, `turn_probe_interval_secs`, `turn_unhealthy_after_fails` — they are now consumed.

- [ ] **Step 5:** Unit test with a mock alternating-responder. Add to `crates/server/src/turn_pool.rs` `#[cfg(test)]` block:

  ```rust
  #[tokio::test]
  async fn probe_task_flips_healthy_on_failure_threshold() {
      use std::sync::atomic::Ordering;
      // Build pool with one server pointing at a port no one is listening on.
      // Use an ephemeral port that we bind and immediately drop so it's closed.
      let sock = tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap();
      let dead_addr = sock.local_addr().unwrap();
      drop(sock); // now the port is closed / silent
      let url = format!("turn:{}:{}", dead_addr.ip(), dead_addr.port());
      let pool = TurnPool::new(vec![crate::config::TurnServerCfg {
          url: url.clone(),
          region: "test".into(),
          priority: 0,
      }]);
      // Start probe with short interval and threshold=2.
      pool.start_probe_task(
          std::time::Duration::from_millis(50),
          2,
      );
      // Wait long enough for 3 probe rounds.
      tokio::time::sleep(std::time::Duration::from_millis(250)).await;
      // Server should now be marked unhealthy.
      assert!(
          pool.healthy().is_empty(),
          "server should be unhealthy after consecutive probe failures"
      );
  }
  ```

- [ ] **Step 6:** Commit
  ```
  git commit -m "feat(server): TurnPool probe task with unhealthy threshold"
  ```

---

### Task 2.4: `/api/turn-credentials` serves only healthy pool

**Model:** Sonnet
**Files:** `crates/server/src/router.rs` (AppState + handler), `crates/server/src/main.rs`
**Depends on:** Task 2.3

- [ ] **Step 1:** Extend `AppState` in `router.rs`:

  ```rust
  #[derive(Clone)]
  pub struct AppState {
      pub rooms: oxpulse_signaling::Rooms,
      pub turn_secret: String,
      pub turn_urls: Vec<String>,   // kept for legacy / fallback
      pub stun_urls: Vec<String>,
      pub pool: Option<sqlx::PgPool>,
      pub turn_pool: oxpulse_chat::turn_pool::TurnPool,  // NEW
  }
  ```

- [ ] **Step 2:** Replace the `turn_credentials` handler body:

  ```rust
  async fn turn_credentials(
      headers: axum::http::HeaderMap,
      State(state): State<AppState>,
  ) -> impl IntoResponse {
      if state.turn_secret.is_empty() {
          return (
              StatusCode::SERVICE_UNAVAILABLE,
              Json(serde_json::json!({"error": "TURN not configured"})),
          )
              .into_response();
      }
      let client_region = geo_hint(&headers);
      let mut healthy = state.turn_pool.healthy();
      healthy.sort_by_key(|s| {
          let region_match = if Some(s.cfg.region.as_str()) == client_region.as_deref() {
              0i32
          } else {
              1
          };
          (region_match, s.cfg.priority, s.last_rtt_ms() as i32)
      });
      let urls: Vec<String> = healthy.iter().take(3).map(|s| s.cfg.url.clone()).collect();
      if urls.is_empty() {
          return (
              StatusCode::SERVICE_UNAVAILABLE,
              Json(serde_json::json!({"error": "no healthy TURN servers"})),
          )
              .into_response();
      }
      let creds = oxpulse_turn::generate_credentials(
          &state.turn_secret,
          "chat-user",
          Duration::from_secs(86_400),
          &urls,
          &state.stun_urls,
      );
      (StatusCode::OK, Json(serde_json::to_value(creds).unwrap())).into_response()
  }
  ```

- [ ] **Step 3:** Update `main.rs` to wire `turn_pool` into `AppState`:

  ```rust
  let turn_pool = oxpulse_chat::turn_pool::TurnPool::new(config.turn_servers.clone());
  turn_pool.start_probe_task(
      std::time::Duration::from_secs(config.turn_probe_interval_secs),
      config.turn_unhealthy_after_fails,
  );
  let state = AppState {
      rooms: Rooms::new(),
      turn_secret: config.turn_secret.clone(),
      turn_urls: config.turn_urls.clone(),
      stun_urls: config.stun_urls.clone(),
      pool: db_pool,
      turn_pool,
  };
  ```

- [ ] **Step 4:** Integration test in `crates/server/tests/http_integration.rs` — build an `AppState` with a pool that has one healthy and one unhealthy server, POST `/api/turn-credentials`, assert only the healthy URL appears in the response and status is 200. Also assert a pool with all servers unhealthy returns 503.

  ```rust
  #[tokio::test]
  async fn turn_credentials_excludes_unhealthy_servers() {
      use std::sync::atomic::Ordering;
      let dir = tempdir().unwrap();
      fs::write(dir.path().join("index.html"), "SPA").unwrap();

      let cfgs = vec![
          oxpulse_chat::config::TurnServerCfg { url: "turn:good:3478".into(), region: "ru".into(), priority: 0 },
          oxpulse_chat::config::TurnServerCfg { url: "turn:bad:3478".into(), region: "ru".into(), priority: 1 },
      ];
      let pool = oxpulse_chat::turn_pool::TurnPool::new(cfgs);
      // Mark second server unhealthy.
      pool.all()[1].healthy.store(false, Ordering::Relaxed);

      let state = AppState {
          rooms: oxpulse_signaling::Rooms::new(),
          turn_secret: "test-secret".into(),
          turn_urls: vec![],
          stun_urls: vec![],
          pool: None,
          turn_pool: pool,
      };
      let app = build_router(state, dir.path().to_str().unwrap());
      let server = TestServer::new(app).unwrap();
      let resp = server.post("/api/turn-credentials").await;
      resp.assert_status_ok();
      let body: serde_json::Value = resp.json();
      let ice_servers = body["iceServers"].as_array().unwrap();
      let urls: Vec<&str> = ice_servers.iter()
          .flat_map(|s| s["urls"].as_array().unwrap_or(&vec![]))
          .filter_map(|u| u.as_str())
          .collect();
      assert!(urls.iter().any(|u| u.contains("good")));
      assert!(!urls.iter().any(|u| u.contains("bad")));
  }
  ```

- [ ] **Step 5:** Commit
  ```
  git commit -m "feat(server): serve only healthy TURN servers in /api/turn-credentials"
  ```

---

### Task 2.5: Geo-hint from client headers

**Model:** Haiku
**Files:** `crates/server/src/router.rs` (new helper function)
**Depends on:** Task 2.4

- [ ] **Step 1:** Add `geo_hint` helper (already referenced in Task 2.4's handler):

  ```rust
  /// Read a coarse geo hint from proxy-set headers.
  /// Prefers X-Client-Region (partner Caddy), falls back to CF-IPCountry.
  /// Returns lowercased value or None.
  fn geo_hint(headers: &axum::http::HeaderMap) -> Option<String> {
      headers
          .get("x-client-region")
          .or_else(|| headers.get("cf-ipcountry"))
          .and_then(|v| v.to_str().ok())
          .map(|s| s.to_ascii_lowercase())
  }
  ```

- [ ] **Step 2:** Add unit test:

  ```rust
  #[test]
  fn geo_hint_prefers_x_client_region() {
      let mut headers = axum::http::HeaderMap::new();
      headers.insert("x-client-region", "RU-MSK".parse().unwrap());
      headers.insert("cf-ipcountry", "DE".parse().unwrap());
      assert_eq!(geo_hint(&headers), Some("ru-msk".to_string()));
  }

  #[test]
  fn geo_hint_falls_back_to_cf_ipcountry() {
      let mut headers = axum::http::HeaderMap::new();
      headers.insert("cf-ipcountry", "DE".parse().unwrap());
      assert_eq!(geo_hint(&headers), Some("de".to_string()));
  }

  #[test]
  fn geo_hint_returns_none_when_absent() {
      let headers = axum::http::HeaderMap::new();
      assert_eq!(geo_hint(&headers), None);
  }
  ```

- [ ] **Step 3:** Commit
  ```
  git commit -m "feat(server): geo-aware TURN selection via X-Client-Region / CF-IPCountry"
  ```

---

### Task 2.6: SIGHUP hot-reload of TURN server list

**Model:** Opus
**Files:** `crates/server/src/turn_pool.rs` (add `reload`, change inner to `ArcSwap`), `crates/server/src/main.rs` (signal handler), `crates/server/Cargo.toml` (add `arc-swap`)
**Depends on:** Task 2.3

- [ ] **Step 1:** Add `arc-swap = "1.7"` to `[dependencies]` in `crates/server/Cargo.toml`.

- [ ] **Step 2:** Change `TurnPool` inner type from `Arc<Vec<Arc<TurnServer>>>` to `arc_swap::ArcSwap<Vec<Arc<TurnServer>>>`:

  ```rust
  use arc_swap::ArcSwap;

  #[derive(Clone)]
  pub struct TurnPool {
      servers: Arc<ArcSwap<Vec<Arc<TurnServer>>>>,
  }

  impl TurnPool {
      pub fn new(cfgs: Vec<TurnServerCfg>) -> Self {
          let servers = cfgs
              .into_iter()
              .map(|cfg| Arc::new(TurnServer {
                  cfg,
                  healthy: AtomicBool::new(true),
                  consecutive_failures: AtomicU32::new(0),
                  last_rtt_ms: AtomicU32::new(0),
              }))
              .collect::<Vec<_>>();
          Self {
              servers: Arc::new(ArcSwap::from_pointee(servers)),
          }
      }

      pub fn healthy(&self) -> Vec<Arc<TurnServer>> {
          self.servers.load().iter().filter(|s| s.is_healthy()).cloned().collect()
      }

      pub fn all(&self) -> Vec<Arc<TurnServer>> {
          self.servers.load().iter().cloned().collect()
      }

      pub fn len(&self) -> usize { self.servers.load().len() }
      pub fn is_empty(&self) -> bool { self.servers.load().is_empty() }

      /// Hot-reload: replace the server list.
      /// Servers present in both old and new lists (matched by URL) retain their
      /// atomic state (healthy, consecutive_failures, last_rtt_ms).
      /// New servers start optimistically healthy.
      /// Old servers not in the new list are dropped from the pool.
      pub fn reload(&self, new_cfgs: Vec<TurnServerCfg>) {
          let old = self.servers.load();
          let new_servers: Vec<Arc<TurnServer>> = new_cfgs
              .into_iter()
              .map(|cfg| {
                  // Preserve state for existing servers by URL match.
                  if let Some(existing) = old.iter().find(|s| s.cfg.url == cfg.url) {
                      Arc::clone(existing)
                  } else {
                      Arc::new(TurnServer {
                          cfg,
                          healthy: AtomicBool::new(true),
                          consecutive_failures: AtomicU32::new(0),
                          last_rtt_ms: AtomicU32::new(0),
                      })
                  }
              })
              .collect();
          self.servers.store(Arc::new(new_servers));
      }
  }
  ```

- [ ] **Step 3:** Add signal handler in `main.rs` (UNIX only):

  ```rust
  #[cfg(unix)]
  {
      use tokio::signal::unix::{signal, SignalKind};
      let pool_reload = state.turn_pool.clone();
      let servers_env = std::env::var("TURN_SERVERS").unwrap_or_default();
      tokio::spawn(async move {
          let mut sighup = signal(SignalKind::hangup())
              .expect("failed to register SIGHUP handler");
          while sighup.recv().await.is_some() {
              // Re-read TURN_SERVERS from env (or TURN_SERVERS_FILE if set).
              let src = std::env::var("TURN_SERVERS_FILE")
                  .ok()
                  .and_then(|path| std::fs::read_to_string(path).ok())
                  .unwrap_or_else(|| servers_env.clone());
              let new_cfgs = oxpulse_chat::config::parse_turn_servers(&src);
              pool_reload.reload(new_cfgs);
              tracing::info!("turn_pool reloaded via SIGHUP");
          }
      });
  }
  ```

- [ ] **Step 4:** Unit test (add to `turn_pool.rs` tests):

  ```rust
  #[test]
  fn reload_adds_new_server_and_removes_old() {
      let pool = TurnPool::new(vec![
          TurnServerCfg { url: "turn:a:3478".into(), region: "ru".into(), priority: 0 },
          TurnServerCfg { url: "turn:b:3478".into(), region: "de".into(), priority: 1 },
      ]);
      // Mark server-b unhealthy before reload.
      pool.all()[1].healthy.store(false, Ordering::Relaxed);

      pool.reload(vec![
          TurnServerCfg { url: "turn:a:3478".into(), region: "ru".into(), priority: 0 }, // kept
          TurnServerCfg { url: "turn:c:3478".into(), region: "us".into(), priority: 2 }, // new
          // "turn:b:3478" is gone
      ]);

      let all = pool.all();
      assert_eq!(all.len(), 2);
      let urls: Vec<&str> = all.iter().map(|s| s.url()).collect();
      assert!(urls.contains(&"turn:a:3478"), "server a must be retained");
      assert!(urls.contains(&"turn:c:3478"), "server c must be added");
      assert!(!urls.contains(&"turn:b:3478"), "server b must be removed");
      // Server a retains its health state (was healthy).
      let a = all.iter().find(|s| s.url() == "turn:a:3478").unwrap();
      assert!(a.is_healthy(), "server a should still be healthy");
      // Server c starts optimistically healthy.
      let c = all.iter().find(|s| s.url() == "turn:c:3478").unwrap();
      assert!(c.is_healthy(), "new server c should start healthy");
  }
  ```

- [ ] **Step 5:** Commit
  ```
  git commit -m "feat(server): hot-reload TURN pool on SIGHUP via ArcSwap"
  ```

---

## Phase 3 — Observability

**Estimated:** ~1 hour. Tasks 3.1 and 3.2 are sequential (3.2 consumes 3.1's `Metrics`). Task 3.3 is a config task that can run in parallel with 3.2 if two subagents are dispatched. Task 3.5 is a docs task, fully independent.

### Task 3.1: `/metrics` endpoint

**Model:** Sonnet
**Files:** `crates/server/src/metrics.rs` (create), `crates/server/Cargo.toml`, `crates/server/src/router.rs`, `crates/server/src/main.rs`
**Depends on:** none (can start in parallel with Phase 2 but needs to land before Task 3.2)

- [ ] **Step 1:** Add dependency to `crates/server/Cargo.toml`:
  ```toml
  prometheus = { version = "0.13", default-features = false }
  ```

- [ ] **Step 2:** Create `crates/server/src/metrics.rs`:

  ```rust
  use prometheus::{
      Histogram, HistogramOpts, IntCounter, IntCounterVec, IntGauge, Opts, Registry,
  };

  pub struct Metrics {
      pub registry: Registry,
      pub rooms_active: IntGauge,
      pub ws_connects_total: IntCounter,
      pub ws_disconnects_total: IntCounter,
      pub ws_join_total: IntCounterVec,          // label: result=ok|err
      pub ws_handshake_failed_total: IntCounter,
      pub call_duration_seconds: Histogram,
      pub turn_servers_healthy: IntGauge,
      pub turn_creds_issued_total: IntCounter,
      pub turn_cred_latency_seconds: Histogram,  // required by SLO turn_cred_latency
      pub analytics_events_total: IntCounterVec, // label: result=ok|err
  }

  impl Metrics {
      pub fn new() -> Self {
          let registry = Registry::new();

          macro_rules! reg {
              ($m:expr) => {{
                  let m = $m;
                  registry.register(Box::new(m.clone())).expect("metric registration");
                  m
              }};
          }

          let rooms_active = reg!(IntGauge::with_opts(
              Opts::new("rooms_active", "Currently active signaling rooms")
          ).unwrap());
          let ws_connects_total = reg!(IntCounter::with_opts(
              Opts::new("ws_connects_total", "Total WebSocket connections accepted")
          ).unwrap());
          let ws_disconnects_total = reg!(IntCounter::with_opts(
              Opts::new("ws_disconnects_total", "Total WebSocket connections closed")
          ).unwrap());
          let ws_join_total = reg!(IntCounterVec::new(
              Opts::new("ws_join_total", "Room join attempts by result"),
              &["result"],
          ).unwrap());
          let ws_handshake_failed_total = reg!(IntCounter::with_opts(
              Opts::new("ws_handshake_failed_total", "WS transport/handshake failures before join")
          ).unwrap());
          let call_duration_seconds = reg!(Histogram::with_opts(
              HistogramOpts::new("call_duration_seconds", "Duration of completed calls")
                  .buckets(vec![5.0, 15.0, 30.0, 60.0, 120.0, 300.0, 600.0, 1800.0])
          ).unwrap());
          let turn_servers_healthy = reg!(IntGauge::with_opts(
              Opts::new("turn_servers_healthy", "Number of TURN servers currently healthy")
          ).unwrap());
          let turn_creds_issued_total = reg!(IntCounter::with_opts(
              Opts::new("turn_creds_issued_total", "Total TURN credential responses issued")
          ).unwrap());
          let turn_cred_latency_seconds = reg!(Histogram::with_opts(
              HistogramOpts::new(
                  "turn_cred_latency_seconds",
                  "Latency of /api/turn-credentials handler (p99 SLO target: 150ms)",
              )
              .buckets(vec![0.005, 0.01, 0.025, 0.05, 0.1, 0.15, 0.25, 0.5, 1.0, 2.0])
          ).unwrap());
          let analytics_events_total = reg!(IntCounterVec::new(
              Opts::new("analytics_events_total", "Analytics insert results by outcome"),
              &["result"],
          ).unwrap());

          Self {
              registry,
              rooms_active,
              ws_connects_total,
              ws_disconnects_total,
              ws_join_total,
              ws_handshake_failed_total,
              call_duration_seconds,
              turn_servers_healthy,
              turn_creds_issued_total,
              turn_cred_latency_seconds,
              analytics_events_total,
          }
      }
  }

  impl Default for Metrics {
      fn default() -> Self { Self::new() }
  }
  ```

- [ ] **Step 3:** Add `pub metrics: std::sync::Arc<Metrics>` to `AppState` in `router.rs`. Construct as `Arc::new(Metrics::new())` in `main.rs`.

- [ ] **Step 4:** Add `/metrics` handler with constant-time token check:

  ```rust
  async fn metrics_handler(
      axum::extract::TypedHeader(auth): axum::extract::TypedHeader<
          axum::headers::Authorization<axum::headers::authorization::Bearer>
      >,
      State(state): State<AppState>,
  ) -> impl IntoResponse {
      let expected = std::env::var("METRICS_TOKEN").unwrap_or_default();
      if expected.is_empty() || !constant_time_eq(auth.token().as_bytes(), expected.as_bytes()) {
          return (StatusCode::UNAUTHORIZED, "").into_response();
      }
      use prometheus::Encoder;
      let enc = prometheus::TextEncoder::new();
      let mut buf = Vec::new();
      enc.encode(&state.metrics.registry.gather(), &mut buf).ok();
      (
          StatusCode::OK,
          [(axum::http::header::CONTENT_TYPE, "text/plain; version=0.0.4")],
          String::from_utf8(buf).unwrap_or_default(),
      )
          .into_response()
  }

  fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
      if a.len() != b.len() { return false; }
      a.iter().zip(b.iter()).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
  }
  ```

  Mount in `build_router`: `.route("/metrics", get(metrics_handler))`.

  **Note:** If `axum-extra` or `axum::headers` is not already a dependency, implement the token check via a plain `HeaderMap` extract:
  ```rust
  async fn metrics_handler(
      headers: HeaderMap,
      State(state): State<AppState>,
  ) -> impl IntoResponse {
      let expected = std::env::var("METRICS_TOKEN").unwrap_or_default();
      let provided = headers
          .get("x-internal-token")
          .and_then(|v| v.to_str().ok())
          .unwrap_or("");
      if expected.is_empty() || !constant_time_eq(provided.as_bytes(), expected.as_bytes()) {
          return (StatusCode::UNAUTHORIZED, "").into_response();
      }
      // ... encode and return ...
  }
  ```
  Add `X-Internal-Token: $METRICS_TOKEN` to Dozor scrape config.

- [ ] **Step 5:** Expose `pub mod metrics;` from `lib.rs`.

- [ ] **Step 6:** Commit
  ```
  git commit -m "feat(server): /metrics endpoint with Prometheus text format + token auth"
  ```

---

### Task 3.2: Wire metrics into hot paths

**Model:** Sonnet
**Files:** `crates/signaling/src/handler.rs`, `crates/signaling/src/rooms.rs`, `crates/server/src/analytics.rs`, `crates/server/src/router.rs`, `crates/server/src/turn_pool.rs`
**Depends on:** Task 3.1 (needs `Metrics` struct), Task 2.4 (needs `turn_pool` in AppState)

The three new metrics flagged in `docs/slo/oxpulse-chat.yaml` as `UNMEASURABLE_UNTIL_TASK_3_2` MUST be added in this task:
- `ws_join_total{result="ok"|"err"}` (SLI: `call_join_success`)
- `turn_cred_latency_seconds` histogram (SLI: `turn_cred_latency`)
- `ws_handshake_failed_total` counter (SLI: `signaling_ws_availability`)

- [ ] **Step 1:** Pass `Arc<Metrics>` to the signaling crate. The cleanest approach is to add an optional `Arc<Metrics>` to `Rooms` or pass it explicitly to `handle_call_ws`. Check the existing signature in `crates/signaling/src/handler.rs` and choose the least-invasive injection point.

- [ ] **Step 2:** `rooms_active` — `metrics.rooms_active.inc()` on successful `rooms.join()`; `metrics.rooms_active.dec()` when `cleanup_expired` removes a room entry.

- [ ] **Step 3:** `ws_connects_total` / `ws_disconnects_total` — increment at top and bottom of `handle_call_ws`.

- [ ] **Step 4:** `ws_join_total` — in `handle_call_ws`, increment `ws_join_total{result="ok"}` right after the `ServerMsg::Joined` send succeeds; increment `ws_join_total{result="err"}` on every other terminal path (join timeout, room full, sink.send failure before Joined).

- [ ] **Step 5:** `ws_handshake_failed_total` — increment in `wait_for_join()` failure paths AND on "Room is full" / sink.send pre-Joined errors. (Distinct from `ws_join_total{result="err"}` — this counter lives against `ws_connects_total` denominator for the `signaling_ws_availability` SLI.)

- [ ] **Step 6:** `call_duration_seconds` — `observe(elapsed_secs)` inside the call-ended log block in `handler.rs`.

- [ ] **Step 7:** `turn_servers_healthy` — `metrics.turn_servers_healthy.set(pool.healthy().len() as i64)` at the end of each probe-loop iteration in `turn_pool.rs::start_probe_task`.

- [ ] **Step 8:** `turn_creds_issued_total` + `turn_cred_latency_seconds` — wrap the `turn_credentials` handler body in an `Instant::now()` / `observe()` pair; increment the counter on successful response.

  ```rust
  let start = std::time::Instant::now();
  // ... existing handler logic ...
  state.metrics.turn_creds_issued_total.inc();
  state.metrics.turn_cred_latency_seconds.observe(start.elapsed().as_secs_f64());
  ```

- [ ] **Step 9:** `analytics_events_total` — in `analytics.rs` insert loop:

  ```rust
  match sqlx::query(...).execute(&pool).await {
      Ok(_)  => { metrics.analytics_events_total.with_label_values(&["ok"]).inc(); }
      Err(e) => {
          tracing::warn!(error = %e, "analytics_insert_failed");
          metrics.analytics_events_total.with_label_values(&["err"]).inc();
      }
  }
  ```

- [ ] **Step 10:** Integration test — start the test server, make a few requests, GET `/metrics` with the internal token, assert all expected metric names appear in the response body:

  ```rust
  #[tokio::test]
  async fn metrics_endpoint_exposes_expected_metric_names() {
      let dir = tempdir().unwrap();
      fs::write(dir.path().join("index.html"), "SPA").unwrap();
      std::env::set_var("METRICS_TOKEN", "test-token-abc");
      let app = build_router(test_state_with_pool(None), dir.path().to_str().unwrap());
      let server = TestServer::new(app).unwrap();
      // Hit a couple of endpoints to seed some counters.
      server.get("/api/health").await;
      let resp = server
          .get("/metrics")
          .add_header("x-internal-token", "test-token-abc")
          .await;
      resp.assert_status_ok();
      let body = resp.text();
      for name in &[
          "rooms_active", "ws_connects_total", "turn_servers_healthy",
          "turn_creds_issued_total", "turn_cred_latency_seconds_bucket",
          "ws_join_total", "ws_handshake_failed_total",
          "analytics_events_total",
      ] {
          assert!(body.contains(name), "missing metric: {name}");
      }
  }
  ```

- [ ] **Step 11:** Commit
  ```
  git commit -m "feat(server): wire Prometheus metrics into all hot paths including 3 new SLO metrics"
  ```

---

### Task 3.3: Dozor alerts

**Model:** Haiku
**Files:** `~/.dozor/deploy-repos.yaml` or `~/.dozor/alerts/oxpulse-chat.yaml` (write outside the repo)
**Depends on:** Task 3.2 must land first (metric names must exist)

**NOTE:** This task writes files in `~/.dozor/`, which is outside the oxpulse-chat repo. Subagent should first run `ls ~/.dozor/` to discover the exact config structure, then add the alerts without committing to the oxpulse-chat repo.

- [ ] **Step 1:** Discover dozor config structure: `ls ~/.dozor/` and read the existing alert config file format.

- [ ] **Step 2:** Add alert rules covering the 4 SLOs from `docs/slo/oxpulse-chat.yaml`:

  | Alert | Condition | Window | Severity | Notes |
  |---|---|---|---|---|
  | `turn_servers_down` | `turn_servers_healthy < 1` | for 2m | page | TURN pool fully dark |
  | `turn_creds_fast_burn` | burn_rate 14.4× on `turn_cred_latency_seconds` | 1h/5m | page | p99 > 150ms |
  | `turn_creds_slow_burn` | burn_rate 6× | 6h/30m | ticket | |
  | `analytics_fast_burn` | burn_rate 14.4× on `analytics_events_total{result="err"}` ratio | 1h/5m | page | silent drop |
  | `analytics_slow_burn` | burn_rate 6× | 6h/30m | ticket | |
  | `ws_join_fast_burn` | burn_rate 14.4× on `ws_join_total{result="err"}` ratio | 1h/5m | page | |
  | `ws_join_slow_burn` | burn_rate 6× | 6h/30m | ticket | |
  | `no_traffic_warn` | `rate(ws_connects_total[5m]) == 0` | for 15m during waking hours | warn | dead traffic |
  | `analytics_no_traffic` | `rate(analytics_events_total[5m]) == 0 and rate(ws_connects_total[5m]) > 0` | for 10m | warn | analytics broken but WS live |

  For the three SLOs that are `UNMEASURABLE_UNTIL_TASK_3_2`: create `absent(metric_name)` stub alerts so the absence of the metric triggers a warning, not silence.

- [ ] **Step 3:** Route all page-severity alerts to Telegram via the existing vaelor channel. Route ticket-severity to a lower-priority channel or same channel with explicit `[TICKET]` prefix.

- [ ] **Step 4:** Test one alert manually — temporarily lower the `turn_servers_healthy < 1` threshold to `< 999`, verify the Telegram message fires, restore.

- [ ] **Step 5:** Commit dozor config in its own repo if applicable, or record the change in `docs/runbooks/dozor-alerts.md`.

---

### Task 3.5: Runbooks for TURN outage and signaling WS failure

**Model:** Haiku
**Files:** `docs/runbooks/turn-outage.md` (create), `docs/runbooks/signaling-ws-outage.md` (create)
**Depends on:** none (pure docs)

- [ ] **Step 1:** Create `docs/runbooks/turn-outage.md`:

  ```markdown
  # Runbook: TURN Server Outage

  **Trigger:** `turn_servers_down` alert fires (turn_servers_healthy < 1 for 2m)
  **SLO:** turn_cred_latency (p99 < 150ms / 28d)

  ## 1. Immediate triage (< 2 min)

  curl -s -H "X-Internal-Token: $METRICS_TOKEN" https://oxpulse.chat/metrics \
    | grep turn_servers_healthy

  If 0: all TURN nodes are down. Go to step 2.
  If > 0: alert may be stale. Check for recent reload or probe restart.

  ## 2. Identify affected nodes

  grep turn_server_down <(docker logs oxpulse-chat 2>&1 | tail -500)
  # Output: region=..., url=..., consecutive_failures=...

  ## 3. Verify node reachability

  # For each down node (replace host:port):
  nc -u -z -w 3 <host> 3478 && echo "UDP open" || echo "UDP closed"

  ## 4. Partner NOC escalation

  If node is in the partner VPN network and UDP 3478 is closed, contact the partner NOC
  via the agreed Telegram ops channel. Include: node URL, region, time of first failure.

  ## 5. Emergency: force iceTransportPolicy=all

  If >50% of regions are down and users are reporting call failures:
    - SIGHUP is insufficient (servers are still in the list, just unhealthy).
    - Edit TURN_SERVERS_FILE to remove the down nodes, then:
      docker kill -s HUP oxpulse-chat
    - The pool reloads with only the surviving nodes.
    - Server automatically returns iceTransportPolicy="all" when pool is empty (Task 4.3).

  ## 6. Recovery verification

  grep turn_server_up <(docker logs oxpulse-chat 2>&1 | tail -200)
  # Should appear within one probe interval (default 30s).
  curl -s -H "X-Internal-Token: $METRICS_TOKEN" https://oxpulse.chat/metrics \
    | grep turn_servers_healthy
  # Should be >= 1.

  ## 7. Post-incident

  - Write a postmortem if outage exceeded 10 minutes (template: docs/postmortems/template.md).
  - Verify TURN failover drill date in this runbook is < 90 days old.

  ## Verified failover behaviour

  Date last verified: (run Task 5.2 to fill in)
  Procedure: iptables -I OUTPUT -d <primary-turn-host> -j DROP on signaling host,
  confirm probe marks node down within TURN_UNHEALTHY_AFTER_FAILS×TURN_PROBE_INTERVAL_SECS
  seconds, verify ICE restart picks up secondary node in < 10s.
  ```

- [ ] **Step 2:** Create `docs/runbooks/signaling-ws-outage.md`:

  ```markdown
  # Runbook: Signaling WebSocket Outage

  **Trigger:** `ws_join_fast_burn` alert (burn_rate 14.4× on ws_join_total{result="err"})
  **SLO:** call_join_success (99.0% / 28d), signaling_ws_availability (99.95% / 28d)

  ## 1. Immediate triage

  # Check WS connect rate:
  curl -s -H "X-Internal-Token: $METRICS_TOKEN" https://oxpulse.chat/metrics \
    | grep -E "ws_connects_total|ws_join_total|ws_handshake_failed_total"

  # Check container health:
  docker ps | grep oxpulse-chat
  docker logs oxpulse-chat --tail 50

  ## 2. Differentiate failure modes

  - ws_connects_total rising, ws_join_total{result="err"} rising:
    → Join-level failure (room full, DB issue, signaling logic). Check logs for
      "room_full", "join_timeout", "sink.send error".

  - ws_handshake_failed_total rising vs ws_connects_total:
    → Transport/TLS issue. Check Caddy logs and TLS certificate expiry.

  - ws_connects_total = 0:
    → No traffic at all. Likely ingress/DNS issue. Check Caddy, check DNS.

  ## 3. Quick recovery options

  Container crash:
    docker compose up -d --no-deps --force-recreate oxpulse-chat

  Room state corruption:
    Rooms are in-memory only. Restart clears all state. Users must rejoin.
    (Acceptable: calls are ephemeral, no persistent room state.)

  ## 4. Rollback

  docker compose up -d --no-deps oxpulse-chat  # uses previous image tag in compose file
  # Or specify explicit tag:
  # IMAGE_TAG=v0.1.4 docker compose up -d --no-deps --force-recreate oxpulse-chat

  ## 5. Escalation

  If the outage exceeds 5 minutes and cannot be resolved by restart:
  - Declare SEV-1 in the ops Telegram channel.
  - Write a postmortem within 24h (docs/postmortems/template.md).
  ```

- [ ] **Step 3:** Commit
  ```
  git commit -m "docs(runbooks): TURN outage + signaling WS outage runbooks"
  ```

---

## Phase 4 — Abuse protection

**Estimated:** ~1.5 hours. Tasks 4.1 and 4.2 are independent (different crates); can be dispatched in parallel. Task 4.3 depends on Task 2.4's pool-in-AppState.

### Task 4.1: Per-IP rate limit on API endpoints

**Model:** Sonnet
**Files:** `crates/server/src/rate_limit.rs` (create), `crates/server/Cargo.toml`, `crates/server/src/router.rs`
**Depends on:** none

- [ ] **Step 1:** Add dependencies to `crates/server/Cargo.toml`:
  ```toml
  governor = "0.6"
  ```

- [ ] **Step 2:** Create `crates/server/src/rate_limit.rs`:

  ```rust
  use std::net::IpAddr;
  use std::num::NonZeroU32;
  use std::sync::Arc;
  use std::time::Duration;

  use axum::extract::ConnectInfo;
  use axum::http::{HeaderMap, Request, StatusCode};
  use axum::middleware::Next;
  use axum::response::{IntoResponse, Response};
  use governor::clock::DefaultClock;
  use governor::state::keyed::DashMapStateStore;
  use governor::{Quota, RateLimiter};

  pub type KeyedLimiter = Arc<RateLimiter<IpAddr, DashMapStateStore<IpAddr>, DefaultClock>>;

  /// Build a token bucket: `per_second` sustained, burst `burst_size`.
  pub fn build_limiter(per_second: u32, burst_size: u32) -> KeyedLimiter {
      let quota = Quota::per_second(NonZeroU32::new(per_second).unwrap())
          .allow_burst(NonZeroU32::new(burst_size).unwrap());
      Arc::new(RateLimiter::keyed(quota))
  }

  /// Extract the client IP from X-Forwarded-For last hop, or fall back to
  /// the direct connection address.
  pub fn client_ip(headers: &HeaderMap, connect_info: Option<&std::net::SocketAddr>) -> IpAddr {
      headers
          .get("x-forwarded-for")
          .and_then(|v| v.to_str().ok())
          .and_then(|s| s.split(',').last())
          .and_then(|s| s.trim().parse::<IpAddr>().ok())
          .or_else(|| connect_info.map(|a| a.ip()))
          .unwrap_or(IpAddr::V4(std::net::Ipv4Addr::UNSPECIFIED))
  }

  /// Axum middleware: check the keyed limiter for this request's IP.
  /// Returns 429 with Retry-After when the bucket is exhausted.
  pub async fn rate_limit_middleware<B>(
      limiter: axum::extract::Extension<KeyedLimiter>,
      headers: HeaderMap,
      connect_info: Option<ConnectInfo<std::net::SocketAddr>>,
      req: Request<B>,
      next: Next<B>,
  ) -> Response {
      let ip = client_ip(&headers, connect_info.as_ref().map(|c| &c.0));
      match limiter.check_key(&ip) {
          Ok(_) => next.run(req).await,
          Err(negative) => {
              let wait = negative.wait_time_from(governor::clock::DefaultClock::default().now());
              let secs = wait.as_secs().max(1).to_string();
              (
                  StatusCode::TOO_MANY_REQUESTS,
                  [(axum::http::header::RETRY_AFTER, secs)],
                  "rate limit exceeded",
              )
                  .into_response()
          }
      }
  }
  ```

- [ ] **Step 3:** In `router.rs`, build two limiters and attach them as layers:

  ```rust
  use crate::rate_limit::{build_limiter, rate_limit_middleware};
  use axum::middleware;

  // In build_router:
  let turn_cred_limiter = build_limiter(10, 20); // 10 rps, burst 20
  let event_limiter     = build_limiter(5, 10);  // 5 rps, burst 10

  Router::new()
      // ...
      .route(
          "/api/turn-credentials",
          post(turn_credentials).layer(
              middleware::from_fn(rate_limit_middleware)
                  .layer(axum::extract::Extension(turn_cred_limiter))
          ),
      )
      .route(
          "/api/event",
          post(crate::analytics::ingest).layer(
              middleware::from_fn(rate_limit_middleware)
                  .layer(axum::extract::Extension(event_limiter))
          ),
      )
  ```

- [ ] **Step 4:** Integration test:

  ```rust
  #[tokio::test]
  async fn turn_credentials_rate_limits_single_ip() {
      // Build a server with a very tight limiter (1 rps, burst 1) for testing.
      // ... set up test server ...
      // Send 3 rapid requests, assert at least one 429.
      let mut got_429 = false;
      for _ in 0..5 {
          let resp = server.post("/api/turn-credentials").await;
          if resp.status_code() == 429 {
              got_429 = true;
              assert!(resp.header("retry-after").to_str().unwrap().parse::<u64>().unwrap() >= 1);
          }
      }
      assert!(got_429, "rate limiter should have kicked in");
  }
  ```

- [ ] **Step 5:** Commit
  ```
  git commit -m "feat(server): per-IP token-bucket rate limit on /api/turn-credentials and /api/event"
  ```

---

### Task 4.2: Room-join rate limit and ID validation

**Model:** Sonnet
**Files:** `crates/signaling/src/handler.rs`, `crates/signaling/src/rooms.rs`
**Depends on:** none (independent of Task 4.1)

- [ ] **Step 1:** In `crates/signaling/src/handler.rs`, validate room ID at the entry of `handle_call_ws` before `wait_for_join`:

  ```rust
  static ROOM_ID_RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();

  fn valid_room_id(id: &str) -> bool {
      ROOM_ID_RE
          .get_or_init(|| regex::Regex::new(r"^[A-Za-z0-9_-]{4,64}$").unwrap())
          .is_match(id)
  }

  // At top of handle_call_ws, after extracting room_id from the path:
  if !valid_room_id(&room_id) {
      let _ = sink.send(axum::extract::ws::Message::Close(Some(
          axum::extract::ws::CloseFrame {
              code: 1008, // Policy Violation
              reason: "invalid room id".into(),
          }
      ))).await;
      return;
  }
  ```

  Add `regex = "1"` to `crates/signaling/Cargo.toml` if not already present.

- [ ] **Step 2:** Per-IP room-creation rate limit in `crates/signaling/src/rooms.rs`. Add a `DashMap<IpAddr, (std::time::Instant, std::collections::HashSet<String>)>` tracking distinct rooms joined per IP in a 60-second window:

  ```rust
  use dashmap::DashMap;
  use std::collections::HashSet;
  use std::net::IpAddr;
  use std::time::Instant;

  // Add to Rooms struct:
  ip_room_tracker: DashMap<IpAddr, (Instant, HashSet<String>)>,

  // New method:
  pub fn check_ip_room_rate(&self, ip: IpAddr, room_id: &str) -> bool {
      const WINDOW_SECS: u64 = 60;
      const MAX_ROOMS: usize = 10;
      let mut entry = self.ip_room_tracker.entry(ip).or_insert_with(|| (Instant::now(), HashSet::new()));
      if entry.0.elapsed().as_secs() >= WINDOW_SECS {
          *entry = (Instant::now(), HashSet::new());
      }
      entry.1.insert(room_id.to_string());
      entry.1.len() <= MAX_ROOMS
  }
  ```

  In `handle_call_ws`, call `rooms.check_ip_room_rate(peer_ip, &room_id)` and close with 1008 if it returns false.

- [ ] **Step 3:** Unit tests:

  ```rust
  #[test]
  fn valid_room_id_rejects_too_short() {
      assert!(!valid_room_id("abc")); // 3 chars < 4
  }
  #[test]
  fn valid_room_id_rejects_special_chars() {
      assert!(!valid_room_id("room/hack"));
  }
  #[test]
  fn valid_room_id_accepts_standard_format() {
      assert!(valid_room_id("TQFA-9412"));
      assert!(valid_room_id("room_1234"));
  }
  ```

- [ ] **Step 4:** Commit
  ```
  git commit -m "feat(signaling): room-id validation regex + per-IP room-join rate limit"
  ```

---

### Task 4.3: Server-decided `iceTransportPolicy`

**Model:** Sonnet
**Files:** `crates/server/src/router.rs` (handler response), `web/src/lib/webrtc.ts`, `web/src/lib/signaling.ts`
**Depends on:** Task 2.4 (turn_pool in AppState)

- [ ] **Step 1:** Extend the `turn_credentials` response in `router.rs` to include `ice_transport_policy`:

  ```rust
  // At the end of turn_credentials handler, before returning:
  let ice_transport_policy = if healthy.is_empty() { "all" } else { "relay" };
  let mut resp_json = serde_json::to_value(&creds).unwrap();
  resp_json["ice_transport_policy"] = serde_json::Value::String(ice_transport_policy.to_string());
  (StatusCode::OK, Json(resp_json)).into_response()
  ```

- [ ] **Step 2:** In `web/src/lib/webrtc.ts`, change `iceTransportPolicy` from a hardcoded `"relay"` to reading the server-provided value:

  ```ts
  // Before: const pc = new RTCPeerConnection({ iceServers, iceTransportPolicy: 'relay' });
  // After:
  const pc = new RTCPeerConnection({
      iceServers,
      iceTransportPolicy: (serverResponse.ice_transport_policy as RTCIceTransportPolicy) ?? 'relay',
  });
  ```

- [ ] **Step 3:** In `web/src/lib/signaling.ts` (or wherever credentials are fetched), pass `ice_transport_policy` through to the `RTCPeerConnection` constructor.

- [ ] **Step 4:** Integration test in `http_integration.rs` — when pool is empty, response includes `"ice_transport_policy": "all"`; when pool has healthy servers, response includes `"ice_transport_policy": "relay"`.

- [ ] **Step 5:** Commit
  ```
  git commit -m "feat: server-decided iceTransportPolicy — relay when TURN healthy, all on outage"
  ```

---

## Phase 5 — Load and Chaos

**Estimated:** ~30-45 min (Task 5.1 is mechanical; Task 5.2 is a manual procedure doc).

### Task 5.1: WebSocket load test

**Model:** Sonnet
**Files:** `crates/server/tests/load.rs` (create), `docs/runbooks/load-baseline.md` (create)
**Depends on:** Phase 2 complete

- [ ] **Step 1:** Add `tokio-tungstenite` to `[dev-dependencies]` in `crates/server/Cargo.toml` (check if already present):
  ```toml
  tokio-tungstenite = { version = "0.21", features = ["native-tls"] }
  ```

- [ ] **Step 2:** Create `crates/server/tests/load.rs`:

  ```rust
  //! WebSocket load test — run manually with:
  //!   cargo test -p oxpulse-chat --test load -- --ignored --nocapture
  //!
  //! Requires LOAD_TEST_BASE_URL env (default: http://localhost:3000).

  use std::time::{Duration, Instant};
  use tokio_tungstenite::connect_async;
  use futures_util::{SinkExt, StreamExt};

  const CONCURRENCY: usize = 500;

  #[tokio::test(flavor = "multi_thread", worker_threads = 8)]
  #[ignore]
  async fn ws_500_concurrent_clients_join_unique_rooms() {
      let base = std::env::var("LOAD_TEST_BASE_URL")
          .unwrap_or_else(|_| "ws://localhost:3000".to_string());

      let mut handles = Vec::with_capacity(CONCURRENCY);
      let start = Instant::now();

      for i in 0..CONCURRENCY {
          let url = format!("{base}/ws/call/load-test-room-{i:05}");
          handles.push(tokio::spawn(async move {
              let t0 = Instant::now();
              let (ws, _) = connect_async(&url).await?;
              let (mut tx, mut rx) = ws.split();
              // Send a Join message.
              let join_msg = serde_json::json!({"type": "join"});
              tx.send(tokio_tungstenite::tungstenite::Message::Text(
                  join_msg.to_string(),
              ))
              .await?;
              // Wait for Joined response (or any message) — just proof of liveness.
              tokio::time::timeout(Duration::from_secs(5), rx.next()).await
                  .map_err(|_| "timeout waiting for join response")?;
              Ok::<Duration, Box<dyn std::error::Error + Send + Sync>>(t0.elapsed())
          }));
      }

      let results: Vec<_> = futures_util::future::join_all(handles).await;
      let latencies: Vec<Duration> = results
          .into_iter()
          .filter_map(|r| r.ok().and_then(|r| r.ok()))
          .collect();

      let total = start.elapsed();
      let success = latencies.len();
      let mut sorted = latencies.clone();
      sorted.sort();
      let p50 = sorted[sorted.len() / 2];
      let p99 = sorted[(sorted.len() as f64 * 0.99) as usize];

      println!("\n=== Load Test Results ===");
      println!("Total time:    {total:?}");
      println!("Successful:    {success}/{CONCURRENCY}");
      println!("p50 latency:   {p50:?}");
      println!("p99 latency:   {p99:?}");

      assert!(
          success as f64 / CONCURRENCY as f64 >= 0.99,
          "less than 99% success rate: {success}/{CONCURRENCY}"
      );
      assert!(p99 < Duration::from_secs(2), "p99 latency exceeded 2s: {p99:?}");
  }
  ```

- [ ] **Step 3:** Create `docs/runbooks/load-baseline.md` documenting the baseline numbers once the test is run (fill in after first execution):

  ```markdown
  # Load Test Baseline — oxpulse-chat

  Methodology: 500 concurrent WS clients, each joining a unique room, sending one Join
  message and waiting for Joined response. Measured on production hardware.

  | Metric | Baseline (fill in after Task 5.1) |
  |---|---|
  | Success rate | ? |
  | p50 connect→joined | ? |
  | p99 connect→joined | ? |
  | Total wall-clock for 500 clients | ? |
  | Server CPU peak | ? |
  | Server memory peak | ? |

  Run command:
  `LOAD_TEST_BASE_URL=ws://127.0.0.1:3000 cargo test -p oxpulse-chat --test load -- --ignored --nocapture`

  Next re-baseline trigger: any change to signaling handler or Rooms structure.
  ```

- [ ] **Step 4:** Commit
  ```
  git commit -m "test(server): WS load test 500 concurrent clients + load baseline doc"
  ```

---

### Task 5.2: TURN failover drill

**Model:** Haiku
**Files:** `docs/runbooks/turn-outage.md` (update the "Verified failover behaviour" section)
**Depends on:** Task 2.3 (probe loop must be deployed)

- [ ] **Step 1:** Execute the drill in a staging environment with ≥ 2 TURN nodes:

  ```bash
  # 1. Start a call with both TURN nodes visible in /metrics.
  # 2. Block UDP traffic to the primary TURN node:
  sudo iptables -I OUTPUT -d <primary-turn-host> -p udp --dport 3478 -j DROP
  # 3. Observe logs — within TURN_UNHEALTHY_AFTER_FAILS * TURN_PROBE_INTERVAL_SECS seconds
  #    you should see: turn_server_down {region=..., url=..., consecutive_failures=3}
  # 4. Observe the in-progress call — ICE restart should pick up the secondary within 10s.
  # 5. Confirm /metrics shows turn_servers_healthy reduced by 1.
  # 6. Remove the iptables rule:
  sudo iptables -D OUTPUT -d <primary-turn-host> -p udp --dport 3478 -j DROP
  # 7. Confirm turn_server_up log line appears within the next probe interval.
  ```

- [ ] **Step 2:** Update the "Verified failover behaviour" section of `docs/runbooks/turn-outage.md` with the date and measured failover time.

- [ ] **Step 3:** Commit
  ```
  git commit -m "docs(runbooks): TURN failover drill procedure + verified date"
  ```

---

## Phase 6 — Launch checklist

**Estimated:** ~30 min verification pass.

### Task 6.1: Launch readiness review

**Model:** Sonnet
**Files:** `docs/runbooks/launch-readiness.md` (create), git tag
**Depends on:** All phases complete

- [ ] **Step 1:** Walk the launch checklist from `docs/slo/error-budget-policy.md` and verify each item:

  | Item | Status |
  |---|---|
  | All Phase 1-4 tasks merged and deployed | |
  | ≥ 2 partner TURN nodes in different regions, healthy in /metrics for ≥ 24h | |
  | Dozor alerts verified firing on test (Task 3.3 Step 4) | |
  | Load test passed (Task 5.1 — >99% success, p99 < 2s) | |
  | TURN failover drill completed (Task 5.2) | |
  | `docs/partners/onboarding.md` reviewed by partner ops lead | |
  | Privacy policy updated re: partner TURN relay network | |
  | Rollback tested: previous image tag serves calls correctly | |
  | DB-down chaos test passed (Phase 5.3, already done `a85d4ee`) | |
  | All SLO metrics measurable (Task 3.2 shipped `ws_join_total`, `turn_cred_latency_seconds`, `ws_handshake_failed_total`) | |

- [ ] **Step 2:** For any item not yet green, create a ticket or sub-task before proceeding to tag.

- [ ] **Step 3:** Tag and announce:

  ```bash
  git tag v0.2.0-partner-launch
  git push origin v0.2.0-partner-launch
  ```

- [ ] **Step 4:** Commit launch readiness doc:
  ```
  git commit -m "docs: launch readiness checklist for v0.2.0-partner-launch"
  ```

---

## Out of scope (covered by separate plans)

- User accounts, contacts, push notifications → `docs/ROADMAP.md` Phase 2
- Encrypted chat / file sharing → Phase 3
- Mobile wrappers → Phase 4
- Group calls (SFU) → backlog
- Admin dashboard rewrite → separate plan
- Real GeoIP (MaxMind GeoLite2) → out of scope this phase
- TURNS (TURN over TLS/DTLS) → future work (see `docs/design/turn-pool.md` §7)
