---
format: aep.planning-md/3
id: story:library-entries-from-loom-and-canon
kind: story
status: active
title: Library entry points of Loom and Canon, known and requested
summary: Facts from the two neighbour repositories on what Runbook can call in process, and requests for what is missing.
tags:
- one-binary
relations:
- decomposes: epic:runbook-one-binary-and-daemon
scope:
- confidence: cited
  path: .engineering/planning/upstream-blocker
- confidence: inferred
  path: docs/library-entries.md
revision: 6
transitions:
- {from: "draft", to: "proposed", at: "2026-11-22T01:47:30Z", actor: "human:operator", revision: 2, executor: "agent:plan-onebin", correlation: "operator-requirement-2026-11-22-one-binary"}
- {from: "proposed", to: "active", at: "2026-11-22T01:47:31Z", actor: "human:operator", revision: 3, executor: "agent:plan-onebin", correlation: "operator-requirement-2026-11-22-one-binary"}
---
# Story: library-entries-from-loom-and-canon

## Outcome

Runbook knows, from the two neighbour repositories and not from guesses, which library entry points
of Loom and Canon the engine (`story:engine-on-loom-library-entry`) and the CLI can call in
process, and what is missing is requested from their sessions as upstream blockers.

## Source

The operator, 2026-11-22: "runbook should not rely on binaries [of] canon, loom. Instead, it should
just embed it as a dependency directly."

## Context

Loom and Canon are Cargo workspaces (`crates/*`). Starting points, read from `main` of each with
`git -C ../<repo> show <ref>:<file>` and not yet verified for this purpose (inferred): Loom
`crates/loom-bindings` (loader), `loom-cli` (the `tree run` command), `loom-executor`,
`loom-agent`, `loom-boundary`, `loom-runlog`, `loom-sdk`, `loom-conformance` (suite runner); Canon
`crates/canon` (kernel), `canon-cli`. Nothing is built or written in either repository; a missing
capability is requested from that repository's session (`AGENTS.md`, "Rules").
`upstream-blocker:loom-tree-run`, `loom-decider-per-class` and `loom-confinement` already cover
parts of the Loom side.

## Acceptance

- `test -f docs/library-entries.md` exits 0. For each capability below it has one row with the
  crate, the public function and its signature, the repository commit it was read at and the
  `path:line`, and the word `exists` or `requested`: start a tree run from a protocol set, a
  bindings set, an intake, a context directory and settings; an event sink that receives each
  record; answer a decision request; cancel a run; a confinement level per run and the list of
  levels; the agent executor and the decider configuration that replace `bindings/tools/llm-node.sh`
  and `bindings/tools/decider.sh`; the suite runner behind `runbook test`; from Canon: compile,
  evaluate, check, validate and simulate as library functions.
- Every row marked `requested` names an `upstream-blocker` id. The two blockers filed with this
  plan, `upstream-blocker:loom-library-entry` and `upstream-blocker:canon-library-entry`, each
  `blocks` `story:engine-on-loom-library-entry`; each body is updated to list the missing rows,
  and a blocker whose rows all say `exists` is moved to `cleared` with the file as its evidence.
  `aep plan artifact list --kind upstream-blocker` prints both with their status.
- The request to each session is sent, and `docs/library-entries.md` holds the answer or says
  "no answer yet" with the date.
- `docs/library-entries.md` names the Cargo dependency form for each crate (git dependency with
  `rev`) and the commit to pin, and says whether both repositories' crates build in one workspace
  with Runbook's rustc (`rust-toolchain.toml`), with the result of one `cargo check` of a scratch
  crate that depends on them.
- `aep plan artifact validate` exits 0.

## Scope

`docs/library-entries.md` (new), `.engineering/planning/upstream-blocker/`, `AGENTS.md` (one line).
No code in `runbook/`, no change in `../loom` or `../canon`.
