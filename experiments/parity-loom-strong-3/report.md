# parity-loom-strong-3: The to-do idea in the strong setting through Loom, against the three driver runs of release-0.1.2 (third record, the decider's answers declared in bindings/tools/answers.yaml)

Written by `sdc-experiment report parity-loom-strong-3` from `experiments/parity-loom-strong-3.yaml` and the run summaries in `experiments/parity-loom-strong-3/runs/`; `sdc-experiment check --all` fails when it is stale. Definitions: `docs/workflow-metrics.md`.

- Kind: prospective; status: complete; created 2026-11-22.
- Purpose: Registered hypothesis, written before the runs: with the decider's answers declared in bindings/tools/answers.yaml (scope.answer, amend.upstream), the runs through Loom get past scope.answer, where all three runs of parity-loom-strong-2 stopped (decider_cannot_decide in IDEA-1.intake), and end like the three driver runs of the strong setting of release-0.1.2, within the tolerance written in story:parity-run-on-loom ("Tolerance, written before the Loom runs"). What counts against it: a run that stops at a decision the answers file declares, or an end outside the tolerance. It is the evaluation run of that story. This record follows parity-loom-strong-2, which stays as the evidence of its three stops and is not edited.
- Hypothesis: none; the record describes its runs.
- Input `todo-app` (idea, sha256 f384a752a2cb): A to-do list web app for me and my partner that also works on the train without a connection
- Tree: `e7dfa4b582bd1af703ad454eff96d97b3860314f`
- Runs per variant: 3; pass rule: clean-delivery (closed, delivered, no reopen, forced step or unstick).
- Decision rule (descriptive): Hypothesis: with the decider's answers declared in bindings/tools/answers.yaml, the runs through Loom get past scope.answer and end like the three driver runs of the strong setting of release-0.1.2, within the tolerance of story:parity-run-on-loom. The record has one variant, so sdc-experiment reports it as descriptive and decides nothing; the verdict is read against the tolerance in the story, run by run: each quality measure of a Loom run lies within the range of the three driver runs (experiments/release-0.1.2/report.md, strong), and the members the story lists as equal are equal. Counts against the hypothesis: a run that stops at a decision the answers file declares, or an end outside the tolerance. Every end counts; no run is repaired, reopened or forced. A run that falls outside is reported as such. This record follows parity-loom-strong-2, whose three runs stopped at scope.answer.

## Settings

| setting | strong |
|---|---|
| `adapter` | metaharness |
| `autonomy` | A5 |
| `decider.adapter` | claude-cli |
| `decider.escalate` | final |
| `decider.kind` | model |
| `decider.max_decider_calls` | 40 |
| `decider.model` | claude-opus-5-5 |
| `loom.bin` | ~/.local/state/runbook/loom/5af3f9c2/b10x-loom |
| `loom.cap_micro_usd` | 18630000 |
| `loom.confinement` | local |
| `loom.sha256` | 074a9e8691addf521b8d843c796bcc363eb42a6ca9cea8359b65b33f083b49e2 |
| `max_llm_calls` | 150 |
| `max_wall_seconds` | 5400 |
| `models.implement.model` | claude-sonnet-5-5 |
| `models.other.model` | claude-sonnet-5-5 |
| `models.review.model` | claude-sonnet-5-5 |
| `models.writer.model` | claude-sonnet-5-5 |
| `review.adapter` | claude-cli |

Known confounds:

- `runtime`: the runs go through Loom with `runbook loom-run`, the driver runs through sdc-drive and run-trial.sh; that is what the experiment tests, so it is not held equal.
- `confinement`: --confinement local, the runs are not confined; the driver runs were not either.
- `unstick`: the driver runs of release-0.1.2 ran with run.unstick on (an Opus unstick that an unstuck run counts against clean); a Loom run has no unstick, a run that stops stays stopped.
- `decider`: the model decider is the one of bindings/tools/decider.sh through the claude-cli adapter in both runtimes; its answers reach Loom as printed decision commands with a named result and the reasons of a rejection as a file, and now also through the answers file bindings/tools/answers.yaml and the helper sdc-answers (scope.answer, amend.upstream), which the second record did not have.
- `canon`: every run uses the canon binary on the login's PATH at run time, which no record pins; release-0.1.2 had the same confound.
- `tree`: the tree is main with the daemon merged (e7dfa4b58); the driver runs of release-0.1.2 ran on the tree of that release (72a3eef).

## Runs

| run | variant | end | clean pass | cost USD | wall s | looping factor | note |
|---|---|---|---|---|---|---|---|
| parity-loom-strong-3.strong.1 | strong | stopped: error in IDEA-1.todo.sync | no | 1.761 | 476 | 1.1 |  |
| parity-loom-strong-3.strong.2 | strong | stopped: decider_cannot_decide in IDEA-1.intake | no | 0.565 | 138 | 1 |  |
| parity-loom-strong-3.strong.3 | strong | stopped: error in IDEA-1.todo.task | no | 1.766 | 480 | 1.2 |  |

## Per variant

Median [min, max] (n) over the ended runs.

| | strong |
|---|---|
| runs listed | 3 |
| ended | 3 |
| clean passes | 0 |
| `run.cost_usd` | 1.7612 [0.5655, 1.7662] (3) |
| `run.llm_calls` | 10 [4, 11] (3) |
| `run.decider_calls` | 6 [5, 7] (3) |
| `run.wall_s` | 476 [138, 480] (3) |
| `run.driver_s` | 352 [58, 368] (3) |
| `run.real_takes` | 67 [18, 75] (3) |
| `run.rework_takes` | 18 [7, 25] (3) |
| `run.looping_factor` | 1.1 [1, 1.2] (3) |
| `run.first_pass_yield` | 0.9 [0.8, 1] (3) |
| `run.rework_cost_usd` | 0 [0, 0.1714] (3) |
| `run.furthest_stage_index` | 6 [1, 6] (3) |
| `run.findings_raised` | 1 [1, 4] (3) |
| `run.waiting_on_person_s` | 0 [0, 0] (3) |
| `run.clean` | 0 [0, 0] (3) |

## Loops

Per variant, over the ended runs: of the writer and approval nodes taken again at least once, the three with the highest median looping factor; and the three edges with the highest loop cost.

- strong (n 3):
  - `story/change.implement` (llm-writer): looping factor median 2 (max 2, n 2), rework takes 2, rework cost 0 USD
  - `spec/spec.author` (llm-writer): looping factor median 1.5 (max 2, n 2), rework takes 1, rework cost 0.171 USD
  - `spec: finding@spec -> intake` (finding): count 2 in 1 runs, loop cost 0.171 USD, loop driver 30 s
  - `story: finding@story -> intake` (finding): count 3 in 2 runs, loop cost 0 USD, loop driver 10 s

## Verdict

descriptive: no hypothesis: the record describes its runs and decides nothing.

## Facts

- Loom commit 5af3f9c2 (b10x-loom 0.58.0), binary sha256 074a9e8691addf521b8d843c796bcc363eb42a6ca9cea8359b65b33f083b49e2; it names every-printed-decision-command-runs and person-action-records-a-named-result (the two conditions of the approval).
- The model login is named at start time only: sdc-experiment run parity-loom-strong-3 --login-file <login file>; no record, log or report names its path.
- Spend: the approved cap is 20.00 USD in total. parity-loom-strong-2 spent 0.37 USD (ledger 366257 micro-USD in 9 entries), so 19.63 USD are left. The ledger is per record id (runs/parity-loom-strong-3.loom-spend.json) and starts empty, so the cap of this record is what is left: 19.63 USD, and loom.cap_micro_usd is the stop line, 18.63 USD (18630000 micro-USD).
- The fix: the decider's answers are declared in bindings/tools/answers.yaml (scope.answer on ApproveAction, amend.upstream on Handoff), each with the command that lists the requests, the command that applies an answer and the command that follows; the runs reach them through sdc-answers on the PATH of the runs. The three runs of parity-loom-strong-2 stopped at scope.answer without it.

## Links

- experiments/parity-loom-strong-2.yaml: the second record; its three runs stopped at decider_cannot_decide in IDEA-1.intake (scope.answer), after 0.37 USD
- experiments/release-0.1.2.yaml: the three driver runs of the strong setting this record is compared with
- story:parity-run-on-loom: the tolerance and the acceptance of the evaluation run
- approval-record:operator-decision-parity-runs-more-2026-11-22: the approval of these three runs from the 19.63 USD left
- approval-record:operator-decision-parity-runs-spend-2026-11-22 and approval-record:operator-decision-parity-runs-unconfined-2026-11-22: the two earlier approvals, the cap of 20 USD in total and the conditions
