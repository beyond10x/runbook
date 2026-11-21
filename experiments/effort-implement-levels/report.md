# effort-implement-levels: Reasoning effort low, medium, high on the implement class, everything else fixed

Written by `sdc-experiment report effort-implement-levels` from `experiments/effort-implement-levels.yaml` and the run summaries in `experiments/effort-implement-levels/runs/`; `sdc-experiment check --all` fails when it is stale. Definitions: `docs/workflow-metrics.md`.

- Kind: prospective; status: planned; created 2026-11-21.
- Purpose: Whether more reasoning effort on the implement class lowers how often change.implement is sent back, three runs per level, as the operator described.
- Hypothesis: With effort high instead of low on the implement class, the looping factor of story/change.implement falls by at least 0.5.
- Input `todo-app` (idea, sha256 f384a752a2cb): A to-do list web app for me and my partner that also works on the train without a connection
- Tree: not pinned yet
- Runs per variant: 3; pass rule: clean-delivery (closed, delivered, no reopen, forced step or unstick).
- Decision rule (complete-separation): held when low and high each have 3 ended runs that took change.implement, every high run's looping factor lies below every low run's (exact one-sided p = 1/20 = 0.05) and the medians differ by at least 0.5; contradicted when the ranges separate the other way or by less than 0.5; inconclusive otherwise. Medium is reported beside them for the shape of the curve. Adopt high on the implement class only when held and every guard holds.

## Settings

| setting | low | medium | high |
|---|---|---|---|
| `adapter` | metaharness | metaharness | metaharness |
| `autonomy` | A5 | A5 | A5 |
| `decider.adapter` | claude-cli | claude-cli | claude-cli |
| `decider.escalate` | final | final | final |
| `decider.kind` | haiku | haiku | haiku |
| `decider.max_decider_calls` | 40 | 40 | 40 |
| `decider.model` | claude-opus-5-5 | claude-opus-5-5 | claude-opus-5-5 |
| `efforts.decider.effort` | xhigh | xhigh | xhigh |
| `efforts.implement.effort` | low | medium | high |
| `efforts.other.effort` | high | high | high |
| `efforts.review.effort` | high | high | high |
| `efforts.writer.effort` | high | high | high |
| `max_llm_calls` | 150 | 150 | 150 |
| `max_wall_seconds` | 5400 | 5400 | 5400 |
| `models.implement.model` | claude-sonnet-5-5 | claude-sonnet-5-5 | claude-sonnet-5-5 |
| `models.other.model` | claude-sonnet-5-5 | claude-sonnet-5-5 | claude-sonnet-5-5 |
| `models.review.model` | claude-sonnet-5-5 | claude-sonnet-5-5 | claude-sonnet-5-5 |
| `models.writer.model` | claude-sonnet-5-5 | claude-sonnet-5-5 | claude-sonnet-5-5 |

## Runs

| run | variant | end | clean pass | cost USD | wall s | looping factor | note |
|---|---|---|---|---|---|---|---|

## Per variant

Median [min, max] (n) over the ended runs.

| | low | medium | high |
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

- low (n 0):
  - no node taken again, no edge
- medium (n 0):
  - no node taken again, no edge
- high (n 0):
  - no node taken again, no edge

## Verdict

inconclusive: too few ended runs with the metric: low 0 and high 0, the rule needs 3 each.

Primary metric `node.story/change.implement.looping_factor` (decrease, at least 0.5):

- low: n 0, median null, range [null, null]
- medium: n 0, median null, range [null, null]
- high: n 0, median null, range [null, null]
- guard `run.delivered`: unknown (worsening -)
- guard `node.story/change.implement.cost_usd`: unknown (worsening -)
- guard `run.cost_usd`: unknown (worsening -)
- recommendation: run more
