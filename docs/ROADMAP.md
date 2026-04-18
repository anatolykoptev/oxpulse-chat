# OxPulse Chat — Roadmap

## Vision

Open-source encrypted messenger with video calls. Works in Russia, offline, and without registration. Growth engine for OxPulse Business (paid widget).

**Live:** https://oxpulse.chat
**Repo:** https://github.com/anatolykoptev/oxpulse-chat

---

## Phase 1: Standalone Calling (v0.1.0) — DONE

**Goal:** Extract call system from oxpulse monolith, deploy as standalone product.

### Signaling Server (Rust/Axum)
- [x] WebSocket endpoint `/ws/call/{room_id}` — JSON relay (offer/answer/ICE)
- [x] Room management — DashMap with TaggedSignal broadcast
- [x] Polite/impolite role assignment (Perfect Negotiation)
- [x] Room cleanup on disconnect (10 min grace period)
- [x] Max 2 participants per room
- [x] Peer ID tracking, connection timestamp, call duration

### TURN Credentials
- [x] `POST /api/turn-credentials` — HMAC-SHA1 temporary credentials (RFC 5766)
- [x] Configurable shared secret, TTL 24h
- [x] ICE servers list in response (STUN + TURN)

### WebRTC Client (SvelteKit 5)
- [x] Perfect Negotiation pattern (0 JS dependencies)
- [x] Adaptive bitrate (1.5Mbps default, reduce on packet loss)
- [x] Codec preferences: H.264 > VP8, audio 48kHz Opus
- [x] ICE restart on disconnected/failed state
- [x] WebSocket reconnect with exponential backoff
- [x] Screen sharing (`getDisplayMedia` + `replaceTrack`)
- [x] Camera flip (rear/front on mobile)
- [x] Speaker/earpiece toggle
- [x] Wake Lock API (screen stays on during call)
- [x] E2EE verification (emoji codes from DTLS fingerprints)
- [x] Auto-hiding controls with idle timer
- [x] Notification sounds (peer connect/disconnect)
- [x] Bilingual (ru/en) with auto-detection
- [x] ABCD-1234 room codes with auto-format input
- [x] Audio-only fallback for low bandwidth
- [x] Local microphone preference (skip iPhone Continuity)

### Infrastructure
- [x] Standalone Rust binary (<30MB Docker image, ~500 LOC)
- [x] Caddy reverse proxy with automatic SSL (Let's Encrypt)
- [x] VLESS/Reality tunnel for Russian users (DPI-invisible)
- [x] Coturn relay on Hostiman (media stays in Russia)
- [x] Brotli + gzip compression
- [x] Immutable cache headers for hashed assets
- [x] Security headers: CSP, X-Frame-Options, HSTS
- [x] Graceful shutdown (SIGINT + SIGTERM)
- [x] GitHub Actions CI (lint + test)
- [x] AGPL-3.0 license

### Tests
- [x] 3 unit tests (TURN credential generation)
- [x] 7 integration tests (join, polite/impolite, signal relay, room full, peer left, TURN, health)

### Analytics (Privacy-Preserving)
- [x] Anonymous device_id (random UUID in localStorage, no fingerprinting)
- [x] 5 event types: page_view, room_created, room_joined, call_connected, call_ended
- [x] Batch transport (sendBeacon, max 20 events/req)
- [x] Source field per domain (oxpulse.chat vs call.piter.now)
- [x] PostgreSQL storage (optional — app works without DB)
- [x] Tracker on both domains (oxpulse.chat + call.piter.now → shared call_events table)
- [x] Admin dashboard at oxpulse.chat/admin/ (Go + HTMX, Chart.js, dark theme)
- [x] Viral funnel: page_view → room_created → call_connected → repeat creators
- [x] Call analytics: daily calls, duration, unique devices
- [x] Device activity: top devices by rooms/calls (truncated IDs, no PII)

---


## Partner Network & Operational Maturity (partner-edge v0.2.0)

**Goal:** Multi-region partner-edge deployments, observability, abuse protection. Parallel track to Phase 2 accounts below — focused on censorship-resistance infra, not user accounts.

Tracked in `docs/superpowers/plans/2026-04-10-oxpulse-chat-partner-launch.md` + `2026-04-11-oxpulse-chat-phase2-continuation.md`.

### TurnPool + geo routing
- [x] Structured `TURN_SERVERS` config format (`region:priority:url`)
- [x] `TurnPool` container + accessors (Task 2.1-2.2)
- [x] STUN binding-request probe loop with health transitions (Task 2.3)
- [x] `/api/turn-credentials` serves only healthy pool members (Task 2.4)
- [x] Geo-hint from client headers (`X-Client-Region` / `CF-IPCountry`) (Task 2.5)
- [x] SIGHUP hot-reload of TURN server list via ArcSwap (Task 2.6)

### Partner-edge v0.2.0 (TURNS-on-:443, Variant A')
- [x] Architectural PoC + DECISION.md (caddy-l4 SNI mux)
- [x] Docker bundle (caddy + xray-client + coturn)
- [x] `install.sh` with `--bake` mode
- [x] `hydrate.sh` per-clone first-boot script
- [x] Cert-watch systemd units for coturn reload
- [x] GHCR images published: `partner-edge-{caddy,xray,coturn}:v0.2.0`
- [x] Cover page + `@probe` matcher (R1 Layer 2 active-probing defense)
- [x] Partner registration API (`POST /api/partner/register` with bootstrap tokens)
- [x] `partner-cli` for token issuance + node listing
- [x] Multi-partner branding resolver (4 partners registered: oxpulse, piter, rvpn, ...)
- [ ] Partner deployment to `rvpn` (blocked on wildcard DNS)
- [ ] Register-to-use end-to-end validation (Task 7 of turn-node-template plan)

### Observability
- [x] Prometheus `/metrics` endpoint with token auth (Task 3.1)
- [x] 10 SLO-aligned metrics wired into hot paths (Task 3.2)
- [ ] Dozor alert rules (Task 3.3) — deferred
- [ ] Grafana dashboard (Task 3.4) — deferred
- [ ] Runbooks for TURN outage + WS failure (Task 3.5) — deferred

### Abuse protection
- [x] Per-IP rate limit on `/api/turn-credentials` + `/api/event` (Task 4.1)
- [x] Room-ID entropy validation + join rate limit (Task 4.2)
- [x] Server-decided `iceTransportPolicy` (Task 4.3)

### Anti-censorship hardening (April 2026)
- [x] 5-way SNI rotation across samsung.com subdomains (all covered by `*.samsung.com` wildcard SAN — active-probing defense intact)
- [x] Deterministic per-node SNI picking via sha256(node_id) — diversifies (IP, SNI) fingerprint for ТСПУ DPI clustering
- [x] ML-KEM-768 post-quantum VLESS encryption on `:5349` (Harvest-Now-Decrypt-Later defense against future CRQC)
- [x] `PartialReality` + `assemble_reality_creds` flow — encryption/SNI picked per-node at register time
- [x] Dead DoH block in xray-client template removed (xray-client never resolves hostnames in our architecture)
- [x] Xray 26.x verified safe from uTLS CVE-2026-27017 (cipher-suite fingerprint mismatch on GREASE ECH)
- [ ] Xray `fingerprint: "random"` or per-node diversified presets — pending
- [ ] VLESS Seed (XTLS Vision Flow) — not XHTTP-compatible, skipped
- [ ] Multi-domain DoH for future hostname-based BACKEND failover (P0.2 second-backend track)

### Load & chaos
- [ ] WebSocket load test (1000 concurrent joins) (Task 5.1)
- [ ] TURN failover drill (Task 5.2)
- [ ] Analytics DB down chaos test (Task 5.3)

### Launch checklist
- [ ] Full partner-launch readiness review (Task 6.1)

---

## Phase 2: Accounts & Contacts (v0.2.0)

**Goal:** Users can register, save contacts, and call with one click.

- [ ] Email registration (magic link, no passwords)
- [ ] User profile (name, avatar)
- [ ] Personal room (permanent link: oxpulse.chat/@username)
- [ ] Contact list (add by username/email)
- [ ] Call history (date, duration, peer)
- [ ] Quick call from contacts (one-click)
- [ ] Push notifications (Web Push API for PWA)
- [ ] PostgreSQL for user data

---

## Phase 3: Encrypted Chat (v0.3.0)

**Goal:** Signal-like 1-on-1 encrypted messaging.

- [ ] Double Ratchet protocol (X3DH key exchange)
- [ ] Prekey bundles (server stores public prekeys)
- [ ] Message storage: encrypted blobs, decrypted client-side
- [ ] Offline message queue (deliver when recipient online)
- [ ] Read receipts, typing indicators
- [ ] File/image sharing (encrypted, size limit)
- [ ] Message deletion (local + remote)
- [ ] Chat UI integrated with call UI

---

## Phase 4: Mobile Apps (v0.4.0)

**Goal:** Native wrapper for push notifications and Bluetooth/mesh.

- [ ] Capacitor wrapper (iOS + Android)
- [ ] Native push notifications (APNs + FCM)
- [ ] Background WebSocket keep-alive
- [ ] App Store / Google Play submission
- [ ] Biometric lock (FaceID/fingerprint)

---

## Phase 5: Offline P2P Chat (v0.5.0)

**Goal:** Direct messaging without internet via Bluetooth/Wi-Fi Direct.

- [ ] Bluetooth Low Energy discovery (nearby OxPulse users)
- [ ] Wi-Fi Direct for higher bandwidth (file transfer, voice)
- [ ] P2P encrypted channel (ECDH key exchange over BLE)
- [ ] Contact exchange via QR code
- [ ] Offline message queue → sync when internet returns
- [ ] Range indicator in UI

---

## Phase 6: Mesh Network (v0.6.0)

**Goal:** Messages hop through other OxPulse users to reach distant peers.

- [ ] Store-and-forward relay (each phone = node)
- [ ] Onion-style routing (each hop knows only prev/next)
- [ ] TTL-based message expiry
- [ ] Gossip protocol for peer discovery
- [ ] Mesh status indicator (connected nodes, hops)
- [ ] Battery-aware relaying (opt-in, skip when <20%)
- [ ] No metadata leakage (sender/recipient hidden from relays)

---

## Backlog (unscheduled)

- [ ] Group calls (SFU via str0m, max 4 participants)
- [ ] Group chats
- [ ] Voice messages
- [ ] AI noise suppression (RNNoise WASM)
- [ ] Background blur (MediaPipe)
- [ ] Live captions (Web Speech API)
- [ ] Client-side recording (MediaRecorder → .webm)
- [ ] Picture-in-Picture (browser API)
- [ ] Accessibility (ARIA, keyboard nav, screen reader)

## Competitive Positioning

| Feature | Zoom | Signal | Jitsi | Briar | **OxPulse** |
|---------|------|--------|-------|-------|-------------|
| Video calls | Yes | Yes | Yes | No | **Yes** |
| E2E encrypted chat | No | Yes | No | Yes | **Planned** |
| Works in Russia | No | No | No | Yes (Tor) | **Yes (VLESS)** |
| No account for calls | No | No | Yes | No | **Yes** |
| Offline/mesh | No | No | No | Yes | **Planned** |
| Self-hosted | No | No | Yes | N/A | **Yes** |
| Open source | No | Yes | Yes | Yes | **Yes** |
| Backend <50MB | No | No | No | N/A | **Yes (~30MB)** |
