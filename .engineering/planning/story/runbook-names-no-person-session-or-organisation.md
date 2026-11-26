---
format: aep.planning-md/3
id: story:runbook-names-no-person-session-or-organisation
kind: story
status: active
title: The repository names no person, session or organisation, and the gate refuses an added line that does
summary: Replace the remaining session names with the operator; a digest-list check refuses an added line that names a person, session or organisation
tags:
- hygiene
scope:
- confidence: cited
  path: AGENTS.md
- confidence: cited
  path: CHANGELOG.md
- confidence: cited
  path: Taskfile.yml
- confidence: cited
  path: driver/Cargo.toml
- confidence: cited
  path: driver/refused-names.txt
- confidence: cited
  path: driver/src/bin/sdc-check-names.rs
- confidence: cited
  path: driver/tests/main.rs
- confidence: cited
  path: driver/tests/names.rs
- confidence: cited
  path: plugin/skills/workflow-builder/references
revision: 5
transitions:
- {from: "draft", to: "proposed", at: "2026-11-26T12:09:25Z", actor: "agent:runbook", revision: 2}
- {from: "proposed", to: "active", at: "2026-11-26T12:09:25Z", actor: "agent:runbook", revision: 3}
---
# Story: runbook-names-no-person-session-or-organisation

## Outcome

This repository is public. Its files, authors and commit messages name no person, no organisation and no session:
the operator is written "the operator", an operator session "the operator session". The gate refuses a change that
adds a line naming one.

Order: the operator, 2026-11-26 13:05 CET, relayed by the operator session: the public runbook repository must
contain no person name, organisation name or session name. The operator session's scan of `main` `b634e080` found
no person or organisation name in files, authors or commit messages, and one session name in fourteen lines: the
approval record `the-ui-step-fits-the-forge-budget-order` (title and body), the body of
`story:the-ui-step-fits-the-forge-budget`, the `correlation` of seven transitions of four stories
(`a-bug-fix-hands-back-only-with-a-spec-reference`, `check-bindings-exits-reads-any-runbook`,
`run-without-model-node-needs-no-caps`, `submitrun-outcome-order`), and four lines of the plugin's
`workflow-builder` references (`frictions.md`, `jumps.md`, `timeline.md`).

## Acceptance

- Every mention in a file outside `.engineering/` and in a title or body of the planning store reads "the operator"
  or "the operator session"; a record whose text is immutable is changed by `aep plan artifact redact`, whose record
  keeps no literal. A line the CLI owns that no verb changes is listed in the change's report with the reason.
- A program of the repository (Rust, no Python) reads a list of SHA-256 digests of lower-cased refused words, never
  the words, and fails when a line it checks holds a word whose digest is listed: each run of letters and digits, and
  each run of up to four such parts joined by `-` or `_` (a session name is a hyphenated word), lower-cased.
- By default it checks the lines a change adds against the merge base with `origin/main` (on a checkout without
  `origin/main`, the first parent of a merge `HEAD`); a full mode checks every line of every tracked file. Each
  finding names the file and line and never prints the word.
- The list holds the digests of the two session names of the order and of the person and organisation names the
  scan searched for.
- A test the forge already runs fails on a line holding a refused word (the word built by the test at run time, never
  a literal in the source) and passes a clean line; another runs the default mode on the repository itself. `task
  check` runs the check.
- Its time on the full tree is measured and reported.

## Scope

- `driver/` (a binary and its tests), `Taskfile.yml`, `plugin/skills/workflow-builder/references/`, the planning
  store through `aep`, `CHANGELOG.md`, `AGENTS.md`.
