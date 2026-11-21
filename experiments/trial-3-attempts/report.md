# trial-3-attempts: Trial 3, attempts 1 to 5 before the batch tree

Written by `sdc-experiment report trial-3-attempts` from `experiments/trial-3-attempts.yaml` and the run summaries in `experiments/trial-3-attempts/runs/`; `sdc-experiment check --all` fails when it is stale. Definitions: `docs/workflow-metrics.md`.

- Kind: retrospective; status: complete; created 2026-11-21.
- Purpose: Run the to-do idea from idea to close with no person, all Haiku beside Sonnet work with a Haiku decider, and fix what stops each attempt before the next.
- Hypothesis: none; the record describes its runs.
- Input `todo-app` (idea, sha256 f384a752a2cb): A to-do list web app for me and my partner that also works on the train without a connection
- Tree: `several`
  - attempt 1: trial/3 at cedd691 (worktree sdc-trial-3): trial-3-haiku, trial-3-mixed
  - attempt 2: trial/3 at f426876 (merge of fix/decider-citations f74c68b): trial-3-haiku-2, trial-3-mixed-2
  - attempt 3: trial/3 at 4b8e177 (merges 9df3062 = 5471dab and 4b8e177 = 37fba0b): trial-3-haiku-3
  - attempt 4: trial/3b at main 753210d (worktree sdc-trial-3b): trial-3-haiku-4, trial-3-mixed-3
  - attempt 5: trial/3b at ea2f111 (main 32265be + fix/attempt4-defects 073c173): trial-3-haiku-5, trial-3-mixed-4; haiku-5 reopened on acd6f12, mixed-4 reopened on 3663770
- Runs per variant: 5; pass rule: clean-delivery (closed, delivered, no reopen, forced step or unstick).
- Decision rule (descriptive): No hypothesis. Each attempt ran on a tree with the previous attempt's fixes, so the runs are a sequence of defect finds, not samples of one setting.

## Settings

| setting | haiku | mixed |
|---|---|---|
| `adapter` | metaharness | metaharness |
| `autonomy` | A5 | A5 |
| `decider.escalate` | final | final |
| `decider.kind` | haiku | haiku |
| `decider.max_decider_calls` | 20 | 20 |
| `decider.model` | claude-haiku-5-5 | claude-haiku-5-5 |
| `max_llm_calls` | 90 | 90 |
| `max_wall_seconds` | 5400 | 5400 |
| `models.implement.model` | claude-haiku-5-5 | claude-sonnet-5-5 |
| `models.other.model` | claude-haiku-5-5 | claude-haiku-5-5 |
| `models.review.model` | null | claude-sonnet-5-5 |
| `models.writer.model` | claude-haiku-5-5 | claude-sonnet-5-5 |

Known confounds:

- `tree`: every attempt ran on a different tree (see trees); the review step was a program stub on all of them
- `decider.adapter`: attempts 2 to 5 set SDC_DECIDER_ADAPTER=claude-cli; attempt 1 did not; run-settings.jsonl does not record it

## Runs

| run | variant | tree | end | clean pass | cost USD | wall s | looping factor | note |
|---|---|---|---|---|---|---|---|---|
| trial-3-haiku | haiku | cedd691 | stopped: awaiting_authority in IDEA-1.direction | no | 0.026 | 91 | 1 |  |
| trial-3-haiku-2 | haiku | f426876 | stopped: writer_spent in IDEA-1.list.spec | no | 0.211 | 786 | 1.75 |  |
| trial-3-haiku-3 | haiku | 4b8e177 | stopped: rejected_without_rework_path in IDEA-1.app.release | no | 0.469 | 1609 | 1.545 |  |
| trial-3-haiku-4 | haiku | 753210d | stopped: writer_spent in IDEA-1.todo.list | no | 0.669 | 2107 | 2 |  |
| trial-3-haiku-5 | haiku | ea2f111 | closed: operating | no | 0.779 | 3106 | 1.636 | reopened once on acd6f12; a reopened run is no end-to-end pass |
| trial-3-mixed | mixed | cedd691 | stopped: awaiting_authority in IDEA-1.direction | no | 0.027 | 94 | 1.2 |  |
| trial-3-mixed-2 | mixed | f426876 | stopped: awaiting_authority in IDEA-1.list.items | no | 0.886 | 888 | 1.273 |  |
| trial-3-mixed-3 | mixed | 753210d | closed: not_delivered | no | 1.487 | 2096 | 1.417 |  |
| trial-3-mixed-4 | mixed | ea2f111 | stopped: decision_not_takeable in IDEA-1.intake | no | 0.042 | 3036 | 1 | reopened once on 3663770 and stopped again at once |

## Per variant

Median [min, max] (n) over the ended runs.

| | haiku | mixed |
|---|---|---|
| runs listed | 5 | 4 |
| ended | 5 | 4 |
| clean passes | 0 | 0 |
| `run.cost_usd` | 0.4686 [0.0264, 0.779] (5) | 0.4643 [0.0269, 1.487] (4) |
| `run.llm_calls` | 13 [5, 18] (5) | 9 [4, 16] (4) |
| `run.decider_calls` | 11 [4, 17] (5) | 7 [3, 13] (4) |
| `run.wall_s` | 1609 [91, 3106] (5) | 1492 [94, 3036] (4) |
| `run.driver_s` | 779 [65, 1215] (5) | 248 [55, 469] (4) |
| `run.real_takes` | 104 [29, 126] (5) | 60 [24, 107] (4) |
| `run.rework_takes` | 54 [13, 64] (5) | 25 [13, 40] (4) |
| `run.looping_factor` | 1.636 [1, 2] (5) | 1.2365 [1, 1.417] (4) |
| `run.first_pass_yield` | 0.727 [0.636, 1] (5) | 0.775 [0.727, 1] (4) |
| `run.rework_cost_usd` | 0.0705 [0, 0.2742] (5) | 0.0479 [0, 0.325] (4) |
| `run.furthest_stage_index` | 6 [2, 7] (5) | 4 [1, 7] (4) |
| `run.findings_raised` | 15 [2, 17] (5) | 3 [1, 5] (4) |
| `run.waiting_on_person_s` | 0 [0, 553] (5) | 0 [0, 2870] (4) |
| `run.clean` | 0 [0, 0] (5) | 0 [0, 1] (4) |

## Loops

Per variant, over the ended runs: of the writer and approval nodes taken again at least once, the three with the highest median looping factor; and the three edges with the highest loop cost.

- haiku (n 5):
  - `spec/spec.author` (llm-writer): looping factor median 4 (max 7, n 4), rework takes 13, rework cost 0.116 USD
  - `story/change.implement` (llm-writer): looping factor median 3.5 (max 4.5, n 3), rework takes 13, rework cost 0.465 USD
  - `direction/vision.write` (llm-writer): looping factor median 1.5 (max 3, n 4), rework takes 3, rework cost 0.009 USD
  - `story: merge.approve:rejected -> change.implement` (approval): count 3 in 2 runs, loop cost 0.524 USD, loop driver 558 s
  - `story: finding@story -> intake` (finding): count 23 in 3 runs, loop cost 0.193 USD, loop driver 334 s
  - `spec: finding@spec -> intake` (finding): count 21 in 4 runs, loop cost 0.09 USD, loop driver 292 s
- mixed (n 4):
  - `direction/vision.write` (llm-writer): looping factor median 2 (max 3, n 2), rework takes 2, rework cost 0.006 USD
  - `story/change.implement` (llm-writer): looping factor median 1.75 (max 2, n 2), rework takes 4, rework cost 0.414 USD
  - `story/merge.approve` (approval): looping factor median 1.667 (max 1.667, n 1), rework takes 2, rework cost 0 USD
  - `story: merge.approve:rejected -> change.implement` (approval): count 2 in 1 runs, loop cost 0.434 USD, loop driver 75 s
  - `story: finding@story -> intake` (finding): count 2 in 2 runs, loop cost 0.096 USD, loop driver 99 s
  - `direction: finding@vision -> intake` (finding): count 2 in 1 runs, loop cost 0.056 USD, loop driver 37 s

## Verdict

descriptive: no hypothesis: the record describes its runs and decides nothing.

## Facts

- Attempt 1: both runs stopped awaiting_authority in IDEA-1.direction.
- trial-3-haiku-2: stopped writer_spent in IDEA-1.list.spec: spec.author ran 4 of 4 times, spec.trace exited 5 in each loop; the Haiku spec writer raised one new genuine finding per re-derivation (fix: fix/spec-trace-loop 7f5ef7c, 37fba0b). Work 0.124 USD, decider 0.087 USD.
- trial-3-mixed-2: stopped awaiting_authority in IDEA-1.list.items: the decider validly rejected change.merge and nothing sent a rejected change back to the implementer.
- trial-3-haiku-3: stopped rejected_without_rework_path at IDEA-1.app.release (release.publish rejected); replay 9 of 9 match; 0.469 USD (work 0.263, decider 0.206).
- trial-3-haiku-4: stopped writer_spent in IDEA-1.todo.list (change.implement 3 runs). merge.approve rejected todo.identity (unbounded JSON nesting depth, no header size limit or read timeout; reworked, then approved) and todo.list (a data file instead of Postgres through a client crate, against r2, q7, q17; the rework was skipped by the "no new decisions" rule, a tool-layer defect).
- trial-3-mixed-3: closed not_delivered: the decider withheld the release because health.required was open with no health route; that obligation is met only after the publish, so the refusal was a tool defect (fixed in 073c173). Its two merge rejections (offline POSTs fail because the service worker answers GET only; sync with no offline queue) were about the product and led to rework.
- trial-3-haiku-5: stopped rejected_without_rework_path at the root close (r1 cites both q10, Postgres, and q12, a data file; the roadmap check held while the artifact was a draft). Reopened on acd6f12 it closed operating by rule:autopilot with no new call. sdc-finding-kpis L7 reads 553 s for the reopen gap: a wait for a fix, not for a person.
- trial-3-mixed-4: stopped decision_not_takeable in IDEA-1.intake: the decider's valid choice for f1 was refused by the program check because r3 cites d6, a dark spot (fix: fix/attempt5-intake 102ce99). Reopened on 3663770 it stopped again at once (the fix acts at write time).
- Pass by the trial-3 report's rule: 0 of the 9 attempt runs (docs/trial-3.md section 1).

## Links

- docs/trial-3.md on branch docs/trial-3 (commit 243a22f; not on main when this record was written): sections 1, 4, 5 and 6
