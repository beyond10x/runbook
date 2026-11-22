# parity-loom-strong-2: The to-do idea in the strong setting through Loom, against the three driver runs of release-0.1.2 (second record, replaces parity-loom-strong)

Written by `sdc-experiment report parity-loom-strong-2` from `experiments/parity-loom-strong-2.yaml` and the run summaries in `experiments/parity-loom-strong-2/runs/`; `sdc-experiment check --all` fails when it is stale. Definitions: `docs/workflow-metrics.md`.

- Kind: prospective; status: planned; created 2026-11-22.
- Purpose: Registered hypothesis, written before the runs: the runs through Loom end like the three driver runs of the strong setting of release-0.1.2, within the tolerance written in story:parity-run-on-loom ("Tolerance, written before the Loom runs"). It is the evaluation run of that story. This record replaces parity-loom-strong, whose three runs ended in their first model step before any model call (the metaharness adapter found no login under Loom); that record stays as the evidence of the attempt and is not edited.
- Hypothesis: none; the record describes its runs.
- Input `todo-app` (idea, sha256 f384a752a2cb): A to-do list web app for me and my partner that also works on the train without a connection
- Tree: `ee6b50bd455ac0b710bf8ce856396cc1f27d2a8c`
- Runs per variant: 3; pass rule: clean-delivery (closed, delivered, no reopen, forced step or unstick).
- Decision rule (descriptive): Hypothesis: the runs through Loom end like the three driver runs of the strong setting of release-0.1.2, within the tolerance of story:parity-run-on-loom. The record has one variant, so sdc-experiment reports it as descriptive and decides nothing; the verdict is read against the tolerance in the story, run by run: each quality measure of a Loom run lies within the range of the three driver runs (experiments/release-0.1.2/report.md, strong), and the members the story lists as equal are equal. Every end counts; no run is repaired, reopened or forced. A run that falls outside is reported as such. This record replaces parity-loom-strong, abandoned after its three runs ended before any model call.

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
| parity-loom-strong-2.strong.1 | strong | no summary: not ended | - | - | - | - |  |
| parity-loom-strong-2.strong.2 | strong | no summary: not ended | - | - | - | - |  |
| parity-loom-strong-2.strong.3 | strong | no summary: not ended | - | - | - | - |  |

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
- The model login is named at start time only: sdc-experiment run parity-loom-strong-2 --login-file <login file>; no record, log or report names its path.
- The spend of the three runs is booked through the shared ledger runs/parity-loom-strong-2.loom-spend.json (one ledger per record id, so it starts empty); cap 20.00 USD, stop line 19.00 USD. The ledger of the replaced record booked 6.00 USD of reservations for steps of unknown cost and no money was spent, so the approved cap of 20.00 USD has not been used.
- The tree is the fix: the metaharness adapter gives metaharness the login in a private HOME for its call (and the claude-cli adapter its config home) in the private directory of the runner, never in the run directory, and a model call that fails without a cost is booked at its reservation and the run goes on, with a reason that names the action, the adapter, the exit and the first line of the adapter stderr.

## Links

- experiments/parity-loom-strong.yaml: the record this one replaces; its three runs ended in idea.dissect before any model call
- experiments/release-0.1.2.yaml: the three driver runs of the strong setting this record is compared with
- story:parity-run-on-loom: the tolerance and the acceptance of the evaluation run
- approval-record:operator-decision-parity-runs-spend-2026-11-22 and approval-record:operator-decision-parity-runs-unconfined-2026-11-22: the approval of the runs, their cap and their conditions
