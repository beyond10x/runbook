---
format: aep.planning-md/3
id: story:program-reads-sibling-child-outcomes
kind: story
status: active
title: A bound program reads the outcome of a sibling child case of its run
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-11-25T17:14:12Z", actor: "agent:runbook", revision: 2}
- {from: "proposed", to: "active", at: "2026-11-25T17:14:13Z", actor: "agent:runbook", revision: 3}
---
## Outcome

A program step of a composed run can read the outcome (id and reason) of any other child case of its run, by child
id, through a documented environment name or a file in its working directory.

## Source

A consumer, 2026-11-25 (relayed by the host operator; accepted under the operator's delegation). A report stage of the
consumer's composed runbook cannot read the outcomes of the read, gate and check stages of the same run: a program step
gets `PATH` and `LOOM_CASE` only (`docs/loom-bindings.md` § program environment), so its digest says "unknown (not
complete)".

## Acceptance

- A fixture composer with two children: the second child's program prints the first child's outcome id and reason.
  Red today (it reads nothing), green after. No model.
- The name or file is documented in `docs/loom-bindings.md`, with what a program sees for a child that has not ended.

## Notes

- Loom part: the program's environment must carry the case id (and the run's tree) for Runbook to resolve siblings;
  scoping decides whether Runbook can do it alone from the tree it already reads.
