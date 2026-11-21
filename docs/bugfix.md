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
| 8 | close | `scripts/run-trial.sh` closes the terminable root through its outcome; no outcome requires a decision, so under `--autopilot` the close is `rule:autopilot`'s (`closed BUG-1: "fixed" by rule:autopilot`, `closed_by` in `status.json`) | `terminated` with `principal` |

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
artifact. The run goes through `slot` when slot is on PATH. Two runs per check:

- the whole suite, `cargo test --no-fail-fast`, for the counts;
- the reproducing test alone, `cargo test --test <name> -- --exact <test>`, for its status.

Its status is taken from its own run because, through `slot`, stdout and stderr reach the log as
two streams. A `Running <target>` line can land after the tests it opened, so the full run cannot
tell which target a test line belongs to. In `runs/bugfix-tally-sonnet` and
`runs/bugfix-kvparse-haiku` (before the fix), the reproducing test's line landed under
`Doc-tests` and was read as "did not run".

- `repro-run` is red only when all of these hold: `repro/repro.json` fits its schema; the test file
  holds `#[test] fn <test>` and no `#[ignore]` or `#[should_panic]`; the file is new to the base
  (`bug.yaml` `base`, default `mainline/`); `impl/<file>` is byte for byte `repro/<file>`; `impl/`
  equals the base except that file; the test's own run reports it FAILED; and in the full run
  nothing else fails. A compile error, a passing test, or another failing test makes it not_red, and
  the log line names the reason.
- `tests-run` refuses a fix that changed or removed the test file (the byte comparison). It passes
  only when the test's own run reports it ok and the full run has nothing failed and nothing
  ignored.

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

`scripts/run-finding-cycles.sh` runs the four in autopilot (`bugfix-full`, `bugfix-not-red`,
`bugfix-edited-test`, `bugfix-never-red`). It checks the outcome, `closed_by: rule:autopilot`,
replay, and the records: the green `test_run` names the red `repro_run`'s repro revision at
another change revision.

More seeds, each with one planted bug of another kind:

- `bugfix-percent/`: an inclusive bound written exclusive (`is_valid(100)` is false);
- `bugfix-kvparse/`: a wrong error path (a line without `=` is taken as success);
- `bugfix-tally/`: a state bug across calls (`remove` leaves its count in the total).

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

Why there are two `dissection.check` records and two `repro.run` records although no writer ran
twice: the first of each is about an artifact that did not exist yet. The fixed policy takes
observations before writes (`driver/src/main.rs`, `useful()`). At the opening, `dissection.check` is
admissible (`dissection.checked` is unknown), so it ran first, against `none`: `e0004`, `invalid`,
"no bug-dissection.json". Then `bug.dissect` wrote the document, and `e0013` checked the new
revision: `valid`. Likewise, once the dissection was valid, `repro.run` ran before `test.reproduce`,
against repro `none`: `e0017`, `not_red`, "no repro/repro.json". That check makes no cargo run. After
the test was written, `e0026` was red. A record about `none` sends no writer back (`retry_when`
skips it: `driver/src/main.rs`, `retry_records`), so no LLM call is spent on it. Removing these
records would take an evidence kind per writer (a "written" record, 4 times the states each) or a
precondition on the presence of an artifact, which Canon has no form for.

## Seed runs (2026-11-21, real, autopilot)

Each seed ran once on `claude-sonnet-5-5` and once on `claude-haiku-5-5`, under
`SDC_MAX_LLM_CALLS=15`. The runs are in `runs/bugfix-<seed>-<model>`. Seconds are summed over the
rounds. "Writer runs" counts dissect / reproduce / fix. Only `tally-sonnet-2` ran after the close by
rule was merged, so only it names `rule:autopilot`.

| Run | Outcome | Calls | USD | s | `not_red` retries | Writer runs | Failed checks |
|---|---|---|---|---|---|---|---|
| percent-sonnet | fixed | 3 | 0.126 | 51 | 0 | 1 / 1 / 1 | none |
| percent-haiku | fixed | 5 | 0.023 | 183 | 0 | 1 / 1 / 3 | gate `fmt` once |
| kvparse-sonnet | fixed | 3 | 0.163 | 74 | 0 | 1 / 1 / 1 | none |
| kvparse-haiku | fixed | 4 | 0.012 | 97 | 1 | 1 / 2 / 1 | `repro.run` not_red once (parser) |
| tally-sonnet | not_fixed | 5 | 0.269 | 111 | 0 | 1 / 1 / 3 | `tests.run` failed (parser) |
| tally-haiku | fixed | 3 | 0.009 | 43 | 0 | 1 / 1 / 1 | none |
| pager-haiku | fixed | 4 | 0.013 | 83 | 0 | 1 / 1 / 2 | gate `clippy` once |
| tally-sonnet-2 | fixed by rule:autopilot | 3 | 0.127 | 140 | 0 | 1 / 1 / 1 | none |

Each model wrote a red test at once for every seed. The one `not_red` (kvparse-haiku) and the one
stop (tally-sonnet) were the same defect in `sdc-bugfix`: it read the test's status from the full
run, where through `slot` the test line landed under the wrong `Running` header (see the programs
above). That is fixed: the test now has its own run. `tally-sonnet-2` is the same seed and model
after the fix. The two gate failures were real: Haiku's fix was not formatted, or had a clippy
finding. The gate record sent `change.fix` back, and the next fix passed.

## As a child case (design, 2026-11-21)

The operator: "Break it down, make it composable." The fix loop of
`story:release-rejection-plans-fix-stories` (a rejected release plans fix work) and a failed
acceptance or a rejected merge in `sdc.story` are where a parent would open `sdc.bugfix` for one
defect it has written down.

Nothing in the bindings or tools assumes the case is the root. Every path is relative to the case's
`work/`. No tool reads the case id. `story.update` creates the story only when no parent handed it
in. The unchanged code the red is observed against is `bug.yaml` `base` (default `mainline/`). A
parent whose change is still open names a copy of its code at the opening, because `repro-install`
resets `impl/` to the base plus the test.

What a parent needs:

- an action with `case_open: {protocol: sdc.bugfix, subject: <artifact>}` and the binding
  `child: {name, protocol: .../sdc-bugfix.protocol.yaml, bindings: .../sdc-bugfix.bindings.json}`
  (`docs/driver.md`, "Child cases"). The child's `work/` is a link to the parent's, so the parent
  writes `bug.yaml` (report, `story`, `base`) and the base copy before it opens the child;
- a `children` claim that lists `fixed`, and an `case_outcome` evidence kind.

What it reads back: the `case_outcome` record (`fixed`, `not_reproduced` or `not_fixed`), `impl/` at
the fixed revision (the parent's `change` moves, so its records about the old revision drop out by
revision binding and its checks run again), and the story at `implemented`.

Probe, run with stubs: `protocol/draft/bugfix-parent.protocol.yaml` (144 states, 0 findings) with
`driver/examples/sdc/draft/bugfix-parent.bindings.json`. Run it with `sdc-drive init`, then
`sdc-bugfix seed --from driver/examples/sdc/fixtures/bugfix-pager` in its `work/`, then
`sdc-drive run`. The parent opened `PARENT-1.bug`, the driver drove the child depth first to `fixed`
and closed it, and the parent's record `case_outcome` (`result: fixed`, `child_case: PARENT-1.bug`)
made it terminable with `passed`. `close` and `replay` returned 0 for both cases.

What Canon 0.9.0 allows today:

- One bug-fix child per parent, read by `{children: {in: [fixed]}}`. The form reads every child of
  the case alike, of every revision (`../canon/crates/canon/src/model/predicate.rs:52-59`). So a child
  that ends `not_fixed` makes the claim false for good, and a second attempt cannot make it true:
  there is no retry loop.
- The cost is 8 × 9 = 72 per listed outcome (`docs/protocol-notes.md:354-366`; the probe has 2 × 72).
  `sdc.story` (49,152 states) cannot take a `children` claim at all within the 65,536 bound.
  `sdc.epic` (36,864) already lists `passed` for its story and release children. A bug-fix child
  fits there only if its success outcome is named `passed` (a second listed outcome costs 16 × 17),
  and every failed fix then sinks `children.passed` for the epic.

What needs the unreleased form `{children: {in: [fixed], subject: <s>, revision: current}}` (Canon
`story:child-outcome-subject`, after `story:child-outcome-revision`; `docs/orchestration.md:236-247`):

- the loop: after a `not_fixed` child, or a new rejection, the parent's subject (the report, or
  the rejection record) moves to a new revision. Only the children opened at the current revision
  count, so the old failure no longer counts and a new child is opened. Then it is bounded by the
  parent's own run cap;
- several bug-fix children beside story and release children, without one child's outcome reaching
  into another family's claim (`subject`);
- `sdc.story` as a parent at all. That still also needs a slice-wise state bound
  (`story:canon-slice-bound-uptake`), or the defect loop placed in the epic.

`sdc.story` and `sdc.epic` are unchanged in this revision. Also not in this revision, for the state
count: the report is not an upstream of the dissection (a revised report does not invalidate it),
and `repro` is not an upstream with invalidation of the fix records. Each would cost 4 to 16 times
(`docs/protocol-notes.md:354-356`).

