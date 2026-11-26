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
  path: CHANGELOG.md
- confidence: cited
  path: docs/daemon-api.md
- confidence: cited
  path: docs/engine.md
- confidence: cited
  path: docs/mcp.md
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
  path: runbook/src/engine/products.rs
- confidence: cited
  path: runbook/src/mcp/schema.rs
- confidence: cited
  path: runbook/src/mcp/tools.rs
- confidence: cited
  path: runbook/src/record_write.rs
- confidence: cited
  path: runbook/src/serve.rs
- confidence: cited
  path: runbook/tests/daemon
revision: 9
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

## Design (taken by the operator, 2026-11-25, reversible before release)

Each answer is the smallest that meets the acceptance; each can be changed before a release.

- Where the files are: the workspace of a daemon run (`<state>/workspaces/<run id>/`, `StateDir::workspace_dir`,
  `runbook/src/state.rs`) is the run's `work/`: the cases of a run share it (`runbook/src/engine/mod.rs`, `Asking`).
  A path in both reads is relative to it (`draft.txt`, `notes/a.md`): no `work/` prefix, never absolute, never a path
  of the host.
- Two views of the specification (`ess/domains/daemon.yaml`), each over an entity no command creates (as `Runbook`):
  `products` (`run_id`; one row: `files` with `path`, `size` and `modified_at` each, `cut`, `hidden`, `left_out`) and
  `product` (`run_id`, `path`; one row: `path`, `size`, `modified_at`, `kind` text or binary, `text`, `shown`,
  `truncated`, `limit`). Their HTTP routes are `GET /daemon/views/products` and `GET /daemon/views/product`: "one HTTP
  route" is read as one route per read, the form every read of the daemon has.
- Refusals: a view has no outcome in the language, so they are the daemon's `{"refused", "message"}` answers, written
  in each view's summary: 404 `no such run`, 404 `no such file`, 404 for a name that starts with a dot, 400 for a path
  that is empty, absolute, holds a NUL or a backslash, or has a `..` component, 403 for a link at any component (no
  link is followed, whether it stays in the workspace or not), 403 for a file with more than one link, 415 for what is
  not a regular file. A refusal names the path as it was asked and holds no byte of a file and no path of the host.
- Bounds, the inspector's (`ui/products.mjs`, `LIMIT`, `MAX_*`): 1 MiB of a file is shown, a longer one answers its
  first 1 MiB with `truncated` and its `size`; text is UTF-8 without NUL, a partial character at the cut dropped, else
  `kind: binary` and no text; the listing visits at most 500 files, 2000 directories, 20000 entries and 6 levels and
  says which bound cut it (`cut`).
- The reader: one module, `runbook/src/engine/products.rs`, used by both views, for both runtimes (a stub run has an
  empty workspace). It reads through `record_write::Held` (`runbook/src/record_write.rs`): the workspace is opened
  `O_NOFOLLOW | O_DIRECTORY`, each directory below it with `openat` and the same flags, the file with `openat`
  `O_NOFOLLOW | O_NONBLOCK`, and the type and link count are read from the open descriptor. `Held` gains three methods:
  an existing child directory opened without making it, an entry's kind and size without following a link, and the
  first bytes of a regular file of one name. The read is in the daemon's process; no program is started.
- The command layer: `Daemon::products_of_run` and `Daemon::product_of_run` (`runbook/src/commands.rs`) check the run
  against the daemon's record first; `Fault` gains a refusal with its status, which `routes::view_answer`
  (`runbook/src/api/routes.rs`) answers. The views join `routes::VIEWS`, so `api::router_full` mounts them behind
  `auth::guard`, the one guard layer; `runbook/src/serve.rs` needs no change.
- The tools: `list_products` and `read_product` are two rows appended to `TOOLS` (`runbook/src/mcp/tools.rs`), the count
  constant goes from 9 to 11, and their input schemas come from the views' query parameters (`schema::view_param`).
- Not built here: a tool for the case tree (the page reads the view `cases` of
  `story:daemon-page-shows-a-run-tree-and-products`), a write of a run's file.
