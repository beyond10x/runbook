# build-constraints: Build constraints known to every stage (fix/superseded-answers on 0.1.2, 3ca26fe), Haiku and mixed against the release-0.1.2 runs

Written by `sdc-experiment report build-constraints` from `experiments/build-constraints.yaml` and the run summaries in `experiments/build-constraints/runs/`; `sdc-experiment check --all` fails when it is stale. Definitions: `docs/workflow-metrics.md`.

- Kind: prospective; status: planned; created 2026-11-21.
- Purpose: Whether one run file of build constraints, read by the writers and the decider and held by requirements.check and the decider's answer check, raises the Haiku setting's clean passes and lowers change.implement reruns against the same settings on 0.1.2.
- Hypothesis: none; the record describes its runs.
- Input `todo-app` (idea, sha256 f384a752a2cb): A to-do list web app for me and my partner that also works on the train without a connection
- Tree: `3ca26fee9a874cba6b0f7707d73a51af1f734df2`
- Runs per variant: 3; pass rule: clean-delivery (closed, delivered, no reopen, forced step or unstick).
- Decision rule (descriptive): Treatment (this record, tree 3ca26fe) against control (release-0.1.2, haiku and mixed, tree 72a3eef), per setting, n = 3 against n = 3, directions registered here before any treatment run. A difference is claimed only when the two ranges separate completely in the registered direction (exact one-sided p = 1/20); overlapping ranges claim no effect. Primary 1: run.clean of the haiku setting, direction increase: held only when all 3 treatment runs are clean and no control run is; any other count is inconclusive or no move and is published as such. Primary 2: node.story/change.implement.rework_takes per run, direction decrease, judged per setting (haiku, mixed); a run that never took change.implement has no value and is listed, not counted. The two primaries are judged separately and no combined claim is made from them (two tests: one holding by chance is more likely than 1/20). Secondary, reported and not decided: the count of stops with cause writer_spent, and the count of change.implement and merge.approve rejections whose reason names a language, a dependency or a stored technology. Guards, against the control per setting: run.delivered (worse when lower), run.cost_usd (worse when higher by more than half the control median), run.decider_calls and the decision classes' invalid_replies (worse when higher; the new decider answer check is expected to add some, reported with their reasons), run.wall_s (worse when higher by more than half the control median). Adopt (merge fix/superseded-answers-0.1.2) only when a primary holds and every guard holds; a refuted or inconclusive result is published as such.

## Settings

| setting | haiku | mixed |
|---|---|---|
| `adapter` | metaharness | metaharness |
| `autonomy` | A5 | A5 |
| `decider.adapter` | claude-cli | claude-cli |
| `decider.escalate` | final | final |
| `decider.kind` | model | model |
| `decider.max_decider_calls` | 40 | 40 |
| `decider.model` | claude-haiku-5-5 | claude-haiku-5-5 |
| `max_llm_calls` | 150 | 150 |
| `max_wall_seconds` | 5400 | 5400 |
| `models.implement.model` | claude-haiku-5-5 | claude-sonnet-5-5 |
| `models.other.model` | claude-haiku-5-5 | claude-haiku-5-5 |
| `models.review.model` | claude-haiku-5-5 | claude-sonnet-5-5 |
| `models.writer.model` | claude-haiku-5-5 | claude-sonnet-5-5 |
| `review.adapter` | claude-cli | claude-cli |

Known confounds:

- `control-time`: the control runs (release-0.1.2) ran earlier on the same day, before these; the model service, the canon checkout and the machine's load may differ between the two launches.
- `tree`: 3ca26fe is 0.1.2 (72a3eef) plus fix/superseded-answers merged, the decider-constraints cycle's principal fix and the re-packed replay recording; only the merge changes what a run does.
- `unstick`: run.unstick is on with an Opus model in both arms and is no launch setting (scripts/run-trial.sh); an unstuck run is not clean.
- `canon`: the canon binary of the canon checkout at run time, not pinned, in both arms.
- `control-complete`: if release-0.1.2 ends with fewer than 3 ended runs of a setting, that setting's comparison is inconclusive and says so.

## Runs

| run | variant | end | clean pass | cost USD | wall s | looping factor | note |
|---|---|---|---|---|---|---|---|

## Per variant

Median [min, max] (n) over the ended runs.

| | haiku | mixed |
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

- haiku (n 0):
  - no node taken again, no edge
- mixed (n 0):
  - no node taken again, no edge

## Verdict

descriptive: no hypothesis: the record describes its runs and decides nothing.

## Facts

- The branch on 0.1.2 passes its cycles: "100 expected, 100 as expected, 0 failed" (head 3ca26fe, reported by the merging session).
- What a run does differently at 3ca26fe: work/build-constraints.yaml in every run, recorded in run-settings.jsonl (`build_constraints`); requirements.check refuses a requirement whose language or dependency contradicts it; the decider gets it on every call but the close, and an answer naming a `ruled_out` entry is refused as an invalid reply and asked again; a contradiction against an answer already resolved re-raises the finding.

## Links

- experiments/release-0.1.2.yaml: the control arm
- experiments/baseline-0.1.0/losses.md: haiku-1's q9 and the loss this change addresses (1.220 USD, 976 s)
- story:build-constraints-every-stage, story:default-answers-within-build-constraints
