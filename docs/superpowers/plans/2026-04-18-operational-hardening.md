# OxPulse Chat — Operational Hardening (2026-04-18)

## Status context

Phase 2 partner-launch tasks landed via:
- TurnPool + probe loop + healthy-pool filter (Tasks 2.1-2.4)
- Prometheus `/metrics` endpoint + hot-path instrumentation (Tasks 3.1-3.2)
- Partner registry with token-based `/api/partner/register` (partner-edge v0.2.0 bundle)

5 tasks remain before partner-launch SLO gates:

| # | Task | Effort | Wave |
|---|---|---|---|
| 1 | Task 4.2 — Room-ID entropy + join rate limit | ~30 min | 1 |
| 2 | Task 2.5 — Geo-hint helper + wire into TurnPool sort | ~15 min | 1 |
| 3 | Task 4.1 — Per-IP rate limit on API endpoints | ~90 min | 2 |
| 4 | Task 4.3 — Server-decided `iceTransportPolicy` | ~30 min | 3 |
| 5 | Task 2.6 — SIGHUP hot-reload of TURN list (ArcSwap) | ~90 min | 3 |

Wave assignments prevent merge conflicts: Wave 1 tasks touch disjoint files (signaling crate vs router.rs helper). Waves 2-3 serialize after 1 merges.

---

## Wave 1 — parallel (non-conflicting)

### Task 4.2 — Room-ID entropy guard + per-IP join rate limit

**Files:** `crates/signaling/src/handler.rs`, `crates/signaling/Cargo.toml` (`governor` or hand-rolled), new `crates/signaling/src/rate_limit.rs`

**Spec:**

1. Before `wait_for_join`, validate `room_id`:
   - Regex: `^[A-Z0-9]{4}-[0-9]{4}$` (product convention, see SPA input auto-format) OR `^[a-z0-9-]{4,32}$` for custom slugs. Pick one and document.
   - On mismatch: send `ServerMsg::Error { message: "invalid room id" }` and return. Increment `on_ws_join_err`.
2. Per-IP join rate limit: `governor::RateLimiter` keyed on `IpAddr` from `ConnectInfo`, quota 30/min per IP.
   - On limit exceeded: send `ServerMsg::Error { message: "rate limit exceeded" }`, return, increment `on_ws_join_err`.
3. Unit test: room-id regex accepts valid, rejects invalid (3-4 cases).
4. Integration test: burst of 40 joins from same IP returns at least 10 rate-limited errors within 1s.

**Acceptance:**
- New `crates/signaling/src/rate_limit.rs` with `JoinLimiter` wrapper type.
- `handle_call_ws` extended with 2 pre-Joined guards.
- All existing tests still pass.
- 1 unit + 1 integration test added.

### Task 2.5 — Geo-hint from client headers + TurnPool sort

**Files:** `crates/server/src/router.rs`

**Spec:**

1. Add helper function:
   ```rust
   fn geo_hint(headers: &HeaderMap) -> Option<String> {
       headers
           .get("x-client-region")
           .or_else(|| headers.get("cf-ipcountry"))
           .and_then(|v| v.to_str().ok())
           .map(|s| s.to_ascii_lowercase())
   }
   ```
2. In `turn_credentials_inner`: if `geo_hint` returns `Some(region)` and any healthy pool member has `cfg.region` starting with that hint, sort those to the top. Otherwise, keep current priority-ascending order.
   - Matching: prefix match case-insensitive (e.g. `ru` matches `ru-spb`, `ru-msk`). If client sends `ru-spb`, first sort entries with exact region match, then prefix match, then other.
3. Handler signature: `turn_credentials(State<AppState>, HeaderMap)` — extract HeaderMap to read hint.
4. Unit tests:
   - `geo_hint_prefers_x_client_region` — both headers present → x-client-region wins
   - `geo_hint_lowercases` — `"RU-MSK"` → `Some("ru-msk".into())`
   - `geo_hint_returns_none_without_headers`
5. Integration test: build a pool with `ru-spb:0` + `de-fra:5`, send `X-Client-Region: ru`, assert `ru-spb` appears first in response.

**Acceptance:**
- Helper + 3 unit tests + 1 integration test.
- `turn_credentials_inner` uses hint to reorder healthy list.
- `X-Client-Region` takes precedence over `CF-IPCountry`.

---

## Wave 2 — after Wave 1 merges

### Task 4.1 — Per-IP rate limit on API endpoints

**Files:** `crates/server/Cargo.toml`, new `crates/server/src/rate_limit.rs`, `crates/server/src/router.rs`

**Spec:** full spec in `docs/superpowers/plans/2026-04-11-oxpulse-chat-phase2-continuation.md` Task 4.1.

Summary:
- `governor` dep
- `KeyedLimiter` type (RateLimiter<IpAddr, DashMapStateStore, DefaultClock>)
- 2 middleware layers:
  - `/api/turn-credentials` — quota 30/min per IP
  - `/api/event` — quota 60/min per IP
- Reject with 429 + `Retry-After` header
- Integration test: burst to limit, assert 429 after N requests

---

## Wave 3 — after Wave 2

### Task 4.3 — Server-decided `iceTransportPolicy`

**Files:** `crates/server/src/router.rs`, `crates/turn/src/lib.rs` (response struct)

**Spec:**
- Extend `/api/turn-credentials` response with `ice_transport_policy: "all" | "relay"`
- Default `all`. If `TurnPool::healthy().is_empty() && turn_urls.is_empty()` → can't force relay, return `all`.
- If env `FORCE_RELAY_REGIONS=ru,ru-spb` is set AND geo_hint matches one of those regions → `relay`.
- Otherwise `all`.
- SPA will consume this field when setting `RTCPeerConnection` config (client-side change tracked separately).

**Acceptance:**
- Response shape extended.
- 3 unit tests covering the decision matrix.

### Task 2.6 — SIGHUP hot-reload of TURN server list

**Files:** `crates/server/Cargo.toml` (add `arc-swap`), `crates/server/src/turn_pool.rs`, `crates/server/src/main.rs`

**Spec:** full spec in `docs/superpowers/plans/2026-04-11-oxpulse-chat-phase2-continuation.md` Task 2.6.

Summary:
- `TurnPool.servers`: `Arc<Vec<Arc<TurnServer>>>` → `arc_swap::ArcSwap<Vec<Arc<TurnServer>>>`
- All accessors get `.load()` wrapping
- New `TurnPool::reload(Vec<TurnServerCfg>)` method — rebuilds the Vec atomically, preserves healthy flags for URLs present in both old and new sets
- In `main.rs` (unix only): `tokio::signal::unix::SIGHUP` handler — re-reads `TURN_SERVERS` env and calls `reload`
- Unit test: `reload` preserves healthy flags for matched URLs, fresh entries start optimistic-true

---

## PR strategy

Each task in its own branch `feat/task-<N>-<slug>`, targeting `main`:
- Wave 1: 2 PRs created in parallel
- Wave 2: 1 PR after Wave 1 merges
- Wave 3: 2 PRs (4.3 first, then 2.6)

Each PR:
- Must be green in CI (cargo test workspace + clippy)
- Must include acceptance tests from spec
- Must NOT break any existing test
- Must have `Task plan` checklist in description

## Subagent handoff contract

Each subagent:
1. Reads only its assigned task section from this plan
2. Works exclusively in its assigned worktree
3. Runs `cargo test -p oxpulse-chat` (and signaling crate for Task 4.2) — must be green
4. Commits following `feat(server|signaling): <imperative summary>` convention
5. Creates PR via `gh pr create --base main` with a body linking back to this plan
6. Reports PR URL + test result summary

Subagents do NOT:
- Merge to main
- Touch other tasks' files
- Skip tests
- Add features beyond the spec's acceptance list
