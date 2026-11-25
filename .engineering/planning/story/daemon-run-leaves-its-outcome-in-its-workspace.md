---
format: aep.planning-md/3
id: story:daemon-run-leaves-its-outcome-in-its-workspace
kind: story
status: active
title: A daemon run leaves its outcome readable in its workspace
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-11-25T17:14:14Z", actor: "agent:runbook", revision: 2}
- {from: "proposed", to: "active", at: "2026-11-25T17:14:14Z", actor: "agent:runbook", revision: 3}
---
## Outcome

When a daemon run ends, its workspace holds `outcome.json` with the same members a run of `runbook run` writes
(`outcome`, `reason`), so a tool pointed at a daemon workspace reads the outcome as it does for a file run.

## Source

A consumer, 2026-11-25 (relayed by the host operator; accepted under the operator's delegation). A daemon run writes no
`outcome.json`; the consumer's tools read the outcome of a daemon run as unknown. The daemon page's run tree
(`story:daemon-page-shows-a-run-tree-and-products`) is the page half, not this.

## Acceptance

- Submit a stub protocol to a test daemon, wait until the run is `Ended`: `<workspace>/outcome.json` exists and its
  `outcome` equals the `outcome` the daemon's run read answers. No model.
- A run that ends refused or stuck writes the file with that outcome and its reason.
- The file is written once, after the run's last record, and never for a run that has not ended.
