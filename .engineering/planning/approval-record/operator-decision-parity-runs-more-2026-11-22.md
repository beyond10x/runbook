---
format: aep.planning-md/3
id: approval-record:operator-decision-parity-runs-more-2026-11-22
kind: approval-record
status: approved
title: 'Operator decision: three more Loom evaluation runs from the 19.63 USD left (2026-11-22)'
tags:
- operator-decision
relations:
- decides: story:parity-run-on-loom
revision: 2
transitions:
- {from: "draft", to: "approved", at: "2026-11-22T12:17:46Z", actor: "human:operator", revision: 2, executor: "agent:runbook-record-parity3", correlation: "operator-pick-parity-more-2026-11-22"}
---
# Operator decision: three more Loom evaluation runs from the 19.63 USD left (2026-11-22)

- Asked, on 2026-11-22: "after the scope.answer fix is merged, start 3 more runs through Loom?"
- Decision, verbatim: "A: 3 runs from the 19.63 left". The operator's pick of 2026-11-22 at 13:16 CET,
  relayed by the host's operator session.
- Amounts: `parity-loom-strong-2`, the second record, spent 0.37 USD (ledger 366257 micro-USD in 9
  entries; its three runs stopped at `decider_cannot_decide in IDEA-1.intake`). Of the 20 USD cap
  (`approval-record:operator-decision-parity-runs-spend-2026-11-22`), 19.63 USD are left.
- Meaning: three more runs through Loom, registered as `experiments/parity-loom-strong-3.yaml`,
  spend at most the 19.63 USD left. The cap of 20 USD in total stays; this record adds no money. The
  conditions of `approval-record:operator-decision-parity-runs-unconfined-2026-11-22` hold.
- Start: the runs start only through `sdc-experiment run parity-loom-strong-3` with this registered
  record. No other start counts.
- Not covered: more than three runs; more than 19.63 USD; another setting than the record names.
- Standing: it holds for this story until the operator revokes it.
