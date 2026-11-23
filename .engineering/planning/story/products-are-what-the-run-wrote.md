---
format: aep.planning-md/3
id: story:products-are-what-the-run-wrote
kind: story
status: active
title: The Products panel lists what the run's actions wrote, not the tree it was seeded with
summary: A product is a file an action of the run wrote or changed, listed by step; a seeded tree is one closed Inputs line; the product opened first is never an input.
relations:
- informed_by: story:mcp-reads-a-run-product
scope:
- confidence: cited
  path: ui/server.mjs
- confidence: cited
  path: ui/web/products.js
revision: 5
transitions:
- {from: "draft", to: "proposed", at: "2026-11-23T11:57:29Z", actor: "human:operator", revision: 2, executor: "agent:runbook"}
- {from: "proposed", to: "active", at: "2026-11-23T11:57:29Z", actor: "human:operator", revision: 3, executor: "agent:runbook"}
---
## What the operator saw

2026-11-23, in the Products panel of a live bug-fix run: "under products, why do I see now they're all
individual files? [...] What does product mean? Product shouldn't this be something which is produced
during that process? And I mean, adding existing code or files there doesn't make much sense, I think."

Measured on that run by the session that started it, after three steps:

- The panel listed 329 files and said the list was cut. 322 of them were the copy of the repository the run
  was seeded with (the tree under test and its base).
- The file the panel showed as the run's product was a Markdown file of the seeded repository, an input,
  chosen as "the newest Markdown file under work/".
- What the run had written by then were five files: the story, the dissection, the reproducing test, its
  result, and that test inside the tree under test.

## Outcome

A product is a file an action of the run wrote or changed. The Products panel lists those, grouped by the
step that wrote them, newest step first.

1. The source is the run's own record of what each action revised, where the run directory has one. Where
   it has none (an older run), the panel says so and lists `work/` as today.
2. A tree the run was seeded with is an input. It is not listed file by file: one closed line "Inputs" with
   the trees and their file counts. A file inside such a tree that an action changed or added is a product
   and is listed, marked as changed or added.
3. The product the panel opens first is the last file the last finished step wrote, never a file of an
   input tree.
4. The list is not cut by depth for products; a bound on the number of lines stays and says how many are
   not shown.

The same meaning holds for the daemon's `list_products` (`story:mcp-reads-a-run-product`): a follow-up
brings it there once that unit is on the default branch.

## What a guard must hold

Unchanged from the panel today: every read through the run-path check and the one checked open, `work/`
only, regular files with one name, no dot-directory component, content shown as text, no library from a
network address. The record of what an action revised names paths: a path in it that leaves `work/` or the
run directory is not listed and not opened.

## Acceptance

A fixture run with a seeded tree of 200 files, three steps and five written files lists five products in
three groups, one "Inputs" line with the count, and opens the last written file; a record that names a path
outside the run lists nothing for it; an older fixture without the record shows the note and today's list.
Both sizes pass the UI gate's layout checks.

## Origin

The operator's words of 2026-11-23, relayed by the consumer session with the measurements above.
