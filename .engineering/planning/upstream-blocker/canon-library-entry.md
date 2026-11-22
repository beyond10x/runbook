---
format: aep.planning-md/3
id: upstream-blocker:canon-library-entry
kind: upstream-blocker
status: open
title: 'Canon: compile, evaluate, check, validate and simulate as library functions'
tags:
- canon
- one-binary
relations:
- blocks: story:engine-on-loom-library-entry
revision: 1
---
Capability requested from the Canon session: the operations Runbook calls through the `canon`
binary today (compile, evaluate, check, validate, simulate) as public library functions of a Canon
crate, so that `runbook` links Canon as a cargo dependency pinned by commit and needs no `canon`
binary at run time (the operator, 2026-11-22: "runbook should not rely on binaries [of] canon, loom.
Instead, it should just embed it as a dependency directly.").

Which of them exist as library functions, and where, is the answer
`story:library-entries-from-loom-and-canon` collects in `docs/library-entries.md`; this blocker is
cleared when every row Runbook needs says `exists` at a named Canon commit. The request is sent to
the Canon session by that story; nothing is built here.
