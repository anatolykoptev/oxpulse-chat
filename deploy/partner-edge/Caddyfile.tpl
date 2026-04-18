# Rendered by install.sh → /etc/oxpulse-partner-edge/Caddyfile
# Placeholders: {{PARTNER_DOMAIN}}
#
# Traffic split on partner edge:
#   / (SPA)         → xray-client:3080 (backend renders branded index.html)
#   /_app/immutable → xray-client:3080 BUT cached 1 year at Caddy
#   /api/*          → xray-client:3080 with X-Forwarded-Host header
#   /ws/*           → xray-client:3080 (WebSocket upgrade preserved by Caddy)

{
	# Global options — ACME on, admin on localhost only.
	admin localhost:2019
	email admin@{{PARTNER_DOMAIN}}
	servers {
		protocols h1 h2
	}
}

{{PARTNER_DOMAIN}} {
	encode gzip zstd

	header {
		Strict-Transport-Security "max-age=31536000; includeSubDomains"
		X-Content-Type-Options "nosniff"
		Referrer-Policy "no-referrer"
		X-Frame-Options "DENY"
		-Server
		-Via
		-Alt-Svc
	}

	# Cache SvelteKit hashed assets for a year (immutable by filename hash).
	@immutable path_regexp /_app/immutable/.*
	header @immutable Cache-Control "public, max-age=31536000, immutable"

	# API — preserve partner domain so backend branding resolver picks right config.
	reverse_proxy /api/* xray-client:3080 {
		header_up X-Forwarded-Host {{PARTNER_DOMAIN}}
		header_up X-Forwarded-Proto https
		header_up Host oxpulse.chat
	}

	# WebSocket — Caddy auto-upgrades on Upgrade: websocket.
	reverse_proxy /ws/* xray-client:3080 {
		header_up X-Forwarded-Host {{PARTNER_DOMAIN}}
		header_up X-Forwarded-Proto https
		header_up Host oxpulse.chat
	}

	# Event telemetry.
	reverse_proxy /events/* xray-client:3080 {
		header_up X-Forwarded-Host {{PARTNER_DOMAIN}}
		header_up X-Forwarded-Proto https
		header_up Host oxpulse.chat
	}

	# SPA fallback — everything else goes through the tunnel so backend can
	# inject partner branding into index.html before shipping to browser.
	reverse_proxy xray-client:3080 {
		header_up X-Forwarded-Host {{PARTNER_DOMAIN}}
		header_up X-Forwarded-Proto https
		header_up Host oxpulse.chat
	}
}
