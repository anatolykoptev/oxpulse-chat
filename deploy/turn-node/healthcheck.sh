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
check "oxpulse-turn-render ran"        bash -c 'systemctl show oxpulse-turn-render.service --property=Result | grep -q "Result=success"'
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
