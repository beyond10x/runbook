# parity-loom-strong: The to-do idea in the strong setting through Loom, against the three driver runs of release-0.1.2

Written by `sdc-experiment report parity-loom-strong` from `experiments/parity-loom-strong.yaml` and the run summaries in `experiments/parity-loom-strong/runs/`; `sdc-experiment check --all` fails when it is stale. Definitions: `docs/workflow-metrics.md`.

- Kind: prospective; status: abandoned; created 2026-11-22.
- Purpose: Registered hypothesis, written before the runs: the runs through Loom end like the three driver runs of the strong setting of release-0.1.2, within the tolerance written in story:parity-run-on-loom ("Tolerance, written before the Loom runs"). It is the evaluation run of that story.
- Hypothesis: none; the record describes its runs.
- Input `todo-app` (idea, sha256 f384a752a2cb): A to-do list web app for me and my partner that also works on the train without a connection
- Tree: `0c28adc5238445213b13d231c89ec556682ed95b`
- Runs per variant: 3; pass rule: clean-delivery (closed, delivered, no reopen, forced step or unstick).
- Decision rule (descriptive): Hypothesis: the runs through Loom end like the three driver runs of the strong setting of release-0.1.2, within the tolerance of story:parity-run-on-loom. The record has one variant, so sdc-experiment reports it as descriptive and decides nothing; the verdict is read against the tolerance in the story, run by run: each quality measure of a Loom run lies within the range of the three driver runs (experiments/release-0.1.2/report.md, strong), and the members the story lists as equal are equal. Every end counts; no run is repaired, reopened or forced. A run that falls outside is reported as such.

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
| `loom.cap_micro_usd` | 19000000 |
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
- `decider`: the model decider is the one of bindings/tools/decider.sh through the claude-cli adapter in both runtimes; its answers reach Loom as printed decision commands with a named result and the reasons of a rejection as a file.
- `canon`: every run uses the canon binary on the login's PATH at run time, which no record pins; release-0.1.2 had the same confound.

## Runs

| run | variant | end | clean pass | cost USD | wall s | looping factor | note |
|---|---|---|---|---|---|---|---|
| parity-loom-strong.strong.1 | strong | no summary: not ended | - | - | - | - |  |
| parity-loom-strong.strong.2 | strong | no summary: not ended | - | - | - | - |  |
| parity-loom-strong.strong.3 | strong | no summary: not ended | - | - | - | - |  |

## Per variant

Median [min, max] (n) over the ended runs.

| | strong |
|---|---|
| runs listed | 3 |
| ended | 0 |
| clean passes | 0 |
| `run.cost_usd` | - |
| `run.llm_calls` | - |
| `run.decider_calls` | - |
| `run.wall_s` | - |
| `run.driver_s` | - |
| `run.real_takes` | - |
| `run.rework_takes` | - |
| `run.looping_factor` | - |
| `run.first_pass_yield` | - |
| `run.rework_cost_usd` | - |
| `run.furthest_stage_index` | - |
| `run.findings_raised` | - |
| `run.waiting_on_person_s` | - |
| `run.clean` | - |

## Loops

Per variant, over the ended runs: of the writer and approval nodes taken again at least once, the three with the highest median looping factor; and the three edges with the highest loop cost.

- strong (n 0):
  - no node taken again, no edge

## Verdict

descriptive: no hypothesis: the record describes its runs and decides nothing.

## Facts

- Loom commit 5af3f9c2 (b10x-loom 0.58.0), binary sha256 074a9e8691addf521b8d843c796bcc363eb42a6ca9cea8359b65b33f083b49e2; it names every-printed-decision-command-runs and person-action-records-a-named-result (the two conditions of the approval).
- The model login is named at start time only: sdc-experiment run parity-loom-strong --login-file <login file>; no record, log or report names its path.
- The spend of the three runs is booked through the shared ledger runs/parity-loom-strong.loom-spend.json; cap 20.00 USD, stop line 19.00 USD.

## Links

- experiments/release-0.1.2.yaml: the three driver runs of the strong setting this record is compared with
- story:parity-run-on-loom: the tolerance and the acceptance of the evaluation run
- approval-record:operator-decision-parity-runs-spend-2026-11-22 and approval-record:operator-decision-parity-runs-unconfined-2026-11-22: the approval of the runs, their cap and their conditions
