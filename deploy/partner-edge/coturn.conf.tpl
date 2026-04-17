# Rendered by install.sh → /etc/oxpulse-partner-edge/coturn.conf
# DO NOT EDIT DIRECTLY — regenerated on reinstall / upgrade.
#
# Pattern mirrored from deploy/turn-node/templates/turnserver.conf.tmpl:
#   HMAC shared-secret auth, anti-SSRF denylist, UDP-only media relay.
# Placeholders: {{TURN_SECRET}} {{PARTNER_DOMAIN}} {{PUBLIC_IP}} {{EXTERNAL_IP_LINE}}

listening-port=3478
alt-listening-port=3479

# TURNS on 5349 (TLS) — Caddy occupies 443, so TURNS-on-443 is not possible
# on a co-brand edge. Clients fall back to UDP 3478 which almost always works
# since the edge itself is reachable on 443 for signaling.
tls-listening-port=5349

fingerprint
lt-cred-mech
use-auth-secret
static-auth-secret={{TURN_SECRET}}
realm={{PARTNER_DOMAIN}}

total-quota=200
stale-nonce=600
no-loopback-peers
no-multicast-peers

# No built-in cert on partner edge — TURNS uses the Caddy-managed cert path
# mounted as a volume (optional; falls back to plaintext 3478 if cert missing).
# Partner can mount certs via docker-compose override to enable TURNS.
no-tls
no-dtls

# Block TCP relay — forces UDP-only media, what clients actually use.
no-tcp-relay

# Anti-SSRF: deny relay into RFC1918 + link-local + CGNAT + loopback.
denied-peer-ip=0.0.0.0-0.255.255.255
denied-peer-ip=10.0.0.0-10.255.255.255
denied-peer-ip=100.64.0.0-100.127.255.255
denied-peer-ip=127.0.0.0-127.255.255.255
denied-peer-ip=169.254.0.0-169.254.255.255
denied-peer-ip=172.16.0.0-172.31.255.255
denied-peer-ip=192.0.0.0-192.0.0.255
denied-peer-ip=192.0.2.0-192.0.2.255
denied-peer-ip=192.168.0.0-192.168.255.255
denied-peer-ip=198.18.0.0-198.19.255.255
denied-peer-ip=198.51.100.0-198.51.100.255
denied-peer-ip=203.0.113.0-203.0.113.255
denied-peer-ip=224.0.0.0-239.255.255.255
denied-peer-ip=240.0.0.0-255.255.255.255

# Relay port range — must match firewall rule in install.sh.
min-port=49152
max-port=65535

external-ip={{EXTERNAL_IP_LINE}}

# Logging — container writes to mounted /var/log/turnserver.
log-file=/var/log/turnserver/turn.log
pidfile=/var/run/turnserver/turnserver.pid
no-stdout-log
simple-log
syslog

# We own this file; don't load extras.
no-cli
