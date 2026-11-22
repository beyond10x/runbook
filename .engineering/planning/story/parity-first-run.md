---
format: aep.planning-md/3
id: story:parity-first-run
kind: story
status: implemented
title: 'Parity first run: protocol id stem, feedback copies, tool environment, child outcome revision'
summary: Close the Runbook-side classes of the first Loom parity comparison
tags:
- loom
- parity
relations:
- decomposes: epic:runbook-one-binary-and-daemon
- depends_on: story:parity-run-on-loom
scope:
- confidence: cited
  path: CHANGELOG.md
- confidence: cited
  path: bindings/tools/sdc-tool.sh
- confidence: cited
  path: docs/loom-bindings.md
- confidence: cited
  path: docs/parity.md
- confidence: cited
  path: runbook/src/parity.rs
- confidence: cited
  path: runbook/tests/cli.rs
- confidence: cited
  path: scripts/check-bindings.sh
revision: 13
transitions:
- {from: "draft", to: "proposed", at: "2026-11-22T05:37:12Z", actor: "human:operator", revision: 9, executor: "agent:rb2-parity-first-run", correlation: "operator-standing-acceptance-2026-11-22"}
- {from: "proposed", to: "active", at: "2026-11-22T05:37:12Z", actor: "human:operator", revision: 10, executor: "agent:rb2-parity-first-run", correlation: "operator-standing-acceptance-2026-11-22"}
- {from: "active", to: "implemented", at: "2026-11-22T06:20:44Z", actor: "human:operator", revision: 13, decided_on: {"recorded":{"test_result":1}}, executor: "agent:parity-equal", correlation: "operator-standing-acceptance-2026-11-22"}
---
# Story: parity-first-run

## Outcome

The first real run of `stub-cycle` under Loom compared with the driver's run printed 59 lines. Four
classes were defects or missing rules on Runbook's side and are closed here: the spelling of a
protocol id, the feedback copies in the product tree, a bound program that cannot find the `ess`
toolchain without `HOME`, and the revision a child's `case_outcome` record carries in its parent.
The other classes (actors, one `upstream_revisions` member) stay visible.

## Context

Decisions of the design owner, 2026-11-22. Loom starts a bound program with `PATH` and `LOOM_*`
variables only, and that stays. The driver records a protocol's declared id (`sdc.intake`), Loom the
registration stem (`sdc-intake`). The driver hands retry feedback to the writer in a variable; Loom's
`llm-node.sh` copies it into `.sdc/loom-feedback/<16 hex>/`. The driver writes a child's
`case_outcome` record with the parent's revision of the subject at the moment the child is opened and
keeps it; Loom replaces that revision at the child's termination (read by Loom's session at Loom
`64e8f5e2`); Loom's reading is the intended one and the driver is not changed.

## Acceptance

- `runbook parity compare` reads `cases[].protocol`, `copies[].from_protocol` and `protocols[].id` as
  registration stems (`.` and `_` as `-`): two summaries that differ only in that spelling compare
  equal, `sdc.intake` against `sdc.plan` still differs, and `compare --rules` and `docs/parity.md`
  name the rule and say what it cannot tell apart (tests in `runbook/tests/cli.rs`).
- The product tree listing of a summary leaves out everything under `.sdc/loom-feedback/` on the
  driver side and the Loom side, and lists a file under `.sdc/other/` (tests in `runbook/tests/cli.rs`).
- In Loom mode `bindings/tools/sdc-tool.sh spec-validate` exits 0 under `env -i PATH=... LOOM_CASE=...`
  with the host's `ess` dispatcher, because the tool names `ESS_TOOLCHAIN_DIR` from the passwd entry of
  the user when none of `ESS_TOOLCHAIN_DIR`, `XDG_CACHE_HOME` and `HOME` is set; outside Loom mode
  nothing changes (`scripts/check-bindings.sh`, "loom environment").
- `runbook parity compare` neither compares nor counts the `subject_revision` and `upstream_revisions`
  of a `case_outcome` record when it names labels in order of first appearance: a pair that differs
  only in such a record's revision compares equal with the labels of the following records aligned,
  and a pair that differs in the revision of another record still differs; the named rule and the
  known differences of the two runtimes are in `compare --rules` and `docs/parity.md`.
- `docs/parity.md` marks each item of "What the first real run needs" that the first run settled.

## Result: equal on what both sides record

The cycle `stub-cycle` was run again from one export of Runbook `ee90434`, by the driver (exit 0,
108 s) and under Loom `2c0a6f48` (version 0.58.0; six `tree run` calls, five `tree approve` calls,
`tree verify` exit 0 with `{"cases":8}`, 43 s), both with principal `person:owner`. The summaries,
written by the `runbook` binary of `45287aa`, both print `8 cases, 69 evidence records, 54 program
runs, 5 person steps, 62 product files`. `runbook parity compare` and `compare --product-digests`
exit 0 and print `equal on what both sides record: 8 cases, 69 evidence records, 54 program runs, 5
person steps, 62 product files`, then six lines `not recorded on one side` (`source.run_settings`,
`copies[].action`, `copies[].subject`, `copies[].subject_revision`, `decision_requests`,
`protocols[].digest`). With Loom `64e8f5e2` one more line was left, an `upstream_revisions.vision` on
`roadmap_approval`; Loom `7d071ad8` (in `2c0a6f48`) records it. The facts and what the word "equal"
does not cover are in `docs/parity.md`, "First equal comparison (2026-11-22)".

Acceptance, line by line:

- The registration stem: met. The two spellings (`sdc.intake`, `sdc-intake`) are in the two
  summaries and no line names them; the test is
  `parity_compare_reads_a_protocol_id_by_its_registration_stem` in `runbook/tests/cli.rs`.
- The feedback copies: met. The product tree of both summaries holds 62 files and Loom's workspace
  held 3 under `.sdc/loom-feedback/`, which are not listed; the files are Loom's retry-feedback
  document handed over without a retry (`docs/parity.md`, "The rules").
- `sdc-tool.sh` under Loom without `HOME`: met. The Loom run had no `HOME` for its programs and
  the last `tree run` ended with exit 0 while `ess` ran below `b10x-loom` in the sampled process
  tree; `scripts/check-bindings.sh` ("loom environment") holds the case.
- The revision of a `case_outcome` record: met. The comparison is equal with the rule; the test is
  `parity_compare_leaves_the_revision_of_a_case_outcome_record_out`.
- `docs/parity.md` marks the settled items: met, and the equal comparison adds to items 11 and 14.

Tests: `cargo test --test cli parity_` in `runbook/` at the commit of this section's parent: 39
passed, 0 failed, 1.18 s of test time (46 s job, 427 s queued), among them the two named above.
This unit changed text only.
