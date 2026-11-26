---
format: aep.planning-md/3
id: story:the-daemon-serves-a-runs-metrics
kind: story
status: active
title: The daemon serves a run's metrics
summary: A read-only view answers a run's state, times, steps, model calls, tokens and cost from the daemon's own record; a UI pane file renders it.
relations:
- informed_by: story:daemon-page-shows-a-run-tree-and-products
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-11-26T19:18:50Z", actor: "agent:runbook", revision: 3}
- {from: "proposed", to: "active", at: "2026-11-26T19:18:51Z", actor: "agent:runbook", revision: 4}
---
# Story: the-daemon-serves-a-runs-metrics

## Outcome

The daemon serves what a run cost and how long it took, read from its own record: the read-only view
`run-metrics` (`GET /daemon/views/run-metrics?run_id=...`) answers one row per run with its state, outcome,
start, end or time running, duration, a row per step and the run's totals. The web UI has a pane file that renders
that row; the Metrics tab can come back with content once the pane is wired.

## Context

The operator, 2026-11-26, from the daemon's page: "The daemon serves no metrics of a case: the case tree and the
Products tab show what it serves." The journal holds per run the daemon's `runbook.action-started` (case, action,
role, try), Loom's `ActionFinished` and `HandoffChecked` lines (action, exit), the relay's `runbook.model-call`
(step, route, tokens where the answer reported them, `cost_known`, `micro_usd`) and `runbook.cost-booked` (calls and
micro USD of a step of a run on no route). The spend ledger holds the same calls without an instant.

## Acceptance

- The view is declared in `ess/domains/daemon.yaml` (entity `RunMetrics`, view `MetricsOfRun`, wire name
  `run-metrics`) and `ess/generated/` is regenerated from it; `task check:ess-generated` passes.
- A row holds `run_state`, `outcome`, `submitted_at`, `started_at`, `ended_at`, `running_ms` (a live run) and
  `duration_ms` (an ended run with both instants), `steps` and the totals. A step row is one action of one case in one
  role: `case_id`, `action`, `role`, `tries`, `step_state` (`running`, `ended`, `unfinished`), `started_at`, `ended_at`,
  `exit`, `duration_ms` or `running_ms`, `model_calls`, `input_tokens`, `output_tokens`, `calls_without_tokens`,
  `cost_micro_usd` (calls of known cost only), `unknown_cost_calls` and `booked_micro_usd`.
- A value the records do not hold is absent, never 0: tokens no call reported, an end no line recorded, a duration
  without both instants. A call of unknown cost adds nothing to `cost_micro_usd` and one to `unknown_cost_calls`.
- An unknown run is `404` `no such run`; a query without `run_id` is `400`.
- The read is bounded: it walks the run's own events once (the per-run index, not the daemon's whole record) and
  keeps at most 500 step rows; the steps past the bound are counted in `steps_left_out`, and their calls stay in the
  totals.
- Cases: `runbook/tests/daemon/run_metrics.rs`, red first: exact numbers on a scripted runtime with a set clock, a
  live run and a cancelled one, the bound, and a run on a stand-in route through the relay (priced and unpriced). No
  model call, no network beyond loopback.
- `docs/daemon-api.md` has a section for the view; `ui/daemon/metrics.js` renders the row into a pane with text
  nodes only, with a spec under `ui/test/` against a synthetic row; no shared UI file is edited.

## Not in scope

- Today's calls and booked spend per route (the Routes dialog's missing count): the ledger holds no instant, and a
  read of the whole journal per request is not bounded.
- An MCP tool for the view, and wiring the pane into `ui/web/app.js`.
