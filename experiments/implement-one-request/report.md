# implement-one-request: change.implement as one structured request per call against the session, mixed setting

Written by `sdc-experiment report implement-one-request` from `experiments/implement-one-request.yaml` and the run summaries in `experiments/implement-one-request/runs/`; `sdc-experiment check --all` fails when it is stale. Definitions: `docs/workflow-metrics.md`.

- Kind: prospective; status: planned; created 2026-11-21.
- Purpose: Whether change.implement as one request with its whole input assembled by a program (SDC_IMPLEMENT_MODE=request, story:implement-one-request) costs less per run than the session with file tools, without losing delivery, clean passes or first builds, in the mixed setting; the strong setting is the sibling record implement-one-request-strong on the same tree.
- Hypothesis: With SDC_IMPLEMENT_MODE request instead of session, the cost of story/change.implement per run falls by at least 0.25 USD in the mixed setting.
- Input `todo-app` (idea, sha256 f384a752a2cb): A to-do list web app for me and my partner that also works on the train without a connection
- Tree: `ea5f0e07fa33ac7204b109e44a41f609e7b1f13f`
- Runs per variant: 3; pass rule: clean-delivery (closed, delivered, no reopen, forced step or unstick).
- Decision rule (complete-separation): held when session and request each have 3 ended runs that took change.implement, every request run's change.implement cost lies below every session run's (exact one-sided p = 1/20 = 0.05) and the medians differ by at least 0.25 USD; contradicted when the ranges separate the other way; inconclusive otherwise. Run cost (run.cost_usd) is reported beside it and must not rise (guard). Adopt request as the default only when held here and in implement-one-request-strong, and every guard holds in both: delivered and clean passes not fewer, first-pass yield of change.implement not lower by more than 0.25, tests.run failures and change.review rejections sent back to change.implement not more by more than one per run. Every end counts; no run is repaired, reopened or forced.

## Settings

| setting | session | request |
|---|---|---|
| `adapter` | metaharness | metaharness |
| `autonomy` | A5 | A5 |
| `decider.adapter` | claude-cli | claude-cli |
| `decider.escalate` | final | final |
| `decider.kind` | model | model |
| `decider.max_decider_calls` | 40 | 40 |
| `decider.model` | claude-haiku-5-5 | claude-haiku-5-5 |
| `implement_mode` | session | request |
| `max_llm_calls` | 150 | 150 |
| `max_wall_seconds` | 5400 | 5400 |
| `models.implement.model` | claude-sonnet-5-5 | claude-sonnet-5-5 |
| `models.other.model` | claude-haiku-5-5 | claude-haiku-5-5 |
| `models.review.model` | claude-sonnet-5-5 | claude-sonnet-5-5 |
| `models.writer.model` | claude-sonnet-5-5 | claude-sonnet-5-5 |
| `review.adapter` | claude-cli | claude-cli |

Known confounds:

- `adapter`: in request mode change.implement runs on claude-cli (sdc-reply adapters, SDC_IMPLEMENT_ADAPTER unset, adapter metaharness), in session mode on metaharness; the harness prefix each pays differs (claude -p with --tools "" offers no tool definitions).
- `canon`: every run uses the canon binary of the canon checkout at run time, which no record pins.

## Runs

| run | variant | end | clean pass | cost USD | wall s | looping factor | note |
|---|---|---|---|---|---|---|---|

## Per variant

Median [min, max] (n) over the ended runs.

| | session | request |
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

- session (n 0):
  - no node taken again, no edge
- request (n 0):
  - no node taken again, no edge

## Verdict

inconclusive: too few ended runs with the metric: session 0 and request 0, the rule needs 3 each.

Primary metric `node.story/change.implement.cost_usd` (decrease, at least 0.25):

- session: n 0, median null, range [null, null]
- request: n 0, median null, range [null, null]
- guard `run.delivered`: unknown (worsening -)
- guard `run.clean`: unknown (worsening -)
- guard `run.cost_usd`: unknown (worsening -)
- guard `node.story/change.implement.first_pass_yield`: unknown (worsening -)
- guard `edge.story: tests.run:failed -> change.implement.count`: unknown (worsening -)
- guard `edge.story: change.review:rejected -> change.implement.count`: unknown (worsening -)
- recommendation: run more

## Facts

- experiments/baseline-0.1.0/inputs.md: change.implement is 1.71 USD per mixed run, 0.81 of it conversation growth; the median session takes 23 turns with 45 Reads per run.
- experiments/baseline-0.1.0/costs.md: mixed run 2.55 USD, implement class 1.51 USD; the free agent 0.47 USD in 13 to 14 requests.
- The scripted cycles implement-request-build, -tests-rework, -review-rework, -invalid-retry, -invalid-named and -path-escape pass on the tree (fixture adapter, no model).

## Links

- experiments/implement-one-request-strong.yaml: the same comparison in the strong setting
- docs/driver.md: Implement as one request
