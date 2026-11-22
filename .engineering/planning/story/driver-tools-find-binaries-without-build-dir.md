---
format: aep.planning-md/3
id: story:driver-tools-find-binaries-without-build-dir
kind: story
status: active
title: The driver's tools find their binaries without a build directory in the tree
summary: In driver mode llm-node.sh, sdc-tool.sh and decider.sh read only the build directory; PATH counts in Loom mode alone
tags:
- loom
- parity
relations:
- decomposes: epic:runbook-one-binary-and-daemon
scope:
- confidence: cited
  path: bindings/tools/decider.sh
- confidence: cited
  path: bindings/tools/llm-node.sh
- confidence: cited
  path: bindings/tools/sdc-tool.sh
- confidence: cited
  path: scripts/check-bindings.sh
- confidence: cited
  path: scripts/run-full-cycle.sh
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-11-22T06:09:04Z", actor: "human:operator", revision: 2, executor: "agent:parity-equal", correlation: "operator-standing-acceptance-2026-11-22"}
- {from: "proposed", to: "active", at: "2026-11-22T06:09:04Z", actor: "human:operator", revision: 3, executor: "agent:parity-equal", correlation: "operator-standing-acceptance-2026-11-22"}
---
# Story: driver-tools-find-binaries-without-build-dir

## Outcome

The driver's tools find `sdc-reply`, `sdc-findings` and `sdc-bugfix` on `PATH` in driver mode too,
so a run of the old driver works from an export of a commit, which has no `driver/target/debug/`,
when the binaries are on `PATH`. Today only Loom mode reads `PATH`.

## Context

Found in the first equal comparison (`docs/parity.md`, "First equal comparison"). The driver run of
`stub-cycle` was made from an export of Runbook commit `ee90434`. The export has no
`driver/target/debug/`. The first attempt, with the prebuilt binaries on `PATH`, stopped at
`change.review`: `sdc-drive` printed "`change.review` could not run (exit 127 ...): llm-node:
sdc-reply not found (<export>/driver/target/debug/sdc-reply)". The run then needed that directory
linked to the prebuilt binaries.

Cause, read in the files: `bindings/tools/llm-node.sh` and `bindings/tools/sdc-tool.sh` define
`sibling()` (PATH first, then `driver/target/debug/<name>`) and use it in Loom mode only
(`llm-node.sh:54-55`, `:85`, `:238`; `sdc-tool.sh:74`, `:79`, `:89`). Without `LOOM_CASE` they take
`${SDC_REPLY:-$root/driver/target/debug/sdc-reply}` (`llm-node.sh:138`, `:159`, `:176`),
`${SDC_FINDINGS:-$root/driver/target/debug/sdc-findings}` (`llm-node.sh:241`, `sdc-tool.sh:82`) and
the same for `sdc-bugfix` (`sdc-tool.sh:88`); `decider.sh:63` and `llm-adapter-fixture.sh:90,115,145`
take the build directory by default as well. `scripts/run-full-cycle.sh:29` does the same for
`sdc-drive` (`DRIVE` overrides it).

## Acceptance

- In driver mode (no `LOOM_CASE`), `llm-node.sh`, `sdc-tool.sh` and `decider.sh` resolve `sdc-reply`,
  `sdc-findings` and `sdc-bugfix` as in Loom mode: the variable (`SDC_REPLY`, `SDC_FINDINGS`,
  `SDC_BUGFIX`) first, then `PATH`, then `driver/target/debug/<name>`; a case in
  `scripts/check-bindings.sh` runs a tool from a tree with no `driver/target/debug/` and the binary
  stub on `PATH` and it exits 0.
- A binary found nowhere still exits 127 with a message that lists the three places it looked at.
- `scripts/run-full-cycle.sh` finds `sdc-drive` on `PATH` when `DRIVE` is unset and the build
  directory has none, and its usage text says so.
