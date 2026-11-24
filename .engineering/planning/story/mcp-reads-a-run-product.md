---
format: aep.planning-md/3
id: story:mcp-reads-a-run-product
kind: story
status: active
title: The daemon lists and serves the products of a run
summary: MCP tools list_products and read_product and one HTTP route serve the files under work/ of a run the daemon started, behind the daemon's token check, with the guards of the inspector's file route.
relations:
- informed_by: story:mcp-exposure
scope:
- confidence: cited
  path: runbook/src/engine/mod.rs
- confidence: cited
  path: runbook/src/mcp/schema.rs
- confidence: cited
  path: runbook/src/mcp/tools.rs
- confidence: cited
  path: runbook/src/serve.rs
revision: 7
transitions:
- {from: "draft", to: "proposed", at: "2026-11-23T10:20:10Z", actor: "human:operator", revision: 2, executor: "agent:runbook"}
- {from: "proposed", to: "active", at: "2026-11-23T10:20:10Z", actor: "human:operator", revision: 3, executor: "agent:runbook"}
---
## Outcome

A person who started a run through the daemon reads what the run produced: the daemon lists and serves the
files under `work/` of a run, to an MCP client (tools `list_products` and `read_product`) and over its HTTP
route, behind the daemon's one token check.

## Source

The consumer's first gap after its first run through the daemon (2026-11-23): `submit_run` of its own
runbook over `runbook mcp` ended in 49 s with no refusal; the client gets the run's state and events and has
no way to read the product the run wrote. The operator starts that consumer's runbooks by hand from an MCP
client and reads the product there.

## Scope

- `ess/`: the daemon's specification gets the two reads (a view or query each), with their refusals.
- `runbook/src/mcp/tools.rs`, `runbook/src/mcp/schema.rs`: the two tools.
- `runbook/src/engine/`: the read of a run's files, in one place, used by both routes.
- `runbook/src/serve.rs`: the HTTP route, in the one guard layer.
- `runbook/tests/`, `docs/engine.md`, `docs/mcp.md`, `CHANGELOG.md`.

## Acceptance

- `list_products` with a run id answers the files under `work/` of that run with their sizes; `read_product`
  with a run id and a path answers the bytes of one file as text, up to a stated size bound, and says so when
  the file is larger or is not text.
- The guards of the inspector's file route hold here too, each with a case: only paths under `work/`;
  regular files only; one name only (a file with more than one link is refused); a path with a dot-directory
  component is neither listed nor served; a path that leaves the run through `..` or a link is refused; no
  program opens the file by name.
- Both reads sit behind the daemon's token check and its Host and Origin guard, in the one guard layer; a
  request without the token gets the same answer as for any other read.
- A run id the daemon does not know and a path that does not exist are refused by name, and the refusal
  prints no content of a file.
- The synthesized suite of the daemon's specification covers both reads.
- A security review pass covers the change before it merges.

## Not in this story

- The page of the daemon showing the products: `story:daemon-page-shows-a-run-tree-and-products`.
- Writing or changing a file of a run.
