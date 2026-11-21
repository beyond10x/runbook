---
format: aep.planning-md/3
id: story:docs-site-checks
kind: story
status: proposed
title: Smoke test, axe and links for the site
summary: Every page at two widths in both themes, axe, internal links, the fixture build, task docs
tags:
- docs
relations:
- decomposes: epic:public-docs-site
scope:
- confidence: cited
  path: Taskfile.yml
- confidence: cited
  path: website/playwright.config.ts
- confidence: cited
  path: website/tests/fixture.spec.ts
- confidence: cited
  path: website/tests/smoke.spec.ts
revision: 6
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T15:09:50Z", actor: "human:operator", revision: 2, executor: "agent:sdc-docs-site", correlation: "operator-standing-acceptance-2026-11-21"}
---
# Story: docs-site-checks

## Outcome

One command checks the site: `task docs` checks the protocol data is current, builds the site and
the fixture build, and runs the smoke test.

## Context

The smoke test opens every page and every protocol route at 1440x900 and 390x844 in both themes
and asserts no horizontal page scroll, no console error, each protocol page's actions, 0 serious
or critical axe violations, the flow's box sizes and unbroken identifiers, and that every
internal link answers 200.

## Acceptance

`slot npx playwright test --workers=2` in `website/` passes with the real and the fixture build
served.

## Scope

`website/playwright.config.ts`, `website/tests/smoke.spec.ts`, `website/tests/fixture.spec.ts`,
`Taskfile.yml`.
