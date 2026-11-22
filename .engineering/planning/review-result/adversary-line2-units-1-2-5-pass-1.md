---
format: aep.planning-md/3
id: review-result:adversary-line2-units-1-2-5-pass-1
kind: review-result
status: active
title: 'Adversary pass on line 2 units 1, 2 and 5 at b2ce1cb: red, 7 findings'
summary: 'runbook canon, the inspector and the Loom pin: 3 introduced findings (the doctor still needs a canon binary), 4 pre-existing in the inspector server; 8 red cases, 2 green. The reviewer resent the report with the header counts corrected; the resend is recorded.'
tags:
- wave-2026-11-22-w1
relations:
- reviews: story:driver-runtime-removed
revision: 1
---
The table and the block are right: 3 `introduced` and 4 `pre-existing`. Finding 5 is `pre-existing`, because the same lines are at base `199f311ffb`: `server.mjs` `walkMd` and `/api/blob`, and `decisions.mjs` `artifactPath`. Unit 2 did not create it or newly reach it. I changed the header only. Everything else is as before, and I made no further test run and did not touch the tree.

unit: units 1, 2 and 5 of line 2 (`impl/loom-line-2`) at b2ce1cb
verdict: red
cases: executed 0→10 (only my new cases and two existing-fixture runs; no full suite run), red 8
origin: introduced 3 / pre-existing 4 / undecided 0
wrote-outside-worktree: `<scratch>/` only (see 6)
needs-coordinator: no

## 1. Diff stat

```
 runbook/tests/adv_canon/mod.rs          | 52 +++++++++++++++++++++++++
 runbook/tests/cli.rs                    |  1 +
 runbook/tests/loom_run/inproc.rs        | 27 +++++++++++++
 ui/test/adv-reads-only-the-run.spec.mjs | 67 +++++++++++++++++++++++++++++++++
 4 files changed, 147 insertions(+)
```

Every path is a test file. `git --no-pager diff --stat -- '*/src/*' ui/server.mjs ui/web` printed nothing. No implementation file was touched and no mutant was used.

## 2. Cases and their red output (each run alone, before any wider run)

**`runbook/tests/adv_canon/mod.rs`** (new module, plus `mod adv_canon;` in `cli.rs`). It ran with `cargo nextest run --locked adv_canon`, 4 tests, 0.32 s, 42 s wall with the build. All 4 are red.
- `doctor_needs_no_canon_binary`: a host with every tool except a `canon` binary. It asserts `runbook doctor` exits 0. Red output:
  ```
  canon check: fail  canon   missing (pinned Canon 0.11.0)   required   every runbook: sdc-drive compiles and evaluates the protocols (driver/src/main.rs)  -- fix: set CANON=<a canon binary of the pinned version, driver/canon-pin.json>, or run scripts/check-protocols.sh once to install the pinned Canon into the cache
    left: ["canon"]   right: []
  ```
- `doctor_names_no_deleted_canon_file`: asserts the doctor's canon line names neither `canon-pin.json` nor an install that no script makes. It fails on the same line.
- `evaluate_cases_script_resolves_no_canon`: the story acceptance (`grep -n canon_resolve scripts/evaluate-cases.sh` prints nothing). Red: `["canon_resolve() {"]`.
- `canon_without_a_subcommand_says_why`: asserts `runbook canon` with no subcommand prints a reason. Red: exit 64 with empty stdout and stderr; the `canon` binary prints clap's usage error.

**`ui/test/adv-reads-only-the-run.spec.mjs`** (new spec). It ran with `slot npx playwright test --workers=2 --project=wide`, 4 tests, 5.1 s wall. All 4 are red.
- `/api/workfacts`: a symlink `work/aep/leak.md` pointing outside the run is read and served.
  ```
  Expected substring: not "OUTSIDE-THE-RUN-MARKER"
  Received string: "{\"files\":[{\"path\":\"C1/work/aep/leak.md\",\"text\":\"OUTSIDE-THE-RUN-MARKER\\n\"}]}"
  ```
- `/api/blob`: a symlink `blobs/<64 hex>` pointing outside the run is served. `Received string: "OUTSIDE-THE-RUN-MARKER`.
- Another Host header: `GET /api/sources` with `Host: rebind.example` returns 200, where a 400, 403 or 421 is expected.
- Request path `//a:99999/`: the server process dies.
  ```
  Expected: 200   Received: "dead"
  ```
  The server log shows `TypeError: Invalid URL` thrown from `new URL(req.url, 'http://x')` and `Node.js v22.23.3`.

**`runbook/tests/loom_run/inproc.rs`** (two tests appended). Both are green; they are cases for unit 5 that I could not make fail.
- `adv_the_program_of_a_person_action_gets_path_and_loom_variables_and_nothing_of_the_runbook_process` (6.2 s): a marker environment variable in the runner's environment does not reach a person's program. Only `PATH` and `LOOM_*` do.
- `adv_an_answer_under_a_model_principal_cannot_take_a_person_action` (4.2 s): a scripted answer under `decider:model` on a person step ends the run with exit 1 and the program never runs.

## 3. Suite run

I did not run the whole crate or the whole UI step: they are known to exceed the budget on this host. These are the only runs after the cases existed; none was over 60 s:
- `cargo nextest run --locked adv_canon`: 4 run, 4 failed.
- `cargo nextest run --locked adv_the_program adv_an_answer`: 2 run, 2 passed.
- The UI spec by file: 4 run, 4 failed.

No test I added is over 5 s except the two green ones in unit 5 (6.2 s and 4.2 s).

Probes that are not committed (scratch):
- `diff-canon.sh` runs the `canon` binary of the same revision (`e81f970d`, from the host's cache) against `runbook canon` on 64 inputs.
- `probe1.sh` runs the traversal and crash probes against the server.

## 4. Findings

| # | file:line | verdict | origin | what was measured / what reaches it |
|---|---|---|---|---|
| 1 | `runbook/src/doctor.rs:77` and `:229-241` | NEEDS-CHANGE | introduced | The doctor still requires a `canon` binary. On a host without one, `runbook doctor` exits 1 with `fail canon missing`. The hint names `driver/canon-pin.json`, which this unit deleted, and says `check-protocols.sh` installs Canon, which it no longer does. The `by:` text still says "sdc-drive compiles and evaluates". Brief item 3 asked that the doctor report Canon from the linked library. Reached by anyone on a clean checkout running `runbook doctor`. The unit's own test `doctor_finds_the_cached_pinned_canon_and_ignores_a_sibling_checkout` (`cli.rs:~270`) pins the stale behaviour: it asserts that doctor fails without a `canon` binary. Fix: probe the linked pin and drop the `CANON` lookup. |
| 2 | `scripts/evaluate-cases.sh:36` | NEEDS-CHANGE | introduced | The story acceptance (`grep -n canon_resolve scripts/evaluate-cases.sh` prints nothing) is not met. The unit left a shim `canon_resolve()` that writes `target/canon-shim/canon` and exports `CANON`. `scripts/run-full-cycle.sh:28`, `run-trial.sh:141` and `run-finding-cycles.sh:143` still call it. The unit's comment says the scripts of the deletion unit use it; it has to go in the same wave as those scripts. |
| 3 | `runbook/src/main.rs:153` | NEEDS-CHANGE | introduced | `runbook canon` with no subcommand returns exit 64 and prints nothing. The `canon` binary prints clap's usage error. Also `runbook canon -V` exits 64 where `canon -V` exits 0. No script calls either. Small. |
| 4 | `ui/server.mjs:785,792` | NEEDS-CHANGE | pre-existing (the same two lines at `199f311ffb:ui/server.mjs:834,839`) | One request path (`//a:99999/`) makes `new URL(req.url, 'http://x')` throw outside any handler, which kills the inspector. A web page in the operator's browser can send that path to `127.0.0.1`, so any page can stop the inspector. Fix: wrap the parse in try/catch and answer 400. |
| 5 | `ui/server.mjs:578` (`walkMd`), `:830` (`/api/blob`), `ui/decisions.mjs:43-49` (`artifactPath`) | NEEDS-CHANGE | pre-existing (the same code at `199f311ffb`; unit 2 neither created nor newly reached it) | `statSync` and `readFileSync` follow links, so a link inside a run directory serves a file outside it. Measured on `/api/workfacts` and `/api/blob`. Reached because the `work/` tree of a run is written by a confined model and the inspector reads it. This does not meet the unit's own claim that nothing outside the started run is served, but the defect was already on main. `artifactPath` only checks the string prefix, not the real path. Fix: `realpath` the file and compare it with the real run root. |
| 6 | `ui/server.mjs:792` (no Host check) | NEEDS-CHANGE | pre-existing | The server answers any `Host` header, so a DNS-rebinding page can read run data. The daemon sits behind a token; the inspector has nothing. Fix: refuse a Host that is not `127.0.0.1:<port>` or `localhost:<port>`. |
| 7 | `ui/web/decide.js:82-139` | INFEASIBLE | pre-existing (from commit `86e78eb`, on main) | The inspector serves a file with a token field and `POST /daemon/commands/answer-decision`. It only runs when the page's `Server-Timing` header names `runbookd`, which `ui/server.mjs` never sends, so it is dead code in the inspector. The new `read-only.spec.mjs` checks `app.js`, `inbox.js` and `index.html` for act code but not `decide.js`. I found nothing that reaches it from the inspector; it is a smaller finding. |

## 5. Attacked and not broken

**Unit 1**
- Differential run of `canon` (`e81f970d`) against `runbook canon` on 64 inputs, comparing exit code, stdout and stderr byte for byte. The inputs were:
  - committed protocols;
  - malformed, empty, BOM, binary, partial, directory, missing and symlinked protocol files, and file names with spaces and control characters;
  - bad, missing and self-imports;
  - the evidence-directory cases (a txt file, a subdirectory, a bad record);
  - `--at junk`;
  - a missing authority file.
  
  Everything is identical, including the refusal text, the `error[unreadable]` and `error[parse]` lines, and the exits 0, 1, 2 and 64. The only differences are the `Usage:` line (`runbook canon …`) and the `--help` summary lines.
- `conform`, `diff`, `floor`, `generate` and `mutate` exit 64 under `runbook canon`. Nothing in the repository calls them.
- `canon_cmd.rs` is a line-for-line copy of `canon-cli` `main.rs`, `check.rs`, `simulate.rs` and `evaluate.rs` at `e81f970`. It can drift from the library on a Canon bump. The pin is read from `Cargo.lock`, so the version number follows the lock.
- Nothing downloads or `cargo install`s a canon binary any more. `protocol/check-ledger.json` digests changed because the Canon commit is in the digest input; the `check` and `simulate` result strings are unchanged.

**Unit 2**
- Every non-GET and non-HEAD method answers 405.
- `ui/server.mjs` has no `spawn`, `exec` or `writeFile`. The one process started on a request is the read-only `yq` in `decisions.mjs:52` (10 s timeout), also on main.
- No route accepts a body, writes into a run directory, or proxies to the daemon.
- Static-file traversal: nine forms (`..%2f`, `%2e%2e`, `%5c`, `//etc/passwd`, `/%2fetc/passwd`, nested `..` paths) all return 404. `/api/blob` rejects `..` and requires a 64-hex hash.
- `app.js` and `inbox.js` carry no act code.

**Unit 5**
- The pin is a forge `rev` for three crates in `Cargo.toml`. All 14 Loom packages in `Cargo.lock` have source `git+https://github.com/beyond10x/loom?rev=70b0e6e0…`. There is no `[patch]`, no `file://` and no `insteadOf`.
- The exit constants in `loom_run/mod.rs:130-153` match `docs/loom-run.md` (0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11 and 64).
- `settings.rs:101` forces `decider.principal` to be `decider:<kind>`, so a model or rule cannot name a person principal. A scripted answer under `decider:model` on a person step ends the run with an error.
- I could not reach exits 2 and 4 from the command line: `runbook run` needs `--settings` and `--spend-ledger`. I confirmed exit 64 (missing required options) and the constants, and nothing more. The unit's existing tests reach exits 0 and 1.
- Design points, not findings: a scripted answer with no principal is recorded under the bound person principal (`library.rs:107-113`, `declared.rs:84`), and a daemon client's `answered_by` is a label.

## 6. Paths written outside the worktree

All under `<scratch>/`, the directory you assigned:
- `<scratch>/cases.patch`
- `<scratch>/probe1.sh`, `<scratch>/diff-canon.sh`, `<scratch>/diff-canon.out`, `<scratch>/server.log`
- `<scratch>/fx/`, `<scratch>/dc/`, `<scratch>/runs/`, `<scratch>/emptyhome/`, `<scratch>/idea.txt`

Inside the worktree (`<trees>/`), git-ignored: `ui/test/fixtures/runs/adv-reads-only-the-run/` (spec fixture) and `ui/node_modules/.pw-results/` (Playwright output). They are not in `git status`.

`<scratch>/cases.patch` is the 147-line diff above. After `git apply -R` and `git reset -q`, `git status --porcelain` printed nothing. `git apply --check <scratch>/cases.patch` succeeded.

```findings
[
  {"file": "runbook/src/doctor.rs", "line": 77, "category": "contract-drift", "severity": "blocker", "verdict": "NEEDS-CHANGE", "origin": "introduced", "message": "runbook doctor still requires a canon binary and fails on a host without one, and its hint names the deleted driver/canon-pin.json and an install no script makes; the brief asked for Canon to be reported from the linked library, and the unit's own doctor test pins the stale behaviour."},
  {"file": "scripts/evaluate-cases.sh", "line": 36, "category": "acceptance", "severity": "warning", "verdict": "NEEDS-CHANGE", "origin": "introduced", "message": "canon_resolve is still defined (a shim that writes target/canon-shim/canon and exports CANON) and called by run-full-cycle.sh, run-trial.sh and run-finding-cycles.sh, so the story acceptance that grep canon_resolve prints nothing is not met."},
  {"file": "runbook/src/main.rs", "line": 153, "category": "boundary", "severity": "note", "verdict": "NEEDS-CHANGE", "origin": "introduced", "message": "runbook canon with no subcommand exits 64 and prints nothing (the canon binary prints the usage error), and runbook canon -V exits 64 where canon -V exits 0."},
  {"file": "ui/server.mjs", "line": 785, "category": "boundary", "severity": "warning", "verdict": "NEEDS-CHANGE", "origin": "pre-existing", "message": "A request path such as //a:99999/ makes new URL(req.url, 'http://x') throw outside any handler and kills the inspector process, so any web page in the operator's browser can stop it."},
  {"file": "ui/server.mjs", "line": 578, "category": "boundary", "severity": "blocker", "verdict": "NEEDS-CHANGE", "origin": "pre-existing", "message": "A symlink inside a run directory (work/aep/*.md, blobs/<hash>, or a bound artifact via decisions.mjs artifactPath) makes /api/workfacts, /api/blob and /api/inbox serve a file outside the run, because stat and read follow links and no realpath check exists."},
  {"file": "ui/server.mjs", "line": 792, "category": "boundary", "severity": "warning", "verdict": "NEEDS-CHANGE", "origin": "pre-existing", "message": "The inspector answers any Host header with run data and has no token, so a DNS-rebinding page can read a run."},
  {"file": "ui/web/decide.js", "line": 82, "category": "judgement", "severity": "note", "verdict": "INFEASIBLE", "origin": "pre-existing", "message": "The inspector serves decide.js, which holds a token field and a POST to the daemon's answer-decision; it runs only when the page's Server-Timing header names runbookd, which ui/server.mjs never sends, and read-only.spec.mjs does not scan this file for act code."}
]
```
