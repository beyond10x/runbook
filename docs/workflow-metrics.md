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
| `pool_waits`, `ran_over_time` | takes whose tool exited 69 (`ran_over_time`: 70, the build pool ended a started job at its run limit; the step is not run again): no build slot within the tool's wait bound, nothing ran. Apart from `could_not_run` and from failures; not a real take, never rework (`docs/driver.md`, "The build pool wait"). The run sum is `pool_waits` of the run; a run stopped by it has `stop_cause` `pool_wait_spent` | `action_finished` `exit` |
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
- **Run sums**: `takings`, `real_takes`, `rework_takes`, `not_called`, `pool_waits`, `ran_over_time`, `cost_usd` (LLM + decider),
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
- `fixed` and `variants` (`id`, `settings`, `runs`): full run settings, the dotted keys of
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
  stopped; without `--run` the record becomes `abandoned` with `stopped` (at, by, reason).

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
