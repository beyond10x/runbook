---
format: aep.planning-md/3
id: story:the-routes-row-says-whether-a-new-run-would-join-a-wake
kind: story
status: active
title: The routes row says whether a run submitted now would join a wake in flight
summary: wake_joinable, with the joined wake's instant and the end of its join, from the daemon's own join rule; wake_needed keeps its meaning
tags:
- consumer-asks-2026-11-28-pin-0-15
relations:
- informed_by: story:the-routes-view-answers-the-days-wake-figures
- informed_by: story:route-wake-behind-a-daily-cap
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-11-28T03:31:06Z", actor: "agent:runbook", revision: 2}
- {from: "proposed", to: "active", at: "2026-11-28T03:31:06Z", actor: "agent:runbook", revision: 3}
---
## Outcome

A row of the view `routes` (and a route of the view `catalog`, and of the MCP tool `list_routes`) for a managed route
with a wake cap says whether a run submitted now would join a wake in flight, which wake, and until when. A consumer
reads the daemon's own join rule instead of rebuilding it from the events.

## Context

A consumer repository that moved its pin to runbook 0.15.0 (2026-11-28, relayed by the host's operator session). Its
page rebuilds "a run submitted now joins the wake already sent" from `runbook.route-wake` and
`runbook.route-wake-answered` of `daemon-events`, and gets it wrong when the rule turns on an answer or on whether the
sending run still lives.

What runbook has on origin/main (`df6cd197`):

- The join rule is `wake_sent_within` (`runbook/src/commands.rs:937-954`): the newest sent wake of the route less than
  the route's `start_wait_seconds` old that may still start the model: answered `2xx` or `spend-ceiling`, or answered
  `5xx`, not answered in time or not answered yet while its run lives. A `4xx`, or a run that ended without a `2xx`,
  is not joined.
- `wake_route` applies it before it books a wake (`runbook/src/commands.rs:3021-3026`): the run writes
  `runbook.route-wake` with `sent: false`, `reason: "joined a wake sent within the wait"`, `joined` and `joined_at`
  (`docs/daemon-api.md:366-374`). A run wakes only when the route is managed, its wake is on and the gateway gave the
  phase `stopped` at the submit (`route_wait`, `runbook/src/commands.rs:1195-1199`).
- `wake_needed` (`runbook/src/commands.rs:1107-1112`; `docs/daemon-api.md:452`, "The day's figures of a route") means
  something else: a run waits now and the daemon sends no wake for it. It is false both when a wake may be joined and
  when no run waits, so it cannot answer the question.
- `wake_figures` (`runbook/src/commands.rs:1093-1120`) calls `wake_sent_within` only to negate it into `wake_needed`;
  the wake it finds is not answered.

## What this story does

1. ESS first (`sdc.daemon.ModelRoute`, the view `sdc.daemon.Routes`, `sdc.daemon.CatalogRoute`), then `wake_figures`:
   optional members for a managed route with a wake cap:
   - `wake_joinable`: true when `wake_sent_within` over the route's `start_wait_seconds` at the daemon's clock finds a
     wake, so a run submitted now that would wake the route joins it and sends and books none; false otherwise.
     Present with `wake_cap_micro_usd`.
   - `wake_joinable_sent_at` and `wake_joinable_until`: the `at` of that wake and that instant plus the route's
     `start_wait_seconds`, the last instant a run joins it (it may stop being joinable sooner: a `4xx` answer, its run
     ending without a `2xx`). Present while `wake_joinable` is true.
   The unit may name the members otherwise; the ESS summary says that the member does not say whether the run would
   wake at all (that turns on the phase and `wake_on_submit`, both in the row).
2. The members come from the one function the wake reads (`wake_sent_within`), so the view and `wake_route` never
   differ. `wake_needed` keeps its meaning; the docs set the two side by side.
3. `docs/daemon-api.md`, "The day's figures of a route", names the members.

## Acceptance

With the daemon's test clock and a stand-in gateway on loopback (`runbook/tests/daemon/route_wake.rs`, a section of
this story), a managed route with the wake on and `start_wait_seconds = 600`:

- Before any wake: `wake_joinable: false`, no instants.
- A run's wake answered `202` at 10:00:00 on the test clock: the row answers `wake_joinable: true`,
  `wake_joinable_sent_at` equal to that wake's `at` and `wake_joinable_until` 10:10:00; a run submitted at 10:05:00
  joins it (`joined_at` equal to `wake_joinable_sent_at`). At 10:10:00 the row answers false.
- A wake answered `404`: false at once. A wake answered `503` while its run lives: true; once that run is cancelled:
  false.
- A wake answered `spend-ceiling`: true within the wait (a run that joins it ends `stopped: route`).
- A managed route with the wake off answers `wake_joinable: false`; a managed route without wake keys, a hosted and a
  local route answer none of the members. The catalog's route and `list_routes` answer the same as the row.
- The OpenAPI document carries the members (generated from ESS).
- Each case runs under 5 s. No model call, no money, no network beyond loopback.

## Relations

- Informed by `story:the-routes-view-answers-the-days-wake-figures` (the row's figures, `wake_needed`) and
  `story:route-wake-behind-a-daily-cap` (the wake and its join).
- One change of the row with `story:the-routes-row-says-how-the-newest-wake-was-answered` and
  `story:a-refused-wake-records-the-cap-in-force`: all three add members to `wake_figures` and to the same three ESS
  types. It reads the route's wait that `story:the-routes-view-row-answers-start-wait-seconds` puts into the row.
