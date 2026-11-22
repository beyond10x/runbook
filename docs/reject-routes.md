# Reject and cap routes (audit, 2026-11-21)

This audit covers every point in the eight protocols where an unattended run (`scripts/run-trial.sh
<run> --autopilot`, decider final, no person) can be told no, and every writer whose cap can stop it.
It was prompted by these stops: `rejected_without_rework_path` (`runs/trial-3-haiku-3`
`release.publish`, `runs/trial-3-haiku-5` root `close`) and `writer_spent`
(`runs/trial-3-haiku-2` `spec.author`, `runs/trial-3-haiku-4` `change.implement`).

The tables describe the state after these branches merge: `fix/attempt5-intake` (102ce99),
`impl/review-node` (5e2791e) and `fix/attempt5-close` (83d3cbf). A "Branch" column names the
branch that changes a row. Line numbers are on `main` (d241d21) plus this branch.
Routes R1-R9 were ruled on by the coordinator (D145). Each has a cycle in
`scripts/run-finding-cycles.sh`.

## Table 1: where a no is possible in autopilot

`decide` (`scripts/run-trial.sh:379-503`) handles every reject. A reject at an approval goes to
`rework` (`:531-566`); `withhold` (`:578-596`) handles a reject with an end in the protocol; any
other reject stops the run, exit 6, with cause `rejected_without_rework_path` (`:494-500`). Rework
is capped at 2 per approval. The third rejection stops the run with cause `rework_spent` (`:551-555`).

| # | Protocol / action | Asked in A5 | Reject 1 | Reject 2 | Reject 3 | Verdict | Branch |
|---|---|---|---|---|---|---|---|
| 1 | direction `context.approve` (`owner.approve`, `protocol/sdc-direction.protocol.yaml:204-215`) | decider (class `context`; `decider:requester` with `SDC_REQUESTER_DECIDER`, `run-trial.sh:388-392`) | the rejection is recorded and `context.default` reworks it (`retry_when` `context_approval`, `driver/examples/sdc/sdc-direction.bindings.json:26-33`) | rework | stop `rework_spent` (`run-trial.sh:551-555`) | has route for 1-2. Reject 3: missing route (R5) | — |
| 2 | direction `vision.approve` (`:232-244`) | decider (class `vision`) | `vision.write` reworks (`bindings:82-86`) | rework | stop `rework_spent` | has route for 1-2. Reject 3: missing route (R5) | — |
| 3 | direction `roadmap.approve` (`:272-286`) | decider (class `roadmap`) | `roadmap.write` reworks (`bindings:114-121`); before R6 that rework shared the writer's cap with `roadmap.validate` retries | rework | stop `rework_spent` | has route for 1-2. Reject 3: missing route (R5) | this branch (R6) |
| 4 | intake `scope.answer`, one open scope-changing question at a time (`run-trial.sh:602-619`) | decider (class `scope_changing_answer`) | before R1: `reject` was a valid reply (`decider.sh:87`), so exit 6 `rejected_without_rework_path`. Now the check refuses it (`decider.sh:89`), the decider is asked again with the reason, and after 2 retries the default is taken (`run-trial.sh:443-448`) | same | same | missing route, fixed (R1) | this branch |
| 5 | intake `scope.answer` with no open question but unattributed scope-changing answers (`check-questions` exit 3, `sdc-tool.sh:116-119`), decided as an approval (`run-trial.sh:680-689`) | decider | the rejection cannot be recorded: the binding has no `results` (`sdc-intake.bindings.json:190-197`, `driver/src/main.rs:1688`), and `withhold` has no end for it, so exit 6 `rejected_without_rework_path` | — | — | missing route, fixed (R2) | — |
| 6 | `amend.upstream` finding decision (direction, intake, spec, plan, story) (`run-trial.sh:626-646`) | decider (class `finding`) | `reject` is refused by the check (`decider.sh:88`), asked again, then the default; an escalate takes `best`, else the default (`run-trial.sh:468-472`) | same | same | has route | — |
| 7 | `amend.upstream`: the program check refuses the decider's decision (`refused_decision`) | decider, once | stop `decision_not_takeable`, exit 7 (`run-trial.sh:634-637`, `:349-357`) | — | — | missing route, fixed (R3) | `fix/attempt5-intake` refuses only what the decision wrote |
| 8 | story `merge.approve` (`change.merge`, `protocol/sdc-story.protocol.yaml:197-212`) | decider (class `merge`) | `change.implement` reworks (`sdc-story.bindings.json:50-60`). Before R6 the rework shared the writer's cap with test retries (`runs/trial-3-haiku-4`) | rework | stop `rework_spent` | has route for 1-2. Reject 3: missing route (R4) | this branch (R6) |
| 9 | story `change.review` (`:182`) | model reviewer `llm-review` (on `main`: the stub `review-change`, `sdc-tool.sh:295`) | `change.implement` reworks (`retry_when` `code_review`) | rework, while `change.implement` has runs left | stop `writer_spent` on `change.implement`, a cap shared with test retries (Table 2, R7) | has route; the cap is R7 | `impl/review-node` |
| 10 | spec `spec.review` (`protocol/sdc-spec.protocol.yaml:300-311`) | the stub reviewer `review-spec` (`sdc-tool.sh:294`); class `spec_acceptance` | before R8: no writer had `retry_when` `spec_review`, so nothing was useful and the run ended `stuck` (`main.rs:1233`). Now `spec.author` reworks (`sdc-spec.bindings.json:44-63`) | rework within `spec.author`'s cap | `writer_spent` | missing route, fixed (R8) | this branch |
| 11 | release `release.publish` (`release.approve`, `protocol/sdc-release.protocol.yaml:150-159`) | decider (class `release`) | `rework` cannot record a rejection, so `withhold` runs: the capability is denied, `explicitly_withheld` is recorded by `rule:autopilot`, the release closes `withheld`, the epic `not_released`, the root `not_delivered` (`run-trial.sh:578-596`, `:720-723`) | — (the run ends) | — | has route (an end by rule) | `fix/attempt5-close`: the root is closed by `rule:autopilot` |
| 12 | root `close` through `operating` (`protocol/sdc.protocol.yaml:143`) | on `main`: decider (class `close`, `run-trial.sh:724-729`); a reject is exit 6 (`runs/trial-3-haiku-5`) | after the merge: closed by `rule:autopilot`; the decider is not asked | — | — | has route | `fix/attempt5-close` |
| 13 | close of a child case | nobody: the driver closes a child that has one legitimate outcome (`main.rs:1563`) | — | — | — | has route (nothing to reject) | — |
| 14 | decisions `explicitly_withheld` (release `:200`), `explicitly_abandoned` (root `:151`, epic `:205`, spec `:332`), `explicitly_dropped` (story `:255`) | only `explicitly_withheld` is taken, by rule after `release.approve` is denied (`main.rs:1795`); nobody takes the others in A5 | — | — | — | not a decision point; R4 and R5 use these as ends | — |
| 15 | decider fallback after 3 invalid replies, or an escalate with `best: reject` (`run-trial.sh:449-458`, `:473-477`) | rule | `rejected_on_checks` or `reject` take the route of the row's own reject (rows 1-3, 8, 11) | | | as its row | — |
| 16 | `release.rollback`, `context.change`, `spec.reconstruct`, `spec.reuse` | not bound in the bindings, so never offered | — | — | — | not applicable | — |

## Table 2: writers and their caps

The driver counts a writer's runs since the last arrival (`main.rs:564-583`, `:698-730`). Runs that
end in a program error or `not_called` do not count. The default cap is 8 (`:712`); since R7 it holds per retry kind (`:698-730`). After the cap
the run stops `writer_spent`, exit 4 (`:1220-1229`), which autopilot records as a stop (`run-trial.sh:316`). The driver passes
the failed check records of `retry_when` in `SDC_FEEDBACK` (`main.rs:1391-1424`). `llm-node.sh`
turns them into `in/feedback.md` and `in/rejection.json` (`bindings/tools/llm-node.sh:400-436`).
Before R9, when a writer's own apply step failed (`llm-node.sh:475-530`, exit 1, nothing recorded), the
failure was not fed back, and the writer, which has no `may_produce`, was run again blind (`main.rs:680`).
Now the failure reaches the next run (`llm-node.sh:412-426`, `:538`).

| Node | Protocol | Class | Cap | Sent back by | After the cap | Verdict | Branch |
|---|---|---|---|---|---|---|---|
| `context.default` | direction | LLM writer | 3 (`sdc-direction.bindings.json:34`) | `context_defaults` invalid (check log), `context_approval` rejected | `writer_spent` | missing route: the reworks used up the cap (fixed, R6); its own apply error was not fed back (fixed, R9) | this branch |
| `vision.write` | direction | LLM writer | 3 (`:80`) | `vision_approval` rejected | `writer_spent` | its 2 reworks fit the cap; own apply error was not fed back (fixed, R9) | — |
| `roadmap.write` | direction | LLM writer | 3 (`:122`) | `roadmap_validation` invalid, `roadmap_approval` rejected | `writer_spent` | missing route: shared cap (fixed, R6); own apply error (fixed, R9) | this branch |
| `idea.dissect` | intake | LLM writer | 3 (`sdc-intake.bindings.json:43`) | `dissection_shape` invalid | `writer_spent` | stop is right (one check, its log fed back); own apply error fixed (R9) | — |
| `questions.write` | intake | LLM writer | 3 (`:82`) | `questions_shape` invalid; `not_called` while no check found it invalid (`llm-node.sh:52`) | `writer_spent` | stop is right; own apply error fixed (R9) | — |
| `questions.answer` | intake | LLM writer | 3 (`:100`) | no `retry_when`; it runs again until a run exits 0 at this snapshot (`main.rs:680`). The refusal "changed a scope-changing question" (`llm-node.sh:522-524`) is not fed back | `writer_spent` after 3 blind runs | missing route, fixed (R9): the refusal reaches the next run as `in/feedback.md` | — |
| `requirements.normalise` | intake | LLM writer | 3 (`:156`) | `requirements_shape` invalid | `writer_spent` | stop is right: since `fix/attempt5-intake` the check names each citation fault; own apply error fixed (R9) | `fix/attempt5-intake` |
| `plan.decompose` | plan | LLM writer | 3 (`sdc-plan.bindings.json:55`) | 4 kinds: `plan_validation`, `decomposition_coverage`, `stories_scoped`, `story_states` (`:41-54`) | `writer_spent` | missing route: 4 different checks shared 3 runs (fixed, R7: 3 per kind, 8 in all); R9 fixed | — |
| `spec.author` | spec | LLM writer | 4 (`sdc-spec.bindings.json:64`) | 6 kinds (`:44-63`, `spec_review` added by R8); findings through `in/findings.yaml` | `writer_spent` (`runs/trial-3-haiku-2`: each run was sent back by a new finding; the cycles `spec-own-default` and `spec-duplicate` cover that cause) | missing route: different checks shared the cap (fixed, R7: 4 per kind, 8 in all); R9 fixed | this branch (R8) |
| `change.implement` | story | LLM writer | 3 (`sdc-story.bindings.json:61`) | `conformance_run` failed, `code_review` rejected, `merge_approval` rejected (`:50-60`) | `writer_spent` (`runs/trial-3-haiku-4`) | missing route: merge reworks shared the cap (fixed, R6); test failures and review rejections shared it (fixed, R7); R9 fixed | this branch, `impl/review-node` |
| `change.review` | story | LLM review, an observation (`effect: read`) | binding `max_runs` 3 with no effect: the driver caps writers only (`main.rs:660`, `:698`); 2 retries inside `llm-node.sh` (`SDC_REVIEW_RETRIES`) | the check's reason goes into the retry prompt | 3 invalid replies: exit 2, nothing recorded, the run ends `stuck` (cycle `review-invalid` on `impl/review-node`) | missing route: the stop is `stuck` with no named cause (pending `review-invalid-named`). Proposed: drop `max_runs` from the binding (one line); the driver's `stuck` names the observation that exited without a record (`review_invalid`) | `impl/review-node` |
| `amend.upstream` (5 protocols) | all | tool writer | 12 | none; deterministic program | `writer_spent` | stop is right (the same program on the same input) | — |
| `scope.answer` | intake | manual tool | 8 (`sdc-intake.bindings.json:197`) | taken only through `approve` | — | not applicable | — |
| `intake.open`, `direction.open`, `epics.open`, `spec.open`, `plan.open`, `stories.open`, `release.open` | root, epic | driver action | 8 (default) | none | `writer_spent` | stop is right (deterministic) | — |
| `release.build`, `change.merge` | release, story | tool writer | 8 (default) | none; a failure reruns the same program | `writer_spent` | stop is right (needs a code fix) | — |
| decider, requester decider | `run-trial.sh` | retry rule | 2 retries with the check's reason (`run-trial.sh:416-437`) | invalid reply | fallback, no stop (`:440-460`) | has route | — |

## Routes (D145)

| Route | Step | Today | Route | Cycle | State |
|---|---|---|---|---|---|
| R1 | intake `scope.answer`, a question rejected | exit 6 `rejected_without_rework_path` | `reject` is refused for a question, as for a finding: in the schema (`driver/src/reply/schemas.rs`, `decider_schema`), in the check (`decider.sh:89`) and in the prompt (`decider.sh:437`). The decider is asked again, then the default is taken | `decider-question-reject` | fixed |
| R2 | intake `scope.answer` decided as an approval, rejected | exit 6 | each unattributed scope-changing answer is put to the decider as a question (as R1) | `scope-unattributed-reject` | built (`fix/attempt5-close`): `answer_questions` puts each unattributed answer to the decider, the approval of `scope.answer` is then taken by its principal |
| R3 | finding decision refused by the program check | exit 7 `decision_not_takeable` | the decider is asked again with the refusal reason, at most 2 times, then the stop `decision_not_takeable` | `finding-refused-reask` | built (`fix/attempt5-close`): `decide_findings` asks again under autopilot (below A5 a person decides, as before) with `refusal` in the request, counted by the finding's decisions in `decisions.jsonl`; then run.unstick (when on), keep-going, or the stop |
| R4 | third reject of `merge.approve` | `rework_spent` | the story ends `dropped` (`explicitly_dropped`, `sdc-story.protocol.yaml:255`) by `rule:autopilot` with the reasons. The epic then ends `not_delivered`, not `not_released`, because a child before the release ended (`sdc-epic.protocol.yaml:6`, `:209`), and the root ends `not_delivered`; the run closes. No protocol change. The `decide` guard needs the capability denied directly before (`main.rs:1795`), but `change.merge` is already granted by the recorded rejections, so the guard has to accept a rejected approval too | `rework-spent-dropped` | pending (`fix/attempt5-close` agent) |
| R5 | third reject of `context.approve`, `vision.approve` or `roadmap.approve` | `rework_spent` | the root ends `abandoned` (`explicitly_abandoned`, `sdc.protocol.yaml:151`) recorded by `rule:autopilot` with the reasons. Ending the direction case itself needs a protocol change, which was not made: `sdc.direction` has one outcome, `passed` (`sdc-direction.protocol.yaml:301`); one more outcome, `abandoned` on `explicitly_abandoned`, would be needed. The state count was not determined. Without that change the direction child stays open under an abandoned root | `rework-spent-abandoned` | pending (`fix/attempt5-close` agent) |
| R6 | a writer's cap used up before a rework | `writer_spent` | a rejection recorded by a principal is an arrival (`main.rs:564-583`); the rework count still caps reworks at 2 | `rework-fresh-allowance` | fixed |
| R7 | one cap shared by different checks (`spec.author`, `plan.decompose`, `change.implement`) | `writer_spent` | runs are counted per retry kind, each with its existing cap, plus a backstop over all kinds of `max(8, max_runs)` runs per arrival, so the 12 runs of `amend.upstream` stay allowed (`main.rs` `useful`). The stop `writer_spent` names the kind that ran out (`kind` in the spent entry and the message) | `implement-percause` | fixed |
| R8 | `spec.review` rejected | `stuck` | `spec.author` gets `retry_when` `spec_review: [rejected]`; the stub reviewer rejects a tree that holds `.review-reject` (`sdc-tool.sh:294`) | `spec-review-rework` | fixed |
| R9 | a writer's own apply error | rerun blind, then `writer_spent` | the failed run's own log is fed back as `in/feedback.md` (`llm-node.sh`, which also prints why its output was not applied into the step log). A rejection whose log is not an `sdc.rejection/1` document, such as a stub reviewer's, now reaches the writer in `in/feedback.md` too | `writer-own-error` | fixed |
| R13 | a failed program check sent back to its writer (runs/trial-3-haiku-8) | the step log was the check's stdout followed by its stderr, so it ended with the build pool's `slot: job … ran 1s` line. `log_line` reported that line in `writer_spent`, and `impl-gate` showed no panic message | each command in `conform-run` writes its stderr with its stdout and the `slot: job` lines are dropped. A failed command is named (`conform-run: FAILED \`task check\` in impl (exit 101)`). `story-tests-run` ends with each failed step's last 30 lines under a heading that names the step. `impl-gate` shows a panic with its message. `in/feedback.md` holds the last 80 lines without `slot: job` lines and the check's exit. A check that wrote nothing is said so in words. `log_line` skips `slot: job` lines | `writer-sees-test-failure` | fixed |
| R14 | `change.implement`: a requirement of the story not built, or cited at a line or test the tree lacks (`story:implement-cites-requirements`) | the review sent the build back after `tests.run` (baseline-0.1.0: 4 of 10 review rejections) | the binding's `postcondition` (`sdc-reply check-coverage`) refuses the revision before `tests.run`, naming the ids; the refusal reaches the next run through `SDC_FEEDBACK` as a failed check does (R13), under its own retry kind `requirement_coverage` against `max_runs` (R7); no protocol rule (`docs/driver.md`, "Requirement coverage") | `coverage-not-built`, `coverage-spent` | built |
| — | `change.review`: 3 invalid replies | `stuck` | a stop with a named cause that names `change.review` | `review-invalid-named` | pending (needs `impl/review-node`) |

### What a failed check hands its writer (R13)

These are the program checks whose result is named in a `retry_when`:

- Wrapper line last, fixed: `tests.run` (`story-tests`, `conformance_run`).
- A verdict or count read from stdout and stderr mixed, fixed. `impl-gate`'s test count is now
  read from stdout only. `decider.sh`'s grounding verdict is read from stdout only. Output that
  cannot be read exits 126, a tool error, and is never sent back to the writer
  (`driver/tests/impl_gate.rs`).
- The check's own output, with no wrapper: `spec.validate`, `spec.compile`, `spec.synthesize`,
  `spec.scan`, `roadmap.validate` and `plan.validate` (ess, aep or grep output).
- No output of their own when they fail:
  - `dissection.check`;
  - `questions.check` (exits 1 and 3; exit 2 names missing keys only);
  - the shape step of `requirements.check`, `context.check`, `spec.trace` and `plan.cover`;
  - `plan.scope` and `plan.status`, which print only `false`;
  - the stub reviewers `spec.review` and `change.review`.

  Their writer now reads "The check wrote no output of its own". Naming the failed condition in
  each of these checks is not done.

## Pending cycles

`scripts/run-finding-cycles.sh` lists these in `pending` and prints them as `pending: N (K still
failing: …)`. They never fail the script, and each moves to `expected` when its route is built:
`scope-unattributed-reject`, `finding-refused-reask`, `rework-spent-dropped`,
`rework-spent-abandoned`, `review-invalid-named`. `writer-own-error` and `implement-percause` were
pending until R9 and R7 were built.

## Not determined

- Whether row 5 (unattributed scope-changing answers) can occur in a real run. The answering agent
  may not touch scope-changing questions (`llm-node.sh:522-524`), so it needs `questions.write`, or
  an amendment, to write one that is already answered. *Inferred*: rare.
- Whether a third rejection carries the same reasons as the first two (that would make the stop the
  right end) or different ones. The trial runs did not reach a third rejection.

## The check (D148)

`sdc-check-routes` (`driver/src/bin/sdc-check-routes.rs`) turns this audit into a program. It reads:

- the compiled IR of every protocol (`protocol/compiled/*.ir.json`);
- every bindings file (`results`, `retry_when`, `max_runs`, `manual`, `llm`, `argv`);
- `decision-classes.json`;
- the declared rule ends, `driver/examples/sdc/rule-ends.json`.

It prints one line for each state where nothing points to a next step. `scripts/check-bindings.sh`
runs it with `--doc docs/reject-routes.md`, and fails unless its findings equal the block under
"Check output" line by line. A fix therefore removes its line from the block, and a new dead end
adds one, in the same commit.

`rule-ends.json` holds the autopilot rules that lived only in code. `run-trial.sh` reads two of
them: the rework limit (`rework_limit`, which was the literal 2; the default of the one effective rework cap, D184: `SDC_MAX_REJECTIONS` overrides it, run-settings.jsonl records `rework_limit: {value, source}`, and the driver and the writer's rework prompt in `llm-node.sh` read that value) and the ends for a refusal (`approvals.*.reject`, which was
the `refusal_end` array). The check reads all of them. The `runtime` block states facts of a run of `runbook run`: which
effects are capped (`capped_effects`), whether the cap holds per kind (`cap_per_kind`, R7),
whether a rejection is an arrival (`rejection_is_arrival`, R6), and whether the writer's own
failure (`own_failure_fed_back`) and a rejection log (`rejection_logs_fed_back`, R9) are fed back.
Its `provided_by` object says who holds each member: `loom: <what>, <file:line>`,
`runner: <what>` or `absent: <what>`. Loom's default of 8 runs per action with no
`budget.max_runs` is Loom's (`crates/loom-bindings/src/lib.rs:84` at `70b0e6e0`) and no member of
the file. A route built in code is declared there as well. The other routes stay `null`
until they are built: R2 (`approvals."sdc.intake/scope.answer"`), R3 (`findings.refused_by_check`),
R4 and R5 (`approvals.*.after_limit`), and the root close (`close."close:operating"`, which
`fix/attempt5-close` sets).

The check refuses a `runtime` block it cannot read as a whole: it exits 2, prints nothing on
standard output and names every reason on standard error, one line each. It refuses:

- no block `runtime` (the old key `driver` is named as renamed) or no `provided_by` in it;
- a member it does not know, a member with no `provided_by` entry, an entry for no member;
- a member of the wrong type: `capped_effects` is a list of `read`, `write`, `external`; every
  other member is `true` or `false`;
- a `provided_by` value with no known prefix, with nothing after the prefix, or a `loom:` value
  with no `file:line` (a relative path ending in `rs`, `toml`, `md`, `yaml`, `yml`, `json` or
  `sh`, a colon and a line or range of lines: `crates/x/src/y.rs:740`, `retry.rs:4-8`; a web
  address or a host name with a port is none). This checks the form only: it does not open the
  file at the pinned commit;
- a member that contradicts its entry: `true` (a non-empty `capped_effects`) next to `absent:`, or
  `false` (an empty list) next to `loom:` or `runner:`. No member can rightly be `false` next to
  `loom:` or `runner:`: `false` says the run has not got it, and then the entry says `absent:` and
  names why.

`scripts/check-bindings.sh` tells the two ends apart: exit 1 for findings (it goes on and
reports), exit 2 of the check is a refused file (it prints `REFUSED routes`, stops and exits 3).

| Finding | When |
|---|---|
| `negative-result` | a check result that no claim accepts, which no `retry_when` names and no rule end covers, after which no outcome and no write action becomes possible |
| `stale-retry` | a `retry_when` kind that no bound action of the protocol produces |
| `not-handed` | a `retry_when` result whose producer's log does not reach the writer |
| `own-failure` | an LLM writer whose failed run is not fed back |
| `shared-cap` | different retry kinds, or reworks, that share one `max_runs` |
| `inert-cap` | a `max_runs` on a node the runtime does not cap (`change.review` on `impl/review-node`) |
| `unrecorded-exit` | an LLM observation with an exit its `results` do not map: no record, so the case is stuck (`change.review` exit 2 on `impl/review-node`) |
| `rework-limit` | an approval with no end after its last rework |
| `no-reject-route` | a decision class whose reject has no route |

After `impl/review-node` merges, the check will report `inert-cap` and `unrecorded-exit` for
`change.review`. The proposed fix is to drop `max_runs` from that binding (one line) and to map exit
2 to a stop with a named cause (pending cycle `review-invalid-named`).

## Check output

This block is the check's output on this branch: 36 findings (13 `own-failure`, 8 `shared-cap`,
6 `negative-result`, 4 `rework-limit`, 3 `no-reject-route`, 1 each of `not-handed`
and `unrecorded-exit`). `fix/attempt5-close` removes the line `no-reject-route sdc/close:operating
(close)`.

```text
shared-cap sdc.bugfix change.fix: 2 retry kinds share max_runs 3
own-failure sdc.bugfix bug.dissect: a failed run's log does not reach its next run
own-failure sdc.bugfix change.fix: a failed run's log does not reach its next run
own-failure sdc.bugfix test.reproduce: a failed run's log does not reach its next run
not-handed sdc.direction amend.upstream: retry_when context_defaults=finding, but the log of context.check does not reach it
shared-cap sdc.direction context.default: reworks after a rejected context_approval count against max_runs 3
shared-cap sdc.direction roadmap.write: reworks after a rejected roadmap_approval count against max_runs 3
shared-cap sdc.direction vision.write: reworks after a rejected vision_approval count against max_runs 3
own-failure sdc.direction context.default: a failed run's log does not reach its next run
own-failure sdc.direction roadmap.write: a failed run's log does not reach its next run
own-failure sdc.direction vision.write: a failed run's log does not reach its next run
negative-result sdc.epic spec.gate ess_gate=failed: no writer's retry_when names it, no rule end covers it, no outcome or write follows
own-failure sdc.intake idea.dissect: a failed run's log does not reach its next run
own-failure sdc.intake questions.answer: a failed run's log does not reach its next run
own-failure sdc.intake questions.write: a failed run's log does not reach its next run
own-failure sdc.intake requirements.normalise: a failed run's log does not reach its next run
negative-result sdc.plan spec.gate ess_gate=failed: no writer's retry_when names it, no rule end covers it, no outcome or write follows
shared-cap sdc.plan plan.decompose: 4 retry kinds share max_runs 3
own-failure sdc.plan plan.decompose: a failed run's log does not reach its next run
negative-result sdc.release acceptance.run release_conformance=failed: no writer's retry_when names it, no rule end covers it, no outcome or write follows
negative-result sdc.release release.observe health_observation=unhealthy: no writer's retry_when names it, no rule end covers it, no outcome or write follows
negative-result sdc.release spec.gate ess_gate=failed: no writer's retry_when names it, no rule end covers it, no outcome or write follows
shared-cap sdc.spec spec.author: 6 retry kinds share max_runs 4
own-failure sdc.spec spec.author: a failed run's log does not reach its next run
negative-result sdc.story spec.gate ess_gate=failed: no writer's retry_when names it, no rule end covers it, no outcome or write follows
shared-cap sdc.story change.implement: 3 retry kinds share max_runs 3
shared-cap sdc.story change.implement: reworks after a rejected merge_approval count against max_runs 3
own-failure sdc.story change.implement: a failed run's log does not reach its next run
unrecorded-exit sdc.story change.review: an exit its results do not map records nothing, and the case ends stuck
no-reject-route sdc/close:operating (close): the decider is asked and its reject has no route
rework-limit sdc.direction/context.approve (context): rejection 3 ends in the stop rework_spent, no end by rule
rework-limit sdc.story/merge.approve (merge): rejection 3 ends in the stop rework_spent, no end by rule
rework-limit sdc.direction/roadmap.approve (roadmap): rejection 3 ends in the stop rework_spent, no end by rule
no-reject-route sdc.intake/scope.answer (scope_changing_answer): a reject is not recordable and no rule end covers it
rework-limit sdc.direction/vision.approve (vision): rejection 3 ends in the stop rework_spent, no end by rule
no-reject-route amend.upstream (finding): a decision the program check refuses ends in the stop decision_not_takeable
```

## Found by the check beyond Tables 1 and 2

The four `negative-result` lines about `ess_gate=failed` and the two about the release are not
reject points and not caps, so Tables 1 and 2 do not list them. They are dead ends all the same:
nothing is admissible after them and no outcome is legitimate. No coordinator ruling yet and no
cycle; the routes below are only proposals.

| # | State | Proposed route |
|---|---|---|
| R10 | `spec.gate` fails after the spec stage, in the epic, plan, story or release (`protocol/sdc-epic.protocol.yaml:125`, `sdc-plan.protocol.yaml:147`, `sdc-story.protocol.yaml:150`, `sdc-release.protocol.yaml:121`). The gate already passed on the same specification revision, so this needs a changed toolchain. *Inferred*: rare | an end by rule (story `dropped`, epic `abandoned`). The plan has no outcome for it, so a protocol change would be needed there |
| R11 | `acceptance.run` fails (`sdc-release.protocol.yaml:138`) | the release ends `withheld` by rule, the same end as a refused `release.publish` (row 11) |
| R12 | `release.observe` reports `unhealthy` (`:162`). `release.rollback` (`:174`) is not bound | bind `release.rollback`; the outcome `rolled_back` already exists |

## Visible to Canon's `simulate`?

This is a list for a later decision; no protocol was changed for it. All state costs are
*inferred*: they count the extra claim values per case and were not computed.

| Route | As a protocol fact | Visible to `simulate` | Cost in states |
|---|---|---|---|
| R2 | a `rejected` result of a `scope_answer` evidence kind on `scope.answer` | yes: a rejected state with no admissible action | ×3 for the intake (one more claim with 3 values) |
| R3 | the amendment check as evidence (`amendment_check` refused) with a refusal count | yes, if the count is a claim | ×4 (count 0-3) |
| R4 | a rejection count per approval as a claim (`merge.rejections` 0-3) | yes: after 3, no admissible action and no outcome | ×4 for the story |
| R5 | the same for `context.approve`, `vision.approve` and `roadmap.approve` | yes | ×4 per approval, ×64 for the direction |
| R7 | a run count per writer and kind as a claim | yes | ×(cap+1) per kind: `spec.author` ≈ 5^6 ≈ 15 600, too costly |
| R9 | none: it lives inside the writer | no | — |
| R10-R12 | already protocol facts | yes, with no change | 0 |
| root close (`fix/attempt5-close`) | a capability on the close | yes | ×2 |
| `change.review` exit 2 | a `code_review` result `invalid` with no route | yes | ×1 (one more result value) |
