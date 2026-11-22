---
format: aep.planning-md/3
id: upstream-blocker:loom-library-entry
kind: upstream-blocker
status: open
title: 'Loom: library entry points for an in-process runtime (tree run, event sink, answer, cancel, confinement, agent node, suite runner)'
tags:
- loom
- one-binary
relations:
- blocks: story:engine-on-loom-library-entry
revision: 1
---
Capability requested from the Loom session: library entry points that Runbook can call in process,
so that `runbook` links Loom as a cargo dependency and starts no `loom` or `b10x-loom` process
(the operator, 2026-11-22: "runbook should not rely on binaries [of] canon, loom. Instead, it should
just embed it as a dependency directly.").

Wanted as public functions of a Loom crate, each with an event sink and no process exit:

- start a tree run from a protocol set, a bindings set, an intake, a context directory and settings;
- a sink that receives every record the run writes;
- answer a decision request; cancel a run;
- a confinement level per run and the list of levels (see `upstream-blocker:loom-confinement`);
- the agent executor and the decider configuration that replace Runbook's `bindings/tools/llm-node.sh`
  and `bindings/tools/decider.sh` (see `upstream-blocker:loom-decider-per-class`);
- the suite runner behind `runbook test`.

Which of these exist, and where, is the answer `story:library-entries-from-loom-and-canon` collects in
`docs/library-entries.md`; this blocker is cleared when every row Runbook needs says `exists` at a
named Loom commit. The request is sent to the Loom session by that story; nothing is built here.
