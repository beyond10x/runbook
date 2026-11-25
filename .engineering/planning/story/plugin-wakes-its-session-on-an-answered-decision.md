---
format: aep.planning-md/3
id: story:plugin-wakes-its-session-on-an-answered-decision
kind: story
status: draft
title: A plugin hook follows a session's events and wakes the host session when a decision is answered
summary: Nothing tells a host session that a person answered a decision of its runs; the plugin follows session-events and wakes the session on each answer.
tags:
- consumer-asks-2026-11-25
relations:
- depends_on: story:daemon-runs-take-a-gateway
- informed_by: story:runbook-plugin
- informed_by: story:daemon-skeleton-on-a-runtime-trait
scope:
- confidence: cited
  path: docs/daemon-api.md
- confidence: inferred
  path: plugin
- confidence: cited
  path: plugin/.claude-plugin/plugin.json
- confidence: cited
  path: plugin/.codex-plugin/plugin.json
- confidence: cited
  path: plugin/README.md
- confidence: inferred
  path: runbook/src/api/sse.rs
- confidence: cited
  path: runbook/src/client/sse.rs
- confidence: cited
  path: runbook/src/commands.rs
- confidence: inferred
  path: runbook/src/main.rs
- confidence: inferred
  path: runbook/tests/daemon/sse.rs
revision: 6
---
## What is missing

The daemon streams the events of every run of a session (`session-events`, `docs/daemon-api.md:170`) as server-sent events, resumed by position without loss or repeat (`docs/daemon-api.md:188-227`), and the `runbook` command already has a client of such a stream (`runbook/src/client/sse.rs`, `runbook runs events`). The plugin carries skills only: `plugin/README.md` lists three skills and plans three more, and the manifests (`plugin/.claude-plugin/plugin.json`, `plugin/.codex-plugin/plugin.json`) name no hook. Nothing tells a host session that a person answered a decision of one of its runs: a session that waits on a run has to poll the view `open-decisions` or read the stream itself.

One fact needs checking first. The answer of the command `answer-decision` publishes a domain event `DecisionAnswered` (`runbook/src/commands.rs:897-898`, `docs/daemon-api.md:77-85`); a row of the event stream is a run-log line of the runtime or a `runbook.*` event the daemon writes (`docs/daemon-api.md:163-186`). The loom runtime writes a run-log line of the kind `decision_answered` (`runbook/src/loom_run/run_files.rs:56-59`); the daemon writes `runbook.answer-undeliverable` only when an answer cannot be delivered (`runbook/src/commands.rs:894`). Whether every runtime's stream carries one row per answer, and under which kind, is not stated in `docs/daemon-api.md`.

## Outcome

1. A stream row marks every answer of a decision request of a session's runs, under one kind that `docs/daemon-api.md` names. Where a runtime's stream carries none, the daemon writes `runbook.decision-answered` (`request_id`, `answered_by`, `principal`) in the same step as the answer, as it writes `runbook.answer-undeliverable`.
2. The plugin ships a hook that follows `session-events` of a named session and wakes the host session on each such row. The follower is a Rust command of `runbook` that reuses `runbook/src/client/sse.rs` and prints one line per answer; the hook starts it and the host wakes on its lines. The story names the hosts it covers (Claude Code, Codex) and says so for a host whose plugin format carries no hook.
3. The follower resumes by position after a dropped connection and stops when the session's stream ends (`docs/daemon-api.md:213-216`).

## What a guard must hold

- The follower only reads: it never answers, cancels or submits.
- The token is read from a file (`--token-file`) or the environment value the client already takes; it is in no argument, no printed line and no log.
- One wake per answer: a reconnect with the last position repeats none, and a row at or below the printed position is never printed again (the rule of `runbook runs events`).
- The printed line carries the run id, the request id, `answered_by` and the principal, not the answer text; a host reads the answer through the view `decisions`.
- A host without a daemon, or a daemon that is down, makes the hook end with a named message and does not hang the host session.

## Acceptance

Against the stub runtime and a daemon on loopback: a client opens a session, submits a run that stops on a decision and answers it; the follower of that session prints exactly one line naming that run and request; a second answer to the same request is refused `DecisionAlreadyAnswered` and prints no line; after the follower's connection is cut and reopened, no line repeats and an answer given during the gap is printed once. The same holds for a fixture of the loom runtime. A check fails when the manifests or the hook file of the plugin name a repository path that does not exist (the rule of `story:runbook-plugin`) and `task release:check` still holds the versions equal.

## Scope

`plugin/`, `plugin/.claude-plugin/plugin.json`, `plugin/.codex-plugin/plugin.json`, `plugin/README.md`, `runbook/src/client/sse.rs`, `runbook/src/main.rs`, `runbook/src/commands.rs`, `runbook/src/api/sse.rs`, `docs/daemon-api.md`, `runbook/tests/daemon/sse.rs`.

## Depends on

- `story:daemon-runs-take-a-gateway`: the order is behind the gateway release; there is no code dependency.
- Informed by `story:runbook-plugin` (the plugin layout and its release check) and `story:daemon-skeleton-on-a-runtime-trait` (the event streams of its part C2).
- Order: third, after `story:mcp-opens-and-closes-sessions`; a consumer needs it once its runs wait on a person.
