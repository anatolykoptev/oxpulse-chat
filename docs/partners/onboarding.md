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

## 4. Step 1 — Install coturn

**Debian / Ubuntu:**

```bash
sudo apt-get update
sudo apt-get install -y coturn
sudo sed -i 's/^#TURNSERVER_ENABLED=1/TURNSERVER_ENABLED=1/' /etc/default/coturn
```

**RHEL / Rocky / Alma:**

```bash
sudo dnf install -y epel-release
sudo dnf install -y coturn
```

## 5. Step 2 — /etc/turnserver.conf

Replace the shipped config with the block below. Substitute
`<REPLACE_WITH_SHARED_SECRET>`, `<PUBLIC_IPV4>`, and `<PRIVATE_IPV4>`
(omit the `/` and private half on bare-metal hosts without NAT).

```conf
listening-port=3478
fingerprint
lt-cred-mech
use-auth-secret
static-auth-secret=<REPLACE_WITH_SHARED_SECRET>
realm=oxpulse.chat
total-quota=200
stale-nonce=600
no-tls
no-dtls
no-tcp-relay
denied-peer-ip=10.0.0.0-10.255.255.255
denied-peer-ip=172.16.0.0-172.31.255.255
denied-peer-ip=192.168.0.0-192.168.255.255
min-port=49152
max-port=65535
external-ip=<PUBLIC_IPV4>/<PRIVATE_IPV4>
log-file=/var/log/turnserver/turn.log
```

Key lines:

- `use-auth-secret` + `static-auth-secret` — enables coturn's HMAC REST
  API. The signaling server hands clients a short-lived
  `{unix_expiry}:chat-user` username and the matching
  `base64(HMAC-SHA1(secret, username))` credential (see
  `crates/turn/src/lib.rs`). The secret on both sides MUST match
  byte-for-byte.
- `denied-peer-ip` — blocks relay into RFC1918 networks. **Critical.**
  Without these lines the relay becomes an SSRF vector into the
  partner's internal infra.
- `external-ip` — fixes NAT reflection on cloud VMs where the public
  IP is one-to-one mapped to a private NIC address. On bare metal
  with a directly attached public IP, use `external-ip=<PUBLIC_IPV4>`
  without the slash.
- `no-tls` / `no-dtls` — TLS termination for the app happens at Caddy
  on the signaling host; TURN/TLS (port 5349) is deliberately
  out-of-scope for v1. If the partner wants TURNS later, open a
  ticket — will require an additional cert + firewall rule.
- `min-port` / `max-port` — must match the firewall rule in step 3.

## 6. Step 3 — Firewall

**ufw:**

```bash
sudo ufw allow 3478/udp
sudo ufw allow 3479/udp
sudo ufw allow 3478/tcp
sudo ufw allow 49152:65535/udp
```

**nftables** (append to the `inet filter input` chain):

```nft
udp dport 3478 accept
udp dport 3479 accept
tcp dport 3478 accept
udp dport 49152-65535 accept
```

**iptables** (legacy):

```bash
sudo iptables -A INPUT -p udp --dport 3478 -j ACCEPT
sudo iptables -A INPUT -p udp --dport 3479 -j ACCEPT
sudo iptables -A INPUT -p tcp --dport 3478 -j ACCEPT
sudo iptables -A INPUT -p udp --dport 49152:65535 -j ACCEPT
```

## 7. Step 4 — Start and verify

```bash
sudo mkdir -p /var/log/turnserver
sudo systemctl enable --now coturn
sudo systemctl status coturn
```

Verify from any host with a public route:

```bash
# STUN binding probe (should print Mapped Address = your public IP)
turnutils_stunclient <public_ip>

# Raw TCP reachability
nc -zv <public_ip> 3478
```

If `turnutils_stunclient` times out, the firewall, cloud security
group, or `external-ip` setting is wrong. Fix before continuing —
the signaling-side probe (step 6) will fail otherwise.

## 8. Step 5 — Register with the operator

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

## 9. Step 6 — Verify the node is live

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

## 10. Draining a node for maintenance

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
priority and verify per step 6.

## 11. Incident response

For an active TURN outage affecting production traffic, see
`docs/runbooks/turn-outage.md` (ships in **Task 3.5**). Until then,
page the oxpulse-chat on-call via the partner escalation channel.

## 12. Appendix — credential flow

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
