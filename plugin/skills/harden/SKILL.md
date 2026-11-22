---
name: harden
description: Check and harden a runbook protocol (a Canon `protocol/1` file with its bindings) in four steps with the commands of this repository's scripts and binaries - `canon check`, the route check `sdc-check-routes`, `canon simulate` compared with the pinned findings, then measured experiments with `sdc-experiment`. Use when a protocol or its bindings were written or changed, when a draft from the `record` skill needs its first checks, when a protocol check or simulate finding changed, or when a setting of a runbook that already passes must be compared by experiment. Works only inside a checkout of this repository (the SDC repository); not for writing a new protocol (the `record` skill), not for building a workflow to its first clean pass (the `workflow-builder` skill), and not for a checkout that lacks `scripts/check-protocols.sh`.
---

# harden

Four steps, cheapest first. Each step ends on a command that exits 0, or on a recorded reason it does not.

This skill works today inside a checkout of this repository. Run `test -x scripts/check-protocols.sh`
from the checkout root; when it fails, stop and tell the person that `harden` needs this repository.

Needs: `runbook` (built by the scripts with `cargo build --locked --bin runbook` in `runbook/`; it links Canon, and `runbook/Cargo.lock` is the pin), `cargo`, `jq`,
`yq`. Prefix every build or test with `slot` when `slot` is on `PATH`. A command that takes more than 30 seconds
runs in the background; wait on its output file.

## 1. Check the protocol with `canon check`

`scripts/check-protocols.sh` validates, compiles and checks every `protocol/*.protocol.yaml`, then
proves the ESS gate property with probe copies, then runs the simulate step of section 3. It takes
about 12 minutes: run it in the background and read its output file.

```console
scripts/check-protocols.sh > "$SCRATCH/check-protocols.txt" 2>&1 &
```

For one protocol, the command the script runs is (from the repository root, `<file>` the path of
the protocol):

```console
repo=$(pwd)
source scripts/evaluate-cases.sh && runbook_resolve && "$runbook" canon check --path "$repo/<file>"
```

`validate` and `compile` take the same `--path`. `scripts/evaluate-cases.sh` evaluates every case
under `cases/` against its `expected.json`.

Done when `canon check` prints no finding for the protocol. A finding is fixed in the protocol; a
protocol that Canon cannot check is not passed on to step 2.

## 2. Check the routes

`scripts/check-bindings.sh` checks each `driver/examples/sdc/<protocol>.bindings.json` against its
compiled protocol and then runs the route check. It reads `protocol/compiled/`, so run
`scripts/check-protocols.sh` or `scripts/evaluate-cases.sh` first, and build the driver once with
`cargo build --bins` in `driver/`.

The route check alone:

```console
cargo run --locked -q --manifest-path driver/Cargo.toml --bin sdc-check-routes -- --root . --doc docs/reject-routes.md
```

It prints one line per state where nothing points to a next step (`negative-result`,
`stale-retry`, `not-handed`, `own-failure`, `shared-cap`, `inert-cap`, `unrecorded-exit`,
`rework-limit`, `no-reject-route`). With `--doc` the lines must equal the block under "Check output"
in `docs/reject-routes.md`. Rules that live in the driver and not in the protocol (rework limit,
ends by rule, caps) are declared in `driver/examples/sdc/rule-ends.json`.

The script also runs `sdc-experiment check --all` and `sdc-protocol-site check`, so it can fail
on a stale experiment report or site data. Done when `scripts/check-bindings.sh` exits 0. A new finding is fixed in the protocol, the bindings
or `rule-ends.json`; the pinned block in `docs/reject-routes.md` changes only when a finding was
removed, in the same commit.

## 3. Simulate against the expected findings

`runbook canon simulate` walks the state space of a protocol. The simulate step of
`scripts/check-protocols.sh` runs `runbook canon simulate --path` on each `protocol/*.protocol.yaml` with
the Canon commit in `protocol/simulate/CANON_REV` (the one of `runbook/Cargo.lock`), and compares the output byte for byte with the
pinned report `protocol/simulate/<name>.txt`. Canon is the library `runbook` links; no Canon is installed.
A different finding set fails with a diff and
`simulate: <protocol>: findings changed (was N, now M)`.

1. Read the diff. Classify each new or removed finding with the verdicts of
   `docs/simulate-findings.md` (`real`, `left only by a decision`, `left by revision`,
   `left by a second record of the same step`, `cannot tell`) and the limits of the walk in
   `docs/protocol-notes.md`, section "Simulate".
2. A finding that is `real` is fixed in the protocol. Run step 1 again.
3. A deliberate change of the finding set is pinned: `scripts/check-protocols.sh --repin`, then
   `git diff protocol/simulate/`. The re-pinned reports are committed with the protocol, and the
   commit body names the reason.
4. `SDC_SIMULATE=off` skips the step when there is no network; a skipped step is reported as
   skipped, never as passed.

Done when the simulate step prints `findings N` per protocol equal to the pinned reports and every
new finding has a verdict.

## 4. Measure with experiments

Run this step only for a runbook that already passes its first runs and only to compare one
variable (a model, an effort, a cap, a prompt, a route). `sdc-experiment` is the only way a real
run is started and accounted for; a real run costs money, so start `run` only after the person who
owns the budget has said so.

Below, `sdc-experiment <args>` stands for
`cargo run --locked -q --manifest-path driver/Cargo.toml --bin sdc-experiment -- <args>`.

```console
sdc-experiment check <id>
sdc-experiment run <id> --print
sdc-experiment run <id>
sdc-experiment status <id>
sdc-experiment stop <id> --reason "<why>"
sdc-experiment report <id>
sdc-experiment check --all
```

1. Write `experiments/<id>.yaml` (`sdc.experiment/1`) before any run: purpose, one hypothesis on one
   node or edge with direction and size, one variable, the input with its sha256, full settings,
   `runs_per_variant` of at least 3, the primary metric and the guards, a pinned `tree_commit`.
   The fields: `docs/workflow-metrics.md`, section 7.
2. `check <id>` and `run <id> --print` first; `run <id>` in the background.
3. When every run has ended, `report <id>` writes the verdict (`held`, `contradicted`,
   `inconclusive`, or `descriptive` for a record without a hypothesis test) from the decision rule of the record. The rule is not changed after the runs.
4. `check --all` exits 0 before the record, its `experiments/<id>/` directory and
   `experiments/INDEX.md` are committed. The loop around an adopted change: `docs/improving.md`.

Done when `sdc-experiment check --all` exits 0 and the verdict of the record is written by `report`.
