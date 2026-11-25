---
format: aep.planning-md/3
id: story:daemon-caps-are-keys-of-one-file
kind: story
status: active
title: 'The daemon''s caps are keys of one file: per run, per day, one override per run, a stop that names the key'
summary: A daemon ledger with a figure per run and per day; every cap a documented key of the daemon's configuration file; a stop outcome that names the key; never unbound without the operator writing none.
tags:
- upstream-asks-2026-11-24
refs:
- provider: consumer
  reference: upstream-asks-filed/A2
relations:
- depends_on: story:daemon-run-names-its-caps
- depends_on: story:runbook-toml-carries-defaults
- depends_on: story:runs-share-a-ledger-without-reserving-the-cap
- informed_by: story:caps-are-optional
- informed_by: story:daemon-runs-take-autonomy-and-a-decider
scope:
- confidence: inferred
  path: docs/daemon-config.md
- confidence: cited
  path: docs/engine.md
- confidence: cited
  path: docs/mcp.md
- confidence: inferred
  path: ess/domains/daemon.yaml
- confidence: cited
  path: runbook/src/engine
- confidence: cited
  path: runbook/src/loom_run/ledger.rs
- confidence: inferred
  path: runbook/src/mcp/schema.rs
- confidence: inferred
  path: runbook/src/serve.rs
- confidence: inferred
  path: runbook/tests/daemon
revision: 5
transitions:
- {from: "draft", to: "proposed", at: "2026-11-25T12:41:41Z", actor: "agent:runbook", revision: 4}
- {from: "proposed", to: "active", at: "2026-11-25T12:41:41Z", actor: "agent:runbook", revision: 5}
---
## Outcome

Every cap of a run the daemon starts is a documented key of one configuration file. The daemon keeps a
spend ledger with a figure per run and a figure per day. A submit may override a key for its own run, once
per key. A run that reaches a cap ends with an outcome that names the key.

## Source

Ask A2 of a consumer, filed in its story `upstream-asks-filed` (read 2026-11-24): a daemon run has no ledger,
no wall-time cap and no model-call cap, and a consumer runs the daemon every day. The operator's words of
2026-11-23 22:15 CET, quoted in that story verbatim:

- "we should never run unbound"
- "we need an easy way to extend or expand these caps via configuration"

## What exists

- `story:daemon-run-names-its-caps` (proposed): `submit_run` takes optional wall time, call cap, spend cap and
  ledger; the daemon's start options can state a bound for every run, a submit may name a lower one and never
  a higher one. It names no per-day figure, no file of keys and no key in the stop outcome. Its `max_wall_seconds`
  part is built on the branch `impl/daemon-page-shows-a-run-tree-and-products` (not merged).
- `story:caps-are-optional` (active): a cap is optional, none named means no limit, a named cap stops the run.
- `story:runbook-toml-carries-defaults` (active): the `runbook.toml` the binary finds carries the defaults of
  `runbook run`, the caps among them.
- `story:spend-cap-with-nothing-left-starts-nothing`: a named cap with nothing left starts nothing.

## Conflict to settle: "never unbound" and "none named means no limit"

`story:caps-are-optional` records the operator's earlier word (2026-11-23) that runbook has no money cap of its
own. The two sentences above are about runs nobody watches. Default taken here, reversible by editing this
story: `runbook run` keeps `story:caps-are-optional` unchanged. A run the daemon starts is never silently
unbound: every key either has a value in the file or the submit, or is written `"none"` in the file by the
operator. A daemon whose file leaves a key out refuses to start a run that has a model node, naming the key and
the line to add. Writing `"none"` is the operator's decision and is printed by `runbook config`.

## What this story does

1. The daemon's configuration file (the `runbook.toml` it reads) names the caps as keys. Per run:
   `max_wall_seconds`, `max_llm_calls`, `cap_micro_usd`. Per day: `day_max_llm_calls`, `day_cap_micro_usd`. The
   unit fixes the exact names and lists every key, its unit, its default (none: a key is stated) and its place
   in `docs/daemon-config.md`. An unknown key is refused by name when the daemon starts.
2. Raising or lowering a cap is a change of the file, read at the next start of a run. No code change and no
   restart of a running run.
3. The daemon keeps one ledger in its state directory with the spend and the calls of each run and of each day
   (the day is a UTC date), under the file lock, shared by every run it starts. A run's figure and the day's
   figure are both held: the first one reached stops the run.
4. A submit may name a value for a key, at most once per key per run. It may lower a key. Raising a key is the
   operator's, by the file. The value that holds is written in the run's record.
5. A run that reaches a cap ends with the outcome of a reached cap and names the key: the outcome file, the
   event and the view of the run carry `cap_key` (for example `day_cap_micro_usd`), and the stop line reads
   `stopped: cap <key>`.
6. A cap of the day applies to a run that starts after the cap was reached: it starts nothing and ends with the
   same outcome and key.

## Acceptance

With the stub runtime, a stand-in model program and the daemon's test clock (no model call, no network):

- A file with the five keys and a submit that names none: the run holds the file's values (read from the run's
  record).
- A submit that names `max_llm_calls` lower than the file: that value holds; a higher one is refused by name
  before the run starts.
- A run that reaches each of the five keys ends with the outcome of a reached cap and `cap_key` equal to that
  key; the stop line names it.
- Two runs started together under one day figure with room for both end as ended
  (`story:runs-share-a-ledger-without-reserving-the-cap`); a third that starts after the day figure is reached
  starts no program.
- A day ends at 00:00 UTC on the test clock: the next run starts with the day figure at 0.
- A file that leaves a key out, with a run that has a model node: the run is refused naming the key; the same
  file with the key written `"none"` runs without that bound.
- An unknown key in the file is refused when the daemon starts, by name.
- `runbook config` prints every cap key with its value and its place (file, submit, built-in).
- Each case runs under 5 s; no test makes a model call.

## Scope

- `ess/` (the daemon's specification: the outcome member `cap_key`), `runbook/src/engine/`,
  `runbook/src/config.rs`, `runbook/src/serve.rs`, `runbook/src/mcp/schema.rs`, `runbook/src/loom_run/ledger.rs`,
  `runbook/tests/daemon/`, `docs/daemon-config.md` (new), `docs/engine.md`, `docs/mcp.md`, `CHANGELOG.md`.

## Not in this story

- A decider of the daemon and the gateway of its runs (`story:daemon-runs-take-autonomy-and-a-decider`,
  `story:daemon-runs-take-a-gateway`).
- A per-week or per-month figure.
