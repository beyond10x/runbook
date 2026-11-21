---
format: aep.planning-md/3
id: epic:public-docs-site
kind: epic
status: proposed
title: Public documentation site
summary: 'The SDC documented at beyond10x.github.io/sdc: what it is, the flow, a protocol browser rendered from the YAML, the evaluation per release, one run step by step'
tags:
- docs
relations:
- decomposes: initiative:roadmap
revision: 2
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T15:09:49Z", actor: "human:operator", revision: 2, executor: "agent:sdc-docs-site", correlation: "operator-standing-acceptance-2026-11-21"}
---
# Epic: public-docs-site

## Outcome

The SDC has a public documentation site at https://beyond10x.github.io/sdc/, in the style of the
other beyond10x project sites (Docusaurus with the docs-system theme). It says what the SDC is
and what it is built on, draws the main flow and every protocol from the protocol YAML, shows
the evaluation per release, and shows one run step by step.

## Why Now

The operator, 2026-11-21: "I think now it's time to handle the public documentation." The
improvement loop (`epic:improvement-loop`) starts once the site is published, so that every
later release adds a column to the evaluation charts.

## Scope

1. `story:protocol-site-generator`: `sdc-protocol-site` writes the site's protocol data from the
   YAML, its bindings and the committed check outputs; `check` fails on stale data.
2. `story:docs-site-pages`: the site with the landing page, What it is, the flow, the protocol
   browser, Built on and Run it.
3. `story:docs-evaluation`: the evaluation section, one chart per metric over the releases.
4. `story:docs-llm-not-the-runtime`: the principle and one run's timeline from
   `sdc.run-timeline/1`, with the comparison with a free-running agent.
5. `story:docs-site-checks`: the smoke test, axe, internal links, the Taskfile tasks.
6. `story:docs-site-publication`: the validation workflow and the publishing caller, the first
   publication on the operator's approval.

## Out of Scope

Hand-written figures: every number comes from a committed file. Anything under `runs/`.

## Risks

- Citations link to tag 0.1.1; a later release moves lines, so a citation can point at the wrong
  line until it is updated.
- The timeline page needs the timelines on main; until they are there it says "Timeline pending".

## Done When

The site is published from main by the bot and every story is delivered.
