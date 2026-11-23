---
format: aep.planning-md/3
id: decision-blocker:runbook-declarations-without-a-format-member
kind: decision-blocker
status: open
title: Nobody has decided which file of a runbook states what neither its protocol nor its bindings can hold
summary: 'Start inputs, a program before the open and after a close, the default root case id and the programs a runbook needs have no member in protocol/1 or loom.bindings/1; default: one file of the runbook beside its bindings.'
tags:
- generic-runner
relations:
- blocks: story:run-inputs-declared-by-runbook
- blocks: story:conformance-and-root-case-from-runbook
- blocks: story:runner-source-names-no-runbook
revision: 1
---
## Question

In which file does a runbook state the four things that neither its protocol nor its bindings have a member for?

1. Which artifact takes the intake text, and which files a person may give at the start of a run.
2. A program the runner starts before the tree opens (today the seed of a bug fix) and one it starts after a close (today the conformance observation).
3. The default id of its root case.
4. The programs its own tools need and `runbook doctor` looks for (today the table `SPECS` of `runbook/src/doctor.rs:79-109`, whose rows say `sdc:` and `sdc-bugfix:`).

## Why nothing settles it

- `epic:runner-knows-no-runbook` says "A runbook is its protocol, its bindings and its instruction texts, and nothing else" and, in item 3, "the root case id and whether a protocol can be started come from the protocol".
- `protocol/1` of the pinned Canon (0.11.0, `e81f970`) is closed: unknown members are refused (`crates/canon/src/model/mod.rs:73-155`); the header is `id`, `revision`, `description`; `case.inputs` holds inputs of a case with closed values. It has no member for a default case id, for a file given at the start or for a program outside an action.
- `loom.bindings/1` of the pinned Loom (`db2726ff`) is closed: its top members are `format`, `protocol`, `program_root`, `artifacts`, `independence`, `actions`. A program exists only as the executor of an action.
- Read at `main` `826b09c7` of this repository: they are Rust today (`runbook/src/loom_run/here.rs:18` the case id `IDEA-1`; `runbook/src/loom_run/bugfix.rs:22-27` the runbook, the case id `BUG-1` and the helper of the seed; `runbook/src/loom_run/goals.rs`; `runbook/src/loom_run/conformance.rs`; `runbook/src/loom_run/run_loop.rs:296-299`).
- A runbook already has files that are none of the three kinds the epic names: `driver/examples/sdc/decision-classes.json`, `bindings/tools/answers.yaml`, `bindings/tools/node-classes.json`, `views/`.
- No `ess/1` document of this repository declares a runbook beyond its id and version (`ess/domains/daemon.yaml`, the types `sdc.daemon.RunbookId` and `sdc.daemon.RunbookVersion`).

## Options

- **A (default).** One file of the runbook beside its bindings, in a closed format of this repository that the runner reads: the default id of the root case, the artifact of the intake, the files a person may give (each by the name of an entry of the `artifacts` map of the bindings), a program before the open, a program after a close, the programs it needs. The sentence of the epic then names four kinds of file. The step can be taken back: the members can move into the formats of Canon and Loom later.
- **B.** Members in `protocol/1` (Canon) and `loom.bindings/1` (Loom). Two changes in two other repositories and a pin of each have to be on `main` before the two stories below can start.
- **C.** No new place. A file is given against the `artifacts` map of the bindings; the seed and the conformance observation become actions of the SDC protocols; `--case` has no default. The revisions of the protocols change and the results of their cycles with them, which the acceptance of the epic forbids without a record.

## What it stops

- `story:run-inputs-declared-by-runbook`, `story:conformance-and-root-case-from-runbook` and the part of `story:runner-source-names-no-runbook` that is `runbook doctor`: they are drafted so that their acceptance does not depend on the answer, and none can be built before it.
- Outside this epic, the same question stands behind one line of `story:runbooks-load-from-configured-paths` ("a protocol says itself whether it can be started"): `protocol/1` has no member for that either. A rule that needs no member is possible: a protocol can be started when no other protocol of the set opens it as a child case.
