---
format: aep.planning-md/3
id: story:driver-test-budget
kind: story
status: active
title: The driver test step ends within 120 s, no test over 60 s, one integration-test binary
summary: driver tests from about 324 s with 3 tests over 60 s to under 120 s under nextest, one test binary, a killed test over 60 s
tags:
- test-budget
relations:
- decomposes: epic:conductor-succession
scope:
- confidence: cited
  path: AGENTS.md
- confidence: cited
  path: Taskfile.yml
- confidence: cited
  path: driver/.config/nextest.toml
- confidence: cited
  path: driver/Cargo.toml
- confidence: cited
  path: driver/tests
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-11-22T01:33:53Z", actor: "human:operator", revision: 3, executor: "agent:rb2-u5-driver-test-budget", correlation: "operator-standing-acceptance-2026-11-22"}
- {from: "proposed", to: "active", at: "2026-11-22T01:33:54Z", actor: "human:operator", revision: 4, executor: "agent:rb2-u5-driver-test-budget", correlation: "operator-standing-acceptance-2026-11-22"}
---
# Story: driver-test-budget

## Outcome

The test step of `driver/` ends within 120 s, no test runs over 60 s, and a test over 60 s is
killed and fails the step. The integration tests are one binary. Every test that existed still
exists: 122 integration tests and 157 unit tests, 279 in all, with the same names and the same
assertions.

Rule served (`../AGENTS.md`, "Tests stay fast"): the test step ends within 2 minutes, no single test
runs longer than 60 s, an expensive step is made once and never per test, the runner enforces the
budget and the step prints its ten slowest tests, test binaries are few.

## Acceptance

- `slot task test:driver` runs the 279 tests of `driver/` (cargo-nextest) and exits 0, with the
  test run at most 120 s (the nextest summary line) and the ten slowest tests printed.
- A test that sleeps 70 s, added for the probe and removed again, is killed at 60 s (`TIMEOUT`) and
  `slot task test:driver` exits non-zero.
- `cargo nextest list` shows one integration-test binary of `sdc-drive`, `main`, with the 122
  integration tests; their names equal the names of the eleven former binaries.
- `a_capture_that_does_not_appear_is_unavailable_and_the_figure_unknown` ends under 60 s and each
  conformance test no longer pays an unoptimized digest of the pinned ess binary.
- `the_request_validates_against_the_sdc_decision_shape` passes under nextest: the generated
  schemas are made once per run, in a cache on disk.

## Measured

Before: every integration binary run alone with `cargo nextest run --test <name>`, tests in
parallel, machine load average 30 to 50 on 20 CPUs (the times are load times; `Summary` is the
nextest run time, wall).

| binary | tests | run time | slowest test |
| --- | --- | --- | --- |
| conformance | 33 | 280.4 s | 101.2 s `a_capture_that_does_not_appear_is_unavailable_and_the_figure_unknown` |
| bugfix_prompt | 8 | 28.0 s | 26.8 s `the_prompt_of_each_decider_is_its_golden_byte_for_byte` |
| decision_request | 9 | 6.1 s, 1 test fails | 6.1 s `the_shape_check_refuses_a_request_that_is_not_in_the_shape` |
| bugfix_languages | 7 | 4.1 s | 4.1 s `a_javascript_repository_with_node_test_goes_red_then_green` |
| questions_check | 19 | 3.1 s | 2.8 s `loom_mode::a_story_case_takes_story_and_epic_from_the_names_loom_gave` |
| run_compare | 10 | 0.4 s | 0.3 s |
| run_metrics | 11 | 0.3 s | 0.3 s |
| run_timeline | 7 | 0.2 s | 0.2 s |
| progress | 4 | 0.06 s | 0.06 s |
| impl_gate | 4 | 0.09 s | 0.09 s |
| plugin | 10 | 0.04 s | 0.03 s |
| unit tests of the 17 binaries | 157 | 1.4 s | 0.6 s |

The binaries one after another, as `cargo test` runs them: about 324 s. Three conformance tests ran
over 60 s alone (101 s, 86 s, 74 s) and two more over 55 s.

After: one `main` binary and the unit tests in one nextest run, `slot task test:driver`, four
runs on the same loaded machine: test run 88 s, 107 s (with the 70 s probe), 76 s, 63 s; all 279
pass. Slowest test per module of the last run:

| module | tests | slowest test |
| --- | --- | --- |
| conformance | 33 | 27.1 s `the_start_command_is_one_the_delivered_readme_or_manifest_states` |
| bugfix_prompt | 8 | 8.3 s `the_prompt_of_each_decider_is_its_golden_byte_for_byte` |
| questions_check | 19 | 5.7 s |
| bugfix_languages | 7 | 2.6 s |
| decision_request | 9 | 2.4 s |
| others | 59 | 0.7 s or less |

Tests over 20 s on the loaded machine, and what they spend it on:

- `conformance::the_start_command_is_one_the_delivered_readme_or_manifest_states` (27 s) and the
  other conformance tests of 10 to 26 s: 6 to 8 scenarios of a stub product, one process start
  each, and the pinned ess run three times (`compile`, `synthesize`, `run`), per test; the time is
  CPU contention. Alone the first one runs in 9 s at the same load.
- `conformance::a_capture_that_does_not_appear_...` (24 s): a sleep by design. The table's capture
  never appears, so each of the 7 scenarios waits `timeout_s` for it, in 3 runs (the run and 2
  asks again). `timeout_s` is 1 to 120 in the format; it was 3 (63 s), now 1 in this test (21 s of
  waiting).
- `bugfix_prompt::the_prompt_of_each_decider_...` and `..._agent_node_...` (8 s alone, up to 41 s
  under load): about 30 runs of `bindings/tools/decider.sh` and `llm-node.sh` in one test, each a
  bash process with `jq` calls. The file belongs to other units of wave rb2 and is not touched
  here.

## What was wrong

1. Every `sdc-conform` call (`ess-path`, `observe`) resolves the pinned ess by the SHA-256 of its
   113 MB binary (`resolve_ess`, `driver/src/conformance.rs`). The `sha2` crate was built
   unoptimized in the test profile: 5.5 s of CPU per digest, twice per test (the test's `ess()` and
   `observe`), 11 s of CPU per test that did nothing else. `[profile.dev.package.sha2]
   opt-level = 3` makes a digest 0.1 s; one test from 12.3 s to 2.2 s wall.
2. `decision_request.rs` built its generated schemas in an in-process `OnceLock`. Under nextest each
   test is its own process: each paid `ess generate`, and they wrote one shared directory at the
   same time, so `the_request_validates_against_the_sdc_decision_shape` failed. The schemas are
   now a cache on disk, `driver/target/tmp/decision-request-schemas/<digest>`, keyed by the digest
   of `ess/` and the `ess` version, built in a private directory and renamed into place.
3. 11 test binaries linked one after another; now one (`driver/tests/main.rs`, `autotests =
   false`), the files stay at their paths, one `mod` line each.

Hazards of one process for the former binaries, searched in `driver/tests/*.rs`: no
`set_var`/`remove_var`/`set_current_dir` in the process (the tests set the environment of the child
they start), no fixed port or socket of a test (the stub product takes the port it is given), no
`static` or `Mutex`, no shared scratch directory: each file names its own directory under
`CARGO_TARGET_TMPDIR`. Nothing relied on being alone in its process.

## Scope

- `driver/Cargo.toml`: `autotests = false`, `[[test]] main`, `[profile.dev.package.sha2]`.
- `driver/tests/main.rs` (new), `driver/tests/decision_request.rs`, `driver/tests/conformance.rs`
  (one table's capture timeout).
- `driver/.config/nextest.toml` (new): 30 s period, terminate after 2 (60 s), no fail-fast.
- `Taskfile.yml`: task `test:driver`, called from `check`.
- `AGENTS.md`: one line.

## Found, not built

- The runner has no whole-step timeout: the 120 s is checked after the run from nextest's summary
  line, and a hung run is bounded by the 60 s per test only. A hard kill at 120 s needs a wrapper
  that does not count the slot queue wait.
- `bugfix_prompt` (8 s alone, 41 s under load) is the next test to watch for the 60 s bound; one
  golden per test, or a digest of the instruction files and the tools, would cut it. Not done: the
  file is edited by other units of the wave at the same time.
- The conformance run takes the pinned ess digest per `sdc-conform` call; a digest cache keyed by
  path, size and mtime in `driver/src/conformance.rs` would remove the remaining 0.5 s per call.
