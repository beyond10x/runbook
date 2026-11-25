---
format: aep.planning-md/3
id: story:decider-answers-with-a-declared-result
kind: story
status: draft
title: A decider answers a person action with one of the results its bindings declare
summary: A verdict is approve or reject, shaped to performed or rejected; an action that declares A to F and free is refused, so a decider can reply with a declared result.
tags:
- consumer-asks-2026-11-25
relations:
- informed_by: story:decision-options-come-from-a-case-artifact
- informed_by: story:decision-contract-goals-grounds
scope:
- confidence: cited
  path: docs/engine.md
- confidence: cited
  path: docs/loom-run-decisions.md
- confidence: cited
  path: docs/loom-run-settings.md
- confidence: cited
  path: runbook/src/engine/shape.rs
- confidence: cited
  path: runbook/src/loom_run/decide.rs
- confidence: cited
  path: runbook/src/loom_run/decider.rs
- confidence: inferred
  path: runbook/src/loom_run/decider/prompt.rs
- confidence: cited
  path: runbook/src/loom_run/decider/rules.rs
- confidence: inferred
  path: runbook/tests/loom_run
revision: 10
---
## What is missing

A consumer's person action `decision.answer` declares the results A, B, C, D, E, F and `free`. A decider can answer it with `approve` or `reject` only, and Runbook maps those to the results `performed` and `rejected`, so the run refuses the answer: "the decider answered performed for action decision.answer, but the bindings of the run declare the results A, B, C, D, E, F, free for it: Loom would refuse the result".

Read on `main` at `81079535`:

- A verdict is `Answer { approve: bool, principal, cost_micro_usd, model_calls, reasons }`; it has no member for a result (`runbook/src/loom_run/decide.rs:30-38`).
- The rule decider reads `approve` or `reject` from its reply and nothing else (`runbook/src/loom_run/decider.rs:431-451`); the kind `rule` has a `verdict` of `approve` or `reject` (`docs/loom-run-settings.md:146`).
- `person_result` names `performed` for an approve and `rejected` for a reject and refuses any other when the action declares results (`runbook/src/engine/shape.rs:72-97`); `declared_results` reads the declared keys from the bindings copy (`runbook/src/engine/shape.rs:35`). The refusal text is the line at `shape.rs:96`.
- The run keeps the decision open for a person when the verdict is one Loom would refuse (`docs/engine.md:270-271`), so nothing is lost, but a decider can never take such an action.

## Outcome

1. A decider's reply may name a result. A result the action declares is the answer; the principal is the decider's, as today.
2. A result the action does not declare is refused before Loom is called, with the refusal Runbook prints today, and the request stays open for a person.
3. A reply that names no result is shaped as today: `approve` is `performed`, `reject` is `rejected`, byte for byte.
4. The decision text (`approve` or `reject`) keeps its meaning for what it is told about (a capability approval has no result; its reject is a denial).

## What a guard must hold

- The list a result is checked against is the bindings copy's, read through its held directory as `declared_results` reads it; a decider (a model among them) cannot widen it.
- A result is never taken from text that is not the structured member of the reply, and never from the free text of the reasons.
- The free answer (`free`) takes text from the reply only where `story:decision-contract-goals-grounds` has settled how; until then a verdict that names `free` is refused by name and left open.

## Open questions, each with the option recommended

- Where a rule names its result. Recommended: the reply's optional member `result` beside `decision`; the `verdict` setting of the kind `rule` stays `approve` or `reject`.
- Whether the model decider's prompt lists the declared results. Recommended: yes, as the options it may name, so a model answers from the list and not from a guess.

## Acceptance

With a fixture runbook whose person action declares A, B and `free`, the stub runtime and a rule decider:

- a reply with `result: B` ends in an answer recorded with result B and the decider's principal, and the run resumes;
- a reply with `result: Z` leaves the request open and the run's record holds the refusal that names `Z` and the declared results;
- a reply with no result for an action that declares `performed` and `rejected` is answered as before (an existing test holds it);
- a reply with `result: free` is refused by name and left open.

## Scope

`runbook/src/loom_run/decide.rs`, `runbook/src/loom_run/decider.rs`, `runbook/src/loom_run/decider/rules.rs`, `runbook/src/loom_run/decider/prompt.rs`, `runbook/src/engine/shape.rs`, `docs/loom-run-decisions.md`, `docs/loom-run-settings.md`, `docs/engine.md`, `runbook/tests/loom_run`.

## Depends on

- Informed by `story:decision-options-come-from-a-case-artifact` (the letters of a person's answer, which that story maps to effects; this one is the decider's side of the same results) and `story:decision-contract-goals-grounds` (the free answer).
- Shares `engine::shape` with `story:decision-options-come-from-a-case-artifact` (the declared results are also what its open-decision row lists).
