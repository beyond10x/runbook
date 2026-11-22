# Workflow metrics and A/B experiments

The operator, 2026-11-21: "During that testing we need a way to assess objectively the quality of
the workflow. Besides total duration: retries or reattempts or looping factor… these metrics are
also local in some way, on the specific nodes or edges. […] we form a hypothesis, make an adjusted
change and run an A/B test, see whether it's true or not, and then adopt the change." No LLM judges
anything here: programs read the records.

- `sdc-run-metrics <run dir> [--json <file>|-] [--text <file>|-]`: one run, `sdc.run-metrics/1`
  (`driver/src/bin/sdc-run-metrics.rs`, shared code `driver/src/run_metrics.rs`).
- `sdc-experiment compare|check|report|index|run|status|stop`: groups of runs, and the experiment
  records that launch and account for every real run (`driver/src/bin/sdc-experiment.rs`; records
  in `experiments/<id>.yaml`, listed in `experiments/INDEX.md`).
- Tests: `driver/tests/run_metrics.rs`. Stub cycle `run-metrics` in `scripts/run-finding-cycles.sh`.

Cite format `path:line`. *(proposed)* marks a rule this document introduces and nobody has
approved yet.

## 1. Units

- **Node**: `(stage, action)`. The stage is the case's protocol id without `sdc.` (`sdc` is
  `idea`), read from the `opened` event.
- **Case instance**: one case directory of the tree (`store/IDEA-1/`, or a bug-fix run's
  `store/BUG-1/`, then `children/<id>/`, recursively; `run_metrics.rs:710`).
- **Take**: one `action_started` event, with its `action_finished` (`driver/src/main.rs:1404-1411`).
- **Real take**: a take with an `action_finished` that is neither `not_called` (the tool did
  nothing, `docs/driver.md:116-124`) nor could-not-run (exit 75, 126 or 127, `docs/driver.md:146-150`).
- **Node kind** (`run_metrics.rs:283`):
  - `approval`: a `manual` binding;
  - `copy`: a `copy` binding;
  - `llm-writer` or `writer`: IR `effect: write`, with or without `llm` on the take;
  - `check`: any other effect.
- **Negative record**: an evidence result in `failed, finding, invalid, rejected, refused,
  uncovered, unassessed, draft, withheld, not_released`, or a take that exited non-zero
  (`run_metrics.rs:24`, `:299`).

## 2. Node metrics (per case and summed over the cases of a stage)

| Metric | Definition | Read from |
|---|---|---|
| `takings` | takes | `events.jsonl` `action_started` |
| `real_takes`, `not_called`, `could_not_run` | as § 1 | `action_finished` `not_called`, `exit` |
| `instances` | case instances with at least one real take of the node | case tree |
| `looping_factor` (LF) | `real_takes / instances`. The denominator is the number of takes of a run without rework: such a run takes each action once per case instance (one arrival of the tree at that protocol), so LF 1.0 means no rework (`run_metrics.rs:487`) | as above |
| `rework_takes` | `real_takes − instances`: every real take of an action after its first in that case | as above |
| `retries_by_cause` | each rework take gets one cause (`run_metrics.rs:319`). With `retry_of` (the kinds `retry_when` fed back, `driver/src/main.rs:1409`): `A:R`, the action and result of the latest evidence of that kind, e.g. `merge.approve:rejected`. Without it: `arrival:A:R`, the first negative record since the node's previous take, reached through `amend.upstream` or an outside revision. Without either: `decision:<action>`, `arrival:outside`, `arrival:amend.upstream` or `unexplained` | `retry_of`, `evidence/<id>.json` `kind`, `result` |
| `first_pass_yield` (FPY) | the share of instances with exactly one real take: the first output passed every downstream check and approval (`run_metrics.rs:491`) | as above |
| `sent_back` | rework takes of writers that name this node as the source of their cause | edges |
| `driver_s` | Σ over takes of `observed_at(action_finished) − observed_at(action_started)` | `observed.jsonl` (`docs/driver.md:229-232`) |
| `model_s` | Σ `duration_ms / 1000` of the node's model calls | `llm-calls.jsonl` |
| `llm_calls`, `llm_cost_usd`, `tokens` | the call lines of that case and action. `tokens`: input, output, cache_read, cache_creation, thinking (`usage`) | `llm-calls.jsonl` |
| `decider_cost_usd`, `decider_calls` | the decider's lines on that case and action (an approval node) | `decisions.jsonl` |
| `rework_cost_usd`, `rework_driver_s` | the cost and driver seconds of the rework takes. A call is joined to the take whose observed span holds its `started` (`run_metrics.rs:768`) | as above |
| `no_call_takes`, `unmatched_llm_calls` | real model takes with no call joined (an own-default amendment that called no model); call lines with no take | as above |
| `models`, `efforts` | calls per `model` and per `effort`. `default` when the line has `effort: null`, `unrecorded` when it has no field (before `feat/effort-per-class`) | `llm-calls.jsonl` |

LF and FPY answer the question for writers and approvals. A check runs once per snapshot,
including before its subject exists (`docs/driver.md:107-110`). Its LF counts those runs too: read
it as "evaluations per case", not as rework.

## 3. Edges

An edge sends the run back to a writer W: an `llm-writer` or a `writer`, but not `amend.upstream`,
which only applies findings (`run_metrics.rs:858`). Each cause of each rework take of W is one
edge occurrence. A take with k causes counts once on each of its k edges and gives each 1/k of its
loop.

| Kind | Key | Count |
|---|---|---|
| `check` | `<stage>: A:R -> W`, with `retry_of` from a check | rework takes of W with that cause |
| `approval` | `<stage>: A:R -> W`, with the record from a `manual` action or a principal, or a kind ending `_approval` (`merge.approve:rejected -> change.implement`) | as above |
| `arrival` | `<stage>: arrival:… -> W`, with no `retry_of` and no `amend.upstream` in between | as above |
| `finding` | `<stage>: finding@S -> T`. S is the finding's `stage`; T is the protocol stage of what `resolution.amends` names (`r*`, `q*`, `d*`: intake; `context.*`, `vision`, `roadmap`: direction) (`run_metrics.rs:620`, `:638`) | findings not `rejected` with that S and T (`work/findings.yaml`) |

- **Loop** of a rework take (`run_metrics.rs:376`). It starts at the take. It ends at the end of
  the next take of the action that sent W back. Without such an action, or when that take comes
  later, it ends before W's next real take or at the case's last take.
- **`loop_cost_usd`**: the joined model cost of every take inside the loop, plus the decider cost
  of the lines of that case timed inside it.
- **`loop_driver_s`**: Σ `driver_s` of those takes. **`loop_wall_s`**: observed end − start.
- **Finding edges**: the loops of rework takes whose cause is a finding (`amend.upstream` between
  W's takes) are summed per protocol stage, then shared out over that stage's finding edges by
  count (`run_metrics.rs:918`).
- The loops of two writers of one case can overlap. A sum over edges can exceed the run's cost.

## 4. Decision classes

From `decisions.jsonl` (`scripts/run-trial.sh:379-483`). The class is the line's `class`
(`scope_changing_answer`, `finding`, `approval`, `close`), also by class and action
(`run_metrics.rs:556`).

| Metric | Definition |
|---|---|
| `asked` | effective lines: `decision` approve, reject or escalate |
| `approved`, `rejected`, `escalated` | those lines by decision |
| `escalation_suppressed` | `escalation_suppressed` lines (autopilot; the effective line follows with `suppressed: true`) |
| `invalid_replies` | replies the program check refused: `invalid_reply` lines, plus lines from before the retry rule with `valid: false` |
| `retries` | `invalid_reply` lines with `attempt` ≤ 2: the decider was asked again (`scripts/run-trial.sh:431-436`) |
| `refused_by_check` | effective decisions the program took after every reply was refused (`fallback`), plus `valid: false` effective lines from before the retry rule |
| `calls`, `cost_usd` | lines that name a `model` and do not repeat a call (`suppressed`, `fallback`) |
| `invalid_reasons` | `invalid_reason` up to its first colon, counted |

## 5. Stage and run

- **Stage**: the node sums over its cases, and `cases`.
- **Run sums**: `takings`, `real_takes`, `rework_takes`, `not_called`, `cost_usd` (LLM + decider),
  `rework_cost_usd`, `tokens`.
- **`looping_factor`, `first_pass_yield`**: over the `llm-writer` nodes (`run_metrics.rs:966`).
- **`furthest_stage`**: the latest of idea, intake, direction, epic, spec, plan, story, release
  with an opened case. `furthest_stage_passed`: the same over cases terminated `passed`.
- **`end`**:
  - `closed: <outcome>`, from the root's `terminated` event or `status.json`;
  - else `stopped: <cause or verdict_at_stop> in <case>`, from `run-stopped.json`;
  - else `open`. `finished` is false only for `open`.
- **`delivered`**: `status.json` `delivered`, else outcome `operating`.
- **`clean`**: closed, with `reopens`, `forced` and `unstuck` all 0 (`run_metrics.rs:1018`).
  - `reopens`: `run-stopped.<at>.json` files, or `status.json` `reopens`.
  - `forced`: `defects.jsonl` lines with `forced: true`, or `status.json` `forced`.
  - `unstuck`: `status.json` `unstuck` (a number or a list).
  - These fields come from branches in flight (`fix/attempt5-close`). An absent one reads 0.
- **`driver_s`**: Σ `rounds.jsonl` `seconds`. `wall_s`: the first to the last observed event.
- **`waiting_on_person_s`**: the reading of `sdc-run-compare` (`docs/run-compare.md` "Time
  waiting on a person"). A last stop recorded by a `rule:` waits on nobody.
- **`conformance`**: from `oracle.json` `conformance`: `spec scenarios proven passed: k of n (lower
  bound); f failed[, of which p refused for a precondition the specification does not model][; d
  disputed]; u not measurable; tables agree on a of n` (` (held)` only when k = n, or
  `not measured (<cause>), n scenarios` when no table was counted), `not observed` without it;
  `conformance_k`, `conformance_f`, `conformance_p`, `conformance_d`, `conformance_u` (null when not measured), `conformance_n`,
  `conformance_agreement`, `conformance_tables`, `conformance_result` (`held`, `contradicted`,
  `unknown`), `conformance_cause`, `conformance_measured` (`docs/conformance.md`). Not in the
  compared run fields.
- **`settings`**: the last `run-settings.jsonl` line, flattened to dotted keys (`decider.model`,
  `models.implement.model`, `efforts.implement.effort`).

## 6. Groups of runs

- **Groups**: `sdc-experiment compare --group a=<dir>,… --group b=…`, or `--by <settings key>
  --runs <dir>,…`.
- **Figures**: per metric, each group's median, min, max and n, and the median difference
  against the first group.
- **Overlap**: when the two [min, max] ranges intersect, no effect is claimed. When they are
  disjoint, the report prints `separate` with p = 2 / C(n1 + n2, n1). That is the exact two-sided
  chance of so complete a separation when the runs are exchangeable.
- **What n counts** (`run_metrics.rs:1130`). Runs that are not finished are excluded unless
  `--include-open` is given. n counts only runs that have the metric:
  - a node: runs that took it;
  - an edge: runs that took its target node, where a missing edge reads 0;
  - a finding edge: runs that opened a case of its stage;
  - a decision field: runs with a decision log.

**When a difference is real** *(proposed)*. `docs/eval-design.md` § 5.1 calls a difference null
when its 95 % interval covers 0. With 3 to 10 runs per group no interval method fits without
assumptions, so the rule here is complete separation:

- n ≥ 3 per variant;
- the direction is written down before the runs;
- every treatment run lies beyond every baseline run in that direction;
- the medians differ by at least the size the hypothesis names.

Under exchangeability the chance of that separation is 1 / C(2n, n) one-sided: 0.05 at n = 3,
0.014 at n = 4. The operator's "three times" is therefore the smallest n at which the rule can
hold at all. Two-sided, or without a written direction, n = 4 is needed (2 / 70 = 0.029). A
retrospective record, whose hypothesis was written after the runs, never decides: it proposes.

## 7. Experiment records

The operator, 2026-11-21: "make sure to organize these trials in a somewhat scientific and
organized, manageable and scaling manner. It shouldn't end up messy." An experiment record is
therefore the one way a real run is started and accounted for (AGENTS.md, Rules).

`experiments/<id>.yaml`, `sdc.experiment/1` (`sdc-experiment.rs:451`):

- `kind`: `prospective` (written before its runs, launched by `run`) or `retrospective` (registers
  runs that exist; it lists them and renames nothing);
- `status`: `planned`, `running`, `complete` or `abandoned`; `created`, `title`, `purpose` (one
  line);
- `inputs`: each `name`, `kind` (`idea`, `seed` with its `seed` directory, `fixture`), `text` and
  its `sha256`; a prospective record has one;
- `tree_commit`: the commit the runs use (40 characters in a prospective record; `null` only while
  it is planned); `several` with `trees`, one line per tree, in a retrospective one;
- `fixed` and `variants` (`id`, `runner`: `sdc`, `free-agent` or `loom` (§ 7.1), `settings`,
  `runs`): full run settings, the dotted keys of
  `run-settings.jsonl` plus `decider.adapter`, `review.adapter`, `flags.dry` and
  `flags.recorded` (`:361`); a variant's settings are `fixed` with its own over them. A run entry
  is its directory name, or `{run, input, tree, note}`;
- `runs_per_variant`, `pass_rule: clean-delivery` (closed, delivered, no reopen, forced step or
  unstick, `:863`), `decision_rule`: `complete-separation` with a hypothesis, `descriptive`
  without;
- with a `hypothesis` (one sentence naming the `target`): `variable` (the one key the variants
  differ in), `baseline`, `treatment`, `primary`, `guards`, `adoption` (`:615`);
- `known_confounds`, `facts`, `links`; `launch` (`max_parallel`, `stagger_s`, `principal`);
- `result`, written by `report` (`:882`): per variant runs listed, ended and clean passes; with a
  hypothesis the verdict (`held`, `contradicted`, `inconclusive`), the guards and a recommendation
  (`run prospectively` for a retrospective record that held: it never decides); without one
  `descriptive`.

Files beside the record, all written by programs:

- `experiments/<id>/runs/<run>.json`: per ended run, `sdc.run-metrics/1` without `cases` and
  with every string that holds an absolute path replaced (`:725`). Metrics hold no prompt and no
  transcript. `report` rewrites them from `runs/` when the directory is there, and removes the
  summary of a run that is open again (`:767`); without `runs/` it reads them as they are.
- `experiments/<id>/report.md` (`:1045`): inputs, trees, settings per variant, each run's end,
  clean pass, cost and wall time, per variant median [min, max] (n) of the run figures, the three
  loop nodes and edges, the verdict, facts and links.
- `experiments/INDEX.md` (`:1172`): one row per record with date, purpose, tree, variants × runs,
  status and verdict.
- `experiment.json` in each launched run directory (`:1486`, `sdc.experiment-run/1`): `experiment`,
  `variant`, `ordinal`, `run`, `tree_commit`, `input_sha256`, `settings_sha256` (sha256 of the
  variant's settings as sorted-key JSON), `started_at`, `retrospective` (false). Retrospective runs
  have none: their record maps them.

Commands (`sdc-experiment.rs`):

- `check <id> [--runs-dir runs]`: the record's fields; every listed run's recorded settings
  against its variant, read from its summary, else its run directory (`:817`). `check --all`
  (`:1195`, run by `scripts/check-bindings.sh`) also fails on a stale `report.md`, a recorded
  verdict the summaries do not give, a summary of an unlisted run, an absolute path in a summary,
  and a stale `INDEX.md`.
- `report <id> [--runs-dir runs]`: summaries, `result`, `report.md`, `INDEX.md`; a prospective
  record whose variants all have `runs_per_variant` ended runs becomes `complete`. `index`
  rewrites `INDEX.md` alone.
- `run <id>` (`:1650`) refuses a record that fails its check, a retrospective one, and one with no
  pinned commit. It builds the commit once in `<builds>/<commit>` (default
  `$XDG_STATE_HOME/sdc-experiment/builds`; a shared clone, `driver`, `goals` and `watch` built;
  `<commit>.built.json` beside it), refuses that directory at another commit or with changes, and
  never builds it again; an unbuilt one that runs already used is refused (`:1546`). It starts
  variants × n as transient systemd user units `sdc-exp-<id>-<variant>-<n>`, ordinal 1 of every
  variant first, at most `max_parallel` at once (default 3), `stagger_s` apart (default 30). Each
  unit runs `env -i` with the login's base variables (`HOME`, `PATH`, `USER`, `LANG`, `XDG_*`,
  `DBUS_SESSION_BUS_ADDRESS`; never an `SDC_` one, `:390`), `SDC_RUNS_DIR`, `SDC_PRINCIPAL` and the
  record's settings (`:1447`), and calls the build's `scripts/run-trial.sh` with `--autopilot`
  (autonomy A5), the run name `<id>.<variant>.<n>` and the input (`:1459`). It writes
  `experiment.json` once run-trial.sh has made the run directory, adds the run to the record and
  sets `running`. Started again, it skips runs that have `experiment.json`. `--print` shows the
  units, variables and commands and starts nothing.
- `progress [--experiments <dir>]`: the progress ledger, `docs/progress.md`, `docs/progress.json`
  and `docs/img/progress.svg`, from the release table `experiments/releases.yaml` and the records
  it names (`sdc-experiment/progress.rs`, the image `sdc-experiment/chart.rs`). `report` writes it
  too; `check --all` fails on a stale one. Each run in `docs/progress.json` carries `metrics`:
  the run figures of its summary, `merge_rejections`, `review_rejections` (null for a stub
  review) and the takes of `spec.author`, `plan.decompose`, `change.implement` and
  `change.review`; a value the summary lacks is null, never 0 (`run_metrics_j`). The loop around
  it: `docs/improving.md`.
- `status <id>`: each run with its unit state, cost, wall seconds and end. `stop <id> --reason
  <text> [--run <name>]`: `run-trial.sh <run> --stop <reason>` for each open run, then the unit is
  stopped; without `--run` the record becomes `abandoned` with `stopped` (at, by, reason). A
  `loom` run is stopped as § 7.1 says.

### 7.1 Runner loom

A variant with `runner: loom` (beside `sdc`, the default, and `free-agent`) starts a model run
through Loom: the unit runs `runbook loom-run` of the record's tree (the `runbook` crate, built from
`tree_commit` beside the driver's crates), which makes one run of the case tree through the
`b10x-loom` binary and writes a run directory. Nothing of `run-trial.sh` takes part, and a `loom`
unit is given no `SDC_` variable.

**Settings.** A prospective `loom` variant sets exactly these (a missing one is refused by its
name, `check` and `run` both); a setting of the old driver is refused for it by name with the reason,
none is ignored silently, and a name that is no setting at all is refused too.

| setting | meaning | becomes |
|---|---|---|
| `loom.bin` | the `b10x-loom` binary, absolute or `~/…` (the launcher's `HOME`; a record holds no home path) | `--loom`; never recorded by a run |
| `loom.sha256` | 64 hex digits; `runbook loom-run` refuses another binary | `--loom-sha256`, `settings.json` |
| `loom.confinement` | `local` or `substrate`; no default | `--confinement`, passed to `tree run` as given |
| `loom.cap_micro_usd` | the spend cap over every run of the record, a whole number ≥ 1 | `--cap-micro-usd`, `settings.json`; the runs share the ledger `<runs-dir>/<record id>.loom-spend.json` |
| `models.writer.model`, `models.implement.model`, `models.review.model`, `models.other.model` | the model of each node class (as for `sdc`) | `settings.json` `models.<class>.model` |
| `decider.model` | the model of the decider that answers person steps | `settings.json` `decider.model`; the principal of those answers is `decider:model` |
| `max_llm_calls` | cap on model calls of one run (as for `sdc`) | `--max-llm-calls` |
| `max_wall_seconds` | wall-time cap of one run (optional for `sdc`, required here) | `--max-wall-seconds` |
| `loom.login_file` | optional: a path (absolute or `~/…`) to the login the model adapter reads | `settings.json` `loom.login_file` only; see below |

Refused for a `loom` variant, each with its reason in the message: `decider.kind` (the decider is
the model of `decider.model`), `decider.escalate` (a person step is answered by the decider, never
escalated), `decider.max_decider_calls` (`max_llm_calls` counts every call, the decider's too),
`adapter`, `decider.adapter`, `review.adapter` (a node's adapter is the one its binding names),
`autonomy` (no levels: the decider answers every person step), `implement_mode`, `review_mode`,
`implement_coverage`, `conformance`, `rule_approvals` (switches of `run-trial.sh`),
`flags.dry`, `flags.recorded` (flags of `run-trial.sh`), `efforts.<class>.effort` (no argument of
`llm-node.sh` in Loom mode carries an effort, so the class defaults of `tools/node-classes.json`
apply), `free.*`. A `loom` input is an `idea` (the case is `IDEA-1`). A record that compares the
runners puts each runner's own settings in its variants' `settings` and the shared ones (models,
`max_llm_calls`) in `fixed`.

**`run --print`** for a `loom` run prints, besides the unit name, `description:` (the record and the
run, never a setting), `env NAME=value` for each variable the unit gets, the idea file and the
settings file it will write (under `<builds-dir>/../launch/<id>/<run>/`, outside the run directory,
which `runbook loom-run` creates and refuses to find), the settings the file will hold, and the
command, one line:

```
<build>/runbook/target/debug/runbook loom-run --run <name> --runs-dir <dir> --loom <bin> --loom-sha256 <hex>
  --bindings <build>/bindings --protocols <build>/protocol --root-protocol <id>@<revision> --case IDEA-1
  --idea <file> --confinement <setting> --settings <file> --spend-ledger <runs-dir>/<record id>.loom-spend.json
  --cap-micro-usd <n> --max-llm-calls <n> --max-wall-seconds <n>
```

The root protocol is read from `protocol/sdc.protocol.yaml` of the tree (`protocol.id`,
`protocol.revision`, a `.` in the id written `-`): from the build directory when it exists, else with
`git show <tree_commit>:…` in the working directory's repository. The same read refuses a tree
with no `runbook/Cargo.toml`: `refused: tree_commit <c> has no runbook/ crate` (at `run`, with or
without `--print`; `check` accepts a planned record whose commit is not pinned yet). The build step
(`build:` line of `--print`) runs each crate as its own cargo job and adds `runbook`
(`cargo build --locked`, debug) for a `loom` record; a build record that lacks it gets that crate
added without rebuilding the others.

**Environment.** The unit is `systemd-run --user … env -i` as for `sdc`, with exactly the login's
base variables that exist: `HOME`, `PATH`, `USER`, `LOGNAME`, `LANG`, `XDG_RUNTIME_DIR`,
`XDG_CONFIG_HOME`, `XDG_STATE_HOME`, `XDG_CACHE_HOME`, `XDG_DATA_HOME`, `DBUS_SESSION_BUS_ADDRESS`
(the list of the `sdc` runner, `BASE_ENV`), and neither `SDC_RUNS_DIR` nor `SDC_PRINCIPAL` nor any
setting as a variable. `runbook loom-run` builds the environment of `b10x-loom` and of its programs
itself.

**The login file.** `loom.login_file` is named by path in the settings file and nowhere else. The
launcher never reads it and never copies it; `run --print` prints
`login: loom.login_file is set (…)` and `<loom.login_file>` in place of the path, and the unit's
description holds no setting. The settings file itself holds the path (it is not committed, it is
under the state directory).

**`status` and `stop`.** A `loom` run's unit is `sdc-exp-<id>-<variant>-<n>` and its run `<id>.<variant>.<n>`,
as for the other runners. `stop` writes `run-stopped.json` (`sdc.run-stopped/1`, `at`, `by`, `reason`) into the
run's directory when it exists, is not stopped and not finished (it never calls `run-trial.sh`),
then stops the unit: systemd sends SIGTERM to `runbook loom-run`, which writes its outcome on that
signal (unit R1, `story:loom-run-core`). A run already stopped keeps its record.

**Example record** (not registered; `experiments/` holds none):

```yaml
format: sdc.experiment/1
id: example-loom
kind: prospective
status: planned
created: 2026-11-22
title: A model run of the cycle through Loom
purpose: Whether a registered record starts a model run through Loom.
inputs:
- {name: invoices, kind: idea, text: Bill customers with invoices., sha256: <sha256 of the text>}
tree_commit: null
fixed:
  loom.bin: ~/.cargo/bin/b10x-loom
  loom.sha256: <64 hex digits>
  loom.confinement: local
  loom.cap_micro_usd: 5000000
  models.writer.model: claude-sonnet-5-5
  models.implement.model: claude-sonnet-5-5
  models.review.model: claude-sonnet-5-5
  models.other.model: claude-sonnet-5-5
  decider.model: claude-opus-5-5
variants:
- {id: low, runner: loom, settings: {max_llm_calls: 100, max_wall_seconds: 3600}, runs: []}
- {id: high, runner: loom, settings: {max_llm_calls: 200, max_wall_seconds: 3600}, runs: []}
runs_per_variant: 1
pass_rule: clean-delivery
decision_rule: {rule: descriptive, text: the record describes its runs and decides nothing}
```

**Run directories of a `loom` run (the contract of the unit that writes them).** `report` and
`check --runs-dir` read a run directory through `run_metrics::load` (`driver/src/run_metrics.rs`),
the same code as for a driver run, and the clean-pass rule of `report` (`clean_pass`:
`closed && delivered && clean`). A line that does not parse is skipped and counted
(`records.malformed_lines`); a file that is absent is absent, never an error, except the root
case's `events.jsonl`. A summary replaces every string that holds an absolute host path (a user home,
temp, runtime or device-shared directory, or `/root/`) by `<absolute path removed>`.

| file (under the run directory) | members read | used for | when absent |
|---|---|---|---|
| `free-agent.json` | existence | makes `load_run` read the directory as a free-agent run | must be absent |
| `store/IDEA-1/events.jsonl` (root; else the one directory under `store/` holding an `events.jsonl`) | per line `event`: `opened` (`protocol.id`), `action_started` (`action`, `llm`, `retry_of`), `action_finished` (`action`, `exit`, `not_called`, `refused.kind`, `principal`, `evidence`), `terminated` (`outcome`), `revised` | takes, stages (`sdc` = idea, `sdc.<stage>`), rework, the root's `terminated.outcome` = how the run ended | `report` fails, exit 2: `cannot read …/events.jsonl: no such file` |
| `<case>/observed.jsonl` | `index`, `observed_at` (RFC 3339 `Z`) | take and loop times, `wall_s`, the join of model calls to takes | no times: `driver_s` 0, `wall_s` null, calls joined in order |
| `<case>/ir.json` | `actions.<a>.effect` | node kind (`write` = writer, else check) | every node `other` |
| `<case>/bindings.json` | `<a>.manual`, `<a>.copy` | node kind approval, copy | none |
| `<case>/evidence/<id>.json` | `kind`, `result` | negative results (rework causes, edges) | cause `<kind>:?` |
| `<case>/children/<id>/…` | the same five files | child cases | not counted |
| `<root>/work/findings.yaml` | `findings[].stage,status,resolution.amends` | `findings_raised`, finding edges | 0 |
| `llm-calls.jsonl` | `case` (the case directory's name), `action`, `started`, `cost_usd`, `duration_ms`, `model`, `effort`, `usage.*` | `run.llm_calls` (lines), `run.llm_cost_usd` (sum), node cost and tokens | both null; `cost_usd` counts 0 |
| `decisions.jsonl` | `case`, `action`, `class`/`type`, `decision`, `model`, `cost_usd`, `duration_ms`, `principal`, `time`, `suppressed`, `fallback`, `valid` | `run.decider_*`, `decision.<class>.*` | `decider_*` null; counts 0 |
| `status.json` | `state`, `outcome`, `delivered`, `forced`, `unstuck`, `reopens` | a close when the root has no `terminated`; `delivered`; clean counters | counters 0; `delivered` from the outcome |
| `run-stopped.json` | `at`, `by`, `cause` or `verdict_at_stop`, `case`, `reason` | `end: stopped: …`, `stop_cause`, waiting time | run is `open` unless closed |
| `run-stopped.<x>.json` | name only | one reopen each | 0 |
| `defects.jsonl` | `forced` | forced steps | 0 |
| `rounds.jsonl` | `seconds` | `run.driver_s`, waiting time | null |
| `run-settings.jsonl` | the last line, nested objects flattened by dots (`at`, `format`, `nodes`, `llm_calls_before` dropped) | `settings`; `check --runs-dir` compares it with the record: `loom.sha256`, `loom.confinement`, `loom.cap_micro_usd`, the models, `decider.model`, `max_llm_calls`, `max_wall_seconds`, whichever the record sets (not `loom.bin`, `loom.login_file`) | every setting reads `not recorded` and `check` refuses the run |
| `oracle.json` | `conformance.*` | the `conformance*` metrics | `not observed` |

The run's end: `closed_outcome` is the root's `terminated.outcome`, else `status.json` `outcome`
when its `state` is `closed`; `end_kind` is `closed` with one, `stopped` with a `run-stopped.json`,
else `open`; `finished` is `end_kind != open` and only a finished run gets a summary and counts as
ended. `delivered` needs a close: `status.json` `delivered` when it is a boolean, else the outcome
is `operating`. `clean` is `closed` with no reopen (`run-stopped.<x>.json`, `status.json`
`reopens`), no forced step (`defects.jsonl`, `status.json` `forced`) and no unstick (`status.json`
`unstuck`). The smallest directory that reads as a clean delivered run with a cost: the root
`events.jsonl` (an `opened` event and `terminated` with outcome `operating`, or any outcome with a
`status.json` that says `delivered`), `llm-calls.jsonl` with `cost_usd` per call and, for `check`,
`run-settings.jsonl`; a test feeds `report` exactly that
(`driver/tests/experiment_loom.rs`).

**Procedure** (at most 30 lines):

1. Look at the loop nodes and edges of the last records (`experiments/INDEX.md`, each
   `report.md`); pick one node or edge.
2. Write `experiments/<id>.yaml`, `kind: prospective`, `status: planned`: purpose, a hypothesis
   on that node or edge with direction and size, one variable, the input with its sha256, full
   settings in `fixed` and the variants, `runs_per_variant` ≥ 3, the primary metric and guards
   (`run.delivered`, `run.cost_usd`). Pin `tree_commit` to a commit of `main`.
3. `sdc-experiment check <id>`; `sdc-experiment run <id> --print`; commit the record.
4. `sdc-experiment run <id>` in the background. No run is started by hand.
5. `sdc-experiment status <id>` while it runs; `sdc-experiment stop <id> --reason "<why>"` when it
   cannot go on.
6. When every run has ended: `sdc-experiment report <id>`; the verdict comes from the rule, not
   from reading the numbers. Commit the record, `experiments/<id>/` and `INDEX.md`.
7. Held and every guard holds: adopt (commit the change; `adoption.decision: adopted`, `commit`).
   Contradicted or a guard fails: `dropped`. Inconclusive: a new record `<id>-2` with the same
   settings (the operator: "and repeat that"). Never change the rule after the runs.
8. After an adoption the treatment is the new baseline; re-run the baseline before the next
   experiment, because the bindings and the driver change between days.
9. Runs made before the records existed are registered retrospectively: a `runs:` list per
   variant, the trees they ran on, facts from the notes; `report` writes their summaries.

## 8. Relation to M1-M11 (`docs/eval-design.md` § 2)

These metrics extend the eval design's metrics; they do not redefine them.

| Here | Eval design |
|---|---|
| `run.delivered`, `run.end` | M1 success is stricter (a hidden oracle). Until a run writes `oracle.json`, `delivered` is the only success figure; it is not M1 |
| check edges after a writer exit 0 | M2 false done at node level: `FD_node` counts writer runs whose next check fails; the check edges and `first_pass_yield` count the same events per node |
| `takings`, `real_takes`, run `takings` | M4 driver steps (`action_finished` per case run); agent tool calls are not counted here |
| `cost_usd`, `tokens`, `llm_calls` | M5 cost, with the basis `provider-reported` from `cost_usd` |
| `rework_takes`, `looping_factor` | new: M4 counts steps, not why they repeat; LF splits the repeats by cause |
| edges, `loop_cost_usd` | new: loops per transition; nothing in M1-M11 is local to an edge |
| `first_pass_yield` | W1 first-pass success (`docs/eval-design.md` D11), per node instead of per case |
| decision classes | new; `escalation_suppressed` is the count § 7 requires |
| `end: stopped: writer_spent`, `stuck` | M10 time to stuck: the stop and its cause; M10's step count is `takings` up to it |
| M3, M6-M9, M11 | not computed here (they need oracles, tool-call traces or Canon) |

## 9. What the records do not allow

- **Tool calls inside a node** (M6-M8). `llm-calls.jsonl` cuts each tool input at 300 characters
  and has no per-call time.
- **Which finding caused which rework.** Findings carry no time and no take. Finding edges share
  out loop cost by count.
- **Revisions.** A run made before the launcher does not record the driver and bindings
  revision: `opened.bindings` names a worktree path, not a commit, and runs from different hours
  ran different code. A launched run records its `tree_commit` in `experiment.json`; the canon
  binary is pinned in `driver/canon-pin.json` and checked by the scripts.
- **Effort.** Absent before `feat/effort-per-class`; those runs read `unrecorded`.
- **Quality of the delivered app.** No run writes `oracle.json`.
- **Retries inside a session.** An invalid model reply retried inside `llm-node.sh` (review
  attempts) is one call line.
