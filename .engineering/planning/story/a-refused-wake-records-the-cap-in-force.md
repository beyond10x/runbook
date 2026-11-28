---
format: aep.planning-md/3
id: story:a-refused-wake-records-the-cap-in-force
kind: story
status: active
title: A wake refused for the wake cap answers the cap in force at the refusal
summary: The refused runbook.route-wake documents cap_micro_usd, and the routes row answers the day's newest cap refusal and its cap
tags:
- consumer-asks-2026-11-28-pin-0-15
relations:
- informed_by: story:the-routes-view-answers-the-days-wake-figures
- informed_by: story:route-wake-behind-a-daily-cap
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-11-28T03:31:05Z", actor: "agent:runbook", revision: 2}
- {from: "proposed", to: "active", at: "2026-11-28T03:31:05Z", actor: "agent:runbook", revision: 3}
---
## Outcome

A wake refused for the day's wake cap ("wake cap reached") is answered with the cap in force at the refusal, in the
documented record of the run event and in the row of the view `routes`, so a client can tell a refusal made under a
cap that has since been raised from one that still holds.

## Context

A consumer repository that moved its pin to runbook 0.15.0 (2026-11-28, relayed by the host's operator session). The
operator raises a route's `wake_daily_cap_micro_usd` in `runbook.toml` and starts the daemon again; the client sees a
refusal and the row's cap, and cannot tell whether the refusal is still in force.

What runbook has on origin/main (`df6cd197`):

- `wake_route` writes the refusal: `sent: false`, `reason: "wake cap reached"`, with `day`, `cost_micro_usd`,
  `booked_micro_usd` and `cap_micro_usd` (`runbook/src/commands.rs:3032-3039`). The test holds that record
  (`runbook/tests/daemon/route_wake.rs:364`). The code writes the cap; the contract does not promise it:
  `docs/daemon-api.md:380-381` says the refusal carries "what the day booked so far" and names `cap_micro_usd` only for
  a sent wake (`docs/daemon-api.md:376`).
- The row of the view `routes` answers the cap declared now, `wake_cap_micro_usd` (`runbook/src/commands.rs:1100`;
  `docs/daemon-api.md:449`), and nothing of a refusal: `wake_figures` (`runbook/src/commands.rs:1093-1120`) answers
  `wake_needed`, which turns true when "the wake would pass the day's cap" (`docs/daemon-api.md:452`), but not when the
  cap refused a wake or under which cap. A client that reads the row, as the story of the row's figures intends,
  cannot tell.
- The cap is read from the daemon's file at start only (`Wake.daily_cap_micro_usd`, `runbook/src/routes.rs:97-104`):
  a raised cap takes effect at the next start, and the journal of the day before the restart stays.
- The route's wake events are indexed (`State::wake_events_of`, `runbook/src/state.rs:615-617`); the refusals are
  among them.

## What this story does

1. `docs/daemon-api.md` ("The wake of a managed route") and the ESS summary of `WaitForRoute`
   (`ess/domains/daemon.yaml`) say that a refused wake's `runbook.route-wake` carries `cap_micro_usd`, the cap in
   force when it was refused, and `pod_micro_usd` where it applies.
2. ESS first (`sdc.daemon.ModelRoute`, the view `sdc.daemon.Routes`, `sdc.daemon.CatalogRoute`), then `wake_figures`:
   two optional members for a managed route with a wake cap, present when a wake of the route was refused for the cap
   on the daemon's day:
   - `wake_cap_refused_at`: the `at` of the newest such `runbook.route-wake`;
   - `wake_cap_refused_cap_micro_usd`: its `cap_micro_usd`, the cap that refused it.
   A client compares it with `wake_cap_micro_usd`; the unit may name the two members otherwise and says so in the ESS
   summary.
3. The members are read from the route's wake events (`State::wake_events_of`); no new journal line.

## Acceptance

With the daemon's test clock and a stand-in gateway on loopback (`runbook/tests/daemon/route_wake.rs`, a section of
this story):

- A route with a wake cap of 250 and a cost of 100, two wakes taken: the third run's wake is refused, its
  `runbook.route-wake` has `cap_micro_usd: 250`, and the row answers `wake_cap_refused_at` equal to that event's `at`
  and `wake_cap_refused_cap_micro_usd: 250`, with `wake_cap_micro_usd: 250`.
- The daemon is stopped and started again on the same state directory, the same day of the test clock, with the cap
  raised to 500: the row answers `wake_cap_micro_usd: 500` and still `wake_cap_refused_cap_micro_usd: 250`; a new run's
  wake is sent and booked.
- After 00:00 UTC on the test clock both refusal members are absent.
- A route whose wakes were never refused, a managed route without wake keys, a hosted and a local route answer
  neither member. The catalog's route and `list_routes` answer the same as the row.
- The OpenAPI document carries both members (generated from ESS).
- Each case runs under 5 s. No model call, no money, no network beyond loopback.

## Relations

- Informed by `story:the-routes-view-answers-the-days-wake-figures` (the row's wake figures) and
  `story:route-wake-behind-a-daily-cap` (the cap and its refusal).
- One change of the row with `story:the-routes-row-says-how-the-newest-wake-was-answered` and
  `story:the-routes-row-says-whether-a-new-run-would-join-a-wake`: all three add members to `wake_figures` and to the
  same three ESS types.
