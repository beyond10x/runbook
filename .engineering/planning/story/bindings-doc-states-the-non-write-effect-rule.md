---
format: aep.planning-md/3
id: story:bindings-doc-states-the-non-write-effect-rule
kind: story
status: draft
title: 'docs/loom-bindings.md says that an action not marked effect: write records nothing when it changes an artifact'
summary: Loom discards the step of a read action that changes a placed artifact; Runbook states this only for the old driver, so one line goes into the Loom bindings page.
tags:
- consumer-asks-2026-11-25
scope:
- confidence: cited
  path: docs/loom-bindings.md
revision: 2
---
## What is missing

A consumer found that Loom drops the evidence of a step whose action is marked `effect: read` when the step changes an artifact, so it marks a route step `effect: write`. That is Loom's rule, not Runbook's, and nothing in Runbook's documents of the Loom bindings says it.

Read at the Loom this repository pins (`runbook/Cargo.toml:24`, rev `ed644a4d3487d4f05120aa705085b9e143905982`):

- "An action whose effect class is not `write` (`read` or none) whose program changed a placed artifact (its digest after the run differs from before) records nothing: the step is `Discarded` (`ChangedByNonWriter`) and its moves are still written" (`crates/loom-bindings/src/program.rs:84-88`; the decision is at `:393`).
- Runbook states the same rule only for the old driver: "An observing action (`effect` not `write`) that changed an artifact records nothing (event `discarded`)" (`docs/driver.md:125`), and the no-write check for `effect: read` as generic (`docs/boundary.md:91`).
- `docs/loom-bindings.md`, the page on the bindings in Loom's format, has no line on it: its field mapping and its sections on timeouts, budgets and pending items (`docs/loom-bindings.md:34-70`, `:70-120`, `:210`) never say that an action that changes an artifact must be `effect: write`. A person who writes a bindings file for a step that updates an artifact meets a step that records nothing, and the page does not say why (what the run shows for a discarded step was not read).

Runbook cannot check the rule before a run: whether a program changes an artifact is known only when it runs. A line in the page is what Runbook can add.

## Outcome

`docs/loom-bindings.md` says, in its "Field mapping" section, that an action whose `effect` is not `write` and whose program changes a placed artifact records nothing, that Loom discards the step and still writes the artifact's moves, and that such an action is marked `effect: write`. The line names the pinned Loom revision and the file and lines it was read at.

## What a guard must hold

- The line states only what Loom's source says at the pinned revision; it is rechecked against the source when the pin moves, as the other lines of that page that cite Loom are.
- No change of code, of a bindings file or of a check: the line is the whole change.

## Acceptance

`docs/loom-bindings.md` holds the line; `git grep -n "ChangedByNonWriter" docs/loom-bindings.md` prints it; the file and lines it cites hold at the revision in `runbook/Cargo.toml` (`git show <rev>:crates/loom-bindings/src/program.rs` shows the sentence at the cited lines).

## Scope

`docs/loom-bindings.md`.
