---
format: aep.planning-md/3
id: story:daemon-page-shows-a-run-tree-and-products
kind: story
status: proposed
title: The daemon's page shows a run's case tree and products, with one address per run
summary: For a run the daemon started, its page shows the case tree and the Products tab and opens on a run by a stable address.
relations:
- depends_on: story:mcp-reads-a-run-product
revision: 2
transitions:
- {from: "draft", to: "proposed", at: "2026-11-23T10:20:12Z", actor: "human:operator", revision: 2, executor: "agent:runbook"}
---
## Outcome

The page of the daemon shows a run the daemon started the way the inspector shows a run directory: the case
tree, the Products tab of a step, and one stable address per run that opens on that run.

## Source

The consumer's second gap after its first run through the daemon (2026-11-23): the inspector pointed at the
daemon's state directory finds no case in a run's tree, and the daemon's page has no case tree, no Products
tab and no link per run.

## Scope

- `runbook/src/engine/`: a run of the daemon writes, or serves, what the tree view and the Products tab
  read.
- `ui/`: the daemon's page uses the inspector's tree and Products panels; one address per run.
- `runbook/src/serve.rs`: the routes the panels read, in the one guard layer.
- `ui/test/`, `docs/engine.md`, `CHANGELOG.md`.

## Acceptance

- For a run started through the daemon, the page shows its cases as a tree with each case's state, and for a
  step with products the Products tab lists and shows them.
- `<page>#/runs/<run id>` (or the form the page already uses for its routes) opens on that run after a
  reload and from another browser tab.
- The reads use `story:mcp-reads-a-run-product`'s one read of a run's files; the page adds no second reader.
- The content of a run never becomes markup or code of the page: the cases of the inspector's panels for
  that rule run against the daemon's page too.
- The inspector itself gets no route to the daemon and stays read-only.

## Not in this story

- Run directories of `runbook run` on the daemon's page: `story:daemon-page-watches-run-directories`.
