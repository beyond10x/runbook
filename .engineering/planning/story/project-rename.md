---
format: aep.planning-md/3
id: story:project-rename
kind: story
status: active
title: Rename the project to Runbook (three stages)
summary: 'Decided 2026-11-21: the product is Runbook; SDC is its first runbook. Stage 1 names people read, stage 2 one runbook command and release binaries, stage 3 the repository rename.'
tags:
- conductor-succession
- rename-decided
relations:
- decomposes: epic:conductor-succession
scope:
- confidence: inferred
  path: AGENTS.md
- confidence: inferred
  path: docs
- confidence: inferred
  path: driver
- confidence: inferred
  path: goals
- confidence: inferred
  path: protocol
- confidence: inferred
  path: watch
revision: 8
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T02:25:43Z", actor: "human:timo", revision: 4, executor: "agent:sdc-planning", correlation: "operator-standing-acceptance-2026-11-21"}
- {from: "proposed", to: "active", at: "2026-11-21T19:30:31Z", actor: "human:operator", revision: 8, executor: "agent:rename-stage-1", correlation: "rename-runbook-stage-1"}
---
# Story: project-rename

## Outcome

The project carries the name Runbook. Runbook is the product: the driver, the protocol format, the
run inspector, the experiment tools, the documentation site and the coming Claude plugin. One line
describes it: "Runbook: agentic runbooks. A runbook is a fixed, checked way to handle a case. A
program decides the next step from recorded facts; a model carries out single steps."

The software delivery cycle (SDC) keeps its name and is the first runbook: the one that takes an
idea to a built, tested product. Where a text speaks about that cycle, its protocols, its trials or
its measurements, it says SDC. Other runbooks are planned, not built: incident handling,
investigations, ticket triage.

## Decision

The operator, 2026-11-21, recorded by actor `human:operator`: "Yes, runbook. Agentic runbook. So I
think that's perfect. So we can start now renaming this whole thing to runbook. This is what it
is."

This replaces the postponement of the same day ("For now, just note down somewhere the intent to
renaming this later to a better thing. Let's postpone it.") and the candidates Docket, Score, Mill
and Assay, none of which was chosen.

## Stages

1. Names people read. `README.md`, `AGENTS.md`, the documentation site (title, tagline, navbar
   brand and mark, footer, page titles and descriptions, landing hero, "What it is", "What it is
   built on", "Run it", "The flow"), the run inspector (page title, header, `ui/README.md`) and
   `CHANGELOG.md`. Branch `rename/runbook-stage-1`.
2. One `runbook` command and release binaries. Crate, binary and file names (`sdc-drive` and the
   others), `SDC_*` environment names, `sdc.*` format ids, protocol ids and the files under
   `protocol/`, the bindings and scripts. Whether old ids are refused with a message naming the new
   one or read for one version is decided in this stage (requires-stakeholder-input, the operator).
3. The GitHub repository rename to `beyond10x/runbook`, with the site URL and base path
   (`/sdc/`), the Gates enrolment and every URL that names `beyond10x/sdc`. Last, because it breaks
   every existing link and checkout path.

## Acceptance

Stage 1 is done when:

- `git grep -n SDC -- README.md AGENTS.md website/docs website/docusaurus.config.ts ui/README.md ui/web/index.html`
  finds the name only where the text speaks of the software delivery cycle, its protocols, trials
  or measurements, or of a stage 2 or 3 name (`sdc-drive`, `SDC_*`, `sdc.*`, `beyond10x/sdc`);
- the site title, tagline, navbar brand, logo mark, footer, page titles and descriptions and the
  run inspector's page title and header say Runbook;
- `slot task docs` and the run inspector's tests pass;
- no crate, binary, environment, format, protocol, URL or path name changed.

The whole story is done when stages 2 and 3 are done and no text people read or program names
still carries `sdc` for the product.

## Scope

The whole repository; scope recorded as the top-level directories a rename touches. Stage 1 touches
only `README.md`, `AGENTS.md`, `CHANGELOG.md`, `website/` (not `website/src/routes/Evaluation.tsx`
or the chart components) and `ui/`.

## Ambiguities

- Refuse old ids or read them for one version (stage 2): requires-stakeholder-input (the operator).
