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
checked) and puts its directory ahead of the traps, on the forge and here alike. On 2026-11-24 the forge's first run
of the step failed 34 tests with `aep is not on PATH`; the local forge way showed the same 34 with no aep reachable,
before the change.

On Linux that `sdc-conform` is a static build for `<arch>-unknown-linux-musl` (`cargo run --target
<arch>-unknown-linux-musl --bin sdc-conform`), so it names the pin's static musl aep, which needs no glibc: the gnu aep
of aep 0.81.0 needs glibc 2.39 or newer and does not start on an older host (story
`the-driver-tests-install-the-musl-aep`). The step adds the rustup target when `rustup target list --installed` lacks
it, and runs `aep-install` when `aep-path` finds no pinned aep; both fetch once per host, before any test. The driver
crate has no C code, so the build needs no `musl-tools`: rustup and the host's C compiler as linker are the
prerequisites, here and on the forge's runner, whose earlier step installs the gnu aep the tests no longer run. The
case `aep_pin::the_aep_the_driver_tests_run_is_static` fails when the first `aep` on the tests' `PATH` asks for a
dynamic loader (`PT_INTERP`). On other systems the step builds `sdc-conform` for the host.

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

## Measured on the build node, 2026-11-25 (after the cuts of story `the-ui-step-fits-the-forge-budget`)

| Step | 2 CPUs (`taskset -c 0-1`) | all CPUs | On the forge |
|---|---|---|---|
| `test:ui` | `UI_TEST_FORGE=1`: 571, 550, 587 of 611 passed when the 120 s run bound ended the run | 611 passed in 69.5 s and 93.0 s, 1 of 611 failed in 72.6 s | no |

The load average of the node was 11 to 50 at the start of those runs. The cuts: under `prefers-reduced-motion`, which
the UI tests ask for, the page runs no animation and no transition and draws no blur behind a dialog or the drawer
(each made the browser draw the page again frame after frame), and the run files are read without a real path per
absent name (`ui/runfiles.mjs`). One run without the run bound, under a reporter that measures CPU per test, took
161 s on 2 CPUs: 102 s of CPU in the renderers, 78 s in the children the tests start (servers, players, their guards),
62 s in the Playwright workers, 44 s in the browsers' main processes, 12 s in their compositors.

## The runbook step's waits, 2026-11-26

The budget of `test:runbook` is measured on the forge: the job "Build documentation", step "Test the runbook crate
with the local gate's task". There it took 123.3 s (pull request 112, over the budget) and 112.7 s for the same 1902
tests, whose times summed to 446 s: the step's time is that sum over the 4 test threads, so a test that waits holds a
thread as long as one that computes. This host's disk is slower to sync than the runner's: the same step takes 180 s to
395 s on 2 of its CPUs, most of it in `fsync`, so a local run shows which tests changed and not the forge's time.

| Change | Tests | Forge, before (summed) | Local forge way, after (summed) |
|---|---|---|---|
| End the runner before the stand-in's shell; 1 s to prove an ignored HUP | 5 of `gateway_*` | 38.7 s | 2.9 s, 2.9 s |
| End the pauses once all runs' programs pause (`end_pauses_together`) | 5 of `drain`, 3 of `caps` | 22.9 s | 12.7 s, 34.3 s |
| Stand-in gateways answer once the event their case names happened (the run's steps booked, the cancel returned) | 5 of `adversary_wave3`, 1 of `adversary_b` | 10.3 s | 5.5 s, 6.6 s |
| Stand-ins of a delegation wait for each other, at most 10 s by the clock, under the delegate's wait of 30 s | 1 of `delegate` | 3.5 s | 0.8 s, 1.0 s |

A stand-in that answers on an earlier event than the one its case names lets a wrong product pass: the two late-call
cases of `adversary_wave3` first answered once the program's process was gone, before the engine booked the step, and
passed against a `Gate::book` that ignored the calls in flight (10 of 10) and a relay that read the reservation as it
booked the call (9 of 10); they answer once the run's route file is gone, and both go red 10 of 10. The delegation
stand-ins counted 2000 sleeps of 10 ms (22 s idle), and on one CPU under load a server that takes one call at a time
passed in 33 s, past the delegate's wait of 30 s; they wait 10 s by bash's clock, and that server is red in 11 s.

The helpers that look for the processes below a test walk `/proc/<pid>/task/<tid>/children`: a scan of every process
of a host with thousands of them cost the tests that poll every 10 ms more CPU than their runs. They are one helper,
`runbook/tests/process_sample/mod.rs`, and it judges a child by the program it runs after its `exec`: a child that runs
its parent's executable with its parent's arguments has not called `exec` yet and is left to the next sample. Before,
the name such a child carries from the thread that forked it (`runbook`, `run-<id>`) failed the two cases that refuse
an unexpected program 3 and 1 times in 50 runs each. The `runbook` crate builds at `opt-level = 1` like its
dependencies: a run of all tests on 2 CPUs here took 161.5 s of CPU before and 144.6 s with it and the first of the
changes.

What stays: about 1450 calls of the pinned Loom's `run_tree` with a cancel flag each return up to 100 ms after their
run ended (its watcher sleeps in steps of 100 ms), about 72 s of the summed time, about 18 s of the forge's step.
That is a change of Loom.

## The runbook step's syncs, 2026-11-26

The forge ran the step of one head (`f9f27125`, the waits above cut) twice: 154.5 s (612.8 s summed) and 92.8 s
(367.5 s summed), load 4.8 and 4.9 at the start. The tests that compute took the same on both runners (the decider's
unit tests and `loom_run::decisions` 1.02 times); the tests that write the product's durable state took 1.8 to 3.7
times as long on the slower one (`daemon::caps` 55.1 s against 17.2 s, `daemon::copy_integrity` 49.6 against 17.1,
`daemon::autonomy` 45.2 against 14.9, `daemon::engine` 48.0 against 26.6, `loom_run::inproc` 74.7 against 42.4):
203 s of the 247 s between the two runs. Those writes sync: Loom's run store (it syncs each file it replaces and its
directory), the run's directory, the spend ledgers. Traced here, one test of `loom_run::inproc` made 373 `fsync` calls
(203 in the run's store, 135 in its directory), one of `daemon::autonomy` 209 (112 in the run's tree, 80 in its
directory, 13 in the ledgers), and none with the preload below.

A sync of a file in a test's home under `target/` proves nothing a test can observe (a killed process leaves its
writes in the page cache), so the step runs its tests with the preload `runbook/tests/nosync` (`LD_PRELOAD`, built
by `task test:runbook`): `fsync`, `fdatasync`, `syncfs`, `sync_file_range` and `sync` return at once; the write, the
rename and every error of an open or a write are the product's. A child whose environment a case clears keeps it
(`.env_clear().envs(crate::kept())`; `every_cleared_environment_of_a_test_child_keeps_the_preload` fails on a clear
without it).

| Local forge way (2 CPUs, 4 threads) | Step | Summed | Load at the start |
|---|---|---|---|
| Without the preload | 165.1 s, 199.4 s, 280.6 s | 655.8 s, 794.4 s, 1118.9 s | 20.5, 5.6, 31.2 |
| With it, children of a cleared environment without it | 138.9 s | 550.9 s | 7.2 |
| With it everywhere | 114.1 s, 112.9 s | 453.7 s, 449.1 s | 6.5, 21.4 |

Projected on the forge, each module's time times the share the preload left of it here (the first two runs without
it, the two with it everywhere): 61 s on the faster runner; on the slower one 72 s when its modules that sync take
the faster runner's time, 93 s when they keep the slower runner's extra time except that share. The forge's run is
the measurement of record.

## Why `test:ui` stays out

At 2 CPUs its 611 tests need about 125 s to 133 s (the share that finished in 120 s, taken to the whole run; 161 s in
the one run without the bound); the step budget is 120 s. Before the cuts it was about 177 s, at 4 CPUs about 144 s,
and 94 s on all 64 CPUs of the node. The rules of
"Tests stay fast" allow no raised bound, no shards and no skipped test, so the step goes into the forge when it fits:
that is a test-budget defect of `test:ui` with a story of its own. Until then the forge does not run the UI specs, and a
UI change is checked by the local gate only.
