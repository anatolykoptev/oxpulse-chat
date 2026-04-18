# Partner-edge Architecture

Complete reference for deploying an oxpulse.chat partner edge node:
components, traffic flows, DNS, ACME, registration protocol, secrets,
upgrade/drain/rollback, and the relationship to the piter/krolik
backbone.

This document describes the state as of 2026-04-18
(partner bundle version `partner-edge-v0.2.1`). The hands-on runbook
for partner DevOps is [`onboarding.md`](onboarding.md).

## 1. Purpose

1. **Bypass Russian ТСПУ/DPI blocking** for WebRTC signaling and the
   branded SPA. Traffic from the browser to the edge looks like normal
   TLS to the partner's own domain — the upstream origin `oxpulse.chat`
   never appears on the wire.
2. **Geographically distributed TURN relays.** Partners host coturn near
   their end users, reducing WebRTC media latency.
3. **TURNS-on-:443** as a fallback for users behind strict NAT/DPI where
   UDP is blocked entirely. Media is carried over TCP/TLS on port 443
   and is indistinguishable from HTTPS traffic.

## 2. Components

A partner node consists of three Docker containers and a small set of
systemd units, all under `/etc/oxpulse-partner-edge/` and
`/var/lib/oxpulse-partner-edge/`.

```
partner-edge VM (Debian 12 / Ubuntu 22+/RHEL 9)
├── docker compose
│   ├── caddy     — TLS, ACME, SNI mux, reverse-proxy /api, /ws, /   (ports 80, 443)
│   ├── xray-client — VLESS + ML-KEM + Reality + XHTTP tunnel outbound (no host port)
│   └── coturn    — TURN/STUN on 3478 UDP/TCP + TURNS :5349 (exposed via caddy-l4 on :443)
└── systemd
    ├── oxpulse-partner-edge.service       — docker compose up
    ├── oxpulse-partner-edge-hydrate.*     — oneshot for VM clones (re-seed secrets)
    ├── oxpulse-partner-cert-watch.path    — inotify on Caddy cert renewal
    └── oxpulse-partner-cert-watch.service — signals SIGUSR2 to coturn on renewal
```

### 2.1 Caddy (`oxpulse-partner-caddy`)

- TLS termination for `<partner-domain>` and `<turns-sub>.<partner-domain>`.
  Certificates are obtained via ACME HTTP-01 from Let's Encrypt and
  stored in the `caddy-data` volume.
- **SNI multiplexing via caddy-l4 (`listener_wrappers.layer4`)**: on :443,
  the wrapper peeks at the TLS ClientHello before the HTTP app sees it.
  If the SNI matches `<turns-sub>.<partner-domain>`, the connection is
  raw-TCP proxied to `127.0.0.1:5349` where coturn terminates its own
  TLS using the same Caddy-issued cert. Any other SNI falls through to
  the normal HTTP app.
- HTTP handlers on `<partner-domain>`:
  - `/api/*`, `/ws/*`, `/events/*` → `xray-client:3080` (through VLESS tunnel)
  - `/_app/immutable/*` → `xray-client:3080` with `Cache-Control: 1y immutable`
  - `GET /` without Origin/Cookie → cover page from `/srv/cover`
    (R1 Layer 2 active-probing defense)
  - everything else (SPA fallback) → `xray-client:3080`
- Headers: HSTS, `X-Content-Type-Options`, `X-Frame-Options: DENY`,
  `Referrer-Policy: no-referrer`. `Server`, `Via`, and `Alt-Svc` are
  stripped.
- **TURNS subdomain site block**: Caddy keeps a dedicated block for
  `<turns-sub>.<partner-domain>` with `disable_tlsalpn_challenge` and
  `respond 421`. The point is: ACME HTTP-01 on port 80 (which Caddy
  fully owns) keeps working, while :443 traffic for this SNI is
  intercepted by caddy-l4 and forwarded to coturn before the HTTP app
  ever sees it. TLS-ALPN-01 must be disabled, otherwise renewal breaks
  after the first cycle.

### 2.2 xray-client (`oxpulse-partner-xray`)

- **Inbound**: `dokodemo-door` on `:3080` (docker-network only).
  The forward target is set to `127.0.0.1:8907`, but routing immediately
  sends all input to the `vless-tunnel` outbound.
- **Outbound**: VLESS + Reality + XHTTP →
  `<backend_endpoint>` (default `192.9.243.148:5349`, which is the
  operator-side xray-reality on krolik, Oracle Cloud).
  - `serverName: www.samsung.com` / `cdn.samsung.com` etc.
    (the `*.samsung.com` SAN covers them; rotation via backend).
  - `fingerprint: chrome` (uTLS, CVE-2026-27017 fix since xray ≥ 26.2.6).
  - **`encryption: mlkem768x25519plus.native.0rtt.<long-payload>`** —
    ML-KEM-768 post-quantum hybrid. Server and client must match
    byte-for-byte; a mismatch silently drops packets after a "successful"
    TLS handshake. This was the root cause of the v0.2.0 regression (see §9).
  - Transport: **XHTTP** (uplink and downlink are split into a pair of
    HTTP connections on path `/xh`). Migrated from plain TCP on 2026-03-25.

Architecturally this is a simplified one-channel version of what runs on
piter: only CH1 (VLESS+ML-KEM+Reality+XHTTP). CH2 AmneziaWG and CH3
wstunnel-WSS are not yet available on partner-edge — see roadmap in §12.

### 2.3 coturn (`oxpulse-partner-coturn`)

- `network_mode: host` — TURN needs a real public IP and the UDP
  relay port range 49152–65535 (opened by the partner's firewall).
- Ports:
  - `UDP 3478` + `UDP 3479` — main STUN/TURN
  - `TCP 3478` — TURN/TCP fallback
  - `TCP 5349` — TURNS (TLS), listens on `0.0.0.0:5349` and `127.0.0.1:5349`.
    :5349 is closed at the firewall; only the Caddy container reaches it
    over loopback via l4 mux.
  - `UDP 49152–65535` — relay ports (opened by the partner's firewall)
- Auth: `use-auth-secret` + `static-auth-secret=<TURN_SECRET>`.
  Credentials are minted by the signaling server
  (`/api/turn-credentials`) using the formula
  `username = <expiry_unix>:chat-user`,
  `credential = HMAC-SHA1(secret, username)`.
  coturn never speaks to the signaling server — just the shared secret.
- TLS cert/pkey read from the read-only volume `caddy-data:/data`, path
  `/data/caddy/certificates/acme-v02.api.letsencrypt.org-directory/<turns-sub>.<domain>/...`.
  **Important**: the mount used to be `caddy-data:/data/caddy:ro`, which
  hid the cert one level deeper (`/data/caddy/caddy/certificates/...`)
  and caused coturn to silently disable the TLS listener. Fixed in
  v0.2.1.
- On renewal the `cert-watch.path` unit triggers
  `docker exec coturn kill -s USR2 1`; coturn reloads the cert
  without restarting.

### 2.4 Systemd units

| Unit | Purpose |
|------|---------|
| `oxpulse-partner-edge.service` | `docker compose up -d` in `/etc/oxpulse-partner-edge`. Primary control point |
| `oxpulse-partner-edge-hydrate.service` | Oneshot, sentinel-gated. On the first boot of a VM clone (from `install.sh --bake` snapshot) calls `hydrate.sh`, which re-calls `/api/partner/register` and re-renders templates |
| `oxpulse-partner-cert-watch.path` | inotify on `/var/lib/docker/volumes/oxpulse-partner-edge_caddy-data/_data/caddy/certificates/<turns-sub>.<domain>/<turns-sub>.<domain>.crt` |
| `oxpulse-partner-cert-watch.service` | Sends `SIGUSR2` to coturn when triggered. If this unit fails, renewal still lands eventually via the 10-minute coturn watchdog |

## 3. Traffic flows

### 3.1 WebRTC signaling (HTTP / WebSocket)

```
Browser (RU) → call.partner.example:443 (Caddy TLS)
             → caddy-l4: SNI not matched → HTTP app
             → reverse_proxy xray-client:3080
             → VLESS+ML-KEM+Reality+XHTTP → krolik xray-reality:5349
             → freedom outbound → 127.0.0.1:8907 (oxpulse-chat signaling)
```

Caddy sets:
- `X-Forwarded-Host: <partner-domain>` — the signaling server resolves
  partner branding by this header
- `Host: oxpulse.chat` — backend origin logic works as if called directly
- `X-Forwarded-Proto: https`

### 3.2 TURN/STUN (UDP 3478)

```
Browser → <partner-domain>:3478/UDP (direct, no TLS)
        ↔ coturn (HMAC auth) ↔ Browser peer B
```

coturn relays media peer-to-peer; the signaling host is never in the
media path.

### 3.3 TURNS-on-:443 (TCP/TLS)

For users behind DPI where UDP is blocked entirely:

```
Browser → <turns-sub>.<partner-domain>:443 (TLS ClientHello)
        → Caddy l4 listener_wrapper peeks SNI
        → SNI matches "turns-sub" → raw-TCP proxy to 127.0.0.1:5349
        → coturn terminates TLS (cert issued by Caddy via HTTP-01 ACME)
        → TURNS messages flow as with a regular TURN server
```

Key properties:
- `<turns-sub>` is backend-assigned, format `api-<6-hex>`, stable per
  node across re-registration.
- This is **not a separate listener** on port 5349 — everything goes
  through :443.
- The Caddy-issued cert for `<turns-sub>.<partner-domain>` is shared
  with coturn through a read-only volume; smooth reload on renewal via
  SIGUSR2.
- To the client this looks like ordinary HTTPS to :443, which defeats
  UDP-specific DPI.

### 3.4 ACME validation

```
Let's Encrypt → http://<domain>/.well-known/acme-challenge/<token>
              → Caddy (owns :80) → served inline
→ cert written to caddy-data volume
```

- Two certs are issued: one for the apex and one for
  `<turns-sub>.<domain>`.
- TLS-ALPN-01 is disabled for the turns-sub, because once caddy-l4
  captures :443 for that SNI LE can no longer complete the challenge.

## 4. DNS

The partner must publish two A records (both pointing at the VM's
public IP):

| Record | Value | Reason |
|--------|-------|--------|
| `<partner-domain>` | `<public-ip>` | Apex, cert + trusted-proxy match |
| `<turns-sub>.<partner-domain>` | `<public-ip>` | TURNS cert + caddy-l4 SNI mux |

`<turns-sub>` is returned by the backend in the `/api/partner/register`
response (format `api-<6-hex>`). Two supported workflows:

- **Wildcard (recommended)**: `*.<partner-domain> A <public-ip>` is
  published once and covers any future turns-sub. `install.sh` succeeds
  in a single run.
- **Two-phase**: `install.sh --bake` (does not start Caddy) →
  read `/var/lib/oxpulse-partner-edge/install.env` →
  publish the exact A record → wait for propagation →
  `systemctl start oxpulse-partner-edge`.

Keep TTL ≤300 s on new records during onboarding. Verify with an
external resolver:
```bash
dig +short <turns-sub>.<partner-domain> @1.1.1.1
```

## 5. Secrets / registration

### 5.1 Partner tokens (`partner-cli`)

Admin CLI for the operator (us):

```bash
# Issue a token — the raw value is shown ONLY once.
docker exec oxpulse-chat partner-cli issue-token --partner rvpn --valid-for 30d

# List active tokens / nodes
docker exec oxpulse-chat partner-cli list-tokens
docker exec oxpulse-chat partner-cli list-nodes

# Revoke
docker exec oxpulse-chat partner-cli revoke-token <token-id>
```

Tokens are stored in `partner_tokens` as `sha256(raw)`. They are
one-shot: on the first successful `/api/partner/register` call the row
is marked `used_at=NOW()`, `used_from_ip=<ip>`, `node_id=<id>`.

### 5.2 POST /api/partner/register

Implementation lives in `crates/server/src/partner_registry/`.

Request:
```json
{
  "partner_id": "rvpn",
  "domain": "call.rvpn.online",
  "token": "ptkn_<hex>",
  "public_ip": "70.34.243.184"
}
```

Response (200):
```json
{
  "node_id": "rvpn-<6-hex>",
  "backend_endpoint": "192.9.243.148:5349",
  "reality_uuid": "fae87c2c-...",
  "reality_public_key": "gV5XA0q...",
  "reality_short_id": "abcd1234",
  "reality_server_name": "cdn.samsung.com",
  "reality_encryption": "mlkem768x25519plus.native.0rtt.<long>",
  "turn_secret": "<hex-64>",
  "turns_subdomain": "api-<6-hex>",
  "config_version": 1
}
```

All five `reality_*` fields plus `backend_endpoint`, `turn_secret` and
`turns_subdomain` are required for a valid install. If any are missing,
`install.sh` must fail before rendering templates. v0.2.0 had incomplete
filtering (see §9).

Error codes (see `partner_registry/error.rs`):
- `token_not_found` / `token_revoked` / `token_already_used` / `token_expired`
- `partner_mismatch` (partner id in request ≠ partner id in token row)
- `reality_not_configured` / `turn_not_configured` /
  `backend_endpoint_not_configured` (operator-side misconfiguration)
- `rate_limited` (per-IP sliding window, see `rate_limit.rs`)

### 5.3 Secret files on the VM

Everything is `chmod 0600 root:root`:
- `/etc/oxpulse-partner-edge/xray-client.json` — UUID + ML-KEM-768 string
- `/etc/oxpulse-partner-edge/coturn.conf` — `static-auth-secret`
- `/var/lib/oxpulse-partner-edge/install.env` — `PARTNER_ID`, `NODE_ID`,
  `TURNS_SUBDOMAIN` (all non-sensitive, no raw secrets)

Rotation: `upgrade.sh` re-hits `/api/partner/register` (idempotent
via the `ON CONFLICT` clause on `(partner_id, domain)`) and re-renders
templates.

## 6. Installation

### 6.1 One-command bootstrap

```bash
curl -fsSL https://github.com/anatolykoptev/oxpulse-chat/releases/latest/download/partner-edge-installer.sh \
  | sudo bash -s -- \
      --domain=<partner-domain> \
      --partner-id=<id> \
      --token=ptkn_<hex>
```

`partner-edge-installer.sh` is the tiny `bootstrap.sh`: it downloads
`partner-edge-<version>.tar.gz` + `SHA256SUMS`, verifies the checksum,
then runs the unpacked `install.sh`.

### 6.2 install.sh stages

1. **preflight** — OS check, free ports 80/443/3478/5349
2. **docker** — installs via `get.docker.com` + `docker-compose-plugin` +
   `dnsutils/bind-utils`
3. **detect IPs** — `PUBLIC_IP`, optionally `PRIVATE_IP` (behind-NAT)
4. **pull images** — `ghcr.io/anatolykoptev/oxpulse-partner-edge-{caddy,xray,coturn}:<tag>`
5. **register** — `POST /api/partner/register`, extracts all secrets
6. **render** — substitutes placeholders → `/etc/oxpulse-partner-edge/*`
7. **start** — `docker compose up -d`
8. **healthcheck** — 120 s poll, 9 checks (see §7)
9. **install systemd** — enables `oxpulse-partner-edge.service` and
   `oxpulse-partner-cert-watch.path`
10. **report** — Node ID, Public IP, diagnostic commands

### 6.3 Modes

| Flag | Effect | When needed |
|------|--------|-------------|
| *(default)* | Full install | Standard |
| `--dry-run` | Renders templates into `$TMPDIR`, prints plan, installs nothing | Sanity-check before a real run |
| `--bake` | Pulls images + installs systemd units without secrets or start | Snapshot workflow for VM clones |
| `--manual-config=<path>` | Reads node-config JSON locally, skips `/api/partner/register` | Emergency mode when the backend is unreachable |
| `--tunnel=vless\|wg\|https` | Transport variant (only `vless` is production-ready) | Experimental channels |
| `--image-version=<tag>` | Pin specific tag | Reproducible deploys |

Env overrides: `OXPULSE_BACKEND_API` (default `https://api.oxpulse.chat`),
`OXPULSE_REPO_RAW`, `OXPULSE_IMAGE_REGISTRY`, `OXPULSE_IMAGE_VERSION`.

## 7. Healthcheck

`oxpulse-partner-edge-healthcheck` performs 9 checks; the full list is
in `deploy/partner-edge/healthcheck.sh`.

| # | Check | Source |
|---|-------|--------|
| 1 | containers up + healthy | `docker compose ps --format json` |
| 2 | `https://<domain>/api/health → 2xx` | external curl (Caddy → xray → backend) |
| 3 | `/api/branding` returns `"partner_id":"<id>"` | same |
| 4 | TCP :443 LISTEN | `ss -ltn` |
| 5 | UDP :3478 LISTEN | `ss -lun` |
| 6 | TCP :5349 LISTEN | `ss -ltn` (coturn TLS listener, §2.3) |
| 7 | xray-client tunnel established | `ss -tn state established` inside the container |
| 8 | coturn `static-auth-secret` matches rendered config | rendered vs live comparison |
| 9 | TURNS-443 handshake: `openssl s_client -connect turns-sub:443 -servername turns-sub` → `Verify return code: 0 (ok)` | external openssl |

The `--local` flag uses an internal probe through the Caddy admin API
for post-install checks before DNS is live.

## 8. Upgrade / Drain / Rollback

### 8.1 Upgrade (`oxpulse-partner-edge-upgrade`)

1. `dig +short <turns-sub>.<partner-domain>` — DNS preflight; aborts if
   the record does not resolve to this node's public IP.
2. `docker pull ghcr.io/...:<new-tag>` for all three images.
3. `hydrate.sh --reseed` re-calls `/api/partner/register`
   (`ON CONFLICT` preserves `node_id` and `turns_subdomain`) and
   re-renders templates.
4. `docker compose up -d --no-deps --force-recreate` per service.
5. Healthcheck — on FAIL, `docker compose` rolls back to the prior tag.

### 8.2 Maintenance drain

**Soft drain**: the operator bumps the entry priority to `999` in
`TURN_SERVERS` and reloads — the pool stops selecting this node for new
calls. After 10 minutes (p99 call duration) → `systemctl stop
oxpulse-partner-edge.service`.

**Hard drain**: the operator removes the entry entirely — in-progress
calls on this relay will drop when the partner stops coturn.
Coordinated over the escalation channel.

### 8.3 Rollback

Three layers:
1. Image version — `install.sh --image-version=<prev>` + `systemctl
   restart`.
2. Partner-edge bundle — run `install.sh` from the
   `partner-edge-v<prev>/partner-edge-installer.sh` pinned URL.
3. Remove the partner from `TURN_SERVERS` on the operator side. This
   rollback does not touch the partner VM; it is instant and safe.

## 9. Bug history (v0.2.0 → v0.2.2)

A textbook chain of seven independent regressions — each safe in
isolation, collectively fatal:

1. **`BACKEND_API` default = `https://api.oxpulse.chat`**, but that
   vhost was never set up in the operator's Caddy. TLS handshake to
   192.9.243.148:443 with SNI=api.oxpulse.chat failed with an internal
   error. Fix: added the `api.oxpulse.chat` vhost (`/api/*` →
   `localhost:8907`, 404 otherwise) on the operator side; kept the
   default in install.sh pointing at `api.oxpulse.chat`.
2. **`json_get reality_encryption`** was missing in install.sh. The
   backend returned the field but the client dropped it. The template
   `xray-client.json.tpl` kept the literal `{{REALITY_ENCRYPTION}}`
   placeholder, which fell back to `encryption: "none"`. The TLS
   handshake to 192.9.243.148:5349 succeeded (Samsung cert), but
   payload decryption on the server side failed → VLESS silently
   drops packets → every `/api/*` from the partner times out with no
   client logs.
3. **`json_get turns_subdomain`** was missing. Templates kept the
   env default `turns` instead of the backend-assigned `api-<hex>`.
   Caddy tried to issue a cert for the literal placeholder, and the
   cert-mount path in coturn.conf pointed at a non-existent directory.
4. **`render()` sed** did not substitute `{{REALITY_ENCRYPTION}}` +
   `{{TURNS_SUBDOMAIN}}`. `hydrate.sh` already substituted both —
   `install.sh` was out of sync with the templates.
5. **Volume mount `caddy-data:/data/caddy:ro`** in coturn. Caddy
   inside the container sets `$XDG_DATA_HOME=/data` and stores certs
   under `caddy/certificates/...`. With that mount, coturn sees
   `/data/caddy/caddy/certificates/...`, while `coturn.conf.tpl` reads
   from `/data/caddy/certificates/...`. The cert "did not exist" → coturn
   silently skipped the TLS listener and :5349 was not listening.
   Fix: mount `caddy-data:/data:ro`.
6. **Healthcheck step 9** — `timeout 10 openssl | grep -q` with
   `pipefail`. After a successful handshake openssl blocks on the
   half-closed socket until `timeout` fires with exit 124; `pipefail`
   propagates this even though grep already matched. Fix: capture
   output to a tempfile, grep independently.

7. **coturn startup race with ACME** (found during v0.2.1 clean-install
   retest). `docker compose up` brings coturn up before Caddy has
   obtained the TURNS subdomain cert. coturn evaluates its cert/pkey
   paths once at startup and silently disables the TLS listener if
   they are missing. `cert-watch.path` only fires SIGUSR2 on renewal,
   so the listener would stay disabled until the first Let's Encrypt
   renewal months later. Fix in v0.2.2: `install.sh` polls the cert
   path in the caddy-data volume for up to 180 s and, once the cert
   appears, issues `docker compose restart coturn`.

Bug 5 was the cruelest: everything looked OK (containers healthy,
certs issued), yet the TLS listener was silently absent. Bug 7 was
a close second — passed all unit and dry-run checks, only surfaced on
the very first clean-VM install with real ACME timing.

Diagnosis took seven iterations; v0.2.1 closed six bugs and v0.2.2
closed the seventh.

## 10. Relationship to piter-server

piter (81.90.183.114, Hostiman, SPB) — the Russian node of the krolik
infrastructure — uses an **identical** VLESS+ML-KEM+Reality+XHTTP
channel to 192.9.243.148:5349 to bypass ТСПУ. Partner-edge is
essentially the younger sibling of piter:

| Aspect | piter | partner-edge |
|--------|-------|--------------|
| Transport | VLESS+ML-KEM+Reality+XHTTP → krolik xray-reality :5349 | **identical** |
| Backend endpoint | 192.9.243.148:5349 | **same** |
| Reality creds | `fae87c2c-de37-...` + ML-KEM shared secret | **same** |
| Internal forward | WireGuard (plus multi-channel) | TCP signaling (single channel) |
| Split routing | Yes (RU direct, foreign via VPN) | No (everything through the tunnel) |
| Auto-failover | Watchdog + 3 channels (CH1 Reality, CH2 AWG, CH3 wstunnel) | No |
| TURNS-on-443 | No (coturn on :3478 only) | Yes |

The `REALITY_ENCRYPTION` value is the same
`mlkem768x25519plus.native.0rtt.<seed>` string; the shared seed lives
in `/root/.vless-enc-prod` on krolik (mode 0600). Rotation procedure:
`~/deploy/krolik-server/oxpulse-reality/README.md`.

**Roadmap — multi-channel for partner-edge.** Port the piter pattern:
- CH2 AmneziaWG (fallback, UDP 43891) — activates if CH1 is identified
  by Reality-specific DPI signals
- CH3 wstunnel WSS (emergency, :443/HTTPS) — last resort when DPI
  blocks Reality altogether

This is `onboarding.md §3.1`-level work, not MVP-blocking, but it
significantly improves resilience against ТСПУ escalations in 2026+.

## 11. Links

- Partner DevOps runbook: [`onboarding.md`](onboarding.md)
- Installer + templates: `deploy/partner-edge/`
- Backend registration: `crates/server/src/partner_registry/`
- Admin CLI: `crates/partner-cli/`
- piter-side ТСПУ architecture: `~/src/piter-server/docs/`
- Operator Reality endpoint: `~/deploy/krolik-server/oxpulse-reality/README.md`
- Incident runbook (TURN down in production): `docs/runbooks/turn-outage.md`
  (Task 3.5, pending)

## 12. Document changelog

- **2026-04-18** — first version after the v0.2.1 fix cycle (rvpn
  launch). Revised the same day to add bug #7 (coturn/ACME startup
  race) and the v0.2.2 fix.
