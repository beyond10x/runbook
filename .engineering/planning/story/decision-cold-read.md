---
format: aep.planning-md/3
id: story:decision-cold-read
kind: story
status: proposed
title: Cold-read check refuses a request that cannot be restated
relations:
- decomposes: epic:decisions-a-person-can-take-cold
- depends_on: story:decision-request-from-question
- depends_on: story:stop-message-prints-request
scope:
- confidence: inferred
  path: driver/src/bin/sdc-decision-cold-read.rs
- confidence: inferred
  path: scripts/run-trial.sh
revision: 8
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T01:30:35Z", actor: "human:timo", revision: 3}
---
# Story: decision-cold-read

## Outcome

A request that a reader with no context cannot restate never reaches a person: a bounded model call
reads only the rendered request and must say what is being decided and what each option changes.

## Acceptance

`sdc-decision-cold-read <request.json>` exits 0 on the rendered trial-2 request and exits 1 on
a fixture request with its consequences stripped, printing which option's change could not be
restated; `scripts/run-trial.sh` stops with that refusal instead of the request when it exits 1; the
call (adapter, model, tokens, cost) is a line in `llm-calls.jsonl` marked `eval: cold-read`.

## Scope

A new `sdc-decision-cold-read` and its fixtures, and the call site in `scripts/run-trial.sh`
(after the request is printed by the stop-message story). Bound to `scripts/`.

## Placement and kind of check

An eval-style check, a judgement and not evidence: `docs/eval-design.md:22` ("Success comes from
programs only") means its verdict is never a Canon outcome or a success criterion; it gates only
whether a request is shown. The call is one bounded call through the adapter of `docs/eval-design.md`
§ 5.5 (the `llm` object's `model`, `max_turns`, `max_budget_usd`), at the pilot's default
`claude-haiku-5-5`. The model returns a restatement as JSON (what is decided, one change per option
letter); a program judges it (every option letter present, every change non-empty), as
`driver/examples/sdc/tools/decider.sh` validates its reply (`:47`). Runs at A5 with no person
(`docs/eval-design.md` § 7) use the same check. Where it lives (`eval/` beside the adversaries, or the
trial's tool set) is inferred: `docs/eval-design.md` has no section for an LLM-judged check, and the
story adds one.

## Tooling note

- New tool: the Rust binary `sdc-decision-cold-read` (`driver/src/bin/sdc-decision-cold-read.rs`), in place of `tools/decision-cold-read.sh` named before.
- Changes the existing bash tool `scripts/run-trial.sh`: allowed, a fix to prototype glue; no new bash tool.
(AGENTS.md, "Rules": anything new that runs is Rust; existing bash tools get fixes only.)

