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
# shellcheck source=/dev/null
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
    dnf install -y coturn coturn-utils chrony gettext curl ca-certificates iproute
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
  # Ensure log/run dirs exist before the systemd service bind-mounts them
  # (ProtectSystem=strict requires ReadWritePaths to exist on the host).
  install -d -m 0750 -o coturn -g coturn /var/log/turnserver /var/run/turnserver 2>/dev/null || \
    install -d -m 0750 /var/log/turnserver /var/run/turnserver

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
