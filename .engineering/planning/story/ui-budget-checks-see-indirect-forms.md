---
format: aep.planning-md/3
id: story:ui-budget-checks-see-indirect-forms
kind: story
status: proposed
title: The UI budget checks see a timeout or a port that is set indirectly
summary: budget.spec.mjs reads spec files with regular expressions that miss a helper that sets a timeout or a port in a template
tags:
- review-rb2
scope:
- confidence: cited
  path: ui/playwright.config.mjs
- confidence: cited
  path: ui/test/budget.spec.mjs
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-11-22T02:50:58Z", actor: "human:operator", revision: 3, executor: "agent:rb2-review-fixes-ui", correlation: "operator-standing-acceptance-2026-11-22"}
---
# Story: ui-budget-checks-see-indirect-forms

## Outcome

A spec of the gate lane cannot get past the budget by moving a timeout, a port or a `runs/` path into a helper or a template string.

## Context

Found in the wave rb2 review of runbook/, protocol/, ui/, views/, docs/ (S3 list): ui/test/budget.spec.mjs (file checks for setTimeout, ports and runs/). Filed by the review-fix unit of wave rb2; the fix is not part of that unit.

## Acceptance

- A test in `ui/test/budget.spec.mjs` feeds the check a spec that sets its timeout in a helper and one that builds a port in a template string, and the check names both files.

## Scope

`ui/test/budget.spec.mjs`, `ui/playwright.config.mjs`.
