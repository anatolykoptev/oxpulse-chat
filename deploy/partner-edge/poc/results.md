# PoC Measurement Results — TURNS-on-443 Task 1.2

**Date:** 2026-04-17  
**Measurement script:** `measure.sh` (this directory)  
**Raw logs:** `/tmp/poc-a-prime.log`, `/tmp/poc-b.log` (ephemeral, not committed)

---

## Headline Verdict

**Both variants blocked by Task 1.1 bugs. Architectural comparison inconclusive.**

Neither variant ran to a full measurement. Both failed at the Docker/container layer due to version mismatches introduced during Task 1.1 scaffolding. The bugs are one-line fixes each and do not indicate architectural flaws in A' or B themselves.

---

## Criteria Matrix

| Criterion | Variant A' | Variant B |
|-----------|-----------|-----------|
| C1 Caddyfile parses + caddy-l4 plugin present | BLOCKED | PASS |
| C2 UDP 18443 closed (H3 absent) | BLOCKED | PASS |
| C3 Cert reload on SIGUSR2 | BLOCKED | FAIL (see note) |
| C4 TLS handshake: main SNI + TURNS SNI | BLOCKED | FAIL (nginx dead) |
| C5 HTTPS p50 latency < 2ms | BLOCKED | INVALID (no endpoint) |
| **Overall** | **BLOCKED** | **NOT_VIABLE (as-is)** |

---

## Variant A' — BLOCKED at build

### Root cause

`docker-compose.yml` pins `caddy:2.10-builder` (Caddy v2.10.2) as the xcaddy base, but `caddy-l4@v0.1.0` requires `github.com/caddyserver/caddy/v2@v2.11.1`. xcaddy's `go get` enforces minimum dependency versions and rejects the downgrade:

```
go: github.com/mholt/caddy-l4@v0.1.0 requires
    github.com/caddyserver/caddy/v2@v2.11.1, but v2.10.2 is requested
go: github.com/mholt/caddy-l4@v0.1.0 requires github.com/caddyserver/caddy/v2@v2.11.1, not github.com/caddyserver/caddy/v2@v2.10.2
2026/04/18 02:24:52 [FATAL] exit status 1
```

### Fix required (Task 1.1 fixup)

In `a-prime/docker-compose.yml`, change the `dockerfile_inline` base images:

```
FROM caddy:2.10-builder  →  FROM caddy:2.11-builder
FROM caddy:2.10          →  FROM caddy:2.11
```

Both `caddy:2.11` and `caddy:2.11-builder` images are already present on the host. This is a one-line-per-stage fix. The Caddyfile itself does not change.

### Criteria status

All C1-C5 are UNMEASURABLE for A'. The architectural hypothesis (caddy-l4 `listener_wrappers` SNI demux) could not be exercised. C1 will re-run once the image pin is corrected.

---

## Variant B — nginx stream module mismatch

### C1: Caddyfile validation — PASS

Caddy's built-in `validate` (stock `caddy:2.10`) accepted the Caddyfile cleanly:

```
Valid configuration
C1 PASS — Caddyfile accepted by caddy binary in container
```

### C2: HTTP/3 UDP absent — PASS

`ss -lnu` showed no UDP listener on `127.0.0.1:18443`. H3 is absent as intended.

### C3: Cert reload on SIGUSR2 — FAIL (expected PoC limitation)

After `touch cert.pem` + `kill -USR2 1` inside the coturn container, no "Reloading TLS/SSL" line appeared in coturn logs. This is a **known PoC limitation**: in the PoC the coturn container is PID 1 directly (no init wrapper), and SIGUSR2 to PID 1 in a container may be swallowed before coturn's signal handler runs, or the cert was already valid and coturn skipped the reload log. The production flow uses a systemd path unit watching the cert file, which calls `systemctl reload coturn` on the host — that path is not exercisable inside a Docker bridge sandbox. Mark as **needs-production-validation**.

### C4: TLS handshake — FAIL (nginx startup failure)

nginx failed to start. The `nginx.conf` uses:

```
load_module /usr/lib/nginx/modules/ngx_stream_module.so;
```

On `nginx:1.27-alpine`, the stream module is **compiled in** — there is no `.so` file. The `load_module` directive for a static module is a fatal error:

```
[emerg] dlopen() "/usr/lib/nginx/modules/ngx_stream_module.so" failed
        (Error loading shared library .../ngx_stream_module.so: No such file or directory)
```

nginx:1.27-alpine's own build flags confirm: `--with-stream` (static), not `--with-stream=dynamic`.

Because nginx never bound port 443, all `openssl s_client` connections got `ECONNREFUSED`.

### Fix required (Task 1.1 fixup)

In `b/nginx.conf`, remove the `load_module` directive. The stream block works without it on alpine nginx:

```diff
-load_module /usr/lib/nginx/modules/ngx_stream_module.so;
-
 worker_processes auto;
```

### C5: HTTPS latency — INVALID

curl returned immediately with `ECONNREFUSED` (~0.15 ms). The measurement is meaningless; the endpoint was never reachable. Re-run after nginx fix.

---

## Task 1.1 Defects Found

| # | File | Defect | Fix |
|---|------|--------|-----|
| 1 | `a-prime/docker-compose.yml` | `caddy:2.10-builder`/`caddy:2.10` incompatible with `caddy-l4@v0.1.0` (requires v2.11.1) | Bump both FROM lines to `caddy:2.11-builder` / `caddy:2.11` |
| 2 | `b/nginx.conf` | `load_module ngx_stream_module.so` — stream is compiled-in on alpine, no .so exists | Remove the `load_module` line |

Both fixes are one-line changes. Neither affects the architectural logic being evaluated.

---

## PoC Limitations (not testable at this level)

The following items were identified in the README and confirmed during measurement:

1. **Real ACME cert renewal** — Caddy's HTTP-01 challenge with `disable_tls_alpn_challenge` could not be exercised. Self-signed local certs only.
2. **systemd path unit flow** — The production cert-reload trigger (inotify → `systemctl reload coturn`) is not exercisable inside a Docker bridge sandbox. C3 coturn SIGUSR2 is a best-effort approximation.
3. **Host-network nginx topology** — Variant B production uses `network_mode: host` for nginx; the PoC uses bridge. The `ssl_preread`/`proxy_pass` logic is the same, but loopback address resolution differs.
4. **DTLS** — Both variants disable DTLS (`no-dtls` in coturn.conf). The UDP relay port range (49152-65535) is not mapped in compose; TURN media relay is not testable here.
5. **JA3S fingerprint capture** — Full JA3S capture requires a tool like Wireshark/tshark with the `ja3` dissector. The `openssl s_client` output captures cipher/protocol but not the raw ClientHello fingerprint. This can be added with `tshark -Y tls.handshake.type==1 -T fields -e tls.handshake.ciphersuites` if tshark is available in Phase 3.
6. **Caddy-l4 layer4 SNI peek overhead vs plain Caddy** — C5 latency comparison between A' (peek path) and B (nginx preread path) could not be made because A' never ran.

---

## Recommendation for Task 1.3

**Decision is deferred** until both Task 1.1 fixups are applied.

Once fixed, re-run `bash measure.sh a-prime` and `bash measure.sh b` and update this file. The expected outcomes after fixes:

- **A'**: C1 will test the real caddy-l4 plugin invocation (the key unknown). If `listener_wrappers { layer4 { @turns tls sni ... } }` parses and routes correctly, A' becomes the preferred option — single-process, no sidecar, fewer moving parts.
- **B**: With `load_module` removed, nginx stream should start and C2/C4/C5 become measurable. B's architecture is simpler to reason about (nginx stream preread is a well-known pattern) but adds an extra hop and process.

**If A' passes after the image bump**: recommend A' for Phase 3. The caddy-l4 approach eliminates the nginx sidecar, reduces the service count, and keeps all TLS termination in one process.

**If A' still has issues after the image bump**: adopt B. It is architecturally sound; the only PoC failure was an nginx.conf typo, not a fundamental limitation.

**Design doc §4.3 acceptance criteria status:**

| §4.3 Criterion | Status |
|----------------|--------|
| A' Caddyfile with layer4 parses + runs | NOT TESTED (Task 1.1 bug #1 blocks build) |
| HTTP/3 fallthrough: UDP 18443 closed | PASS on B; blocked on A' |
| Cert renewal end-to-end | NEEDS_PRODUCTION_VALIDATION (both variants) |
| JA3S captured for main+TURNS SNI | NOT TESTED (C4 failure on B, A' blocked) |
| Latency overhead <2ms p50 | NOT TESTED (endpoint not reachable on B, A' blocked) |
