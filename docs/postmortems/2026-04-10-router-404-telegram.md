# Postmortem: Router 404 Breaks Telegram Link Previews

- **Date:** 2026-04-10
- **Author:** @anatoly
- **Status:** Resolved
- **Severity:** SEV-3 (user-visible, conversion-impacting, no data loss)
- **Incident window:** approx. 2026-04-03 17:04 PDT (commit `237fca5`) through 2026-04-10 20:58 PDT (commit `a0f4a4a`)
- **Duration:** ~7 days

## Summary

On 2026-04-10 at approximately 20:42 PDT, a user reported that `oxpulse.chat` room URLs shared in Telegram and iMessage had been rendering as generic "file" attachments rather than rich link preview cards. Investigation showed the HTTP response for any unknown path like `/TQFA-9412` returned `HTTP 404` with an empty body and no `content-type` header, because the axum router used `fallback_service(ServeDir::new(room_assets_dir))` without an SPA fallback. Link previewers fell back to treating the URL as an opaque binary. The bug was present in production for at least 7 days — since commit `237fca5` (2026-04-03) or earlier — and was fixed by caching `index.html` in a `OnceLock<String>` and serving it via an explicit axum handler returning `200 text/html; charset=utf-8`.

## Impact

- **User-visible:** every room link shared via Telegram, iMessage, Slack, or any Open Graph consumer rendered as a "file" attachment badge instead of a rich preview card with title, description, and image. The primary viral sharing path produced poor first impressions.
- **Funnel metrics:** `page_view` events originating from chat-app referrers would have been the detection signal, but were unmeasurable because a parallel bug (the analytics `data`-binding drop also fixed in `a0f4a4a`) was silently dropping every `/api/event` insert over the same window. Both bugs co-existed and masked each other.
- **Signaling path:** unaffected. `/ws/call/{room_id}` and `/api/turn-credentials` are explicit routes and never hit the `ServeDir` fallback. Any user who manually typed or pasted a room URL into a browser tab (bypassing the preview) could still join calls normally.
- **Static assets:** unaffected. `/_app/immutable/*` and `/fonts/*` are served by their own nested `ServeDir` instances and resolve normally.
- **No user accounts:** oxpulse.chat does not have login; no authentication or session impact. No PII was exposed, lost, or corrupted.
- **Blast radius duration:** at least 7 days — from the deployment of commit `237fca5` on 2026-04-03 through the fix on 2026-04-10. The underlying `ServeDir` usage may predate `237fca5`; the postmortem uses the conservative lower bound anchored to the last deploy observed in the logs before symptom onset.

## Timeline (all times PDT unless noted)

- **2026-04-03 17:04** — commit `237fca5` lands on `main` (`feat: add /api/event endpoint for analytics ingestion`). The auto-deploy webhook rebuilds the Docker service. The `ServeDir` fallback behavior is already in place; from this point onward every `/{roomId}` URL shared via chat apps renders as a "file" attachment. Undetected.
- **2026-04-03 → 2026-04-10** — bug active in production. No alerts, no synthetic probes, no user reports. Analytics pipeline is concurrently broken (Bug B), so even the indirect funnel signal is unavailable.
- **2026-04-10 ~20:42** — user shares a room link in Telegram, notices the preview shows a file badge instead of a card, reports to the assistant.
- **2026-04-10 ~20:48** — assistant runs `curl -sI https://oxpulse.chat/TQFA-9412`. Response is `HTTP/2 404` with empty body and no `content-type` header. Root cause identified: `ServeDir` returns 404 for unknown paths. Telegram's link previewer treats status+headers (not body) as the signal for "file vs page."
- **2026-04-10 ~20:55** — first fix attempt: `ServeDir::not_found_service(ServeFile::new(index_html))`. Implementer re-runs `curl -sI` and observes `HTTP/2 404` is still returned despite a correct HTML body being served. Fix discarded.
- **2026-04-10 21:02** — commit `a0f4a4a` lands (`fix(server): SPA fallback returns 200 and analytics persists data field`). The working fix caches `index.html` into `SPA_INDEX: OnceLock<String>` at router construction and installs an axum handler `spa_fallback` via `ServeDir::fallback(...)` that returns `(StatusCode::OK, content-type: text/html; charset=utf-8, body)`. The same commit also fixes an unrelated but co-located analytics bug (the `.bind(&event.data)` drop from `f36ddae`).
- **2026-04-10 21:11** — commit `3a98953` lands (`test(server): add SPA fallback regression tests`). In-process tests using `axum::Router` + `tower::ServiceExt::oneshot` + a tempdir-backed `room_assets_dir` assert `/{roomId}` returns `200` + `text/html; charset=utf-8` + a non-empty body.
- **2026-04-10 21:25** — commit `e09f2e8` lands (`test(server): live E2E for room-link preview regression`). A reqwest-based test hits the production URL, gated on `E2E_BASE_URL`, and asserts `200`, `text/html`, and the presence of OG meta tags in the body.
- **2026-04-10 21:50** — commit `aafedc7` lands (`fix(server): don't panic on missing index.html in build_router`). The earlier `a0f4a4a` fix used `std::fs::read_to_string(...).unwrap_or_else(|e| panic!(...))`, which broke 7 pre-existing integration tests that passed a synthetic `room_assets_dir` like `/nonexistent`. The panic is replaced with `tracing::warn!`; `spa_fallback` already had a placeholder fallback for the `OnceLock::get() == None` case.
- **2026-04-10 21:55** — all server tests green. Incident closed.

## Root cause

Three contributing factors combined:

1. **Primary cause — missing SPA fallback.** The router used `fallback_service(ServeDir::new(room_assets_dir))` in `crates/server/src/router.rs`. `ServeDir` returns `404` with empty body and no `content-type` header for any path not resolving to a file on disk. The SvelteKit static adapter expects the server to rewrite unknown paths to `index.html` so the client router can handle them (room URLs like `/TQFA-9412` are client-rendered). The Rust server was unaware of that contract. The fix is architectural: an SPA-serving backend must always have a catch-all to `index.html` with `200 text/html`.

2. **Contributing cause — tower-http footgun.** The initial in-session fix used `ServeDir::not_found_service(ServeFile::new(index_html_path))`. tower-http 0.6 resolves the `not_found_service` body but preserves the original `404` status code from the outer `ServeDir`. Chat app link previewers (Telegram bot, iMessage, Slack Unfurl) use the status code — not just the body — to decide whether a URL is a page or a file. The fix appeared to work in logs (body was correct HTML) but `curl -sI` still showed `HTTP/2 404`. The working approach is `ServeDir::fallback(handler)` combined with an explicit axum handler, which does honor the handler's declared status code.

3. **Contributing cause — no detection mechanism existed.** There were no Prometheus metrics on HTTP status by path, no 4xx rate alerts, no synthetic probes hitting room URLs, and no live E2E in CI. The only signal available was user reports, which is by definition slow and unreliable for a conversion-funnel regression where users just see "broken preview" and move on without reporting.

## Detection

User-reported, approximately 7 days after the regression entered production. No automated signal — alerting, synthetic probes, CI E2E, or funnel metrics — fired at any point. This is the most important failure in the incident: the detection mechanism for this class of bug was entirely absent. The user report arrived via conversational channel, not a structured bug tracker, making the "time to ticket" effectively zero but the "time to first signal" seven days.

## What went well

- **Fast diagnosis once reported.** A single `curl -sI` against production produced the exact response headers and identified root cause within roughly one minute of the user report.
- **The implementer did not trust library docs.** After the first fix (`ServeDir::not_found_service`), the implementer ran `curl` again, observed the still-`404` response, and kept debugging rather than declaring victory. This caught a still-broken fix before it shipped.
- **Regression tests landed in the same session.** Both an in-process test (`3a98953`) using a tempdir and a live E2E (`e09f2e8`) using reqwest against the production URL are now in `crates/server/tests/http_integration.rs`. Either would have caught this bug if written earlier.
- **Scope discipline.** The fix in `a0f4a4a` also corrected a second unrelated bug (analytics `data` field binding drop) that was discovered in the same session, but each bug has a separate root-cause section in the commit message.

## What went poorly

- **Seven days of undetected, user-visible breakage on a critical funnel path.** Link sharing in chat apps is the primary viral vector for oxpulse.chat. Zero monitoring protected it.
- **The first fix was insufficient.** The implementer initially trusted the tower-http `not_found_service` API without running `curl` first. Two commits were required to fully resolve what should have been one.
- **The initial fix introduced a panic regression.** Commit `a0f4a4a` used `std::fs::read_to_string(...).unwrap_or_else(|e| panic!(...))`. This broke 7 pre-existing integration tests that pass `room_assets_dir = "/nonexistent"`. The regression was caught by the next task's pre-implementation test run in `aafedc7` — not by the contributor running the full test suite before pushing `a0f4a4a`.
- **No alert on 4xx rate for public paths.** A single Prometheus counter `http_requests_total{path, status}` with a rule `rate(http_requests_total{status=~"4.."}[5m]) / rate(http_requests_total[5m]) > 0.05` would have fired within minutes of deployment.
- **Two independent silent failures masked each other.** The router 404 bug and the analytics `data`-drop bug were active simultaneously. The analytics funnel would have been the observable signal for the router bug — but the analytics pipeline itself was broken, so there was no funnel data to miss.

## Action items

| # | Action | Type | Owner | Priority | Tracking |
|---|---|---|---|---|---|
| AI-1 | Add Prometheus metric `http_requests_total{path, status}` and Dozor alert on `rate(http_requests_total{status=~"4.."}[5m]) / rate(http_requests_total[5m]) > 0.05` for public paths | Prevention | @anatoly | P0 | Task 3.2 (partner-launch plan) |
| AI-2 | SLO `call_join_success` (99.0% / 28d) — partially catches this class via upstream user symptom | Prevention | @anatoly | P0 | Task 0.2 (partner-launch plan) |
| AI-3 | Live E2E in CI — asserts `200 text/html` on `/{roomId}` against an ephemeral server | Detection | @anatoly | DONE | commit `562633e` |
| AI-4 | SPA fallback in-process regression test — tempdir + tower `ServiceExt::oneshot` | Prevention | @anatoly | DONE | commit `3a98953` |
| AI-5 | File an upstream issue with tower-http documenting that `ServeDir::not_found_service` preserves the `404` status even when the fallback resolves | Process | @anatoly | P2 | none yet |
| AI-6 | Add `docs/runbooks/link-preview-broken.md` runbook with the `curl -sI` diagnosis and fix steps | Mitigation | @anatoly | P1 | none yet |
| AI-7 | Add synthetic Dozor probe: `server_web_fetch https://oxpulse.chat/TEST-0001`, assert 200 HTML + OG meta tags, run every 5 minutes | Detection | @anatoly | P1 | none yet |

## Lessons learned

1. **Every user-visible path needs at least one synthetic probe.** A 5-minute probe against a room URL would have caught this in minutes, not days. The cost of one probe is negligible; the cost of seven days of broken shares is not.
2. **"The tests pass" and "the server returns the right status" are different claims.** Verify with `curl` against the actual HTTP wire response, not just with library-level assertions inside the process. In this incident, the tower-http `not_found_service` body assertion would have passed in a unit test while the real wire response was still broken.
3. **Silent failures compound.** Bug A (router 404) and Bug B (analytics `data` drop) were simultaneously active in the same deploy window; neither could be detected via the other's signal. Observability design must assume the monitoring system itself is also broken and add independent out-of-band probes.
4. **Library defaults are not always correct for the app's contract.** `ServeDir` is a file server, not an SPA server. The SPA contract (rewrite unknowns to `index.html` with 200) must be explicit in application code, not assumed from the library.

## Prevention summary

The root cause is that the default `ServeDir` behavior was assumed correct for an SPA without verification. Prevention is three-pronged:

- **Better tests** — in-process regression test (`3a98953`) + live E2E (`e09f2e8`) landed this session. Both assert the exact wire response (`200` + `text/html; charset=utf-8` + body) rather than relying on library-level assumptions.
- **Better monitoring** — 4xx rate alert (AI-1, Task 3.2) + synthetic link-preview probe (AI-7). The synthetic probe is the strongest signal because it directly reproduces the user experience.
- **Better docs** — runbook for link-preview regression (AI-6) with the exact `curl -sI` command and expected output.

## Notes for future engineers

- tower-http's `ServeDir::not_found_service` preserves the outer `404` status even when the inner service resolves. Use `ServeDir::fallback(handler)` with an explicit handler if you need a specific status code.
- SvelteKit's `adapter-static` fallback contract assumes the serving layer rewrites unknown paths to `index.html` with `200`. Any Rust backend wrapping adapter-static output must implement that contract explicitly.
- When fixing a bug that involves HTTP responses, always verify with `curl -sI` against the actual running service, not just with in-process assertions. The wire format is the ground truth.

## Appendix: exact diff of the fix

The effective diff applied to `crates/server/src/router.rs` across `a0f4a4a` and `aafedc7`, relative to `a0f4a4a^`:

```diff
-use axum::http::header::{CACHE_CONTROL, CONTENT_SECURITY_POLICY, X_FRAME_OPTIONS};
-use axum::http::{HeaderValue, StatusCode};
+use axum::http::header::{CACHE_CONTROL, CONTENT_SECURITY_POLICY, CONTENT_TYPE, X_FRAME_OPTIONS};
+use axum::http::{HeaderMap, HeaderValue, StatusCode};
 use axum::response::IntoResponse;
 use axum::routing::{get, post};
 use axum::{Json, Router};
 use tower_http::services::ServeDir;
 use tower_http::set_header::SetResponseHeaderLayer;

+static SPA_INDEX: std::sync::OnceLock<String> = std::sync::OnceLock::new();
+
 pub fn build_router(state: AppState, room_assets_dir: &str) -> Router {
     let immutable_dir = ServeDir::new(format!("{room_assets_dir}/_app/immutable"));
     let fonts_dir = ServeDir::new(format!("{room_assets_dir}/fonts"));
-    let static_dir = ServeDir::new(room_assets_dir);
+    // SPA fallback: unknown paths (e.g. /{roomId}) must serve index.html with
+    // status 200 so the SvelteKit client router can take over AND link
+    // previewers (Telegram/iMessage) see a valid HTML page with OG tags.
+    // tower-http's ServeDir::not_found_service preserves 404 even when the
+    // fallback resolves, so we use an axum handler via ServeDir::fallback
+    // which does honor the handler's status code.
+    let index_html_path = format!("{room_assets_dir}/index.html");
+    match std::fs::read_to_string(&index_html_path) {
+        Ok(body) => {
+            SPA_INDEX.set(body).ok();
+        }
+        Err(e) => {
+            tracing::warn!(
+                path = %index_html_path,
+                error = %e,
+                "SPA index.html not found — fallback handler will serve a placeholder."
+            );
+        }
+    }
+    let static_dir = ServeDir::new(room_assets_dir)
+        .fallback(axum::handler::HandlerWithoutStateExt::into_service(spa_fallback));

     // ... router construction unchanged ...
 }

+async fn spa_fallback() -> impl IntoResponse {
+    let body = SPA_INDEX
+        .get()
+        .cloned()
+        .unwrap_or_else(|| "<!doctype html><html><body>OxPulse</body></html>".to_string());
+    let mut headers = HeaderMap::new();
+    headers.insert(CONTENT_TYPE, HeaderValue::from_static("text/html; charset=utf-8"));
+    (StatusCode::OK, headers, body)
+}
```
