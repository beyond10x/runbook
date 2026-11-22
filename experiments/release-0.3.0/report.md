# release-0.3.0: Release 0.3.0 on main ae7a697, the to-do idea in the three baseline settings

Written by `sdc-experiment report release-0.3.0` from `experiments/release-0.3.0.yaml` and the run summaries in `experiments/release-0.3.0/runs/`; `sdc-experiment check --all` fails when it is stale. Definitions: `docs/workflow-metrics.md`.

- Kind: prospective; status: planned; created 2026-11-22.
- Purpose: Baseline of release 0.3.0, and the registered comparison with release-0.1.2 on the same input and settings, with the four switches added since at their defaults.
- Hypothesis: none; the record describes its runs.
- Input `todo-app` (idea, sha256 f384a752a2cb): A to-do list web app for me and my partner that also works on the train without a connection
- Tree: `ae7a697616857a2bf2744c85a33e58b0b5927dc8`
- Runs per variant: 3; pass rule: clean-delivery (closed, delivered, no reopen, forced step or unstick).
- Decision rule (descriptive): Registered sentence: on the to-do idea, release 0.3.0 delivers in at least as many runs per setting as release 0.1.2 did and run.cost_usd per setting is not higher by complete separation. The tree is the variable, which one record cannot hold (docs/improving.md step 4; a record with a hypothesis needs two variants of one tree that differ in one setting), so sdc-experiment reports this record as descriptive and the comparison with release-0.1.2 (same input, same settings, tree 72a3eef) is made per setting with the registered predictions below. Delivery: a setting meets P1 when its run.delivered count is at least release 0.1.2's (strong 3 of 3, mixed 2 of 3, haiku 2 of 3). Cost: n = 3 against n = 3, run.cost_usd is higher only when every 0.3.0 run of the setting costs more than every release-0.1.2 run of it (exact one-sided p = 1/20); overlapping ranges claim no effect. Every end counts; no run is repaired, reopened or forced. A refuted or inconclusive result is published as such.

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
| `implement_coverage` | off | off | off |
| `implement_mode` | session | session | session |
| `max_llm_calls` | 150 | 150 | 150 |
| `max_wall_seconds` | 5400 | 5400 | 5400 |
| `models.implement.model` | claude-sonnet-5-5 | claude-sonnet-5-5 | claude-haiku-5-5 |
| `models.other.model` | claude-sonnet-5-5 | claude-haiku-5-5 | claude-haiku-5-5 |
| `models.review.model` | claude-sonnet-5-5 | claude-sonnet-5-5 | claude-haiku-5-5 |
| `models.writer.model` | claude-sonnet-5-5 | claude-sonnet-5-5 | claude-haiku-5-5 |
| `review.adapter` | claude-cli | claude-cli | claude-cli |
| `review_mode` | session | session | session |
| `rule_approvals` | off | off | off |

Known confounds:

- `unstick`: run.unstick (D147, 0.1.1) is on under autopilot unless SDC_UNSTICK=off, with the model claude-opus-5-5 unless SDC_LLM_MODEL_UNSTICK is set (scripts/run-trial.sh); neither is a launch setting of sdc-experiment, so every variant runs with an Opus unstick, as in release-0.1.2. An unstuck run is not a clean pass.
- `canon`: release-0.1.2 used the canon binary of the canon checkout at run time; 0.3.0 pins Canon 0.10.0 (driver/canon-pin.json) and every script refuses another version, so the two series differ in the Canon version.
- `ess`: sdc-tool.sh runs ess 0.96.0 whatever ess is on PATH (docs/releases/0.3.0.md); release-0.1.2 used the installed ess.
- `instructions`: what a model call is told moved from code to files under instructions/ (622819b) with 40 golden prompts held byte for byte; its effect is unmeasured (experiments/releases.yaml).
- `launcher`: both series are started by sdc-experiment run, with experiment.json; the run directory and the build of the tree are made by the tool.
- `switches`: SDC_IMPLEMENT_MODE, SDC_RULE_APPROVALS, SDC_REVIEW_MODE and SDC_IMPLEMENT_COVERAGE are set in `fixed` to their defaults (session, off, session, off), so run-settings.jsonl records them and none of the four is on; no 0.3.0 run measures a switch.

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

- Tree: main at ae7a697616857a2bf2744c85a33e58b0b5927dc8, the merge of pull request 2 and the tag 0.3.0 (experiments/releases.yaml, 0.3.0: no run has used this tree).
- Comparison basis: experiments/release-0.1.2/report.md, input todo-app (sha256 f384a752a2cb), the same three variants and the same models; strong 3 of 3 clean passes, mixed 2 of 3, haiku 2 of 3; the two stops were writer_spent in IDEA-1.sharedlist.list (mixed.1) and in IDEA-1.list.sync (haiku.1).
- Not launched: this record is committed before any run.

## Links

- experiments/release-0.1.2.yaml: the baseline this record is compared with
- experiments/releases.yaml: the ledger entry of 0.3.0, whose limit says no run has used this tree
