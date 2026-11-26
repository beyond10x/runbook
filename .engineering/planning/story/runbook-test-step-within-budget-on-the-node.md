---
format: aep.planning-md/3
id: story:runbook-test-step-within-budget-on-the-node
kind: story
status: draft
title: The runbook test step stays within its budget when it runs on the build node again
revision: 1
---
## Outcome

The `test:runbook` step ends within its 120 s budget wherever the gate runs it, and its time is measured again when Runbook jobs go back to the build node.

## Notes

- 2026-11-25, 23:0x CET on: Runbook jobs run on the workstation, not the build node (the node lacks an aep release binary for its glibc, upstream beyond10x/aep#175, and the ess 0.98.0 cache file).
- Measured on the workstation with 1776 tests: 181.9 s alone (5 test threads, load average about 10); 313.9 s inside `task check` at load average 38.
- Measured on the build node the same evening with 1749 to 1763 tests: 22 s to 36 s.
- Until the step runs on the node again, a budget miss on the workstation with every test passing does not block a merge (the operator, 2026-11-25).

## Acceptance

- On the build node, `task check` runs `test:runbook` in under 120 s, three runs in a row; the times are recorded here.
- If the step is over 120 s there too, the slowest ten tests are named and the step is brought under the budget by the repository's rule (an expensive product made once, not per test).
