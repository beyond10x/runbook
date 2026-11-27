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
revision: 3
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

1. The six fields are members of a route row of `routes` and of `catalog`, in the ESS domain first
   (`ess/domains/daemon.yaml`, `sdc.daemon.CatalogRoute` and the row of `sdc.daemon.Routes`), then in the handler:
   - `wake_booked_micro_usd`: the figure `wakes_booked` gives for the route and the daemon's day.
   - `wake_cap_micro_usd` and `wake_cost_micro_usd`: the route's declared values.
   - `wake_last_sent_at`: the `at` of the newest sent `runbook.route-wake` of the route, on any day.
   - `wake_needed`: the filter of `route_wait`, read from the route's last read phase.
   - `ceiling_refused_at`: the `at` of the newest `runbook.route-wake-answered` of the route with code
     `spend-ceiling` on the daemon's day.

   A field that does not apply is null: a route that is not managed, or one with no wake declared.
2. The view does not scan the whole journal on every read. The daemon keeps the day's figures per route in memory,
   updated where the two events are written and rebuilt from the journal at start, as `wakes_booked` rebuilds them.
3. When `story:a-wake-is-booked-only-when-it-succeeds` changes what books, `wake_booked_micro_usd` follows the same
   rule. It is the figure the next wake is checked against.

## Acceptance

With the daemon's test clock (`Daemon::start_with_clock`) and a stand-in gateway on loopback:

- A managed route with a wake cap of 3,000,000 and a cost of 1,000,000: after two wakes the row answers
  `wake_booked_micro_usd: 2000000`, the cap, the cost, and `wake_last_sent_at` equal to the second wake's event `at`.
- After 00:00 UTC on the test clock the row answers `wake_booked_micro_usd: 0`, and `wake_last_sent_at` still names
  the earlier wake.
- A wake the stand-in answers `spend-ceiling` sets `ceiling_refused_at` to that event's `at`. On the next day it is null.
- `wake_needed` is true while the stand-in reports the phase `stopped` for a managed route with the wake on, and false
  for `ready`, for a hosted route and for a managed route with the wake off.
- A hosted and a local route answer the six fields as null.
- A daemon started again on the same state directory answers the same figures as before its stop.
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
