<div align="center">

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="docs/img/runbook-logo-dark.svg">
  <img alt="Runbook" src="docs/img/runbook-logo.svg" width="320">
</picture>

Agentic runbooks: a fixed, checked way to handle a case.

[Docs](https://beyond10x.github.io/runbook/) ·
[How it works](https://beyond10x.github.io/runbook/docs/) ·
[Evaluation](https://beyond10x.github.io/runbook/evaluation) ·
[Releases](https://github.com/beyond10x/runbook/releases) ·
[Changelog](CHANGELOG.md)

</div>

## What it is

A runbook is a fixed, checked way to handle a case. A program decides the next step from recorded
facts; a model carries out single steps. The first runbook is the software delivery cycle (SDC):
it takes an idea to a built, tested product. Other runbooks are planned, not built: incident
handling, investigations and ticket triage.

Runbook is a prototype that runs on [Canon](https://github.com/beyond10x/canon) protocols.

## Progress

![Clean passes of ended runs per release and model setting, with the median cost and wall time of a clean pass](docs/img/progress.svg)

How often one idea runs with no person to a closed, delivered app, per release and model setting.
Each dot is one run. Every figure, its experiment and its limits are in
[docs/progress.md](docs/progress.md) and on the
[evaluation page](https://beyond10x.github.io/runbook/evaluation). The chart is written by
`sdc-experiment progress`; how an entry is added: [docs/improving.md](docs/improving.md).

## How it works

- **Eight stages.** An idea goes through idea, intake, direction, epic, spec, plan, story and
  release. Each stage is a Canon `protocol/1` case with child cases.
  [The flow](https://beyond10x.github.io/runbook/docs/flow)
- **No stored phase.** The stage is derived from facts: files in the case's repository and
  recorded check results.
  [The flow](https://beyond10x.github.io/runbook/docs/flow)
- **One driver.** `sdc-drive` runs one case tree. It reads Canon's decision for the case, runs the
  tool bound to one admissible action, records the result as Canon evidence and evaluates again.
  [The LLM is not the runtime](https://beyond10x.github.io/runbook/llm-is-not-the-runtime)
- **The model is an executor.** An LLM only carries out one action. Its output is an artifact
  revision, never evidence and never the next state.
  [Context](https://beyond10x.github.io/runbook/docs/context)
- **People decide what is theirs.** A decision that belongs to a person stops the run and prints
  the command that takes it. [docs/decisions.md](docs/decisions.md)
- **Built on layers.** Canon gives the semantics; Loom is meant to give the runtime.
  [What it is built on](https://beyond10x.github.io/runbook/docs/built-on)
  and [docs/layers.md](docs/layers.md)

Every protocol drawn from its YAML is in the
[protocol browser](https://beyond10x.github.io/runbook/protocols). Start in the repository with
[docs/phases.md](docs/phases.md), [docs/driver.md](docs/driver.md) and
[docs/orchestration.md](docs/orchestration.md).

## Install the plugin

The repository is also a Claude Code and Codex plugin, `runbook`, with skills for building,
recording and hardening a runbook. In Claude Code:

```console
/plugin marketplace add beyond10x/runbook
/plugin install runbook@runbook
```

The skills and what each needs: [plugin/README.md](plugin/README.md).

## Try it

Needs a `canon` binary of the pinned version (`driver/canon-pin.json`): the scripts take `CANON` when set, else the cached install of the pinned commit, and refuse any other `canon --version`,
Rust, `jq`, `yq`, and Node 22 for the live view in `ui/`.

The dry run uses a stub idea and stub executors that write the artifacts, prints every LLM command
and runs none, with scripted approvals:

```console
scripts/run-trial.sh --dry trial-dry
```

The scripted checks and a real run: [Run it](https://beyond10x.github.io/runbook/docs/run-it).

## Repository layout

| Path | Holds |
|---|---|
| `protocol/` | The eight SDC protocols and `sdc-bugfix` |
| `cases/` | Fixture cases with expected decisions, run by `scripts/evaluate-cases.sh` |
| `driver/` | The Rust crate `sdc-drive`, its tool bindings (`driver/examples/sdc/`) and other binaries |
| `runbook/` | `runbook`, the command people type: `doctor`, `list` and `--version` so far |
| `watch/` | `sdc-watch`: file changes of a run |
| `goals/` | `sdc-goals`: checks of `sdc.goals/1` documents ([docs/requester.md](docs/requester.md)) |
| `ui/` | A live web view of a run |
| `decision/`, `ess/`, `tools/` | The decision contract, its ESS specification and its shell tools |
| `eval/` | Eval runner, adversaries and personas |
| `experiments/` | Experiment records, run summaries and reports |
| `website/` | The documentation site; its protocol data is written by `sdc-protocol-site` |
| `docs/` | Design notes, results and release notes |
| `scripts/` | Check, evaluate and trial scripts |
| `plugin/`, `.claude-plugin/` | The `runbook` plugin (the skills `workflow-builder`, `harden` and `record`) and its marketplace |
| `.engineering/planning/` | The planning store (AEP) |

Recorded runs (`runs/`) are not in this repository. Agents: [AGENTS.md](AGENTS.md).

## Status

A prototype. No interface is stable. Releases are source releases at bare-version tags such as
`0.4.1`; nothing is published to crates.io. The latest is 0.4.1 (2026-11-22).

- Notes per release: [CHANGELOG.md](CHANGELOG.md), [docs/releases/](docs/releases/) and
  [docs/releasing.md](docs/releasing.md).
- New in 0.4.1 over 0.4.0 (nothing is unreleased): a fix (a model step of `runbook loom-run` that
  started no adapter costs nothing), the ESS specification and the product checks on ESS 0.97.3 with
  the daemon's synthesized suite committed, the reference check `sdc-check-unreferenced`, the two
  evaluation tools reading a loom-run directory, and UI test children that end with their parent; no
  protocol or behaviour change ([docs/releases/0.4.1.md](docs/releases/0.4.1.md)).
- New in 0.4.0 over 0.3.1: the daemon (`runbook serve`, its HTTP API, event
  streams, MCP and an embedded page, on a stub runtime that runs no real case) with `runbook run
  --detach`, `runbook runs` and `runbook decide` as its client, and `runbook loom-run`, which runs
  one case on a pinned `b10x-loom` binary
  ([docs/releases/0.4.0.md](docs/releases/0.4.0.md)).
- Three trials are written up: [Trial 1](docs/trial-1.md) (a real LLM; round 1 stopped, as
  designed, on three scope-changing questions for a person), [Trial 2](docs/trial-2.md) (a
  late-found ambiguity and the loop that repairs it; verified with stub executors, the real-LLM
  run is in its run notes) and [Trial 3](docs/trial-3.md) (autopilot runs of the to-do idea:
  five of the eight runs of the batch tree at `acd6f12` passed; the limits are in
  [docs/releases/0.1.0.md](docs/releases/0.1.0.md)).

## Licence

Apache-2.0, see [LICENSE](LICENSE).
