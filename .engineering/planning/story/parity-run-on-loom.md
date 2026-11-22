---
format: aep.planning-md/3
id: story:parity-run-on-loom
kind: story
status: active
title: 'Parity of a run under the old driver and under Loom: the driver''s half'
summary: runbook parity summarize, compare and approvals read a driver run into a runtime-neutral summary and compare it with Loom's
tags:
- loom
- parity
relations:
- decomposes: epic:runbook-one-binary-and-daemon
- depends_on: story:engine-on-loom-library-entry
- depends_on: story:run-in-process-and-cli-as-daemon-client
scope:
- confidence: cited
  path: CHANGELOG.md
- confidence: cited
  path: docs/parity.md
- confidence: cited
  path: runbook/src/main.rs
- confidence: cited
  path: runbook/src/parity.rs
- confidence: cited
  path: runbook/tests/cli.rs
revision: 21
transitions:
- {from: "draft", to: "proposed", at: "2026-11-22T01:55:52Z", actor: "human:operator", revision: 2, executor: "agent:parity-driver-side", correlation: "operator-requirement-2026-11-22-one-binary"}
- {from: "proposed", to: "active", at: "2026-11-22T01:55:52Z", actor: "human:operator", revision: 3, executor: "agent:parity-driver-side", correlation: "operator-requirement-2026-11-22-one-binary"}
---
# Story: parity-run-on-loom

## Outcome

The parity check between the old driver and Loom is ready on both sides, so the comparison runs the
minute Loom can run a case tree. `runbook parity` reads a finished run of the old driver, or the JSON
that Loom's `tree status` printed, into a runtime-neutral summary (`runbook.parity-summary/1`), compares
two summaries by one table of rules, and lists the person steps a second runtime must repeat. What
the first real run under Loom still has to confirm is listed in `docs/parity.md`.

## Context

Operator, 2026-11-22: "can we just run one of the evals where we knew that was working fine? Can we
now run it through Loom and basically compare it? ... as soon as we know the loom stuff is basically
working and there is parity towards that dummy runtime we implemented, I would really like to remove
that runtime itself from runbook." Loom's `tree run` command is being built; its first target is the
cycle `stub-cycle` (`scripts/run-finding-cycles.sh`). Wave rb2, units `parity-driver-side` and
`parity-loom-side`. The readers are `Source::Driver` and `Source::Loom` in `runbook/src/parity.rs`.
Loom's reader takes the JSON of `tree status` and the product tree and nothing else of Loom; it was
read against Loom commit `1016c251`, read-only.

## Acceptance

- `runbook parity summarize --driver <run dir> --store <store dir>` writes `runbook.parity-summary/1`
  of a finished driver run: cases (id, link name, parent, protocol and revision, outcome or stop, open
  obligations) with their evidence in the order received, program runs and person steps, plus copies,
  decision requests, protocols and the sorted product tree with sha256; no absolute path and no time;
  a member the driver does not record is listed under `not_recorded` (tests in `runbook/tests/cli.rs`
  on synthetic stores).
- `runbook parity compare <a> <b>` prints one `equal:` line and exits 0 for two equal summaries, and one
  line per difference and exit 1 otherwise; revisions, case ids and digests are compared by pattern,
  product file digests only with `--product-digests`; `--rules` prints the table; one test per
  difference class (another result, another order, a missing record, another revision pattern,
  another product file, another person answer).
- `runbook parity approvals <summary>` prints the person steps in order as JSON (path, case, action,
  capability, answer, actor): the `tree approve` calls Loom's run must make.
- Two summaries of two separate driver runs of `stub-cycle` compare equal with `--product-digests`.
- `runbook parity summarize --loom-status <file> [--product-tree <dir>]` writes the same summary from
  the JSON `tree status` printed (`loom.tree-status/1`) and the product tree, and from nothing else of
  Loom: a status of another format is refused by name; a member it needs and does not find is an error
  naming the member and the case, never a default; the summary of the hand-written fixture status
  `runbook/tests/fixtures/parity/loom-status.json` (a parent with two children, an approval with an
  action, a grant without, a retried run, a copied record) is the golden file beside it.
- A member `tree status` holds nothing for (`source.run_settings`, `copies[].action`, `.subject`,
  `.subject_revision`, `decision_requests`, `protocols[].digest`) is listed under `not_recorded`, and
  `compare` names it `not recorded on one side` and compares nothing there; the words the two runtimes
  name apart (`Allow`, `Deny`, the stops) live in the one `RULES` table of `runbook/src/parity.rs`, and
  `docs/parity.md` holds the same table (driver member, Loom member, rule), a test holding the two
  equal, with the section "What the first real run needs".
- The driver-side fixture summary and the Loom-side summary of the same small tree compare equal with
  `--product-digests`; one record missing on the Loom side, another outcome of one child and a `Deny`
  against the driver's approval each produce a named difference (tests in `runbook/tests/cli.rs`, each
  under 5 s).

## Next step: Loom's reader

`Source::Loom { status, product_tree }` is built (unit `parity-loom-side`). It reads the JSON of
`tree status` of Loom `1016c251` and not the store: the earlier plan to read the case files, the run
logs and `observations.json` is replaced, because `tree status` already carries the evidence, the runs,
the approvals and the copies of every case. The table of each member, and the seventeen things that
`1016c251` does not settle, are in `docs/parity.md`.

Open: a first real run. Run `stub-cycle` under Loom, print `tree status`, summarize it beside the
driver's summary of the same cycle and compare with `--product-digests`; every line `compare` prints
is a difference to explain, and every line of "What the first real run needs" the run does not confirm
is a change to the reader. Where Loom records a member in a form the table of rules compares badly,
change `RULES` in `runbook/src/parity.rs` and say why in the story; do not loosen it silently.

## Acceptance of the evaluation run

- For the three Loom runs of the evaluation and the three driver runs of `release-0.1.2` (strong), the
  comparison prints, per run pair, each member of the summary as `same` or `differs` with the
  difference, and a closing line `parity: pass` only when every case outcome, every evidence kind and
  result and every decision answer is the same, the product tree's file lists agree up to the declared
  tolerance, and each quality measure of the Loom runs lies within the range of the three driver
  runs; otherwise it prints `parity: fail` and exits 1.
- The tolerance for the file list and the quality measures is written in the story's report before the
  Loom runs start, and the report states for each member how the driver runs vary among themselves.
- A summary of a deliberately altered run (one outcome changed) makes the comparison print
  `parity: fail`: a test on recorded summaries, no model call, within 60 s.
- The Loom runs start no `sdc-drive` process (sampled from the process tree) and write their records
  under the run's state directory.
- The comparison report is committed under `experiments/` and `experiments/INDEX.md` lists it
  (`sdc-experiment index`).
- `runbook run` of the cycle `stub-cycle` on Loom and the driver's run of it give summaries whose
  comparison is empty, printed as `parity: identical` with exit 0.

The evaluation run is the strong setting of `experiments/release-0.1.2.yaml`, whose three driver runs
all ended `closed: operating` with a clean pass (`experiments/release-0.1.2/report.md`); the choice may
be changed to another complete setting of that record by the story's report, with the reason. The
driver's root outcome is still `operating` where Loom's bindings may say `delivered`
(`story:root-outcome-delivered`); the summary maps names it knows are the same and lists every mapping
it applies. The runs start only through `sdc-experiment run` with a registered record.

## Approval of the evaluation spend (2026-11-22)

The operator's word on the money for the three evaluation runs was given on 2026-11-22 and is
recorded in `approval-record:operator-decision-parity-runs-spend-2026-11-22`: "A: 3 runs, cap 20
USD". The three Loom runs of the evaluation start without asking again once the `stub-cycle`
summaries are equal on both runtimes, and they stop at 20 USD of spend. The story's report gives the
three results and the spend. The runs still start only through `sdc-experiment run` with a registered
record.

## State after the first equal comparison

2026-11-22. `stub-cycle` run by the old driver and under Loom `2c0a6f48`, both from one export of
Runbook `ee90434`, compare equal on what both sides record: `equal on what both sides record: 8
cases, 69 evidence records, 54 program runs, 5 person steps, 62 product files`, with and without
`--product-digests`, exit 0, and six members listed as not recorded by Loom (`docs/parity.md`, "First
equal comparison"). The Loom run was made with `b10x-loom tree` commands by hand: six `tree run`,
five `tree approve`, `tree verify` exit 0.

The stub-cycle condition of the spend approval
(`approval-record:operator-decision-parity-runs-spend-2026-11-22`: the three evaluation runs start
once the `stub-cycle` summaries are equal on both runtimes) is met on these facts. The three
evaluation runs have not started and no spend was made.

Not met: the last line of "Acceptance of the evaluation run". The run was not made by `runbook run`,
and `compare` prints `equal on what both sides record:` and not `parity: identical`; the words
`parity: identical`, `parity: pass` and `parity: fail` are in no code of `runbook/src/`.

"Acceptance of the evaluation run", line by line:

1. Per run pair, each member `same` or `differs`, closing `parity: pass` or `parity: fail`, exit 1
   on fail: not met. `compare` prints one line per difference or one `equal` line, and no closing
   `parity:` line; the three Loom runs of the evaluation and the three driver runs of
   `release-0.1.2` were not made, and no quality measure was compared.
2. The tolerance written in the story's report before the Loom runs start, and how the driver runs
   vary among themselves: not met. No report exists; the evaluation has not started.
3. A deliberately altered run makes the comparison print `parity: fail`, a test on recorded
   summaries within 60 s: not met. The red-case tests (another result, another order, a missing
   record, another outcome, `Deny`) assert named differences and exit 1, and none asserts the
   closing line.
4. The Loom runs start no `sdc-drive` process (sampled) and write their records under the run's
   state directory: not met for the evaluation, which has not run. For `stub-cycle`, sampled every
   20 ms over the six `tree run` calls, no `sdc-drive` and no `canon` process was seen; a program
   that lives shorter than the interval can be missed.
5. The comparison report committed under `experiments/` and listed in `experiments/INDEX.md`: not
   met. Nothing was added there.
6. `runbook run` of `stub-cycle` on Loom and the driver's run give summaries whose comparison is
   empty, printed as `parity: identical` with exit 0: not met, as above. The comparison is not
   empty either: six members are named as not recorded by Loom.

## State after the integration (wave rb3, 2026-11-22)

Goal moved: the last line of "Acceptance of the evaluation run". Met, with evidence:

- Met: a `stub-cycle` run through `runbook loom-run` (the real `b10x-loom` `2c0a6f48`, `--confinement
  local`, five scripted approvals with the principal `person:owner`, the stub adapter, no model, a
  ledger that stayed at 0) and the driver's run give summaries whose comparison is empty, printed
  `parity: identical`, exit 0. Evidence: `docs/parity.md`, "Identical comparison through `runbook
  loom-run`" (`equal: 8 cases, 69 evidence records, 54 program runs, 5 person steps, 62 product
  files`, then `parity: identical`); slow lane `task test:loom-real`, test
  `loom_run::slow::the_stub_cycle_on_the_real_loom_compares_identical_with_the_driver_run`, passed in
  191 s. The command is `runbook loom-run`, not `runbook run`, which does not exist yet; the story's
  words name `runbook run`, so this is met for the command that exists.
- Met: the six members `tree status` holds nothing for and the principal now come from the run
  directory and the settings (`docs/parity.md`, "Where the seven values come from now"); `compare`
  names no member as not recorded for a run read with `--loom-run`.
- Met in part: "The Loom runs start no `sdc-drive` process (sampled from the process tree)". For the
  `stub-cycle` run, sampled twice a second over 136 samples, the tree held `runbook`, `b10x-loom`
  and the bound programs and what they start, never `sdc-drive`, `run-trial.sh` or
  `run-finding-cycles.sh` (table in `docs/parity.md`). Not met for the evaluation runs, which have
  not run.
- Met in part: "A summary of a deliberately altered run makes the comparison print `parity: fail`".
  The closing lines are `parity: identical`, `parity: equal on what both sides record` and `parity:
  different` (exit 1); tests in `runbook/tests/cli.rs` assert all three on recorded summaries, each
  under 5 s. The words `parity: pass` and `parity: fail` of this story are not in the code: the line
  of a different run is `parity: different`. The quality measures and the tolerance are not built.
- Not met: the per-pair `same`/`differs` report of the evaluation with its tolerance and quality
  measures; the written tolerance and the variation among the three driver runs; the report under
  `experiments/` and its line in `experiments/INDEX.md`. The three evaluation runs have not started
  and no spend was made.

## Tolerance, written before the Loom runs

Written on 2026-11-22 before any of the three runs of `experiments/parity-loom-strong.yaml` starts. No
range below is widened, narrowed or reread after a run; a run outside it is reported as outside.

The rule is the one of "Acceptance of the evaluation run": each quality measure of a Loom run lies
within the range of the three driver runs. The driver runs are the three runs of the strong setting
of `experiments/release-0.1.2.yaml` (`release-0.1.2.strong.1` to `.3`, tree `72a3eef`), all ended
`closed: operating`, clean (`experiments/release-0.1.2/report.md`, the run summaries in
`experiments/release-0.1.2/runs/`). The ranges are over those three runs, the lowest and the highest
value; a Loom run is inside when `min <= value <= max`, and a measure of the experiment passes when all
three Loom runs are inside.

Quality measures, the numbers of the three driver runs (run 1, run 2, run 3) and the range:

| measure | driver run 1 | run 2 | run 3 | range |
|---|---|---|---|---|
| `run.cost_usd` | 4.6952 | 4.8158 | 4.4604 | [4.4604, 4.8158] |
| `run.llm_cost_usd` | 2.8415 | 3.1904 | 2.3716 | [2.3716, 3.1904] |
| `run.decider_cost_usd` | 1.8537 | 1.6254 | 2.0889 | [1.6254, 2.0889] |
| `run.wall_s` | 972 | 1108 | 886 | [886, 1108] |
| `run.llm_calls` | 19 | 17 | 18 | [17, 19] |
| `run.decider_calls` | 13 | 10 | 13 | [10, 13] |
| `run.real_takes` | 93 | 96 | 114 | [93, 114] |
| `run.rework_takes` | 24 | 25 | 44 | [24, 44] |
| `run.looping_factor` | 1.167 | 1.083 | 1.167 | [1.083, 1.167] |
| `run.first_pass_yield` | 0.833 | 0.917 | 0.833 | [0.833, 0.917] |
| `run.rework_cost_usd` | 0.5720 | 0.5126 | 0.4971 | [0.4971, 0.5720] |
| `run.furthest_stage_index` | 7 | 7 | 7 | [7, 7] |
| `run.findings_raised` | 2 | 5 | 4 | [2, 5] |
| `run.waiting_on_person_s` | 0 | 0 | 0 | [0, 0] |
| `run.clean` | 1 | 1 | 1 | [1, 1] |

How the driver runs vary among themselves: cost by 0.36 USD (8 % of the median), wall time by 222 s
(23 % of the median), the number of model calls by 2, decider calls by 3, first takes by 21, rework
takes by 20 (one run, the third, took 44 against 24 and 25), findings by 3. The furthest stage, the
clean end and the waiting time do not vary at all.

Not compared (the driver runs of the record do not hold them, or hold them in another sense):

- `run.driver_s` (592, 711, 887 s in the driver runs): the driver's own time, which a Loom run has no
  counterpart for; `rounds.jsonl` of a Loom run is the runner's time. Not compared.
- `run.tokens.*`, `run.model_s`, `run.takings` and `run.writer_rework_takes`: held by the summaries,
  not in the report's per-variant table, which is where the story's ranges come from. Not compared;
  the Loom values are reported beside them.
- The loop table of `report.md` (edges such as `change.review:rejected -> change.implement`, the
  looping factor of single nodes): three runs hold one to four counts of an edge; not compared.
- Per-node cost and the cost of single actions: not compared.

Members of the run summaries, by name. A Loom run's `runbook.parity-summary/1` is read with `runbook
parity summarize --loom-run <run dir>`. The committed driver runs hold the metrics summary
(`sdc.run-metrics/1` without `cases`), not a parity summary, so a member-by-member `parity compare`
of a Loom run with a driver run is possible only where the driver run directory still exists; where it
does not, the members under "must be equal" are checked against the metrics members that name the
same fact (`run.end`, `run.closed`, `run.delivered`, `run.outcome`, `run.stop_cause`,
`run.furthest_stage_index`) and the case structure is read from the Loom summary alone.

Must be equal, by member:

- `cases[].id` as a pattern, `link`, `parent`, `protocol`, `protocol_revision`: the same case tree
  (the root, the direction case, the epics, and below them spec, plan, stories and release, as in
  `stub-cycle`).
- `cases[].outcome` of every case: `passed`, and of the root `operating` (run summary `run.outcome`).
  `cases[].stop` null, `cases[].open_obligations` empty. The run is `closed`, `delivered`, with
  `reopens`, `forced` and `unstuck` 0 (`run.clean` 1).
- The kinds of evidence each case holds and the last `result` of each kind (`cases[].evidence[].kind`
  and `.result`): every kind of the driver's case is present and ends `passed`, `approved`, `valid`,
  `clean` and so on, as the driver's does. The order, the number of records and the number of
  rejected or invalid ones before the last may differ.
- `person_steps[].action`, `.capability` and `.answer` of the last step of each action, and
  `decision_requests[].answer`: every approval ends approved, every question is answered. How many
  steps an action took (a rejection first) may differ.
- `protocols[].id` as a stem and `.revision`.
- `product_tree` file list: the same top-level directories (`.sdc`, `aep`, `impl`, `mainline`,
  `published`, `release`, `spec`) and the files whose names the bindings and the tools fix: the
  `Cargo.toml`, `Cargo.lock`, `Taskfile.yml`, `README.md` and `.derived-from` of each tree,
  `release/PROVENANCE`, `published/PROVENANCE`, `spec/system.yaml`, `spec/topology.yaml`,
  `spec/components.yaml`, `spec/traces.json` and the `.sdc/` reports. A missing one of these is a
  difference.

May differ (model text, costs, times and what a model chooses):

- `cases[].evidence[].index`, `.subject_revision` and `.upstream_revisions` (values a run generates;
  `compare` reads them by order of first appearance), the number of evidence records.
- `cases[].program_runs[]`: `attempt`, `exit` and `retry_followed` of retried actions, and the number of
  program runs; rework differs with the model's text.
- `product_tree` digests (`--product-digests` is not used for these runs: a model run is not
  deterministic) and the names of the files a model names: `spec/domains/*.yaml`,
  `aep/epic/*.md`, `aep/story/*.md`, `aep/vision/*.md`, `impl/src/**`, `impl/tests/**` and the same
  paths under `mainline`, `published` and `release`.
- `.sdc/reasons/*` (the reasons file of a rejection, which only a Loom run has) and
  `.sdc/loom-feedback/**`.
- The text of every decider answer, the costs, times and token counts above, `source` and
  `not_recorded` (never compared).

The spend condition is not a tolerance: the three runs stop at the 19.00 USD stop line of the
launcher, 20.00 USD being the approved cap (`approval-record:operator-decision-parity-runs-spend-2026-11-22`,
`approval-record:operator-decision-parity-runs-unconfined-2026-11-22`).

## State after the move to Loom 5af3f9c2 (wave rb3, 2026-11-22)

Goal moved: the three evaluation runs of "Acceptance of the evaluation run". Their two start
conditions (`approval-record:operator-decision-parity-runs-unconfined-2026-11-22`) are met on Loom
`5af3f9c2` and the runs are registered, not started.

- `runbook loom-run` is pinned to Loom `5af3f9c2` and runs every printed decision command as
  printed; a rejection is `tree approve --action <a> --result rejected --reasons-file <file>`.
  The substitutes of `2c0a6f48` are gone (`docs/loom-run-decisions.md`).
- `stub-cycle` through `runbook loom-run` on `5af3f9c2` compares `parity: identical` with the driver
  summary (`docs/parity.md`, "Identical comparison on Loom `5af3f9c2`"). A second run with one person
  step rejected ends `closed: operating`, delivered, after the writer ran again with the reasons.
  The slow lane `task test:loom-real` passes on the new binary.
- The record is `experiments/parity-loom-strong.yaml` (planned, three runs, the strong setting of
  `release-0.1.2`, spend stop line 19.00 USD under the approved cap of 20.00 USD). The tolerance is
  written above, before any run. The login is named at start time only:
  `sdc-experiment run parity-loom-strong --login-file <login file> --runs-dir <a directory outside the
  repository>`.
- Not met: the per-pair `same`/`differs` report with `parity: pass` and `parity: fail`; the code
  prints `parity: identical`, `parity: equal on what both sides record` and `parity: different`
  (as in "State after the integration"). The three runs have not started and no spend was made.

The first three runs of `experiments/parity-loom-strong.yaml` were started on 2026-11-22 and ended within 17 s each in their first model step (`idea.dissect`), before any model call and without spend (the ledger booked 2.00 USD of reservation per run for a step of unknown cost, 6.00 USD, none of it spent). A program bound under Loom has `PATH` and `LOOM_*` and no `HOME`; the bindings handed the login path to the claude-cli adapter only, and metaharness reads the login under `$HOME`, so it refused to plan the launch. The record is abandoned and stays as the evidence of the attempt; `experiments/parity-loom-strong-2.yaml` replaces it, with the same settings, hypothesis, models and cap (20.00 USD, stop line 19.00 USD) on the tree that fixes the adapter and the failed-call rule of `runbook loom-run` (`docs/loom-run.md`, "How each adapter gets the login under Loom" and "A failed model call"). The tolerance section above stands as written: no range was read again, as no run reached a model.
