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
revision: 8
transitions:
- {from: "draft", to: "proposed", at: "2026-11-22T01:55:52Z", actor: "human:operator", revision: 2, executor: "agent:parity-driver-side", correlation: "operator-requirement-2026-11-22-one-binary"}
- {from: "proposed", to: "active", at: "2026-11-22T01:55:52Z", actor: "human:operator", revision: 3, executor: "agent:parity-driver-side", correlation: "operator-requirement-2026-11-22-one-binary"}
---
# Story: parity-run-on-loom

## Outcome

The driver's half of the parity check between the old driver and Loom is ready, so the comparison
runs the minute Loom can run a case tree. `runbook parity` reads a finished run of the old driver into
a runtime-neutral summary (`runbook.parity-summary/1`), compares two summaries by one table of rules,
and lists the person steps a second runtime must repeat. Loom's reader is the next step, listed below.

## Context

Operator, 2026-11-22: "can we just run one of the evals where we knew that was working fine? Can we
now run it through Loom and basically compare it? ... as soon as we know the loom stuff is basically
working and there is parity towards that dummy runtime we implemented, I would really like to remove
that runtime itself from runbook." Loom's `tree run` command is being built; its first target is the
cycle `stub-cycle` (`scripts/run-finding-cycles.sh`). Wave rb2, unit `parity-driver-side`. The reader
is `Source::Driver` in `runbook/src/parity.rs`; `Source::Loom` is not built.

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

## Next step: Loom's reader

Add `Source::Loom { store, workspace }` to `runbook/src/parity.rs` and read what `tree run` leaves on
disk into the same summary. What is known (Loom's message of 2026-11-22, `crates/loom-governor/src/
file_store.rs:1-30`; integration branch `wave/2026-11-21-w7`) and where each member is expected:

- store: one file per case `case-<hex of case id>.json` (`case-sha256-<hex>.json` for ids over 100
  bytes; members `StoredCase` in Loom's `ess/domains/governor.yaml`), `observations.json`, `.lock`;
  beside each case file `runlog-<stem>.jsonl` (hash-chained lines: `seq`, `at`, `kind`, event members);
  beside the root case `tree-protocols.json` and the recorded workspace path.
- `tree status` prints JSON per case (the stable reading): id, protocol, revision, outcome or stop,
  open obligations, paths of case file and run log.

| Summary member | Expected in Loom | Status |
|---|---|---|
| `cases[].id` | case file stem (hex of the id) and `tree status` id | known |
| `cases[].link`, `cases[].parent` | the case link's `name` (child case name); the parent from the link: `tree status` or the `StoredCase` members | link known, parent unknown |
| `cases[].protocol`, `protocol_revision` | `tree status` (protocol, revision) and `tree-protocols.json` | known |
| `cases[].outcome`, `stop` | `tree status` (outcome or stop) | known; the stop causes' names unknown |
| `cases[].open_obligations[]` | `tree status` | known |
| `cases[].evidence[]` (kind, result, subject, `subject_revision`, `upstream_revisions`) | the case file (`StoredCase` evidence members) | unknown |
| `cases[].evidence[].index` (order received) | line order (`seq`) of the run log | expected, unknown event kind |
| `cases[].evidence[].action` | run log event members or `observations.json` | unknown |
| `cases[].program_runs[]` (action, attempt, exit) | `observations.json` (program runs) and the program result table | members unknown |
| `cases[].program_runs[].retry_followed` | retry records in the run log | kind unknown |
| `cases[].person_steps[]` (action, answer, actor, capability) | the run log line of `tree approve`; a `person` action stops with exit 3 until approved | unknown |
| `copies[]` | run log of the target case (`record-copy-between-cases`) | unknown |
| `decision_requests[]` | story `decision-request-file-store` | not in `stub-cycle`: empty on both sides |
| `protocols[]` (id, revision, digest) | `tree-protocols.json` (per protocol id: revision, file name, sha256) | known; digest is of the file, the driver's of the compiled form: compared by pattern |
| `product_tree[]` | the workspace directory (path recorded beside the root case), same exclusions | known |

Where Loom records a member in a form the table of rules compares badly, change the table in
`runbook/src/parity.rs` (`RULES`) and say why in the story; do not loosen it silently. The driver side
of the comparison is run with `runbook parity summarize` on `runs/stub-cycle` and its store
(`RUNS=<dir>/runs scripts/run-full-cycle.sh stub-cycle <dir>/store-stub-cycle`); the list of
`tree approve` calls comes from `runbook parity approvals`.
