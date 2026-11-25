---
format: aep.planning-md/3
id: story:consumer-contract-test
kind: story
status: active
title: Runbook publishes the interfaces a consumer uses, with a test run against the consumer's pinned version
summary: One versioned contract file lists the CLI flags, intake file, work layout, call record, view file, daemon API and MCP tools; a command checks a binary against it; the gate fails when an interface changes and the file does not.
tags:
- upstream-asks-2026-11-24
refs:
- provider: consumer
  reference: upstream-asks-filed/A6
relations:
- informed_by: story:daemon-api-specified-in-ess
- informed_by: story:mcp-exposure
- informed_by: story:runbook-command-knows-the-text-layout
- informed_by: story:ui-view-data-per-runbook
scope:
- confidence: inferred
  path: Taskfile.yml
- confidence: inferred
  path: docs/consumer-contract.md
- confidence: inferred
  path: runbook/src/contract.rs
- confidence: inferred
  path: runbook/src/main.rs
- confidence: inferred
  path: runbook/tests
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-11-25T12:41:40Z", actor: "agent:runbook", revision: 3}
- {from: "proposed", to: "active", at: "2026-11-25T12:41:41Z", actor: "agent:runbook", revision: 4}
---
## Outcome

Runbook publishes the interfaces a consumer uses, with a version, and a test the consumer runs against the
runbook version it has pinned. A change that breaks an interface is caught before a release, and a consumer
learns at its own pin whether its pin still holds.

## Source

Ask A6 of a consumer, filed in its story `upstream-asks-filed` (read 2026-11-24): a consumer contract test.
The interfaces the consumer uses are the CLI flags, the intake file, the `work/` layout, the `.sdc/llm`
record, the view file, the daemon API and the MCP tools. Today each is described in a page of `docs/` and
none has a check that a change to it fails.

## What exists

- The daemon API is specified in ESS and its OpenAPI document and JSON Schemas are generated and checked
  (`story:daemon-api-specified-in-ess`, `task check:ess-generated`).
- The MCP tools are listed by `tools/list` (`story:mcp-exposure`; `docs/mcp.md`).
- The run directory has a page (`docs/loom-run-directory.md`), the settings a page
  (`docs/loom-run-settings.md`), the view file a page (`ui/README.md`, "The view file"), the command a page
  (`docs/cli.md`). `story:runbook-command-knows-the-text-layout` gives the command the layout of a checkout.
- Nothing lists them together, gives them one version, or lets a consumer run a check against a binary.

## What this story does

1. One committed contract file (`runbook.contract/1`) lists each interface with a member list and a digest of
   the thing it describes: the flags of each command, the members of the intake file the runner reads, the
   files and members of the `work/` layout, the members of the `.sdc/llm` record and of `calls.jsonl`, the
   members of a view file, the digest of the daemon's generated OpenAPI document, the names and schemas of the
   MCP tools. The generated parts are read from their sources, not copied by hand.
2. A command a consumer runs against the binary it pinned (`runbook contract --check`, the unit decides the
   name) checks every interface against stand-ins that ship with the binary and exits 0 or exits 1 naming the
   interface and the member that broke. It runs offline, makes no model call and starts no daemon outside the
   test.
3. The repository's gate fails when an interface changed and the contract file did not: a fast check compares
   the digests (a committed generated file with a digest of its sources). A change that breaks an interface
   raises the major of the contract and the `CHANGELOG.md` entry says breaking.
4. `docs/consumer-contract.md` tells a consumer how to run the check in its own gate and how to read a failure.

## Acceptance

- `runbook contract` prints the contract version and one line per interface with its digest;
  `runbook contract --check` on the binary it was built with exits 0 and prints each interface checked.
- A stand-in binary with one flag removed, one MCP tool renamed, one member of the `.sdc/llm` record dropped
  and one file of the `work/` layout moved each exits 1 naming that interface and member (four cases).
- Removing a flag in the source without changing the contract file fails the repository's check step, in under
  5 s; changing both passes it.
- The generated interfaces (daemon API, MCP schemas) are not written twice: changing the specification changes
  the contract's digest through the existing generation step.
- The consumer's check ends within 60 s; no test makes a model call or a network request.

## Scope

- `runbook/src/main.rs` (one command), a new `runbook/src/contract.rs`, a committed contract file under
  `docs/` or `ess/generated/`, `Taskfile.yml` (the fast check), `runbook/tests/`, `docs/consumer-contract.md`
  (new), `CHANGELOG.md`.

## Not in this story

- Freezing an interface: this story detects a change, it does not forbid one.
- The Canon and Loom contracts the runner links, which their own repositories publish.
