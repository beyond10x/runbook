---
format: aep.planning-md/3
id: story:class-of-a-person-action-from-the-case-artifact
kind: story
status: draft
title: The decision class of a person action comes from a field of the case's artifact
summary: One person action has one class, so a request's kind cannot pick its holder; a classes-document member names the artifact field that holds the class.
tags:
- consumer-asks-2026-11-25
relations:
- depends_on: story:decision-options-come-from-a-case-artifact
- informed_by: story:decisions-routed-by-runbook-data
scope:
- confidence: cited
  path: docs/engine.md
- confidence: cited
  path: docs/loom-run-decisions.md
- confidence: cited
  path: driver/examples/sdc/decision-classes.json
- confidence: cited
  path: runbook/src/loom_run/decider.rs
- confidence: cited
  path: runbook/src/loom_run/decider/approval.rs
- confidence: cited
  path: runbook/src/loom_run/decider/routing.rs
- confidence: inferred
  path: runbook/tests/loom_run
revision: 8
---
## What is missing

A consumer built a decision protocol on Runbook 0.9.3: one program step routes a decision request by its kind, and one person action answers it. The consumer wants a kind that a rule may hold answered by that rule, and a kind reserved for a person left open with no default. Runbook cannot give the two kinds two holders, because the class of a decision comes from the action's name and never from the request.

Read on `main` at `81079535`:

- `class_of` takes the class of an approval as the first class of the classes document that lists `<protocol>/<action>`, else `approval`; a question, a finding and a close have a fixed class each (`runbook/src/loom_run/decider/routing.rs:41-54`). One person action therefore has one class.
- The holder of that class at the level decides who may answer: anything but `decider` ends in `CannotDecide` (`runbook/src/loom_run/decider.rs:293-303`). The consumer's refusal at A3 with a rule decider reads "reserved for a person: autonomy A3 has no holder for the class approval (decision.answer)", the line at `decider.rs:302`.
- The contract already has the member: a decision request carries its `decision_class` (`ess/domains/decision.yaml:237`), and the class is "one row each, named as the keys of sdc.decision-classes/1" (`ess/domains/decision.yaml:176-180`). The daemon's engine never fills it from the case: the class is derived at routing time from the action only.

## Outcome

1. The classes document of a runbook can say, for a person action, which field of the decision-request artifact of the case holds the class of that request, and which classes the field may name. The routing reads the field when the artifact is there and takes the class from it.
2. A request whose artifact names an admitted class is routed by that class's holder at the run's level; the same action with another request of another class may have another holder.
3. A request whose artifact is missing, whose field is empty, or whose class is not one the action admits stops for a person with a named cause: the request stays open, no default is taken.
4. A person action the document does not name a field for is classed as today, byte for byte.

## What a guard must hold

- The field can only select among the classes the runbook's own document lists for that action. A case artifact (written by a bound program, possibly a model) never names a class the action was not declared to take, so it cannot move a decision to a holder the runbook did not allow for that action.
- The class is read once, when the request opens, and the request keeps it: a later write to the artifact changes nothing for an open request (the run's routing is pinned for the same reason, `Decider::pin_routing`, `runbook/src/loom_run/decider/routing.rs:6-9`).
- Failing closed: anything the engine cannot read or check is a person's decision, never a decider's or a rule's.
- A reservation for a person has no default and no silence rule: the request waits.

## Open questions, each with the option recommended

- Where the field is named. Recommended: in the classes document, next to the action's entry, because that document is the data of the runbook that maps an action to a class (`story:decisions-routed-by-runbook-data`). The other option is the binding of the action.
- Which artifact file and how the engine finds it. Recommended: the file `story:decision-options-come-from-a-case-artifact` settles; this story reads one field of it.

## Acceptance

With a fixture runbook of one person action, a classes document that names the field and admits two classes, a level table that gives one class to `decider` and the other to `person` at A3, the stub runtime and a rule decider, and two cases whose artifacts name the two classes:

- the request of the first class is answered by the decider and recorded under its principal; the request of the second stays open, with no answer and no default, in the view `open-decisions`;
- an artifact that names a class outside the two, or has no value in the field, leaves its request open with the cause in the run's record;
- the same fixture with a classes document that names no field gives the routing it gave before this change (an existing test holds the cells).

## Scope

`runbook/src/loom_run/decider/routing.rs`, `runbook/src/loom_run/decider.rs`, `runbook/src/loom_run/decider/approval.rs`, `driver/examples/sdc/decision-classes.json`, `docs/loom-run-decisions.md`, `docs/engine.md`, `runbook/tests/loom_run`.

## Depends on

- `story:decision-options-come-from-a-case-artifact`: it settles the artifact file and how a binding finds it; this story reads one more field of it.
- Informed by `story:decisions-routed-by-runbook-data`, which moves the classes and the holders into one document of the runbook; the member this story adds goes into that document.
