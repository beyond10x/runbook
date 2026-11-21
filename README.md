# Runbook

Runbook: agentic runbooks. A runbook is a fixed, checked way to handle a case. A program decides
the next step from recorded facts; a model carries out single steps. Runbook is a prototype that
runs on [Canon](https://github.com/beyond10x/canon) protocols.

The first runbook is the software delivery cycle (SDC): it takes an idea to a built, tested
product. An idea goes through eight stages (idea, intake, direction, epic, spec, plan, story,
release), each a Canon `protocol/1` case with child cases. The stage is derived from facts (files
in the case's repository and recorded check results), never from a stored phase. Other runbooks
are planned, not built: incident handling, investigations and ticket triage.

`sdc-drive` runs one case tree: it reads Canon's decision for the case, runs the tool bound to one
admissible action, records the result as Canon evidence and evaluates again. An LLM is only ever
the executor of one action; its output is an artifact revision, never evidence and never the next
state. Decisions that belong to a person stop the run and print the command that takes them.

The [documentation site](https://beyond10x.github.io/runbook/) shows the flow, every protocol drawn
from its YAML, what Runbook is built on, the evaluation per release and one run step by step.

## State

A prototype; no interface is stable. The latest release is 0.1.4 (2026-11-21), after 0.1.3, 0.1.2,
0.1.1 and the first, 0.1.0; all are source releases at bare-version tags: see [CHANGELOG.md](CHANGELOG.md),
[docs/releases/0.1.4.md](docs/releases/0.1.4.md), [docs/releases/0.1.3.md](docs/releases/0.1.3.md),
[docs/releases/0.1.2.md](docs/releases/0.1.2.md),
[docs/releases/0.1.1.md](docs/releases/0.1.1.md),
[docs/releases/0.1.0.md](docs/releases/0.1.0.md) and [docs/releasing.md](docs/releasing.md).

- Trial 1 (`docs/trial-1.md`): a to-do list idea run with a real LLM; round 1 stopped, as
  designed, on three scope-changing questions for a person.
- Trial 2 (`docs/trial-2.md`): a late-found ambiguity and the loop that repairs it; verified with
  stub executors, the real-LLM run is recorded in its run notes.
- Trial 3 (`docs/trial-3.md`, `docs/releases/0.1.0.md`): five of eight autopilot runs (two more with the review model call)
  of the to-do idea closed delivered; the limits are in the release notes.
- Recorded runs (`runs/`) are not in this repository.

## Layout

- `protocol/`: the eight protocols of the SDC and `sdc-bugfix`; `cases/`: fixture cases with expected decisions.
- `driver/`: the Rust crate `sdc-drive` and its tool bindings (`driver/examples/sdc/`).
- `watch/`: file changes of a run; `ui/`: a live web view of a run; `goals/`: `sdc-goals`, checks of
  `sdc.goals/1` documents (`docs/requester.md`).
- `decision/`, `ess/`, `tools/`: the decision contract and its ESS specification.
- `eval/`: eval runner, adversaries and personas; `docs/`: design notes and results.
- `website/`: the documentation site; its protocol data is written by `sdc-protocol-site`.
- `.engineering/planning/`: the planning store (AEP).

## Progress

How often one idea runs with no person to a closed, delivered app, per release and model setting.
Each dot is one run; the figures come from the experiment records and are written by
`sdc-experiment progress`. Every figure, its experiment and the limits of each measurement:
[docs/progress.md](docs/progress.md). How an entry is added: [docs/improving.md](docs/improving.md).

![Clean passes of ended runs per release and model setting, with the median cost and wall time of a clean pass](docs/img/progress.svg)

## Requirements

A Canon checkout with a built `canon` binary (`CANON_DIR`, default `$HOME/beyond10x/canon`),
Rust, `jq`, `yq` and Node 22 for `ui/`.

## Checks

```console
scripts/check-protocols.sh
scripts/check-bindings.sh
scripts/evaluate-cases.sh
(cd goals && cargo test)
(cd watch && cargo test)
```

## Dry run

Stub idea, stub executors write the artifacts, every LLM command is printed and none is run,
approvals are scripted:

```console
scripts/run-trial.sh --dry trial-dry
```

Start with `docs/phases.md`, `docs/driver.md` and `docs/orchestration.md`; `docs/layers.md` places
Runbook on Canon and Loom.

## Licence

Apache-2.0.
