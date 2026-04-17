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
# shellcheck disable=SC2016
envsubst '${TURN_SECRET} ${REALM} ${EXTERNAL_IP_LINE} ${COTURN_USER} ${COTURN_GROUP}' \
  < "$TMPL_FILE" > "$tmp"

# Atomic replace + strict perms (config contains the HMAC secret).
install -o root -g "$COTURN_GROUP" -m 0640 "$tmp" "$OUT_FILE"

# Log dir coturn will write to — package install may have created it but not always.
install -o "$COTURN_USER" -g "$COTURN_GROUP" -m 0750 -d /var/log/turnserver /var/run/turnserver

echo "render-conf: wrote $OUT_FILE (public=$PUBLIC_IPV4 private=${PRIVATE_IPV4:-none} realm=$REALM)"
