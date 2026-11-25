---
format: aep.planning-md/3
id: story:a-named-rule-takes-a-person-action
kind: story
status: draft
title: A named rule answers a person action with a result, recorded as rule:<name>
summary: The holder rule has no rule behind it below A5 and rule:autopilot only closes; a runbook names a rule that takes a class and answers with a declared result.
tags:
- consumer-asks-2026-11-25
relations:
- depends_on: story:decider-answers-with-a-declared-result
- informed_by: story:class-of-a-person-action-from-the-case-artifact
- informed_by: story:decisions-routed-by-runbook-data
scope:
- confidence: cited
  path: docs/autonomy-levels.md
- confidence: cited
  path: docs/engine.md
- confidence: cited
  path: docs/loom-run-decisions.md
- confidence: cited
  path: runbook/src/engine/shape.rs
- confidence: cited
  path: runbook/src/loom_run/arbiter.rs
- confidence: cited
  path: runbook/src/loom_run/decider.rs
- confidence: cited
  path: runbook/src/loom_run/decider/routing.rs
- confidence: cited
  path: runbook/src/loom_run/decider/rules.rs
- confidence: inferred
  path: runbook/tests/daemon
- confidence: inferred
  path: runbook/tests/loom_run
revision: 11
---
## What is missing

A consumer wants a decision kind held by a rule answered by that rule, recorded as `rule:<name>`. Runbook has the holder `rule` in its table of who decides, and no rule that can take a person action.

Read on `main` at `81079535`:

- A class whose holder at the level is `rule` (below A5) is refused: "autonomy A3 gives the class merge_on_green (decision.answer) to rule, not to the decider" (`runbook/src/loom_run/decider.rs:293-303`). `docs/loom-run-decisions.md:54` says the same: "no rule is built".
- The one rule that answers is the close of the root case at A5, recorded as `rule:autopilot` (`runbook/src/loom_run/decider/routing.rs:29`, `closes_by_rule` at `:119-126`, answered at `runbook/src/loom_run/decider.rs:589`). It answers no result, and it is the only rule principal.
- The kinds `rule` of `runbook.toml` (`verdict` `approve` or `reject`) are deciders, recorded as `decider:<kind>`, not rules (`docs/engine.md:250-252`).
- The daemon keeps the prefix `rule:` for itself: a client label that starts with it is refused (`docs/engine.md:273-278`, `docs/daemon-api.md:33-42`), and an answer of a rule is recorded as `rule:<name>` in `answered_by` and `principal`.

## Outcome

1. A runbook names a rule by a name and states which classes it takes; the rule is a program of the runbook, deterministic, with no model and no network.
2. A request whose class has the holder `rule` at the run's level goes to the named rule. The rule answers with one of the results the action declares and its reasons, or says it cannot decide.
3. The answer is recorded in `answered_by` and in `principal` as `rule:<name>`, with cost 0, and the run resumes. A rule that cannot decide leaves the request open for a person.
4. `rule:autopilot` for the close at A5 and every holder other than `rule` behave as today, byte for byte.

## What a guard must hold

- A rule is asked only where the table gives the class to `rule`; no setting and no request widens that.
- A result a rule names is checked against the results the action declares before Loom is called (`runbook/src/engine/shape.rs:35`, `:72-97`); a result outside the list leaves the request open.
- The principal is built by Runbook from the rule's name in the runbook's document, never from what the rule prints; the label rule for clients stands.
- A rule that fails, times out or prints nothing readable leaves the request open: it never answers by default.
- The rule runs as a bound program does (a bounded process group, a clean environment); what it reads is the case's files, as `story:decision-options-come-from-a-case-artifact` states them.

## Open questions, each with the option recommended

- What a rule is. Recommended: a program of the runbook's bindings named in the classes document per class (`rule: <name>`), with the argv Loom's program contract already gives a bound program.
- Whether `rule_and_decider` and `rule_final` are in this story. Recommended: no; only `rule`. The others are separate stories once this one has run.

## Acceptance

With a fixture runbook, a classes document that gives one class the holder `rule` at A3 and names a rule program for it, the stub runtime and a rule decider:

- a request of that class is answered with a declared result and recorded under `rule:<name>`, the run resumes, and no decider is asked;
- a rule that prints a result the action does not declare, or exits non-zero, leaves the request open and the run's record holds the cause;
- a request of a class held by `person` stays open, and the close at A5 is still answered as `rule:autopilot`;
- a client answer labelled `rule:<name>` is still refused `400`.

## Scope

`runbook/src/loom_run/decider.rs`, `runbook/src/loom_run/decider/routing.rs`, `runbook/src/loom_run/decider/rules.rs`, `runbook/src/loom_run/arbiter.rs`, `runbook/src/engine/shape.rs`, `docs/loom-run-decisions.md`, `docs/engine.md`, `docs/autonomy-levels.md`, `runbook/tests/loom_run`, `runbook/tests/daemon`.

## Depends on

- `story:decider-answers-with-a-declared-result`: a rule answers with a declared result through the same shaping.
- Informed by `story:class-of-a-person-action-from-the-case-artifact` (a class a rule holds is often one of several of one action) and `story:decisions-routed-by-runbook-data` (the classes document is where the rule is named).
