# oxpulse-chat Partner Edge Bundle

One-command installer for a production-grade **co-brand mirror node** that
participates in the oxpulse.chat network. Tested on Debian 12, Ubuntu
22.04 / 24.04, AlmaLinux 9, Rocky Linux 9.

The bundle runs three containers on the partner's VPS:

- **Caddy** — TLS termination (ACME via Let's Encrypt), SPA CDN, reverse
  proxy of `/api/*` + `/ws/*` through the tunnel.
- **xray-client** — VLESS + Reality + XHTTP tunnel to the main backend
  (`krolik-server:5349`). Exposes only `:3080` inside the docker network.
- **coturn** — TURN/STUN relay with HMAC auth (`:3478/udp+tcp`,
  `:5349/tcp` for TURNS). Runs in host network mode.

Relationship to [`deploy/turn-node/`](../turn-node): that package is a
bare TURN relay only. `partner-edge` is the full app-mirror bundle.

## Prerequisites

- Debian 12+, Ubuntu 22.04+, AlmaLinux 9+, or Rocky 9+ (`systemd` + `bash`)
- 1 vCPU, 1 GB RAM, 20 GB disk (minimum)
- Public IPv4 reachable from the internet
- A DNS A record for your partner domain pointing at the VPS's public IP
- Ports free: **80, 443/tcp+udp, 3478/tcp+udp, 3479/udp, 5349/tcp,
  49152-65535/udp**

## Quickstart

```bash
# On a freshly provisioned VM, as root:
curl -fsSL https://github.com/anatolykoptev/oxpulse-chat/releases/latest/download/partner-edge-installer.sh \
  -o install.sh
sudo bash install.sh \
  --domain=call.rvpn.online \
  --partner-id=rvpn \
  --manual-config=./node-config.json
```

The installer runs 10 steps (preflight → docker → IP detect → fetch config
→ render templates → pull → start → healthcheck → systemd → report) and
prints a banner on success.

### CLI flags

| Flag                      | Default       | Notes |
|---------------------------|---------------|-------|
| `--domain=<fqdn>`         | required      | Partner edge domain. Must already resolve. |
| `--partner-id=<id>`       | required      | Short tag; must match backend `config/partners/<id>.json`. |
| `--token=<ptkn_...>`      | —             | Registration token (calls `/api/partner/register`). |
| `--manual-config=<path>`  | —             | Alternative: read node config from a local JSON file. |
| `--tunnel=vless\|wg\|https` | `vless`     | Backend tunnel kind (only `vless` implemented in v0.1.0). |
| `--image-version=<tag>`   | `latest`      | Pin images to a specific published tag. |
| `--dry-run`               | off           | Render templates + print plan, skip docker/systemd. |

### Manual config fallback (v0.1.0)

The `/api/partner/register` endpoint is **not yet implemented** (Task 4 of
the partner-mirror plan). Until it lands, the partner operator receives a
small JSON file from OxPulse ops out-of-band and passes it to
`--manual-config`.

```json
{
  "node_id": "rvpn-call1",
  "backend_endpoint": "krolik.oxpulse.chat:5349",
  "turn_secret": "<shared-hmac-secret>",
  "reality_uuid": "<uuid-v4>",
  "reality_public_key": "<base64-reality-pubkey>",
  "reality_short_id": "<8-hex-chars>",
  "reality_server_name": "www.samsung.com"
}
```

Keep this file `chmod 0600` — it contains the fleet-wide TURN secret.

## What the installer lays down

| Path                                             | Purpose                      |
|--------------------------------------------------|------------------------------|
| `/etc/oxpulse-partner-edge/docker-compose.yml`   | Rendered compose file        |
| `/etc/oxpulse-partner-edge/Caddyfile`            | Rendered Caddy config        |
| `/etc/oxpulse-partner-edge/xray-client.json`     | Rendered xray-client config  |
| `/etc/oxpulse-partner-edge/coturn.conf`          | Rendered turnserver.conf     |
| `/var/lib/oxpulse-partner-edge/install.env`      | Partner/version state        |
| `/usr/local/sbin/oxpulse-partner-edge-healthcheck` | 8-point verification        |
| `/usr/local/sbin/oxpulse-partner-edge-upgrade`   | Upgrade / rollback tool      |
| `/etc/systemd/system/oxpulse-partner-edge.service` | Systemd unit               |

## Verification

```bash
sudo oxpulse-partner-edge-healthcheck          # full external check
sudo oxpulse-partner-edge-healthcheck --local  # docker-network only (pre-DNS)
```

The 8-point healthcheck covers:

1. All three containers running
2. `/api/health` returns 2xx
3. `/api/branding` reports the expected `partner_id` (needs Task 3 backend)
4. TCP :443 listening
5. UDP :3478 listening
6. TCP :5349 listening
7. xray-client has an ESTABLISHED outbound to the backend
8. Coturn process loaded the rendered shared secret

**Expected-fail until backend lands:** checks 2 and 3 return FAIL until
`/api/branding` and the host-based branding resolver (Task 3) are deployed.
Use `--local` during that window.

## Upgrade / rollback

```bash
# Pull :latest and recreate (auto-rolls back on healthcheck failure):
sudo oxpulse-partner-edge-upgrade

# Pin to a specific tag:
sudo oxpulse-partner-edge-upgrade v0.2.0

# See whether an upgrade is pending without applying:
sudo oxpulse-partner-edge-upgrade --check

# Explicit rollback to the previous compose file:
sudo oxpulse-partner-edge-upgrade --rollback
```

The upgrade tool keeps the previous `docker-compose.yml` at
`/var/lib/oxpulse-partner-edge/docker-compose.yml.prev`, so rollback works
even if the new images are removed from GHCR.

## Troubleshooting

**Port already in use at install time**
  Some other service (nginx, apache) is on 80/443. Stop it or uninstall
  before running `install.sh`.

**Caddy can't get a TLS cert**
  Verify DNS A record for your domain resolves to this host's public IP.
  Caddy logs: `docker compose -f /etc/oxpulse-partner-edge/docker-compose.yml logs caddy`.
  If you fronted the domain with Cloudflare, set DNS-only (grey cloud) —
  Caddy needs direct HTTP-01 to the edge.

**xray-client keeps reconnecting**
  Reality credentials don't match the backend. Check `reality_public_key`
  and `reality_short_id` in your manual-config JSON against what OxPulse ops
  provisioned for your partner-id.

**TURN doesn't work, signaling does**
  UDP 3478 + 49152-65535 are blocked upstream. The VPS provider's firewall,
  or a local `ufw`/`firewalld`, is filtering them. TURNS on :5349 works as
  a TCP fallback from restrictive client networks.

**Installer says "backend_endpoint missing"**
  Your manual-config JSON is missing a required field. Diff against the
  schema above.

## How branding is applied

The partner-edge bundle is **brand-agnostic**. All branding lives on the
backend at `config/partners/<partner_id>.json` and is injected into the
SvelteKit SPA at response time.

Request flow:

```
browser → https://call.rvpn.online/          (Caddy)
      → xray-client:3080                     (VLESS+Reality to backend)
      → krolik-server:5349                   (xray-reality)
      → oxpulse-chat:8907                    (Rust/Axum)
```

Caddy adds `X-Forwarded-Host: call.rvpn.online` so the backend's branding
resolver picks the right config. This means a single installer binary
handles any partner — no image rebuild per partner.

## Uninstall

```bash
sudo systemctl disable --now oxpulse-partner-edge
sudo docker compose -f /etc/oxpulse-partner-edge/docker-compose.yml down -v
sudo rm -rf /etc/oxpulse-partner-edge /var/lib/oxpulse-partner-edge \
            /etc/systemd/system/oxpulse-partner-edge.service \
            /usr/local/sbin/oxpulse-partner-edge-*
sudo systemctl daemon-reload
```

## Support

- GitHub issues: https://github.com/anatolykoptev/oxpulse-chat/issues
- Operator runbook: [`docs/partners/onboarding.md`](../../docs/partners/onboarding.md)
- Design spec: `docs/superpowers/specs/2026-04-17-oxpulse-partner-mirror-design.md`

## Security notes

- The TURN shared secret is fleet-wide — rotating it requires coordinated
  redeploy of all nodes. Ask ops for the current value out-of-band.
- Reality credentials are per-partner. Compromising one partner node does
  not expose the backend or other partners.
- The Caddy container has no write access to `/etc/caddy/Caddyfile` (it's
  mounted `ro`). Cert storage is in an isolated docker volume.
- Coturn runs in **host network mode** because TURN must see the real
  public IP to advertise relay candidates. The `denied-peer-ip` list in
  `coturn.conf` blocks SSRF into RFC1918 / CGNAT / link-local ranges.
