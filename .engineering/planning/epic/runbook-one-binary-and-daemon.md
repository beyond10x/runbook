---
format: aep.planning-md/3
id: epic:runbook-one-binary-and-daemon
kind: epic
status: proposed
title: 'Runbook as one binary and a daemon: Canon and Loom linked, API, SSE, MCP, embedded UI, confinement per run, presets'
summary: One prebuilt runbook binary that runs a case in process and serves many runs as a daemon, each at a chosen confinement level, with presets improved through the evaluations; the driver runtime is removed after parity.
tags:
- one-binary
- operator-requirement
relations:
- decomposes: initiative:roadmap
- informed_by: story:runbook-cli-skeleton
- informed_by: story:sdc-on-loom-tree-run
- informed_by: story:canon-0-10-0-pin
- informed_by: epic:live-view
- informed_by: epic:improvement-loop
- informed_by: story:confined-bound-commands
- informed_by: story:loom-concurrent-children
revision: 2
transitions:
- {from: "draft", to: "proposed", at: "2026-11-22T01:44:58Z", actor: "human:operator", revision: 2, executor: "agent:plan-onebin", correlation: "operator-requirement-2026-11-22-one-binary"}
---
# Epic: runbook-one-binary-and-daemon

## Source

The operator, 2026-11-22 (`human:operator`), verbatim:

- "runbook should not rely on binaries [of] canon, loom. Instead, it should just embed it as a dependency directly."
- "What we want is one pre-built runbook binary, which can just run a certain case, given that context, the intake, and given the protocol which is being specified, and given certain command line arguments."
- "there should also be a runbook daemon, runbookd. It has a simple API where we submit something, and then we seek events over SSE."
- "All of this should be ESS specified, of course."
- "using the CLI would allow us to start a case, and this would either happen basically in process or via a daemon, and the daemon would maintain all the different runs. And different sessions."
- "we also want MCP exposure, and we want the web UI to be embedded into the binary or into the server. So when we do runbook serve, or runbookd the daemon, then we should just also specify --ui ... and then it would ... serve it besides the API itself. And multiple such runs should be possible to run there."
- "of course we also want to specify the confinement, the level of sandboxing."
- "the runbook basically is a utility daemon which is able to run multiple such cases in an isolated, secure way, offering an API to control it, and it comes with protocol presets which are optimized and self-improved through the evals."
- On the parity check: "can we just run one of the evals where we knew that was working fine? Can we now run it through Loom and basically compare it?"
- On the removal: "as soon as we know the loom stuff is basically working and there is parity towards that dummy runtime we implemented, I would really like to remove that runtime itself from runbook."

Earlier words that still bind: "It's runbook, it's specs, it's text, not code"; one copy of the runtime
(Loom's); "The instruction should come from the outside and not be inside this driver binding."

## Outcome

Runbook is one prebuilt binary, `runbook`, that links Canon and Loom as Rust library crates, pinned
by commit in the cargo lock file. It runs one case in process (`runbook run`) and, as `runbookd`
(the same binary, equal to `runbook serve`), serves many runs at once through an API, an SSE event
stream, MCP and the embedded web UI. Each run has a confinement level, and the daemon has a floor
below which it refuses a run. The binary carries the repository's runbooks as presets, which the
evaluations measure and improve from release to release. The API is specified in ESS first. The
`sdc-drive` runtime copy and its scripts are deleted once a run on Loom matches the driver's.

Design decisions of this epic (design authority: the Runbook session; reversible unless marked):

1. One crate `runbook/`, one binary; no runtime binary (`canon`, `b10x-loom`, `sdc-drive`) is needed at run time. `runbookd` is a second name of the same binary.
2. One engine module `runbook::engine`: start a run (runbook, intake, context, settings, confinement, state directory, event sink) and control it (answer a decision, cancel). `runbook run` calls it in process; `runbook serve` calls the same code for many runs; the CLI talks to a daemon when `--daemon <url>` or `RUNBOOK_DAEMON` is given.
3. Commands: `runbook run <runbook> --intake <file|-> [--context <dir>] [--set key=value]... [--confinement <level>] [--detach]`; `runbook serve [--listen <addr>] [--ui] [--mcp] [--min-confinement <level>]`; `runbook runs list|show|events|cancel`; `runbook decide <run> <request> <answer>`; `runbook mcp` (stdio); the existing `doctor`, `list`, `check`.
4. API: HTTP with JSON bodies: submit, list, read, cancel a run; list and answer decision requests; sessions (a named group of runs of one client with its own event stream). SSE per run, per session and for the whole daemon; each event has an id and a client resumes with `Last-Event-ID`; an event carries the runtime's record unchanged plus run id and sequence number.
5. The API is specified in ESS first (`ess/`): entities, commands, views and the event stream; OpenAPI and JSON Schemas are generated from it; a conformance suite runs against the daemon. No handler is written before its command is in the specification.
6. MCP exposes the same commands as tools. HTTP, MCP and the CLI are three faces of one command layer.
7. The files of `ui/web/` are embedded at build time and served by `runbook serve --ui` beside the API; the Node server (`ui/server.mjs`, `ui/decisions.mjs`) is removed when the UI no longer needs it.
8. Confinement: per run (`--confinement`), with a daemon floor (`--min-confinement`); the levels are the ones the runtime offers through Substrate; the default is the confined level; unconfined must be asked for by name; below the floor is refused.
9. Presets: the repository's runbooks are embedded and `runbook list` shows each with its version and measured quality from `experiments/releases.yaml`; `--runbook-dir <dir>` runs one from outside. A preset changes only through a release with a registered hypothesis and measured runs.
10. Security floor: loopback by default; any other address needs a bearer token and the daemon refuses to start without one; the same check guards API, SSE, UI and MCP; no secret in an event; one state directory per run (`$XDG_STATE_HOME/runbook/runs/<id>`); the daemon lists its runs again after a restart.
11. One prebuilt binary per release for Linux x86_64 first, attached to the GitHub Release (a publication: the operator's word per release, as today).
12. End state: `driver/src/main.rs`, `scripts/run-trial.sh`, `scripts/run-finding-cycles.sh`, `bindings/tools/llm-node.sh`, `bindings/tools/decider.sh`, `driver/canon-pin.json` (the pin is the cargo lock file), the Canon binary resolution of `scripts/evaluate-cases.sh` and the Node UI server are gone.

## Stories, in the order of the design

| | story | depends on |
|---|---|---|
| A | `story:library-entries-from-loom-and-canon` | none, starts now |
| B | `story:daemon-api-specified-in-ess` | none, starts now |
| C | `story:daemon-skeleton-on-a-runtime-trait` | B |
| D | `story:engine-on-loom-library-entry` | A, C |
| E | `story:run-in-process-and-cli-as-daemon-client` | C, D |
| F | `story:ui-embedded-in-the-binary` | C, D |
| G | `story:mcp-exposure` | C |
| H | `story:confinement-level-per-run` | C, D |
| I | `story:presets-embedded-and-runbook-test` | D, E |
| J | `story:prebuilt-release-binary` | E, F, I |
| K | `story:parity-run-on-loom` | D, E |
| L | `story:driver-runtime-removed` | K |

## Relation to what exists

Continues `story:runbook-cli-skeleton` (the crate, `doctor`, `list`; its "later stories add `run`, `ui`")
and `story:sdc-on-loom-tree-run` (the cycle on Loom's tree run). Changes the premise of
`story:canon-0-10-0-pin` (no Rust dependency on Canon; the pin in `driver/canon-pin.json`).
Moves the scope of `epic:live-view` and the `ui-*` stories (they name `ui/server.mjs`). Overlaps
`story:confined-bound-commands` (H) and `story:loom-concurrent-children` (L deletes the same
driver code). `epic:improvement-loop` and `sdc-experiment` start runs through
`scripts/run-trial.sh`, which L deletes. None of these is edited by this epic; the orchestrator
decides.

## Out of Scope

- A second target platform (macOS, Windows) and crates.io publication.
- A multi-user or multi-tenant daemon: one token, one operator.
- New runbooks; new evaluations beyond K.
- Building the capabilities that are Loom's, Canon's or Substrate's (A asks for them).

## Risks

- Loom or Canon has no stable library entry for what D needs (A finds out first; the upstream blockers it files carry the request).
- A linked runtime makes a Runbook build slow and large; the test steps stay within 60 s per test and 120 s per step (`AGENTS.md`, "Tests stay fast").
- The Loom bindings still run `bindings/tools/llm-node.sh` and `decider.sh` until Loom has its agent executor and decider configuration; L waits on that (see its Context).

## Done When

The stories are implemented, and `runbook test cycles/suite.yaml` (the suite run by Loom's suite
runner, linked in) prints every entry as expected; the process tree of a run holds `runbook` and
the bound programs of the bindings and no `sdc-drive`, `canon` or `b10x-loom`; the files of
decision 12 are deleted.
