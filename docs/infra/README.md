# Upstream Caddy Config — krolik-server

## Files

- `caddyfile-krolik-server.snapshot.conf` — **authoritative source** of the
  reverse-proxy / TLS layer running on `krolik-server` in front of the main
  `oxpulse.chat` backend (and other services). Live location is
  `/etc/caddy/Caddyfile` on the host.

## Secrets

Bearer tokens referenced in the Caddyfile use Caddy's parse-time env syntax:

```
"Bearer {$MCP_TOKEN_ANATOLY}"
```

Values live in `/etc/caddy/caddy.env` on the host (mode `600 root:root`,
**NOT tracked in any repo**). Systemd drop-in `/etc/systemd/system/caddy.service.d/env.conf`
loads it via `EnvironmentFile=`. See `man systemd.exec` / `EnvironmentFile`.

Env vars used:
- `MCP_TOKEN_ANATOLY` — mcp.krolik.run auth (operator)
- `MCP_TOKEN_GUEST1` — mcp.krolik.run auth (guest)
- `MCP_TOKEN_GUEST2` — mcp.krolik.run auth (guest)
- `MCP_WP_BEARER` — `/wp/*` route Authorization header

## Deployment

This config is currently **hand-maintained** on the host. To reproduce on a
fresh server:

1. Install Caddy >= 2.10 (env placeholder syntax requires v2+)
2. Copy this snapshot to `/etc/caddy/Caddyfile`, chown `caddy:caddy`
3. Create `/etc/caddy/caddy.env` with the four `MCP_*` values (mode `600 root:root`)
4. Create `/etc/systemd/system/caddy.service.d/env.conf`:
   ```ini
   [Service]
   EnvironmentFile=/etc/caddy/caddy.env
   ```
5. `systemctl daemon-reload && systemctl restart caddy`
6. Verify: `curl -sS -o /dev/null -w "%{http_code}\n" https://mcp.krolik.run/ping`
   -> should return `401` (unauthed) -- confirms Caddy is up + auth flow wired

## Rotation

To rotate a token without config change:
```bash
sudo sed -i 's/^MCP_TOKEN_ANATOLY=.*/MCP_TOKEN_ANATOLY=<new-value>/' /etc/caddy/caddy.env
sudo systemctl restart caddy
```

Must be `restart`, not `reload` -- systemd reads `EnvironmentFile` at process
spawn, and Caddyfile `{$VAR}` is baked in at adapter parse time.

## Drift detection (manual)

Periodically compare snapshot to live:
```bash
diff /etc/caddy/Caddyfile /home/krolik/src/oxpulse-chat/docs/infra/caddyfile-krolik-server.snapshot.conf
```
Empty diff = no drift. Non-empty = reconcile (either update the snapshot and
commit, or revert the live change -- depends on intent).

## History

- **2026-04-18** -- initial snapshot after Bearer-token -> env-file refactor.
  Prior state had 4 live tokens inline in the file (untracked). This change
  (a) moved secrets to `/etc/caddy/caddy.env`, (b) rewrote config to use
  parse-time `{$VAR}` placeholders, (c) wired systemd `EnvironmentFile=`.
  Backup of pre-refactor config kept at `/etc/caddy/Caddyfile.bak.task03` on
  the host (not tracked).
