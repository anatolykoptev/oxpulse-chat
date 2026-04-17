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
  curl -fsSL --proto '=https' --tlsv1.2 "https://api.github.com/repos/${REPO_SLUG}/releases" \
    | grep -oE '"tag_name":\s*"turn-node-v[0-9]+\.[0-9]+\.[0-9]+"' \
    | head -1 \
    | sed -E 's/.*"(turn-node-v[0-9.]+)".*/\1/'
}

# shellcheck disable=SC2015  # A&&B||C is intentional: die on empty TARGET
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
curl -fsSL --proto '=https' --tlsv1.2 "${base}/turn-node-${VERSION}.tar.gz" -o "$work/tarball.tgz"
curl -fsSL --proto '=https' --tlsv1.2 "${base}/SHA256SUMS"                  -o "$work/SHA256SUMS"

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
backup=""
if [[ -d "$PREFIX_SHARE" && "$current" != "none" ]]; then
  backup="$BACKUP_DIR/turn-node-${current}-${ts}"
  log "backing up to $backup"
  install -d -m 0700 "$backup"
  cp -a "$PREFIX_SHARE" "$backup/share"
  install -d -m 0755 "$backup/sbin"
  for bin in oxpulse-turn-render oxpulse-turn-healthcheck oxpulse-turn-upgrade; do
    [[ -f "$PREFIX_SBIN/$bin" ]] && cp -a "$PREFIX_SBIN/$bin" "$backup/sbin/$bin"
  done
  install -d -m 0755 "$backup/systemd"
  for u in oxpulse-turn-render.service oxpulse-turn-upgrade.service oxpulse-turn-upgrade.timer; do
    [[ -f "/etc/systemd/system/$u" ]] && cp -a "/etc/systemd/system/$u" "$backup/systemd/$u"
  done
  [[ -f "/etc/systemd/system/coturn.service.d/override.conf" ]] \
    && cp -a "/etc/systemd/system/coturn.service.d/override.conf" "$backup/systemd/coturn-override.conf"
fi

# ---- rollback helper ----
_rollback_from_backup() {
  [[ -z "${backup:-}" || ! -d "$backup" ]] && { warn "no backup available — cannot restore"; return 1; }
  # Restore share prefix atomically
  if [[ -d "$backup/share" ]]; then
    rm -rf "$PREFIX_SHARE"
    cp -a "$backup/share" "$PREFIX_SHARE"
  fi
  # Restore binaries
  for bin in oxpulse-turn-render oxpulse-turn-healthcheck oxpulse-turn-upgrade; do
    [[ -f "$backup/sbin/$bin" ]] && cp -a "$backup/sbin/$bin" "$PREFIX_SBIN/$bin"
  done
  # Restore systemd units
  for u in oxpulse-turn-render.service oxpulse-turn-upgrade.service oxpulse-turn-upgrade.timer; do
    [[ -f "$backup/systemd/$u" ]] && cp -a "$backup/systemd/$u" "/etc/systemd/system/$u"
  done
  if [[ -f "$backup/systemd/coturn-override.conf" ]]; then
    install -d -m 0755 /etc/systemd/system/coturn.service.d
    cp -a "$backup/systemd/coturn-override.conf" "/etc/systemd/system/coturn.service.d/override.conf"
  fi
  systemctl daemon-reload
  return 0
}

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
  if _rollback_from_backup; then
    systemctl restart coturn || die "rollback also failed — manual recovery required"
    die "upgrade rolled back"
  else
    die "no backup to restore from; upgrade state is inconsistent — manual recovery required"
  fi
fi

sleep 2
if ! "$PREFIX_SBIN/oxpulse-turn-healthcheck"; then
  warn "healthcheck failed — rolling back"
  if _rollback_from_backup; then
    systemctl restart coturn || die "rollback restart also failed — manual recovery required"
    die "upgrade rolled back due to post-upgrade healthcheck failure"
  else
    die "no backup; upgrade state is inconsistent — manual recovery required"
  fi
fi

log "upgraded to turn-node v${VERSION} successfully"
