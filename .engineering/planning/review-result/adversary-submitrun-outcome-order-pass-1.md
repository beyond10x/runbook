---
format: aep.planning-md/3
id: review-result:adversary-submitrun-outcome-order-pass-1
kind: review-result
status: active
title: 'Adversary pass 1 of the unit submitrun-outcome-order at aceaf2c8: red, 3 warnings, 4 notes, no blocker'
summary: 'The floor guard held over 288 requests below the floor and the generated files regenerate byte for byte. The changelog states counts the head does not print, the table of outcomes in the code keeps the old order unchecked, the page lists the two refusals in the old order; pre-existing: no test holds the floor for a replayed idempotency key, two false numbers in the page, a long key answers 400 first.'
relations:
- reviews: story:submitrun-outcome-order
revision: 1
---
unit: SubmitRun outcome order, branch impl/submitrun-outcome-order, head aceaf2c8 (commits 4d59e7e0, aceaf2c8) against base 566a0c3f
verdict: red (blocker 0, warning 3, note 4)
cases: executed 75→80, red 1 (5 cases added: 1 red on the unit as it stands, 4 green and shown red on three mutants)
origin: introduced 4 / pre-existing 3 / undecided 0
wrote-outside-worktree: 1 directory, `<scratch>/adv/` (copies, logs; listed in part 6); build targets of `runbook/` and `driver/` removed with `cargo clean`
needs-coordinator: whether the handler change and the new scenario are in the scope of the story (the story's scope is `ess/` only and its acceptance says the counts stay the same)

No handler path accepts anything it refused before. The one red case is the stale order of the outcome table `OUTCOMES`. Three of the findings are about what the unit's documents and tests say.

## 1. git --no-pager diff --stat (worktree, after my cases)

```
 .engineering/planning/story/submitrun-outcome-order.md | 7 +++++--
 runbook/tests/daemon/mod.rs                            | 1 +
 2 files changed, 6 insertions(+), 2 deletions(-)
?? runbook/tests/daemon/adv_submit_order.rs   (315 lines, untracked)
```

The story file was already modified when I was handed the tree (the coordinator's; not touched by me). My paths are the new test module and its `mod adv_submit_order;` line in `runbook/tests/daemon/mod.rs`. No implementation file changed.

## 2. Cases added (module `daemon::adv_submit_order`, in the existing `cli` test binary)

| case | asserts | now |
|---|---|---|
| `the_outcome_table_lists_the_submit_outcomes_in_the_declared_order` (:312) | the `submit-run` rows of `OUTCOMES` (`runbook/src/commands.rs`) are in the order of the outcomes of `ess/domains/daemon.yaml` | red |
| `every_pair_of_conditions_of_submit_run_answers_the_earlier_declared_outcome` (:103) | the order is read from the YAML; all 27 satisfiable pairs among the first 8 outcomes, the pair already-submitted > submitted, and each outcome alone: a request where both hold gets the earlier outcome | green |
| `no_request_below_the_floor_is_accepted_or_changes_what_the_daemon_stores` (:152) | 288 requests below the floor (session none/missing/open/closed x own/other daemon x runbook known/missing x intake small/over 1 MiB x context none/inside/outside x key none/fresh/used): each is a refusal with the outcome an independent oracle gives, no run id, no published event, and the journal bytes, the `runs/` and `workspaces/` directories, the run view and the daemon events are unchanged; a control at the floor with an open session is `submitted` | green |
| `the_token_is_checked_before_the_order_on_the_http_route` (:246) | with a token set: no header, `Bearer nope`, `Bearer ` each on three requests (below floor + missing session, below floor + open session, at floor + missing session) give one identical 401 that names no session or floor, and the journal and run count are unchanged; with the token the answers are 409 NoSuchSession and 409 BelowConfinementFloor; `sessions` view lists the session | green |
| `the_token_is_checked_before_the_order_on_the_mcp_tool` (:276) | the same on `tools/call submit_run` of `/mcp` with a token | green |

Red output, the case run alone before the suite (log `<scratch>/adv/run1.log`):

```
running 1 test
test daemon::adv_submit_order::the_outcome_table_lists_the_submit_outcomes_in_the_declared_order ... FAILED
thread '...' panicked at tests/daemon/adv_submit_order.rs:309:5:
assertion `left == right` failed: runbook/src/commands.rs OUTCOMES keeps the order of the domain file
  left: ["intake-too-large", "no-runbook", "context-outside-roots", "no-daemon", "below-floor", "no-session", "session-closed", "already-submitted", "submitted"]
 right: ["intake-too-large", "no-runbook", "context-outside-roots", "no-daemon", "no-session", "below-floor", "session-closed", "already-submitted", "submitted"]
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 545 filtered out
```

(One of my cases, the floor case, was first red for a fault of my own: it took its baseline while the runs of its setup still wrote their end to the journal. I fixed the case to wait for them and ran it again, green. That red was not a finding.)

Pair table (every pair holds; `file:line` is the handler `runbook/src/commands.rs` at aceaf2c8; full output `<scratch>/adv/run2.log`):

| adjacent pair | result | where the earlier one answers |
|---|---|---|
| intake-too-large > no-runbook | holds | :470 |
| no-runbook > context-outside-roots | holds | :473 |
| context-outside-roots > no-daemon | holds | :476 |
| no-daemon > no-session | holds | :484 |
| no-session > below-floor | holds | :488 |
| below-floor > session-closed | holds | :491 |
| session-closed > already-submitted | holds | :494 |
| already-submitted > submitted | holds | :501 |

The non-adjacent pairs also hold (listed in `run2.log`). The pair no-session with session-closed cannot hold together (one session id).

Mutants (copies of the tree in `<scratch>/adv/mut/`; the tree untouched):

| mutant | what the tests do |
|---|---|
| M1: the handler of base 566a0c3f put back | red: the unit's own `a_missing_session_is_refused_before_the_floor_and_an_existing_one_is_not` (`commands.rs:577`, got below-floor), the conformance gate (`the_committed_suite_holds_its_counts_against_the_daemon`: authored 13, passed 12, the new scenario `daemon-a-missing-session-is-refused-before-the-floor` fails), and my pair, floor, http and mcp cases. The scenario does catch the old handler. |
| M3: the floor check skipped when the idempotency key is already used (a below-floor request replays and answers the first run) | the unit's tests, `daemon::commands`, `conformance`, `http`, `mcp`: 77 passed and stayed green. Red only on my pair case (`below-floor > already-submitted: got already-submitted`) and my floor case (6 of 288). Log `<scratch>/adv/mut3.log`. |

## 3. The suite, after the cases exist

```
cd runbook && SLOT_TIMEOUT=30m cargo test --locked --test cli -- daemon::adv_submit_order daemon::commands daemon::conformance daemon::http daemon::mcp
test daemon::adv_submit_order::the_outcome_table_lists_the_submit_outcomes_in_the_declared_order ... FAILED
test result: FAILED. 79 passed; 1 failed; 0 ignored; 0 measured; 466 filtered out; finished in 6.32s
exit 101
```

80 executed (75 without my 5; the 75 is this run minus my module, not a separate run). The slowest test is under 1 s (the 288 combinations: 0.81 s); no test over 5 s. Log `<scratch>/adv/suite.log`.

Counts of claim 4, run by me on the tree: `CONFORMANCE_SUITE=ess/generated/conformance/suite.json CONFORMANCE_ONLY=sdc.daemon cargo test ... daemon::conformance` prints `total 34 executed 32 passed 27 failed 5 skipped 2`, the five failures all "the daemon was started with the principal principal-N and holds agent:conformance" (`StartDaemon/already-started`, `SubmitRun/grant/denied`, `SubmitRun/outcome/no-session`, `/session-closed`, `/submitted`). The authored gate (13 of 13) is green in the suite above. Authored suite on the interpreted target of ESS (`ess verify conform run --target interpreted`, in a scratch copy): 23 scenarios, 23 passed, so the reference interpretation agrees with the new scenario.

Generated files: in `<scratch>/adv/regen/` `task check:ess-generated` exits 0 (validate valid: 4 files, 23 scenarios; schema 145 artifacts current; openapi current; authored current; suite current, sources 3db79226...). In `<scratch>/adv/regen2/` (the `ess/generated` directory deleted, then `task ess-suite:authored`, `task ess-suite:record`, `ess generate` for schema and openapi without `--check`; ess 0.97.4): `DIGEST`, `authored.json`, `suite.json`, every file under `schema/` and `openapi/` are byte-identical to the committed ones. Only the two `.ess-output/state.json` differ, by the random `anchor_id` of a fresh output directory; regenerated over the committed directory (`regen/`) the whole `ess/generated` tree is identical (`diff -r`). The OpenAPI 409 list reads `no-daemon`, `no-session`, `below-floor`, `session-closed` and its `oneOf` follows (`runbookd.yaml:377,390-391`).

## 4. Findings

| severity | file:line | verdict | origin | what was measured / what reaches it |
|---|---|---|---|---|
| warning | `CHANGELOG.md:18` | NEEDS-CHANGE | introduced | The entry says "the answers do not change" and "the daemon's conformance run prints the same counts as before (authored 12 of 12; synthesized 33 total, 31 executed, 26 passed, 5 failed, 2 skipped)". In the tree as it stands the run prints authored 13 of 13 and synthesized 34/32/27/5/2 (measured above), and the next bullet (`:19`) says one answer changes. Both sentences are true of commit 4d59e7e0 alone and false of the head. Reaches: any reader of the changelog. |
| warning | `ess/ess-inputs.yaml:31`, `runbook/src/commands.rs:485-495`, story `Scope` and `Acceptance` | CONFIRMED | introduced | The story's scope is `ess/domains/daemon.yaml`, the generated suites and `DIGEST`; its acceptance is "the same scenario counts as before the reorder". The head also changes the handler (a changed answer for one input: below the floor + missing session, 409 BelowConfinementFloor before, 409 NoSuchSession now) and adds a scenario that moves the counts 12→13 and 33→34. The handler change and the answer are documented and held, and I found no unsafe effect; whether this fits the story is the coordinator's. Reaches: a client that read `below-floor` for that input (HTTP 409, `error` differs). |
| warning | `runbook/src/commands.rs:490-491` (test gap: `runbook/tests/daemon/commands.rs:551-585`) | CONFIRMED | pre-existing | A request below the floor that names a used idempotency key and is accepted as `already-submitted` (a replay: it hands out the first run's id) is not held by any test of the unit or of `daemon::commands`, `conformance`, `http`, `mcp`: mutant M3 stays green on all of them (77 passed). Red on my pair and floor cases. Origin by reading the base tests (`git show 566a0c3f:runbook/tests/daemon/commands.rs`: no test combines `Confinement::Local` with a key); I did not run the mutant against the base. Reaches: nothing found in the code today; it is a guard with a missing test. |
| note | `runbook/src/commands.rs:134-135` | NEEDS-CHANGE | introduced | `OUTCOMES` lists `below-floor` above `no-session`; the domain file now lists the reverse. Its comment says a test holds it equal to the domain file, but `the_outcome_table_is_the_domain_file` (`commands.rs:792`) compares sets, so nothing sees the order. Red case: `adv_submit_order.rs:312`. Nothing reads the table's order (`OUTCOMES` is only looked up by name), so no behavior depends on it. |
| note | `docs/daemon-api.md:37` | CONFIRMED | introduced | The refusals of `submit-run` are listed `BelowConfinementFloor`, `NoSuchSession`: the reverse of what the handler and the domain file now do. The list was not in the declared order before either (`NoSuchRunbook` before `IntakeTooLarge`), and the page does not call it an order. A reader may take it as the order of the answers. |
| note | `docs/daemon-api.md:343` and `:350-351` | CONFIRMED | pre-existing | The unit edited this paragraph (12→13) and left two false sentences: "3 more are of `sdc.decision`" (the authored scenarios are 13 of `sdc.daemon` and 10 of `sdc.decision`, also 12 and 10 at base; the conformance line prints `out of domain: 10`) and "15 of 15 scenarios, as before" on the interpreted target (it runs 23 of 23). |
| note | `runbook/src/commands.rs:460-462` | CONFIRMED | pre-existing | The length check of `idempotency_key` answers `Fault::Invalid` (HTTP 400) before every declared outcome, including `intake-too-large`; it is not an outcome of `SubmitRun`. Unchanged by the unit (the same lines at base). Reaches: a client that sends a key over the limit. |

## 5. Attacked and held

- Every path through `submit_run_for` before and after (read both): the only difference in order is no-session before below-floor. The checks, in the order no-daemon, no-session, below-floor, session-closed, key replay, then the first write (directory creation :508, `Fact::RunSubmitted`, spawn) after them, are the same set; nothing was removed, and no refusal became an acceptance. The new session read `st.session(sid)` runs under the same lock, before the floor, and writes nothing.
- No request below the floor is accepted or changes the stored state, over the 288 combinations (session missing/open/closed/none, daemon id not the daemon's, runbook missing, intake over 1 MiB, context root inside/outside, key none/fresh/used): journal bytes, run and workspace directories, runs and daemon events all unchanged. "Daemon not started" is reached only as a daemon id the daemon does not hold (`submit_run_for("another-daemon", ...)`); an in-process daemon always holds its row.
- The host facts (`runbook_known`, `context_under_roots`, which canonicalizes a path) are read before the lock and before the daemon check, as at base; nothing the unit moved reaches a file read earlier.
- Token before everything: the middleware answers 401 before the route, for HTTP `POST /daemon/commands/submit-run` and for `/mcp` `tools/call submit_run`; the 401 is identical for the three request shapes and names no session or floor. The unit did not touch the middleware.
- Session existence: an authenticated caller can now probe it with a request that costs nothing (confinement `local` under a `linux` floor: no-session or below-floor). Before, a probe needed an at-floor request that starts a run. It tells nothing new: the `sessions` view lists the same names behind the same token.
- Generated files: byte-identical on regeneration (part 3); the OpenAPI 409 order follows the declaration.
- The scenario goes red on the old handler (M1); it also holds the closed-session case (`below-floor` above `session-closed`). The interpreted target of ESS passes it.
- Documents: the new CHANGELOG sentence at :19 ("still refused as BelowConfinementFloor" with an open or closed session) and the added daemon-api numbers (13 of 13) are true.

## 6. Paths written outside the worktree

- `<scratch>/adv/` (all of it mine): `run1.log`, `run2.log`, `run3.log`, `suite.log`, `synth.log`, `mut1.log`, `mut3.log`, `record.log`, `commands.old.rs`, `commands.new.rs`, `regen/`, `regen2/`, `mut/` (copy of the tree without `target`, `.git`, `website`; M1 and M3 mutated `mut/runbook/src/commands.rs`; its `target` removed with `cargo clean`).
- `<scratch>/adversary-1.md` (this report).
- The harness saved one large tool output under its own project directory (not written by me on purpose).
- `cargo clean` run in `<trees>/b10x/runbook/ess-reorder/runbook` (1.2 GiB removed) and `.../driver` (nothing to remove). The test scratch of `daemon::*` is under `runbook/target/tmp`, gone with it.

Not checked: the stdio MCP transport (`runbook mcp` as a child process) with a token; `runbook serve` as a process; the UI paths (`ui_*` tests); the whole `runbook/` test step and `task check` (not run, by the brief); the base handler under the mutant M3 (origin by reading); the next ESS release 0.98.0 (not installed; the unit's claim that it accepts the file rests on the ESS session's notice).

```findings
- file: CHANGELOG.md
  line: 18
  category: contract-drift
  severity: warning
  verdict: NEEDS-CHANGE
  origin: introduced
  message: the entry says the answers do not change and the conformance counts stay at 12 of 12 and 33 total, while the head prints 13 of 13 and 34/32/27/5/2 and the next bullet says one answer changes
- file: runbook/src/commands.rs
  line: 485
  category: acceptance
  severity: warning
  verdict: CONFIRMED
  origin: introduced
  message: the story scopes the change to ess/ and asks for unchanged scenario counts, the head also changes the handler's answer for one input and moves the counts 12 to 13 and 33 to 34; whether that fits the story is the coordinator's call
- file: runbook/src/commands.rs
  line: 491
  category: mutant
  severity: warning
  verdict: CONFIRMED
  origin: pre-existing
  message: a mutant that skips the floor check for a request whose idempotency key is already used (a below-floor replay answered as already-submitted) leaves every existing daemon, conformance, http and mcp test green and fails only the adversary cases
- file: runbook/src/commands.rs
  line: 134
  category: contract-drift
  severity: note
  verdict: NEEDS-CHANGE
  origin: introduced
  message: OUTCOMES lists below-floor above no-session while the domain file lists the reverse, and the test that is said to hold the table equal to the file compares sets, so the order is unchecked (red case adv_submit_order.rs:312)
- file: docs/daemon-api.md
  line: 37
  category: contract-drift
  severity: note
  verdict: CONFIRMED
  origin: introduced
  message: the refusal list of submit-run still reads BelowConfinementFloor before NoSuchSession, the reverse of the handler and the domain file
- file: docs/daemon-api.md
  line: 343
  category: contract-drift
  severity: note
  verdict: CONFIRMED
  origin: pre-existing
  message: the edited paragraph says 3 more scenarios are of sdc.decision (10 are) and says the interpreted target passes 15 of 15 (it runs 23 of 23)
- file: runbook/src/commands.rs
  line: 461
  category: boundary
  severity: note
  verdict: CONFIRMED
  origin: pre-existing
  message: an idempotency_key over the limit answers HTTP 400 before every declared outcome of SubmitRun, including intake-too-large, and the declared order does not list it
```
