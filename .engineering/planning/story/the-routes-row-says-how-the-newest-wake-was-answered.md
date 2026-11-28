---
format: aep.planning-md/3
id: story:the-routes-row-says-how-the-newest-wake-was-answered
kind: story
status: draft
title: The routes row says how the newest wake of a route was answered, and when
summary: wake_last_answer (2xx, 4xx, 5xx, spend-ceiling, no answer, none yet) and wake_last_answered_at in the views routes and catalog
tags:
- consumer-asks-2026-11-28-pin-0-15
relations:
- informed_by: story:the-routes-view-answers-the-days-wake-figures
- informed_by: story:route-wake-behind-a-daily-cap
revision: 1
---
## Outcome

A row of the view `routes` (and a route of the view `catalog`, and of the MCP tool `list_routes`) for a managed route
with a wake cap says how the newest sent wake of the route was answered, and when: a `2xx`, a `4xx`, a `5xx`,
`spend-ceiling`, no answer within the daemon's bound, or none yet (the wake is out). A client then reads the end of a
gateway's spend ceiling from the row instead of guessing it from "a wake sent after `ceiling_refused_at`".

## Context

A consumer repository that moved its pin to runbook 0.15.0 (2026-11-28, relayed by the host's operator session).
Today its page takes "a wake sent after `ceiling_refused_at`" as the end of the ceiling. That rule is wrong both
ways: a later wake answered `4xx`, `5xx` or not at all says nothing about the ceiling, and a wake still out has not
said anything yet.

What runbook has on origin/main (`df6cd197`):

- The row's wake figures come from `wake_figures` (`runbook/src/commands.rs:1093-1120`), merged into each row by
  `Daemon::routes` (`runbook/src/commands.rs:2598-2617`). The row names the newest sent wake's instant only:
  `wake_last_sent_at` (`runbook/src/commands.rs:1104-1106`), "whatever its answer" (`docs/daemon-api.md:451`).
- `ceiling_refused_at` is the newest `spend-ceiling` answer of the day (`ceiling_refused_at`,
  `runbook/src/commands.rs:1077-1082`; `docs/daemon-api.md:453`). No member says whether a later wake was answered,
  or how.
- The journal holds every answer: `wake_route` writes `runbook.route-wake` (`runbook/src/commands.rs:3033`) and, after
  the call, `runbook.route-wake-answered` with `status` (null when no answer came within 5 s) and `code`
  (`runbook/src/commands.rs:3045-3058`); a run closed while its wake was out takes the daemon's line
  `route.wake-answered` instead (`Fact::RouteWakeAnswered`, `runbook/src/commands.rs:3057`; `WakeAnswer { run_id,
  record, at }`, `runbook/src/state.rs:178-181`).
- `sent_wakes` pairs each sent wake with its answer (`SentWake`, `runbook/src/commands.rs:895-905`; the walk,
  `runbook/src/commands.rs:909-932`), but keeps the answer's record only, not its `at`. A wake whose daemon went away
  before its answer has none for good (`runbook/src/commands.rs:900-904`).
- The ESS row types have no such member: `sdc.daemon.ModelRoute` (`ess/domains/daemon.yaml:453`, wake members
  `ess/domains/daemon.yaml:469-474`), the view `sdc.daemon.Routes` (`ess/domains/daemon.yaml:1888`, members
  `ess/domains/daemon.yaml:1908-1913`) and `sdc.daemon.CatalogRoute` (`ess/domains/daemon.yaml:259`, members
  `ess/domains/daemon.yaml:276-282`).

## What this story does

1. ESS first (`ess/domains/daemon.yaml`: `sdc.daemon.ModelRoute`, the view `sdc.daemon.Routes`,
   `sdc.daemon.CatalogRoute`), then `wake_figures`: two optional members, present with `wake_last_sent_at`:
   - `wake_last_answer`: how the newest sent wake of the route (the one `wake_last_sent_at` names) was answered:
     `2xx`, `4xx`, `5xx` (a status class of `status`), `spend-ceiling` (the `code`, whatever the status), `no-answer`
     (`status` null: not answered within the daemon's bound), `none-yet` (not answered, its run lives: the wake is
     out), or `lost` (not answered and its run no longer lives: the daemon went away while it was out). The unit may
     rename the words; it keeps one word per case and writes them in the ESS summary.
   - `wake_last_answered_at`: the `at` of that answer, the run event `runbook.route-wake-answered` or the daemon's line
     `route.wake-answered`. Absent for `none-yet` and `lost`.
2. `SentWake` carries the answer's `at` beside its record, so the row reads both from the walk the cap and the view
   already share (`sent_wakes` over `State::wake_events_of`). No new journal line.
3. `docs/daemon-api.md`, "The day's figures of a route", says which answer ends a ceiling for a client: the newest
   wake's answer, not the instant of a send; `ceiling_refused_at` keeps its meaning.

## Acceptance

With the daemon's test clock and a stand-in gateway on loopback (`runbook/tests/daemon/route_wake.rs`, a section of
this story):

- A wake the stand-in answers `202`: the row answers `wake_last_answer: "2xx"` and `wake_last_answered_at` equal to
  the `at` of its `runbook.route-wake-answered`.
- A wake answered `spend-ceiling`, then a later wake (next day of the test clock) answered `404`: the row answers
  `4xx` with the second answer's `at`; a later wake answered `503` answers `5xx`.
- A wake the stand-in holds past the daemon's bound: `none-yet` while it is out, then `no-answer` with the instant the
  daemon wrote the null status.
- A wake whose run was cancelled before the answer came: the row answers the class of the daemon's line
  `route.wake-answered` and its `at`.
- A daemon killed with a wake out and started again on the same state directory: `lost`, no `wake_last_answered_at`.
- A managed route with no wake sent, a managed route without wake keys, a hosted and a local route answer neither
  member. The catalog's route and `list_routes` answer the same as the row.
- The OpenAPI document carries both members (generated from ESS).
- Each case runs under 5 s. No model call, no money, no network beyond loopback.

## Relations

- Informed by `story:the-routes-view-answers-the-days-wake-figures` (the row's wake figures, `ceiling_refused_at`) and
  `story:route-wake-behind-a-daily-cap` (the wake and its answers).
- One change of the row with `story:a-refused-wake-records-the-cap-in-force` and
  `story:the-routes-row-says-whether-a-new-run-would-join-a-wake`: all three add members to `wake_figures` and to the
  same three ESS types. They stay three stories; built together or one after another, they touch the same lines.
