# OxPulse Chat — Production Hardening & TURN Partner Launch

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Turn the current MVP into a production service ready to onboard a VPN-network partner who will run multi-region TURN relays, with observability, abuse protection, and a repeatable TURN onboarding process.

**Architecture:** Keep the existing single-binary Rust/Axum signaling server and SvelteKit SPA. Add a `TurnPool` abstraction that tracks multiple partner TURN servers with active health probing and optional geo-routing. Add a Prometheus `/metrics` endpoint and wire it into the existing Dozor monitoring / Grafana stack. Add rate-limiting, an analytics-failure alert, and a runbook for TURN outages.

**Tech Stack:** Rust 1.88 + Axum 0.8 + tower-http 0.6 + SQLx + DashMap; SvelteKit 5 + TypeScript; PostgreSQL 17; Dozor (existing `:8765`); Prometheus text format; partner-operated coturn on VPN nodes.

**Scope boundary:** This plan covers **infra, reliability, observability, and partner onboarding** for the existing calling product. User accounts, encrypted chat, mobile apps, and mesh networking (existing `docs/ROADMAP.md` Phases 2-6) are explicitly **out of scope** and stay in the roadmap as separate plans.

---

## Phase 0 — Context: bugs already fixed today (reference)

These fixes are already committed locally and rebuilding. Listed here so reviewers understand Phase 1 test coverage.

| File | Bug | Fix |
|---|---|---|
| `crates/server/src/router.rs` | `ServeDir` returned 404 for `/{roomId}` with no `content-type`, so Telegram/iMessage previewed room links as "file" instead of landing page. | Added SPA fallback: `ServeDir::new(dir).not_found_service(ServeFile::new("index.html"))`. |
| `crates/server/src/analytics.rs` | Since commit `f36ddae` (7 days ago) the `INSERT` referenced `$6` for `data` but the binding was replaced with `batch.source`, leaving 5 binds for 6 placeholders. Every analytics insert silently errored (`let _ =`). | Added `.bind(&event.data)` and replaced `let _ =` with `if let Err(e) = res { tracing::warn!(...) }`. |

Phase 1 adds regression tests so these cannot reoccur.

---

## File Structure Overview

**Files to create:**
- `crates/server/src/turn_pool.rs` — healthy-server tracking + selection
- `crates/server/src/turn_probe.rs` — STUN binding-request probe
- `crates/server/src/metrics.rs` — Prometheus text encoder + registry wrapper
- `crates/server/src/rate_limit.rs` — per-IP token bucket for `/api/turn-credentials` and `/api/event`
- `crates/server/tests/http_integration.rs` — axum `TestServer` integration tests
- `crates/server/tests/turn_pool.rs` — unit + integration for pool
- `docs/partners/onboarding.md` — partner onboarding runbook
- `docs/runbooks/turn-outage.md` — TURN incident runbook
- `docs/runbooks/analytics-zero-traffic.md` — analytics-alert runbook
- `deploy/grafana/oxpulse-chat.json` — Grafana dashboard JSON
- `web/tests/e2e/room-link-preview.spec.ts` — Playwright test that fetches `/ROOMID` and asserts HTML + OG tags

**Files to modify:**
- `crates/server/src/router.rs` — wire new layers (rate limit, metrics)
- `crates/server/src/main.rs` — start TurnPool + probe task
- `crates/server/src/config.rs` — accept multiple TURN URLs with region tags, probe interval, rate-limit knobs
- `crates/server/src/analytics.rs` — increment metrics on success/failure
- `crates/signaling/src/rooms.rs` — expose counters for metrics
- `crates/server/Cargo.toml` — add deps (`prometheus`, `stun-rs` or `stun_codec`, `governor`)
- `web/src/lib/useCall.svelte.ts` — fix 3 client-side bugs (duplicate `call_connected`, `closed→failed` misclassification, `timerStr` derived shape)
- `web/src/lib/webrtc.ts` — make `iceTransportPolicy` configurable; respect pool-provided `iceTransportPolicy` from server
- `crates/server/src/turn.rs` *(or turn crate)* — already fine; we add pool on top
- `.github/workflows/ci.yml` — add web typecheck + vitest + Playwright smoke
- `compose/apps.yml` — add new env vars, expose `/metrics` (only on internal network)

---

## Phase 1 — Stabilization (regression-proof the two bugs + client cleanups)

### Task 1.1: Integration test for SPA fallback

**Files:**
- Create: `crates/server/tests/http_integration.rs`
- Modify: `crates/server/Cargo.toml` (dev-dependency on `axum-test = "17"`)

- [ ] **Step 1: Add dev-dependency**

Modify `crates/server/Cargo.toml`:
```toml
[dev-dependencies]
axum-test = "17"
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
tempfile = "3"
```

- [ ] **Step 2: Write failing test**

Create `crates/server/tests/http_integration.rs`:
```rust
use axum_test::TestServer;
use oxpulse_chat::router::{build_router, AppState};
use oxpulse_signaling::Rooms;
use std::fs;
use tempfile::tempdir;

fn test_state() -> AppState {
    AppState {
        rooms: Rooms::new(),
        turn_secret: String::new(),
        turn_urls: vec![],
        stun_urls: vec![],
        pool: None,
    }
}

#[tokio::test]
async fn spa_fallback_serves_index_html_for_unknown_paths() {
    let dir = tempdir().unwrap();
    let index_path = dir.path().join("index.html");
    fs::write(&index_path, "<html><head><title>OxPulse</title></head></html>").unwrap();

    let app = build_router(test_state(), dir.path().to_str().unwrap());
    let server = TestServer::new(app).unwrap();

    let response = server.get("/TQFA-9412").await;
    response.assert_status_ok();
    assert!(response.text().contains("<title>OxPulse</title>"));
    let ct = response.header("content-type");
    assert!(ct.to_str().unwrap().starts_with("text/html"));
}

#[tokio::test]
async fn spa_fallback_does_not_shadow_known_static_files() {
    let dir = tempdir().unwrap();
    fs::write(dir.path().join("index.html"), "SPA").unwrap();
    fs::write(dir.path().join("robots.txt"), "User-agent: *").unwrap();

    let app = build_router(test_state(), dir.path().to_str().unwrap());
    let server = TestServer::new(app).unwrap();

    let response = server.get("/robots.txt").await;
    response.assert_status_ok();
    assert_eq!(response.text(), "User-agent: *");
}
```

- [ ] **Step 3: Run — expect it to PASS against current code**

Run: `cargo test -p oxpulse-chat --test http_integration`
Expected: both tests pass (the Phase 0 router fix is already in place).

- [ ] **Step 4: Verify the test actually fails when regressed**

Temporarily revert `router.rs:25` to `let static_dir = ServeDir::new(room_assets_dir);` (no `not_found_service`). Re-run the first test — it must fail with 404. Restore the fix.

- [ ] **Step 5: Commit**

```bash
git add crates/server/Cargo.toml crates/server/tests/http_integration.rs
git commit -m "test(server): add SPA fallback regression tests"
```

### Task 1.2: Integration test for analytics insert (all 6 fields)

**Files:**
- Modify: `crates/server/tests/http_integration.rs`

- [ ] **Step 1: Write failing test**

Append to `crates/server/tests/http_integration.rs`:
```rust
#[tokio::test]
async fn analytics_insert_persists_all_fields() {
    // Requires DATABASE_URL pointing at a throwaway Postgres.
    let url = match std::env::var("TEST_DATABASE_URL") {
        Ok(v) => v,
        Err(_) => { eprintln!("TEST_DATABASE_URL unset, skipping"); return; }
    };
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(2).connect(&url).await.unwrap();
    oxpulse_chat::migrate::run(&pool).await;
    sqlx::query("TRUNCATE call_events").execute(&pool).await.unwrap();

    let mut state = test_state();
    state.pool = Some(pool.clone());
    let dir = tempdir().unwrap();
    fs::write(dir.path().join("index.html"), "SPA").unwrap();
    let app = build_router(state, dir.path().to_str().unwrap());
    let server = TestServer::new(app).unwrap();

    let response = server.post("/api/event").json(&serde_json::json!({
        "did": "test-device-1",
        "src": "oxpulse.chat",
        "events": [
            { "e": "page_view", "r": null, "d": { "referrer": "t.me" } },
            { "e": "room_created", "r": "TEST-0001", "d": {} }
        ]
    })).await;
    response.assert_status(axum::http::StatusCode::NO_CONTENT);

    let rows: Vec<(String, Option<String>, String, serde_json::Value)> = sqlx::query_as(
        "SELECT event_type, room_id, source, data FROM call_events ORDER BY event_type"
    ).fetch_all(&pool).await.unwrap();

    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].0, "page_view");
    assert_eq!(rows[0].2, "oxpulse.chat");
    assert_eq!(rows[0].3["referrer"], "t.me"); // the bug was that `data` was dropped
    assert_eq!(rows[1].0, "room_created");
    assert_eq!(rows[1].1.as_deref(), Some("TEST-0001"));
}
```

- [ ] **Step 2: Run with test DB**

```bash
TEST_DATABASE_URL=postgres://postgres:postgres@localhost:5432/oxpulse_chat_test \
  cargo test -p oxpulse-chat --test http_integration analytics_insert
```
Expected: PASS against current code (Phase 0 fix in place).

- [ ] **Step 3: Regression-verify** — temporarily remove `.bind(&event.data)` from `analytics.rs`, re-run. Must fail with "6 parameters expected, 5 supplied". Restore.

- [ ] **Step 4: Commit**

```bash
git add crates/server/tests/http_integration.rs
git commit -m "test(server): assert analytics persists all 6 fields including data"
```

### Task 1.3: Web E2E test for room-link preview

**Files:**
- Create: `web/playwright.config.ts`
- Create: `web/tests/e2e/room-link-preview.spec.ts`
- Modify: `web/package.json` (add `@playwright/test`, `test:e2e` script)

- [ ] **Step 1: Install Playwright**

```bash
cd web && npm i -D @playwright/test && npx playwright install chromium
```

- [ ] **Step 2: Write the test**

`web/tests/e2e/room-link-preview.spec.ts`:
```ts
import { test, expect } from '@playwright/test';

const BASE = process.env.E2E_BASE_URL ?? 'https://oxpulse.chat';

test('room URL responds with HTML + OG tags (not 404)', async ({ request }) => {
    const res = await request.get(`${BASE}/TQFA-9412`);
    expect(res.status()).toBe(200);
    expect(res.headers()['content-type']).toMatch(/text\/html/);
    const body = await res.text();
    expect(body).toContain('property="og:title"');
    expect(body).toContain('property="og:image"');
});

test('room code renders the join page in a browser', async ({ page }) => {
    await page.goto(`${BASE}/TQFA-9412`);
    await expect(page).toHaveTitle(/OxPulse/);
});
```

- [ ] **Step 3: Commit** and wire to CI in Task 1.8.

### Task 1.4: Fix `call_connected` duplicate tracking

**Files:**
- Modify: `web/src/lib/useCall.svelte.ts:131-145`

**Context:** `onConnectionState` fires every time the connection transitions to `connected` — including after ICE restarts / network switches. Today we fire `track('call_connected', ...)` on every transition, inflating the funnel.

- [ ] **Step 1: Add a `hasConnectedOnce` guard in `useCall`**

```ts
let hasConnectedOnce = false;
// inside onConnectionState:
if (state === 'connected') {
    if (!hasConnectedOnce) {
        hasConnectedOnce = true;
        track('call_connected', opts.roomId, { audio_only: !videoEnabled });
    }
    status = 'connected';
    stopRinging();
    playConnectSound();
    call?.startStatsPolling();
    acquireWakeLock();
    verificationEmoji = call?.getVerificationEmoji() ?? '';
    showControls();
    if (!timer) timer = setInterval(() => { elapsed += 1; }, 1000);
}
```

- [ ] **Step 2: Add unit test** in `web/src/lib/useCall.test.ts` that wraps `createCall` in a mock and asserts `track` is called once across two `connected` transitions.

- [ ] **Step 3: Commit**

```bash
git add web/src/lib/useCall.svelte.ts web/src/lib/useCall.test.ts
git commit -m "fix(web): track call_connected once per call, not per ICE restart"
```

### Task 1.5: Fix `closed` state misclassified as `failed`

**Files:**
- Modify: `web/src/lib/useCall.svelte.ts:142-146`

**Why:** `connectionState === 'closed'` happens on normal hangup. Setting `status = 'failed'` briefly flashes a failure UI before the `ended` transition.

- [ ] **Step 1: Update the branch**

```ts
} else if (state === 'failed') {
    stopRinging();
    status = 'failed';
} else if (state === 'closed') {
    stopRinging();
    if (status !== 'ended') status = 'waiting';
}
```

- [ ] **Step 2: Commit**

```bash
git add web/src/lib/useCall.svelte.ts
git commit -m "fix(web): don't treat 'closed' peer-state as failure"
```

### Task 1.6: Fix `timerStr` derived shape

**Files:**
- Modify: `web/src/lib/useCall.svelte.ts:416-420, 440`
- Modify: `web/src/routes/[roomId]/+page.svelte:122, 147`

**Why:** `$derived(() => ...)` stores the arrow function itself; the code calls it as `call.timerStr()`. Works but bypasses reactive caching. Use `$derived.by`.

- [ ] **Step 1: Change declaration**

```ts
const timerStr = $derived.by(() => {
    const m = Math.floor(elapsed / 60);
    const s = (elapsed % 60).toString().padStart(2, '0');
    return `${m}:${s}`;
});
// getter stays
get timerStr() { return timerStr; },
```

- [ ] **Step 2: Update call sites**

In `web/src/routes/[roomId]/+page.svelte`, replace `call.timerStr()` with `call.timerStr` (both occurrences).

- [ ] **Step 3: Commit**

```bash
git add web/src/lib/useCall.svelte.ts web/src/routes/[roomId]/+page.svelte
git commit -m "fix(web): use \$derived.by for timerStr so reactivity caches"
```

### Task 1.7: Extend CI with web checks

**Files:**
- Modify: `.github/workflows/ci.yml`

- [ ] **Step 1: Add `web` job**

```yaml
  web:
    runs-on: ubuntu-latest
    defaults: { run: { working-directory: web } }
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-node@v4
        with: { node-version: '20', cache: 'npm', cache-dependency-path: web/package-lock.json }
      - run: npm ci
      - run: npm run check
      - run: npm test -- --run
      - run: npx playwright install --with-deps chromium
      - run: E2E_BASE_URL=https://oxpulse.chat npm run test:e2e
```

- [ ] **Step 2: Commit** `ci: add web typecheck, vitest, and production-URL e2e smoke`.

### Task 1.8: Deploy Phase 1 + manual verification

- [ ] **Step 1:** `cd ~/deploy/krolik-server && docker compose build oxpulse-chat && docker compose up -d --no-deps --force-recreate oxpulse-chat`
- [ ] **Step 2:** `curl -sI https://oxpulse.chat/TEST-0001 | head -5` — expect `200` + `content-type: text/html`.
- [ ] **Step 3:** Post a room link in Telegram — verify preview card shows, no "file" badge.
- [ ] **Step 4:** Open any room in two tabs, hang up one, confirm no failure flash in the other.
- [ ] **Step 5:** After a real call, `psql -c "SELECT event_type, data FROM call_events ORDER BY created_at DESC LIMIT 10;"` — expect `data` populated, not `{}`.

---

## Phase 2 — Multi-TURN Partner Integration

**Why now:** The partner brings a VPN-server network that can host coturn instances in multiple regions. Today we accept a flat `TURN_URLS` env list with no awareness of whether any given server is alive, no health probing, and no geo preference. Partner onboarding a new node requires a redeploy. We need a `TurnPool` that:

1. Accepts `[(region, priority, url)]` tuples, refreshable without restart via SIGHUP or a watch file.
2. Actively probes each server via STUN Binding-Request and tracks a rolling health flag.
3. Returns only healthy servers in `/api/turn-credentials`, sorted by region proximity to the caller.
4. Exposes health + probe metrics (Phase 3 consumes them).

### Task 2.1: Define the `TurnPool` type and config shape

**Files:**
- Create: `crates/server/src/turn_pool.rs`
- Modify: `crates/server/src/config.rs`, `crates/server/src/lib.rs`

- [ ] **Step 1:** Add config:

```rust
// config.rs
#[derive(Clone, Debug)]
pub struct TurnServerCfg {
    pub url: String,       // turn:host:3478?transport=udp
    pub region: String,    // ru-msk, ru-spb, de-fra, ...
    pub priority: i32,     // lower = preferred
}

pub struct Config {
    // ...existing fields...
    pub turn_servers: Vec<TurnServerCfg>,
    pub turn_probe_interval_secs: u64,   // default 30
    pub turn_unhealthy_after_fails: u32, // default 3
}
```

Parse from env `TURN_SERVERS="ru-msk:0:turn:host1:3478?transport=udp,ru-spb:1:turn:host2:3478?transport=udp"`.

- [ ] **Step 2:** Skeleton type in `turn_pool.rs`:

```rust
use std::sync::atomic::{AtomicU32, AtomicBool, Ordering};
use std::sync::Arc;
use crate::config::TurnServerCfg;

pub struct TurnServer {
    pub cfg: TurnServerCfg,
    pub healthy: AtomicBool,
    pub consecutive_failures: AtomicU32,
    pub last_rtt_ms: AtomicU32,
}

#[derive(Clone)]
pub struct TurnPool {
    servers: Arc<Vec<Arc<TurnServer>>>,
}

impl TurnPool {
    pub fn new(cfgs: Vec<TurnServerCfg>) -> Self {
        let servers = cfgs.into_iter().map(|cfg| Arc::new(TurnServer {
            cfg,
            healthy: AtomicBool::new(true), // optimistic until first probe
            consecutive_failures: AtomicU32::new(0),
            last_rtt_ms: AtomicU32::new(0),
        })).collect::<Vec<_>>();
        Self { servers: Arc::new(servers) }
    }

    pub fn healthy(&self) -> Vec<Arc<TurnServer>> {
        self.servers.iter().filter(|s| s.healthy.load(Ordering::Relaxed)).cloned().collect()
    }

    pub fn all(&self) -> Vec<Arc<TurnServer>> { self.servers.iter().cloned().collect() }
}
```

- [ ] **Step 3:** Expose module from `lib.rs`:
```rust
pub mod turn_pool;
```

- [ ] **Step 4:** Commit `feat(server): TurnPool skeleton with per-server health flags`.

### Task 2.2: STUN health probe

**Files:**
- Create: `crates/server/src/turn_probe.rs`
- Modify: `crates/server/Cargo.toml` (add `stun_codec = "0.3"`, `bytecodec = "0.4"`, `rand = "0.8"`)

- [ ] **Step 1:** Implement probe that sends STUN Binding-Request over UDP, waits for a response with matching transaction ID, measures RTT.

```rust
use std::net::SocketAddr;
use std::time::{Duration, Instant};
use tokio::net::UdpSocket;
use tokio::time::timeout;

pub async fn probe(addr: SocketAddr) -> Result<u32, String> {
    let sock = UdpSocket::bind("0.0.0.0:0").await.map_err(|e| e.to_string())?;
    sock.connect(addr).await.map_err(|e| e.to_string())?;

    // Minimal STUN Binding-Request: 20-byte header, type=0x0001, magic cookie 0x2112A442, random tid.
    let mut buf = [0u8; 20];
    buf[0..2].copy_from_slice(&0x0001u16.to_be_bytes());
    buf[2..4].copy_from_slice(&0u16.to_be_bytes()); // length 0
    buf[4..8].copy_from_slice(&0x2112A442u32.to_be_bytes());
    rand::Rng::fill(&mut rand::thread_rng(), &mut buf[8..20]);
    let tid = buf[8..20].to_vec();

    let start = Instant::now();
    sock.send(&buf).await.map_err(|e| e.to_string())?;

    let mut resp = [0u8; 1500];
    let fut = async {
        loop {
            let n = sock.recv(&mut resp).await.map_err(|e| e.to_string())?;
            if n >= 20 && &resp[8..20] == tid && (resp[0] & 0x01) == 0x01 { break Ok(()) }
        }
    };
    timeout(Duration::from_secs(3), fut).await.map_err(|_| "timeout".to_string())??;
    Ok(start.elapsed().as_millis() as u32)
}
```

- [ ] **Step 2:** Unit test with a local UDP echo server that returns a valid Binding-Response.
- [ ] **Step 3:** Commit `feat(server): STUN binding-request health probe`.

### Task 2.3: Probe loop + health transitions

**Files:**
- Modify: `crates/server/src/turn_pool.rs`, `crates/server/src/main.rs`

- [ ] **Step 1:** Add to `TurnPool`:

```rust
pub fn start_probe_task(self: &Self, interval: Duration, unhealthy_after: u32) {
    let servers = self.servers.clone();
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(interval);
        loop {
            tick.tick().await;
            for server in servers.iter() {
                let addr = match crate::turn_pool::parse_host_port(&server.cfg.url) {
                    Some(a) => a, None => continue,
                };
                match crate::turn_probe::probe(addr).await {
                    Ok(rtt) => {
                        server.consecutive_failures.store(0, Ordering::Relaxed);
                        server.last_rtt_ms.store(rtt, Ordering::Relaxed);
                        if !server.healthy.swap(true, Ordering::Relaxed) {
                            tracing::info!(region = %server.cfg.region, url = %server.cfg.url, rtt, "turn_server_up");
                        }
                    }
                    Err(e) => {
                        let fails = server.consecutive_failures.fetch_add(1, Ordering::Relaxed) + 1;
                        if fails >= unhealthy_after && server.healthy.swap(false, Ordering::Relaxed) {
                            tracing::warn!(region = %server.cfg.region, url = %server.cfg.url, fails, error = %e, "turn_server_down");
                        }
                    }
                }
            }
        }
    });
}
```

- [ ] **Step 2:** Add `parse_host_port` helper (strip `turn:`/`turns:`, cut `?transport=`, resolve to first SocketAddr via `tokio::net::lookup_host`).
- [ ] **Step 3:** Wire it in `main.rs` after building `AppState`.
- [ ] **Step 4:** Integration test with a mock UDP listener that alternates between responding and silence, assert health flips.
- [ ] **Step 5:** Commit `feat(server): TurnPool probe task with unhealthy threshold`.

### Task 2.4: `/api/turn-credentials` returns only healthy pool

**Files:**
- Modify: `crates/server/src/router.rs` (handler `turn_credentials`)
- Modify: `crates/server/src/main.rs` (pass pool into AppState)
- Modify: `crates/turn/src/lib.rs` (accept `&[String]` that already contains selected urls — unchanged)

- [ ] **Step 1:** Extend `AppState` with `pub turn_pool: TurnPool`.
- [ ] **Step 2:** Change handler:

```rust
async fn turn_credentials(
    headers: axum::http::HeaderMap,
    State(state): State<AppState>,
) -> impl IntoResponse {
    if state.turn_secret.is_empty() {
        return (StatusCode::SERVICE_UNAVAILABLE,
                Json(serde_json::json!({"error": "TURN not configured"})));
    }
    let client_region = geo_hint(&headers); // see Task 2.5
    let mut healthy = state.turn_pool.healthy();
    healthy.sort_by_key(|s| (
        if Some(&s.cfg.region) == client_region.as_ref() { 0 } else { 1 },
        s.cfg.priority,
        s.last_rtt_ms.load(Ordering::Relaxed),
    ));
    let urls: Vec<String> = healthy.iter().take(3).map(|s| s.cfg.url.clone()).collect();
    if urls.is_empty() {
        return (StatusCode::SERVICE_UNAVAILABLE,
                Json(serde_json::json!({"error": "no healthy TURN servers"})));
    }
    let creds = oxpulse_turn::generate_credentials(
        &state.turn_secret, "chat-user",
        Duration::from_secs(86400), &urls, &state.stun_urls,
    );
    (StatusCode::OK, Json(serde_json::to_value(creds).unwrap()))
}
```

- [ ] **Step 3:** Integration test against an `AppState` whose pool has some unhealthy servers — assert they're not in the response.
- [ ] **Step 4:** Commit `feat(server): serve only healthy TURN servers in /api/turn-credentials`.

### Task 2.5: Geo hint from Caddy headers

**Files:**
- Modify: `crates/server/src/router.rs` (new `geo_hint` helper)
- Modify: partner-side Caddy config documented in `docs/partners/onboarding.md`

**Decision:** Don't ship a MaxMind DB. Trust a `CF-IPCountry` or custom `X-Client-Region` header that the edge (Caddy on each partner node) sets. If absent, return None and fall back to `priority` ordering.

- [ ] **Step 1:** Helper:

```rust
fn geo_hint(headers: &axum::http::HeaderMap) -> Option<String> {
    headers.get("x-client-region")
        .or_else(|| headers.get("cf-ipcountry"))
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_ascii_lowercase())
}
```

- [ ] **Step 2:** Document in `docs/partners/onboarding.md`:

> Each partner edge node must set `X-Client-Region: {region-tag}` on proxied requests to `/api/turn-credentials`. Region tags must match the `region` field in the operator's `TURN_SERVERS` entry (e.g. `ru-msk`, `de-fra`).

- [ ] **Step 3:** Test with a mocked header, assert ordering.
- [ ] **Step 4:** Commit `feat(server): geo-aware TURN selection via X-Client-Region header`.

### Task 2.6: Hot-reload of TURN servers (SIGHUP)

**Files:**
- Modify: `crates/server/src/main.rs`
- Modify: `crates/server/src/turn_pool.rs`

**Why:** Adding a partner node shouldn't require a redeploy. Re-read `TURN_SERVERS` env from a file `TURN_SERVERS_FILE=/etc/oxpulse/turn_servers.toml` on SIGHUP.

- [ ] **Step 1:** Add `TurnPool::reload(Vec<TurnServerCfg>)` that swaps the inner `Arc<Vec<_>>` atomically (wrap in `ArcSwap`). Servers missing from the new list are dropped; new ones start "optimistically healthy" until first probe.
- [ ] **Step 2:** Add signal handler in `main.rs`:

```rust
#[cfg(unix)]
tokio::spawn({
    let pool = state.turn_pool.clone();
    let path = config.turn_servers_file.clone();
    async move {
        let mut sighup = signal::unix::signal(signal::unix::SignalKind::hangup()).unwrap();
        while sighup.recv().await.is_some() {
            match load_turn_servers(&path) {
                Ok(new) => { pool.reload(new); tracing::info!("turn_pool reloaded"); }
                Err(e) => tracing::error!(error = %e, "turn_pool reload failed"),
            }
        }
    }
});
```

- [ ] **Step 3:** Unit test: build pool with 2 servers, reload to 3, verify old objects drained.
- [ ] **Step 4:** Commit `feat(server): hot-reload TURN pool on SIGHUP`.

### Task 2.7: Partner onboarding runbook

**Files:**
- Create: `docs/partners/onboarding.md`

- [ ] **Step 1:** Write the runbook covering:
  1. Provision coturn on a partner VPN node (systemd unit template).
  2. Shared `TURN_SECRET` — generate once, share out-of-band.
  3. Firewall: UDP 3478-3479, TCP 3478 (fallback), UDP 49152-65535 (relay ports).
  4. Add `region:priority:turn:host:port?transport=udp` line to `/etc/oxpulse/turn_servers.toml`, send `SIGHUP` to container (`docker kill -s HUP oxpulse-chat`).
  5. Verify in logs: `turn_server_up {region=...}` appears within one probe interval.
  6. Verify from browser devtools: `/api/turn-credentials` response contains the new URL.
  7. Draining a node: set `priority=999`, wait 10 min, remove from file, reload.
- [ ] **Step 2:** Commit `docs(partners): onboarding runbook for TURN operators`.

---

## Phase 3 — Observability (metrics, alerts, runbooks)

### Task 3.1: `/metrics` endpoint

**Files:**
- Create: `crates/server/src/metrics.rs`
- Modify: `crates/server/Cargo.toml` (add `prometheus = { version = "0.13", default-features = false }`)
- Modify: `crates/server/src/router.rs`, `crates/server/src/main.rs`

- [ ] **Step 1:** Define metrics:

```rust
use prometheus::{IntCounter, IntCounterVec, IntGauge, Histogram, HistogramOpts, Opts, Registry};

pub struct Metrics {
    pub registry: Registry,
    pub rooms_active: IntGauge,
    pub ws_connects_total: IntCounter,
    pub ws_disconnects_total: IntCounter,
    pub call_duration_seconds: Histogram,
    pub turn_servers_healthy: IntGauge,
    pub turn_creds_issued_total: IntCounter,
    pub analytics_events_total: IntCounterVec, // labels: result=ok|err
}

impl Metrics {
    pub fn new() -> Self { /* register all */ }
}
```

- [ ] **Step 2:** Handler:

```rust
async fn metrics_handler(State(state): State<AppState>) -> (StatusCode, [(HeaderName, &'static str); 1], String) {
    use prometheus::Encoder;
    let enc = prometheus::TextEncoder::new();
    let mut buf = Vec::new();
    enc.encode(&state.metrics.registry.gather(), &mut buf).ok();
    (StatusCode::OK, [(axum::http::header::CONTENT_TYPE, "text/plain; version=0.0.4")], String::from_utf8(buf).unwrap_or_default())
}
```

- [ ] **Step 3:** Mount on a private route that requires `X-Internal-Token` header (compare constant-time to `METRICS_TOKEN` env) so `/metrics` isn't public.
- [ ] **Step 4:** Commit.

### Task 3.2: Wire metrics into hot paths

**Files:**
- Modify: `crates/signaling/src/rooms.rs`, `crates/signaling/src/handler.rs`
- Modify: `crates/server/src/analytics.rs`, `crates/server/src/router.rs`, `crates/server/src/turn_pool.rs`

- [ ] **Step 1:** `rooms_active` — `inc()` on `join`, `dec()` when `cleanup_expired` removes.
- [ ] **Step 2:** `ws_connects_total.inc()` at the top of `handle_call_ws`, `ws_disconnects_total.inc()` at the bottom.
- [ ] **Step 3:** `call_duration_seconds.observe()` inside the existing `call_ended` log block in `handler.rs`.
- [ ] **Step 4:** `turn_servers_healthy.set(pool.healthy().len() as i64)` at the end of each probe cycle.
- [ ] **Step 5:** `turn_creds_issued_total.inc()` on every successful `turn_credentials` response.
- [ ] **Step 6:** `analytics_events_total.with_label_values(&["ok"|"err"]).inc()` per row in the analytics loop.
- [ ] **Step 7:** Integration test: hit endpoints, scrape `/metrics`, assert expected samples.
- [ ] **Step 8:** Commit.

### Task 3.3: Dozor integration + alerts

**Files:**
- Modify: `~/.dozor/deploy-repos.yaml` (add oxpulse-chat)
- Create: `~/.dozor/alerts/oxpulse-chat.yaml` (or wherever dozor reads alert rules)

- [ ] **Step 1:** Alerts to configure:
  - `turn_servers_healthy < 1` for 2 min → page
  - `rate(turn_creds_issued_total[5m]) == 0` for 15 min during waking hours → page
  - `rate(analytics_events_total{result="err"}[5m]) / rate(analytics_events_total[5m]) > 0.01` for 10 min → warn
  - `rate(call_duration_seconds_count[1h]) == 0` for 2 h (no completed calls) → warn
  - Container down / not-healthy → page (already covered by dozor generic rules)

- [ ] **Step 2:** Route pages to Telegram via vaelor (existing channel).
- [ ] **Step 3:** Commit `ops(dozor): alerts for oxpulse-chat TURN health, analytics, traffic`.

### Task 3.4: Grafana dashboard

**Files:**
- Create: `deploy/grafana/oxpulse-chat.json`

- [ ] **Step 1:** Panels:
  - Rooms active (gauge)
  - Rooms created / joined / ended per hour
  - Call duration p50/p95/p99
  - WS connect success rate
  - TURN servers healthy (status table by region)
  - TURN creds issued per minute by region
  - Analytics error rate
- [ ] **Step 2:** Import once, export JSON, commit.
- [ ] **Step 3:** Commit.

### Task 3.5: Runbooks

**Files:**
- Create: `docs/runbooks/turn-outage.md`, `docs/runbooks/analytics-zero-traffic.md`

- [ ] **Step 1:** `turn-outage.md` — what to do when `turn_servers_healthy` alert fires:
  1. Check `/metrics` to identify which nodes are down.
  2. `journalctl -u coturn` on the affected node (via partner ops channel).
  3. If >50% of regions down, drain traffic via Caddy to the single healthy edge and page partner NOC.
  4. Recovery verification: `turn_server_up` log line + `turn_servers_healthy` back to full count.
- [ ] **Step 2:** `analytics-zero-traffic.md` — distinguish "no traffic" from "analytics broken":
  1. Check `rate(ws_connects_total[5m])` — if also zero, it's a traffic problem, not analytics.
  2. Check `analytics_events_total{result="err"}` — non-zero means DB/migration issue.
  3. Tail `journalctl | grep analytics_insert_failed` for the exact error.
- [ ] **Step 3:** Commit.

---

## Phase 4 — Abuse Protection & Rate Limits

### Task 4.1: Per-IP rate limit on `/api/turn-credentials` and `/api/event`

**Files:**
- Create: `crates/server/src/rate_limit.rs`
- Modify: `crates/server/Cargo.toml` (add `governor = "0.6"`, `nonzero_ext = "0.3"`)
- Modify: `crates/server/src/router.rs`

- [ ] **Step 1:** Token bucket: 10 req/s burst 20 for TURN creds, 5 req/s burst 10 for analytics, keyed on the trusted client-IP header (`X-Forwarded-For` last hop).
- [ ] **Step 2:** Middleware returns 429 with `Retry-After` when exhausted.
- [ ] **Step 3:** Integration test hammering one IP, asserting 429.
- [ ] **Step 4:** Commit.

### Task 4.2: Room-join rate limit + room-id entropy guard

**Files:**
- Modify: `crates/signaling/src/handler.rs`, `crates/signaling/src/rooms.rs`

- [ ] **Step 1:** Reject room IDs longer than 64 chars or not matching `^[A-Za-z0-9_-]{4,64}$` with a close frame before `join`.
- [ ] **Step 2:** Per-IP cap: at most 10 *distinct* rooms in any 60-second window (track in a `DashMap<IpAddr, (window_start, HashSet<String>)>`).
- [ ] **Step 3:** Unit tests.
- [ ] **Step 4:** Commit.

### Task 4.3: Relax `iceTransportPolicy` safely

**Files:**
- Modify: `web/src/lib/webrtc.ts:143`
- Modify: `crates/server/src/router.rs` (extend credential response with `ice_transport_policy`)

**Why:** Today the client forces `relay`. If the TURN pool goes cold, calls fail silently even though STUN P2P would work. Let the server decide policy — `relay` when the pool is healthy (privacy win), fall back to `all` when it isn't (availability win).

- [ ] **Step 1:** Server: include `"ice_transport_policy": "relay" | "all"` in response; pick "all" when pool is empty.
- [ ] **Step 2:** Client: read that field; pass to `RTCPeerConnection`.
- [ ] **Step 3:** Integration test.
- [ ] **Step 4:** Commit `feat: server-decided iceTransportPolicy with STUN fallback on TURN outage`.

---

## Phase 5 — Load and Failover Testing

### Task 5.1: WebSocket load test

**Files:**
- Create: `crates/server/tests/load.rs` (gated behind `#[ignore]`, run manually)

- [ ] **Step 1:** Use `tokio-tungstenite` to open 500 concurrent WS clients that each join a unique room, send one signal, leave. Measure p99 latency from connect→joined.
- [ ] **Step 2:** Document baseline in `docs/runbooks/load-baseline.md`.
- [ ] **Step 3:** Commit.

### Task 5.2: TURN failover drill

- [ ] **Step 1:** In a staging env with 2 TURN nodes, `iptables -j DROP` on the primary during an active call, confirm ICE restart picks up the secondary within 10 s and the call resumes.
- [ ] **Step 2:** Document in `docs/runbooks/turn-outage.md` as "verified failover behaviour" with date.
- [ ] **Step 3:** Commit.

### Task 5.3: Chaos check — analytics DB down

- [ ] **Step 1:** Stop postgres in staging, hit `/api/event`. Expect 204 or 5xx *without* crashing the server, metrics must show the failure, app continues serving calls.
- [ ] **Step 2:** Commit a note in the runbook.

---

## Phase 6 — Launch Checklist

- [ ] All Phase 1-4 tasks merged and deployed.
- [ ] Partner onboarded at least **2** TURN nodes in different regions and they appear healthy in `/metrics` for ≥ 24 h.
- [ ] Grafana dashboard committed and displaying real traffic.
- [ ] Dozor alerts fire in a test (temporarily disable one TURN node).
- [ ] Load test passes baseline (from Task 5.1).
- [ ] `docs/partners/onboarding.md` reviewed by partner ops lead.
- [ ] Privacy policy updated to mention partner TURN network (legal review).
- [ ] Go-live post-mortem template at `docs/runbooks/go-live.md` ready.
- [ ] Roll-back plan tested: `docker compose up -d oxpulse-chat` with previous image tag, verify calls work.
- [ ] Tag release `v0.2.0-partner-launch`, announce in internal channel.

---

## Out of Scope (separate plans)

- **User accounts, contacts, push notifications** → existing `docs/ROADMAP.md` Phase 2.
- **Encrypted chat / file sharing** → Phase 3.
- **Mobile wrappers** → Phase 4.
- **Offline / mesh** → Phases 5-6.
- **Group calls (SFU)** → backlog.
- **Admin dashboard rewrite** — current Go+HTMX dashboard stays; observability moves to Grafana.

---

## Self-Review Notes

- Every task lists concrete file paths and shows the actual code.
- All regression tests are tied to the specific bugs from Phase 0 so CI blocks a re-introduction.
- The `TurnPool` design is the critical piece for partner onboarding; Task 2.6 (SIGHUP reload) is what makes "adding a partner node" a 30-second operation instead of a redeploy.
- Phase 3 alerts specifically cover the *failure class* of both Phase 0 bugs: an "analytics error-rate > 1%" alert would have caught the silent analytics drop on day one, and "0 calls per hour during waking hours" would have caught the Telegram-preview bug via collapsed viral funnel.
- Phase 4 Task 4.3 resolves the latent bug I flagged in the review: `iceTransportPolicy: 'relay'` + STUN fallback being incompatible. It now becomes an explicit server-side decision.
