# TURN Operator Onboarding

Runbook for partner DevOps engineers provisioning a coturn relay for
oxpulse.chat. Target: adding a new node in minutes, no code changes,
no container rebuild.

## 1. Purpose

oxpulse.chat is a peer-to-peer WebRTC video calling service. When two
peers cannot reach each other directly (symmetric NAT, strict
firewalls, mobile carrier CGNAT), their media is relayed through a
TURN server. We need multiple TURN relays geographically close to end
users — that's what the partner hosts. The partner operates the
coturn boxes; oxpulse-chat operators only push a config line that tells
the signaling server to advertise the new relay to clients.

## 2. Architecture

```
  Browser ──WSS──▶ Caddy ──▶ oxpulse-chat (Rust/Axum)
     │                            │
     │  POST /api/turn-credentials │
     │  ◀──── {ice_servers: [...]} │  (HMAC-signed per request)
     │
     └──UDP 3478──▶ coturn (partner) ◀──UDP 3478── Browser (peer B)
                       │
                       └── relays media peer-to-peer
```

Signaling never proxies media. The Rust server only issues short-lived
HMAC credentials; media flows browser ↔ coturn ↔ browser directly.

## 3. Prerequisites

- Linux VM (Debian 12 / Ubuntu 22.04 / RHEL 9) with public IPv4 (IPv6 optional)
- Reachable from the public internet:
  - UDP 3478 — STUN + TURN
  - UDP 3479 — TURN alternate
  - TCP 3478 — TURN/TCP fallback
  - UDP 49152–65535 — relay port range
- Root or sudo access
- The shared `TURN_SECRET` from the oxpulse-chat operator. Delivered
  out of band (signal, 1Password share, age-encrypted file). **Never
  committed to git, never pasted in chat history.** Rotated on a
  schedule — operator notifies the partner before each rotation.

### 3.1 ASN selection for Russian ТСПУ environments

TURNS-on-:443 relies on sustained long-lived TCP/TLS sessions —
typically 10–60 minutes per active call. Under current Russian ТСПУ
behaviour (Q2 2026), certain hosting ranges are subject to a ~16 KB
per-connection cap on TLS traffic. When a TURNS session hits that cap
the connection is terminated by the middlebox, dropping the call within
seconds of start. Non-ТСПУ users (EU, US, CIS outside Russia) are not
affected; the cap is applied only at the ТСПУ enforcement points inside
Russian ISPs.

**Flagged ASN ranges — avoid for Russian-facing deployments:**

- Cloudflare (all ranges)
- Fastly (all ranges)
- Hetzner (most /24s in AS24940)
- DigitalOcean (most /24s in AS14061)
- OVH (most /24s in AS16276)

These providers are individually listed on the RKN "foreign hosting"
registry or have been observed to trigger ТСПУ throttling in direct
testing. The list is current as of Q2 2026 and may change as RKN
updates its registry.

**Recommended choices (in order of preference):**

1. Smaller European VPS providers not listed on the RKN "foreign
   hosting" registry. Examples: Serverius (NL), Worldstream (NL),
   combahton (DE), RETN-peered providers.
2. Russian-domestic hosting — acceptable if the partner's compliance
   posture permits (data localisation rules apply for PII). Examples:
   Selectel, Reg.ru, TimeWeb.
3. A dedicated IP with no history of Tor exit node, open proxy, or CDN
   fronting, on any provider not in the flagged list above.

**Provider-specific notes:**

*Hetzner.* `install.sh` completes normally on Hetzner infrastructure
and the coturn relay accepts connections. Russian ТСПУ users experience
call drops within seconds because the ~16 KB TLS cap terminates the
underlying TURNS session. EU, US, and non-RU CIS users are unaffected
— call quality is normal from those geographies.

*DigitalOcean.* Same behaviour as Hetzner: install and basic
connectivity succeed; Russian users hit the per-connection TLS cap and
the relay is unreliable for that traffic segment. Non-ТСПУ deployments
are unaffected.

*AWS (EC2 direct, not CloudFront).* AWS EC2 addresses are not currently
on the RKN registry at the AS16509 level. If the partner uses EC2
directly — not via CloudFront — reliability for Russian users is
generally acceptable. Verify the public IP does not fall in a
CloudFront range before relying on it for ТСПУ traffic.

Partner-edge installs on flagged ASNs still function for non-ТСПУ
traffic. This guidance is about user-geography reliability, not a hard
install gate.

### 3.2 DNS requirements (v0.2.0+)

v0.2.0 introduces a second A-record requirement alongside the existing
primary record. Both records must be in place **before** running
`install.sh` or `hydrate.sh`.

**Required DNS records:**

| Record | Value | Purpose |
|--------|-------|---------|
| `<your-domain>` | `<public-ip>` | Primary vhost; Caddy ACME HTTP-01 |
| `<turns-subdomain>.<your-domain>` | `<public-ip>` | TURNS cert; Caddy ACME HTTP-01 |

`<turns-subdomain>` is backend-assigned during partner registration and
returned in the `turns_subdomain` field of the register response. The
format is `api-<6-hex>` (example: `api-a3f8c1`). Both records point to
the same `<public-ip>`.

**Why this matters.** `install.sh` and `hydrate.sh` run a DNS preflight
that resolves `<turns-subdomain>.<your-domain>` before touching
anything else. If the record is missing or mismatched, both scripts
abort with a clear error and make no changes. Caddy cannot issue the
TURNS TLS certificate via ACME HTTP-01 without the A-record resolving
to the server's public IP.

**Propagation.** Set TTL ≤300 seconds on both records during
onboarding. Verify from an external resolver before proceeding:

```bash
dig +short <turns-subdomain>.<your-domain> @1.1.1.1
# must return <public-ip>
```

**Upgrading from v0.1.** `upgrade.sh` runs the same DNS preflight on
startup and aborts before any mutation if the turns-subdomain record is
missing. Add the record and re-run `upgrade.sh`. The script is
co-located with `install.sh` in `deploy/partner-edge/`.

## 4. One-command install

On a freshly provisioned VM (Debian 12, Ubuntu 22.04/24.04, RHEL/Alma/Rocky/CentOS Stream 9),
as root:

```bash
curl -fsSL https://github.com/anatolykoptev/oxpulse-chat/releases/latest/download/partner-edge-installer.sh \
  | sudo bash -s -- \
      --domain=<your-domain> \
      --partner-id=<your-id> \
      --token=<bootstrap-token>
```

Pin a specific version (recommended for reproducible deploys):

```bash
VERSION=0.2.0 curl -fsSL \
  https://github.com/anatolykoptev/oxpulse-chat/releases/download/partner-edge-v0.2.0/partner-edge-installer.sh \
  | sudo bash -s -- --domain=<your-domain> --partner-id=<your-id> --token=<bootstrap-token>
```

The installer (bootstrap → full `install.sh` from the release tarball):
- downloads the release bundle (`partner-edge-<version>.tar.gz`) + `SHA256SUMS`,
  verifies the checksum, and extracts to a temporary directory;
- detects the distro, installs Docker via `get.docker.com`, plus
  `docker-compose-plugin` + `bind-utils` (RHEL) or `docker-compose-plugin` +
  `dnsutils` (Debian);
- registers the node via `POST /api/partner/register` using the bootstrap token,
  receives the backend-assigned `turns_subdomain` (format `api-<6-hex>`);
- renders the Caddyfile, coturn config, xray-client config, and
  `docker-compose.yml` from templates;
- issues ACME certs for both `<your-domain>` and
  `<turns_subdomain>.<your-domain>` (see §3.2 — both DNS A-records must
  already point at this VM's public IP before install);
- starts three containers (Caddy + xray-client + coturn) and enables the
  systemd `oxpulse-partner-edge-hydrate.service` for idempotent re-runs.

### Legacy simple TURN-only relay

If you only need a plain coturn relay (no TURNS-on-:443, no co-brand SPA):

```bash
curl -fsSL https://github.com/anatolykoptev/oxpulse-chat/releases/latest/download/turn-node-installer.sh \
  | TURN_SECRET='<shared-secret>' REGION='<region>' bash
```

This is the `deploy/turn-node/` installer. It predates the partner-edge
architecture and is still supported for partners who don't need TURNS-443.

## 5. Verify

```bash
/usr/local/sbin/oxpulse-turn-healthcheck
```

The last line of output is the registration string to send the operator.

## 6. Cloning to other regions

Snapshot the VM *after* `install.sh` succeeds and *before* any partner-specific
state accumulates. Clone, then on each clone:

```bash
vi /etc/default/oxpulse-turn   # update REGION; blank PUBLIC_IPV4 so it autodetects
systemctl restart coturn
/usr/local/sbin/oxpulse-turn-healthcheck
```

Send the new registration line to the operator.

## 7. Register with the operator

Send the oxpulse-chat operator one line in this exact format:

```
<region>:<priority>:turn:<public_host>:3478?transport=udp
```

- `region` — **operator-assigned short tag**, not freeform. Current
  allocations: `ru-msk`, `ru-spb`, `de-fra`, `sg-sin`, `us-sfo`. Ask
  for a new tag if your PoP isn't on the list.
- `priority` — non-negative integer, lower is preferred. Use `0` for
  the primary node in a region, `10`/`20`/... for in-region failovers,
  `100`+ for cross-region fallback.
- `public_host` — hostname or IPv4 that end-user browsers can resolve
  and reach. Prefer a hostname so the node can be moved without
  partner-side config changes.

Example:

```
ru-msk:0:turn:msk1.turn.example.net:3478?transport=udp
```

The operator applies the entry on the signaling host. There are two
paths depending on whether hot-reload has shipped:

**Today (pre-Task-2.6):**
1. Operator appends the entry to the `TURN_SERVERS` env var in
   `~/deploy/krolik-server/.env` (comma-separated).
2. Operator runs
   `docker compose up -d --force-recreate oxpulse-chat`.
3. Brief drop in accept-new-call capacity while the container cycles
   (~5s). Active calls are not affected — media flows through coturn,
   not the signaling process.

**After Task 2.6 ships (SIGHUP hot-reload, target: Q2 2026):**
1. Operator appends the entry to `/etc/oxpulse-chat/turn_servers.toml`
   (planned file, delivered via docker volume + systemd drop-in).
2. Operator runs `docker kill -s HUP oxpulse-chat`.
3. No container restart, no call interruption.

The config format parsed by
`crates/server/src/config.rs::parse_turn_servers` is stable across
both paths — only the delivery mechanism differs.

## 8. Verify the node is live

**Operator side (logs):**

```bash
docker logs oxpulse-chat 2>&1 | grep turn_server_up
# expect: turn_server_up {region="ru-msk", url="turn:msk1..."}
```

The `turn_server_up` log line is emitted by the probe loop landing in
**Task 2.3 (STUN binding-request health probe)**. If the line is
absent after a full probe interval (`TURN_PROBE_INTERVAL_SECS`,
default 30s), the probe failed — walk the checklist:
- Firewall rule missing on UDP 3478?
- `static-auth-secret` mismatch?
- `external-ip` wrong (mapped address ≠ configured external IP)?
- Cloud security group blocking UDP?

**Client side (browser devtools):**

1. Open https://oxpulse.chat in Chrome.
2. DevTools → Network → filter XHR.
3. Start a call.
4. Inspect `POST /api/turn-credentials`. The new URL should appear in
   `ice_servers[*].urls` within one probe interval after registration.
   Note: today the handler returns the full `TURN_URLS` list
   unconditionally; region-filtered output is wired in **Task 2.4**.

**Metrics (planned — Task 3.1):**

```bash
curl -H "X-Internal-Token: $OPERATOR_TOKEN" \
  http://127.0.0.1:8907/metrics | grep turn_servers_healthy
# expect: turn_servers_healthy{region="ru-msk"} 1
```

The `/metrics` endpoint and the `turn_servers_healthy` gauge ship in
**Task 3.1** (Prometheus export). Until 3.1 lands, use the log line
check above as the source of truth.

## 9. Draining a node for maintenance

Two options, both operator-driven:

**Soft drain (preferred):**
1. Operator updates the entry priority to `999` and reloads.
2. Pool stops selecting the node for new calls but keeps it in rotation
   as a last-resort fallback.
3. Wait 10 minutes (assumes p99 call duration < 10min).
4. `sudo systemctl stop coturn` on the partner VM.

**Hard drain:**
1. Operator removes the entry entirely and reloads.
2. New calls stop routing to the node immediately.
3. In-progress calls using this relay will drop when the partner stops
   coturn — coordinate the shutdown window with the operator.

Maintenance window complete → re-add the entry with the original
priority and verify per §8.

## 10. Incident response

For an active TURN outage affecting production traffic, see
`docs/runbooks/turn-outage.md` (ships in **Task 3.5**). Until then,
page the oxpulse-chat on-call via the partner escalation channel.

## 11. Appendix — credential flow

The signaling server does not speak to coturn at all. Instead, both
sides share `static-auth-secret`, and the signaling server mints
short-lived credentials the client sends directly to coturn:

```
username   = "{unix_expiry_seconds}:chat-user"
credential = base64(HMAC-SHA1(static_auth_secret, username))
```

Reference: `crates/turn/src/lib.rs::generate_credentials`. TTL is
hardcoded to 86400 seconds (24h) in
`crates/server/src/router.rs::turn_credentials` — not exposed as an
env var today. Clients request new credentials per call, so the TTL
is an upper bound, not a session length. The `user_id` segment is
hardcoded to `chat-user`; coturn does not enforce per-user quotas
against it, so this is cosmetic.

Because the credential is a pure HMAC of a timestamp + the shared
secret, any drift between the signaling host clock and the coturn
host clock > TTL will cause auth failures. Run NTP on both sides.
