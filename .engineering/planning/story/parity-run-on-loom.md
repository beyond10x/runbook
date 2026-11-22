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
revision: 15
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
