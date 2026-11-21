# SDC KPIs and the baseline suite

Purpose (the operator, 2026-11-20): clear KPIs, and a standard suite that runs at any time. The effect of
any change (protocol, binding, model, adapter, technique) then shows as a KPI delta against a
baseline. Running the eval and seeing what changed is one command. Formulas and statistics are in
`docs/eval-design.md` (cited as `eval-design § n`).

Read on 2026-11-20. `path:line` citations as in `eval-design.md`. *(inferred)* marks my
construction.

## 0. What exists to build on

- **takt's scorecard** is the organisation's existing baseline instrument.
  - `takt-scorecard/1` holds one row per case run with outcome, steps, tokens, refusals,
    `no_progress`, errors and `wall_ms`, and a version per metric definition.
  - It carries a comparison `rule: {trials, margin}`.
  - The outcome comes from a held-out check, never from the run's own record.
  - A scenario is worse when its outcome shares fall short by more than the margin, or when its
    median tokens exceed the saved maximum. A side with fewer cases than `trials` gets no verdict.
  - Sources: `takt/crates/takt/src/scorecard.rs:1-37`; a saved one,
    `takt/docs/runs/2026-10-19-improvement-turns-2-and-3/promotion/scorecard-44f96a48….json`.
  - Promotion happens only if no scenario regresses (`takt/README.md:36`).
- **takt's loop metrics** declare each metric with a version, a definition and an optional
  `target: {at_least | at_most}`, as `format: takt-loop-metrics/1`
  (`takt/protocols/loop-improvement/metrics.yaml:1-23`).
- **ADR 0101**: "takt's scenarios and scorecard stay the measuring instrument". A trial records
  the revision of each layer and attributes cost and failures by layer
  (Atlas ADR 0101, Decision 1-2).
- **ADR 0094**: protocols live in engineering-protocols; "Runtime-specific data that is not a
  protocol (takt's `loop-improvement/metrics.yaml` …) stays in the runtime" (Decision 2). So
  workflow KPIs are not protocol fields. Its Consequences record `self.improvement/1` at
  5,971,968 states, over Canon's bound.
- **Atlas O6**: the system proposes improvements, lands them and measures them again; landing
  is still owed (Atlas's roadmap).
- **bench** compares harnesses (O3). It computes no scalar score in its fact table, and a score is
  a pre-registered projection (the private bench repository). A KPI here is such a projection.

## 1. Catalogue

Columns:

- **dir**: ↑ higher is better, ↓ lower is better, = must equal the target.
- **gate**:
  - `gate`: blocks a merge on any worsening;
  - `ratchet`: blocks a merge when worse than the baseline by more than the margin, and moves the
    baseline when better;
  - `advisory`: printed, never blocks;
  - `n/a`: informational.

A KPI from LLM runs gates only once its cell has at least `trials` runs on both sides (takt's
rule). Below that it is printed as "no verdict".

### 1a. Structural: from the protocol alone (Canon, pure, no clock, no run)

| id | KPI | formula | source | dir | gate | today |
|---|---|---|---|---|---|---|
| S1 | state space | product of dimension radices | `canon check` last line `N states` | ↓ | ratchet; hard limit 65,536 (Canon refuses above) | yes |
| S2 | headroom | `65,536 − S1` | derived | ↑ | advisory | yes |
| S3 | unreachable outcomes | count of `unreachable-outcome` findings | `canon check` findings | = 0 | gate | yes |
| S4 | dead actions | actions whose precondition holds in no state; with simulate, admissible in no reachable state | `canon check`; `canon simulate` `dead-action` | = 0 | gate | check: yes; simulate: drafted |
| S5 | unproduced evidence | claims or outcomes reading evidence no action produces | `canon check` | = 0 | gate | yes |
| S6 | authority bypass | `authority-bypass` findings | `canon check` | = 0 | gate | yes |
| S7 | failing declared properties | count | `canon check` | = 0 | gate | yes |
| S8 | witnesses | scenarios per outcome and blocked action | `canon generate` file count | n/a | n/a | yes |
| S9 | reachable states | `|V|` of the walk | `canon simulate` summary line | ↓ | ratchet | drafted (`canon-simulate.md:68`) |
| S10 | terminal outcomes | terminable states per outcome; `U_out` | `canon simulate` | n/a | n/a | drafted |
| S11 | stuck states | `stuck-state` findings | `canon simulate` | = 0 | gate | drafted |
| S12 | cycles | `cycle` findings | `canon simulate` | = 0 | gate | drafted |
| S13 | branching per action | `b_res(a)` per action; report the max and the sum | Canon static numbers (02 § 4 item 2) | ↓ | advisory | proposed |
| S14 | LLM edges on the longest path | `ND_max` (02 P8) | `canon simulate` + `author`/`decided_by` | ↓ | ratchet | proposed |
| S15 | replay universes | `U_replay` (02 P9), `log2` | same | ↓ | advisory | proposed |
| S16 | unchecked judgement | `unchecked-judgement` + `self-judged-evidence` findings | `canon validate`/`check` (`loom-sdk-and-agent-node.md:163-164`) | = 0 | gate | proposed |

### 1b. Run: generic, any workflow (run logs plus an eval harness)

`N` = runs in the cell. Rates are reported with counts and a Wilson interval (eval-design § 2).

| id | KPI | formula | source (run-log field) | dir | gate |
|---|---|---|---|---|---|
| R1 | verified success rate | runs closed through the success outcome with every hidden oracle check `held` / N | `terminated.outcome` in `events.jsonl`; `oracle.json` | ↑ | ratchet (margin from `rule`) |
| R2 | false-done rate | runs closed through success with an oracle check `contradicted` / runs closed through success | same | ↓ | gate at 0 on the scripted suite; ratchet on LLM suites |
| R3 | regression rate | oracle checks held at open and failing at close / held at open | `oracle.json` before and after | ↓ | ratchet |
| R4 | LLM calls | agent-node calls per run, and Σ `num_turns` | `calls.jsonl` lines, `num_turns` (landed, `llm-node.sh:262-270`); `action_started.llm` | ↓ | advisory |
| R5 | tokens | median input+output tokens per run | `calls.jsonl` `usage`; Loom metering later (`loom/AGENTS.md:159`) | ↓ | ratchet: median ≤ saved max (takt's rule) |
| R6 | cost per verified success | Σ `cost_usd` / successes; `cost_basis` = provider-reported when `cost_usd` is the adapter's, else unknown | `calls.jsonl` `cost_usd` | ↓ | advisory unless every run is provider-reported |
| R7 | wall time | median seconds, open to verdict | node trace `t_ms`; runner clock outside the case | ↓ | advisory (noisy on a shared host) |
| R8 | human interventions | person actions beyond those the protocol requires: `approve` calls and edits of `work/` that the scripted person did not plan | `authority_recorded`, `action_finished.principal`; the suite's expected approvals | ↓ | ratchet |
| R9 | driver steps | `action_finished` count per run | `events.jsonl` | ↓ | advisory |
| R10 | stuck rate, time to stuck | runs ending `stuck` or `step_budget_spent` / N; median steps to it | verdict events | ↓ | ratchet |
| R11 | LLM edges on the path | judgement action steps plus LLM chooser decisions per run | `action_finished.action` ∈ judgement set | ↓ | advisory |
| R12 | distinct tool sequences | `D/N` over abstract call sequences | `calls.jsonl` `tool_calls[].name`, `operations`, `input` | ↓ | advisory |
| R13 | observed branching | mean `b_obs(q)` over places | same | ↓ | advisory |
| R14 | path entropy | `Ĥ`, Miller-Madow corrected | same | ↓ | advisory |
| R15 | discarded observations | `discarded` events per run | `events.jsonl` | ↓ | gate at 0 on stub runs |
| R16 | replay match | cases whose state and decision digests match / cases | `sdc-drive replay` | = 1 | gate |
| R17 | adversary assertions held | variants whose assertion holds / variants | `eval/runs/summary.tsv` | = all | gate |

### 1c. Workflow-specific: declared per protocol or per trial project

Declared next to the trial project, not in the protocol (ADR 0094, Decision 2), in the shape of
`takt-loop-metrics/1`: `format: sdc-kpis/1`, `metrics: {<id>: {version, definition, formula,
source, direction, gate, target}}` *(inferred format)*. SDC's:

| id | KPI | formula | source | dir | gate |
|---|---|---|---|---|---|
| W1 | ESS gate first-pass rate | spec revisions written by `spec.author` whose `spec_validation`, `spec_compile`, `open_markers` and `spec_synthesis` records at that revision are all passing results / spec revisions written by `spec.author` | `evidence/*.json` (`kind`, `result`, `subject_revision`) joined to `revised` events | ↑ | ratchet |
| W2 | spec revisions to gate | `revised` events of `spec` from case open until `spec.complete` is true | `revised` events; `evaluations/*.json` claim `spec.complete` | ↓ | ratchet |
| W3 | story conformance pass rate | `conformance_run` records `passed` at the first `change` revision / stories. Valid only once `tests.run` tests `impl/` (eval-design D1) | evidence records | ↑ | ratchet after D1 |
| W4 | review rejection rate | `spec_review` + `code_review` `rejected` / reviews | evidence records | n/a | advisory |
| W5 | intake questions for a person | `questions.check` exit 3 share | `action_finished.exit` | n/a | advisory |
| W6 | plan coverage first pass | `plan.cover` `covered` at the first `plan` revision / plans | evidence records | ↑ | ratchet |
| W7 | spec oracle | `eval/oracle-spec.sh` `held` / spec cases passed | oracle output | ↑ | ratchet |

### 1d. Decisions: person, rule, decider (2026-11-21)

Who took each decision that waits on a person (approvals, scope-changing questions, the root's
close). `SDC_DECIDER` in `scripts/run-trial.sh`; principal `decider:approve` or `decider:reject`
is a rule, `decider:haiku` an LLM decider, any other principal a person.

| id | KPI | formula | source | dir | gate |
|---|---|---|---|---|---|
| H1 | decisions by who | count per class (person, rule, decider) of approvals, answers and closes | `authority_recorded.principal`, `action_finished.principal` (manual actions) in `events.jsonl`; `answered_by` in `questions.yaml`; `runs/<run>/decisions.jsonl` `principal` | n/a | advisory |
| H2 | decider escalations | `decisions.jsonl` lines with `decision` `escalate` or `escalation_suppressed` / decider decisions; invalid replies (`valid: false`, counted as escalate) reported apart | `decisions.jsonl` | ↓ | advisory |
| H3 | decider decisions later reversed | decider decisions followed by a decision on the same case and action (or question) by another principal with a different result, or by a person's rewrite of the approved artifact (`context.change`, an edit of an answered question) / decider decisions | `decisions.jsonl` joined to later `action_finished.principal` and `revised` events of the subject | ↓ | ratchet once runs exist |
| H4 | decider cost | Σ `cost_usd`, Σ `tokens` of decider calls per run | `decisions.jsonl` | ↓ | advisory |

## 2. Ownership

| Scope | Who computes | Needs | Today |
|---|---|---|---|
| 1a structural | **Canon, purely**: a function of `protocol/1` with no clock, no evidence and no run (`canon/AGENTS.md:22-26` via eval-design). S13-S16 need Canon changes: `author`/`decided_by`, static per-action numbers, `canon simulate` counts (02 § 4) | the protocol file | S1-S8 from `canon check`/`generate` |
| 1b run | **the runtime writes the record** (`sdc-drive` events today, Loom's metering and session observer later, `loom/AGENTS.md:152, 159`); **an eval harness reads records and oracles** and computes the KPIs. Canon never reads run data (`boundary.md:25, 70` via 02 § 4 item 5) | run logs, node traces, oracle files | R9, R10, R15-R17 from `sdc-drive` events; R1-R3 for the scripted suite; R4-R7 and R12-R14 need the node trace (eval-design D3) |
| 1c workflow | **the trial project declares them**; the harness computes them from evidence records and events | per-project `sdc-kpis/1` | W1, W2, W4-W7 computable from today's case store; W3 after D1 |

**Home of the harness.** Options are in eval-design D13. Recommendation *(inferred)*:

- now, the runner stays in `sdc/eval/`;
- the scorecard and baseline use takt's `takt-scorecard/1` shape and its comparison rule
  (`{trials, margin}`, no verdict below `trials`), so the suite can move under ADR 0101's
  instrument without a format change;
- bench stays the place for cross-harness facts (`aep eval matrix`).

## 3. Baseline suite

### 3.1 Contents

| Suite | Part | What runs | Cost | Cadence |
|---|---|---|---|---|
| global | structural | S1-S8 for every `protocol/*.protocol.yaml` (`canon check`, `canon generate`); S9-S16 once Canon has them | $0; spec generate takes over 120 s | every change to a protocol; pre-merge gate |
| global | scripted | the adversary suite of `eval/run-adversaries.sh` (10 variants); `sdc-drive replay` per case tree *(not yet in `eval/run.sh`)* | $0, about 30 s (`eval/runs`) | every change to protocol, driver or bindings; pre-merge gate |
| SDC | scripted trial ideas | the stub full cycle per trial idea. Today one idea, `driver/examples/sdc/fixtures/idea.yaml` (billing); add ideas whose spec has a reference target (eval-design D6) | $0 | as above |
| SDC | LLM trial runs | per trial idea: `spec.author` at L2 (and the node's current production level) × M1 × 5 runs, through the adapter | ≤ $0.50 per run by cap: 5 ideas × 5 runs ≤ $12.50 | nightly, and on demand before merging a change to a binding, prompt, model or adapter |
| per project | the same three parts over the project's protocols, ideas and `sdc-kpis/1` | | | the project's choice |

Each run of the suite records the revision of every layer: protocols' digests, `canon`,
`sdc-drive`, `ess`, `aep`, adapter, model id (ADR 0101, Decision 1). `eval/run-adversaries.sh`
already writes the bindings, `sdc-drive` and `canon` digests per variant (`bindings.sha256`).

### 3.2 Baseline file

`eval/baseline.json` *(inferred format, takt-shaped)*:

```json
{
  "format": "sdc-eval-baseline/1",
  "suite": "sdc",
  "layers": {"canon": "<sha256>", "sdc-drive": "<sha256>", "protocols": {"sdc.spec": "<ir digest>"},
             "ess": "<version>", "aep": "<version>", "adapter": "<id@version>", "model": "claude-haiku-5-5"},
  "rule": {"trials": 5, "margin": 1},
  "kpis": {
    "S1.sdc.spec": {"value": 65536, "dir": "down", "gate": "ratchet", "version": 1},
    "R17":         {"value": 9, "of": 10, "dir": "eq", "gate": "gate", "version": 1},
    "W1.billing":  {"value": 0.6, "n": 5, "dir": "up", "gate": "ratchet", "version": 1}
  }
}
```

Rules:

- **Ratchet.** A `ratchet` KPI that is better than the baseline is written back, but only by
  `--ratchet`, and the changed file is committed with the change that earned it. Worse by more
  than the margin fails the suite.
- **Definition changes.** A KPI whose `version` changed is printed both ways and never gates
  (takt's rule, `scorecard.rs:30-33`).
- **Known failures.** R17's baseline today is 9 of 10. The `story-readme` false done is a known
  defect, recorded as such, not hidden. When D1 is fixed the ratchet moves to 10 of 10.

The built `eval/run.sh` writes a simpler form than the example above: `layers` and `kpis` with
`value`, `dir` and `gate`. It has no `rule` and no `version` yet, because every KPI it computes is
deterministic, so one run is a verdict. Both join with the LLM part.

### 3.3 Delta report: one command

**Built** for the free part: structural S1 and the S3-S7 findings for all 8 protocols, plus R2,
R9, R10 and R17 from the scripted suite. The three layers are pinned together, because the binding
format, the driver and the protocols change together (eval-design § 4.5, re-runs):

```console
$ EXAMPLES=<pinned driver/examples/sdc> DRIVE=<pinned sdc-drive> PROTOCOLS=<pinned protocol/> \
    slot eval/run.sh <scratch> [--against eval/baseline.json] [--ratchet]
```

- It writes `eval/reports/<UTC>.json` (format `sdc-eval-baseline/1`: `layers`, then per KPI
  `value`, `dir`, `gate`), `<UTC>.txt` (the delta table) and `<UTC>-runs/` (the scripted suite's
  per-variant `summary.json`, `events/`, `run.log`).
- Exit codes: 0 nothing worse; 1 a gate or ratchet KPI is worse (`FAIL` on its line); 2 the suite
  could not run.
- `--ratchet` writes the candidate to `eval/baseline.json` when nothing is worse.
- Wall time is 4-5 minutes on this shared host, mostly `canon check` of the 65,536-state
  protocols. It runs under `slot`, which may queue it.

Observed deltas (local time 2026-11-21; reports in `eval/reports/`):

- **Protocol change, caught as a delta.** Seen by comparing the adversary runs'
  `summary.tsv`, before `eval/run.sh` existed with a baseline. Before: 68 driver steps (23:45
  driver, `sdc.direction` r2). After: 65, once `sdc.direction` r3 added two stops. R9 −3, traced to
  two `context.default` re-runs and one `vision.write` re-run that no longer happen.
- **Driver and bindings change, same KPIs.** `20261120T232046Z.txt`: `layers changed: sdc-drive,
  bindings`, every KPI `same`, exit 0. Behaviour still changed: spec adversaries stall 4 runs, not
  8. The KPI set could not see it, so R10 was added.
- **Current baseline.** `eval/baseline.json`, written by `--ratchet` from `20261120T232814Z`:
  - pins: `sdc-drive` `0e667d8c…`, bindings `476ee9e4…`, protocols `6e5c84d8…`, `ess 0.96.0`,
    `aep 0.80.0`;
  - S1: 4,608 (sdc) to 65,536 per protocol, 0 findings each;
  - R17: 9 of 10 (the known `story-readme` false done);
  - R9: 65;
  - R10: 4 runs to stuck for each spec adversary, 1 for `spec-overreach`, 3 for `story-noop`.

Not built yet *(inferred next steps)*:

- the LLM part (`--llm`): the pilot variants `llm-L*` of `eval/run-adversaries.sh`, N per cell,
  with takt's `{trials, margin}` rule and the W-KPIs;
- `canon generate` counts (S8);
- the per-project suite file `sdc-kpis/1`.

The layer line is ADR 0101's attribution: a delta is read against what changed.

## 4. Late findings and human interaction (added 2026-11-21, for trial 2)

New section; nothing above is changed. Goal (the operator, 2026-11-21): fewer human interventions and
longer autonomous runs per task, measured on the late-finding loop of `docs/trial-2.md`.
`driver/target/debug/sdc-finding-kpis <run dir> [--json]` (source `driver/src/bin/sdc-finding-kpis.rs`)
prints L1 to L7 for one run. Every value is per task (one idea, one case tree). Sources are the run directory that
`run-trial.sh` or `run-full-cycle.sh` writes: `store/IDEA-1/**/events.jsonl` (the driver's log,
field names as written: `event`, `action`, `principal`, `artifacts`), `store/IDEA-1/work/`
(`findings.yaml`, `questions.yaml`, `context.yaml`) and `rounds.jsonl` (written by
`scripts/run-trial.sh`, one line per round).

A *person* is a `person:<id>` principal (2026-11-21: written only where a person acted). Runs
recorded before principals were typed name a person by a bare id (`operator`, a login) or `human`,
and those still count; `decider:*`, `rule:*`, `agent:*`, `non-interactive`, `derived`, `dry-run`
and `operator-step` never do. A decider's decisions are counted by R19 to R25
(`autonomy-levels.md` § 5.1), not here.

Eval numbers come from autopilot runs only (`docs/eval-design.md` § 7). A run in which a script or
a session stands in for the person is an end-to-end test of the person path: its run settings say
`stand_in: true` with `stand_in_by`, L1 repeats that (`stand_in`, `stand_in_by`), and its numbers
are not eval numbers.

| id | KPI | formula | run-log field | dir | gate |
|---|---|---|---|---|---|
| L1 | human interventions per task | person actions + person answers. Person actions = `action_finished` events with a person `principal` (approvals, `scope.answer`) + `authority_recorded` events with a person `principal` not taken with an action at the same `at`. Person answers = scope-changing questions answered by a person + findings with `decided_by: human` + findings with `raised_by: human` + context fields with `status: changed` or `accepted` | `events.jsonl` `action_finished.principal`, `authority_recorded.principal`; `questions.yaml` `answered_by`, `status`; `findings.yaml` `decided_by`, `raised_by`; `context.yaml` `<field>.status` | ↓ | ratchet once N ≥ `trials` |
| L2 | autonomous run time between human interventions | per round (one `run` until it stops): `seconds` = `ended_epoch − started_epoch`. Reported: median, max, sum, and share = sum / (last `ended_epoch` − first `started_epoch`), which counts the time spent waiting on a person as not autonomous | `rounds.jsonl` `seconds`, `started_epoch`, `ended_epoch`, `verdict` | ↑ | advisory (shared host, wall clock) |
| L3 | defaults accepted without change | three ratios: findings resolved with `resolution.default_taken: true` / findings resolved; questions `status: accepted` / questions answered; context fields with `status` `default` or `accepted` / context fields | `findings.yaml`, `questions.yaml`, `context.yaml` | ↑ (a high share means the defaults were right, so fewer decisions reach a person) | advisory |
| L4 | late findings per stage | findings not `rejected` whose `stage` is not `requirements` (raised after intake), counted per `stage` (`context`, `vision`, `roadmap`, `spec`, `plan`, `story`) | `findings.yaml` `stage`, `status` | ↓ over time per idea class (more defined up front); a planted finding must appear (trial 2) | advisory |
| L5 | findings caught by program vs person | count by the prefix of `raised_by`: `program:<rule>`, `llm:<node>` (accepted by the program check), `human`; plus `rejected` (an LLM finding the program check refused) and resolved by `agent` vs `human` | `findings.yaml` `raised_by`, `status`, `decided_by` | program share ↑, human ↓ | advisory |
| L6 | rework per artifact | per artifact: distinct non-`none` revisions over the whole tree − 1; reported per artifact and summed | `events.jsonl` `revised.artifacts` | ↓ | advisory |
| L7 | time waiting on a person (added 2026-11-21) | Σ over the stops after which the run did not go on by itself: from the round's `ended_epoch` to the next round's `started_epoch` when a new leg began between them (a `run-settings.jsonl` line: the script had ended and was started again); for the last round of a run not closed (no `decision.json`), to the stop record's `at` (`run-stopped.json`), else to now (`ongoing`). Reported with each stop (verdict, case, seconds, `resumed`, `stopped` or `ongoing`). Without `run-settings.jsonl`, rounds ending `awaiting_person` or `awaiting_authority` count | `rounds.jsonl`, `run-settings.jsonl` `at`, `decider.kind`, `run-stopped.json` `at`, `decision.json` | ↓ | at A3 and up (the last leg's decider is not `person`) a value above 0 is a failure (`failure: true`, printed `FAILED`); at A0 to A2 advisory |
| stopped_for_fix | time stopped for a fix, reopens (added 2026-11-21) | a gap between rounds that a reopened stop record (`run-stopped.<at>.json`) lies in is not L7: Σ from each such record to the next leg (`seconds`), and `reopens`, the number of those records. `forced` (keep-going, `defects.jsonl` lines with `forced: true`) and `unstuck` (run.unstick, lines with `unstuck: true`, each with its move, defect class and diagnosis) beside it; `clean_run` is true only with no forced continuation, no unstick move and no reopen | `rounds.jsonl`, `run-stopped.<at>.json`, `defects.jsonl` | ↓ | a run with a reopen, a forced continuation or an unstick move is not a clean run |

Notes:

- L1 replaces nothing: R8 counts unplanned person actions against a suite's expected approvals;
  L1 counts all of them, so a run that needs no unplanned action still shows its required ones.
  The required floor in the stub cycle is 5 (context, vision and roadmap approvals, one merge,
  one release); an idea with scope-changing questions adds 1 (`scope.answer`).
- L2 needs a real clock. Scripted runs pass a fixed `--at` per round and write no `rounds.jsonl`,
  so L2 is empty for them. The driver's `at` cannot be used: one `run` takes one `--at` for every
  step (driver gap 10).
- L4 and L5 read the final `findings.yaml`. A finding is never deleted: resolved, dismissed and
  rejected ones stay, so the counts are over the whole run.
- Measured values (stub runs, 2026-11-21): see `docs/trial-2.md`, "Verification".
- `sdc-run-compare` reads two or more run directories and writes one comparison report of these figures and the run's cost, time and end state (`docs/run-compare.md`).

## 5. Flow (multi-item runs) (added 2026-11-21, decided)

The operator, 2026-11-21: for orchestration and multi-item protocols, concurrent unfinished work, open
work trees and in-progress AEP items must stay within a healthy amount. Agents that dispatch all
the time and finish nothing end with 100+ work trees, and a session must reconcile all that state
before it can start. Parallelism must still speed the run up; the sweet spot is tuned during a
run or chosen by eval. The limits that hold these KPIs are enforced by the admission program of
`sdc.orchestrate` (`docs/orchestration.md` § 2.4a); the WIP limit is an eval variable (F17).

An *item* is a story child case. *Opened*: its `case_outcome` `open` record in the parent; *closed*:
any terminal outcome; *merged*: its branch merged by `wave.integrate`. Sources of truth: case
events (`events.jsonl` of every case in the tree), the planning store (`aep plan artifact list
--format json`, transitions), `git worktree list --porcelain` of the area's repository, and the
fs-change record (`changes.jsonl`, `docs/fs-changes.md`).

| id | KPI | formula | source of truth | dir | gate |
|---|---|---|---|---|---|
| F1 | in-progress items | child cases opened and not closed: instant value, maximum over the run, and time-weighted mean (Σ count × interval / run time) | case events: `case_outcome` open and terminal records | ≤ `limits.wip.max_in_progress` | gate at admission (§ 2.4a) |
| F2 | in-progress AEP items | stories `active` with no `implemented` transition | planning store | ≤ F1 limit | advisory |
| F3 | open worktrees | worktrees of the area's repository other than the main checkout | `git worktree list --porcelain` | ≤ `limits.wip.max_open_worktrees` | gate at admission |
| F4 | orphan worktrees | open worktrees whose story case is closed or does not exist | `git worktree list` joined to case events | = 0 | gate; each orphan is a finding |
| F5 | finished but unmerged | count of items closed `passed` whose branch is not merged, and the age of the oldest | case events; `wave_result` records | count ≤ `limits.wip.max_unmerged` | gate at admission |
| F6 | oldest in-progress item | age of the oldest open item at the instant | case events | ≤ `limits.wip.max_item_age` | gate at admission |
| F7 | throughput | items closed per hour of run time | case events | ↑ | advisory |
| F8 | cycle time | open → merged per item, p50 and p90 | case events; `wave_result` | ↓ | ratchet |
| F9 | flow efficiency | per item: time with an action in flight (`action_started` to `action_finished`) / cycle time; reported as the median | case events | ↑ | advisory |
| F10 | finish ratio | items closed / items opened, per window (one wave, and the whole run) | case events | ≥ 1 once the run drains | advisory |
| F11 | overlap rework | merge conflicts at `wave.integrate` plus rework actions (re-runs of `change.implement` after an integrate failure) caused by overlap | `wave_result` records; case events; intake entries filed by `wave.integrate` | ↓ | ratchet |
| F12 | reconcile cost of a `continue` | seconds and actions from the start of a `continue` to the first new action | case events of the `continue` round | = 0 (the stage is derived from facts, `docs/phases.md:3-6`) | gate |
| F13 | cost per closed item | Σ `cost_usd` of the run / items closed | `calls.jsonl`, `decisions.jsonl`; case events | ↓ | advisory |
| F14 | speedup | wall time of the sequential run of the same suite / wall time of this run | two runs' case events | ↑ | advisory (shared host) |
| F15 | parallel efficiency | F14 / time-weighted mean of F1 | derived | ↑ | advisory |
| F16 | person decisions per closed item | person `authority_recorded` and person `action_finished` events (as L1) / items closed | case events | ↓ | ratchet |
| F17 | WIP sweep | F1 to F16 per value of `limits.wip.max_in_progress` (for example 1, 2, 4, 8) over the standard suite; the setting with the best F7 and F8 at F4 = 0 and F12 = 0 becomes the default per level | eval reports | n/a | picks the default |

Notes:

- F17 adds the WIP limit as one more factor of an eval cell, beside node, level, adapter and model
  (`docs/eval-design.md:436`); the chosen defaults go with the other per-level limit defaults
  (`docs/autonomy-levels.md:358-375`).
- A limit change during a run is the event `limits_changed` (who, old, new), taken by a person or
  a rule and read at the next admission; it is never retroactive, and no LLM raises a limit
  (`docs/autonomy-levels.md:271-272`). The KPIs report each interval under the limit then in force.
