# Runbook — analytics DB is down

Scope: postgres behind oxpulse-chat is unreachable or rejecting writes while the service container is still running. Covers detection, blast radius, triage, recovery, and the chaos drill that validates this document.

All behavior statements are **as of 2026-04-10**. Re-verify after any change to `crates/server/src/analytics.rs`, `crates/server/src/main.rs`, or `crates/server/src/migrate.rs`.

## Symptoms

- Dozor alert fires: `rate(analytics_events_total{result="err"}[10m]) / rate(analytics_events_total[10m]) > 0.01` (alert rule ships in Task 3.3).
- Container healthcheck on oxpulse-chat stays **green** — the DB failure does not take the process down, so `docker ps` looks normal.
- Grafana "oxpulse-chat / analytics" dashboard (Task 3.4) shows the error-rate stat in red.
- Users are still placing and receiving calls — signaling is DB-free.
- `/api/event` continues to return `204 No Content` from the edge. **There is no 5xx spike on this endpoint** — see "Known behavior" below. Do not wait for a 5xx signal; trust the metric.
- `docker logs oxpulse-chat` shows repeated `analytics_insert_failed` warn lines with the underlying sqlx error.

## Blast radius

Unaffected (verify these still work before escalating):

- `/ws/call/{room_id}` WebSocket signaling — rooms live in memory (`oxpulse_signaling::Rooms`).
- `/api/turn-credentials` — derived from `TURN_SECRET`, no DB read.
- `/api/health` — static.
- SPA static assets and the `/{roomId}` fallback route.

Affected:

- `/api/event` POSTs complete with `204` from the client's perspective, but rows never reach `call_events`. The analytics funnel goes blind until the DB recovers.
- Historical events produced during the outage are **permanently lost**. The web tracker (`web/src/lib/tracker.ts`) batches in memory and ships via `navigator.sendBeacon` or `fetch(..., { keepalive: true })`; neither path inspects the response and there is **no localStorage retry queue**. Once the beacon is handed to the browser, the client forgets it existed.
- Any downstream consumer of `call_events` (funnels, partner dashboards, cohort jobs) sees a gap for the outage window.

## Immediate triage (first 5 minutes)

- Confirm it is the DB and not oxpulse-chat itself:
  - `docker exec oxpulse-chat sh -c 'nc -zv postgres 5432'` — connection refused means the DB is unreachable; success means the DB is up but writes are broken.
- Check the postgres container:
  - `docker ps --filter name=postgres --format '{{.Names}}\t{{.Status}}'`
  - If state is `exited` or `restarting`: `cd ~/deploy/krolik-server && docker compose up -d postgres`
- Pull recent postgres logs:
  - `docker logs postgres --tail 100`
- Check connection saturation (oxpulse-chat pool is tiny, but other services share this postgres):
  - `docker exec postgres psql -U memos -c "SELECT count(*) FROM pg_stat_activity;"`
  - `docker exec postgres psql -U memos -c "SHOW max_connections;"`
- If nothing obvious in postgres, check the host:
  - `df -h` on the host and `docker exec postgres df -h` inside the container.
  - `dmesg | tail -50` for OOM or I/O errors.

## Root causes to investigate

- **Disk full** on the postgres volume — postgres refuses writes once the data directory fills. Check both host `df -h` and `docker exec postgres df -h /var/lib/postgresql/data`.
- **OOM kill** of postgres or oxpulse-chat — `dmesg | grep -i oom` on the host.
- **Connection pool exhaustion** — oxpulse-chat uses `PgPoolOptions::new().max_connections(3)` (see `crates/server/src/main.rs`). Three connections is deliberately small; if postgres is slow under load every slot can block waiting. Other services on the same postgres may be eating the global `max_connections` budget.
- **Lock contention** on `call_events` — unlikely, the table is append-only and has no foreign keys, but worth checking `pg_locks` if writes hang rather than fail fast.
- **Broken migration applied mid-deploy** — `crates/server/src/migrate.rs` runs on boot and panics on any failure (`.expect("migration failed")`). If oxpulse-chat was **just restarted** and refuses to come up, this is the likely cause. Look for `migration failed` in the exit logs.
- **Startup connect failure** — `main.rs` calls `.expect("failed to connect to database")` when `DATABASE_URL` is set. If postgres is down at the moment oxpulse-chat starts, the container will panic and crash-loop. This is the only scenario where the container healthcheck also goes red.

## Resolution patterns

- **Postgres container exited cleanly** → `docker compose up -d postgres`, then verify with a probe event (see below).
- **Connection pool exhaustion, postgres healthy** → `docker compose restart oxpulse-chat` to drop stale sqlx connections.
- **Disk full** → free space on the volume, `docker compose restart postgres`, then `docker compose restart oxpulse-chat` to reset its pool.
- **Migration failure on boot** → revert the offending migration in `crates/server/migrations/`, rebuild and redeploy. Do not hand-patch the DB without also committing the fix, or the next deploy will re-panic.
- **Startup connect panic** → fix postgres first, then `docker compose up -d oxpulse-chat`; the container will stop crash-looping once it can reach the DB.

## Recovery verification

- Send a probe event through the real edge:
  ```
  curl -sX POST https://oxpulse.chat/api/event \
    -H 'content-type: application/json' \
    -d '{"did":"postmortem-probe","src":"runbook","events":[{"e":"page_view","r":null,"d":{}}]}'
  ```
  Expect `204 No Content`. A 204 alone does **not** prove recovery (see "Known behavior") — you must check the DB.
- Confirm the row landed:
  ```
  docker exec postgres psql -U memos -d oxpulse -c \
    "SELECT event_type, created_at FROM call_events WHERE device_id='postmortem-probe' ORDER BY created_at DESC LIMIT 1;"
  ```
  A row with a recent `created_at` means the pipeline is flowing again.
- Watch `rate(analytics_events_total{result="err"}[5m])` drop to zero in Grafana.
- Clean up the probe row:
  ```
  docker exec postgres psql -U memos -d oxpulse -c \
    "DELETE FROM call_events WHERE device_id='postmortem-probe';"
  ```

## Known behavior (as of 2026-04-10)

- `crates/server/src/analytics.rs::ingest` returns `StatusCode::NO_CONTENT` **on every code path** — success, per-event insert failure, and the `pool = None` case (unset `DATABASE_URL`). Insert errors are only surfaced via `tracing::warn!(error = %e, ..., "analytics_insert_failed")`. This is deliberate: the edge contract for a best-effort analytics pipeline does not push error handling onto the browser.
- Consequence: **external synthetic monitors that POST to `/api/event` cannot detect a DB outage via HTTP status codes.** Anything that only blackbox-probes this endpoint will stay green during a full outage. Synthetic monitoring must read `/metrics` (once Task 3.1 ships) and alert on `analytics_events_total{result="err"}`.
- Task 3.3 closes this gap by alerting on the metric directly; this runbook assumes that alert exists.
- The web tracker does **not** retry failed batches. `sendBeacon` is fire-and-forget; the `fetch` fallback uses `.catch(() => {})`. No localStorage queue, no backoff. Events generated during the outage window are gone.
- Container healthcheck is DB-independent, so `docker ps` will show oxpulse-chat as healthy throughout an analytics outage. The only scenario where the container itself crashes is a startup connect failure or migration panic — both on boot, not mid-flight.

## Chaos drill — how to validate this runbook

Run on staging, never production. This drill has **not** yet been executed as of 2026-04-10; scheduling is tracked under Phase 5 Task 5.3.

1. Baseline: confirm Grafana shows zero error rate and a probe event round-trips successfully.
2. `docker compose stop postgres` on the staging host.
3. Within 10 seconds, POST a probe event (see command above). Expected: `204 No Content`, and `docker logs oxpulse-chat --tail 20` shows `analytics_insert_failed` warn lines.
4. Open two browser tabs on the staging domain and start a WebRTC call between them. Expected: call connects and media flows — this proves the blast radius is limited to analytics.
5. Confirm the Task 3.3 alert fires in Dozor within its 10-minute window.
6. `docker compose start postgres`.
7. Wait for postgres to pass its healthcheck, then repeat the probe POST and the `SELECT` from "Recovery verification". Expected: row present, error-rate metric drops to zero.
8. Record the drill date, operator, and any deviations from this runbook in the partner-launch plan.

Do not fabricate drill results. If a step fails, update this runbook rather than the drill log.
