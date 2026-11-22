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
| 2 | `bug.dissect` | LLM (`sdc-node`), a JSON data document under a schema (`sdc-bugfix schema dissection`, passed as `json_schema`) | none |
|   | `dissection.check` | `sdc-bugfix dissection-check`: the schema, plus `where` names a file (and line) of `impl/` | `dissection_check` valid / invalid |
| 3 | `test.reproduce` | LLM writes `repro/repro.json` `{file, test}` and the test file; `sdc-bugfix repro-install` sets `impl/` to `mainline/` plus that file | none |
|   | `repro.run` | `sdc-bugfix repro-run` (see below) | `repro_run` red / not_red |
| 4 | `change.fix` | LLM, edits a copy of `impl/`; admissible only with `repro.red` | `implementation_change` |
| 5 | `tests.run` | `sdc-bugfix tests-run` (see below) | `test_run` passed / failed |
| 6 | `story.update` | `sdc-bugfix story-update`: the story named by `bug.yaml` `story` moved to `implemented` through `aep`, actor and executor `agent:story.update`; created first if no parent handed it in | `story_record` |
| 7 | `gate.run` | `sdc-bugfix gate-run`: every `gate` command of the case's `checks.yaml` in order (for Rust: `cargo build`, `cargo test`, `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check`) | `gate_run` passed / failed |
| 8 | close | the former trial script closed the terminable root through its outcome; no outcome requires a decision, so under its `--autopilot` the close was `rule:autopilot`'s (`closed BUG-1: "fixed" by rule:autopilot`, `closed_by` in `status.json`); under `runbook run` the close is a `Close` decision (`docs/loom-run-decisions.md`) | `terminated` with `principal` |

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

The commands are the case's `work/checks.yaml` (below), not code of `sdc-bugfix`. Every command
runs as an argv, never through a shell, in a copy under `.sdc/bugfix/`, so nothing is written into
an artifact: `tree/` (a copy of `impl/`) or `base/` (a copy of the base, `bug.yaml` `base`, default
`mainline/`). It goes through `slot` when slot is on PATH (the wait for a slot is bounded, `--pool-wait`; no slot in time is exit 69 and a job the pool ends at its run limit exit 70, both program errors: `docs/driver.md`, "The build pool wait"), with `checks.yaml` `env` and
`CARGO_TARGET_DIR=.sdc/bugfix/target`. A program that is not on PATH (or, for a program with a
`/`, not a file of the tree) exits 127 and an unusable `checks.yaml` 126: program errors, which
a run never records as a check result. Two kinds of run:

- the whole suite, `checks.yaml` `test` (Rust: `cargo test --offline --no-fail-fast`);
- the reproducing test alone, `checks.yaml` `repro` (Rust: `cargo test --offline --test {stem} --
  --exact {test}`), for its status.

The test's status is taken from its own run because, through `slot`, stdout and stderr reach the
log as two streams. A `Running <target>` line can land after the tests it opened, so a full cargo
run cannot tell which target a test line belongs to. In `runs/bugfix-tally-sonnet` and
`runs/bugfix-kvparse-haiku` (before the fix), the reproducing test's line landed under
`Doc-tests` and was read as "did not run".

- `repro-run` is red only when all of these hold: `repro/repro.json` fits its schema (`file`
  matches `test_file` and lies inside the tree, `test` matches `test_name`); the test file defines
  the test (`test_def`, else it names it) and matches no `test_forbid` (Rust: `#[ignore]`,
  `#[should_panic]`); the file is new to the base; `impl/<file>` is byte for byte `repro/<file>`;
  `impl/` equals the base except that file; the test's own run fails as a test (the red rule
  below); and the whole suite on the base, without the test, passes (the `suite` rule, else exit
  0). A test that does not build, a passing test, or another failing test makes it not_red, and the
  log line names the reason and quotes the run's last 40 lines.
- `tests-run` refuses a fix that changed or removed the test file (the byte comparison). It passes
  only when the test's own run passes (the `green` rule, else exit 0) and the whole suite passes
  (the `suite` rule, else exit 0). Rust's rules keep the earlier meaning: the test reported `ok`
  under its name, nothing `FAILED` and nothing `ignored`.

### The red rule

Red keeps its meaning in every language: the one reproducing test ran and failed against the
unchanged code. Two parts:

- The test's own run fails as a test. With a `red` rule (`exit` codes, `output` that must match,
  `refuse` that must not), the rule decides. Rust: exit 101 and the line `test <name> ... FAILED`;
  a compile error also exits 101 but prints no such line, and an unknown test exits 0. Without a
  rule, any exit but 0 counts.
- The suite without the test passes: `test` runs on `base/`. This is the cheapest sound rule that
  needs no rule at all: it is one run, and a failure that the suite shows without the test was not
  caused by the test. A broken command fails here too, so it is never read as red. It also stands
  in for Rust's former check that nothing else failed in the full run with the test.

What a rule cannot tell, no rule tells either: without a `red` rule, a test file that does not
build counts as red in a compiled language. The detection table sets a rule wherever the tool
prints a clear per-test line (Rust, Go, Node's TAP, pytest, Maven, Gradle).

### The commands of a case: `checks.yaml` (`sdc.checks/1`)

| key | | |
|---|---|---|
| `format` | required | `sdc.checks/1` |
| `test` | required | argv that runs the whole suite |
| `repro` | required | argv template that runs one test; placeholders `{file}` (its path in the tree), `{dir}` (its directory, `.` at the root), `{stem}` (the file name without its last extension), `{test}` (its name), replaced inside any argument |
| `gate` | optional | ordered list of `{name, argv}`; each must exit 0; every one runs; may be empty |
| `test_file` | required | a pattern (`^...$`) the reproducing test's path must match: the test area |
| `test_name` | optional | a pattern (`^...$`) its name must match; default `^[A-Za-z_][A-Za-z0-9_]*$` |
| `test_def` | optional | a pattern the test file must match (with `{test}`); without it the file must contain the name |
| `test_forbid` | optional | a pattern the test file must not match (skip, ignore, expected failure) |
| `env` | optional | `NAME: value` for every command |
| `red`, `green`, `suite` | optional | rules `{exit?: [codes], output?, refuse?}` for a failing own run, a passing own run, a passing suite; patterns are multi-line, `{test}` and the others are replaced by their regex-escaped value |

Validation is strict (an unknown key is refused, every miss named). `sdc-bugfix checks` prints the
commands a case runs with.

Where it comes from, at seed time, first match wins:

1. `sdc-bugfix seed --checks <file>`: the outside controller supplies it (with a ticket, or for a
   repository the table does not know);
2. the repository's own `.runbook/checks.yaml` (for a seed from a ticket, only when git tracks it);
3. detection by marker files at the tree's root, from the table
   `driver/examples/sdc/checks-detect.yaml` (`sdc.checks-detect/1`, a data file beside the bindings,
   compiled into `sdc-bugfix`; `SDC_CHECKS_DETECT=<file>` names another): the first row with a marker
   present (and its `contains` pattern matching) gives the commands;
4. nothing matched: the seed fails before it writes anything, with a message that names
   `--checks <file>`. It never guesses.

The seed writes the result to `work/checks.yaml` and prints its source (`--checks`,
`.runbook/checks.yaml`, or `detected <row> (<marker>)`); a seed from a ticket records it in
`.sdc/seed.json` `checks`. A case with no `checks.yaml` (a parent that wrote its `work/` itself)
takes what the seed would have found for the base: its `.runbook/checks.yaml`, else the table.

The table's rows, in order:

| row | marker | suite | one test | red rule |
|---|---|---|---|---|
| `rust` | `Cargo.toml` | `cargo test --offline --no-fail-fast` | `cargo test --offline --test {stem} -- --exact {test}` | exit 101, `test {test} ... FAILED` |
| `go` | `go.mod` | `go test -count=1 ./...` | `go test -count=1 -v -run ^{test}$ ./{dir}` | exit 1, `--- FAIL: {test}` |
| `javascript-node-test` | `package.json` whose `test` script is `node --test` | `node --test --test-reporter=tap` | the same with `{file}` | exit 1, `not ok N - {test}` |
| `javascript-npm` | `package.json` | `npm test` | `npm test -- {file}` | none |
| `python-pytest` | `pyproject.toml`, `pytest.ini` | `python3 -m pytest -q` | `python3 -m pytest -v {file}::{test}` | exit 1, `{file}::{test} FAILED` |
| `java-maven` | `pom.xml` | `mvn -o -q test` | `mvn -o test -Dtest={stem}#{test}` | exit 1, `{stem}.{test}` |
| `java-gradle-wrapper`, `java-gradle` | `gradlew`; `build.gradle`, `build.gradle.kts` | `./gradlew --offline test` (or `gradle`) | `... test --tests *{stem}.{test}` | exit 1, `{stem} > {test} FAILED` |
| `make` | `Makefile` with a `test:` target | `make test` | `make test` (the whole suite: red only through the suite-without-the-test run) | none |

The gate per row: Rust as before; Go `go build`, `go vet`, `go test`; Node the suite again; npm
`npm test`; pytest the suite; Maven `mvn -o -q verify`; Gradle `check`; make none.

Tested here (`driver/tests/bugfix_languages.rs`, no network, no toolchain download): Rust (the
pager seed and its cycles, unchanged), JavaScript with `node --test` (`fixtures/bugfix-js`, no
dependency; Node 22 is a requirement of this repository), and any repository with a supplied
`checks.yaml` (`fixtures/bugfix-script`: plain executable test scripts, no red rule). Each is seeded,
red on the unchanged code with its reproducing test, green after the fix, and a failing gate command
exits 1 and is named. The rows for Go, Python, Java, npm and make exist and are not tested here.

What the record shows, by program:

- red is `repro_run`, `result: red`, `subject: repro`, `subject_revision: R` (the test),
  `upstream_revisions.change: C1` (the unchanged code with the test);
- green is `test_run`, `result: passed`, `subject: change`, `subject_revision: C2`,
  `upstream_revisions.repro: R`: the same test at the next code revision.

`canon-evidence/1` refuses unknown fields (`deny_unknown_fields`,
`../canon/crates/canon/src/model/evidence.rs:16`). So the test id lives in the repro artifact at
revision R (`repro/repro.json`) and in each step's log blob, not in the record itself. The exit
status is the same: `red` says the test exited non-zero (and the suite without it passed), `passed`
says it and the whole suite exited 0; the numbers are in the log blob (`exit N`).

These two records are the red-then-green evidence; the protocol declares both kinds and requires
them, so no third kind exists. The revisions are tree digests: C1 is the tree the red was observed
on, C2 the fixed tree the green was observed on. `change.fix` is admissible only on claim
`repro.red`, `tests.run` and `story.update` also on `repro.red`, and outcome `fixed` on `repro.red`
and `tests.pass`. A fix whose test was never red is therefore refused by the protocol's evaluation:
`cases/12-bugfix-red-first/green-without-red` (a fix and a passed suite, no red) has `change.fix`,
`story.update` and `fixed` blocked, and the cycle `bugfix-never-red` asserts the same on the run's
`decision.json` (`repro.red` false, `change.fix` and `tests.run` blocked, `fixed` blocked,
`not_reproduced` legitimate). The cycles `bugfix-full`, `bugfix-not-red`, `bugfix-edited-test` and
`bugfix-from-ticket` are cycles of `cycles/tree-suite.yaml` (`task test:cycles-loom`,
`docs/cycles-suite.md`). `bugfix-full` asserts one red `repro_run`, one `implementation_change` and
one passed `test_run` among the run's evidence records. The order of the three (the red precedes the
fix, which precedes the green), that the fix and the green name the red's test revision R, and that
the green's tree digest is set and differs from C1, were asserted by the former cycles script. That
C1 and C2 differ compares two records: the protocol cannot make that comparison, and no check of
the suite file makes it.

## Entry and fixture

```console
$ runbook run sdc-bugfix --bugfix <seed dir> --intake <bug report> ...      # docs/loom-run.md, `--dry` for the stub run
```

This opens root `BUG-1` on `sdc.bugfix`. `sdc-bugfix seed` copies the code tree (`<seed dir>` when
it holds a `Cargo.toml`, else `<seed dir>/crate` or `<seed dir>/repo`, else `<seed dir>`) to
`work/mainline/` and `work/impl/` and writes `work/checks.yaml` (above; `--checks <file>` supplies
it). It writes `work/bug.yaml` from the text
(story `story:bug-<run>`), or copies `<seed dir>/bug.yaml`. It creates the story in `work/aep/` and
moves it to `active` (actor `agent:bugfix.seed`). The rest of the run is the runner's loop (`docs/loom-run.md`).

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

The former cycles script ran the four in autopilot (`bugfix-full`, `bugfix-not-red`,
`bugfix-edited-test`, `bugfix-never-red`) and checked the outcome, `closed_by: rule:autopilot`,
replay, and the records: the green `test_run` names the red `repro_run`'s repro revision at
another change revision (the state of each cycle is in `docs/cycles-suite.md`). A fifth, `bugfix-from-ticket`, starts from a ticket (below).

More seeds, each with one planted bug of another kind:

- `bugfix-percent/`: an inclusive bound written exclusive (`is_valid(100)` is false);
- `bugfix-kvparse/`: a wrong error path (a line without `=` is taken as success);
- `bugfix-tally/`: a state bug across calls (`remove` leaves its count in the total).

`cases/12-bugfix-red-first/` holds Canon evaluations. Two matter most: `green-without-red` (a fix
and a green suite with no red record) leaves `change.fix`, `tests.run` and `story.update` blocked;
`test-edited` makes `change.fix` the writer again and `not_fixed` legitimate.

## From a ticket: `sdc.ticket/1` (2026-11-21)

The operator, 2026-11-21: "then I can actually point it to say Jira ticket and then it will just do
it. Maybe we can have external outside controller session, which is maybe first fetching the ticket
context, and the repo context and such, and then starting the runbook with a certain protocol."
`story:ticket-input-and-repo-seed`. The controller that fetches a ticket is a later story; this one
defines the document it writes and makes the bug-fix runbook take it.

```console
$ runbook run sdc-bugfix --bugfix-ticket <ticket file> --intake <any text> ...   # on Loom: the intake is not read for the ticket
$ sdc-bugfix seed --ticket <ticket file> [--repo <worktree>] # what `runbook run` calls in work/
$ sdc-bugfix schema ticket                                         # the JSON schema of the format
```

### The format

| key | | |
|---|---|---|
| `format` | required | `sdc.ticket/1` |
| `id` | required | the ticket's key in its system, a string (quote a number) |
| `kind` | required | `bug` or `feature`; `sdc-bugfix` refuses `feature`: its protocol (`sdc-feature`) does not exist yet |
| `title`, `text` | required | the ticket's words |
| `context` | optional | a list of `{kind: comment\|linked\|doc, ref, text}` |
| `source` | optional | `{system, url?, fetched_at}`: `system` is one free word (`jira`, `github`), `fetched_at` an RFC 3339 instant |
| `repository` | required | `{path, remote?, base: {ref, commit}}`: `path` a local git worktree (relative to the ticket file), `commit` the full commit id its HEAD must be at |
| `check` | optional | `{argv: [...]}`: recorded in `bug.yaml`, not run; the commands that run are `checks.yaml`'s (`seed --checks`) |
| `constraints` | optional | a list of strings, added to the code writers' prompts |
| `branch` | optional | the branch a later hand-back step will create |

Validation is strict: an unknown key at any level is refused, every missing required key is named,
and no refusal quotes a value of the ticket.

An invented example, also the fixture of the cycle `bugfix-from-ticket`
(`driver/examples/sdc/fixtures/bugfix-pager/ticket.yaml`; the cycle sets the commit):

```yaml
format: sdc.ticket/1
id: PAGER-7
kind: bug
title: The last partial page is never shown
text: >-
  An invented ticket. With 11 items and 5 items per page, pager::page_count(11, 5) returns 2 and
  pager::page(&items, 3, 5) returns an empty slice, so item 11 is never shown. Expected: 3 pages,
  the third holding item 11.
context:
- kind: comment
  ref: comment-1
  text: Seen with every total that is not a multiple of the page size.
source:
  system: example
  url: https://tickets.example.invalid/PAGER-7
  fetched_at: 2026-11-21T09:00:00Z
repository:
  path: repo
  base:
    ref: main
    commit: "0000000000000000000000000000000000000000"
check:
  argv: [cargo, test, --offline]
constraints:
- Keep the public API of the crate unchanged.
- Add no dependency.
branch: fix/pager-7
```

### What the seed does

`sdc-bugfix seed --ticket` (`driver/src/bin/sdc-bugfix.rs`, `seed_ticket`) refuses, before it
writes anything, a path that is not the top of a git worktree, a worktree with a changed tracked
file (untracked and ignored files do not count: they are not copied), a HEAD other than
`repository.base.commit`, a submodule or a symlink, a tree above the size cap, and a repository
whose commands are not found (`--checks <file>`, a tracked `.runbook/checks.yaml`, or a tracked
marker of the detection table). `--repo`, when given, must name the same directory as
`repository.path`.

It then copies exactly the files `git ls-files` lists to `work/mainline/` and `work/impl/`, and
writes `work/checks.yaml` and:

- `work/bug.yaml`: `title`; `text`, the title, the text and each context entry in that order,
  separated by a blank line, an entry headed `--- <kind> <ref>`; `story: story:ticket-<id>` (the id
  lower case, every run of other characters one `-`); `ticket` (`id`, `kind`, `source`, no text);
  `constraints`; `check` when the ticket has one;
- `work/.sdc/seed.json` (`sdc.bugfix-seed/1`): the ticket's id, kind and branch, the repository's
  path, remote and base (`ref`, `commit`), the number of files and bytes copied, and where
  `checks.yaml` came from (`checks`).

The story is created in `work/aep/` and moved to `active`, as in the other form. From there the run
is the same as from a seed directory.

Prompts (`sdc-node`, `instructions/`): when `bug.yaml` carries `ticket`, `change.fix`
loses "Standard library only", and `test.reproduce` and `change.fix` end with the ticket's
constraints. A run without a ticket keeps its prompts byte for byte
(`driver/tests/bugfix_prompt.rs` against `driver/tests/fixtures/bugfix-prompt/`). The code writers
get `in/checks.yaml`, and their task quotes its test file pattern, how the test and the suite will
be run, and the gate commands, so the test is written in the repository's language; no prompt names
a language of its own (`story:bugfix-any-language`).

### Limits

- The repository's commands must be found: `--checks`, its `.runbook/checks.yaml`, or a marker of
  the detection table (above). Tested are Rust, JavaScript with `node --test`, and a supplied
  `checks.yaml`.
- The detected commands run offline (`cargo --offline`, `mvn -o`, `gradle --offline`): the
  repository's dependencies must be fetched before the run.
- Bug tickets only: `kind: feature` is refused.
- Size cap: 2000 files and 16 MiB (`SEED_MAX_FILES`, `SEED_MAX_BYTES`), raised for one run with
  `SDC_SEED_MAX_FILES` and `SDC_SEED_MAX_BYTES`. Every model call copies the code tree into its
  scratch directory and back (the model node, `input impl` and `seed impl`).
- No hand-back: nothing is committed or pushed to the repository, and `branch` is only recorded.

The ticket's text reaches the prompts and the run's own records (`runs/`, never committed) only.
The seed prints the repository, the commit and the counts; the cycle checks that the run's output
holds no ticket text.

## Run `runs/bugfix-1` (2026-11-21, real, autopilot)

Command, as given to the former trial script: `bugfix-1 --bugfix driver/examples/sdc/fixtures/bugfix-pager
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
records took an evidence kind per writer (a "written" record) and a precondition on it, since
Canon has no form for the presence of an artifact: `bug.dissect` records `dissection_written`
(claim `dissection.written`) and `dissection.check` waits for it; `test.reproduce` records
`repro_written` (claim `repro.written`) and `repro.run` waits for it. Loom runs a read before a
write and ends the case at the first legitimate outcome, so with the first record about `none`
(`invalid`, `not_red`) it ended `not_reproduced` before any writer ran. A run now holds one
`dissection_check` and one `repro_run` per written revision.

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
`driver/examples/sdc/draft/bugfix-parent.bindings.json`. It was run with the driver's `init`, then
`sdc-bugfix seed --from driver/examples/sdc/fixtures/bugfix-pager` in its `work/`, then
the driver's `run`. The parent opened `PARENT-1.bug`, the driver drove the child depth first to `fixed`
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

