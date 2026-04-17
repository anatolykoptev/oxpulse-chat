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

## 4. One-command install

On a freshly provisioned VM (Debian 12, Ubuntu 22.04/24.04, RHEL/Alma/Rocky/CentOS Stream 9),
as root:

```bash
curl -fsSL https://raw.githubusercontent.com/anatolykoptev/oxpulse-chat/main/deploy/turn-node/install.sh \
  | TURN_SECRET='<shared-secret>' \
    REGION='<operator-assigned>' \
    PUBLIC_HOST='<dns-name-you-registered>' \
    bash
```

The installer:
- detects the distro, installs `coturn` (+ `coturn-utils` on RHEL), `chrony`, and a firewall tool;
- lays down `/etc/default/oxpulse-turn` (the one file you edit later — see
  `deploy/turn-node/README.md` for the full variable list);
- installs a systemd `ExecStartPre` oneshot that renders `/etc/turnserver.conf`
  from a repo-controlled template on every restart;
- opens the firewall (UDP 3478/3479, TCP 3478, UDP 49152-65535) via
  `firewalld` or `ufw`;
- enables `chronyd` — TURN credential TTL is HMAC-over-timestamp, so clock
  drift breaks auth.

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
