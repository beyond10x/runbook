# release-0.1.2: Release 0.1.2 on main 72a3eef, the to-do idea in the three baseline settings

Written by `sdc-experiment report release-0.1.2` from `experiments/release-0.1.2.yaml` and the run summaries in `experiments/release-0.1.2/runs/`; `sdc-experiment check --all` fails when it is stale. Definitions: `docs/workflow-metrics.md`.

- Kind: prospective; status: planned; created 2026-11-21.
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

## Per variant

Median [min, max] (n) over the ended runs.

| | strong | mixed | haiku |
|---|---|---|---|
| runs listed | 0 | 0 | 0 |
| ended | 0 | 0 | 0 |
| clean passes | 0 | 0 | 0 |
| `run.cost_usd` | - | - | - |
| `run.llm_calls` | - | - | - |
| `run.decider_calls` | - | - | - |
| `run.wall_s` | - | - | - |
| `run.driver_s` | - | - | - |
| `run.real_takes` | - | - | - |
| `run.rework_takes` | - | - | - |
| `run.looping_factor` | - | - | - |
| `run.first_pass_yield` | - | - | - |
| `run.rework_cost_usd` | - | - | - |
| `run.furthest_stage_index` | - | - | - |
| `run.findings_raised` | - | - | - |
| `run.waiting_on_person_s` | - | - | - |
| `run.clean` | - | - | - |

## Loops

Per variant, over the ended runs: of the writer and approval nodes taken again at least once, the three with the highest median looping factor; and the three edges with the highest loop cost.

- strong (n 0):
  - no node taken again, no edge
- mixed (n 0):
  - no node taken again, no edge
- haiku (n 0):
  - no node taken again, no edge

## Verdict

descriptive: no hypothesis: the record describes its runs and decides nothing.

## Facts

- Route changes since the baseline tree (CHANGELOG 0.1.1 Fixed, 0.1.2 Changed): R9 5297ef0, R7 a2f363e, R13 97024ce, R2 a919898, R3 306fde9, keep-going, run.unstick, one rework cap D184 9940eed (value 2, as before).
- Checked from the run files before this record: trial-0.1.0-haiku-1 q9 "Postgres database" was a finding of spec.author (f8) accepted with answered_by non-interactive (work/questions.yaml), not a decision of the decider; its list.shared implement was sent back by change.review twice (r1 against q9, an EditTitle divergence) and by tests.run failed once (E0308).
- trial-0.1.0-haiku-3: spec.validate refused four times (unreachable_branch, conflicting_declaration, then twice unsupported_format_version with the hint "declare `format: ess/18`"); the last spec.author wrote format ess/18 into spec/domains/list.yaml and left spec/system.yaml at ess/1 (llm/014-spec.author), so the fix was applied to the wrong file, not ignored.
- trial-0.1.0-haiku-2: run-stopped.json names f11, the decider's amendment refused by the program check (context.deployment and context.platform against r6); R3 of 0.1.1 is the route for it.

## Links

- experiments/baseline-0.1.0.yaml: the baseline this record is compared with
