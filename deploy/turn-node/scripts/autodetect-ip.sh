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
    # shellcheck disable=SC2317  # exit 1 is the fallback when not sourced
    return 1 2>/dev/null || exit 1
  }
fi
if [[ -z "${PRIVATE_IPV4:-}" ]]; then
  PRIVATE_IPV4=$(_detect_private_ipv4 || true)
fi
export PUBLIC_IPV4 PRIVATE_IPV4
