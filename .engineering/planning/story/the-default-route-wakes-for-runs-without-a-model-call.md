---
format: aep.planning-md/3
id: story:the-default-route-wakes-for-runs-without-a-model-call
kind: story
status: draft
title: The default route wakes a managed route for a run that makes no model call
summary: route_policy.default goes to every run with a bound program, so a wake_on_submit default wakes a paid model for runs that never call it
relations:
- informed_by: story:route-wake-behind-a-daily-cap
revision: 1
---
# The default route wakes a managed route for a run that makes no model call

## Context

Reported on 2026-11-27 by a consumer repository that pins release 0.13.0. A daemon whose `runbook.toml` names a
`[route_policy] default` gives that route to every run whose runbook has a bound program, whether or not a program of
the run calls a model. When the default is a managed route with `wake_on_submit = true` and its model is stopped, the
submit of such a run (a merge-on-green run that only runs a shell step) books and sends a wake and holds the run up to
the route's `start_wait_seconds`. A wake costs money; the run never calls the model it woke. The consumer leaves the
default unset and names its route in each model runbook's route entries instead.

Confirmed from the code (no run, no pod, no real route):

- `runbook/src/engine/mod.rs:923-926`: `runs_programs` is true for any bound program of the runbook (any entry of
  `budget::runbook_budgets(..).programs`, `runbook/src/budget.rs:413-417`), not for a program that calls a model.
- `runbook/src/commands.rs:596-606`: only a run with neither a model decider nor a program takes no route.
- `runbook/src/commands.rs:616-622`, `:629-640`: a program run that names no route takes `route_policy.default` when the
  file declares one and the relay serves runs.
- `runbook/src/commands.rs:1299-1320`: the submit asks `route_wait` for every run that took a route.
- `runbook/src/commands.rs:901-916`: `route_wait` lists for a wake each route whose model is `stopped`, whose kind is
  managed and whose `wake_on_submit` is on.
- `runbook/src/commands.rs:2396-2402`: the run's thread sends that wake (`wake_route`, `:2496`) before the run starts.
- The design says it: `docs/model-routes.md:265-272` ("the default route applies to it the same way").

## Acceptance

1. A run whose runbook has bound programs, none of which calls a model, and that names no route sends no wake and does
   not wait on a route, on a daemon whose default route is a managed route with `wake_on_submit` (a stand-in gateway
   on loopback, a fake route; no pod, no paid call).
2. A run of a runbook whose program calls a model through the route file still takes the default and wakes it as today.
3. How a runbook says that its program calls a model (an agent node, a route entry, a member of the binding) is decided
   in the story before it is built, and `docs/model-routes.md` section 2 says it.

## Evidence

- The file:line list under Context.
- A consumer repository, 2026-11-27: with `[route_policy] default` on its managed route, every run of a runbook with a
  bound program (merge-on-green, preflight) woke the pod and waited up to 900 s; it removed the default.

## Scope

`runbook/src/commands.rs`, `runbook/src/engine/mod.rs`, `runbook/src/budget.rs`, `runbook/tests/daemon/run_route.rs`,
`docs/model-routes.md`, `docs/daemon-api.md`.
