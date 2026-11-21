---
format: aep.planning-md/3
id: story:docs-site-pages
kind: story
status: proposed
title: Site pages and the protocol browser
summary: Landing, What it is, the flow, one route per protocol, Built on and Run it, in the docs-system style
tags:
- docs
relations:
- decomposes: epic:public-docs-site
scope:
- confidence: cited
  path: website/docs
- confidence: cited
  path: website/docusaurus.config.ts
- confidence: cited
  path: website/plugins/sdc-data.ts
- confidence: cited
  path: website/src/components
- confidence: cited
  path: website/src/css/custom.css
- confidence: cited
  path: website/src/routes
revision: 8
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T15:09:49Z", actor: "human:operator", revision: 2, executor: "agent:sdc-docs-site", correlation: "operator-standing-acceptance-2026-11-21"}
---
# Story: docs-site-pages

## Outcome

A reader opens the site and finds what the SDC is (software delivery cycle, `README.md:3`), the
main flow drawn from the root protocol and its child cases, one browser route per protocol
(graph, actions with what they wait on and record, executor, ends, children and parents, state
count, simulate findings, the YAML at the release tag), and Built on: the layered diagram with
one section per beyond10x component and what it supplies, "not used yet" stated plainly.

## Context

The operator, 2026-11-21: "make sure we visualize the main flow and we visualize the
subcomponents of that flow. Basically make a sort of a browser for these different protocols."
Docusaurus with `@beyond10x/docs-system`, as the sibling sites.

## Acceptance

`npm run build` in `website/` succeeds with broken links and anchors set to throw, and the smoke
test of `story:docs-site-checks` passes on every page and protocol route.

## Scope

`website/` (config, plugin, components, routes, docs pages, CSS).
