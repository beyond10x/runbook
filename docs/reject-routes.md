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
| 7 | `amend.upstream`: the program check refuses the decider's decision (`refused_decision`) | decider, once | stop `decision_not_takeable`, exit 7 (`run-trial.sh:634-637`, `:349-357`) | — | — | missing route (R3) | `fix/attempt5-intake` refuses only what the decision wrote |
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
turns them into `in/feedback.md` and `in/rejection.json` (`driver/examples/sdc/tools/llm-node.sh:400-436`).
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
| R1 | intake `scope.answer`, a question rejected | exit 6 `rejected_without_rework_path` | `reject` is refused for a question, as for a finding: in the schema (`driver/src/bin/sdc-reply.rs:333`), in the check (`decider.sh:89`) and in the prompt (`decider.sh:437`). The decider is asked again, then the default is taken | `decider-question-reject` | fixed |
| R2 | intake `scope.answer` decided as an approval, rejected | exit 6 | each unattributed scope-changing answer is put to the decider as a question (as R1) | `scope-unattributed-reject` | built (`fix/attempt5-close`): `answer_questions` puts each unattributed answer to the decider, the approval of `scope.answer` is then taken by its principal |
| R3 | finding decision refused by the program check | exit 7 `decision_not_takeable` | the decider is asked again with the refusal reason, at most 2 times, then the stop `decision_not_takeable` | `finding-refused-reask` | pending (`fix/attempt5-close` agent) |
| R4 | third reject of `merge.approve` | `rework_spent` | the story ends `dropped` (`explicitly_dropped`, `sdc-story.protocol.yaml:255`) by `rule:autopilot` with the reasons. The epic then ends `not_delivered`, not `not_released`, because a child before the release ended (`sdc-epic.protocol.yaml:6`, `:209`), and the root ends `not_delivered`; the run closes. No protocol change. The `decide` guard needs the capability denied directly before (`main.rs:1795`), but `change.merge` is already granted by the recorded rejections, so the guard has to accept a rejected approval too | `rework-spent-dropped` | pending (`fix/attempt5-close` agent) |
| R5 | third reject of `context.approve`, `vision.approve` or `roadmap.approve` | `rework_spent` | the root ends `abandoned` (`explicitly_abandoned`, `sdc.protocol.yaml:151`) recorded by `rule:autopilot` with the reasons. Ending the direction case itself needs a protocol change, which was not made: `sdc.direction` has one outcome, `passed` (`sdc-direction.protocol.yaml:301`); one more outcome, `abandoned` on `explicitly_abandoned`, would be needed. The state count was not determined. Without that change the direction child stays open under an abandoned root | `rework-spent-abandoned` | pending (`fix/attempt5-close` agent) |
| R6 | a writer's cap used up before a rework | `writer_spent` | a rejection recorded by a principal is an arrival (`main.rs:564-583`); the rework count still caps reworks at 2 | `rework-fresh-allowance` | fixed |
| R7 | one cap shared by different checks (`spec.author`, `plan.decompose`, `change.implement`) | `writer_spent` | runs are counted per retry kind, each with its existing cap, plus a backstop over all kinds of `max(8, max_runs)` runs per arrival, so the 12 runs of `amend.upstream` stay allowed (`main.rs` `useful`). The stop `writer_spent` names the kind that ran out (`kind` in the spent entry and the message) | `implement-percause` | fixed |
| R8 | `spec.review` rejected | `stuck` | `spec.author` gets `retry_when` `spec_review: [rejected]`; the stub reviewer rejects a tree that holds `.review-reject` (`sdc-tool.sh:294`) | `spec-review-rework` | fixed |
| R9 | a writer's own apply error | rerun blind, then `writer_spent` | the failed run's own log is fed back as `in/feedback.md` (`llm-node.sh`, which also prints why its output was not applied into the step log). A rejection whose log is not an `sdc.rejection/1` document, such as a stub reviewer's, now reaches the writer in `in/feedback.md` too | `writer-own-error` | fixed |
| — | `change.review`: 3 invalid replies | `stuck` | a stop with a named cause that names `change.review` | `review-invalid-named` | pending (needs `impl/review-node`) |

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

