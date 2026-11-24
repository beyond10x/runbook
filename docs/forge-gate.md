# What the forge runs of the local gate, and what it does not

`task check` runs four steps side by side: `test:driver`, `test:runbook`, `test:ui` and `check:steps`. The forge
(`.github/workflows/pages.yml`, job "Build documentation") runs the test steps that end within the 120 s step budget
at its CPU count, as the same task the local gate runs, for a change the classifier calls `code`. `task check:ci-paths`
holds the list: each `test:` dependency of `task check` has a step `run: task <name>` in the workflow or an `OUTSIDE`
line in that check with its reason, never both and never neither.

## The forge's runner, and how the local gate meets it

The runner (`ubuntu-latest`) has 4 CPUs. `CI` is set there, and each test task reads it: `test:runbook` and
`test:driver` run the nextest profile `forge` (4 test threads; `runbook/.config/nextest.toml`,
`driver/.config/nextest.toml`), `test:ui` starts Playwright without the `slot` prefix. The forge installs go-task,
cargo-nextest, the pinned ess (`sdc-conform ess-install`) and the pinned aep (`sdc-conform aep-install`,
`driver/aep-pin.json`) in steps of their own before the test steps; it has no `ess` and no `aep` on `PATH`, so both
cargo test steps put the traps `scripts/host-tool-traps/ess` and `scripts/host-tool-traps/aep` first on the `PATH` of
their tests here too.

The driver tests need a real aep: the programs they start (`sdc-bugfix`, `sdc-node`, `sdc-tool.sh`) keep their
planning store with it, and the expected outputs hold the files and messages aep writes. A stand-in would have to
write the same bytes. So `test:driver` asks `sdc-conform aep-path` for the pinned binary (its digest and `--version`
checked) and puts its directory ahead of the traps, on the forge and here alike; without it the step stops with the
install command, `cargo run --locked --manifest-path driver/Cargo.toml --bin sdc-conform -- aep-install`. On
2026-11-24 the forge's first run of the step failed 34 tests with `aep is not on PATH`; the local forge way showed the
same 34 with no aep reachable, before the change.

The local forge way runs a step's forge profile on 2 CPUs of the host it runs on (`taskset`), which came closest to
the forge's time for `runbook/` (the forge 123 s, 2 CPUs of the build node 127 s, all 64 of them 91 s, at 4 threads):

| Step | Local forge way |
|---|---|
| `test:runbook` | `RUNBOOK_TEST_FORGE=1 slot task test:runbook` |
| `test:driver` | `DRIVER_TEST_FORGE=1 slot task test:driver` |
| `test:ui` | `UI_TEST_FORGE=1 slot task test:ui` |

Each prints the load average before and after its run: other work on those 2 CPUs slows it.

## Measured on the build node, 2026-11-24 (64 CPUs, 4 threads or workers)

| Step | 2 CPUs (`taskset -c 0-1`) | 4 CPUs | 64 CPUs | On the forge |
|---|---|---|---|---|
| `test:driver` | `DRIVER_TEST_FORGE=1`: 846 passed, 3 skipped in 86.5 s, 83.1 s, 78.2 s | | | yes |
| `test:ui` | `UI_TEST_FORGE=1`: 406, 412, 417 of 609 passed when the 120 s run bound ended the run | 508 of 609 at 120 s | 609 passed in 94.1 s | no |

The load average of the node was 31 to 94 at the start of those runs. Before the change the same runs of the base
(the commands by hand) gave 844 passed in 79.8 s, 82.4 s and 73.1 s for the driver and 412, 413 and 413 of 609 for
the UI. With the trap `ess` on their `PATH`, three driver tests of `driver/tests/decision_request.rs` failed: they
started the `ess` on `PATH`, which the forge does not have; they start the pinned ess now.

## Why `test:ui` stays out

At 2 CPUs its 609 tests would need about 177 s, at 4 CPUs about 144 s (the share that finished in 120 s, taken to the
whole run); the step budget is 120 s, and even on all 64 CPUs of the node the run takes 94 s of it. The rules of
"Tests stay fast" allow no raised bound, no shards and no skipped test, so the step goes into the forge when it fits:
that is a test-budget defect of `test:ui` with a story of its own. Until then the forge does not run the UI specs, and a
UI change is checked by the local gate only.
