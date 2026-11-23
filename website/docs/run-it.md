---
title: Run it
description: The scripted checks and a dry run of Runbook with stub executors.
---

# Run it

## What it needs

Canon is a library of the `runbook` crate (the pin is `runbook/Cargo.lock`); no `canon` binary is
installed. Rust, `jq`, `yq`, and Node 22 for the live view in `ui/`. Build `runbook` and the
programs it runs once, in the repository root
([`README.md`](https://github.com/beyond10x/runbook/blob/main/README.md)):

```console
(cd driver && cargo build --locked --bins)
(cd runbook && cargo build --locked --bin runbook)
```

The binary is `runbook/target/debug/runbook`; the commands below write it as `runbook`.

## The scripted checks

From the repository root
([`README.md:53-59`](https://github.com/beyond10x/runbook/blob/0.1.1/README.md#L53-L59)):

```console
scripts/check-protocols.sh
scripts/check-bindings.sh
scripts/evaluate-cases.sh
(cd goals && cargo test)
(cd watch && cargo test)
```

`task check` builds, tests and lints the three crates and runs the bindings check, the fixture cases
and the planning store check; `task check:protocols` compiles and simulates the nine protocols and
takes about 12 minutes ([`Taskfile.yml`](https://github.com/beyond10x/runbook/blob/0.1.1/Taskfile.yml)).

## A dry run

The idea text you give with `--intake`, stub executors that write the artifacts, every model
command printed and none run, approvals scripted
([`docs/loom-run.md`](https://github.com/beyond10x/runbook/blob/main/docs/loom-run.md), "A dry run"):

```console
runbook run sdc --dry --intake <idea file> --run trial-dry --runs-dir runs --settings <settings.json> \
  --confinement local --spend-ledger <ledger file> --cap-micro-usd <n> --max-llm-calls <n> --max-wall-seconds <n>
```

`<settings.json>` is a JSON object in the format `runbook.loom-run-settings/1`
([`docs/loom-run-settings.md`](https://github.com/beyond10x/runbook/blob/main/docs/loom-run-settings.md)).
`--dry` keeps every other option required; a dry call costs nothing and reads no login file
([`docs/loom-run.md`](https://github.com/beyond10x/runbook/blob/main/docs/loom-run.md)). The run
writes its run directory, `runs/trial-dry/`, which is not in git
([`docs/loom-run-directory.md`](https://github.com/beyond10x/runbook/blob/main/docs/loom-run-directory.md)).
Where each case stands and why can be followed in the live view (`ui/README.md`).

## A real run

A real run calls models and costs money. With autonomy level A5 in its settings, nobody is asked
anything; the spend ledger and the three caps bound what it can spend:

```console
runbook run sdc --intake <idea file> --run <name> --runs-dir runs --settings <settings.json> \
  --confinement <local|substrate> --spend-ledger <ledger file> --cap-micro-usd <n> --max-llm-calls <n> --max-wall-seconds <n>
```

([`docs/loom-run.md`](https://github.com/beyond10x/runbook/blob/main/docs/loom-run.md)).
The runs measured on the [evaluation](pathname:///runbook/evaluation) page are started from experiment
records with `sdc-experiment run <id>`
([`docs/improving.md:11-12`](https://github.com/beyond10x/runbook/blob/0.1.1/docs/improving.md#L11-L12)).

## This site

```console
task docs:generate   # write website/data/protocols/ from the protocols
task docs:build      # build the site into website/build, and the fixture build
task docs:test       # the smoke test: every page, both themes, two widths, axe, links
task docs            # all of it, after checking the protocol data is current
```

The smoke test serves the two builds itself on 127.0.0.1:7591 and 7592, ports no other suite of
the repository uses, and stops them when the run ends.

Every protocol page, the flow and the index are generated from the protocol YAML and its bindings
by `sdc-protocol-site` (`driver/src/bin/sdc-protocol-site.rs`); `scripts/check-bindings.sh` fails
when the committed data is stale. The evaluation and the timeline read only committed files:
`docs/progress.json`, `experiments/` and their timelines.
