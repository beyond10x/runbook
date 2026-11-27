---
format: aep.planning-md/3
id: story:the-routes-view-answers-the-days-wake-figures
kind: story
status: active
title: The routes view answers each route's wake figures for the daemon's day
summary: Booked wakes, wake cap, wake cost, last wake sent, wake needed and the last spend-ceiling refusal per route, in the views routes and catalog
tags:
- consumer-asks-2026-11-27-agents
relations:
- informed_by: story:route-wake-behind-a-daily-cap
- informed_by: story:a-wake-is-booked-only-when-it-succeeds
- informed_by: story:daemon-page-shows-the-route
revision: 7
transitions:
- {from: "draft", to: "proposed", at: "2026-11-27T22:03:34Z", actor: "agent:runbook", revision: 2}
- {from: "proposed", to: "active", at: "2026-11-27T22:03:35Z", actor: "agent:runbook", revision: 3}
---
## Outcome

Each route row of the view `routes` and of the view `catalog` answers the daemon's day figures of its wake:
`wake_booked_micro_usd`, `wake_cap_micro_usd`, `wake_cost_micro_usd`, `wake_last_sent_at`, `wake_needed` and
`ceiling_refused_at`. A client reads them in one small request instead of rebuilding them from every event of the
daemon.

## Context

Gap G1 of a consumer repository that builds a page which dispatches agents through the daemon (2026-11-27, relayed by
the host's operator session). Today it rebuilds these figures from `GET /daemon/views/daemon-events` from position 0
(2.8 MB on its daemon).

What runbook has today (origin/main `76551cdd`):

- A route's wake is configuration: `Wake { on_submit, daily_cap_micro_usd, cost_micro_usd }`
  (`runbook/src/routes.rs:95-99`). Only a managed route may carry it (`runbook/src/routes.rs:289-293`).
- The day's booked figure is no stored counter. `wakes_booked` sums `cost_micro_usd` over the events
  `runbook.route-wake` with `sent: true` of the route and day across the whole journal
  (`runbook/src/commands.rs:871-877`). The day is the first 10 characters of the daemon's instant
  (`runbook/src/commands.rs:2509`).
- `wake_route` writes `runbook.route-wake` with `sent`, `day`, `cost_micro_usd`, `booked_micro_usd` and
  `cap_micro_usd` (`runbook/src/commands.rs:2512-2517`). After the call it writes `runbook.route-wake-answered` with
  the gateway's `status` and `code` (`runbook/src/commands.rs:2522-2526`).
- The last wake sent: `wake_sent_within` finds the newest sent wake of the route within its wait
  (`runbook/src/commands.rs:843-852`).
- "A wake is needed" is the filter `route_wait` applies: managed, `wake_on_submit`, phase `stopped` at the submit
  (`runbook/src/commands.rs:908-912`). No view answers it.
- The gateway's `spend-ceiling` answer is `answered_ceiling` (`runbook/src/commands.rs:865-867`; the code is
  `runbook/src/routes.rs:537`). No time is kept apart from the event's `at`.
- The view `routes` rows come from `rows_of` (`runbook/src/routes.rs:696-740`; `Daemon::routes`,
  `runbook/src/commands.rs:2188`). They hold `route, gateway, alias, kind, wires, status`, plus `phase`,
  `max_model_len`, `tool_calling` and `read_at` when known. The catalog adds the prices and `start_wait_seconds`
  (`runbook/src/commands.rs:2211-2228`). The ESS struct `sdc.daemon.CatalogRoute` (`ess/domains/daemon.yaml:250`)
  has none of the six fields. The view `daemon-events` is `runbook/src/api/routes.rs:159`.

## What this story does

1. The fields are optional members of a route row of `routes` and of `catalog`, in the ESS domain first
   (`ess/domains/daemon.yaml`: the entity `sdc.daemon.ModelRoute`, the view `sdc.daemon.Routes` and the struct
   `sdc.daemon.CatalogRoute`), then in the handler (`Daemon::routes` and `wake_figures`, `runbook/src/commands.rs`):
   - `wake_booked_micro_usd`: what the route's wakes booked on the daemon's day, `wakes_booked`: the figure the next
     wake is checked against (a wake books on a 2xx answer, and while it is out).
   - `wake_cap_micro_usd` and `wake_cost_micro_usd`: the route's declared values.
   - `pod_booked_micro_usd` (added in this story): for a managed route that declares `price_per_hour_micro_usd`, the
     pod's time of the day up to now at that price, `pod_booked`, the other part of what the cap counts. The cap sends
     no wake when `wake_booked_micro_usd + pod_booked_micro_usd + wake_cost_micro_usd` is above `wake_cap_micro_usd`,
     and the event `runbook.route-wake` holds the same figures (`booked_micro_usd` the sum, `pod_micro_usd` the pod's
     part).
   - `wake_last_sent_at`: the `at` of the newest sent `runbook.route-wake` of the route, on any day.
   - `wake_needed`: a run waits on the route and the daemon sends no wake for it. True when the route's wake is on
     (`wake_on_submit`), the phase the view last read of the route is `stopped`, a run that takes the route (or a delegation the daemon classifies, on the classifier route) is
     `WaitingForRoute`, no wake of the route is out (sent, not answered yet, its run live, also past the route's wait)
     and none may be joined (`wake_sent_within` over the route's
     `start_wait_seconds`: a wake the gateway took or answered `spend-ceiling`, or one out, failed or not answered
     while its run lives). It is so when the wake the run would send passed the day's cap, after a `4xx` answer, and
     when the wake a run joined is past its wait while the run still waits. False otherwise, also for a route whose
     wake is off.
   - `ceiling_refused_at`: the `at` of the newest `spend-ceiling` answer to a wake of the route on the daemon's day: a
     `runbook.route-wake-answered`, or the daemon's line `route.wake-answered` of a run closed before the answer came.

   The wake figures answer for a managed route that declares `wake_daily_cap_micro_usd`, and `pod_booked_micro_usd`
   for a managed route that declares `price_per_hour_micro_usd`. On any other route, and while there is no such
   instant, the member is absent: an `Optional` member of the language is absent, not a JSON `null` (the OpenAPI type
   of such a member is not nullable); a client that indexes the row reads it as null.
2. The view does not walk the whole record on every read: the state keeps the wake events of each route in an index
   (`State::wake_events_of`), filled where an event is applied, so a daemon started again rebuilds it from the journal.
   The booking of a wake (`sent_wakes`) walks the same index. The figures come from the functions the cap uses
   (`wakes_booked`, `pod_booked` through `day_booked`, `wake_sent_within`), so the view and the cap read one figure.
   The pod's time walks the runs that take the route, as the cap does.
3. When `story:a-wake-is-booked-only-when-it-succeeds` changes what books, `wake_booked_micro_usd` follows the same
   rule: it is `wakes_booked`.

## Acceptance

With the daemon's test clock (`Daemon::start_with_clock`) and a stand-in gateway on loopback
(`runbook/tests/daemon/route_wake.rs`, the section of this story):

- A managed route with a wake cap of 3,000,000 and a cost of 1,000,000: before any wake the row answers
  `wake_booked_micro_usd: 0` and no `wake_last_sent_at`; after two wakes it answers `wake_booked_micro_usd: 2000000`,
  the cap, the cost, and `wake_last_sent_at` equal to the second wake's event `at`. The catalog's route and the MCP tool
  `list_routes` answer the same.
- After 00:00 UTC on the test clock the row answers `wake_booked_micro_usd: 0`, and `wake_last_sent_at` still names
  the earlier wake.
- A wake the stand-in answers `503` books nothing and is the last wake sent; a wake still out counts, as the cap counts
  it.
- A wake the stand-in answers `spend-ceiling` sets `ceiling_refused_at` to that event's `at`. On the next day it is
  absent.
- `wake_needed` is true while a run waits on a managed route with the wake on, the stand-in reports the phase
  `stopped` and no wake is out or may be joined (the run's wake passed the day's cap); false with no run waiting,
  while the run's wake was taken, once the stand-in reports `ready`, and for a managed route with the wake off.
- A hosted route, a local route and a managed route without wake keys answer none of the fields.
- A daemon started again on the same state directory answers the same booked figures and instants as before its
  stop (`wake_needed` follows the runs: the restart closes a run that waited).
- A managed route with `price_per_hour_micro_usd`: `pod_booked_micro_usd` holds the pod's time up to now while a run
  uses the pod and the run's share once it ended; a wake not sent for "wake cap reached" has `booked_micro_usd` equal
  to `wake_booked_micro_usd + pod_booked_micro_usd` and `pod_micro_usd` equal to `pod_booked_micro_usd` as the view
  answered them, and those plus the cost are above the cap.
- The OpenAPI document and the MCP tool `list_routes` carry the fields (generated from ESS).
- Each case runs under 5 s. No model call, no pod and no network beyond loopback.

## Evidence

- The consumer's design, section 8, item G1 (read 2026-11-27): its page rebuilds the figures from the events
  `runbook.route-wake` and `runbook.route-wake-answered` of `daemon-events`, 2.8 MB from position 0.
- `grep` of `ess/domains/daemon.yaml` for `wake_booked|wake_cap|wake_needed|wake_last|wake_cost|ceiling_refused`:
  no line.

## Relations

- Informed by `story:route-wake-behind-a-daily-cap` (the wake and its cap) and `story:a-wake-is-booked-only-when-it-succeeds`
  (what books). `story:daemon-page-shows-the-route` says "today's calls and booked spend" are not in the view `routes`
  yet. This story adds the wake's figures only; the calls and model spend per route stay that story's.
