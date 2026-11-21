# release-0.1.2: Release 0.1.2 on main 72a3eef, the to-do idea in the three baseline settings

Written by `sdc-experiment report release-0.1.2` from `experiments/release-0.1.2.yaml` and the run summaries in `experiments/release-0.1.2/runs/`; `sdc-experiment check --all` fails when it is stale. Definitions: `docs/workflow-metrics.md`.

- Kind: prospective; status: complete; created 2026-11-21.
- Purpose: Baseline of release 0.1.2 and the registered test of whether the reject and cap routes of 0.1.1 and 0.1.2 move the Haiku setting against baseline-0.1.0.
- Hypothesis: none; the record describes its runs.
- Input `todo-app` (idea, sha256 f384a752a2cb): A to-do list web app for me and my partner that also works on the train without a connection
- Tree: `72a3eefb20e0edff50dc8ad12fcf561f8e0790e3`
- Runs per variant: 3; pass rule: clean-delivery (closed, delivered, no reopen, forced step or unstick).
- Decision rule (descriptive): The tree is the variable, which one record cannot hold (docs/improving.md step 4), so sdc-experiment reports this record as descriptive and the comparison with baseline-0.1.0 (same input, same settings, tree 4468bfd) is made per setting with the registered predictions below and the default rule: n = 3 against n = 3, a difference is claimed only when the two ranges separate completely in the registered direction (exact one-sided p = 1/20); overlapping ranges claim no effect. Primary: the Haiku setting's run.clean, baseline 0 of 3; held only at 3 of 3 clean; 1 or 2 of 3 is inconclusive and is reported as such; 0 of 3 is no move. Every end counts; no run is repaired, reopened or forced. A refuted or inconclusive result is published as such.

## Settings

| setting | strong | mixed | haiku |
|---|---|---|---|
| `adapter` | metaharness | metaharness | metaharness |
| `autonomy` | A5 | A5 | A5 |
| `decider.adapter` | claude-cli | claude-cli | claude-cli |
| `decider.escalate` | final | final | final |
| `decider.kind` | model | model | model |
| `decider.max_decider_calls` | 40 | 40 | 40 |
| `decider.model` | claude-opus-5-5 | claude-haiku-5-5 | claude-haiku-5-5 |
| `max_llm_calls` | 150 | 150 | 150 |
| `max_wall_seconds` | 5400 | 5400 | 5400 |
| `models.implement.model` | claude-sonnet-5-5 | claude-sonnet-5-5 | claude-haiku-5-5 |
| `models.other.model` | claude-sonnet-5-5 | claude-haiku-5-5 | claude-haiku-5-5 |
| `models.review.model` | claude-sonnet-5-5 | claude-sonnet-5-5 | claude-haiku-5-5 |
| `models.writer.model` | claude-sonnet-5-5 | claude-sonnet-5-5 | claude-haiku-5-5 |
| `review.adapter` | claude-cli | claude-cli | claude-cli |

Known confounds:

- `unstick`: run.unstick (D147, 0.1.1) is on under autopilot unless SDC_UNSTICK=off, with the model claude-opus-5-5 unless SDC_LLM_MODEL_UNSTICK is set (scripts/run-trial.sh:377-385); neither is a launch setting of sdc-experiment, so every variant runs with an Opus unstick that baseline-0.1.0 did not have. An unstuck run is not a clean pass, so a stop the routes turn into an unstick counts against P1 and for P2.
- `canon`: every run uses the canon binary of the canon checkout at run time (run-trial.sh's default), which no record pins; baseline-0.1.0 had the same confound.
- `launcher`: baseline-0.1.0 was started by a scheduler script with the same settings; these runs are started by sdc-experiment run, with experiment.json.
- `decider-kind`: decider.kind is model here and haiku in baseline-0.1.0; 0.1.2 made haiku an alias of model and sdc-experiment counts them as one setting (CHANGELOG 0.1.2).

## Runs

| run | variant | end | clean pass | cost USD | wall s | looping factor | note |
|---|---|---|---|---|---|---|---|
| release-0.1.2.strong.1 | strong | closed: operating | yes | 4.695 | 972 | 1.167 |  |
| release-0.1.2.strong.2 | strong | closed: operating | yes | 4.816 | 1108 | 1.083 |  |
| release-0.1.2.strong.3 | strong | closed: operating | yes | 4.46 | 886 | 1.167 |  |
| release-0.1.2.mixed.1 | mixed | stopped: writer_spent in IDEA-1.sharedlist.list | no | 1.373 | 810 | 1.6 |  |
| release-0.1.2.mixed.2 | mixed | closed: operating | yes | 3.035 | 1577 | 1.583 |  |
| release-0.1.2.mixed.3 | mixed | closed: operating | yes | 2.523 | 1821 | 1.75 |  |
| release-0.1.2.haiku.1 | haiku | stopped: writer_spent in IDEA-1.list.sync | no | 1.724 | 3278 | 1.583 |  |
| release-0.1.2.haiku.2 | haiku | closed: operating | yes | 0.882 | 2832 | 1.818 |  |
| release-0.1.2.haiku.3 | haiku | closed: operating | yes | 0.932 | 3231 | 1.545 |  |

## Per variant

Median [min, max] (n) over the ended runs.

| | strong | mixed | haiku |
|---|---|---|---|
| runs listed | 3 | 3 | 3 |
| ended | 3 | 3 | 3 |
| clean passes | 3 | 2 | 2 |
| `run.cost_usd` | 4.6952 [4.4604, 4.8158] (3) | 2.5232 [1.3728, 3.0348] (3) | 0.9317 [0.8816, 1.7237] (3) |
| `run.llm_calls` | 18 [17, 19] (3) | 23 [16, 26] (3) | 25 [20, 25] (3) |
| `run.decider_calls` | 13 [10, 13] (3) | 12 [8, 14] (3) | 22 [20, 22] (3) |
| `run.wall_s` | 972 [886, 1108] (3) | 1577 [810, 1821] (3) | 3231 [2832, 3278] (3) |
| `run.driver_s` | 711 [592, 887] (3) | 868 [537, 1013] (3) | 2153 [1808, 2399] (3) |
| `run.real_takes` | 96 [93, 114] (3) | 111 [92, 124] (3) | 141 [139, 154] (3) |
| `run.rework_takes` | 25 [24, 44] (3) | 44 [42, 56] (3) | 77 [74, 89] (3) |
| `run.looping_factor` | 1.167 [1.083, 1.167] (3) | 1.6 [1.583, 1.75] (3) | 1.583 [1.545, 1.818] (3) |
| `run.first_pass_yield` | 0.833 [0.833, 0.917] (3) | 0.7 [0.667, 0.75] (3) | 0.667 [0.636, 0.727] (3) |
| `run.rework_cost_usd` | 0.5126 [0.4971, 0.572] (3) | 0.7165 [0.3056, 1.368] (3) | 0.3432 [0.1949, 0.7139] (3) |
| `run.furthest_stage_index` | 7 [7, 7] (3) | 7 [6, 7] (3) | 7 [6, 7] (3) |
| `run.findings_raised` | 4 [2, 5] (3) | 9 [7, 9] (3) | 24 [21, 26] (3) |
| `run.waiting_on_person_s` | 0 [0, 0] (3) | 0 [0, 0] (3) | 0 [0, 0] (3) |
| `run.clean` | 1 [1, 1] (3) | 1 [0, 1] (3) | 1 [0, 1] (3) |

## Loops

Per variant, over the ended runs: of the writer and approval nodes taken again at least once, the three with the highest median looping factor; and the three edges with the highest loop cost.

- strong (n 3):
  - `story/change.implement` (llm-writer): looping factor median 1.333 (max 1.667, n 3), rework takes 4, rework cost 0.968 USD
  - `spec/spec.author` (llm-writer): looping factor median 1 (max 2, n 3), rework takes 1, rework cost 0.132 USD
  - `story: change.review:rejected -> change.implement` (check): count 4 in 3 runs, loop cost 1.45 USD, loop driver 346 s
  - `spec: finding@spec -> intake` (finding): count 2 in 1 runs, loop cost 0.132 USD, loop driver 18 s
  - `story: finding@story -> intake` (finding): count 6 in 3 runs, loop cost 0 USD, loop driver 0 s
- mixed (n 3):
  - `direction/vision.write` (llm-writer): looping factor median 4 (max 6, n 3), rework takes 11, rework cost 0.048 USD
  - `spec/spec.author` (llm-writer): looping factor median 2 (max 3, n 3), rework takes 3, rework cost 0.118 USD
  - `story/change.implement` (llm-writer): looping factor median 1.667 (max 3, n 3), rework takes 6, rework cost 1.798 USD
  - `story: change.review:rejected -> change.implement` (check): count 2 in 2 runs, loop cost 1.008 USD, loop driver 274 s
  - `story: merge.approve:rejected -> change.implement` (approval): count 1 in 1 runs, loop cost 0.441 USD, loop driver 147 s
  - `story: finding@story -> intake` (finding): count 4 in 2 runs, loop cost 0.371 USD, loop driver 62 s
- haiku (n 3):
  - `spec/spec.author` (llm-writer): looping factor median 3 (max 4, n 3), rework takes 7, rework cost 0.098 USD
  - `direction/vision.write` (llm-writer): looping factor median 3 (max 3, n 3), rework takes 4, rework cost 0.017 USD
  - `story/change.implement` (llm-writer): looping factor median 2.333 (max 2.5, n 3), rework takes 9, rework cost 1.051 USD
  - `story: change.review:rejected -> change.implement` (check): count 7 in 3 runs, loop cost 1.052 USD, loop driver 1442 s
  - `story: finding@story -> intake` (finding): count 32 in 3 runs, loop cost 0.159 USD, loop driver 223 s
  - `spec: finding@spec -> intake` (finding): count 24 in 3 runs, loop cost 0.089 USD, loop driver 314 s

## Verdict

descriptive: no hypothesis: the record describes its runs and decides nothing.

## Facts

- Route changes since the baseline tree (CHANGELOG 0.1.1 Fixed, 0.1.2 Changed): R9 5297ef0, R7 a2f363e, R13 97024ce, R2 a919898, R3 306fde9, keep-going, run.unstick, one rework cap D184 9940eed (value 2, as before).
- Checked from the run files before this record: trial-0.1.0-haiku-1 q9 "Postgres database" was a finding of spec.author (f8) accepted with answered_by non-interactive (work/questions.yaml), not a decision of the decider; its list.shared implement was sent back by change.review twice (r1 against q9, an EditTitle divergence) and by tests.run failed once (E0308).
- trial-0.1.0-haiku-3: spec.validate refused four times (unreachable_branch, conflicting_declaration, then twice unsupported_format_version with the hint "declare `format: ess/18`"); the last spec.author wrote format ess/18 into spec/domains/list.yaml and left spec/system.yaml at ess/1 (llm/014-spec.author), so the fix was applied to the wrong file, not ignored.
- trial-0.1.0-haiku-2: run-stopped.json names f11, the decider's amendment refused by the program check (context.deployment and context.platform against r6); R3 of 0.1.1 is the route for it.

## Links

- experiments/baseline-0.1.0.yaml: the baseline this record is compared with
