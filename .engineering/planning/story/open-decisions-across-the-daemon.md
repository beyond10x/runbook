---
format: aep.planning-md/3
id: story:open-decisions-across-the-daemon
kind: story
status: draft
title: A daemon-wide view of the open decisions of all runs, filtered by session
summary: open-decisions takes one run; a client that needs every open decision lists runs and asks per run, racing with them.
tags:
- consumer-asks-2026-11-25
relations:
- depends_on: story:daemon-runs-take-a-gateway
- informed_by: story:daemon-skeleton-on-a-runtime-trait
- informed_by: story:mcp-exposure
scope:
- confidence: cited
  path: docs/daemon-api.md
- confidence: cited
  path: docs/mcp.md
- confidence: cited
  path: ess/domains/daemon.yaml
- confidence: inferred
  path: ess/generated
- confidence: cited
  path: runbook/src/api/routes.rs
- confidence: cited
  path: runbook/src/commands.rs
- confidence: cited
  path: runbook/src/mcp/tools.rs
- confidence: inferred
  path: runbook/tests/daemon
revision: 6
---
## What is missing

The view `open-decisions` takes a required `run_id` and lists that run's Open decision requests (`docs/daemon-api.md:112`; `runbook/src/api/routes.rs:140`; `runbook/src/commands.rs:1026-1033`), and so does the MCP tool `read_open_decisions` (`docs/mcp.md:48`). A client that wants every decision a person has to take, across all runs of the daemon or of one session, lists the runs (`runs`, `runs-of-session`: `docs/daemon-api.md:107-109`) and asks one view per run. The listing races with the runs it lists: a request opened or withdrawn between two calls is missed or shown stale. The daemon has the pattern for a daemon-wide read next to a per-run one: `events` and `daemon-events` (`docs/daemon-api.md:166-171`).

## Outcome

One new view, `open-decisions-of-daemon` (the name follows `daemon-events`), with the optional parameter `session_id`. It answers every Open decision request of every run of the daemon, or of the runs of that session, newest first, as one read under one lock. Each row has the members of the rows of `open-decisions` (`request_id`, `run_id`, `case_id`, `action`, `capability`, `question`, `options`, `opened_at`) and the `session_id` of its run, absent for a run with none. The MCP tool `read_open_decisions` makes `run_id` optional and follows the rule of `list_runs`: given `run_id` it calls `open-decisions`, otherwise `open-decisions-of-daemon` with the optional `session_id` (`docs/mcp.md:45`). `open-decisions` is unchanged.

## What a guard must hold

- Only Open requests are listed: an Answered one and a Withdrawn one (after `cancel-run` or an orphaned run, `docs/daemon-api.md:83-85`) are not.
- A closed session's runs are listed with the rest; an unknown `session_id` answers no row, as `runs-of-session` does (`runbook/src/commands.rs:976-978`).
- The view sits behind the same token check as every route (`docs/daemon-api.md:227`, `:268`), and the rows carry no member the per-run view does not carry beyond `session_id`.
- The view is a read: it writes no event and no journal line.

## Acceptance

With the stub runtime and two sessions, `a` with one run waiting on a decision and `b` with two runs of which one waits, and one run with no session that waits: `open-decisions-of-daemon` lists four rows, each with its `run_id`; with `session_id=b` it lists one; with `session_id=nope` it lists none. After `cancel-run` of one run, its row is gone; after the answer of a request, its row is gone. `read_open_decisions` with no argument returns the four rows, with `session_id` `b` the one, and with `run_id` the same rows as before this change. `ess specify validate --path ess` passes and `task check:ess-generated` holds the OpenAPI document equal to the specification.

## Scope

`runbook/src/commands.rs`, `runbook/src/api/routes.rs`, `runbook/src/mcp/tools.rs`, `ess/domains/daemon.yaml`, `ess/generated`, `docs/daemon-api.md`, `docs/mcp.md`, `runbook/tests/daemon`.

## Depends on

- `story:daemon-runs-take-a-gateway`: the order is behind the gateway release; there is no code dependency.
- Informed by `story:daemon-skeleton-on-a-runtime-trait` (the views and the specification they are generated from) and `story:mcp-exposure` (the tool layer).
- Order: fourth; it shares `runbook/src/mcp/tools.rs` and `docs/mcp.md` with `story:decision-options-come-from-a-case-artifact` and `story:mcp-opens-and-closes-sessions`, so it lands after both.
