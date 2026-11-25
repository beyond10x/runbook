---
format: aep.planning-md/3
id: story:a-run-names-its-classes-document-and-level-table
kind: story
status: draft
title: A run or the daemon names the classes document and the level table
summary: Both files are read from two fixed paths of the checkout; a setting of the run and an option of the daemon name them.
tags:
- consumer-asks-2026-11-25
relations:
- informed_by: story:decisions-routed-by-runbook-data
scope:
- confidence: cited
  path: docs/daemon-api.md
- confidence: cited
  path: docs/engine.md
- confidence: cited
  path: docs/loom-run-settings.md
- confidence: inferred
  path: runbook/src/config.rs
- confidence: cited
  path: runbook/src/loom_run/bindings_copy.rs
- confidence: cited
  path: runbook/src/loom_run/decider/routing.rs
- confidence: inferred
  path: runbook/src/serve.rs
- confidence: inferred
  path: runbook/tests/daemon
- confidence: inferred
  path: runbook/tests/loom_run
revision: 10
---
## What is missing

A consumer's runbook keeps its classes document and its level table in its own place, and Runbook reads them from two fixed paths. A run or the daemon cannot be told where they are.

Read on `main` at `81079535`:

- The routing reads `driver/examples/sdc/decision-classes.json` and `tools/autonomy-levels.json`, both "relative to the root above the bindings copy" (`runbook/src/loom_run/decider/routing.rs:3-5`, `:19`, `:21`; `Routing::load`, `:66-69`).
- The run's copy of the bindings brings those two files only because `bindings_copy` copies `driver/examples` and `tools` of the checkout beside the bindings (`COMPANIONS`, `runbook/src/loom_run/bindings_copy.rs:36`); the run then reads them once, from the copy, before its first action (`Decider::pin_routing`, `routing.rs:6-9`).
- The settings keys of a run name no such file (`SETTINGS_KEYS`, `runbook/src/loom_run/bindings_copy.rs:24`), and the daemon's options name none (`docs/daemon-api.md:230-262`). A consumer therefore has to place both files at the two paths of the checkout it runs from.
- `story:decisions-routed-by-runbook-data` makes the classes, their holders and the files a decider is shown the data of the runbook, found beside its bindings, and folds the level table into the classes document. It does not give a run or the daemon a way to name the file.

## Outcome

1. A run names the classes document and the level table by a setting (`runbook run`), and the daemon names them by an option or a `runbook.toml` key; a submit may not name them.
2. A file named is copied into the run directory with the bindings, and the run is routed from the copy, as today.
3. Naming nothing reads the two fixed paths, byte for byte as today.
4. A named file that is missing, is not a regular file, is a link, or is not in the document's format stops the run before its first action, by name.

## What a guard must hold

- Only the operator names the files: a run's settings file and the daemon's start options, never a submit body and never a case artifact.
- A named file is read through the held directory and refused when it is a link, a pipe or a second name, as the records of a run are (`runbook/src/record_write.rs`); its content is checked by the same loader, so a table that is inconsistent stops the run, as `Routing::load` does today.
- The run directory keeps the copy as the record of what the run was routed by, and the run row or the view `configuration` shows the paths that were named, as text.

## Open question, with the option recommended

- How this meets `story:decisions-routed-by-runbook-data`. Recommended: this story names the files as they are today (two); when that story folds them into one document beside the bindings, the setting names that one document and the two-file form is removed with it, in that story's change.

## Acceptance

With a fixture whose classes document and level table sit outside the two fixed paths:

- a run with the two settings named is routed by them: a class the table gives to `decider` is answered by the decider, and a class given to `person` stays open;
- the daemon started with the two options routes a submitted run the same way, and the run's record names both files;
- a named file that is a link, or has a wrong format, stops the run before its first action with a message that names the file and the problem;
- with neither named, an existing test over the fixed paths still passes.

## Scope

`runbook/src/loom_run/decider/routing.rs`, `runbook/src/loom_run/bindings_copy.rs`, `runbook/src/config.rs`, `runbook/src/serve.rs`, `docs/loom-run-settings.md`, `docs/daemon-api.md`, `docs/engine.md`, `runbook/tests/loom_run`, `runbook/tests/daemon`.

## Depends on

- Informed by `story:decisions-routed-by-runbook-data`: its design decides the final shape (one document), and the two stories touch the same loader.
