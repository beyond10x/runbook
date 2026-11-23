---
format: aep.planning-md/3
id: approval-record:operator-decision-parity-runs-4-2026-11-22
kind: approval-record
status: approved
title: 'Operator decision: three Loom evaluation runs on the head of line 2 from the 15.54 USD left (2026-11-22)'
tags:
- operator-decision
relations:
- decides: story:parity-run-on-loom
revision: 2
transitions:
- {from: "draft", to: "approved", at: "2026-11-22T21:22:00Z", actor: "human:operator", revision: 2, executor: "agent:runbook", correlation: "operator-pick-parity-runs-4-2026-11-22"}
---
# Operator decision: three Loom evaluation runs on the head of line 2 from the 15.54 USD left (2026-11-22)

- Asked, on 2026-11-22: whether to start three more runs through `runbook run` on the head of line 2
  once its review fixes are merged, with two options. A: 3 runs within the 15.54 USD left of the
  20 USD. B: no runs now.
- Decision, verbatim: "A: 3 runs, max 15.54 USD". The operator's pick of 2026-11-22 at 21:48:57 CET
  (20:48:57Z), relayed by the host operator's session.
- Amounts: `parity-loom-strong-2`, the second record, spent 0.37 USD (ledger 366257 micro-USD in 9
  entries). `parity-loom-strong-3`, the third record, spent 4.09 USD of model calls (4.093 USD; its
  three runs stopped at `decider_cannot_decide in IDEA-1.intake` and at `error` in `change.review`).
  Of the 20 USD cap (`approval-record:operator-decision-parity-runs-spend-2026-11-22`), 15.54 USD are
  left.
- Meaning: three more runs, registered as `experiments/parity-loom-strong-4.yaml`, run on the head of
  line 2 (`runbook run`, in process on Loom) after its review fixes are merged, with
  `--confinement local`, and spend at most the 15.54 USD left. The cap fails closed: the stop line of
  the record is 14.54 USD (14540000 micro-USD), one USD under the 15.54 USD, and the spend ledger
  refuses a call that would pass it. The cap of 20 USD in total stays; this record adds no money.
  The conditions of `approval-record:operator-decision-parity-runs-unconfined-2026-11-22` hold: the
  login file is named at start time only and is never copied into a committed path, a log or a
  report.
- Start: the runs start only through `sdc-experiment run parity-loom-strong-4` with this registered
  record. No other start counts.
- Not covered: any run beyond these three; more than 15.54 USD, so no raise of the cap; another
  confinement than `local`; another setting than the record names; a start before the review fixes
  of line 2 are merged.
- Standing: it holds for this story until the operator revokes it.
