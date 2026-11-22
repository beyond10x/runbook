---
format: aep.planning-md/3
id: story:parity-run-on-loom
kind: story
status: proposed
title: 'Parity run on Loom: one cycle and one evaluation compared with the driver'
summary: The stub cycle and one green evaluation run through Loom and compared with the driver's runs by a runtime-neutral summary, n = 3 for the evaluation.
tags:
- one-binary
relations:
- decomposes: epic:runbook-one-binary-and-daemon
- depends_on: story:engine-on-loom-library-entry
- depends_on: story:run-in-process-and-cli-as-daemon-client
scope:
- confidence: cited
  path: docs/run-compare.md
- confidence: cited
  path: driver/src/bin/sdc-run-compare.rs
- confidence: cited
  path: driver/tests/run_compare.rs
- confidence: cited
  path: experiments
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-11-22T01:47:37Z", actor: "human:operator", revision: 2, executor: "agent:plan-onebin", correlation: "operator-requirement-2026-11-22-one-binary"}
---
# Story: parity-run-on-loom

## Outcome

The driver's runtime and Loom give the same result for the same case: one deterministic cycle and
one evaluation run that is known to be green on the driver are run through Loom and compared with the
driver's run by a runtime-neutral summary.

## Source

The operator, 2026-11-22: "can we just run one of the evals where we knew that was working fine?
Can we now run it through Loom and basically compare it?" and "as soon as we know the loom stuff is
basically working and there is parity towards that dummy runtime we implemented, I would really
like to remove that runtime itself from runbook."

## Context

The runs compared:

- Deterministic: the cycle `stub-cycle` of `cycles/suite.yaml` (protocol `sdc`, stub fixtures none,
  expected exit 0), run once by the driver (`scripts/run-finding-cycles.sh stub-cycle`) and once by
  `runbook run` on Loom.
- Evaluation: the strong setting of `experiments/release-0.1.2.yaml`, whose three driver runs all
  ended `closed: operating` with a clean pass (`experiments/release-0.1.2/report.md`), run through
  Loom three times and compared with those three. The choice may be changed to another complete
  setting of that record by the story's report, with the reason. It costs about 4.5 to 4.8 USD per
  run on the driver (table of that report), so about 14 USD for the three Loom runs: the operator's
  word is needed before they start (money is not covered by a default). They start only through
  `sdc-experiment run` with a registered record (`AGENTS.md`, "Rules"; no run is started by hand).

The runtime-neutral summary is a program in Rust (a mode of `sdc-run-compare` or of `runbook`),
written before either side is compared, with these members and no others:

- per case: name, protocol, outcome;
- per evidence record: kind, result, subject artifact, upstream revisions, order;
- decision requests and the answers given;
- the file list of the product tree (paths only);
- for the evaluation run: the quality measures of `experiments/` (`experiments/quality-0.1.0`).

The driver's root outcome is still `operating` where Loom's bindings may say `delivered`
(`story:root-outcome-delivered`); the summary maps names it knows are the same and lists every
mapping it applies.

## Acceptance

- `runbook run` of the cycle `stub-cycle` on Loom and the driver's run of it give summaries whose
  `diff` is empty, and the command that compares them (`sdc-run-compare --summary <a> <b>` or the
  named equivalent) prints `parity: identical` and exits 0.
- For the three Loom runs of the evaluation and the three driver runs of `release-0.1.2` (strong), the
  comparison prints, per run pair, each member above as `same` or `differs` with the difference, and
  a closing line `parity: pass` only when every case outcome, every evidence kind and result and
  every decision answer is the same, the product tree's file lists agree up to the declared
  tolerance, and each quality measure of the Loom runs lies within the range of the three driver
  runs; otherwise it prints `parity: fail` and exits 1.
- The tolerance for the file list and the quality measures is written in the story's report
  before the Loom runs start, and the report states for each member how the driver runs vary among
  themselves.
- A summary of a deliberately altered run (one outcome changed) makes the comparison print
  `parity: fail`: a test on recorded summaries, no model call, within 60 s.
- The Loom runs start no `sdc-drive` process (sampled from the process tree) and write their
  records under the run's state directory.
- The comparison report is committed under `experiments/` and `experiments/INDEX.md` lists it
  (`sdc-experiment index`).

## Scope

`driver/src/bin/sdc-run-compare.rs` or `runbook/src/` (the summary and its comparison),
`driver/tests/run_compare.rs`, `experiments/` (the registered record and its report),
`docs/run-compare.md`, `cycles/suite.yaml` (read). `driver/src/bin/sdc-experiment.rs` is not
changed here.
