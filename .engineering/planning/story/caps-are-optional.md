---
format: aep.planning-md/3
id: story:caps-are-optional
kind: story
status: active
title: Money caps are optional; a named cap stops the run
summary: No run is refused or limited because a spend cap, a call cap or a ledger is missing. A cap a user names stops the run when reached; 0 means spend nothing.
relations:
- supersedes: story:run-without-model-node-needs-no-caps
scope:
- confidence: cited
  path: AGENTS.md
- confidence: cited
  path: docs/cli.md
- confidence: cited
  path: docs/loom-run.md
- confidence: cited
  path: runbook/src/loom_run/budget.rs
- confidence: cited
  path: runbook/src/loom_run/here.rs
- confidence: cited
  path: runbook/src/loom_run/mod.rs
- confidence: cited
  path: runbook/src/loom_run/replay.rs
- confidence: cited
  path: runbook/src/loom_run/run_loop.rs
- confidence: cited
  path: runbook/src/main.rs
revision: 12
transitions:
- {from: "draft", to: "proposed", at: "2026-11-23T10:20:07Z", actor: "human:operator", revision: 2, executor: "agent:runbook"}
- {from: "proposed", to: "active", at: "2026-11-23T10:20:07Z", actor: "human:operator", revision: 3, executor: "agent:runbook"}
---
## Outcome

No run is refused, limited or stopped because a money cap is missing. A cap is an option a user may name:
`--cap-micro-usd`, `--max-llm-calls`, `--spend-ledger`, and `loom.cap_micro_usd` in an experiment record.
With none named there is no limit and no refusal. A cap a user names stops the run when it is reached, and
a cap of 0 means spend nothing.

## Source

The operator, 2026-11-23, in his own words: "runbook should not have any limitataions in terms of money cap
- there is basically no technical limitation at all". On the question whether a cap that a user names still
stops a run he picked: "A: named cap stops". Caps are optional everywhere; the cap code and the ledger stay.

This replaces these earlier rules of the repository, each of which refused or limited a run because of a
money cap:

1. `runbook run` refused a run with a model node that named no `--spend-ledger`, no `--cap-micro-usd` or no
   `--max-llm-calls` (exit 64). Now each of the three is optional; none named means no limit.
2. `story:run-without-model-node-needs-no-caps` refused a run without a model node and caps of 0 under
   `--confinement local` (exit 2). Now no rule ties a cap to a confinement level.
3. The same story's checks kept a model program from starting in a run opened without a model node, because
   such a run had no cap. Now a run without a named cap has no limit, so the checks have nothing to hold and
   are removed with rule 2.
4. `sdc-experiment` required the record member `loom.cap_micro_usd` and failed closed without it. Now the
   member is optional; a record that names it is held to it.
5. Replay started no live call without a readable cap value. Now no cap value means no limit; a named value
   stops as before.
6. `story:runbook-toml-carries-defaults` said the caps have no built-in default and fail closed. Now: no cap
   named, in an option or in the file, means no limit.

## Scope

- `runbook/src/loom_run/`: the options of a run (`here.rs`, `mod.rs`), the budget and the ledger
  (`budget.rs`, `run_loop.rs`), the model-node decision as far as it only served rules 2 and 3, replay
  (`replay.rs`).
- `runbook/src/main.rs` and the usage text: the three options are optional.
- `driver/`: the reader of an experiment record (`loom.cap_micro_usd` optional).
- `runbook/tests/`, `driver/tests/`: the cases of the removed refusals are removed or turned into cases of
  the new rule.
- `docs/loom-run.md`, `docs/cli.md`, `docs/experiments.md`, `AGENTS.md` (the money rules, in the operator's
  words), `CHANGELOG.md`.
- `.engineering/planning/story/runbook-toml-carries-defaults.md` is changed by the coordinator.

## Acceptance

- A run with a model node and none of `--spend-ledger`, `--cap-micro-usd`, `--max-llm-calls` starts and
  runs, under every confinement level, and its outcome states the calls made and what the steps recorded.
- A run with caps of 0 starts under `local` and under every other level; a model call it attempts is stopped
  by the cap of 0, with the outcome of a reached cap.
- A named `--cap-micro-usd N` or `--max-llm-calls N` stops the run when reached, as on `main` today; the
  committed spend cases with a named cap pass unchanged.
- An experiment record without `loom.cap_micro_usd` is accepted and runs without a limit; a record that
  names it is held to it.
- Replay without a cap value makes its live calls; with a cap value it stops at it.
- No text of the repository says a cap is required or fails closed when missing.
- A security review pass covers the change before it merges: no guard other than the missing-cap refusals
  moved (confinement, the login, what enters a run directory, the daemon's floor).

## Not in this story

- A named cap with nothing left starts nothing: `story:spend-cap-with-nothing-left-starts-nothing`, built on
  top of this one.
- `--max-wall-seconds`: it bounds time, and stays as it is.
- Caps of a run the daemon starts: `story:daemon-run-names-its-caps`.
