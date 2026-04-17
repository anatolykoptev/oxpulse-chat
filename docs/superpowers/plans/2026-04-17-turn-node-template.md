# TURN-Node Template (one-command partner bootstrap) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the manual step-by-step `docs/partners/onboarding.md` procedure with a single idempotent installer that turns any fresh Debian/Ubuntu or RHEL-family VM into a production-grade oxpulse-chat TURN relay. The first deployment target is the partner's test VM `call.rvpn.online` (70.34.243.184, CentOS Stream 9). Once validated there, the same artifact is cloned/snapshotted across the partner's regional fleet; per-node values live in a single editable env-file so a snapshot → clone → edit-env → restart flow works without image rebakes.

**Architecture:**
- Native coturn via distro package manager + systemd (not Docker — UDP relay 49152-65535 through container netns is a known source of jitter; LiveKit, BigBlueButton and Matrix all ship native). One bash installer in `deploy/turn-node/install.sh` autodetects distro (`/etc/os-release` → `ID` + `ID_LIKE`), installs `coturn` + `chrony` + firewall tool, lays down a systemd `ExecStartPre` oneshot that renders `/etc/turnserver.conf` from a template using values in `/etc/default/oxpulse-turn`, and opens firewall ports (UDP 3478/3479, TCP 3478, UDP 49152-65535).
- `TURN_SECRET` is **shared across the entire partner fleet** (matches operator-side `TURN_SECRET` in `~/deploy/krolik-server/.env`); `PUBLIC_IPV4` and optional `PRIVATE_IPV4` are **per-node** (auto-detected with override). Template (`turnserver.conf.tmpl`) is checked into the repo; rendered `/etc/turnserver.conf` is generated at service start, so the same VM snapshot can be cloned and re-keyed by editing `/etc/default/oxpulse-turn` — no rebuild.
- Signaling server (`oxpulse-chat`) stays centralized. After each node is provisioned, the installer prints the exact registration line (`region:priority:turn:host:port?transport=udp`) that the operator appends to `TURN_SERVERS` in `~/deploy/krolik-server/.env`. Nothing on the partner node talks to our signaling host; the coturn HMAC loop is the entire contract.

**Tech Stack:** Bash (POSIX-safe, shellcheck-clean), coturn 4.x (EPEL on RHEL / default on Debian), systemd drop-ins + oneshot units, chrony (NTP), firewalld (RHEL) / ufw (Debian) / nftables (fallback), `envsubst` (gettext) for template rendering, `turnutils_stunclient` for post-install smoke test.

**Release + upgrade story (addressed by Tasks 8-9):**
- **Tag namespace:** `turn-node-vX.Y.Z` (release-please v4 canonical form, dash separator — independent of core signaling `vX.Y.Z`). SemVer with config-file semantics: MAJOR = env-var rename or template-incompatible change; MINOR = new optional knobs, backwards-compatible; PATCH = template/hardening fixes with no behavior change.
- **Release artifact:** one `turn-node-<version>.tar.gz` of `deploy/turn-node/**` + a `SHA256SUMS` file, both attached to the GitHub Release by a workflow that fires on `turn-node-v*` tag push.
- **Install-time version pin:** `install.sh` accepts `TURN_NODE_REF=<tag-or-branch>` (default `main` for the initial partner test; stable deployments use an explicit tag).
- **In-place upgrades:** `oxpulse-turn-upgrade` on the partner host pulls the **latest** release (or a pinned one) from GitHub Releases API, verifies SHA256, backs up the current artifact tree, runs `install.sh --files-only` (env file never touched), restarts coturn, rolls back on failure.
- **Opt-in automation:** a dormant `oxpulse-turn-upgrade.timer` ships with the installer (disabled by default). Partners enable it explicitly — nobody auto-applies config changes during call hours without consent.

**Scope boundaries (deliberately NOT in this plan):**
- Changes to `crates/server/**` — the signaling server is unchanged. `parse_turn_servers` and `/api/turn-credentials` already shipped.
- TURNS (TLS on 5349). Phase-2 runbook intentionally deferred this; we follow.
- Per-node `TURN_SECRET` rotation automation. Manual rotation stays operator-driven.
- Hot-reload of `TURN_SERVERS` on the signaling side — tracked in the existing Phase-2 continuation plan as Task 2.6.
- GPG signing of release artifacts. SHA256SUMS + HTTPS to github.com is the MVP trust chain; upgrade to minisign / GPG once the partner fleet grows past ~5 nodes.
- Core signaling release pipeline (cargo-dist / binary artifacts for `oxpulse-chat` itself). Separate effort — tracked in `docs/ROADMAP.md` phase work.

---

## File Structure

New files (all under `deploy/turn-node/` in the `oxpulse-chat` repo):

```
deploy/turn-node/
├── README.md                              # partner-facing: how to run install.sh, how to clone
├── install.sh                             # entrypoint — distro detect, packages, units, firewall, NTP
├── render-conf.sh                         # ExecStartPre body: renders /etc/turnserver.conf from env
├── healthcheck.sh                         # manual STUN probe for ops
├── templates/
│   ├── turnserver.conf.tmpl               # envsubst source (authoritative coturn config)
│   └── oxpulse-turn.env.example           # per-node env template with comments
├── systemd/
│   ├── oxpulse-turn-render.service        # oneshot: runs render-conf.sh
│   └── coturn.service.d-override.conf     # drop-in: Requires=+After= the render oneshot
└── scripts/
    └── autodetect-ip.sh                   # sourced by render-conf.sh; multi-source public IP detect
```

Modified:
- `docs/partners/onboarding.md` — collapses §4-§7 (manual install/config/firewall/start) into a single `curl | bash` invocation; §8-§12 (registration, verification, drain, appendix) stay as-is.
- `README.md` (repo root) — link to partner README in the "Running a TURN relay" paragraph.

Unchanged but referenced by the plan:
- `crates/turn/src/lib.rs` — HMAC credential generator (already tested, just read-only context for `static-auth-secret` matching).
- `crates/server/src/config.rs` — `parse_turn_servers` (already tested, just context for the registration string format).

**Decomposition rationale:** every artifact is one concern. `install.sh` is provisioning (runs once per node). `render-conf.sh` is templating (runs on every coturn start). `healthcheck.sh` is diagnostics (run manually). `autodetect-ip.sh` is sourced by both render and install — extracted because it has non-trivial fallback logic and deserves isolated testing.

---

## Task 0: Prep — verify working directory is clean and baseline facts

**Files:** none modified.

- [ ] **Step 1: Sanity-check repo state**

Run:
```bash
cd /home/krolik/src/oxpulse-chat
git status --short
git branch --show-current
```
Expected: working tree clean, branch `main` (or a fresh feature branch — if dirty, STOP and report `NEEDS_CONTEXT` per root `CLAUDE.md`).

- [ ] **Step 2: Confirm target test server is reachable by key**

Run: `ssh -o BatchMode=yes rvpn 'cat /etc/os-release | grep ^ID=; uname -m'`
Expected: `ID="centos"` + `x86_64`. This is the test VM the plan must end up working on.

- [ ] **Step 3: Record the shared TURN secret in a local variable (do NOT commit it)**

Run:
```bash
grep '^TURN_SECRET=' ~/deploy/krolik-server/.env | cut -d= -f2-
```
Keep the value in your shell environment for Task 7 (`export OPERATOR_TURN_SECRET=...`). It is **never** written to files under `deploy/turn-node/` — the example file uses `<REPLACE_ME>` and the real value lands only in `/etc/default/oxpulse-turn` on the partner host.

- [ ] **Step 4: Create feature branch**

```bash
cd /home/krolik/src/oxpulse-chat
git checkout -b feat/turn-node-template
```

---

## Task 1: Scaffold directory + partner README

**Files:**
- Create: `deploy/turn-node/README.md`
- Create: `deploy/turn-node/.gitignore` (ignore `*.env` except `.env.example`)

- [ ] **Step 1: Create directory structure**

```bash
cd /home/krolik/src/oxpulse-chat
mkdir -p deploy/turn-node/templates deploy/turn-node/systemd deploy/turn-node/scripts
```

- [ ] **Step 2: Write `deploy/turn-node/.gitignore`**

```
*.env
!templates/oxpulse-turn.env.example
```

- [ ] **Step 3: Write `deploy/turn-node/README.md`** (partner-facing; this is what the operator hands to the partner)

```markdown
# oxpulse-chat TURN Relay — Partner Node

One-command installer for a production-grade coturn relay that participates in
the oxpulse.chat TURN pool. Tested on Debian 12, Ubuntu 22.04 / 24.04,
AlmaLinux 9, Rocky Linux 9, CentOS Stream 9, RHEL 9.

## Quick start (fresh VM)

```bash
# 1. Log in as root on a freshly provisioned VM with a public IPv4.
#    Minimum: 1 vCPU, 2 GB RAM, 20 GB disk.

# 2. Run the installer (replace with your secret out-of-band):
curl -fsSL https://raw.githubusercontent.com/anatolykoptev/oxpulse-chat/main/deploy/turn-node/install.sh \
  | TURN_SECRET='<shared-secret-from-operator>' \
    REGION='ru-msk' \
    bash

# 3. Verify:
systemctl status coturn
/usr/local/sbin/oxpulse-turn-healthcheck
```

That's it. The installer is idempotent — re-running it upgrades the config
and restarts coturn safely.

## Environment variables (for install and later edits)

| Var            | Required | Default                | Notes                                                                 |
|----------------|----------|------------------------|-----------------------------------------------------------------------|
| `TURN_SECRET`  | yes      | —                      | Shared across the whole fleet. Delivered out-of-band by the operator. |
| `REGION`       | yes      | —                      | Operator-assigned tag (`ru-msk`, `de-fra`, `sg-sin`, ...).            |
| `PUBLIC_IPV4`  | no       | autodetect             | Override if autodetect picks a wrong address.                         |
| `PRIVATE_IPV4` | no       | autodetect (if behind NAT) | Cloud VMs (DO, Hetzner, Vultr) with 1:1 NAT need this.            |
| `REALM`        | no       | `oxpulse.chat`         | Rarely changed.                                                       |
| `PRIORITY`     | no       | `10`                   | Registration priority (lower = preferred).                            |

These are persisted to `/etc/default/oxpulse-turn` at install time. Editing
that file + `systemctl restart coturn` is the supported way to change values
later.

## Cloning to another region

1. Run `install.sh` on the first node (e.g. `call.rvpn.online`).
2. Verify health. Take a VM snapshot at your cloud provider.
3. Clone the snapshot into the new region.
4. SSH into the clone and run:
   ```bash
   vi /etc/default/oxpulse-turn   # update REGION, clear PUBLIC_IPV4 so it autodetects
   systemctl restart coturn
   /usr/local/sbin/oxpulse-turn-healthcheck
   ```
5. Send the operator the registration line printed by the healthcheck.

## Uninstall

```bash
systemctl disable --now coturn oxpulse-turn-render.service
rm -f /etc/turnserver.conf /etc/default/oxpulse-turn \
      /usr/local/sbin/oxpulse-turn-render /usr/local/sbin/oxpulse-turn-healthcheck \
      /etc/systemd/system/oxpulse-turn-render.service \
      /etc/systemd/system/coturn.service.d/override.conf
systemctl daemon-reload
# coturn package left installed — remove with apt/dnf if desired.
```

## Relationship to the operator runbook

The high-level flow (why a TURN relay, credential format, drain procedure,
incident response) lives in `docs/partners/onboarding.md`. This README is
strictly the mechanical how-to for the one-command install.
```

- [ ] **Step 4: Commit**

```bash
git add deploy/turn-node/README.md deploy/turn-node/.gitignore
git commit -m "docs(turn-node): scaffold deploy/turn-node with partner README"
```

---

## Task 2: Public-IP autodetect (the hard part of cloning)

**Files:**
- Create: `deploy/turn-node/scripts/autodetect-ip.sh`

**Why this task is isolated:** when a partner clones a snapshot to a new region, the previous node's public IP is baked into `/etc/default/oxpulse-turn`. Unless we detect "env PUBLIC_IPV4 is the OLD host's IP" and re-derive, coturn advertises the wrong `external-ip` and media relay fails silently. Treat this as the critical seam.

- [ ] **Step 1: Write `deploy/turn-node/scripts/autodetect-ip.sh`**

```bash
#!/usr/bin/env bash
# autodetect-ip.sh — sourced by install.sh and render-conf.sh.
# Exports PUBLIC_IPV4 and PRIVATE_IPV4 (the latter may be empty on bare metal).
# Safe to source multiple times.
set -eu

_is_ipv4() {
  [[ "$1" =~ ^([0-9]{1,3}\.){3}[0-9]{1,3}$ ]]
}

# Public IP: prefer cloud metadata (fast, authoritative), fall back to external probes.
_detect_public_ipv4() {
  local ip=""
  # 1) DigitalOcean / Hetzner Cloud / Vultr / Linode use the EC2-style metadata endpoint.
  ip=$(curl -fsS --max-time 2 http://169.254.169.254/latest/meta-data/public-ipv4 2>/dev/null || true)
  if _is_ipv4 "$ip"; then printf '%s' "$ip"; return 0; fi
  # 2) GCE / Google Cloud
  ip=$(curl -fsS --max-time 2 -H 'Metadata-Flavor: Google' \
    http://169.254.169.254/computeMetadata/v1/instance/network-interfaces/0/access-configs/0/external-ip 2>/dev/null || true)
  if _is_ipv4 "$ip"; then printf '%s' "$ip"; return 0; fi
  # 3) External probes (two independent providers — fail if both disagree/unreachable).
  local a b
  a=$(curl -fsS --max-time 3 https://api.ipify.org 2>/dev/null || true)
  b=$(curl -fsS --max-time 3 https://ifconfig.me 2>/dev/null || true)
  if _is_ipv4 "$a" && _is_ipv4 "$b" && [[ "$a" == "$b" ]]; then printf '%s' "$a"; return 0; fi
  if _is_ipv4 "$a"; then printf '%s' "$a"; return 0; fi
  if _is_ipv4 "$b"; then printf '%s' "$b"; return 0; fi
  return 1
}

# Private IP: the address on the default route interface. Empty if that IP
# equals the public one (bare metal with directly-attached public IP).
_detect_private_ipv4() {
  local iface priv
  iface=$(ip -4 route show default | awk '/default/ {print $5; exit}')
  [[ -z "$iface" ]] && return 0
  priv=$(ip -4 -o addr show dev "$iface" | awk '{print $4}' | cut -d/ -f1 | head -1)
  _is_ipv4 "$priv" || return 0
  # Only report it if it differs from the public IP (NAT case).
  [[ "$priv" == "${PUBLIC_IPV4:-}" ]] && return 0
  printf '%s' "$priv"
}

# Respect caller overrides — only autodetect if the var is unset OR empty.
if [[ -z "${PUBLIC_IPV4:-}" ]]; then
  PUBLIC_IPV4=$(_detect_public_ipv4) || {
    echo "autodetect-ip: unable to determine PUBLIC_IPV4; set it explicitly in /etc/default/oxpulse-turn" >&2
    return 1 2>/dev/null || exit 1
  }
fi
if [[ -z "${PRIVATE_IPV4:-}" ]]; then
  PRIVATE_IPV4=$(_detect_private_ipv4 || true)
fi
export PUBLIC_IPV4 PRIVATE_IPV4
```

- [ ] **Step 2: Shellcheck**

```bash
shellcheck -x deploy/turn-node/scripts/autodetect-ip.sh
```
Expected: exit 0, no warnings. If `shellcheck` is not installed: `sudo apt install shellcheck` or `sudo dnf install ShellCheck`.

- [ ] **Step 3: Dry-run locally on the dev machine**

```bash
( export PUBLIC_IPV4= PRIVATE_IPV4=
  source deploy/turn-node/scripts/autodetect-ip.sh
  echo "public=$PUBLIC_IPV4 private=$PRIVATE_IPV4" )
```
Expected: `public=<your public IP> private=` (or a private if you're behind NAT).

- [ ] **Step 4: Remote-run on rvpn** (validates the real target)

```bash
scp deploy/turn-node/scripts/autodetect-ip.sh rvpn:/tmp/
ssh rvpn 'PUBLIC_IPV4= PRIVATE_IPV4= ; source /tmp/autodetect-ip.sh; echo "public=$PUBLIC_IPV4 private=$PRIVATE_IPV4"'
```
Expected: `public=70.34.243.184 private=<internal>` (Vultr uses 1:1 NAT, so a private is expected).
If the private is empty on rvpn, that's still acceptable — coturn will work without `external-ip` slash-syntax on bare metal.

- [ ] **Step 5: Override path works**

```bash
ssh rvpn 'PUBLIC_IPV4=1.2.3.4 PRIVATE_IPV4=10.0.0.1 ; source /tmp/autodetect-ip.sh; echo "public=$PUBLIC_IPV4 private=$PRIVATE_IPV4"'
```
Expected: `public=1.2.3.4 private=10.0.0.1` (autodetect skipped).

- [ ] **Step 6: Commit**

```bash
git add deploy/turn-node/scripts/autodetect-ip.sh
git commit -m "feat(turn-node): public/private IP autodetect with overrides"
```

---

## Task 3: coturn config template + render script

**Files:**
- Create: `deploy/turn-node/templates/turnserver.conf.tmpl`
- Create: `deploy/turn-node/templates/oxpulse-turn.env.example`
- Create: `deploy/turn-node/render-conf.sh`

- [ ] **Step 1: Write `deploy/turn-node/templates/turnserver.conf.tmpl`** (authoritative config — matches `docs/partners/onboarding.md` §5, with expanded anti-SSRF blocks and logging discipline)

```conf
# Rendered by /usr/local/sbin/oxpulse-turn-render from /etc/default/oxpulse-turn.
# DO NOT EDIT THIS FILE DIRECTLY — it is regenerated on every coturn restart.

listening-port=3478
alt-listening-port=3479
fingerprint
lt-cred-mech
use-auth-secret
static-auth-secret=${TURN_SECRET}
realm=${REALM}

# Capacity + safety knobs
total-quota=200
stale-nonce=600
no-loopback-peers
no-multicast-peers

# TLS deliberately disabled in v1 (see docs/partners/onboarding.md §5).
no-tls
no-dtls

# Block TCP relay — forces UDP-only media, which is what clients use.
no-tcp-relay

# Anti-SSRF: block relay into RFC1918 + link-local + CGNAT + loopback.
# Without these the relay becomes a pivot into the partner's internal infra.
denied-peer-ip=0.0.0.0-0.255.255.255
denied-peer-ip=10.0.0.0-10.255.255.255
denied-peer-ip=100.64.0.0-100.127.255.255
denied-peer-ip=127.0.0.0-127.255.255.255
denied-peer-ip=169.254.0.0-169.254.255.255
denied-peer-ip=172.16.0.0-172.31.255.255
denied-peer-ip=192.0.0.0-192.0.0.255
denied-peer-ip=192.0.2.0-192.0.2.255
denied-peer-ip=192.168.0.0-192.168.255.255
denied-peer-ip=198.18.0.0-198.19.255.255
denied-peer-ip=198.51.100.0-198.51.100.255
denied-peer-ip=203.0.113.0-203.0.113.255
denied-peer-ip=224.0.0.0-239.255.255.255
denied-peer-ip=240.0.0.0-255.255.255.255

# Relay port range — must match firewall rule in install.sh.
min-port=49152
max-port=65535

external-ip=${EXTERNAL_IP_LINE}

# Run as unprivileged user the distro package created.
proc-user=${COTURN_USER}
proc-group=${COTURN_GROUP}

# Logging
log-file=/var/log/turnserver/turn.log
pidfile=/var/run/turnserver/turnserver.pid
no-stdout-log
simple-log
syslog

# Don't load any other conf — we own this file.
no-cli
```

Note: `${EXTERNAL_IP_LINE}` expands to either `PUBLIC/PRIVATE` (NAT case) or just `PUBLIC` (bare metal). `${COTURN_USER}` / `${COTURN_GROUP}` is distro-dependent (Debian=`turnserver`, RHEL=`coturn`).

- [ ] **Step 2: Write `deploy/turn-node/templates/oxpulse-turn.env.example`**

```bash
# /etc/default/oxpulse-turn — per-node configuration for the oxpulse-chat TURN relay.
# Edit this file, then: systemctl restart coturn
#
# This file is read by /usr/local/sbin/oxpulse-turn-render on every coturn start.
# Keep it mode 0600, root:root — it contains the shared HMAC secret.

# REQUIRED — shared across the entire fleet, delivered out-of-band by the operator.
TURN_SECRET=<REPLACE_ME>

# REQUIRED — operator-assigned region tag: ru-msk, ru-spb, de-fra, sg-sin, us-sfo, ...
REGION=<REPLACE_ME>

# Registration priority (lower = preferred). 0 for a primary, 10/20/... for in-region
# failovers, 100+ for cross-region fallback.
PRIORITY=10

# Realm advertised to clients. Rarely changed.
REALM=oxpulse.chat

# PUBLIC_IPV4: leave empty to autodetect from cloud metadata / external probes.
# Set explicitly if autodetect picks the wrong address (multi-homed hosts).
PUBLIC_IPV4=

# PRIVATE_IPV4: only needed on cloud VMs with 1:1 NAT (DO, Hetzner, Vultr, Linode).
# Leave empty on bare metal with a directly-attached public IP.
PRIVATE_IPV4=

# Public hostname registered with the operator. Prefer a hostname over a raw IP
# so the node can be moved without operator-side changes.
PUBLIC_HOST=<REPLACE_ME_e.g._turn-msk-1.example.net>
```

- [ ] **Step 3: Write `deploy/turn-node/render-conf.sh`**

```bash
#!/usr/bin/env bash
# render-conf.sh — ExecStartPre body for coturn. Reads /etc/default/oxpulse-turn,
# autodetects IPs, and writes /etc/turnserver.conf atomically.
set -euo pipefail

ENV_FILE="${OXPULSE_TURN_ENV:-/etc/default/oxpulse-turn}"
TMPL_FILE="${OXPULSE_TURN_TMPL:-/usr/local/share/oxpulse-turn/turnserver.conf.tmpl}"
OUT_FILE="${OXPULSE_TURN_CONF:-/etc/turnserver.conf}"
AUTODETECT="${OXPULSE_TURN_AUTODETECT:-/usr/local/share/oxpulse-turn/autodetect-ip.sh}"

if [[ ! -r "$ENV_FILE" ]]; then
  echo "render-conf: $ENV_FILE missing — run install.sh or copy from oxpulse-turn.env.example" >&2
  exit 2
fi
# shellcheck disable=SC1090
. "$ENV_FILE"

: "${TURN_SECRET:?TURN_SECRET is required in $ENV_FILE}"
: "${REALM:=oxpulse.chat}"

# shellcheck disable=SC1090
. "$AUTODETECT"

# Pick coturn user/group by distro.
if id -u coturn >/dev/null 2>&1; then
  COTURN_USER=coturn; COTURN_GROUP=coturn
elif id -u turnserver >/dev/null 2>&1; then
  COTURN_USER=turnserver; COTURN_GROUP=turnserver
else
  echo "render-conf: neither coturn nor turnserver user found" >&2
  exit 3
fi

if [[ -n "${PRIVATE_IPV4:-}" ]]; then
  EXTERNAL_IP_LINE="${PUBLIC_IPV4}/${PRIVATE_IPV4}"
else
  EXTERNAL_IP_LINE="${PUBLIC_IPV4}"
fi

export TURN_SECRET REALM EXTERNAL_IP_LINE COTURN_USER COTURN_GROUP

tmp=$(mktemp)
trap 'rm -f "$tmp"' EXIT
envsubst '${TURN_SECRET} ${REALM} ${EXTERNAL_IP_LINE} ${COTURN_USER} ${COTURN_GROUP}' \
  < "$TMPL_FILE" > "$tmp"

# Atomic replace + strict perms (config contains the HMAC secret).
install -o root -g "$COTURN_GROUP" -m 0640 "$tmp" "$OUT_FILE"

# Log dir coturn will write to — package install may have created it but not always.
install -o "$COTURN_USER" -g "$COTURN_GROUP" -m 0750 -d /var/log/turnserver /var/run/turnserver

echo "render-conf: wrote $OUT_FILE (public=$PUBLIC_IPV4 private=${PRIVATE_IPV4:-none} realm=$REALM)"
```

- [ ] **Step 4: Shellcheck both scripts**

```bash
shellcheck -x deploy/turn-node/render-conf.sh
```
Expected: exit 0.

- [ ] **Step 5: Dry-run render locally**

```bash
mkdir -p /tmp/oxpulse-render-test
cp deploy/turn-node/scripts/autodetect-ip.sh /tmp/oxpulse-render-test/
cp deploy/turn-node/templates/turnserver.conf.tmpl /tmp/oxpulse-render-test/
cat > /tmp/oxpulse-render-test/env <<'EOF'
TURN_SECRET=test-secret-not-real
REALM=oxpulse.chat
PUBLIC_IPV4=1.2.3.4
PRIVATE_IPV4=10.0.0.5
EOF
# Inject a fake user lookup: pick whichever of coturn/turnserver exists locally,
# or fall back to running under a stub (local dry-run doesn't require the user).
OXPULSE_TURN_ENV=/tmp/oxpulse-render-test/env \
OXPULSE_TURN_TMPL=/tmp/oxpulse-render-test/turnserver.conf.tmpl \
OXPULSE_TURN_AUTODETECT=/tmp/oxpulse-render-test/autodetect-ip.sh \
OXPULSE_TURN_CONF=/tmp/oxpulse-render-test/turnserver.conf \
bash deploy/turn-node/render-conf.sh || echo "expected failure if no coturn/turnserver user locally"
```
Expected on dev machine without coturn: clean failure on "neither coturn nor turnserver user found". That's the intended safety check — not a bug.

- [ ] **Step 6: Commit**

```bash
git add deploy/turn-node/templates/ deploy/turn-node/render-conf.sh
git commit -m "feat(turn-node): coturn config template + render script"
```

---

## Task 4: systemd units (drop-in for coturn + render oneshot)

**Files:**
- Create: `deploy/turn-node/systemd/oxpulse-turn-render.service`
- Create: `deploy/turn-node/systemd/coturn.service.d-override.conf`

- [ ] **Step 1: Write `deploy/turn-node/systemd/oxpulse-turn-render.service`**

```ini
[Unit]
Description=oxpulse-chat — render /etc/turnserver.conf from /etc/default/oxpulse-turn
Documentation=https://github.com/anatolykoptev/oxpulse-chat/tree/main/deploy/turn-node
ConditionPathExists=/etc/default/oxpulse-turn
Before=coturn.service
DefaultDependencies=yes

[Service]
Type=oneshot
RemainAfterExit=no
ExecStart=/usr/local/sbin/oxpulse-turn-render
# If rendering fails, coturn won't start (drop-in below makes it a hard dep).
StandardOutput=journal
StandardError=journal

[Install]
WantedBy=multi-user.target
```

- [ ] **Step 2: Write `deploy/turn-node/systemd/coturn.service.d-override.conf`**

```ini
# Installed at /etc/systemd/system/coturn.service.d/override.conf
# Chains our render oneshot before coturn starts; no-op on re-render if conf unchanged.
[Unit]
Requires=oxpulse-turn-render.service
After=oxpulse-turn-render.service
# chrony keeps our clock sane — HMAC TTL breaks with drift >24h
After=chrony.service chronyd.service

[Service]
# coturn package defaults the ExecStart to use /etc/turnserver.conf — that's what we render.
# Harden a little: restart on failure, rate-limit restart storm.
Restart=on-failure
RestartSec=5s
# CAP_NET_BIND_SERVICE needed for privileged ports; distro already sets user=turnserver/coturn.
```

- [ ] **Step 3: Syntax-validate locally with systemd-analyze**

```bash
systemd-analyze verify deploy/turn-node/systemd/oxpulse-turn-render.service 2>&1 || true
```
Expected: may warn about missing `coturn.service` (not installed locally) — that's fine. Actual verification happens on rvpn in Task 6.

- [ ] **Step 4: Commit**

```bash
git add deploy/turn-node/systemd/
git commit -m "feat(turn-node): systemd render oneshot + coturn override"
```

---

## Task 5: The installer itself

**Files:**
- Create: `deploy/turn-node/install.sh`

- [ ] **Step 1: Write `deploy/turn-node/install.sh`**

```bash
#!/usr/bin/env bash
# install.sh — idempotent bootstrap + file-sync for an oxpulse-chat TURN relay.
#
# Full install (fresh node):
#   TURN_SECRET='...' REGION='ru-msk' bash install.sh
#
# Files-only sync (used by oxpulse-turn-upgrade — no env/firewall/start):
#   bash install.sh --files-only --from-dir /path/to/extracted/turn-node
#
# Optional env overrides on full install:
#   PUBLIC_IPV4, PRIVATE_IPV4, PUBLIC_HOST, PRIORITY, REALM
set -euo pipefail

REPO_RAW="${OXPULSE_REPO_RAW:-https://raw.githubusercontent.com/anatolykoptev/oxpulse-chat/main/deploy/turn-node}"
PREFIX_SBIN=/usr/local/sbin
PREFIX_SHARE=/usr/local/share/oxpulse-turn
SYSTEMD_DIR=/etc/systemd/system

log()  { printf '\033[32m==>\033[0m %s\n' "$*" >&2; }
warn() { printf '\033[33m!!\033[0m  %s\n' "$*" >&2; }
die()  { printf '\033[31mERR\033[0m %s\n' "$*" >&2; exit 1; }

[[ $EUID -eq 0 ]] || die "must run as root"

# ---------- Argument parsing ----------
FILES_ONLY=0
FROM_DIR=""
while [[ $# -gt 0 ]]; do
  case "$1" in
    --files-only) FILES_ONLY=1; shift ;;
    --from-dir)   FROM_DIR="${2:?--from-dir requires a path}"; shift 2 ;;
    -h|--help)    sed -n '2,12p' "$0"; exit 0 ;;
    *)            die "unknown argument: $1" ;;
  esac
done

# ---------- 1. Distro detect (always — upgrader needs it for FAMILY in fetch path too) ----------
. /etc/os-release
ID_LIKE_ALL="$ID ${ID_LIKE:-}"
case " $ID_LIKE_ALL " in
  *" debian "*|*" ubuntu "*) FAMILY=debian ;;
  *" rhel "*|*" fedora "*|*" centos "*) FAMILY=rhel ;;
  *) die "unsupported distro: ID=$ID ID_LIKE=${ID_LIKE:-<empty>}" ;;
esac
log "detected: $PRETTY_NAME (family=$FAMILY) files_only=$FILES_ONLY"

# ---------- 2. Required inputs (full install only — --files-only preserves env) ----------
if [[ $FILES_ONLY -eq 0 ]]; then
  : "${TURN_SECRET:?TURN_SECRET env is required}"
  : "${REGION:?REGION env is required (e.g. ru-msk)}"
  PRIORITY="${PRIORITY:-10}"
  REALM="${REALM:-oxpulse.chat}"
  PUBLIC_IPV4="${PUBLIC_IPV4:-}"
  PRIVATE_IPV4="${PRIVATE_IPV4:-}"
  PUBLIC_HOST="${PUBLIC_HOST:-}"
fi

# ---------- 3. Packages (full install only) ----------
install_packages() {
  if [[ $FAMILY == debian ]]; then
    export DEBIAN_FRONTEND=noninteractive
    apt-get update -q
    apt-get install -y -q coturn chrony gettext-base curl ca-certificates iproute2
    if [[ -f /etc/default/coturn ]]; then
      sed -i 's/^#\?TURNSERVER_ENABLED=.*/TURNSERVER_ENABLED=1/' /etc/default/coturn
      grep -q '^TURNSERVER_ENABLED=1' /etc/default/coturn || echo 'TURNSERVER_ENABLED=1' >> /etc/default/coturn
    fi
  else
    if ! rpm -q epel-release >/dev/null 2>&1; then
      dnf install -y epel-release
    fi
    dnf install -y coturn chrony gettext curl ca-certificates iproute
  fi
}
if [[ $FILES_ONLY -eq 0 ]]; then
  log "installing packages"
  install_packages
  # NTP (credential HMAC is timestamp-based).
  systemctl enable --now chronyd 2>/dev/null || systemctl enable --now chrony
fi

# ---------- 4. Fetch artifacts: explicit --from-dir > script-dir checkout > curl from REPO_RAW ----------
if [[ -n "$FROM_DIR" ]]; then
  SRC_DIR="$FROM_DIR"
  FETCH() { cp "$SRC_DIR/$1" "$2"; }
  log "using artifacts from --from-dir: $SRC_DIR"
elif [[ -f "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/render-conf.sh" ]]; then
  SRC_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
  FETCH() { cp "$SRC_DIR/$1" "$2"; }
  log "using artifacts from local checkout: $SRC_DIR"
else
  SRC_DIR=""
  FETCH() { curl -fsSL "$REPO_RAW/$1" -o "$2"; }
  log "fetching artifacts from $REPO_RAW"
fi

install -d -m 0755 "$PREFIX_SHARE"
FETCH templates/turnserver.conf.tmpl "$PREFIX_SHARE/turnserver.conf.tmpl"
FETCH scripts/autodetect-ip.sh       "$PREFIX_SHARE/autodetect-ip.sh"
chmod 0644 "$PREFIX_SHARE/turnserver.conf.tmpl" "$PREFIX_SHARE/autodetect-ip.sh"

FETCH render-conf.sh   "$PREFIX_SBIN/oxpulse-turn-render"
FETCH healthcheck.sh   "$PREFIX_SBIN/oxpulse-turn-healthcheck"
FETCH upgrade.sh       "$PREFIX_SBIN/oxpulse-turn-upgrade"
chmod 0755 "$PREFIX_SBIN/oxpulse-turn-render" "$PREFIX_SBIN/oxpulse-turn-healthcheck" "$PREFIX_SBIN/oxpulse-turn-upgrade"

FETCH systemd/oxpulse-turn-render.service     "$SYSTEMD_DIR/oxpulse-turn-render.service"
FETCH systemd/oxpulse-turn-upgrade.service    "$SYSTEMD_DIR/oxpulse-turn-upgrade.service"
FETCH systemd/oxpulse-turn-upgrade.timer      "$SYSTEMD_DIR/oxpulse-turn-upgrade.timer"
install -d -m 0755 "$SYSTEMD_DIR/coturn.service.d"
FETCH systemd/coturn.service.d-override.conf  "$SYSTEMD_DIR/coturn.service.d/override.conf"

# Ship VERSION marker — sourced either from local tree or falls back to 0.0.0 for curl-mode.
if [[ -n "$SRC_DIR" && -f "$SRC_DIR/VERSION" ]]; then
  install -m 0644 "$SRC_DIR/VERSION" "$PREFIX_SHARE/VERSION"
else
  # curl-mode: fetch VERSION separately (best-effort).
  if FETCH VERSION "$PREFIX_SHARE/VERSION" 2>/dev/null; then :; else echo 0.0.0 > "$PREFIX_SHARE/VERSION"; fi
fi

# ---------- 5. Env file (full install only) ----------
if [[ $FILES_ONLY -eq 0 ]]; then
  if [[ ! -f /etc/default/oxpulse-turn ]]; then
    log "writing /etc/default/oxpulse-turn"
    install -m 0600 -o root -g root /dev/stdin /etc/default/oxpulse-turn <<EOF
TURN_SECRET=$TURN_SECRET
REGION=$REGION
PRIORITY=$PRIORITY
REALM=$REALM
PUBLIC_IPV4=$PUBLIC_IPV4
PRIVATE_IPV4=$PRIVATE_IPV4
PUBLIC_HOST=$PUBLIC_HOST
EOF
  else
    warn "/etc/default/oxpulse-turn exists — preserving. Edit manually if values changed."
  fi
fi

# ---------- 6. Firewall (full install only) ----------
configure_firewall() {
  if [[ $FAMILY == rhel ]] && systemctl is-active --quiet firewalld; then
    log "firewalld: opening 3478/udp,3479/udp,3478/tcp,49152-65535/udp"
    firewall-cmd --permanent --add-port=3478/udp
    firewall-cmd --permanent --add-port=3479/udp
    firewall-cmd --permanent --add-port=3478/tcp
    firewall-cmd --permanent --add-port=49152-65535/udp
    firewall-cmd --reload
  elif command -v ufw >/dev/null && ufw status | grep -q 'Status: active'; then
    log "ufw: opening TURN ports"
    ufw allow 3478/udp
    ufw allow 3479/udp
    ufw allow 3478/tcp
    ufw allow 49152:65535/udp
  else
    warn "no active firewalld/ufw detected — cloud security group assumed."
    warn "ensure UDP 3478, 3479, 49152-65535 and TCP 3478 are reachable from the public internet."
  fi
}
if [[ $FILES_ONLY -eq 0 ]]; then
  configure_firewall
fi

# ---------- 7. Enable + start (full install) or just daemon-reload (--files-only) ----------
systemctl daemon-reload
if [[ $FILES_ONLY -eq 0 ]]; then
  systemctl enable --now oxpulse-turn-render.service
  systemctl enable --now coturn.service
  # Re-install bumps the config — force restart so new values are live.
  systemctl restart coturn.service
  # upgrade.timer shipped but intentionally NOT enabled — partner opts in explicitly.
fi

# ---------- 8. Registration hint (full install only) ----------
if [[ $FILES_ONLY -eq 0 ]]; then
  # shellcheck disable=SC1091
  . "$PREFIX_SHARE/autodetect-ip.sh"
  HOST_FOR_REG="${PUBLIC_HOST:-$PUBLIC_IPV4}"
  cat <<BANNER

========================================================================
  oxpulse-chat TURN relay installed.
  Verify:  $PREFIX_SBIN/oxpulse-turn-healthcheck
  Upgrade: $PREFIX_SBIN/oxpulse-turn-upgrade [--check] [turn-node-vX.Y.Z]
           systemctl enable --now oxpulse-turn-upgrade.timer  # opt-in nightly

  Send to the operator:

    $REGION:$PRIORITY:turn:$HOST_FOR_REG:3478?transport=udp

  Edit /etc/default/oxpulse-turn + 'systemctl restart coturn' to change values.
========================================================================
BANNER
fi
```

**Note on Task 5 vs Task 10 ordering:** this script references `upgrade.sh` and two systemd units that Task 10 creates. That's deliberate — Task 5 commits the installer *skeleton* with forward references; Task 10 commits the referenced files. Between the two commits, `install.sh` would fail a curl-mode run (missing upload.sh on GitHub) but succeed in local-checkout mode once Task 10's files exist on disk. Since the first real run is Task 6 (which happens *after* Task 10 — see execution order), there's no window where broken state reaches a partner host. If you want airtight per-commit green: swap Task 5 and Task 10 ordering when executing — the subagent runner supports it.

- [ ] **Step 2: Shellcheck**

```bash
shellcheck -x deploy/turn-node/install.sh
```
Expected: exit 0. (`SC1091` for sourcing `/etc/os-release` — acceptable, or add a `# shellcheck source=/dev/null` directive.)

- [ ] **Step 3: Commit**

```bash
git add deploy/turn-node/install.sh
git commit -m "feat(turn-node): idempotent multi-distro installer"
```

---

## Task 6: Healthcheck script + end-to-end run on rvpn

**Files:**
- Create: `deploy/turn-node/healthcheck.sh`
- Modify nothing else — this task is the **first real integration test**.

- [ ] **Step 1: Write `deploy/turn-node/healthcheck.sh`**

```bash
#!/usr/bin/env bash
# healthcheck.sh — operator-side smoke test for a running TURN relay.
# Runs on the TURN host itself. Exit 0 = healthy, nonzero = investigate.
set -euo pipefail

ENV_FILE=/etc/default/oxpulse-turn
[[ -r $ENV_FILE ]] || { echo "no $ENV_FILE" >&2; exit 2; }
# shellcheck disable=SC1090
. "$ENV_FILE"
# shellcheck disable=SC1091
. /usr/local/share/oxpulse-turn/autodetect-ip.sh

FAIL=0
check() {
  printf '%-40s' "$1"
  if "${@:2}" >/dev/null 2>&1; then echo OK; else echo FAIL; FAIL=$((FAIL+1)); fi
}

check "coturn.service active"          systemctl is-active --quiet coturn
check "oxpulse-turn-render ran"        systemctl is-active --quiet oxpulse-turn-render.service
check "chrony synchronised"            bash -c 'chronyc tracking | grep -q "Leap status.*Normal"'
check "UDP 3478 listening"             bash -c 'ss -lunp | grep -q ":3478 "'
check "TCP 3478 listening"             bash -c 'ss -ltnp | grep -q ":3478 "'
check "STUN binding-request replies"   bash -c "turnutils_stunclient '$PUBLIC_IPV4' >/dev/null"
check "conf owned by coturn group"     bash -c 'ls -l /etc/turnserver.conf | grep -qE "coturn|turnserver"'

HOST_FOR_REG="${PUBLIC_HOST:-$PUBLIC_IPV4}"
echo
echo "  Registration line for operator:"
echo "    $REGION:$PRIORITY:turn:$HOST_FOR_REG:3478?transport=udp"
exit "$FAIL"
```

- [ ] **Step 2: Shellcheck**

```bash
shellcheck -x deploy/turn-node/healthcheck.sh
```

- [ ] **Step 3: Ship the whole tree to rvpn and run install.sh from the local checkout**

```bash
# Keep secret in env only — never on the command line (ps/history).
read -rsp "Paste TURN_SECRET from ~/deploy/krolik-server/.env: " TURN_SECRET; echo
export TURN_SECRET

rsync -av --delete deploy/turn-node/ rvpn:/root/oxpulse-turn-node/
ssh rvpn "cd /root/oxpulse-turn-node && \
  TURN_SECRET='$TURN_SECRET' \
  REGION='ru-test' \
  PRIORITY=100 \
  PUBLIC_HOST=call.rvpn.online \
  bash install.sh"
```
Expected final output: the `BANNER` block, with a registration line like `ru-test:100:turn:call.rvpn.online:3478?transport=udp`.

- [ ] **Step 4: Run healthcheck on rvpn**

```bash
ssh rvpn /usr/local/sbin/oxpulse-turn-healthcheck
```
Expected: 7 OK lines, exit 0, registration line printed. If any FAIL, stop and diagnose before proceeding — the healthcheck output points at the broken component.

- [ ] **Step 5: External smoke test from the dev machine**

```bash
# nc reachability
nc -zv -u 70.34.243.184 3478 || true     # UDP — nc prints 'open' or times out; harmless if flaky
nc -zv    70.34.243.184 3478             # TCP — should print 'succeeded'
# STUN probe from outside
turnutils_stunclient 70.34.243.184
```
Expected: `turnutils_stunclient` prints a Mapped Address equal to **your** public IP (that's STUN working — the server is echoing your observed IP).

- [ ] **Step 6: Idempotency check — re-run install.sh**

```bash
ssh rvpn "cd /root/oxpulse-turn-node && \
  TURN_SECRET='$TURN_SECRET' REGION='ru-test' PRIORITY=100 PUBLIC_HOST=call.rvpn.online bash install.sh"
```
Expected: same success banner, no errors, `/etc/default/oxpulse-turn` preserved (warning line `/etc/default/oxpulse-turn exists — preserving`). coturn restarted cleanly.

- [ ] **Step 7: Commit**

```bash
git add deploy/turn-node/healthcheck.sh
git commit -m "feat(turn-node): healthcheck script + validated on call.rvpn.online"
```

---

## Task 7: Register the node in signaling + end-to-end credential test

**Files:**
- Modify: `~/deploy/krolik-server/.env` (add to `TURN_SERVERS` — **operator side, not repo**)

This task validates the full contract: a real client hitting our signaling gets credentials that actually authenticate against the new relay.

- [ ] **Step 1: Inspect current `TURN_SERVERS`**

```bash
grep '^TURN_SERVERS=' ~/deploy/krolik-server/.env || echo "TURN_SERVERS is unset"
```
Note the current value (we're appending, not replacing).

- [ ] **Step 2: Append the new relay**

Edit `~/deploy/krolik-server/.env`:
```
TURN_SERVERS=<existing-value>,ru-test:100:turn:call.rvpn.online:3478?transport=udp
```
If `TURN_SERVERS` was unset, set it fresh:
```
TURN_SERVERS=ru-test:100:turn:call.rvpn.online:3478?transport=udp
```

- [ ] **Step 3: Recycle signaling**

```bash
cd ~/deploy/krolik-server
docker compose up -d --force-recreate oxpulse-chat
```
Expected: container `oxpulse-chat` restarts, `docker logs oxpulse-chat 2>&1 | head -30` shows a TURN pool line referencing the new URL (log format: see `crates/server/src/main.rs` startup logs).

- [ ] **Step 4: Issue credentials via the real endpoint**

```bash
curl -sX POST https://oxpulse.chat/api/turn-credentials | jq .
```
Expected: JSON with `username`, `credential`, `ttl`, `ice_servers[]`. The new URL `turn:call.rvpn.online:3478?transport=udp` should appear somewhere in the `ice_servers[].urls` list (Phase-2 note: until Task 2.4 ships, ALL configured URLs are returned unconditionally — that's expected).

- [ ] **Step 5: Prove the HMAC roundtrip against the new relay**

```bash
# Extract username+credential, probe the new relay with them.
resp=$(curl -sX POST https://oxpulse.chat/api/turn-credentials)
user=$(jq -r .username <<<"$resp")
pass=$(jq -r .credential <<<"$resp")
turnutils_uclient -u "$user" -w "$pass" -v -p 3478 70.34.243.184
```
Expected: `turnutils_uclient` output ends with `...all tests succeeded.` (or equivalent — this command sends a TURN Allocate + permission + send-indication and prints `"success"` counts). A 401 here means the HMAC secret on the relay doesn't match the operator's — STOP and re-check `/etc/default/oxpulse-turn` on rvpn.

- [ ] **Step 6: Clean up test registration**

```bash
# Remove the ru-test entry from TURN_SERVERS (this was a test run; production priority
# will be assigned by the partner when they roll out real nodes).
# Edit ~/deploy/krolik-server/.env to drop ',ru-test:100:...' from TURN_SERVERS,
# then:
cd ~/deploy/krolik-server
docker compose up -d --force-recreate oxpulse-chat
```

- [ ] **Step 7: No commit here** — this task modifies the operator host, not the repo.

---

## Task 8: Auto-release via release-please (GoReleaser-style workflow)

**Files:**
- Create: `release-please-config.json`
- Create: `.release-please-manifest.json`
- Create: `.github/workflows/release-please.yml`

**Why release-please and not cargo-dist / tagpr:** release-please is Google's multi-language answer to GoReleaser for mono-repos with components. It reads Conventional Commits, maintains a draft Release PR per component, bumps versions by commit type, writes component-scoped CHANGELOGs, and tags + creates GitHub Releases on merge. No build logic — that stays in a separate workflow (Task 9), cleanly separating version-management from artifact-packaging. cargo-dist is Rust-binary specific; tagpr is Japanese-scene single-component. For a mono-repo that will eventually host both a Rust core and a bash TURN-node artifact, release-please is the right shape.

- [ ] **Step 1: Write `release-please-config.json`** (at repo root)

```json
{
  "$schema": "https://raw.githubusercontent.com/googleapis/release-please/main/schemas/config.json",
  "release-type": "simple",
  "include-component-in-tag": true,
  "include-v-in-tag": true,
  "separate-pull-requests": true,
  "packages": {
    "deploy/turn-node": {
      "component": "turn-node",
      "release-type": "simple",
      "package-name": "turn-node",
      "changelog-path": "CHANGELOG.md",
      "extra-files": ["VERSION"],
      "include-component-in-tag": true
    }
  }
}
```

`release-type: simple` tells release-please that this component has no language-native manifest to bump (not Cargo.toml, not package.json). It bumps a literal `VERSION` file instead — same pattern GoReleaser-for-bash projects use.

- [ ] **Step 2: Write `.release-please-manifest.json`** (at repo root — state file, tracks current released versions)

```json
{
  "deploy/turn-node": "0.0.0"
}
```

(0.0.0 is the pre-release seed — release-please will bump it to 1.0.0 on the first `feat!:` or explicit release.)

- [ ] **Step 3: Seed initial `deploy/turn-node/VERSION` and `deploy/turn-node/CHANGELOG.md`**

```bash
echo '0.0.0' > deploy/turn-node/VERSION
cat > deploy/turn-node/CHANGELOG.md <<'EOF'
# Changelog

All notable changes to the `turn-node` component are documented here.
This file is maintained by release-please from Conventional Commits.
EOF
```

- [ ] **Step 4: Write `.github/workflows/release-please.yml`**

```yaml
name: release-please

on:
  push:
    branches: [main]

permissions:
  contents: write
  pull-requests: write

jobs:
  release-please:
    runs-on: ubuntu-latest
    steps:
      - uses: googleapis/release-please-action@v4
        with:
          config-file: release-please-config.json
          manifest-file: .release-please-manifest.json
          # The action handles per-component tagging:
          #   component "turn-node" → tag "turn-node-v1.2.3"
          # (The separator is "-", not "/", because release-please v4 normalises
          # component names into tags that way — see action README "Tag names".)
```

Tag format becomes `turn-node-v1.2.3` (release-please's canonical form, `-` separator). Update any prior references in this plan and in Task 9's workflow trigger accordingly — the final format of record is `turn-node-v*`.

- [ ] **Step 5: Reference the commit conventions in `CONTRIBUTING.md` (or create it)**

```bash
cat > CONTRIBUTING.md <<'EOF'
# Contributing

## Commits

This repo uses [Conventional Commits](https://www.conventionalcommits.org/).
`release-please` watches `main` and opens per-component Release PRs that bump
versions, update CHANGELOG.md, and tag on merge.

Commit scopes that drive turn-node releases:

- `feat(turn-node): ...` → MINOR bump (new env var, new distro supported)
- `fix(turn-node): ...` → PATCH bump (template hardening, bug fix)
- `feat(turn-node)!: ...` or a `BREAKING CHANGE:` footer → MAJOR bump (env-var rename, template-incompatible change)

Scopes outside `turn-node` do not cut a turn-node release.
EOF
```

(If `CONTRIBUTING.md` already exists, append the "Commits" section instead of overwriting.)

- [ ] **Step 6: Commit**

```bash
git add release-please-config.json .release-please-manifest.json \
  deploy/turn-node/VERSION deploy/turn-node/CHANGELOG.md \
  .github/workflows/release-please.yml CONTRIBUTING.md
git commit -m "chore(release): set up release-please with turn-node component"
```

- [ ] **Step 7: Verify on a push-to-main dry-run** (after Task 11 merges — noted here for continuity)

After the feature branch is merged to `main`, release-please will open a PR titled
`chore(turn-node): release turn-node 1.0.0` (first Conventional `feat(turn-node):`
commit triggers 0.0.0 → 1.0.0). Merging that PR will:
1. Tag `turn-node-v1.0.0`.
2. Create a GitHub Release with the auto-generated changelog.
3. Trigger the workflow from Task 9 to attach the artifact.

No action required in this task beyond setup; the dry-run happens naturally after merge.

---

## Task 9: Artifact build + attach on tag (the "GoReleaser" half)

**Files:**
- Create: `.github/workflows/turn-node-release.yml`

**Role:** release-please handles version-management and Release-creation. This workflow handles **artifact-packaging** — triggered by the tag that release-please pushes, it creates the tarball, computes SHA256SUMS, and attaches both to the existing Release. Mirrors what GoReleaser does for Go binaries, but for a bash+conf artifact.

- [ ] **Step 1: Write `.github/workflows/turn-node-release.yml`**

```yaml
name: turn-node-release

on:
  push:
    tags:
      - 'turn-node-v*'

permissions:
  contents: write  # upload release assets

jobs:
  package-and-upload:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Extract version from tag
        id: ver
        run: |
          # Tag format: turn-node-v1.2.3 -> VERSION=1.2.3
          tag="${GITHUB_REF#refs/tags/}"
          version="${tag#turn-node-v}"
          echo "version=$version" >> "$GITHUB_OUTPUT"
          echo "tag=$tag" >> "$GITHUB_OUTPUT"

      - name: Verify VERSION file matches tag
        run: |
          file_version=$(cat deploy/turn-node/VERSION)
          if [[ "$file_version" != "${{ steps.ver.outputs.version }}" ]]; then
            echo "::error::VERSION file ($file_version) != tag version (${{ steps.ver.outputs.version }})"
            exit 1
          fi

      - name: Lint shell scripts
        run: |
          sudo apt-get update -qq
          sudo apt-get install -y -qq shellcheck
          shopt -s nullglob
          for f in deploy/turn-node/*.sh deploy/turn-node/scripts/*.sh; do
            echo ">> $f"
            shellcheck -x "$f"
          done

      - name: Build tarball
        run: |
          ver="${{ steps.ver.outputs.version }}"
          tar -czf "turn-node-${ver}.tar.gz" \
            --transform "s,^deploy/turn-node,turn-node-${ver}," \
            deploy/turn-node

      - name: Stage standalone installer as a release asset
        run: |
          # Partners curl this URL; it's the same install.sh, renamed for discoverability.
          cp deploy/turn-node/install.sh turn-node-installer.sh
          # Recompute SHA256SUMS to include all three assets.
          ver="${{ steps.ver.outputs.version }}"
          sha256sum "turn-node-${ver}.tar.gz" turn-node-installer.sh > SHA256SUMS
          cat SHA256SUMS

      - name: Attach assets to the Release created by release-please
        env:
          GH_TOKEN: ${{ github.token }}
        run: |
          ver="${{ steps.ver.outputs.version }}"
          gh release upload "${{ steps.ver.outputs.tag }}" \
            "turn-node-${ver}.tar.gz" turn-node-installer.sh SHA256SUMS \
            --clobber
```

- [ ] **Step 2: Dry-run the workflow locally using `act` (optional)**

```bash
# Only if act is installed; skippable — the real validation is step 4.
act -l 2>/dev/null || echo "act not installed, skipping local dry-run"
```

- [ ] **Step 3: Smoke-test the tarball structure locally** (same commands the workflow runs)

```bash
(cd /tmp && rm -rf turn-node-test && mkdir turn-node-test && cd turn-node-test && \
  tar -czf "turn-node-0.0.0.tar.gz" \
    --transform "s,^deploy/turn-node,turn-node-0.0.0," \
    -C /home/krolik/src/oxpulse-chat deploy/turn-node && \
  tar -tzf turn-node-0.0.0.tar.gz | head -20)
```
Expected: tarball contents live under `turn-node-0.0.0/...` (not `deploy/turn-node/...`) — this keeps extraction clean for partners.

- [ ] **Step 4: Commit**

```bash
git add .github/workflows/turn-node-release.yml
git commit -m "ci(turn-node): package + attach release artifacts on tag push"
```

- [ ] **Step 5: Verify end-to-end (after PR merge — noted here for continuity)**

After merging the feature branch and then the release-please PR, confirm:
1. `turn-node-v1.0.0` tag exists: `git fetch --tags; git tag | grep turn-node`
2. Release has assets: `gh release view turn-node-v1.0.0 --json assets -q '.assets[].name'`
   Expected: `turn-node-1.0.0.tar.gz`, `turn-node-installer.sh`, `SHA256SUMS`.
3. Checksum downloadable: `curl -fsSL https://github.com/anatolykoptev/oxpulse-chat/releases/download/turn-node-v1.0.0/SHA256SUMS`

---

## Task 10: Partner-side upgrader (`oxpulse-turn-upgrade`)

**Files:**
- Create: `deploy/turn-node/upgrade.sh`
- Create: `deploy/turn-node/systemd/oxpulse-turn-upgrade.service`
- Create: `deploy/turn-node/systemd/oxpulse-turn-upgrade.timer`
- Modify: `deploy/turn-node/install.sh` (wire installer to also drop upgrade tool + dormant timer)

**Contract:** `oxpulse-turn-upgrade` pulls the latest (or pinned) `turn-node-*.tar.gz` from GitHub Releases, verifies SHA256, swaps artifacts atomically, restarts coturn, rolls back on failure, never touches `/etc/default/oxpulse-turn`.

- [ ] **Step 1: Write `deploy/turn-node/upgrade.sh`**

```bash
#!/usr/bin/env bash
# oxpulse-turn-upgrade — pull, verify, install a new turn-node release.
# Usage:
#   oxpulse-turn-upgrade                      # latest
#   oxpulse-turn-upgrade turn-node-v1.2.3     # pinned
#   oxpulse-turn-upgrade --check              # report pending upgrade, don't apply
set -euo pipefail

REPO_SLUG="${OXPULSE_REPO_SLUG:-anatolykoptev/oxpulse-chat}"
PREFIX_SHARE=/usr/local/share/oxpulse-turn
PREFIX_SBIN=/usr/local/sbin
BACKUP_DIR=/var/lib/oxpulse-turn/backups

log()  { printf '\033[32m==>\033[0m %s\n' "$*" >&2; }
warn() { printf '\033[33m!!\033[0m  %s\n' "$*" >&2; }
die()  { printf '\033[31mERR\033[0m %s\n' "$*" >&2; exit 1; }

[[ $EUID -eq 0 ]] || die "must run as root"

MODE=apply
TARGET=""
for arg in "$@"; do
  case "$arg" in
    --check) MODE=check ;;
    turn-node-v*) TARGET="$arg" ;;
    *) die "unknown arg: $arg" ;;
  esac
done

resolve_latest() {
  curl -fsSL "https://api.github.com/repos/${REPO_SLUG}/releases" \
    | grep -oE '"tag_name":\s*"turn-node-v[0-9]+\.[0-9]+\.[0-9]+"' \
    | head -1 \
    | sed -E 's/.*"(turn-node-v[0-9.]+)".*/\1/'
}

[[ -z "$TARGET" ]] && TARGET=$(resolve_latest) && [[ -n "$TARGET" ]] || die "cannot resolve latest release"
VERSION="${TARGET#turn-node-v}"

current="none"
[[ -f "$PREFIX_SHARE/VERSION" ]] && current=$(cat "$PREFIX_SHARE/VERSION")
log "current=$current target=$VERSION"

if [[ "$current" == "$VERSION" ]]; then
  log "already on $VERSION — nothing to do"
  exit 0
fi
if [[ "$MODE" == check ]]; then
  echo "UPGRADE_AVAILABLE current=$current target=$VERSION"
  exit 10
fi

# ---- fetch + verify ----
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
base="https://github.com/${REPO_SLUG}/releases/download/${TARGET}"
log "downloading ${TARGET} tarball + SHA256SUMS"
curl -fsSL "${base}/turn-node-${VERSION}.tar.gz" -o "$work/tarball.tgz"
curl -fsSL "${base}/SHA256SUMS"                  -o "$work/SHA256SUMS"

( cd "$work" && grep " turn-node-${VERSION}.tar.gz$" SHA256SUMS \
  | awk -v f=tarball.tgz '{print $1"  "f}' | sha256sum -c - ) \
  || die "SHA256 mismatch — aborting"
log "sha256 verified"

# ---- extract + stage ----
tar -xzf "$work/tarball.tgz" -C "$work"
stage="$work/turn-node-${VERSION}"
[[ -d "$stage" ]] || die "extracted layout unexpected"

# ---- backup current ----
install -d -m 0700 "$BACKUP_DIR"
ts=$(date -u +%Y%m%dT%H%M%SZ)
if [[ -d "$PREFIX_SHARE" && "$current" != "none" ]]; then
  backup="$BACKUP_DIR/turn-node-${current}-${ts}"
  log "backing up to $backup"
  cp -a "$PREFIX_SHARE" "$backup"
  cp -a "$PREFIX_SBIN/oxpulse-turn-render" "$backup/" 2>/dev/null || true
  cp -a "$PREFIX_SBIN/oxpulse-turn-healthcheck" "$backup/" 2>/dev/null || true
  cp -a "$PREFIX_SBIN/oxpulse-turn-upgrade" "$backup/" 2>/dev/null || true
fi

# ---- apply (delegates to install.sh --files-only) ----
log "applying new artifacts"
TURN_SECRET=SKIP REGION=SKIP PRIORITY=SKIP \
  bash "$stage/install.sh" --files-only --from-dir "$stage" \
  || die "install.sh --files-only failed"

echo "$VERSION" > "$PREFIX_SHARE/VERSION"

# ---- restart + probe ----
log "restarting coturn"
if ! systemctl restart coturn; then
  warn "restart failed — rolling back"
  [[ -n "${backup:-}" ]] && cp -a "$backup"/* "$PREFIX_SHARE"/ || true
  systemctl restart coturn || die "rollback also failed — manual recovery required"
  die "upgrade rolled back"
fi

sleep 2
if ! "$PREFIX_SBIN/oxpulse-turn-healthcheck"; then
  warn "healthcheck failed — rolling back"
  [[ -n "${backup:-}" ]] && cp -a "$backup"/* "$PREFIX_SHARE"/ && systemctl restart coturn
  die "upgrade rolled back due to post-upgrade healthcheck failure"
fi

log "upgraded to turn-node v${VERSION} successfully"
```

- [ ] **Step 2: Verify `install.sh` already references upgrade.sh + systemd upgrade units**

The installer committed in Task 5 already has `--files-only` / `--from-dir` flags, the `$FILES_ONLY` gates on env/firewall/start, and `FETCH upgrade.sh`/`FETCH systemd/oxpulse-turn-upgrade.{service,timer}` lines. This task only has to provide those files, not modify `install.sh`.

Quick sanity check:
```bash
grep -nE 'FILES_ONLY|from-dir|upgrade\.sh|oxpulse-turn-upgrade\.(service|timer)' deploy/turn-node/install.sh
```
Expected: matches for the flag parser, the `$FILES_ONLY` gates, and the three FETCH lines. If any are missing, fix `install.sh` now — the skeleton was meant to land intact in Task 5.

- [ ] **Step 3: Write `deploy/turn-node/systemd/oxpulse-turn-upgrade.service`**

```ini
[Unit]
Description=oxpulse-chat TURN node — check + apply release upgrades
Documentation=https://github.com/anatolykoptev/oxpulse-chat/tree/main/deploy/turn-node
Wants=network-online.target
After=network-online.target

[Service]
Type=oneshot
ExecStart=/usr/local/sbin/oxpulse-turn-upgrade
# Network flaps shouldn't brick a restart — never propagate failure to a restart loop.
SuccessExitStatus=0 10
StandardOutput=journal
StandardError=journal
```

- [ ] **Step 4: Write `deploy/turn-node/systemd/oxpulse-turn-upgrade.timer`**

```ini
[Unit]
Description=oxpulse-chat TURN node — nightly upgrade check (disabled by default)
Documentation=https://github.com/anatolykoptev/oxpulse-chat/tree/main/deploy/turn-node

[Timer]
# Every day at a randomised time in a 03:00-05:00 window (partner-local).
OnCalendar=*-*-* 03:00:00
RandomizedDelaySec=2h
Persistent=true
Unit=oxpulse-turn-upgrade.service

[Install]
WantedBy=timers.target
```

Installer lays this down but **does not enable it** — partners opt in with `systemctl enable --now oxpulse-turn-upgrade.timer`. Auto-applying config during call hours without consent is a discourtesy; this reflects the `brainstorming`-level decision to make upgrades pull-not-push.

- [ ] **Step 5: Shellcheck**

```bash
shellcheck -x deploy/turn-node/upgrade.sh deploy/turn-node/install.sh
```

- [ ] **Step 6: Dry-run `--check` on rvpn** (validates the GH API path even before a real release exists — will print `cannot resolve latest release` cleanly)

```bash
rsync -av --delete deploy/turn-node/ rvpn:/root/oxpulse-turn-node/
ssh rvpn 'bash /root/oxpulse-turn-node/upgrade.sh --check' || true
```
Expected before first release: exits with `cannot resolve latest release`. After first release exists: `UPGRADE_AVAILABLE` or `already on X — nothing to do`.

- [ ] **Step 7: Commit**

```bash
git add deploy/turn-node/upgrade.sh deploy/turn-node/systemd/oxpulse-turn-upgrade.*
git add deploy/turn-node/install.sh   # the --files-only patch
git commit -m "feat(turn-node): pull-based upgrader with SHA256 verify + rollback"
```

---

## Task 11: Documentation update + merge

**Files:**
- Modify: `docs/partners/onboarding.md`

- [ ] **Step 1: Open `docs/partners/onboarding.md`** — rewrite §4-§7 to point at the installer.

Replace sections §4 (Install coturn), §5 (turnserver.conf), §6 (Firewall), §7 (Start and verify) with:

```markdown
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
- detects the distro, installs `coturn`, `chrony`, and a firewall tool;
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
```

Keep §1-§3 (Purpose, Architecture, Prerequisites), §8 (Register with operator — still the protocol), §9 (Verify the node is live), §10 (Drain), §11 (Incident response), §12 (Appendix) unchanged.

- [ ] **Step 2: Add a line to repo root `README.md`** in the "Running a TURN relay" section (or create one in the Deployment block) pointing to `deploy/turn-node/README.md`.

- [ ] **Step 3: Commit docs**

```bash
git add docs/partners/onboarding.md README.md
git commit -m "docs(partners): point onboarding at the new installer"
```

- [ ] **Step 4: Update partner README to reference tagged releases**

Edit `deploy/turn-node/README.md` "Quick start" section — replace the raw.githubusercontent.com URL with a release-pinned one:

```bash
# Replace the raw.githubusercontent.com line with:
curl -fsSL https://github.com/anatolykoptev/oxpulse-chat/releases/latest/download/turn-node-installer.sh \
  | TURN_SECRET='<shared-secret>' REGION='ru-msk' bash
```

Also add the upgrade/verification note:

```markdown
## Upgrading

Pull + verify + apply the latest release:
```bash
oxpulse-turn-upgrade           # latest
oxpulse-turn-upgrade --check   # check without applying (exit 10 if upgrade pending)
oxpulse-turn-upgrade turn-node-v1.2.3   # pin to specific version
```

Enable nightly auto-check (opt-in — disabled by default):
```bash
systemctl enable --now oxpulse-turn-upgrade.timer
```
```

Note: `turn-node-installer.sh` as a release asset is added by extending Task 9's workflow to also upload `deploy/turn-node/install.sh` renamed to `turn-node-installer.sh` (one extra `gh release upload` line). Make that extension when the tag-triggered run lands — the plan's Task 9 already wires `--clobber`, so subsequent patches are safe.

- [ ] **Step 5: Open PR**

```bash
git push -u origin feat/turn-node-template
gh pr create --title "feat(turn-node): one-command partner installer + release-please" \
  --body "$(cat <<'EOF'
## Summary
- New `deploy/turn-node/` directory with idempotent multi-distro installer (Debian/Ubuntu + RHEL family autodetect)
- Native coturn + systemd (no Docker) — template renders `/etc/turnserver.conf` from per-node env
- Autodetect public/private IPv4 with clean override path; clone-safe via `/etc/default/oxpulse-turn`
- Validated end-to-end on CentOS Stream 9 (`call.rvpn.online`, 70.34.243.184)
- **release-please** wired for the `turn-node` component — Conventional Commits bump version, open Release PR, tag on merge
- **`turn-node-release.yml`** workflow packages + SHA256 + attaches assets to the GitHub Release on tag push
- **`oxpulse-turn-upgrade`** partner-side tool: pulls from GH Releases API, verifies SHA256, backs up + rollback on failure; dormant systemd timer for opt-in nightly checks
- Operator runbook rewritten to one-command install

## Test plan
- [x] `shellcheck -x` clean for all scripts (install, render, upgrade, healthcheck, autodetect)
- [x] `install.sh` ran successfully on CentOS Stream 9 (rvpn)
- [x] `healthcheck.sh` passes 7/7 on rvpn
- [x] External `turnutils_stunclient` prints mapped address
- [x] `turnutils_uclient` with real credentials from `/api/turn-credentials` authenticates against the new relay
- [x] Re-running `install.sh` is idempotent (env file preserved)
- [x] `oxpulse-turn-upgrade --check` runs cleanly (reports no release available or surfaces pending)
- [ ] After merge: release-please opens `chore(turn-node): release 1.0.0` PR; merging it tags + triggers artifact upload
- [ ] After release-please merge: `oxpulse-turn-upgrade` on rvpn upgrades from 0.0.0 → 1.0.0 and healthcheck still passes
- [ ] Snapshot/clone flow exercised (can be deferred to partner)

🤖 Generated with [Claude Code](https://claude.com/claude-code)
EOF
)"
```

---

## Self-Review

**Spec coverage:**

| User requirement                                          | Covered by   |
|-----------------------------------------------------------|--------------|
| Multi-distro autodetect (CentOS-like + Debian-like)       | Task 5, §1   |
| Test on call.rvpn.online                                  | Task 6       |
| One shared `TURN_SECRET`, easy migration between servers  | Tasks 3, 5   |
| "Slepok" (snapshot) replication across partner fleet      | Tasks 5, 6, Task 11 (README §"Cloning") |
| Senior-grade production hardening                         | Tasks 3 (anti-SSRF, logs, perms), 4 (systemd Restart, chrony dep), 5 (idempotency, firewalld/ufw auto), 10 (rollback on failed upgrade) |
| Research-informed (BigBlueButton, LiveKit, Matrix)        | Architecture section + native-coturn choice |
| GoReleaser-style auto-releases from GitHub                | Task 8 (release-please) + Task 9 (artifact build) |
| Partner-side upgrades from GitHub Releases                | Task 10 (pull-based upgrader, SHA256 verify, rollback, opt-in timer) |

**Placeholder scan:** no `TBD`, no "add appropriate error handling", no bare "similar to Task N" — every file body is verbatim. The only `<REPLACE_ME>` tokens are in `oxpulse-turn.env.example`, which is intentional (that file ships unredacted as the template).

**Type / name consistency:**
- `/etc/default/oxpulse-turn` used consistently across Tasks 3, 4, 5, 6, 8.
- Script names `oxpulse-turn-render`, `oxpulse-turn-healthcheck` match across installer, healthcheck script, systemd unit, and docs.
- Env var names `TURN_SECRET`, `REGION`, `PRIORITY`, `REALM`, `PUBLIC_IPV4`, `PRIVATE_IPV4`, `PUBLIC_HOST` appear identically in README, `oxpulse-turn.env.example`, `install.sh`, and `healthcheck.sh`.
- Registration format `REGION:PRIORITY:turn:HOST:PORT?transport=udp` matches `crates/server/src/config.rs::parse_turn_servers` (verified in code_search output).

---

## Execution Handoff

Plan complete and saved. Per the repo's CLAUDE.md rule *"Execution mode: always Subagent-Driven"* — next step is `superpowers:subagent-driven-development` unless you override.
