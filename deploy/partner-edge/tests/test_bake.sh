#!/bin/bash
# Structural checks: install.sh parses --bake and gates secrets on BAKE_MODE.
set -euo pipefail

REPO_ROOT="${REPO_ROOT:-/home/krolik/src/oxpulse-chat}"
SCRIPT="$REPO_ROOT/deploy/partner-edge/install.sh"

# 1. BAKE_MODE variable exists and defaults to 0.
grep -qE '^BAKE_MODE=0\b' "$SCRIPT" || { echo "FAIL: BAKE_MODE=0 not found"; exit 1; }

# 2. --bake flag is parsed.
grep -qE -- '--bake\)' "$SCRIPT" || { echo "FAIL: --bake case not parsed"; exit 1; }

# 3. The registration call (POST /api/partner/register) is gated behind BAKE_MODE check.
#    Checks curl lines (not log/doc mentions) that reference /api/partner/register.
if ! awk '
    /^if \[ "\$BAKE_MODE" = "0" \]/ { gate=1 }
    /^fi$/ { gate=0 }
    /curl/ && /\/api\/partner\/register/ { if (!gate) { print "register curl outside BAKE_MODE gate at line " NR; exit 1 } }
' "$SCRIPT"; then
    echo "FAIL: /api/partner/register curl call not gated by BAKE_MODE"
    exit 1
fi

# 4. Script still parses cleanly.
bash -n "$SCRIPT" || { echo "FAIL: bash -n failed"; exit 1; }

# 5. Image pre-pull (docker pull) is NOT gated behind BAKE_MODE=0.
#    Bake mode must cache images into the snapshot; gating the pull defeats that.
if awk '
    /^if \[ "\$BAKE_MODE" = "0" \]/ { gate=1 }
    /^fi(\s|$)/ { gate=0 }
    /docker pull / { if (gate) { print "docker pull gated inside BAKE_MODE=0 at line " NR; exit 1 } }
' "$SCRIPT"; then
    :
else
    echo "FAIL: docker pull is gated behind BAKE_MODE=0 — bake cannot cache images"
    exit 1
fi
# Also verify at least one docker pull line exists.
grep -qE 'docker pull ' "$SCRIPT" || { echo "FAIL: no docker pull found in script"; exit 1; }

echo "PASS: install.sh --bake structure present"
