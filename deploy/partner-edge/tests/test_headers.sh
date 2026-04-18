#!/bin/bash
# Test: partner-edge Caddy does NOT leak Via or Alt-Svc headers
set -eu
DOMAIN="${1:?partner domain required}"
resp=$(curl -sI "https://${DOMAIN}/" --max-time 10)

if echo "$resp" | grep -qi '^via:'; then
  echo "FAIL: Via header leaked" >&2
  echo "$resp" | grep -i '^via:' >&2
  exit 1
fi

if echo "$resp" | grep -qi '^alt-svc:'; then
  echo "FAIL: Alt-Svc header leaked" >&2
  echo "$resp" | grep -i '^alt-svc:' >&2
  exit 1
fi

echo "PASS: no Via or Alt-Svc in response headers"
