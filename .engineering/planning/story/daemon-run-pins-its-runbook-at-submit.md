---
format: aep.planning-md/3
id: story:daemon-run-pins-its-runbook-at-submit
kind: story
status: active
title: A run started through the daemon reads the runbook it was submitted with until it ends
summary: The daemon opens a run on the bindings path inside the runbook home, so a revision of the runbook during a run ends it with a protocol mismatch; the run copies its runbook at submit through the copy code of runbook run.
relations:
- informed_by: story:runbooks-load-from-configured-paths
scope:
- confidence: cited
  path: docs/engine.md
- confidence: cited
  path: runbook/src/engine/layout.rs
- confidence: cited
  path: runbook/src/engine/mod.rs
- confidence: cited
  path: runbook/src/loom_run/bindings_copy.rs
- confidence: cited
  path: runbook/tests/daemon
revision: 8
transitions:
- {from: "draft", to: "proposed", at: "2026-11-23T14:25:01Z", actor: "human:operator", revision: 2, executor: "agent:runbook"}
- {from: "proposed", to: "active", at: "2026-11-23T14:25:01Z", actor: "human:operator", revision: 3, executor: "agent:runbook"}
---
## Defect

A consumer's report of 2026-11-23: a run started through the daemon had finished three steps over 22
minutes when the daemon ended it with `runbook.runtime-failed`, reason `BindingsRefused …
ProtocolMismatch`: the bindings file named revision 4 of the protocol and the run had been opened for
revision 3. The runbook's protocol and bindings had been revised in the runbook home while the run was in
flight.

Read on the default branch at `914dffc2`: the daemon opens the tree with the path of the bindings file
inside the runbook home (`runbook/src/engine/mod.rs:169-171`, `runbook/src/engine/layout.rs:32`). The
protocols are kept in the run's store when the tree is opened; the bindings are not, so they are read
again from the runbook home later in the run. `runbook run` copies the bindings into the run directory
before it opens the tree (`runbook/src/loom_run/bindings_copy.rs`).

## Outcome

1. A run started through the daemon takes its runbook at submit: the protocols, the bindings of the root
   and of every child the root names, and every file of the runbook home the bindings name (tools,
   instructions, views). They are copied into the run's own directory, and the run reads only that copy
   until it ends.
2. The copy is made by the code `runbook run` uses, with its checks. There is one implementation.
3. An edit, a removal or a replacement of any file of the runbook home after the submit changes nothing
   for a run in flight. A run submitted after the edit takes the new files.
4. The run's record names the revision of the protocol and a digest of the copied bindings.

## What a guard must hold

- The copy is checked against a rewrite during the run as `runbook run` checks it: a changed copy ends the
  run with a named refusal and is never executed in the changed form.
- The copy takes regular files inside the runbook home only: a link that leaves the home is refused at
  submit, before the run's directory holds anything.
- A submit names a runbook by name and version, never a path; the copy's source is the daemon's own list.
- Nothing of the copy is served by a product route that did not serve it before (a tool can hold what a
  consumer does not publish): the product listing stays restricted to the work directory.

## Acceptance

With a fixture runbook and stand-in programs: a run that waits at a decision while the fixture's bindings
and protocol in the runbook home are revised, then replaced by a file that does not parse, then removed,
runs to its end on the revision it was submitted with; a second submit takes the new revision; the record
of each run names its revision and digest; a link out of the home is refused at submit. No test over 60 s.
A security review pass covers the change before it merges.

## Origin

A consumer's defect report of 2026-11-23.
