---
format: aep.planning-md/3
id: story:inspector-shows-the-steps-of-a-run
kind: story
status: active
title: The run inspector shows the steps of a run of runbook run in the strip and in the sidebar
summary: A run of runbook run has no green strip and one box in the sidebar; the strip and the sidebar draw the steps of each case with their state from the run's records, for any runbook, and the bug-fix runbook gets a view file.
relations:
- informed_by: story:ui-view-data-per-runbook
- informed_by: story:runner-records-a-step-in-flight
- informed_by: story:start-page-lists-live-runs
scope:
- confidence: cited
  path: CHANGELOG.md
- confidence: cited
  path: ui/README.md
- confidence: cited
  path: ui/runfacts.mjs
- confidence: cited
  path: ui/server.mjs
- confidence: cited
  path: ui/test
- confidence: cited
  path: ui/web
- confidence: cited
  path: views/sdc-bugfix.view.json
revision: 10
transitions:
- {from: "draft", to: "proposed", at: "2026-11-23T17:26:27Z", actor: "human:operator", revision: 9, executor: "agent:runbook"}
- {from: "proposed", to: "active", at: "2026-11-23T17:26:28Z", actor: "human:operator", revision: 10, executor: "agent:runbook"}
---
## Defect

The operator, 2026-11-23, about a run of `runbook run` with the bug-fix runbook in the run inspector: the
page has no progress strip on top with the finished steps in green, and the left sidebar shows one box,
where a run of the SDC runbook shows a strip of phases and a tree.

Two causes, both in the inspector (`ui/README.md`, "Phase strip" and "The view file"):

1. The strip and the tree take a phase's state from Canon claims in `evaluations/*.json`. A run of
   `runbook run` writes none, so the strip says "claims not recorded for this run" and no phase turns
   green, with or without a view file.
2. The repository ships a view only for the SDC runbook. A run no view covers gets one stage "Protocols"
   with one phase per protocol, and a run of one case gets one box in the sidebar. The steps of that case
   (for the bug-fix runbook: `bug.dissect`, `dissection.check`, `test.reproduce`, `repro.run`,
   `change.fix`, `tests.run`, `story.update`, `gate.run`) are drawn nowhere outside the graph.

## Outcome

1. For a run of `runbook run`, the strip on top shows the steps of the case in the order of the protocol:
   a step that ended well is green, a step that ended with a failure is marked as failed, a step that
   runs now is marked as running, a step not reached is neutral. The state comes from the run's own
   records (the events of the case and, where the run has one, the runner's record of the step in
   flight), never from a guess and never from a stored phase.
2. This holds for any runbook with no view file: the steps are the actions of the run's protocol. A view
   file may group and label them (its `phases`), and then a phase is green when the steps it names
   ended well.
3. The left sidebar shows a case with its steps under it, each with the same state and its try count
   where a step ran more than once. A click on a step opens what the page already shows for it. A run
   with several cases keeps its case tree, and each case carries its steps.
4. The repository ships `views/sdc-bugfix.view.json`: labels for the steps of the bug-fix runbook and
   `fixed` as its good end. The SDC pages do not change.
5. A run whose records do not say what a step did says so in words ("not recorded for this run"); no
   step is drawn green without a record that it ended well.

## Acceptance

Playwright cases on runs written by hand with invented text, on a server with no view and on one with
the bug-fix view: a run with three finished steps, one running and the rest not reached draws that in the
strip and in the sidebar at 1600x1000 and at 390 px wide; a failed step is not green; a step with two
tries shows 2; a run of the SDC fixture draws the same strip and tree as before (byte-equal screenshots or
the existing cases unchanged); a run with no events of steps says "not recorded for this run".

## Guards

Every read of a run's file goes through the run-path check and the one checked open. No content of a run
becomes markup or code of the page. The inspector stays read-only: no route that acts on a run. The test
step of `ui/` stays inside its budget: no new server per test.

## Origin

The operator's word of 2026-11-23, relayed by the operator's host session. The third part of the same
word (a start page with every run that is live now) is `story:start-page-lists-live-runs`.
