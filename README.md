# SDC

A prototype of a software delivery cycle that runs on [Canon](https://github.com/beyond10x/canon)
protocols. An idea goes through eight stages (idea, intake, direction, epic, spec, plan, story,
release), each a Canon `protocol/1` case with child cases. The stage is derived from facts (files
in the case's repository and recorded check results), never from a stored phase.

`sdc-drive` runs one case tree: it reads Canon's decision for the case, runs the tool bound to one
admissible action, records the result as Canon evidence and evaluates again. An LLM is only ever
the executor of one action; its output is an artifact revision, never evidence and never the next
state. Decisions that belong to a person stop the run and print the command that takes them.

## State

A prototype; no interface is stable. Release 0.1.0 (2026-11-21) is the first, a source release at
a bare-version tag: see [CHANGELOG.md](CHANGELOG.md), [docs/releases/0.1.0.md](docs/releases/0.1.0.md)
and [docs/releasing.md](docs/releasing.md).

- Trial 1 (`docs/trial-1.md`): a to-do list idea run with a real LLM; round 1 stopped, as
  designed, on three scope-changing questions for a person.
- Trial 2 (`docs/trial-2.md`): a late-found ambiguity and the loop that repairs it; verified with
  stub executors, the real-LLM run is recorded in its run notes.
- Trial 3 (`docs/trial-3.md`, `docs/releases/0.1.0.md`): five of eight autopilot runs (two more with the review model call)
  of the to-do idea closed delivered; the limits are in the release notes.
- Recorded runs (`runs/`) are not in this repository.

## Layout

- `protocol/`: the eight protocols of the cycle and `sdc-bugfix`; `cases/`: fixture cases with expected decisions.
- `driver/`: the Rust crate `sdc-drive` and its tool bindings (`driver/examples/sdc/`).
- `watch/`: file changes of a run; `ui/`: a live web view of a run; `goals/`: `sdc-goals`, checks of
  `sdc.goals/1` documents (`docs/requester.md`).
- `decision/`, `ess/`, `tools/`: the decision contract and its ESS specification.
- `eval/`: eval runner, adversaries and personas; `docs/`: design notes and results.
- `.engineering/planning/`: the planning store (AEP).

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
the SDC on Canon and Loom.

## Licence

Apache-2.0.
