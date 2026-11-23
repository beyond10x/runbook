---
format: aep.planning-md/3
id: story:run-without-model-node-needs-no-caps
kind: story
status: active
title: A runbook with no model node runs with zero caps and no ledger
summary: The runner decides at the opening whether the bindings hold a model node; without one it counts no call, reserves nothing and accepts caps of 0; with one nothing changes.
tags:
- backlog-2026-11-23
relations:
- informed_by: story:driver-runtime-removed
scope:
- confidence: cited
  path: CHANGELOG.md
- confidence: cited
  path: docs/cli.md
- confidence: cited
  path: docs/loom-run.md
- confidence: cited
  path: runbook/src/loom_run/bindings_copy.rs
- confidence: cited
  path: runbook/src/loom_run/here.rs
- confidence: cited
  path: runbook/src/loom_run/run_loop.rs
- confidence: cited
  path: runbook/src/loom_run/settings.rs
- confidence: cited
  path: runbook/tests
revision: 13
transitions:
- {from: "draft", to: "proposed", at: "2026-11-23T03:08:21Z", actor: "human:operator", revision: 2, executor: "agent:runbook", correlation: "operator-2026-11-23-consumer-gaps"}
- {from: "proposed", to: "active", at: "2026-11-23T03:10:53Z", actor: "human:operator", revision: 4, executor: "agent:runbook", correlation: "host-operator-2026-11-23-d644"}
---
## Outcome

A runbook whose bindings hold no model node runs with `--max-llm-calls 0` and
`--cap-micro-usd 0`: the runner reserves nothing, counts no call and needs no spend ledger of its
own. A runbook with a model node keeps its caps as they are.

## Source

- The consumer session of the operator's own runbooks, 2026-11-23: "`--max-llm-calls 0` stops
  before the first step; a run with no model step needs 1", and "Each runbook needs its own spend
  ledger (a shared one stopped the 1 micro USD runs)". Its runbooks `preflight` and
  `deploy-verify` run shell programs only; its command line states `--cap-micro-usd 1
  --max-llm-calls 1` to get past the checks.
- Read at `main` `566a0c3f`: the loop stops when `llm_calls >= max_llm_calls` before every
  `tree run` (`runbook/src/loom_run/run_loop.rs:311`, the same at `:522` for a handoff), and each
  `tree run` goes through the ledger "like every call that may start a model program" whether or
  not a binding of the runbook is a model node.

## Acceptance

- The runner decides once, at the opening, whether the run's bindings hold a model node (the rule
  it already uses to find one, `runbook/src/loom_run/bindings_copy.rs`) or a decider that calls a
  model; the answer is written to the run's records.
- Without one: `tree run` is not counted as a call, no amount is reserved, `--max-llm-calls 0` and
  `--cap-micro-usd 0` are accepted, `--spend-ledger` may be left out, and the run's records state
  0 calls and 0 spent. With `--max-llm-calls 0` the run does not end `StoppedLlmCalls`.
- With one: nothing changes. A cap of 0 stops the run before the first `tree run`, a missing
  ledger is refused, the cap fails closed as today (a test holds each).
- The decision cannot be changed by a bound program during the run: a bindings file edited in the
  run copy after the opening does not turn a run with a model node into one without (a test with
  a program that rewrites the copy; under `local` confinement the run then stops or keeps the
  caps of the opening, never fewer).
- A decider or a router that calls a model counts as a model node for this rule.
- A model call attempted in a run that was opened without a model node (caps 0, no ledger) is
  refused: every path by which the runner starts a model program (the program of a model node, a
  decider, a router) reads the decision of the opening and ends the run with a named cause before
  the program starts. A test proves it for each path. (Added on the decision of 2026-11-23 that
  activated this story.)
- A security review pass covers the rule and the line above; nothing of this story is released
  before that pass is green.
- Tests with sh programs; no model call. No test over 60 s.
- `docs/loom-run.md` and `docs/cli.md` say the rule; `CHANGELOG.md` has the line.

- A run without a model node and caps of 0 starts only under a confinement that takes the network from
  bound programs. Under `--confinement local` it is refused by name before the opening and needs its caps
  as before this story; a test holds the refusal and a test holds that the same run with caps starts.
  (Decision of 2026-11-23 after the first fix round: a committed binding that runs the tests of a
  workspace starts code the runbook author did not write, and the rule that finds a model node cannot
  read what that code starts.)
- A third security review pass covers that line; nothing of this story is released before it is green.

## Scope

`runbook/src/loom_run/run_loop.rs`, `runbook/src/loom_run/here.rs`,
`runbook/src/loom_run/bindings_copy.rs`, `runbook/src/loom_run/settings.rs`, `runbook/tests/`,
`docs/loom-run.md`, `docs/cli.md`, `CHANGELOG.md`.

## Not in this story

- A binding that marks itself a model node (`epic:runner-knows-no-runbook`).
- Defaults for the caps in `runbook.toml` (`story:runbook-toml-carries-defaults`).

## Open question for the operator

Decided for now (2026-11-23, by the session that decides in the operator's absence): the line above, fail
closed. Left for the operator:

- The other option: a run without a model node starts under `--confinement local` with caps of 0, and
  `docs/loom-run.md` says that a binding which runs the tests of a workspace runs code the author did not
  write, with the user's rights, caps or not. Its cost: such a run can spend without a cap.
- A reading of the coordinator, not measured: the caps count the calls the runner records. A program that a
  bound program starts by itself and that calls a model without the runner's adapter is not recorded and not
  counted, in a run with a model node too. So under `local` the caps never bounded such code; the refusal of
  caps of 0 under `local` keeps a recorded call counted and changes nothing for an unrecorded one. The second
  security pass of the unit is asked to show both runs side by side.
- The consumer keeps its workaround under `local` (`--cap-micro-usd 1 --max-llm-calls 1`) until this is
  answered.
