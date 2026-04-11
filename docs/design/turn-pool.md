# TurnPool Design

**Status:** DRAFT (pending human review before Task 2.3 merges)
**Author:** claude (Opus 4.6) on behalf of oxpulse-chat team
**Reviewer:** TBD — senior Rust engineer
**Last updated:** 2026-04-10

## 1. Context and motivation

oxpulse-chat is a minimal 1-on-1 WebRTC calling service: a small Rust/Axum
binary handles signaling (`/ws/call/{room_id}`) and issues short-lived TURN
credentials (`/api/turn-credentials`). Media never touches our servers —
peers negotiate ICE and either go peer-to-peer or relay through a
partner-hosted coturn fleet.

We need TURN because a meaningful slice of real users sit behind symmetric
NAT, corporate firewalls, or carrier-grade NAT where plain STUN hole-
punching fails. For those users the TURN relay is not an optimization; it
is the only media path. A dead TURN server means a dead call even when
signaling works perfectly.

Why now: a partner is bringing multi-region VPN infrastructure online and
wants to add new TURN nodes on demand. Each new node must enter and leave
the rotation in minutes without recompiling or redeploying the signaling
binary, and operators need to see which nodes are up in real time.

## 2. Goals and non-goals

**Goals:**

- Deliver only healthy TURN servers to clients.
- Prefer geographically close servers (coarse approximation, not GeoIP).
- Onboard a new partner node without recompiling or redeploying.
- Observable: SRE can see which TURN nodes are up/down in real time.
- Zero-downtime probe: in-flight calls survive a probe transition or reload.

**Non-goals:**

- Not a load balancer — coturn itself handles relay session allocation.
- Not a global anycast TURN — out of scope for this phase.
- Not per-user TURN credentials — still a shared `static-auth-secret`.
- Not TURNS/TLS — coturn runs `no-tls`, `no-dtls` today; future work.
- Not STUN attribute parsing (XOR-MAPPED-ADDRESS etc.) — health check only.

## 3. Design

### Data model

```rust
pub struct TurnServer {
    cfg:                  TurnServerCfg,   // url, region, priority
    healthy:              AtomicBool,      // flipped by probe task
    consecutive_failures: AtomicU32,
    last_rtt_ms:          AtomicU32,
}

pub struct TurnPool {
    servers: Arc<Vec<Arc<TurnServer>>>,    // Task 2.6 → ArcSwap<Vec<…>>
}
```

Each `TurnServer` is an `Arc` so handlers can hand out references without
cloning the whole config and probe tasks can mutate atomics independently.
The pool itself is `Arc<Vec<…>>` for now and will become `ArcSwap<Vec<…>>`
in Task 2.6 so hot-reload can swap the entire vector atomically while live
handlers still hold a read view.

### Probe loop (Task 2.3)

A single tokio task spawned at startup polls every
`TURN_PROBE_INTERVAL_SECS` (default 30s). For each server it:

1. Resolves the host and picks one socket address.
2. Calls `turn_probe::probe(addr, 3s)` — a 20-byte STUN Binding-Request
   over UDP with a random 96-bit transaction ID (see `turn_probe.rs`).
3. On success: zero `consecutive_failures`, store `last_rtt_ms`, flip
   `healthy = true` if it was false.
4. On failure: increment `consecutive_failures`; if it crosses
   `TURN_UNHEALTHY_AFTER_FAILS` (default 3), flip `healthy = false`.

The probe treats both Binding-Success and Binding-Error as proof of life —
we only care that the server is reachable and speaks STUN. TURN auth is
deliberately not exercised; coturn permits Binding-Request without auth
(RFC 5389).

### Geo selection (Task 2.5)

The edge (partner Caddy) sets `X-Client-Region` or Cloudflare sets
`CF-IPCountry`. The handler reads whichever is present and filters/sorts
the healthy pool by `(region match, priority, last_rtt_ms)`. If neither
header is set, it falls back to `(priority, last_rtt_ms)`. Up to three
servers are returned to the client so the browser's ICE agent can fail
over.

### Hot-reload (Task 2.6)

SIGHUP re-reads `TURN_SERVERS_FILE`. New entries start optimistically
healthy; existing entries carry over their `healthy` / `consecutive_failures`
/ `last_rtt_ms` history by URL match; removed entries drop out of the
pool. The new `Vec<Arc<TurnServer>>` is installed via `ArcSwap::store`,
atomically visible to every subsequent handler call.

### Flow

```
Client → POST /api/turn-credentials (with CF-IPCountry / X-Client-Region)
        ↓
       Handler → TurnPool.healthy() → sort by (region, priority, rtt) → take 3
        ↓
       HMAC creds (crates/turn) + ICE servers JSON
        ↓
       Client → RTCPeerConnection(iceServers, iceTransportPolicy='relay')

Background probe task:
       every 30s → for each server → UDP STUN probe → flip atomics → metrics
Background reload:
       SIGHUP → read TURN_SERVERS_FILE → parse → ArcSwap::store → done
```

## 4. Alternatives considered

**A. Passive health (client telemetry).** Trust clients to report
`turn_failed` events and mark servers down from that signal.

- Pro: zero server-side work.
- Con: reactive — we learn a TURN is broken only after a user already
  failed. Client-side NAT quirks conflate with real server failures, and
  the channel is trivially poisonable.
- Rejected: we want to know before users do.

**B. DNS SRV weighting.** Publish `turn.oxpulse.chat` as an SRV record
with priorities/weights; let the client resolve directly.

- Pro: standard, battle-tested.
- Con: WebRTC's `RTCIceServer.urls` takes a literal `turn:` URI; browsers
  do not resolve SRV for `turn:`. No per-server health.
- Rejected: no browser support for SRV on `turn:` URIs.

**C. External blackbox health service.** A standalone Go service probes
coturn and publishes a JSON manifest; signaling fetches the manifest per
request.

- Pro: language-agnostic, reusable beyond this project.
- Con: an extra deployment artifact and an extra network hop per
  credential request, plus stale-manifest risk on the signaling side.
- Rejected: we have one small Rust binary — doubling the fleet to save
  300 lines of Rust is a bad trade.

**D. coturn HTTP health endpoint.** Rely on coturn's built-in HTTP admin
port and probe that.

- Pro: no custom code.
- Con: requires opening another port per partner node, exercises a
  different code path than real users, and doesn't prove UDP 3478 is
  actually reachable end-to-end.
- Rejected: probe what users actually use.

**E. `rand` crate instead of `getrandom` for the STUN transaction ID.**

- Pro: idiomatic Rust default.
- Con: ~1 MB of transitive deps and a generator state machine for 12
  random bytes once every 30 seconds.
- Rejected: `getrandom` is a thin syscall wrapper and we don't need
  `rand`'s full CSPRNG plumbing.

**Chosen:** `Arc<Vec<Arc<TurnServer>>>` + tokio probe task + raw STUN
Binding-Request. It (1) probes the same path users use, (2) adds no new
deployment artifacts, (3) fits in ~300 lines of Rust, (4) adds zero new
transitive deps beyond `getrandom`.

## 5. Observability hooks

What the probe task emits and who reads it:

- **Structured logs** (tracing): `turn_server_up` / `turn_server_down`
  events with `region`, `url`, `consecutive_failures`, `rtt_ms` fields.
- **Prometheus metrics** (Task 3.1/3.2): `turn_servers_healthy` gauge and
  `turn_creds_issued_total` counter, scraped at `/metrics`.
- **Dozor alert** (Task 3.3): `turn_servers_healthy < 1` for 2 minutes
  pages Telegram.

No Grafana. The metrics are for machines (Dozor alerting) and ops who
curl `/metrics` directly. Human-facing product dashboards live in
oxpulse-admin and are out of scope for this design.

## 6. Risks and mitigations

- **Malformed partner node added** — `parse_turn_servers` logs and skips
  broken entries; the process keeps running with the servers it could
  parse (`config.rs`).
- **Thundering herd on reload** — not a concern: probes are independent,
  the `ArcSwap` swap is a pointer store, no probe restart happens.
- **Probe passes but media fails under load** — STUN passes while TURN
  relay allocation saturates. Accepted limitation; Phase 5 load-test
  exercises real allocation.
- **IPv6 bypass** — `turn_probe::probe` binds `0.0.0.0:0` or `[::]:0`
  matching the target family. Partner firewall rules must match; called
  out in the partner runbook.
- **Clock skew on HMAC** — TURN REST credentials are Unix-timestamp
  based; partner nodes MUST run NTP. Documented in
  `docs/partners/onboarding.md`.
- **DoS on `/api/turn-credentials`** — rate limiting lands in Task 4.1.
- **Probe overhead** — 30s × N servers × ~1 ms per probe is negligible;
  scales comfortably past 100 nodes without tuning.

## 7. Future work (out of scope for Phase 2)

- TURNS (TURN over TLS/DTLS) for networks that block UDP 3478.
- Per-user, short-lived TURN credentials for abuse containment.
- Real GeoIP via MaxMind GeoLite2 instead of a trusted header.
- `str0m`-native relay (skip coturn entirely) — speculative, far future.
- Load-based routing (prefer least-loaded, not just least-RTT).

## 8. Review and approval

- **Author:** claude (Opus 4.6) on behalf of oxpulse-chat team
- **Reviewer:** TBD — senior Rust engineer
- **Status:** DRAFT (pending human review before Task 2.3 merges)
- **Last updated:** 2026-04-10
