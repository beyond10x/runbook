# effort-decider-mixed: Decider effort medium against xhigh on the Haiku decider of the mixed setting, everything else fixed

Written by `sdc-experiment report effort-decider-mixed` from `experiments/effort-decider-mixed.yaml` and the run summaries in `experiments/effort-decider-mixed/runs/`; `sdc-experiment check --all` fails when it is stale. Definitions: `docs/workflow-metrics.md`.

- Kind: prospective; status: complete; created 2026-11-21.
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
| effort-decider-mixed.xhigh.1 | xhigh | closed: operating | yes | 5.675 | 3032 | 1.583 |  |
| effort-decider-mixed.xhigh.2 | xhigh | closed: operating | yes | 3.143 | 1913 | 1.75 |  |
| effort-decider-mixed.xhigh.3 | xhigh | closed: operating | yes | 4.122 | 3677 | 1.5 |  |
| effort-decider-mixed.medium.1 | medium | closed: operating | yes | 2.471 | 1424 | 1.917 |  |
| effort-decider-mixed.medium.2 | medium | closed: operating | no | 3.078 | 1831 | 1.75 |  |
| effort-decider-mixed.medium.3 | medium | closed: operating | yes | 3.095 | 2191 | 1.583 |  |

## Per variant

Median [min, max] (n) over the ended runs.

| | xhigh | medium |
|---|---|---|
| runs listed | 3 | 3 |
| ended | 3 | 3 |
| clean passes | 3 | 2 |
| `run.cost_usd` | 4.1218 [3.1431, 5.6753] (3) | 3.0783 [2.4709, 3.0949] (3) |
| `run.llm_calls` | 24 [24, 24] (3) | 26 [23, 27] (3) |
| `run.decider_calls` | 16 [15, 20] (3) | 17 [17, 19] (3) |
| `run.wall_s` | 3032 [1913, 3677] (3) | 1831 [1424, 2191] (3) |
| `run.driver_s` | 1690 [1062, 2682] (3) | 1556 [1161, 1952] (3) |
| `run.real_takes` | 125 [117, 128] (3) | 130 [121, 134] (3) |
| `run.rework_takes` | 55 [47, 58] (3) | 60 [51, 66] (3) |
| `run.looping_factor` | 1.583 [1.5, 1.75] (3) | 1.75 [1.583, 1.917] (3) |
| `run.first_pass_yield` | 0.667 [0.667, 0.667] (3) | 0.75 [0.75, 0.75] (3) |
| `run.rework_cost_usd` | 0.9952 [0.7895, 1.9405] (3) | 0.8463 [0.2009, 0.9645] (3) |
| `run.furthest_stage_index` | 7 [7, 7] (3) | 7 [7, 7] (3) |
| `run.findings_raised` | 12 [10, 13] (3) | 11 [9, 12] (3) |
| `run.waiting_on_person_s` | 0 [0, 0] (3) | 0 [0, 0] (3) |
| `run.clean` | 1 [1, 1] (3) | 1 [0, 1] (3) |

## Loops

Per variant, over the ended runs: of the writer and approval nodes taken again at least once, the three with the highest median looping factor; and the three edges with the highest loop cost.

- xhigh (n 3):
  - `direction/vision.write` (llm-writer): looping factor median 4 (max 5, n 3), rework takes 9, rework cost 0.052 USD
  - `story/change.implement` (llm-writer): looping factor median 2 (max 2, n 3), rework takes 7, rework cost 2.383 USD
  - `spec/spec.author` (llm-writer): looping factor median 2 (max 3, n 3), rework takes 4, rework cost 0.462 USD
  - `story: merge.approve:rejected -> change.implement` (approval): count 3 in 2 runs, loop cost 1.627 USD, loop driver 374 s
  - `story: finding@story -> intake` (finding): count 13 in 3 runs, loop cost 1.401 USD, loop driver 372 s
  - `story: change.review:rejected -> change.implement` (check): count 2 in 1 runs, loop cost 0.443 USD, loop driver 155 s
- medium (n 3):
  - `direction/vision.write` (llm-writer): looping factor median 6 (max 8, n 3), rework takes 15, rework cost 0.082 USD
  - `direction/roadmap.write` (llm-writer): looping factor median 3 (max 4, n 3), rework takes 6, rework cost 0.038 USD
  - `direction/vision.approve` (approval): looping factor median 3 (max 4, n 3), rework takes 6, rework cost 0 USD
  - `story: change.review:rejected -> change.implement` (check): count 4 in 2 runs, loop cost 1.233 USD, loop driver 472 s
  - `story: finding@story -> intake` (finding): count 5 in 2 runs, loop cost 0.531 USD, loop driver 867 s
  - `direction: finding@vision -> intake` (finding): count 11 in 3 runs, loop cost 0.169 USD, loop driver 398 s

## Verdict

inconclusive: the ranges overlap (median difference -1201): no effect is claimed.

Primary metric `run.wall_s` (decrease, at least 300.0):

- xhigh: n 3, median 3032.0, range [1913.0, 3677.0]
- medium: n 3, median 1831.0, range [1424.0, 2191.0]
- guard `run.delivered`: holds (worsening 0)
- guard `run.cost_usd`: holds (worsening -1.043)
- guard `run.decider_calls`: holds (worsening 1)
- guard `run.rework_takes`: holds (worsening 5)
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
