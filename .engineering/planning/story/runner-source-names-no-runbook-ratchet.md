---
format: aep.planning-md/3
id: story:runner-source-names-no-runbook-ratchet
kind: story
status: proposed
title: A check lists every runbook name in the source of the runner and the node program, one shard per source file, and a shard can only shrink
summary: A test in task check reads action ids, artifact paths and protocol ids from the runbook files and compares their places in runbook/src and the node program with a committed list; a new place and a stale line both fail.
tags:
- generic-runner
relations:
- decomposes: epic:runner-knows-no-runbook
scope:
- confidence: cited
  path: AGENTS.md
- confidence: inferred
  path: Taskfile.yml
- confidence: cited
  path: runbook/tests/cli.rs
- confidence: inferred
  path: runbook/tests/source_names
revision: 9
transitions:
- {from: "draft", to: "proposed", at: "2026-11-23T06:01:18Z", actor: "human:operator", revision: 9, executor: "agent:runbook", correlation: "epic-runner-knows-no-runbook-critics-3"}
---
## Outcome

A check reads the names of every runbook of the repository from the files of the runbooks themselves (protocol ids, action ids, artifact paths) and finds each place where the source of the runner (`runbook/src`) or of the node program (`driver/src/bin/sdc-node.rs`, `driver/src/bin/sdc-node/`, and the library modules of the node program under `driver/src/node/` from the day that directory exists) writes one of them in a string. The places of today are committed as one small file per source file (a shard). The check fails on a place that is on no shard and on a line of a shard the source no longer has, so the shards can only shrink. It is part of `task check`.

This is a ratchet. It comes first so that each later story of `epic:runner-knows-no-runbook` removes its lines from the shards of the source files it changes, and `story:runner-source-names-no-runbook` removes the last shards.

## Source

- `epic:runner-knows-no-runbook`, item 5 ("a check fails when `runbook/src` or the node program names an action, an artifact or a protocol of a runbook") and acceptance line 1 ("The check of item 5 is in `task check` and green").
- Read at `main` `826b09c7` (the cited lines hold at `3c0eb863`, release 0.7.0: of the paths read, only `runbook/src/commands.rs` and tests of the daemon differ):
  - The names exist in files: nine bindings files `bindings/*.yaml` with 66 `- action:` entries, their `artifacts` maps (16 distinct paths, among them `idea.yaml`, `findings.yaml`, `aep/vision/product.md`, `impl`, `spec`), and nine protocol ids (`sdc`, `sdc.bugfix`, `sdc.direction`, `sdc.epic`, `sdc.intake`, `sdc.plan`, `sdc.release`, `sdc.spec`, `sdc.story`) with the file stems `sdc`, `sdc-bugfix` and so on.
  - Places that write such a name in a string of code that is not test code, counted with search patterns of the decomposer (a reading, to be counted again by the check itself). Action ids: `driver/src/bin/sdc-node/nodes.rs` 14 (the table, `:140-194`), `call.rs` 6, `sdc-node.rs` 5, `findings.rs` 5, `store.rs` 5. Artifact paths: `nodes.rs` 21, `implement.rs` 8, `call.rs` 6, `findings.rs` 6, `store.rs` 6, `sdc-node.rs` 2, `runbook/src/loom_run/decider/inputs.rs` 9, `decider/requester.rs` 1, `goals.rs` 1, `run_dir.rs` 1, `run_loop.rs` 1. Protocol names: `runbook/src/loom_run/bugfix.rs` 10, `check.rs` 6, `assets.rs` 4, `doctor.rs` 3, `list.rs` 3, `decider/routing.rs` 3, and 1 each in `here.rs`, `run_loop.rs`, `decider/requester.rs`, `sdc-node.rs`, `call.rs`.
  - `runbook/src` has 21,487 lines with its tests; the node program has 2,821 (`wc -l`).
  - No check of this kind exists: the commands of `check` in `Taskfile.yml` are `check:ci-paths`, `check:ess-generated`, `check:cycles-suite`, `check:unreferenced`, `check:protocols`, builds, clippy, `scripts/check-bindings.sh`, `scripts/evaluate-cases.sh` and `plan`. A scan of Rust source with a rule for what is test code already exists in `driver/tests/bugfix_prompt.rs:630-780` (files `tests.rs`, `golden.rs`, `*_tests.rs`).

## Scope

- New: the check, as tests of the one integration-test binary of the runbook crate (`runbook/tests/cli.rs`, a module `runbook/tests/source_names/`), and its shards `runbook/tests/source_names/known/<source path>.txt` (for example `runbook/tests/source_names/known/driver/src/bin/sdc-node/nodes.rs.txt`). Rust; no new binary, no Python.
- `AGENTS.md`: one line that names the check and the rule of the shards.
- `Taskfile.yml` only when the check needs a named task of its own beside `test:runbook`.
- Overlap, by story:
  - Every other story of the set (`story:binding-states-model-node-inputs-and-output` to `story:runner-source-names-no-runbook`) has an edge to this one and types the shards of the source files it types; two stories meet on a shard only where they already meet on its source file.
  - `story:run-without-model-node-needs-no-caps` (active; `bindings_copy.rs`, `here.rs`, `run_loop.rs`), `story:runbooks-load-from-configured-paths` (active; `assets.rs`, `list.rs`, `here.rs`), `story:runner-records-action-times-and-output` and `story:loom-run-records-its-claims` (active; `run_dir.rs`, `run_loop.rs`): this story changes none of their source files. Precondition: the shards are recorded from the `main` this story starts on; one of the four that lands later and removes a name deletes the line of its shard in the same change (the check tells it which), one that adds a name fails the check.
  - `story:project-rename` (active): stage 2 renames source paths; the rule for a renamed path below is written for it.
  - Drafts with no typed scope, each not built in the same wave as this story: `story:held-rows-name-existing-tests` (`runbook/tests/cli.rs`) and `story:replay-lane-budget-and-trigger` (`AGENTS.md`, `Taskfile.yml`).

## Acceptance

- The check reads the names from the protocol files (`id` and the file stem), from every `- action:` of the bindings files and from their `artifacts` maps, for the runbook of the repository (`protocol/`, `bindings/`) and for every runbook fixture under `runbook/tests/fixtures/`.
- The check holds no name of a runbook in its own source: the test `the_check_names_no_runbook_itself` runs the same scan over the Rust files of `runbook/tests/source_names/` and finds no hit. The names its other tests plant are invented and come from a small runbook the test writes for itself.
- The rule of a hit is written in the doc comment of the check: a name counts where it stands in a string literal of code that is not test code, as a whole name (the characters before and after it are not a letter, a digit, `_`, `-` or `.`), so `"sdc"`, `"sdc/close:"` and `"driver/examples/sdc/x"` are hits for the protocol `sdc`, and `"sdc-node"` and `"sdc.run-status/1"` are not (program names and format ids are item 6 of the epic). Comments are not read. Test code is what the scan of `driver/tests/bugfix_prompt.rs:630-780` already treats as test code, plus `#[cfg(test)]` modules.
- An artifact path that is one bare word (no separator and no extension; at `3c0eb863` these are `aep`, `impl`, `release`, `repro` and `spec`) counts only where it stands as a path: in a string literal that holds a `/` or ends with a file extension, and in which the word is a whole segment or the stem of the last segment. So `"out/impl"`, `"impl/.sdc/coverage.json"` and `"spec.md"` are hits, and `"impl"`, `"spec"` and `"release"` alone are not. Such a word can stand in the source for a reason that has nothing to do with a runbook, and `story:runner-source-names-no-runbook` has to reach zero hits. The test `each_bare_artifact_word_is_a_hit_in_path_form_and_no_hit_alone` reads the bare words from the bindings of the repository and from the small runbook it writes for itself, plants for each word one literal of each kind on the small source tree and reads, per word, one hit and one line that is no hit; no word stands in the source of the test, and the report lists the words it ran for.
- A shard has one line per hit and no count: the name, a tab, the string literal the name stands in. A literal that stands twice in a file gives two equal lines. The lines are sorted bytewise. A source file with no hit has no shard. A line holds no line number, so a change that moves code changes no shard.
- The test `no_new_runbook_name_in_runner_or_node_source` compares each source file with its shard and fails with `new: <source file>: <line>` for a hit the shard lacks and with `stale: <source file>: <line>` for a line the source lacks. Two tests on a small source tree written for them prove both directions (`a_planted_name_is_reported_as_new`, `a_line_the_source_lost_is_reported_as_stale`): each fails before the check exists and passes after.
- The shards cannot grow unseen: the test `no_shard_grew_since_the_merge_base` compares each shard with the same shard at the merge base with `main` and fails on an added line and on a shard that is new; where no merge base can be read (an exported tree) it prints `skipped: no merge base` for this part only.
- A renamed source path moves its shard: the change that renames or moves a source file renames its shard with `git mv` in the same commit. The check fails with `shard without source: <shard>` while a shard has no source file. The merge-base comparison takes as the old shard of a new shard the one whose source path `git diff --find-renames --name-status <merge base>` reports as the old name of the source file; a new shard with no such rename fails. The test `a_renamed_source_file_takes_its_shard_along` holds it on the small source tree.
- A hit that is wrong under the rule is fixed by a sharper rule, never by a line that stays on a shard for good.
- The check runs in under 5 s, builds nothing and starts no program but `git`; `task check` runs it.
- The names of the second runbook (`story:second-runbook-with-model-node`) are read by the check from the day that fixture exists, and no shard ever holds one of them.
- `task check` is green at the head of the story.
- The last commit of the unit, made after `main` is merged into it, adds one entry under `[Unreleased]` of `CHANGELOG.md`. No other commit of the story changes that file, and the file is not in the typed scope of the story, so two stories of one wave do not meet on it.
- Tests: no test over 60 s and the test step within 120 s; no model call and no network in a test (stub adapter, sh programs); what is expensive (a built binary, a prepared run) is made once per test run, never per test. Each new test is timed before it is added; a test over 5 s is named with its time in the report. Nothing that stays is Python (Rust or POSIX sh).
- A program, variable, file or format id this story creates for a generic thing carries no `sdc` in its name (epic item 6; the names that exist follow stage 2 of `story:project-rename`).

## Not in this story

- Removing an offender: every later story of the epic does that for its own files.
- The library `driver/src/reply/` and the other programs of the driver crate: the epic limits the check to `runbook/src` and the node program.
- Names that say `sdc` for a generic thing (`sdc-node`, `SDC_*`, `sdc.*` format ids, the directory `.sdc/`): item 6, stage 2 of `story:project-rename`.
