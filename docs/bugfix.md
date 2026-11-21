# One bug fix, red first: `sdc.bugfix` (2026-11-21)

The operator, 2026-11-21: "oh, there's a bug, this is the input, dissect it, write the red test, fix
it, update the story, run the gate, done." And: "Break it down, make it composable." Decision D146;
`story:bugfix-cycle`.

`protocol/sdc-bugfix.protocol.yaml` is one case with no child cases: no intake, direction, epic,
spec or plan. `sdc.story` is unchanged. `canon check`: 1024 states, 0 findings.

## Steps

Each step is admissible only after the one before it (the preconditions in the protocol):

| # | action | executor | records |
|---|---|---|---|
| 1 | (input) | the report `bug.yaml` and the code tree `impl/` with its base `mainline/` | none |
| 2 | `bug.dissect` | LLM (`llm-node.sh`), a JSON data document under a schema (`sdc-bugfix schema dissection`, passed as `json_schema`) | none |
|   | `dissection.check` | `sdc-bugfix dissection-check`: the schema, plus `where` names a file (and line) of `impl/` | `dissection_check` valid / invalid |
| 3 | `test.reproduce` | LLM writes `repro/repro.json` `{file, test}` and the test file; `sdc-bugfix repro-install` sets `impl/` to `mainline/` plus that file | none |
|   | `repro.run` | `sdc-bugfix repro-run` (see below) | `repro_run` red / not_red |
| 4 | `change.fix` | LLM, edits a copy of `impl/`; admissible only with `repro.red` | `implementation_change` |
| 5 | `tests.run` | `sdc-bugfix tests-run` (see below) | `test_run` passed / failed |
| 6 | `story.update` | `sdc-bugfix story-update`: the story named by `bug.yaml` `story` moved to `implemented` through `aep`, actor and executor `agent:story.update`; created first if no parent handed it in | `story_record` |
| 7 | `gate.run` | `sdc-bugfix gate-run`: `cargo build`, `cargo test`, `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check` | `gate_run` passed / failed |
| 8 | close | `scripts/run-trial.sh` closes the terminable root through its outcome (no decision: no outcome requires one) | `terminated` |

Outcomes: `fixed` (all of the above), `not_reproduced` (no valid dissection, or no red), `not_fixed`
(red, but the fix fails the suite or the gate). The two failure outcomes end a case only once nothing
is useful any more, so first the writer spends its runs.

Retries: a failed check is fed back to the writer of that step through `retry_when`
(`dissection_check` invalid to `bug.dissect`, `repro_run` not_red to `test.reproduce`,
`test_run` failed and `gate_run` failed to `change.fix`). The writer gets the record and its log in
`in/feedback.md`. Each writer has `max_runs` 3, as on the story protocol. A spent writer with no
outcome left ends the run `writer_spent`.

No approval and no decider request in this revision. The obvious next addition is a merge approval
(`merge.approve` under the capability `change.merge`, as in `sdc.story`), then `change.merge`
copying `impl/` to `mainline/`.

## The programs (red and green)

Every cargo run happens in a copy of `impl/` under `.sdc/bugfix/`, so nothing is written into an
artifact. stdout and stderr go into one stream, so each `test <name> ... <status>` line is matched to
the `Running <target>` line above it. The run goes through `slot` when slot is on PATH.

- `repro-run` is red only when all of these hold: `repro/repro.json` fits its schema;
  the test file holds `#[test] fn <test>` and no `#[ignore]` or `#[should_panic]`; the file is new
  to `mainline/`; `impl/<file>` is byte for byte `repro/<file>`; `impl/` equals `mainline/` except
  that file; and in `cargo test --no-fail-fast` the named test in that target FAILED while every
  other test is ok. A compile error, a passing test, or another failing test makes it not_red, and
  the log line names the reason.
- `tests-run` refuses a fix that changed or removed the test file (the byte comparison). It passes
  only when the named test is reported ok, nothing failed and nothing is ignored.

What the record shows, by program:

- red is `repro_run`, `result: red`, `subject: repro`, `subject_revision: R` (the test),
  `upstream_revisions.change: C1` (the unchanged code with the test);
- green is `test_run`, `result: passed`, `subject: change`, `subject_revision: C2`,
  `upstream_revisions.repro: R`: the same test at the next code revision.

`canon-evidence/1` refuses unknown fields (`deny_unknown_fields`,
`../canon/crates/canon/src/model/evidence.rs:16`). So the test id lives in the repro artifact at
revision R (`repro/repro.json`) and in each step's log blob, not in the record itself.

## Entry and fixture

```console
$ scripts/run-trial.sh <run> --bugfix <seed dir> ["<bug report>"] [--autopilot | --dry]
```

This opens root `BUG-1` on `sdc.bugfix`. `sdc-bugfix seed` copies the crate (`<seed dir>`, or
`<seed dir>/crate`) to `work/mainline/` and `work/impl/`. It writes `work/bug.yaml` from the text
(story `story:bug-<run>`), or copies `<seed dir>/bug.yaml`. It creates the story in `work/aep/` and
moves it to `active` (actor `agent:bugfix.seed`). The rest of the run is `run-trial.sh`'s loop.

Seed: `driver/examples/sdc/fixtures/bugfix-pager/` holds the crate `pager`, standard library only,
5 passing tests, a clean gate, and one planted bug: `page_count` is `total / per_page`, which drops
the last partial page. `bug.yaml` holds the report. `stub-dissection.json`, `stub-repro/` and
`stub-fix/` are the stubs, and `cycles/<name>/` are the stub cycles (via `SDC_STUB_FIXTURES`):

| cycle | what happens | end |
|---|---|---|
| (none) | the full flow | `fixed` |
| `not-red` | the first test passes on the seed: `repro: not red: ... passes on the unchanged code`, sent back; the second is red | `fixed` |
| `edited-test` | the first fix weakens the test: `tests: FAILED refused: ... the reproducing test was changed`, sent back | `fixed` |
| `never-red` | every test passes on the seed: three runs of `test.reproduce` | `not_reproduced` |

`cases/12-bugfix-red-first/` holds Canon evaluations. Two matter most: `green-without-red` (a fix
and a green suite with no red record) leaves `change.fix`, `tests.run` and `story.update` blocked;
`test-edited` makes `change.fix` the writer again and `not_fixed` legitimate.

## Run `runs/bugfix-1` (2026-11-21, real, autopilot)

Command: `scripts/run-trial.sh bugfix-1 --bugfix driver/examples/sdc/fixtures/bugfix-pager
--autopilot "<the report of bug.yaml>"`, with every class on `claude-sonnet-5-5` and
`SDC_MAX_LLM_CALLS=15`. Run through metaharness.

- The run closed `fixed` in one round (`closed BUG-1: "fixed" (exit 0)`); replay matches.
- 3 LLM calls, no retry, 0.167 USD in total: `bug.dissect` 0.040 USD, `test.reproduce` 0.047 USD,
  `change.fix` 0.080 USD. 51 s wall clock, 37 s of it in the model.
- Red: `e0026-repro.run`, `red`, repro `t1d4f9dfe79dabf19`, change `tf73db91c7ac144cf`. Its log:
  `repro: red: test partial_last_page_is_shown (tests/partial_last_page.rs) fails on the unchanged
  code, the other 5 tests pass`.
- Green: `e0035-tests.run`, `passed`, change `tf9ab48471fb98245`, repro `t1d4f9dfe79dabf19`. Its
  log: `tests: passed: test partial_last_page_is_shown (tests/partial_last_page.rs) ok, 6 tests ok`.
- The fix is one line: `total / per_page` became `total.div_ceil(per_page)`. All four gate checks
  passed, and the story went `active -> implemented` by `agent:story.update`.

## As a child case

Nothing in the bindings or tools assumes the case is the root. Every path is relative to the case's
`work/`. No tool reads the case id. The story step works whether the story exists or not. To open it
as a child with Canon's child cases, a parent (an epic, or `sdc.story` for a bug found in review)
needs:

- an action with `case_open` and the binding `child: {protocol: ".../sdc-bugfix.protocol.yaml",
  bindings: ".../sdc-bugfix.bindings.json", name: "<bug>"}` (`docs/driver.md`, "Child cases");
- in the shared `work/`: `bug.yaml` (with `story: story:<name>`), `impl/` and `mainline/`, and
  optionally `aep/` with the story. Today the parent writes `bug.yaml` before the opening. Two bugs
  at once need a per-child `work/` (`epic:concurrent-child-cases`);
- a `children` claim that lists the outcome `fixed`.

It reads back: the `case_outcome` record (`fixed`, `not_reproduced` or `not_fixed`), `impl/` at the
fixed revision (the parent's `change` moves with it), and the story at `implemented`.

Not in this revision, for the state count: the report is not an upstream of the dissection (a revised
report does not invalidate it), and `repro` is not an upstream with invalidation of the fix records.
Each would cost 4 to 16 times (`docs/protocol-notes.md:354-356`).
