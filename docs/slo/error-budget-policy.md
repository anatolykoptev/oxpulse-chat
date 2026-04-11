# Error Budget Policy

## 1. Purpose

This policy says what we do when the error budget for an SLO is partially or fully consumed. It exists so reliability work isn't negotiable under feature-velocity pressure.

## 2. Definitions

- **Error budget:** `1 - SLO_target`, expressed as time you're allowed to be broken per window. For a 99.9% SLO over 28 days, that's ~40 minutes of allowed badness.
- **Burn rate:** how fast the budget is being consumed relative to a flat 1.0 baseline. A burn rate of 1.0 means you are on track to exactly exhaust the budget at the end of the window. A burn rate of 10 means you will exhaust it 10× faster.
- **Burn-rate alert tiers:**
  - **Page (fast burn):** 14.4× over a 1h/5m window → exhausts 2% of budget in 1h → on-call gets paged.
  - **Ticket (slow burn):** 6× over a 6h/30m window → exhausts 5% of budget in 6h → next-day investigation.

The four SLOs this policy covers are defined in `docs/slo/slo.md`: `signaling_availability`, `signaling_latency`, `turn_cred_latency`, and `ws_session_success`. This document does not redefine them.

## 3. Policy — what we do when budget is consumed

| Budget remaining | Feature work | Reliability work | Response |
|---|---|---|---|
| > 80% | Normal velocity | As planned | Ship features. Write postmortems for any blip. |
| 50–80% | Normal velocity | Prioritized above new feature work for the next sprint | Lowered tolerance for flaky tests. Stricter PR review. |
| 20–50% | Freeze new features on the affected surface | Top priority | Mandatory postmortem for every incident, even transient. |
| 0–20% | **Full feature freeze on the affected SLO's surface** | All hands on reliability | Daily status update to stakeholders (partner operator via Telegram). |
| Exhausted (0) | **Full freeze. No exceptions. No "small" features.** | Only reliability work allowed | Public commitment to restore SLO before any feature PR merges. |

**"Surface" means the area of the service the SLO measures, not the whole service.** If `turn_cred_latency` is burning, frontend UI work can still ship; TURN-pool refactors cannot. If `signaling_availability` is burning, the WebSocket handler, signaling server process, and its deploy pipeline are frozen — but TURN credential caching work is not. When in doubt, freeze wider, not narrower.

Surface mapping, for clarity:

- `signaling_availability` → WebSocket handler, signaling server process, ingress/load balancer, deploy pipeline.
- `signaling_latency` → signaling hot path: message routing, session state, fan-out, serialization.
- `turn_cred_latency` → TURN credential issuance path: auth, HMAC signing, credential cache, TURN admin API client.
- `ws_session_success` → connection lifecycle: handshake, reconnect logic, session close handling.

Two surfaces burning at once means both are frozen, and the operator evaluates whether the root cause is a shared dependency (DB, Redis, config pipeline) that should freeze everything.

## 4. Exception process

If a critical security fix or legal/compliance change must merge during a freeze, the operator waives the freeze in writing with explicit rationale. For a 1-person team, a PR comment is sufficient. The waiver is logged in the next postmortem and counts against future freeze discretion.

## 5. Reset conditions

A freeze ends when **either**:
- The SLO is back above the 50% budget threshold for 7 consecutive days, **or**
- The 28-day rolling window rolls over and the new window starts above 50%.

Whichever comes first. The operator announces the reset in the same channel as the freeze declaration.

## 6. Who is responsible

For a 1-person team, the operator is responsible for:
- Watching the burn-rate alerts (Dozor → Telegram).
- Calling the freeze when a tier is triggered.
- Writing the postmortem.
- Deciding the reset.

When the team grows beyond one engineer, this policy needs a rewrite with defined roles (incident commander, freeze approver, postmortem author).

## 7. Review cadence

Quarterly. Policy is renegotiated if SLOs change, if burn-rate alerts prove noisy or silent, or if the team size changes.

Next review: **2026-07-10**
