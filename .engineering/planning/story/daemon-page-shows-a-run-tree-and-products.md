---
format: aep.planning-md/3
id: story:daemon-page-shows-a-run-tree-and-products
kind: story
status: active
title: The daemon's page shows a run's case tree and products, with one address per run
summary: For a run the daemon started, its page shows the case tree and the Products tab and opens on a run by a stable address.
relations:
- depends_on: story:mcp-reads-a-run-product
scope:
- confidence: cited
  path: CHANGELOG.md
- confidence: cited
  path: docs/daemon-api.md
- confidence: cited
  path: docs/engine.md
- confidence: cited
  path: ess/domains/daemon.yaml
- confidence: cited
  path: ess/generated
- confidence: cited
  path: runbook/src/api/routes.rs
- confidence: cited
  path: runbook/src/commands.rs
- confidence: cited
  path: runbook/src/engine/mod.rs
- confidence: cited
  path: runbook/src/runtime.rs
- confidence: cited
  path: runbook/tests/daemon
- confidence: cited
  path: ui/test
- confidence: cited
  path: ui/web/app.js
- confidence: cited
  path: ui/web/products.js
- confidence: cited
  path: ui/web/source.js
revision: 5
transitions:
- {from: "draft", to: "proposed", at: "2026-11-23T10:20:12Z", actor: "human:operator", revision: 2, executor: "agent:runbook"}
- {from: "proposed", to: "active", at: "2026-11-23T15:29:52Z", actor: "human:operator", revision: 3, executor: "agent:runbook"}
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

## Design (taken by the operator, 2026-11-25, reversible before release)

Each answer is the smallest that meets the acceptance; each can be changed before a release.

- The tree's source: a third view, `cases` (`run_id`; a row per case: `case_id`, `parent`, `name`, `protocol`,
  `state`, `outcome`), over an entity no command creates (`ess/domains/daemon.yaml`). The Loom runtime answers it with
  Loom's `tree_status` of the run's store (`calls::status`, `runbook/src/engine/calls.rs`: the call the engine makes at
  a run's end; Loom reads the store and does not hold it, so it answers while the run goes on). No path of the store or
  the case files is in a row. `Runtime` (`runbook/src/runtime.rs`) gains `cases` with an empty default: the stub runtime
  has no tree. An unknown run is refused 404 `no such run`, as the two product reads.
- A case's state: `Ended` when it has an outcome; else `AwaitingPerson`, `AwaitingAuthority` or `Stuck` by the last stop
  its run log names; else `Running` when the run's step in flight names the case (`Daemon::cases_of_run`,
  `runbook/src/commands.rs`); else `Open`.
- The page: `ui/web/source.js` answers `/api/tree` from `cases`, `/api/products` from `products` and a file from
  `product`; it builds each case with the fields the inspector's tree reads (its parent, its depth, its protocol and a
  state event made from the row's state), so `renderTree` (`ui/web/app.js`) draws the nested cases with its depth
  guides in the existing sidebar (`#left`, "Case tree"), to any depth the run has. A case is labelled by its link name.
  The run-events feed stays the daemon's own events.
- The address: `?run=<run id>`, the form the page already uses (`?run=<name>`); a page the daemon serves looks the run
  up by its id and writes the id into the address when a run is picked. An address that names a run the daemon does not
  have shows a note naming that id, never another run.
- Refresh: on the daemon's page each run change and the two-second tick read the tree again, so a case that waits or a
  run that ends while it is shown changes on the page.
- The Products tab: the run's files (the workspace is shared by its cases) with the inspector's list and file panels
  (`ui/web/products.js`); the file content is read through the view `product` and built with text nodes, as before. The
  evidence records and the action output of a case are not served by the daemon: the tab says so.
- No second reader: the page reads only the daemon's views; `ui/server.mjs` (the inspector) gets no route to the daemon
  and stays read-only.
- Not built here: the graph of a case on the daemon's page (it says the daemon serves none), run directories of
  `runbook run` (`story:daemon-page-watches-run-directories`).
