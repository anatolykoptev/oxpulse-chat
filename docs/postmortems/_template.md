<!--
POSTMORTEM TONE RULES (delete this block when you copy the template):

1. BLAMELESS. Describe what systems and processes did, not what people did.
   Bad:  "Alice forgot to add the test."
   Good: "The pre-merge checklist did not require a DB integration test for
          new columns; this gap let the change land untested."

2. FACTUAL. Every claim is either a commit hash, a log line, a timestamp, or
   a verifiable measurement. No speculation about feelings or motives.

3. SPECIFIC ACTION ITEMS. "Improve X" is not an action item. Name the metric,
   the alert threshold, the file path, the owner.

4. TIMELINE IN UTC unless noted. Include pre-detection events and missed
   detection opportunities, not just the fix.

5. SEPARATE DIRECT CAUSE FROM CONTRIBUTING CAUSES. They are different things.
   A bug has one direct cause and many contributing causes.

6. DON'T SKIP SECTIONS. If a section doesn't apply, write "N/A" and one
   sentence explaining why. Empty sections erode the template.

7. REVIEW BEFORE FINAL. Tag status as DRAFT first, get one reviewer, then FINAL.

This template is based on Google SRE Workbook Chapter 10, adapted for a
small-team single-operator service.
-->

# [Incident title — brief, no blame, no "I screwed up"]

**Status:** DRAFT | IN REVIEW | FINAL
**Severity:** SEV-1 | SEV-2 | SEV-3 | SEV-4
**Incident start:** YYYY-MM-DD HH:MM UTC
**Incident end:** YYYY-MM-DD HH:MM UTC
**Duration:** [computed]
**Author:** @handle
**Reviewers:** @handle1, @handle2
**Related commits:** [hash1, hash2, ...]
**Related runbooks:** [paths]

> **Severity:** SEV-1 full outage/data loss/breach; SEV-2 major break >10% users; SEV-3 degraded subset; SEV-4 internal-only.

## Summary

One paragraph. What broke, how long, root cause in one sentence, fix in one sentence, who detected and how. Written for a reader with zero context on the service.

## Impact

- User-visible impact: [requests failed, features unavailable, error messages shown]
- Business/revenue impact: [lost conversions, SLA credits, refunds]
- Data loss: [row count, time window, recoverable yes/no]
- Customers/tenants affected: [count, names or segments]
- Secondary impact: [downstream services degraded, bugs this masked or unmasked]

## Timeline

All times UTC unless noted. Include pre-detection events.

- YYYY-MM-DD HH:MM — [event]
- YYYY-MM-DD HH:MM — [event]
- YYYY-MM-DD HH:MM — [detection — how, by whom, via what signal]
- YYYY-MM-DD HH:MM — [mitigation attempt]
- YYYY-MM-DD HH:MM — [root cause identified]
- YYYY-MM-DD HH:MM — [fix deployed]
- YYYY-MM-DD HH:MM — [incident declared resolved]

## Root cause

1. **Direct cause:** the specific thing that broke.
2. **Contributing cause (possibility):** what made the direct cause possible to introduce.
3. **Contributing cause (catchability):** what made the direct cause hard to catch in review or testing.
4. **Contributing cause (detectability):** what made the direct cause hard to detect in production.

## Detection

- How was the incident actually detected? [alert, user report, routine check]
- How long after introduction? [minutes, hours, days — compute from commit timestamp]
- Missed detection opportunities: [signals that existed but were not alerted on]
- Ideal detection path: [what signal, what threshold, what alert would have caught it in under N minutes]

## What went well

- [At least two items. Examples: fast rollback, clean commit history aided bisect, runbook existed, on-call responded quickly.]
- [Item 2]

## What went poorly

Describe systems and processes, not individuals. Prefer "the CI job did not run integration tests" over "we forgot to run tests."

- [Item 1]
- [Item 2]

## Action items

| # | Action | Type | Owner | Priority | Tracking |
|---|---|---|---|---|---|
| AI-1 | [specific, actionable, testable] | Prevention | @handle | P0 | [link or task id] |
| AI-2 | [specific, actionable, testable] | Detection | @handle | P1 | [link] |
| AI-3 | [specific, actionable, testable] | Mitigation | @handle | P2 | [link] |
| AI-4 | [specific, actionable, testable] | Process | @handle | P1 | [link] |

> **Type:** Prevention (stops the class of bug) | Detection (catches it faster) | Mitigation (reduces blast radius) | Process (review/testing/alerting standards).
>
> Every action item must be specific, assigned, and tracked. "Improve monitoring" is not an action item; "Add Prometheus metric `foo_errors_total` and Dozor alert on `rate > 0.01`" is.

## Lessons learned

1. [One sentence general principle.]
2. [One sentence general principle.]
3. [One sentence general principle.]

## Prevention summary

One paragraph, plain language, restating the single most important preventive action and why it closes the class of bug.

## Appendix

Optional. Include diffs, log excerpts, screenshots, queries, or supporting commands only if a reviewer would need them to understand the incident. Otherwise write "N/A".
