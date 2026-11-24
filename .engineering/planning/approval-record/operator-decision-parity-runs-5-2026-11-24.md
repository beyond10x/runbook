---
format: aep.planning-md/3
id: approval-record:operator-decision-parity-runs-5-2026-11-24
kind: approval-record
status: approved
title: 'Operator decision: three Loom evaluation runs on main d828dddb from the 11.59 USD left'
revision: 2
transitions:
- {from: "draft", to: "approved", at: "2026-11-24T01:05:41Z", actor: "human:operator", revision: 2, executor: "agent:runbook", correlation: "operator-pick-d763-a-2026-11-24"}
---
# Operator decision: three Loom evaluation runs on main with the Loom pin d8944d49 from the 11.59 USD left (2026-11-24)

- Asked, on 2026-11-24: whether to start three more runs through `runbook run` on main at `d828dddb`
  (the Loom pin `d8944d49` with the rework allowance), with the money left of the 20 USD. Decision D763,
  option A: 3 runs within the 11.59 USD left, unconfined as under the earlier approvals. The pick was
  relayed by the operator's deputy at about 01:41 CET on 2026-11-24.
- Amounts: `parity-loom-strong-2` spent 0.37 USD (ledger 366257 micro-USD in 9 entries),
  `parity-loom-strong-3` 4.09 USD of model calls (4.093 USD), `parity-loom-strong-4` 3.95 USD (ledger
  3953217 micro-USD in 42 entries, nothing reserved). Of the 20 USD cap
  (`approval-record:operator-decision-parity-runs-spend-2026-11-22`), 11.59 USD are left.
- Meaning: three more runs, registered as `experiments/parity-loom-strong-5.yaml`, run on main at
  `d828dddb` (`runbook run`, in process on Loom) with `loom.confinement: local`, and spend at most the
  11.59 USD left. The hard stop line is 11.59 USD. The cap member of the record is 10.59 USD
  (`loom.cap_micro_usd` 10590000 micro-USD), one USD under the stop line, so that a call in flight
  cannot pass 11.59 USD; the spend ledger refuses a call that would pass the cap and the cap fails
  closed. The cap of 20 USD in total stays; this record adds no money. The conditions of
  `approval-record:operator-decision-parity-runs-unconfined-2026-11-22` hold: the login file is named
  at start time only and is never copied into a committed path, a log or a report.
- Start: the runs start only through `sdc-experiment run parity-loom-strong-5` with this registered
  record. No other start counts.
- Not covered: any run beyond these three; more than 11.59 USD, so no raise of the cap; another
  confinement than `local`; another setting than the record names; another tree than `d828dddb`.
- Standing: it holds for this story until the operator revokes it.
