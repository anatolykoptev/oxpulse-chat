# Rendered by install.sh → /etc/oxpulse-partner-edge/Caddyfile
# Placeholders: {{PARTNER_DOMAIN}}, {{TURNS_SUBDOMAIN}}
#
# Traffic split on partner edge:
#   / (SPA)         → xray-client:3080 (backend renders branded index.html)
#   /_app/immutable → xray-client:3080 BUT cached 1 year at Caddy
#   /api/*          → xray-client:3080 with X-Forwarded-Host header
#   /ws/*           → xray-client:3080 (WebSocket upgrade preserved by Caddy)
#   {{TURNS_SUBDOMAIN}}.{{PARTNER_DOMAIN}} TLS passthrough → coturn:5349
#
# caddy-l4 TURNS SNI mux: listener_wrappers peeks TLS ClientHello BEFORE Caddy
# HTTP app sees it. Matching SNI → raw TCP to coturn (coturn terminates own TLS).
# Any other SNI → falls through to HTTP app.

{
    # Global options
    admin localhost:2019
    email admin@{{PARTNER_DOMAIN}}

    # NOTE: listener_wrappers MUST be at global servers{} scope — not in a
    # site-level snippet (Phase 2 PoC confirmed). layer4 applies to the listener
    # itself, not to per-site handlers.
    servers {
        # H3/QUIC disabled — ТСПУ entropy heuristic target (R1 Layer 0).
        protocols h1 h2
        listener_wrappers {
            layer4 {
                @turns tls sni {{TURNS_SUBDOMAIN}}.{{PARTNER_DOMAIN}}
                route @turns {
                    # coturn listens on 127.0.0.1:5349 (host network mode).
                    # caddy-l4 forwards raw TLS TCP — coturn terminates its
                    # own TLS on the Caddy-issued cert mounted read-only.
                    proxy tcp/127.0.0.1:5349
                }
            }
            tls
        }
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

    # R1 Layer 2 — Active-probing defense (design §5.3).
    # Requests without a valid oxpulse session cookie or matching Origin
    # header receive a generic cover page instead of the SPA. Makes the
    # endpoint look like a dormant domain to automated DPI probers that
    # would otherwise fingerprint the real app and flag the IP within
    # ~20 seconds (Tor/Hetzner precedent, scratch/E §Active probing).
    @probe {
        not header Origin https://{{PARTNER_DOMAIN}}
        not header Cookie *oxpulse_session=*
        path /
        method GET
    }
    handle @probe {
        root * /srv/cover
        rewrite * /cover.html
        file_server
    }

    # All other paths/methods/authenticated → normal flow.
    handle {
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
}

# Stub vhost — Caddy issues + renews cert for TURNS subdomain via ACME
# HTTP-01 on :80 (Caddy still owns :80 unmultiplexed). The cert is written
# to the caddy-data volume and bind-mounted read-only into coturn.
# Actual :443 traffic for this SNI is routed by caddy-l4 above → coturn
# before this handler ever sees it — so this respond only fires for
# ACME-challenge + any stray request that bypassed the l4 mux.
{{TURNS_SUBDOMAIN}}.{{PARTNER_DOMAIN}} {
    tls {
        issuer acme {
            # CRITICAL: once l4 routes :443 for this SNI to coturn, Caddy can
            # no longer answer TLS-ALPN-01 (which would come in on :443).
            # Force HTTP-01 which uses :80 (Caddy still owns :80).
            # Silent failure if missing: cert renewal stops after 90 days.
            # Source: scratch/B-certs-client-security.md §1.1
            disable_tlsalpn_challenge
        }
    }
    respond 421
}
