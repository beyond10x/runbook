---
format: aep.planning-md/3
id: approval-record:operator-decision-parity-runs-spend-2026-11-22
kind: approval-record
status: approved
title: 'Operator decision: spend for the three Loom evaluation runs (2026-11-22)'
tags:
- operator-decision
relations:
- decides: story:parity-run-on-loom
revision: 2
transitions:
- {from: "draft", to: "approved", at: "2026-11-22T01:57:08Z", actor: "human:operator", revision: 2, executor: "agent:library-entries", correlation: "operator-pick-parity-spend-2026-11-22"}
---
# Operator decision: spend for the three Loom evaluation runs (2026-11-22)

- Decision, verbatim: "A: 3 runs, cap 20 USD". The operator's pick on 2026-11-22, relayed by the
  host operator session.
- Meaning: the three evaluation runs of `story:parity-run-on-loom` through Loom are approved. They
  start without asking again once `stub-cycle` is equal on both runtimes, and they stop at 20 USD of
  spend. The three results and the spend are reported back.
- Not covered: anything beyond three runs or 20 USD; another setting than the story's report names.
- Standing: it holds for this story until the operator revokes it.
