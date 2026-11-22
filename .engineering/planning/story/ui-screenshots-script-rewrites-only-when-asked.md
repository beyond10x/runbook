---
format: aep.planning-md/3
id: story:ui-screenshots-script-rewrites-only-when-asked
kind: story
status: proposed
title: The UI screenshots script writes screenshots only when it is asked to
summary: package.json screenshots runs the gate config and rewrites screenshots/ on every run
tags:
- review-rb2
scope:
- confidence: cited
  path: ui/README.md
- confidence: cited
  path: ui/Taskfile.yml
- confidence: cited
  path: ui/package.json
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-11-22T02:50:59Z", actor: "human:operator", revision: 3, executor: "agent:rb2-review-fixes-ui", correlation: "operator-standing-acceptance-2026-11-22"}
---
# Story: ui-screenshots-script-rewrites-only-when-asked

## Outcome

The test step does not rewrite `screenshots/`; a separate task takes them and says where they go.

## Context

Found in the wave rb2 review of runbook/, protocol/, ui/, views/, docs/ (S3 list): ui/package.json (scripts.screenshots), ui/Taskfile.yml (screenshots). Filed by the review-fix unit of wave rb2; the fix is not part of that unit.

## Acceptance

- After `task test` in `ui/` the files under `ui/screenshots/` have the same modification times as before, and `task screenshots` rewrites them.

## Scope

`ui/package.json`, `ui/Taskfile.yml`, `ui/README.md`.
