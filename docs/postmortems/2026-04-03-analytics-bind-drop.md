# Postmortem: Analytics silently dropped every event for 7 days

- **Status:** Resolved
- **Severity:** SEV-2 (silent data loss, no user-visible impact)
- **Introduced:** 2026-04-03 18:16 PDT (commit `f36ddae`)
- **Detected:** 2026-04-10 20:42 PDT (user-reported, during unrelated investigation)
- **Fixed:** 2026-04-10 21:02 PDT (commit `a0f4a4a`), deployed 21:44 PDT
- **Duration of silent failure:** ~7 days, 2 hours
- **Authors:** @anatoly
- **Related:** `docs/postmortems/2026-04-10-router-404-telegram.md` (concurrent, mutually masking)

> Dated by root-cause introduction (2026-04-03), not by detection (2026-04-10). This convention makes the silent-failure duration obvious at a glance in the postmortem index.

## Summary

Between commit `f36ddae` on 2026-04-03 and commit `a0f4a4a` on 2026-04-10, every row attempted by `POST /api/event` failed to insert into the `call_events` table while the HTTP handler still returned `204 No Content` to the client. The root cause: the INSERT was updated to 6 positional placeholders when the new `source` column was added, but the `.bind(&event.data)` call was **replaced** by `.bind(&batch.source)` instead of a sixth bind being **appended**, producing a 5-bind-to-6-placeholder mismatch on every row. The per-row `sqlx` error was swallowed by a `let _ = ...` fire-and-forget pattern with no log and no metric, so nothing alerted. The fix re-added `.bind(&event.data)` and replaced `let _ =` with a `tracing::warn!` on error. Detection was accidental: a user asked about a Telegram link-preview bug, and during the router investigation the off-by-one bind count was noticed by inspection. Seven days of analytics data (all users, all event types) were lost permanently — the tracker client does not buffer or retry.

## Impact

- **Business impact.** Seven days of zero analytics data. Zero funnel attribution, zero A/B signal, zero retention insight, zero page-view counts, zero conversion data, zero cohort retention. Data loss is permanent: the widget tracker fires events from its in-memory queue on a short debounce and does not retain-and-retry on server error, and the server never wrote the rows to begin with, so there is no log or queue to replay from. Product decisions made during this window that depended on analytics were made on stale (pre-2026-04-03) data.
- **User-visible impact.** None. The handler returned 204 to clients regardless of the DB outcome, so the chat UI and the oxpulse widget behaved normally throughout. No user complaints, no support tickets, no error pages, no increase in latency. The failure was completely invisible from the outside.
- **Secondary impact.** A concurrent router-404 bug (see `docs/postmortems/2026-04-10-router-404-telegram.md`) could not be detected via its upstream funnel signal, because the upstream funnel signal was itself broken by this bug. Two independent failures were mutually masking: each one ate the observability that would have revealed the other. Had either bug existed in isolation, the other's signal would likely have caught it within hours.
- **Scope of data loss.** All events, from all users, across all event types (`page_view`, `chat_open`, `message_sent`, `widget_loaded`, etc.) for the full 7-day window. No client-side retention means no recovery is possible. Estimated volume lost: the full traffic footprint of oxpulse.chat plus any embedded widget across all partner sites for the period.

## Timeline

All times PDT.

- **2026-04-03 18:16** — Commit `f36ddae` ("feat: add source field to call_events") lands on `main`. It adds the `source` field to the `EventBatch` struct, adds the `source` column to the INSERT SQL, and changes the `.bind` chain. CI is green because no test exercises the INSERT path against a real Postgres. Deployed shortly after. From this point forward, every `/api/event` row errors inside `sqlx::query(...).execute()` with a parameter-count mismatch, and the handler returns 204 to the client.
- **2026-04-03 18:16 → 2026-04-10 20:42** — ~7 days of silent failure. No alert fires. No dashboard shows a drop, because no dashboard existed for analytics ingest rate. Operational routine is unaffected.
- **~2026-04-06** — The user asks the assistant about `page_view` statistics for oxpulse.chat. The assistant investigates the **client-side tracker code**, confirms that `page_view` is fired on main-page visits, and tells the user to "visit the site and wait 3 seconds to trigger the event." The assistant does not run the handler end-to-end and does not query `call_events`. **This was a missed detection opportunity**; the correct answer would have been `psql SELECT count(*) FROM call_events WHERE event_type='page_view'`, which would have returned zero rows.
- **2026-04-10 20:42** — User reports a Telegram link-preview bug (the concurrent router-404 issue). Assistant begins investigating the router. While reading `crates/server/src/analytics.rs` as part of tracing request paths, the assistant notices that the INSERT has six `$N` placeholders but only five `.bind()` calls.
- **2026-04-10 20:48** — Manual bind count confirms the mismatch. Root cause identified: `.bind(&event.data)` was replaced by `.bind(&batch.source)` in `f36ddae` rather than a sixth bind being appended. The `let _ = ...await` had been swallowing the sqlx parameter-count error for seven days.
- **2026-04-10 21:02** — Commit `a0f4a4a` ("fix(server): SPA fallback returns 200 and analytics persists data field") lands. It (a) re-adds `.bind(&event.data)` as the sixth bind and (b) replaces `let _ = sqlx::query(...).execute(...)` with `let res = ...; if let Err(e) = res { tracing::warn!(...) }`. Both the router 404 and the analytics bind drop are fixed in the same commit.
- **2026-04-10 21:17** — Commit `3917ffb` ("test(server): assert analytics persists all 6 fields including data") adds the `analytics_insert_persists_all_fields` regression test in `crates/server/tests/http_integration.rs`. The test POSTs a fixture batch and reads back `data["referrer"] == "t.me"` — the exact field and value the old bug dropped. It is gated on `TEST_DATABASE_URL` so it runs when a real Postgres is available.
- **2026-04-10 21:44** — Fix deployed to production. End-to-end smoke test: `curl -X POST https://oxpulse.chat/api/event -d '{...}'` followed by `psql SELECT data FROM call_events ORDER BY created_at DESC LIMIT 1`. `data.referrer` round-trips correctly. Task 1.8 of the reliability plan marked complete.

## Root cause

1. **Direct cause.** The SQL INSERT in `crates/server/src/analytics.rs` had six positional placeholders (`$1..$6`), but the `.bind()` chain was modified incorrectly when the `source` column was added in `f36ddae`. The implementer replaced the existing `.bind(&event.data)` call with `.bind(&batch.source)` rather than appending a sixth `.bind(&event.data)` after it. Result: five binds to six placeholders → sqlx returned a parameter-count mismatch error on every single call to `execute()`.

2. **Contributing cause — silent fire-and-forget.** The analytics handler used `let _ = sqlx::query(...).execute(pool).await;`. This was intentional: analytics ingest is not a critical path and should never block or fail a client request. The pattern is defensible in principle, but it was implemented as "fire-and-forget and ignore" rather than "fire-and-forget and observe." No log line, no counter, no sampled error. The correct shape is `if let Err(e) = res { metrics::counter!(...); tracing::warn!(...); }` — still non-blocking, but auditable.

3. **Contributing cause — no integration test against a real DB.** The `call_events` handler had unit tests for the request parser (shape, JSON schema) but nothing that actually exercised the handler → Postgres path. Any test that did `POST /api/event` against a test Postgres and then `SELECT` from `call_events` would have failed the moment `f36ddae` landed. We had many tests; we had no end-to-end test for this path.

4. **Contributing cause — runtime-checked SQL.** The codebase uses `sqlx::query(...)` (runtime-checked) rather than `sqlx::query!(...)` (compile-time-checked). The `query!` macro would have caught the parameter-count mismatch at `cargo build` time and the bug would never have shipped. The runtime form was chosen for flexibility (it does not require `DATABASE_URL` to be set at compile time). See Action Item AI-8 for the explicit re-evaluation of this tradeoff.

5. **Contributing cause — small-diff review.** The `f36ddae` PR changed eight lines in `analytics.rs` (`+5 -3`). The diff visually showed `- .bind(&event.data)` and `+ .bind(&batch.source)` directly adjacent. A reviewer skimming the diff saw "one bind changed to another bind" and approved. Small diffs hide subtle semantic bugs precisely because they look trivial; the reviewer's mental model was "rename one bind" when the actual change was "delete one bind, add a different one."

## Detection

- **Actual detection.** User-reported, via an unrelated Telegram-preview bug. The assistant noticed the off-by-one while reading `analytics.rs` during router investigation. This is detection by coincidence, not by signal.
- **Missed detection opportunities.**
  - Four days before root cause was found, the user asked directly about `page_view` counts. The assistant reasoned about the tracker client code and told the user to wait 3 seconds, rather than querying the database to verify the claim. The database was right there; asking a question and inspecting client code is not the same as running the end-to-end path and reading the result.
  - Zero automated detection existed. There is no SLO on analytics ingest success rate, no alert on `call_events` row-rate, no CI integration test, no synthetic probe.
- **Counterfactual.** If the `analytics_durability` SLO from reliability plan Task 0.2 (99.9% success / 28-day window) had been in place with a fast-burn alert, the alert would have fired within roughly an hour of `f36ddae` landing on 2026-04-03, because the success rate would have dropped from ~100% to ~0% instantly. Detection time would have been 1 hour instead of 7 days — a ~170× improvement.

## What went well

- Diagnosis was fast. From "this bind count looks wrong" to root cause confirmed was roughly 6 minutes, and the entire inspection was done against the checked-in source, so there was no guessing about production state.
- Fix was fast. From root cause confirmed to deployed was roughly 60 minutes, including writing a regression test against a real Postgres and validating the round-trip by hand.
- The regression test asserts on the **exact dropped field** (`data["referrer"] == "t.me"`), not merely that a row exists. A weaker test — row-count only — would have passed even if the `data` column were again silently empty or null. The assertion is deliberately on the exact semantic value the old bug dropped, so a future re-break is caught at the field level, not just the row level.
- The fix replaced `let _ =` with an explicit `tracing::warn!` error branch that includes `event_type` as a structured field. Any future re-break of this code path will produce log lines that Dozor can alert on immediately, even before the SLO (AI-3) lands.
- Both concurrent bugs (router 404 and analytics bind drop) were fixed in a single commit (`a0f4a4a`) and deployed together. No staggered deploys, no re-deployment churn, and both postmortems reference the same commit SHA so the correlation is permanent in the record.
- The postmortem dating convention (root-cause introduction, not detection) made the 7-day gap visually obvious in the filename and in the index, reinforcing the "silent failures are the worst class" lesson every time someone skims the postmortem directory.

## What went poorly

- **Seven days undetected.** This is the same class of lesson as the router-404 postmortem: we had no synthetic health check on the analytics path, no SLO, no alert, and no dashboard. A data pipeline with zero observability is a data pipeline that will fail silently, and it did.
- **A direct user question failed to trigger investigation.** Four days before root cause was found, the user asked "why is page_view count at 0?" The assistant reasoned about the tracker rather than running it and reading the result. The correct response to "is X working?" is never "let me look at the code for X." It is always "let me run X and check the output."
- **`let _ =` on a database write is a silent data loss landmine.** The pattern was intentional (non-blocking analytics) but the implementation was wrong (non-blocking *and* non-observable). Fire-and-forget is fine; fire-and-forget-and-never-log is not.
- **No CI job runs handler integration tests against a real Postgres.** `ci.yml` runs `cargo test` and the web checks, but nothing that requires a live database. Literally any integration test for this path would have caught the bug.
- **Two concurrent bugs masked each other.** The router 404 broke the Telegram preview funnel. The analytics bind drop broke the funnel metrics that would have shown the Telegram preview drop. Observability has to assume every component can fail independently — one broken signal is not allowed to hide another broken signal.

## Action items

| # | Action | Type | Owner | Priority | Tracking |
|---|---|---|---|---|---|
| AI-1 | Analytics integration test with real DB (`analytics_insert_persists_all_fields`) | Prevention | @anatoly | DONE | commit `3917ffb` |
| AI-2 | Replace `let _ =` with `if let Err(e) = ... { tracing::warn!(...) }` in analytics handler | Detection | @anatoly | DONE | commit `a0f4a4a` |
| AI-3 | SLO `analytics_durability` (99.9% success / 28d) with multi-window fast-burn alert | Detection | @anatoly | P0 | reliability plan Task 0.2 |
| AI-4 | Metric `analytics_events_total{result="ok"\|"err"}` counter in the handler | Detection | @anatoly | P0 | reliability plan Task 3.2 |
| AI-5 | Dozor alert on `analytics_events_total{result="err"}` rate > 1% for 10m | Detection | @anatoly | P0 | reliability plan Task 3.3 |
| AI-6 | Runbook `docs/runbooks/analytics-db-down.md` | Mitigation | @anatoly | DONE | reliability plan Task 5.3, commit `a85d4ee` |
| AI-7 | Policy: `let _ =` on any DB write fails code review unless paired with a log and a metric. Add to project style guide / `CLAUDE.md`. | Process | @anatoly | P1 | none yet |
| AI-8 | Decide whether `sqlx::query!` (compile-time checked) should replace `sqlx::query(...)` for all INSERT/UPDATE paths. Counterargument: `query!` requires `DATABASE_URL` at compile time, complicating CI and release builds. Document the decision either way. | Process | @anatoly | P2 | none yet |
| AI-9 | Add a CI job that runs `analytics_insert_persists_all_fields` (and future integration tests) against an ephemeral test Postgres on every PR. `ci.yml` currently runs only Rust unit tests and web checks. | Detection | @anatoly | P1 | none yet |
| AI-10 | Operating principle: when the user asks "is X working?", answer by running X end-to-end and reading the output, not by reasoning about the code. Add to the assistant's operating notes. | Process | @anatoly | P1 | operational |

## Why the existing safeguards failed

It is worth being explicit about *why each layer of defense that should have caught this bug did not*, rather than generically blaming "lack of tests." The codebase was not undefended — it was defended with the wrong defenses for this failure mode.

- **`cargo check` and `cargo clippy` passed.** The `sqlx::query(...)` runtime form is opaque to the compiler: the SQL string and the `.bind()` chain are unrelated from the type system's point of view. No lint caught the parameter-count mismatch because no lint could see it. `sqlx::query!` would have, but the project opted out for build-time flexibility (see AI-8).
- **Unit tests passed.** The unit tests for the analytics handler cover request parsing (JSON schema, required fields, body size limits) but stop at the boundary of the database call. They assert "the handler accepts this payload and returns 204," which is exactly what the broken handler did. The tests were correct for what they tested; they just did not test the database write.
- **Manual deployment smoke tests passed.** After `f36ddae` was deployed, the standard post-deploy smoke test (`curl POST /api/event && expect 204`) returned 204. The smoke test did not read back the row, so it could not tell the difference between "inserted" and "silently dropped."
- **Code review passed.** As covered in contributing cause #5: the diff was small, visually tidy, and the `- .bind(&event.data)` / `+ .bind(&batch.source)` pair looked like a rename. Human review at the visual-diff level is not reliable for this kind of bug; this is a task for tooling.
- **Production logs were silent.** There were no error lines to notice because `let _ =` consumed them. A log-based alert on `analytics_insert_failed` would have fired immediately — the bug would have been caught by any log rule that pattern-matched on sqlx error strings. We had no such rule, and no log line to match against even if we had.
- **The client kept sending events.** The widget's tracker retries on network errors but treats 2xx as success. 204 is 2xx, so the client considered every event delivered and moved on. There was no client-side retry queue to drain later.

Every layer of defense was calibrated for a *different* failure mode (bad client input, network errors, payload validation) and none were calibrated for "handler accepts the request, returns success, and silently drops the row." That failure mode needs its own dedicated defense: either compile-time-checked SQL (AI-8) or an end-to-end integration test that reads back what it wrote (AI-1, done).

## Lessons learned

1. **`let _ =` on a DB write converts a loud error into silent data loss.** Always pair fire-and-forget with a log and a metric. "Non-blocking" and "unobservable" are not the same thing and must never be conflated.
2. **Small, visually tidy diffs hide semantic bugs.** An eight-line diff that *looks* like "one bind renamed to another bind" was actually "one bind deleted, a different bind added." Humans skim small diffs. Machines do not. Use compile-time-checked SQL or mandatory integration tests for anything touching database writes.
3. **Silent data pipelines are the worst class of broken.** The UI kept working. Users kept using the product. Nothing visibly changed. We lost a week of data anyway. Assume every data pipeline is broken until proven otherwise; synthetic probes and SLOs are not optional for ingest paths.
4. **Asking questions about code is not a diagnostic tool; running the code is.** When the user asked about event counts four days before detection, the correct action was `psql SELECT count(*)`, not a walk through the client tracker. "Is it working?" always means "did we run it and read the output?"
5. **Independent failures mask each other when observability is thin.** The router 404 ate the Telegram funnel; the analytics bind drop ate the funnel signal that would have revealed the router 404. Every signal must be independently monitored; no component may be the sole watcher of any other component.

## Appendix: exact bug diff

From `git show f36ddae -- crates/server/src/analytics.rs`:

```diff
-            "INSERT INTO call_events (id, device_id, event_type, room_id, data, created_at) \
-             VALUES ($1, $2, $3, $4, $5, now())",
+            "INSERT INTO call_events (id, device_id, event_type, room_id, source, data, created_at) \
+             VALUES ($1, $2, $3, $4, $5, $6, now())",
         )
         .bind(id)
         .bind(&batch.device_id)
         .bind(&event.event_type)
         .bind(&event.room_id)
-        .bind(&event.data)
+        .bind(&batch.source)
         .execute(pool)
         .await;
```

Placeholder count: 6 (`$1..$6`). Bind count after the change: 5 (`id`, `device_id`, `event_type`, `room_id`, `source`). Missing: `.bind(&event.data)` as the sixth bind. Every `execute()` call returned `Err(Database(_))` with a parameter-count mismatch; the `let _ =` consumed it; the handler returned `204 No Content`; `call_events` never grew.

The fix in `a0f4a4a` re-adds the sixth bind and makes the error branch visible:

```diff
-        let _ = sqlx::query(
+        let res = sqlx::query(
             "INSERT INTO call_events (id, device_id, event_type, room_id, source, data, created_at) \
              VALUES ($1, $2, $3, $4, $5, $6, now())",
         )
         .bind(id)
         .bind(&batch.device_id)
         .bind(&event.event_type)
         .bind(&event.room_id)
         .bind(&batch.source)
+        .bind(&event.data)
         .execute(pool)
         .await;
+        if let Err(e) = res {
+            tracing::warn!(error = %e, event_type = %event.event_type, "analytics_insert_failed");
+        }
```
