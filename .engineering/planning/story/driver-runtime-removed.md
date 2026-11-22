---
format: aep.planning-md/3
id: story:driver-runtime-removed
kind: story
status: active
title: The driver runtime removed
summary: After parity, driver/src/main.rs, the driver scripts and the two tools that only they use are deleted and nothing references them.
tags:
- one-binary
relations:
- decomposes: epic:runbook-one-binary-and-daemon
- depends_on: story:parity-run-on-loom
- informed_by: story:loom-concurrent-children
- informed_by: upstream-blocker:loom-decider-per-class
scope:
- confidence: cited
  path: .github/workflows
- confidence: inferred
  path: .gitignore
- confidence: cited
  path: AGENTS.md
- confidence: cited
  path: CHANGELOG.md
- confidence: cited
  path: README.md
- confidence: cited
  path: Taskfile.yml
- confidence: cited
  path: bindings
- confidence: inferred
  path: cycles
- confidence: cited
  path: cycles/suite.yaml
- confidence: cited
  path: cycles/tree-suite.yaml
- confidence: cited
  path: docs
- confidence: cited
  path: driver/Cargo.toml
- confidence: cited
  path: driver/canon-pin.json
- confidence: cited
  path: driver/examples
- confidence: cited
  path: driver/src
- confidence: cited
  path: driver/src/bin/sdc-experiment.rs
- confidence: cited
  path: driver/src/main.rs
- confidence: cited
  path: driver/tests
- confidence: inferred
  path: driver/unreferenced-keep.txt
- confidence: cited
  path: ess
- confidence: inferred
  path: eval
- confidence: cited
  path: eval/README.md
- confidence: inferred
  path: instructions
- confidence: cited
  path: plugin
- confidence: inferred
  path: protocol
- confidence: cited
  path: protocol/check-ledger.json
- confidence: cited
  path: runbook
- confidence: cited
  path: scripts
- confidence: cited
  path: scripts/run-full-cycle.sh
- confidence: cited
  path: tools/autonomy-levels.json
- confidence: cited
  path: ui
- confidence: cited
  path: views
- confidence: inferred
  path: watch
- confidence: inferred
  path: website/data
- confidence: cited
  path: website/docs
revision: 28
transitions:
- {from: "draft", to: "proposed", at: "2026-11-22T01:47:37Z", actor: "human:operator", revision: 2, executor: "agent:plan-onebin", correlation: "operator-requirement-2026-11-22-one-binary"}
- {from: "proposed", to: "active", at: "2026-11-22T19:09:57Z", actor: "human:operator", revision: 28, executor: "agent:runbook"}
---
# Story: driver-runtime-removed

## Outcome

The driver's runtime is deleted from Runbook after parity on Loom is shown: its main file, the driver
scripts and what only they use are gone, and nothing in the repository refers to them.

## Source

The operator, 2026-11-22: "as soon as we know the loom stuff is basically working and there is
parity towards that dummy runtime we implemented, I would really like to remove that runtime itself
from runbook."

## Context

Starts only after `story:parity-run-on-loom` ends `parity: pass`. Three things in the repository
bind the deletion, found on 2026-11-22 and to be settled in this story before any file goes:

- `bindings/*.yaml` (the Loom bindings) still run `tools/llm-node.sh` as their agent node and name
  `tools/decider.sh`; both tools are deleted here, so the bindings need Loom's agent executor and
  decider configuration first (`bindings/PENDING.md`, "Agent nodes"; `upstream-blocker:loom-decider-per-class`;
  `docs/library-entries.md`). Until those exist this story cannot finish; it is not a reason to keep
  the files.
- `driver/src/bin/sdc-experiment.rs` starts runs through `scripts/run-trial.sh`; it starts them
  through `runbook run` after this story.
- The other binaries in `driver/` (`sdc-findings`, `sdc-reply`, `sdc-bugfix` and the rest) are
  bound programs and tools, not the runtime; each stays unless only the deleted files use it, which the
  reference check below decides.
`story:loom-concurrent-children` deletes the same runtime code (`driver/src/main.rs`) from the area
side and overlaps this story. `driver/canon-pin.json` and the Canon binary resolution of
`scripts/evaluate-cases.sh` go with the engine (decision 12).

## Acceptance

- These paths do not exist: `driver/src/main.rs`, `scripts/run-trial.sh`,
  `scripts/run-finding-cycles.sh`, `bindings/tools/llm-node.sh`, `bindings/tools/decider.sh`;
  `git ls-files <each>` prints nothing. The `[[bin]] sdc-drive` entry is gone from `driver/Cargo.toml`.
- `grep -rn 'sdc-drive\|run-trial\.sh\|run-finding-cycles\.sh\|llm-node\.sh\|decider\.sh' . --exclude-dir=.git
  --exclude-dir=target --exclude-dir=node_modules --exclude-dir=runs --exclude-dir=.engineering`
  prints nothing, except files named in the story's report as history (`CHANGELOG.md`,
  `docs/releases/`), each with its reason.
- A reference check (a program in Rust or an existing task target) lists every file that only the
  five deleted files used, and each of them is deleted or named in the report with the reason it
  stays; after the deletions it prints an empty list.
- `driver/canon-pin.json` does not exist and `grep -n canon_resolve scripts/evaluate-cases.sh`
  prints nothing; the pin is `runbook/Cargo.lock`.
- `sdc-experiment run <id>` starts its runs with `runbook run`; a record for one stub run ends with
  exit 0 (a test on a fixture record, no model call).
- `task check` exits 0 within 120 s; `runbook test cycles/tree-suite.yaml` prints every entry
  as `expected`, or as `pending` with the gap or the Loom story it waits for (the operator's decision
  of 2026-11-22: the deletion does not wait for the pending entries); the process tree of a run holds `runbook` and the bound programs of the bindings and
  no `sdc-drive`, `canon` or `b10x-loom`.
- `AGENTS.md`'s directory map and rules no longer name the deleted files, and
  `docs/driver.md` and `docs/layers.md` say the runtime is Loom's.

## Scope

`driver/src/main.rs`, `driver/Cargo.toml`, `driver/Cargo.lock`, `driver/canon-pin.json`,
`driver/src/bin/sdc-experiment.rs`, `scripts/` (`run-trial.sh`, `run-finding-cycles.sh`,
`evaluate-cases.sh`, `run-full-cycle.sh`, `watch-run.sh`), `bindings/` (`tools/llm-node.sh`,
`tools/decider.sh`, the bindings that name them, `PENDING.md`), `cycles/suite.yaml` (its
`script:` and `driver:` fields), `Taskfile.yml`, `AGENTS.md`, `README.md`, `docs/`,
`driver/tests/`, `ui/README.md`. A wide file set: it runs alone, after K.

Added for wave 2026-11-22-w1, read from the diffs of the built units (cited): `runbook`, `ui`,
`driver/examples`, `driver/src`, `plugin`, `website/docs`, `views`, `tools/autonomy-levels.json`,
`cycles/tree-suite.yaml`, `protocol/check-ledger.json`, `.github/workflows`, `ess`,
`eval/README.md`, `scripts/run-full-cycle.sh`, `CHANGELOG.md`. From the briefs of the three units
not built yet (inferred): `protocol`, `website/data`, `instructions`, `.gitignore`, `cycles`,
`eval`, `driver/unreferenced-keep.txt`, `watch`.
