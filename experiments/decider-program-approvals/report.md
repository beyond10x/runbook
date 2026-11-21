# decider-program-approvals: Approvals taken by program checks under A5 against every approval asked of the decider, strong setting

Written by `sdc-experiment report decider-program-approvals` from `experiments/decider-program-approvals.yaml` and the run summaries in `experiments/decider-program-approvals/runs/`; `sdc-experiment check --all` fails when it is stale. Definitions: `docs/workflow-metrics.md`.

- Kind: prospective; status: planned; created 2026-11-21.
- Purpose: Whether taking context.approve, roadmap.approve and release.publish by rule:autopilot when the program check guarding each passed at the exact revision lowers the Opus decider's cost per run without losing a clean pass or letting a merge or review rejection through. The mixed setting is the sibling record decider-program-approvals-mixed (one variable per record).
- Hypothesis: With SDC_RULE_APPROVALS on instead of off, run.decider_cost_usd of the strong setting falls by at least 0.3 USD per run.
- Input `todo-app` (idea, sha256 f384a752a2cb): A to-do list web app for me and my partner that also works on the train without a connection
- Tree: `f2c503bc03d2d51bf563847efccff7a254893e97`
- Runs per variant: 3; pass rule: clean-delivery (closed, delivered, no reopen, forced step or unstick).
- Decision rule (complete-separation): held when off and on each have 3 ended runs, every on run's run.decider_cost_usd lies below every off run's (exact one-sided p = 1/20 = 0.05) and the medians differ by at least 0.3 USD; contradicted when the ranges separate the other way or by less than 0.3 USD; inconclusive otherwise. Adopt the rule as the default under A5 only when held, every guard holds and the sibling record decider-program-approvals-mixed does not contradict it; a refuted or inconclusive result is published as such.

## Settings

| setting | off | on |
|---|---|---|
| `adapter` | metaharness | metaharness |
| `autonomy` | A5 | A5 |
| `decider.adapter` | claude-cli | claude-cli |
| `decider.escalate` | final | final |
| `decider.kind` | model | model |
| `decider.max_decider_calls` | 40 | 40 |
| `decider.model` | claude-opus-5-5 | claude-opus-5-5 |
| `efforts.decider.effort` | xhigh | xhigh |
| `efforts.implement.effort` | high | high |
| `efforts.other.effort` | high | high |
| `efforts.review.effort` | high | high |
| `efforts.writer.effort` | high | high |
| `max_llm_calls` | 150 | 150 |
| `max_wall_seconds` | 5400 | 5400 |
| `models.implement.model` | claude-sonnet-5-5 | claude-sonnet-5-5 |
| `models.other.model` | claude-sonnet-5-5 | claude-sonnet-5-5 |
| `models.review.model` | claude-sonnet-5-5 | claude-sonnet-5-5 |
| `models.writer.model` | claude-sonnet-5-5 | claude-sonnet-5-5 |
| `review.adapter` | claude-cli | claude-cli |
| `rule_approvals` | off | on |

Known confounds:

- `coverage`: vision.approve has no program check (its precondition holds context claims only) and stays with the decider in both arms; merge.approve, scope answers, finding decisions and upstream amendments are asked of the decider in both arms. The rule removes three of the 13 decider calls of a strong run at most.
- `unstick`: run.unstick is on with an Opus model in both variants (scripts/run-trial.sh; not a launch setting); it is the same in both arms, and a run it touched is not clean.
- `canon`: the canon binary of the canon checkout at run time, not pinned; the same for both arms when they run in one launch.
- `quality`: no oracle judges the decider's decisions; the baseline decider approved all of the covered approvals, so the arms differ only if a covered approval would have been rejected, which is seen through the merge and review rejection guards and run.delivered.

## Runs

| run | variant | end | clean pass | cost USD | wall s | looping factor | note |
|---|---|---|---|---|---|---|---|

## Per variant

Median [min, max] (n) over the ended runs.

| | off | on |
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

- off (n 0):
  - no node taken again, no edge
- on (n 0):
  - no node taken again, no edge

## Verdict

inconclusive: too few ended runs with the metric: off 0 and on 0, the rule needs 3 each.

Primary metric `run.decider_cost_usd` (decrease, at least 0.3):

- off: n 0, median null, range [null, null]
- on: n 0, median null, range [null, null]
- guard `run.delivered`: unknown (worsening -)
- guard `run.clean`: unknown (worsening -)
- guard `edge.story: merge.approve:rejected -> change.implement.count`: unknown (worsening -)
- guard `edge.story: change.review:rejected -> change.implement.count`: unknown (worsening -)
- recommendation: run more

## Facts

- Registered basis: in baseline-0.1.0 (3 strong and 3 mixed runs, plus the earlier trial-3 runs on the same decider), the decider stopped nothing on context.approve, roadmap.approve, vision.approve and release.publish: 25 of 25 approved.
- Decider cost of the covered approvals per strong run in baseline-0.1.0 (decisions.by_action): context.approve 0.138, 0.139, 0.151 USD; roadmap.approve 0.155, 0.179, 0.151 USD; release.publish 0.158, 0.171, 0.195 USD; total 0.451, 0.489, 0.496 USD of 2.03, 2.08, 1.82 USD decider cost per run.
- The covered approvals per mixed run cost 0.043 to 0.046 USD of 0.17 to 0.25 USD decider cost (Haiku decider).
- Not launched: this record is committed before any run; the rule is built and its cycles run on the stub adapter (scripts/run-finding-cycles.sh: rule-approvals, rule-approvals-check-failed, rule-approvals-revision-changed, rule-approvals-off, rule-approvals-a3).

## Links

- experiments/decider-program-approvals-mixed.yaml: the mixed setting, primary run.decider_s
- experiments/baseline-0.1.0.yaml: the strong and mixed medians above
- docs/driver.md: Approvals by program
