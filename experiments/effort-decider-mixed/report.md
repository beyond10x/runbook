# effort-decider-mixed: Decider effort medium against xhigh on the Haiku decider of the mixed setting, everything else fixed

Written by `sdc-experiment report effort-decider-mixed` from `experiments/effort-decider-mixed.yaml` and the run summaries in `experiments/effort-decider-mixed/runs/`; `sdc-experiment check --all` fails when it is stale. Definitions: `docs/workflow-metrics.md`.

- Kind: prospective; status: planned; created 2026-11-21.
- Purpose: Whether lowering the Haiku decider's reasoning effort from its default xhigh to medium removes the decider's share of the mixed setting's wall time without costing a clean pass.
- Hypothesis: With the Haiku decider at effort medium instead of xhigh, run.wall_s of the mixed setting falls by at least 300 s.
- Input `todo-app` (idea, sha256 f384a752a2cb): A to-do list web app for me and my partner that also works on the train without a connection
- Tree: `72a3eefb20e0edff50dc8ad12fcf561f8e0790e3`
- Runs per variant: 3; pass rule: clean-delivery (closed, delivered, no reopen, forced step or unstick).
- Decision rule (complete-separation): held when xhigh and medium each have 3 ended runs, every medium run's run.wall_s lies below every xhigh run's (exact one-sided p = 1/20 = 0.05) and the medians differ by at least 300 s; contradicted when the ranges separate the other way or by less than 300 s; inconclusive otherwise, then effort-decider-mixed-2 with the same settings. Adopt medium as the decider's default effort for a Haiku decider only when held and every guard holds; a refuted or inconclusive result is published as such.

## Settings

| setting | xhigh | medium |
|---|---|---|
| `adapter` | metaharness | metaharness |
| `autonomy` | A5 | A5 |
| `decider.adapter` | claude-cli | claude-cli |
| `decider.escalate` | final | final |
| `decider.kind` | model | model |
| `decider.max_decider_calls` | 40 | 40 |
| `decider.model` | claude-haiku-5-5 | claude-haiku-5-5 |
| `efforts.decider.effort` | xhigh | medium |
| `efforts.implement.effort` | high | high |
| `efforts.other.effort` | high | high |
| `efforts.review.effort` | high | high |
| `efforts.writer.effort` | high | high |
| `max_llm_calls` | 150 | 150 |
| `max_wall_seconds` | 5400 | 5400 |
| `models.implement.model` | claude-sonnet-5-5 | claude-sonnet-5-5 |
| `models.other.model` | claude-haiku-5-5 | claude-haiku-5-5 |
| `models.review.model` | claude-sonnet-5-5 | claude-sonnet-5-5 |
| `models.writer.model` | claude-sonnet-5-5 | claude-sonnet-5-5 |
| `review.adapter` | claude-cli | claude-cli |

Known confounds:

- `unstick`: run.unstick is on with an Opus model in both variants (scripts/run-trial.sh:377-385; not a launch setting); it is the same in both arms, and a run it touched is not clean.
- `canon`: the canon binary of the canon checkout at run time, not pinned; the same for both arms when they run in one launch.
- `quality`: no oracle judges the decider's decisions; a lower effort that approves what xhigh would reject (trial-0.1.0-mixed-2's merge rejection of IDEA-1.list.items caught an offline requirement the review had passed) is seen only through run.rework_takes, run.delivered and the decision counts in the report, not as a fault rate.

## Runs

| run | variant | end | clean pass | cost USD | wall s | looping factor | note |
|---|---|---|---|---|---|---|---|

## Per variant

Median [min, max] (n) over the ended runs.

| | xhigh | medium |
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

- xhigh (n 0):
  - no node taken again, no edge
- medium (n 0):
  - no node taken again, no edge

## Verdict

inconclusive: too few ended runs with the metric: xhigh 0 and medium 0, the rule needs 3 each.

Primary metric `run.wall_s` (decrease, at least 300.0):

- xhigh: n 0, median null, range [null, null]
- medium: n 0, median null, range [null, null]
- guard `run.delivered`: unknown (worsening -)
- guard `run.cost_usd`: unknown (worsening -)
- guard `run.decider_calls`: unknown (worsening -)
- guard `run.rework_takes`: unknown (worsening -)
- recommendation: run more

## Facts

- Loss addressed, from sdc-run-metrics and decisions.jsonl of baseline-0.1.0 (runs not in git): decider seconds per mixed run 717, 952, 693 (48, 45, 46 percent of wall 1480, 2091, 1489 s); per strong run (Opus decider, same default effort xhigh) 262, 254, 227 s over 13 decisions each.
- trial-0.1.0-mixed-1: the Haiku decider's merge.approve and release.publish calls took 97, 132, 173 and 96 s with 19,684 to 35,402 thinking tokens each; trial-0.1.0-strong-1's Opus decider took 11 to 29 s per decision with 29 to 640 thinking tokens.
- Upper bound of the effect: the whole decider time, median 717 s; if a medium Haiku decider took the Opus decider's about 20 s per decision, mixed wall would fall by about 450 s (inferred).
- The default efforts are high for every node class and xhigh for the decider, requester and unstick (scripts/run-trial.sh:57-59, run-settings.jsonl of baseline-0.1.0 efforts.*.source default).

## Links

- experiments/baseline-0.1.0.yaml: mixed variant, the decider seconds above
- experiments/release-0.1.2.yaml: its mixed variant has the xhigh arm's settings on the same tree
- experiments/effort-implement-levels.yaml: the effort sweep of the implement class (story:effort-levels-per-class)
