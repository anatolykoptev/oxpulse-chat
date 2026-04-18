#!/usr/bin/env bash
# install.sh — one-command bootstrap for an oxpulse-chat partner edge node.
#
#   curl -fsSL https://install.oxpulse.chat/partner | sudo bash -s -- \
#     --domain=call.rvpn.online --partner-id=rvpn --token=ptkn_xxx
#
# Manual-config fallback (until /api/partner/register lands — Task 4):
#   sudo bash install.sh --domain=call.rvpn.online --partner-id=rvpn \
#        --manual-config=./node-config.json
#
# The manual-config JSON schema is documented in README.md.
set -euo pipefail

# ---------- Constants ----------
PREFIX_ETC=/etc/oxpulse-partner-edge
PREFIX_LIB=/var/lib/oxpulse-partner-edge
PREFIX_SBIN=/usr/local/sbin
SYSTEMD_DIR=/etc/systemd/system
# shellcheck disable=SC2034  # REGISTRY referenced by templates via IMAGE_VERSION, kept for override env surface
REGISTRY="${OXPULSE_IMAGE_REGISTRY:-ghcr.io/anatolykoptev}"
REPO_RAW="${OXPULSE_REPO_RAW:-https://raw.githubusercontent.com/anatolykoptev/oxpulse-chat/main/deploy/partner-edge}"
BACKEND_API="${OXPULSE_BACKEND_API:-https://api.oxpulse.chat}"

log()  { printf '\033[32m==>\033[0m %s\n' "$*" >&2; }
warn() { printf '\033[33m!!\033[0m  %s\n' "$*" >&2; }
die()  { printf '\033[31mERR\033[0m %s\n' "$*" >&2; exit 1; }

# ---------- Args ----------
DOMAIN=""
PARTNER_ID=""
TOKEN=""
TUNNEL=vless
MANUAL_CONFIG=""
IMAGE_VERSION="${OXPULSE_IMAGE_VERSION:-latest}"
# v0.2.0-rc1 placeholder: real per-clone value comes from /api/partner/register
# response rendered by hydrate.sh in Phase 6 (Task 5.2).
TURNS_SUBDOMAIN="${TURNS_SUBDOMAIN:-turns}"
DRY_RUN=0

usage() {
	sed -n '2,18p' "$0" >&2
	cat >&2 <<USAGE

Required:
  --domain=<fqdn>            Partner edge domain (must resolve to this host's public IP)
  --partner-id=<id>          Short partner identifier (e.g. rvpn, piter)

Registration (pick one):
  --token=<ptkn_...>         Fetch node config from $BACKEND_API/api/partner/register
  --manual-config=<path>     Read node config from a local JSON file

Optional:
  --tunnel=vless|wg|https    Backend tunnel kind (default: vless)
  --image-version=<tag>      Pull a specific image tag (default: latest)
  --dry-run                  Render templates + print plan, skip docker/systemd
  -h|--help                  Show this help

Env overrides: OXPULSE_IMAGE_REGISTRY, OXPULSE_BACKEND_API, OXPULSE_REPO_RAW
USAGE
	exit 2
}

while [[ $# -gt 0 ]]; do
	case "$1" in
		--domain=*)         DOMAIN="${1#*=}" ;;
		--partner-id=*)     PARTNER_ID="${1#*=}" ;;
		--token=*)          TOKEN="${1#*=}" ;;
		--manual-config=*)  MANUAL_CONFIG="${1#*=}" ;;
		--tunnel=*)         TUNNEL="${1#*=}" ;;
		--image-version=*)  IMAGE_VERSION="${1#*=}" ;;
		--dry-run)          DRY_RUN=1 ;;
		-h|--help)          usage ;;
		*) die "unknown arg: $1 (try --help)" ;;
	esac
	shift
done

[[ -z "$DOMAIN" ]]     && die "--domain is required"
[[ -z "$PARTNER_ID" ]] && die "--partner-id is required"
if [[ -z "$TOKEN" && -z "$MANUAL_CONFIG" ]]; then
	die "either --token or --manual-config is required (see --help)"
fi
case "$TUNNEL" in
	vless|wg|https) : ;;
	*) die "--tunnel must be one of: vless, wg, https" ;;
esac

if [[ $DRY_RUN -eq 0 && $EUID -ne 0 ]]; then
	die "must run as root (or with sudo) unless --dry-run"
fi

# ---------- Step 1: preflight ----------
log "[1/10] preflight checks"
OS_ID=""; OS_FAMILY=""
if [[ -r /etc/os-release ]]; then
	# shellcheck source=/dev/null
	. /etc/os-release
	OS_ID="$ID"
	case " $ID ${ID_LIKE:-} " in
		*" debian "*|*" ubuntu "*) OS_FAMILY=debian ;;
		*" rhel "*|*" fedora "*|*" centos "*|*" almalinux "*|*" rocky "*) OS_FAMILY=rhel ;;
		*) die "unsupported OS: ID=$ID ID_LIKE=${ID_LIKE:-<empty>} (need Debian/Ubuntu/AlmaLinux/Rocky/RHEL)" ;;
	esac
fi
log "  os=$OS_ID family=$OS_FAMILY"

if [[ $DRY_RUN -eq 0 ]]; then
	check_port_free() {
		local port=$1 proto=$2
		if ss -ln"${proto}" 2>/dev/null | awk '{print $4}' | grep -qE "[:.]${port}\$"; then
			die "port $port/$proto is already in use — free it before installing"
		fi
	}
	for p in 80 443 3478 5349; do check_port_free "$p" t; done
	check_port_free 3478 u
	log "  ports 80/443/3478/5349 are free"
fi

# ---------- Step 2: Docker ----------
log "[2/10] ensuring docker + compose plugin"
if [[ $DRY_RUN -eq 0 ]]; then
	if ! command -v docker >/dev/null 2>&1; then
		log "  docker not found — installing via get.docker.com"
		curl -fsSL --proto '=https' --tlsv1.2 https://get.docker.com -o /tmp/get-docker.sh
		sh /tmp/get-docker.sh
		rm -f /tmp/get-docker.sh
	fi
	if ! docker compose version >/dev/null 2>&1; then
		if [[ $OS_FAMILY == debian ]]; then
			apt-get update -q && apt-get install -y -q docker-compose-plugin
		else
			dnf install -y docker-compose-plugin || dnf install -y docker-compose
		fi
	fi
	systemctl enable --now docker
	log "  docker $(docker --version | awk '{print $3}' | tr -d ,) ready"
else
	warn "  [dry-run] skipping docker install"
fi

# ---------- Step 3: public/private IP autodetect ----------
log "[3/10] detecting IPs"
_detect_public_ipv4() {
	local ip
	ip=$(curl -fsS --max-time 2 http://169.254.169.254/latest/meta-data/public-ipv4 2>/dev/null || true)
	if [[ "$ip" =~ ^([0-9]{1,3}\.){3}[0-9]{1,3}$ ]]; then printf '%s' "$ip"; return 0; fi
	ip=$(curl -fsS --max-time 3 https://api.ipify.org 2>/dev/null || true)
	if [[ "$ip" =~ ^([0-9]{1,3}\.){3}[0-9]{1,3}$ ]]; then printf '%s' "$ip"; return 0; fi
	ip=$(curl -fsS --max-time 3 https://ifconfig.me 2>/dev/null || true)
	if [[ "$ip" =~ ^([0-9]{1,3}\.){3}[0-9]{1,3}$ ]]; then printf '%s' "$ip"; return 0; fi
	return 1
}
PUBLIC_IP="${OXPULSE_PUBLIC_IP:-}"
[[ -z "$PUBLIC_IP" ]] && PUBLIC_IP=$(_detect_public_ipv4 || true)
[[ -z "$PUBLIC_IP" ]] && die "unable to autodetect public IP — set OXPULSE_PUBLIC_IP"
PRIVATE_IP="${OXPULSE_PRIVATE_IP:-}"
if [[ -z "$PRIVATE_IP" ]]; then
	iface=$(ip -4 route show default 2>/dev/null | awk '/default/ {print $5; exit}')
	if [[ -n "$iface" ]]; then
		cand=$(ip -4 -o addr show dev "$iface" 2>/dev/null | awk '{print $4}' | cut -d/ -f1 | head -1 || true)
		[[ "$cand" != "$PUBLIC_IP" ]] && PRIVATE_IP="$cand"
	fi
fi
log "  public=$PUBLIC_IP private=${PRIVATE_IP:-<none>}"

# ---------- Step 4: fetch node config ----------
log "[4/10] fetching node config"
tmp_cfg=$(mktemp)
trap 'rm -f "$tmp_cfg"' EXIT
if [[ -n "$MANUAL_CONFIG" ]]; then
	[[ -r "$MANUAL_CONFIG" ]] || die "manual-config file not readable: $MANUAL_CONFIG"
	cp "$MANUAL_CONFIG" "$tmp_cfg"
	log "  using manual config: $MANUAL_CONFIG"
else
	log "  POST $BACKEND_API/api/partner/register"
	if ! curl -fsSL --proto '=https' --tlsv1.2 --max-time 15 \
		-X POST "$BACKEND_API/api/partner/register" \
		-H 'Content-Type: application/json' \
		-d "{\"partner_id\":\"$PARTNER_ID\",\"domain\":\"$DOMAIN\",\"token\":\"$TOKEN\",\"public_ip\":\"$PUBLIC_IP\"}" \
		-o "$tmp_cfg"; then
		die "registration failed — endpoint may not yet be implemented (Task 4). Retry with --manual-config=<path>"
	fi
fi

# jq-free JSON extraction (small fixed schema).
json_get() {
	local key=$1 file=$2
	python3 -c "import json,sys; d=json.load(open(sys.argv[1])); print(d.get(sys.argv[2],''))" "$file" "$key" 2>/dev/null \
		|| sed -nE "s/.*\"$key\"[[:space:]]*:[[:space:]]*\"([^\"]*)\".*/\1/p" "$file" | head -1
}
NODE_ID=$(json_get node_id "$tmp_cfg")
BACKEND_ENDPOINT=$(json_get backend_endpoint "$tmp_cfg")
TURN_SECRET=$(json_get turn_secret "$tmp_cfg")
REALITY_UUID=$(json_get reality_uuid "$tmp_cfg")
REALITY_PUBLIC_KEY=$(json_get reality_public_key "$tmp_cfg")
REALITY_SHORT_ID=$(json_get reality_short_id "$tmp_cfg")
REALITY_SERVER_NAME=$(json_get reality_server_name "$tmp_cfg")
[[ -z "$NODE_ID" ]]            && NODE_ID="${PARTNER_ID}-$(hostname -s)"
[[ -z "$BACKEND_ENDPOINT" ]]   && die "backend_endpoint missing from config"
[[ -z "$TURN_SECRET" ]]        && die "turn_secret missing from config"
[[ -z "$REALITY_UUID" ]]       && die "reality_uuid missing from config"
[[ -z "$REALITY_PUBLIC_KEY" ]] && die "reality_public_key missing from config"
[[ -z "$REALITY_SHORT_ID" ]]   && die "reality_short_id missing from config"
[[ -z "$REALITY_SERVER_NAME" ]] && REALITY_SERVER_NAME="www.samsung.com"

# Split backend_endpoint "host:port" into host + port for xray config.
BACKEND_HOST="${BACKEND_ENDPOINT%:*}"
BACKEND_PORT="${BACKEND_ENDPOINT##*:}"
if [[ "$BACKEND_HOST" == "$BACKEND_PORT" || -z "$BACKEND_PORT" ]]; then
	die "backend_endpoint must be host:port (got '$BACKEND_ENDPOINT')"
fi

# EXTERNAL_IP_LINE for coturn — "public/private" if behind NAT, else "public".
if [[ -n "${PRIVATE_IP:-}" ]]; then
	EXTERNAL_IP_LINE="${PUBLIC_IP}/${PRIVATE_IP}"
else
	EXTERNAL_IP_LINE="${PUBLIC_IP}"
fi

# ---------- Step 5: stage templates ----------
log "[5/10] rendering templates"
if [[ $DRY_RUN -eq 0 ]]; then
	install -d -m 0755 "$PREFIX_ETC"
	install -d -m 0700 "$PREFIX_LIB"
fi

src_dir=""
if [[ -f "$(cd "$(dirname "${BASH_SOURCE[0]}")" 2>/dev/null && pwd)/docker-compose.yml.tpl" ]]; then
	src_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
	log "  using templates from local checkout: $src_dir"
fi

fetch_tpl() {
	local name=$1 dst=$2
	if [[ -n "$src_dir" && -f "$src_dir/$name" ]]; then
		cp "$src_dir/$name" "$dst"
	else
		curl -fsSL "$REPO_RAW/$name" -o "$dst"
	fi
}

stage=$(mktemp -d)
fetch_tpl docker-compose.yml.tpl "$stage/compose.tpl"
fetch_tpl Caddyfile.tpl          "$stage/caddy.tpl"
fetch_tpl xray-client.json.tpl   "$stage/xray.tpl"
fetch_tpl coturn.conf.tpl        "$stage/coturn.tpl"

render() {
	local src=$1 dst=$2
	# Mustache-style placeholder substitution via sed. No external deps.
	sed \
		-e "s|{{PARTNER_ID}}|${PARTNER_ID}|g" \
		-e "s|{{PARTNER_DOMAIN}}|${DOMAIN}|g" \
		-e "s|{{BACKEND_ENDPOINT}}|${BACKEND_ENDPOINT}|g" \
		-e "s|{{BACKEND_HOST}}|${BACKEND_HOST}|g" \
		-e "s|{{BACKEND_PORT}}|${BACKEND_PORT}|g" \
		-e "s|{{TURN_SECRET}}|${TURN_SECRET}|g" \
		-e "s|{{REALITY_UUID}}|${REALITY_UUID}|g" \
		-e "s|{{REALITY_PUBLIC_KEY}}|${REALITY_PUBLIC_KEY}|g" \
		-e "s|{{REALITY_SHORT_ID}}|${REALITY_SHORT_ID}|g" \
		-e "s|{{REALITY_SERVER_NAME}}|${REALITY_SERVER_NAME}|g" \
		-e "s|{{PUBLIC_IP}}|${PUBLIC_IP}|g" \
		-e "s|{{PRIVATE_IP}}|${PRIVATE_IP:-}|g" \
		-e "s|{{EXTERNAL_IP_LINE}}|${EXTERNAL_IP_LINE}|g" \
		-e "s|{{IMAGE_VERSION}}|${IMAGE_VERSION}|g" \
		"$src" > "$dst"
}

compose_out="$PREFIX_ETC/docker-compose.yml"
caddy_out="$PREFIX_ETC/Caddyfile"
xray_out="$PREFIX_ETC/xray-client.json"
coturn_out="$PREFIX_ETC/coturn.conf"

if [[ $DRY_RUN -eq 1 ]]; then
	# Render to /tmp so caller can inspect without root.
	dryroot=$(mktemp -d)
	compose_out="$dryroot/docker-compose.yml"
	caddy_out="$dryroot/Caddyfile"
	xray_out="$dryroot/xray-client.json"
	coturn_out="$dryroot/coturn.conf"
fi
render "$stage/compose.tpl" "$compose_out"
render "$stage/caddy.tpl"   "$caddy_out"
render "$stage/xray.tpl"    "$xray_out"
render "$stage/coturn.tpl"  "$coturn_out"
rm -rf "$stage"

# Secrets-containing files → 0600.
chmod 0600 "$xray_out" "$coturn_out" || true
log "  rendered → $compose_out (+ Caddyfile, xray-client.json, coturn.conf)"

# Persist install state for upgrade.sh.
if [[ $DRY_RUN -eq 0 ]]; then
	cat > "$PREFIX_LIB/install.env" <<EOF
PARTNER_ID=$PARTNER_ID
PARTNER_DOMAIN=$DOMAIN
NODE_ID=$NODE_ID
TUNNEL=$TUNNEL
IMAGE_VERSION=$IMAGE_VERSION
TURNS_SUBDOMAIN=$TURNS_SUBDOMAIN
INSTALLED_AT=$(date -u +%Y-%m-%dT%H:%M:%SZ)
EOF
	chmod 0600 "$PREFIX_LIB/install.env"
fi

# ---------- Step 6: pull images ----------
log "[6/10] pulling images (tag=$IMAGE_VERSION)"
if [[ $DRY_RUN -eq 0 ]]; then
	(cd "$PREFIX_ETC" && docker compose pull)
else
	warn "  [dry-run] would: docker compose pull"
fi

# ---------- Step 7: start ----------
log "[7/10] starting services"
if [[ $DRY_RUN -eq 0 ]]; then
	(cd "$PREFIX_ETC" && docker compose up -d)
else
	warn "  [dry-run] would: docker compose up -d"
fi

# ---------- Step 8: healthcheck ----------
log "[8/10] waiting for healthcheck (timeout 120s)"
if [[ $DRY_RUN -eq 0 ]]; then
	deadline=$(( $(date +%s) + 120 ))
	hc_script="$PREFIX_SBIN/oxpulse-partner-edge-healthcheck"
	# Ship healthcheck.sh into /usr/local/sbin too so systemd + manual runs both work.
	if [[ -n "$src_dir" && -f "$src_dir/healthcheck.sh" ]]; then
		install -m 0755 "$src_dir/healthcheck.sh" "$hc_script"
	else
		curl -fsSL "$REPO_RAW/healthcheck.sh" -o "$hc_script"
		chmod 0755 "$hc_script"
	fi
	while :; do
		if OXPULSE_EDGE_CONFIG_DIR="$PREFIX_ETC" "$hc_script" --local >/dev/null 2>&1; then
			log "  healthcheck green"
			break
		fi
		if (( $(date +%s) > deadline )); then
			warn "  healthcheck still red after 120s — continuing, inspect with: $hc_script"
			break
		fi
		sleep 3
	done
else
	warn "  [dry-run] skipping healthcheck"
fi

# ---------- Step 9: systemd ----------
log "[9/10] installing systemd unit"
if [[ $DRY_RUN -eq 0 ]]; then
	unit_src=""
	if [[ -n "$src_dir" && -f "$src_dir/systemd/oxpulse-partner-edge.service" ]]; then
		unit_src="$src_dir/systemd/oxpulse-partner-edge.service"
		install -m 0644 "$unit_src" "$SYSTEMD_DIR/oxpulse-partner-edge.service"
	else
		curl -fsSL "$REPO_RAW/systemd/oxpulse-partner-edge.service" \
			-o "$SYSTEMD_DIR/oxpulse-partner-edge.service"
	fi
	# Upgrade script into /usr/local/sbin.
	if [[ -n "$src_dir" && -f "$src_dir/upgrade.sh" ]]; then
		install -m 0755 "$src_dir/upgrade.sh" "$PREFIX_SBIN/oxpulse-partner-edge-upgrade"
	else
		curl -fsSL "$REPO_RAW/upgrade.sh" -o "$PREFIX_SBIN/oxpulse-partner-edge-upgrade"
		chmod 0755 "$PREFIX_SBIN/oxpulse-partner-edge-upgrade"
	fi
	# Cert-watch units (Task 2A.5): inotify path unit + oneshot signal service.
	# Substitute {{TURNS_SUBDOMAIN}} + {{PARTNER_DOMAIN}} before install.
	for unit in oxpulse-partner-cert-watch.path oxpulse-partner-cert-watch.service; do
		local_src=""
		if [[ -n "$src_dir" && -f "$src_dir/systemd/${unit}" ]]; then
			local_src="$src_dir/systemd/${unit}"
		else
			curl -fsSL "$REPO_RAW/systemd/${unit}" -o "/tmp/${unit}.fetched"
			local_src="/tmp/${unit}.fetched"
		fi
		sed -e "s|{{TURNS_SUBDOMAIN}}|${TURNS_SUBDOMAIN}|g" -e "s|{{PARTNER_DOMAIN}}|${DOMAIN}|g" \
			"$local_src" > "/tmp/${unit}.rendered"
		install -m 0644 "/tmp/${unit}.rendered" "$SYSTEMD_DIR/${unit}"
		rm -f "/tmp/${unit}.rendered" "/tmp/${unit}.fetched"
	done
	systemctl daemon-reload
	systemctl enable --now oxpulse-partner-edge.service
	systemctl enable --now oxpulse-partner-cert-watch.path
else
	warn "  [dry-run] skipping systemd install"
fi

# ---------- Step 10: report ----------
log "[10/10] done"
cat <<BANNER

========================================================================
  OxPulse partner-edge node installed.

  Partner   : $PARTNER_ID
  Node ID   : $NODE_ID
  Domain    : https://$DOMAIN
  Public IP : $PUBLIC_IP
  Tunnel    : $TUNNEL
  Version   : $IMAGE_VERSION
  Config    : $PREFIX_ETC/
  State     : $PREFIX_LIB/install.env

  Verify    : $PREFIX_SBIN/oxpulse-partner-edge-healthcheck
  Upgrade   : $PREFIX_SBIN/oxpulse-partner-edge-upgrade
  Logs      : docker compose -f $PREFIX_ETC/docker-compose.yml logs -f
  Systemd   : systemctl status oxpulse-partner-edge

  Next steps:
  1. Point DNS A record for $DOMAIN → $PUBLIC_IP
  2. Wait for Caddy LE cert issuance (~60s after DNS propagates)
  3. Open https://$DOMAIN and verify branding
========================================================================
BANNER
