---
format: aep.planning-md/3
id: story:docs-site-publication
kind: story
status: proposed
title: First publication of the site
summary: The validation workflow and the project-site caller, published from main by the bot on approval
tags:
- docs
relations:
- decomposes: epic:public-docs-site
scope:
- confidence: cited
  path: .github/publish/b10x-docs-site.yml
- confidence: cited
  path: .github/workflows/pages.yml
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T15:09:50Z", actor: "human:operator", revision: 2, executor: "agent:sdc-docs-site", correlation: "operator-standing-acceptance-2026-11-21"}
---
# Story: docs-site-publication

## Outcome

The site is published at https://beyond10x.github.io/sdc/ from main by the bot, the way the
sibling sites publish: `Documentation validation` (`.github/workflows/pages.yml`) builds, tests
and uploads `b10x-project-site`; `Documentation site` calls
`beyond10x/website/.github/workflows/project-site.yml` with `route_base: /sdc/`.

## Context

The caller waits in `.github/publish/b10x-docs-site.yml` until the operator approves the first
publication; GitHub Pages for `beyond10x/sdc` is not enabled yet.

## Acceptance

After Pages is enabled with build type `workflow` and the caller is moved into
`.github/workflows/`, a bot push to main runs both workflows green and
https://beyond10x.github.io/sdc/.well-known/b10x-site.json names the pushed commit.

## Scope

`.github/workflows/pages.yml`, `.github/publish/b10x-docs-site.yml`.
