---
format: aep.planning-md/3
id: approval-record:operator-decision-parity-runs-unconfined-2026-11-22
kind: approval-record
status: approved
title: 'Operator decision: the three Loom evaluation runs run unconfined (2026-11-22)'
tags:
- operator-decision
relations:
- decides: story:parity-run-on-loom
revision: 2
transitions:
- {from: "draft", to: "approved", at: "2026-11-22T09:52:20Z", actor: "human:operator", revision: 2, executor: "agent:runbook-repin", correlation: "operator-pick-parity-unconfined-2026-11-22"}
---
# Operator decision: the three Loom evaluation runs run unconfined (2026-11-22)

- Decision, verbatim: "A: yes, unconfined". The operator's pick of 2026-11-22 at about 07:48 CET,
  relayed by the host operator session.
- Conditions, verbatim: "Cap stays 20 USD for the 3 runs. Start only once Loom names a commit with
  `every-printed-decision-command-runs` and `person-action-records-a-named-result`, as you wrote. The
  login file is never copied into a committed path, a log or a report. Back to me: the 3 results and
  spend."
- Meaning: the three evaluation runs of `story:parity-run-on-loom` run through Loom with
  `--confinement local`, on the model login named at start time. The earlier decision
  (`approval-record:operator-decision-parity-runs-spend-2026-11-22`, "A: 3 runs, cap 20 USD") stays as
  it was approved; this record adds the confinement and the conditions on the start.
- Not covered: more than three runs or 20 USD; a login file named in a committed path, a log or a
  report; a start before Loom names a commit that has both facts above.
- Standing: it holds for this story until the operator revokes it.
