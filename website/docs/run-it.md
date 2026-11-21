---
title: Run it
description: The scripted checks and a dry run of the SDC with stub executors.
---

# Run it

## What it needs

A Canon checkout with a built `canon` binary (`CANON_DIR`, default `$HOME/beyond10x/canon`), Rust,
`jq`, `yq`, and Node 22 for the live view in `ui/`
([`README.md:48-49`](https://github.com/beyond10x/sdc/blob/0.1.1/README.md#L48-L49)).

## The scripted checks

From the repository root
([`README.md:53-59`](https://github.com/beyond10x/sdc/blob/0.1.1/README.md#L53-L59)):

```console
scripts/check-protocols.sh
scripts/check-bindings.sh
scripts/evaluate-cases.sh
(cd goals && cargo test)
(cd watch && cargo test)
```

`task check` builds, tests and lints the three crates and runs the bindings check, the fixture cases
and the planning store check; `task check:protocols` compiles and simulates the nine protocols and
takes about 12 minutes ([`Taskfile.yml`](https://github.com/beyond10x/sdc/blob/0.1.1/Taskfile.yml)).

## A dry run

A stub idea, stub executors that write the artifacts, every model command printed and none run,
approvals scripted
([`README.md:61-68`](https://github.com/beyond10x/sdc/blob/0.1.1/README.md#L61-L68)):

```console
scripts/run-trial.sh --dry trial-dry
```

The run writes its case store under `runs/trial-dry/`, which is not in git. Where each case stands
and why can be followed in the live view (`ui/README.md`).

## A real run

A real run calls models and costs money. Under autopilot nobody is asked anything:

```console
scripts/run-trial.sh <run> "<idea>" --autopilot
```

([`docs/autonomy-levels.md:449`](https://github.com/beyond10x/sdc/blob/0.1.1/docs/autonomy-levels.md#L449)).
The runs measured on the [evaluation](pathname:///sdc/evaluation) page are started from experiment
records with `sdc-experiment run <id>`
([`docs/improving.md:11-12`](https://github.com/beyond10x/sdc/blob/0.1.1/docs/improving.md#L11-L12)).

## This site

```console
task docs:generate   # write website/data/protocols/ from the protocols
task docs:build      # build the site into website/build
task docs:test       # the smoke test: every page, both themes, two widths, axe
```
