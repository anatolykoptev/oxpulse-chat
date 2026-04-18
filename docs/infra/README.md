# Caddy Config Notes — Front-end HTTPS/TLS

## Overview

The reverse-proxy / TLS layer in front of `oxpulse.chat` and related
services is handled by a native Caddy instance (not containerized). Live
config location: `/etc/caddy/Caddyfile` on the operator host.

## Why the snapshot moved

An earlier version of this directory contained
`caddyfile-krolik-server.snapshot.conf` — an authoritative copy of the
Caddy config tracked in this public repo. That snapshot has been
**moved to a private operator-only repo** because it revealed:

- Full vhost map of the operator's personal infrastructure
- Internal service names and ports
- Trusted-proxy IP allowlist (operational info not useful to public
  contributors)

The public version of this file now only documents the general patterns
and operator workflow; the exact hostnames and IPs that are specific to
one deployment live in the private operator playbook.

## Secrets

Bearer tokens referenced in the Caddyfile use Caddy's parse-time env syntax:

```
"Bearer {$SOME_TOKEN}"
```

Values live in `/etc/caddy/caddy.env` on the host (mode `600 root:root`,
**NOT tracked in any repo**). Systemd drop-in
`/etc/systemd/system/caddy.service.d/env.conf` loads it via
`EnvironmentFile=`.

## Deployment pattern (generic)

On a fresh operator host:

1. Install Caddy >= 2.10 (env placeholder syntax requires v2+)
2. Author a `Caddyfile` with your vhosts and `reverse_proxy` / `encode` /
   security headers. Use `{$VAR}` placeholders for any Bearer tokens —
   never inline them.
3. Create `/etc/caddy/caddy.env` with the env values (mode `600 root:root`)
4. Create `/etc/systemd/system/caddy.service.d/env.conf`:
   ```ini
   [Service]
   EnvironmentFile=/etc/caddy/caddy.env
   ```
5. `systemctl daemon-reload && systemctl restart caddy`

## Rotation

To rotate a token without config change:
```bash
sudo sed -i 's/^MY_TOKEN=.*/MY_TOKEN=<new-value>/' /etc/caddy/caddy.env
sudo systemctl restart caddy
```

Must be `restart`, not `reload` — systemd reads `EnvironmentFile` at
process spawn, and Caddyfile `{$VAR}` is baked in at adapter parse time.
