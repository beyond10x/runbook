# implement-cites-requirements: change.implement cites its requirements, checked by program before tests.run, on branch impr/implement-cites-requirements

Written by `sdc-experiment report implement-cites-requirements` from `experiments/implement-cites-requirements.yaml` and the run summaries in `experiments/implement-cites-requirements/runs/`; `sdc-experiment check --all` fails when it is stale. Definitions: `docs/workflow-metrics.md`.

- Kind: prospective; status: planned; created 2026-11-21.
- Purpose: Whether the coverage check of story:implement-cites-requirements lowers the review rejections per run against release 0.1.2 in the strong and mixed settings.
- Hypothesis: none; the record describes its runs.
- Input `todo-app` (idea, sha256 f384a752a2cb): A to-do list web app for me and my partner that also works on the train without a connection
- Tree: `3f3a57703ef30ebaebeb479121a31134f2eb87a7`
  - impr/implement-cites-requirements at 3f3a577 (release 0.1.2, 72a3eef, plus the coverage reply, the postcondition and the review input); the comparison tree is release 0.1.2 at 72a3eef, whose runs belong to experiments/release-0.1.2.yaml
- Runs per variant: 3; pass rule: clean-delivery (closed, delivered, no reopen, forced step or unstick).
- Decision rule (descriptive): The tree is the variable (release 0.1.2 against this branch), which one record cannot hold: sdc-experiment names one tree per record and has no tree setting, and the change has no run setting that turns it off (docs/improving.md step 4). So this record holds the treatment only and is reported as descriptive; the comparison is made per setting against the runs of experiments/release-0.1.2.yaml (same input, same settings, tree 72a3eef; registered on exp/improve-1, f619fd4), with the registered predictions below and the default rule: n = 3 against n = 3, a difference is claimed only when the two ranges separate completely in the registered direction (exact one-sided p = 1/20); overlapping ranges claim no effect. If release-0.1.2 has no complete runs when these end, baseline-0.1.0 (tree 4468bfd) is the comparison and the report says so. Every end counts; no run is repaired, reopened or forced. A refuted or inconclusive result is published as such.

## Settings

| setting | strong | mixed |
|---|---|---|
| `adapter` | metaharness | metaharness |
| `autonomy` | A5 | A5 |
| `decider.adapter` | claude-cli | claude-cli |
| `decider.escalate` | final | final |
| `decider.kind` | model | model |
| `decider.max_decider_calls` | 40 | 40 |
| `decider.model` | claude-opus-5-5 | claude-haiku-5-5 |
| `implement_coverage` | on | on |
| `max_llm_calls` | 150 | 150 |
| `max_wall_seconds` | 5400 | 5400 |
| `models.implement.model` | claude-sonnet-5-5 | claude-sonnet-5-5 |
| `models.other.model` | claude-sonnet-5-5 | claude-haiku-5-5 |
| `models.review.model` | claude-sonnet-5-5 | claude-sonnet-5-5 |
| `models.writer.model` | claude-sonnet-5-5 | claude-sonnet-5-5 |
| `review.adapter` | claude-cli | claude-cli |

Known confounds:

- `tree`: the comparison runs are on another tree, started from another record at another time; everything else that differs between 72a3eef and this tree is the change of this branch only (git diff 72a3eef 3f3a577).
- `unstick`: run.unstick is on under autopilot with an Opus model in both trees (scripts/run-trial.sh); an unstuck run is not a clean pass.
- `canon`: every run uses the canon binary of the canon checkout at run time, which no record pins; release-0.1.2 has the same confound.
- `adapter`: the implement node runs on metaharness, which has no response-schema option, so the coverage schema reaches it in the prompt and the reply is a file (out/coverage.json) checked by program; under claude-cli it would be schema-constrained.

## Runs

| run | variant | end | clean pass | cost USD | wall s | looping factor | note |
|---|---|---|---|---|---|---|---|

## Per variant

Median [min, max] (n) over the ended runs.

| | strong | mixed |
|---|---|---|
| runs listed | 0 | 0 |
| ended | 0 | 0 |
| clean passes | 0 | 0 |
| `run.cost_usd` | - | - |
| `run.llm_calls` | - | - |
| `run.decider_calls` | - | - |
| `run.wall_s` | - | - |
| `run.driver_s` | - | - |
| `run.real_takes` | - | - |
| `run.rework_takes` | - | - |
| `run.looping_factor` | - | - |
| `run.first_pass_yield` | - | - |
| `run.rework_cost_usd` | - | - |
| `run.furthest_stage_index` | - | - |
| `run.findings_raised` | - | - |
| `run.waiting_on_person_s` | - | - |
| `run.clean` | - | - |

## Loops

Per variant, over the ended runs: of the writer and approval nodes taken again at least once, the three with the highest median looping factor; and the three edges with the highest loop cost.

- strong (n 0):
  - no node taken again, no edge
- mixed (n 0):
  - no node taken again, no edge

## Verdict

descriptive: no hypothesis: the record describes its runs and decides nothing.

## Facts

- The change (docs/driver.md, "Requirement coverage"): change.implement replies with a coverage document; the binding's postcondition sdc-reply check-coverage refuses the revision before tests.run when a requirement of the story is missing, not built, cited at a file or line the tree lacks, or named with a test that does not exist; the refusal reaches the next implement run as in/feedback.md and counts as retry kind requirement_coverage against max_runs 3; the review gets the document as in/coverage.json.
- baseline-0.1.0 review rejections, 10 in 9 runs: a requirement not built in the story that covers it 4 (strong-2 twice, strong-3, mixed-3), sync logic wrong 2, scope and build disagree 2, the Postgres default q9 1, build diverges from the spec 1 (experiments/baseline-0.1.0/losses.md, on exp/improve-1).

## Links

- experiments/release-0.1.2.yaml (exp/improve-1, f619fd4): the comparison tree's record
- experiments/baseline-0.1.0.yaml: the earlier baseline, same input and settings
- story:implement-cites-requirements
