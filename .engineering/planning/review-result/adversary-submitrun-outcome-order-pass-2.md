---
format: aep.planning-md/3
id: review-result:adversary-submitrun-outcome-order-pass-2
kind: review-result
status: active
title: 'Adversary pass 2 of the unit submitrun-outcome-order at a7e41a77: green'
summary: 'Six findings of pass 1 are fixed, the long idempotency key is documented as a fault. The mutant that skips the floor for a used key is red on two committed cases. The order test goes red for a swapped table of each of the 15 commands. Counts rerun: 13 of 13 authored; synthesized 34, 32, 27, 5, 2. One note: the order of the answer-decision handler is held by a case that the next commit takes into the unit.'
relations:
- reviews: story:submitrun-outcome-order
revision: 1
---
verdict: green

unit: `submitrun-outcome-order`, fix round 1, head `a7e41a77` against `aceaf2c8` (`git diff aceaf2c8 a7e41a77`). All seven findings of pass 1 are fixed; one new case of mine is green and one gap (below) is a note. No blocker, no warning left.

cases: executed 81→82 (81 is the committed state of the daemon modules: pass 1 had 80, the new order test makes 81; my one new case makes 82), red 0
origin: introduced 0 / pre-existing 1 / undecided 0
wrote-outside-worktree: `<scratch>/adv/` (logs, `mut2/` a copy of `a7e41a77`, `swap.awk`); build targets removed

## What I wrote in the tree (untracked; the tree's other changes are the coordinator's)

```
 runbook/tests/daemon/mod.rs   | 1 +        (the line `mod adv_answer_order;`, not committed)
?? runbook/tests/daemon/adv_answer_order.rs  (untracked, 1 case)
```

`.engineering/` files shown as modified or untracked in the tree are the coordinator's. `runbook/src`, `ess/`, `docs/` and `CHANGELOG.md` are equal to `a7e41a77` (`git diff --quiet`).

New case `daemon::adv_answer_order::answer_decision_answers_in_the_order_of_the_domain_file_where_two_conditions_hold`: a state directory is written with facts (a run `r1` that is not waiting, no run `ghost`, for each of them a Withdrawn, an Answered and an Open request) and a daemon is started on it; for each pair where two conditions hold it asks the handler. Green, run alone first:

```
no-request > no-run: no-request            not-of-run > withdrawn: not-of-run
not-of-run > already-answered: not-of-run  not-of-run > no-run: not-of-run
withdrawn > no-run: withdrawn              withdrawn > run-not-waiting: withdrawn
already-answered > no-run: already-answered
already-answered > run-not-waiting: already-answered
no-run alone: no-run                       run-not-waiting alone: run-not-waiting
test result: ok. 1 passed; 0 failed
```

## 1. State of each pass-1 finding

| pass-1 finding | state | evidence |
|---|---|---|
| `CHANGELOG.md:18`, "answers do not change" and old counts | fixed | now says "One answer changes with it (next bullet)" and 13 of 13, 34/32/27/5/2 with the before numbers; the numbers match what I ran (part 5) |
| `runbook/src/commands.rs:485`, scope of the story vs handler change and counts | not fixed, by nature: it is the coordinator's decision. The changelog now states the change and the counts honestly. | unchanged in `needs-coordinator` |
| `runbook/src/commands.rs:491`, mutant skips the floor for a used key | fixed | mutant in a copy of `a7e41a77`: red on committed `daemon::submit_order::every_pair_of_conditions_of_submit_run_answers_the_earlier_declared_outcome` (`below-floor > already-submitted: got already-submitted`) and `no_request_below_the_floor_is_accepted_or_changes_what_the_daemon_stores` (6 of 288); the committed cases are the ones that catch it now. Log `<scratch>/adv/p2-m3.log`: 79 passed, 2 failed |
| `runbook/src/commands.rs:134`, `OUTCOMES` order | fixed | `submit-run` rows: `no-session` above `below-floor`; `answer-decision`: `already-answered` above `no-run`; comment says "in the order the domain file declares them" |
| `docs/daemon-api.md:37`, refusal list order | fixed | the `submit-run` and `answer-decision` lists are in the domain file's order (`IntakeTooLarge`, `NoSuchRunbook`, `ContextOutsideRoots`, `NoSuchDaemon`, `NoSuchSession`, `BelowConfinementFloor`, `SessionClosedToRuns`) |
| `docs/daemon-api.md:343`, "3 more" and "15 of 15" | fixed | now "10 more are of `sdc.decision`" and "23 of 23 scenarios (13 of `sdc.daemon`, 10 of `sdc.decision`)" |
| `runbook/src/commands.rs:461`, over-long key answers 400 first | fixed in part, as a document: a sentence in `docs/daemon-api.md` says a key over the limit is not an outcome of `submit-run` and is refused 400 before every outcome. The behavior is unchanged, which is right: it is a `Fault`, not an outcome. | the sentence is true (the handler's first statement; HTTP `Fault::Invalid` is 400) |

## 2. My 5 cases as committed

`runbook/tests/daemon/submit_order.rs` against my original (copy `<scratch>/adv/mut/.../adv_submit_order.rs`), `diff`: the module doc comment; `type Post<'a> = &'a dyn Fn(&Value, Option<&str>) -> (u16, String)` used in the signature of `without_auth_is_one_answer`; the one-element loop `for id in ["open-s"] { ... }` written as one `open_session` call. Nothing else differs; the assertions are the same. The five run green (part 4).

## 3. The mutant that skips the floor for a used key

Red on the committed cases (finding 3 above). Done in a scratch copy of `a7e41a77` made with `git archive`; the tree was not touched, and `runbook/src/commands.rs` in the tree has sha256 `f254149b...` equal to the one at `a7e41a77`.

## 4. The new order test

`the_outcome_table_lists_every_command_in_the_order_of_the_domain_file` (`runbook/tests/daemon/commands.rs:798`) groups the rows of the domain file and of `OUTCOMES` by command into a `BTreeMap<command, Vec<outcome>>` and compares the maps whole, so any change of order within a command, and a missing command, fails it.

Mutant in a scratch copy: an awk script (`<scratch>/adv/swap.awk`) swaps the first two rows of each command of `OUTCOMES`: 15 commands (`answer-decision`, `cancel-run`, `close-session`, `end-orphaned-run`, `end-run`, `fail-run-start`, `open-session`, `record-run-event`, `request-decision`, `resume-run`, `start-daemon`, `start-run`, `submit-run`, `wait-on-decision`, `withdraw-decision`). Result: the new test is red and its printed left and right maps differ for each of the 15 keys (compared key by key from the log, `<scratch>/adv/p2-swap.log`); `the_outcome_table_lists_the_submit_outcomes_in_the_declared_order` (mine) is red too; the set test `the_outcome_table_is_the_domain_file` stays green, as before.

The handler of `answer-decision` answers in the file's order (`no-request, not-of-run, withdrawn, already-answered, no-run, run-not-waiting, answered`): read at `runbook/src/commands.rs:578-595` and held by my new case above for every pair that can hold together (`withdrawn` with `already-answered` cannot: one request has one state). The handler lines are not in the diff `aceaf2c8..a7e41a77`.

## 5. The numbers

| claim | I measured |
|---|---|
| 13 of 13 authored | `daemon::conformance` gate passes (`GATE` 13/13/13/0) in the suite below |
| synthesized 34, 32, 27, 5, 2 | `CONFORMANCE_SUITE=ess/generated/conformance/suite.json CONFORMANCE_ONLY=sdc.daemon cargo test ... daemon::conformance --nocapture`: `total 34 executed 32 passed 27 failed 5 skipped 2 (out of domain: 78)`; the five failures are `StartDaemon/outcome/already-started`, `SubmitRun/grant/denied`, `SubmitRun/outcome/no-session`, `/session-closed`, `/submitted`, all "the daemon was started with the principal principal-N and holds agent:conformance" (the synthesized principal, as before) |
| "10 more are of sdc.decision" | 13 `domain: sdc.daemon` and 10 `domain: sdc.decision` in `ess/scenarios/` |
| 23 of 23 on the interpreted target | `ess verify conform run --target interpreted --path ess --suite ess/generated/conformance/authored.json --report-format 2` in a copy: `23 scenarios: 23 passed, 0 failed, 0 error, 0 unsupported`; `ess specify validate`: `valid`, 4 files, 23 scenarios |

## 6. No check removed or moved beyond the reorder

`git diff aceaf2c8 a7e41a77 -- runbook/src` has three hunks, all in the `OUTCOMES` table and its doc comment (lines 117-165); the handlers are untouched. Against base `566a0c3f` the handler differs only by the session read before the floor and the closed-session check after it (pass 1).

## Runner's summary

```
cargo test --locked --test cli -- daemon::adv_answer_order daemon::submit_order daemon::commands daemon::conformance daemon::http daemon::mcp
test result: ok. 82 passed; 0 failed; 0 ignored; 0 measured; 466 filtered out; finished in 11.67s
exit 0
```

Slowest of my cases under 1 s; no test over 5 s. Synthesized run: `test result: ok. 1 passed; ... finished in 3.01s`.

## New findings

| severity | file:line | verdict | origin | what |
|---|---|---|---|---|
| note | `runbook/tests/daemon/commands.rs:798` (and the handler `runbook/src/commands.rs:578-595`) | CONFIRMED | pre-existing | The new order test compares the table with the domain file; the order in which the handler of `answer-decision` answers is held by no committed test (only my untracked `adv_answer_order.rs` holds it; the `SubmitRun` handler is held by `submit_order.rs`). The handler is correct today. Same for the handlers of the other commands, which I did not read. |

## Paths written outside the worktree

`<scratch>/adv/` (all of it mine): `p2-answer.log`, `p2-m3.log`, `p2-swap.log`, `p2-suite.log`, `p2-synth.log`, `left.txt`, `right.txt`, `swap.awk`, `commands.swapped.rs`, `submit_order.committed.rs`, `mut2/` (a copy of `a7e41a77`; M3 and the swapped table were applied to its `runbook/src/commands.rs`; its `target` removed). `<scratch>/adversary-2.md` (this report). `cargo clean` run in `runbook/` (1.1 GiB), `driver/` (nothing to remove) and `mut2/runbook/` (1.3 GiB).

Not checked: the handlers of the other commands against the domain file's order; the stdio MCP transport; the whole `runbook/` test step.

```findings
[
  {
    "file": "runbook/tests/daemon/commands.rs",
    "line": 798,
    "category": "mutant",
    "severity": "note",
    "verdict": "CONFIRMED",
    "origin": "pre-existing",
    "message": "no committed test holds the order in which the handler of answer-decision answers where two conditions hold; only the untracked adversary case adv_answer_order.rs does, and the handler is correct today"
  }
]
```
