# Runbook plugin

A plugin for Claude Code and Codex with skills for building, recording and hardening a runbook: a
fixed, checked way to handle a case, where a program decides the next step from recorded facts and
a model only carries out single steps. The software delivery cycle in this repository is the first
runbook.

## Install

In Claude Code:

```console
/plugin marketplace add beyond10x/runbook
/plugin install runbook@runbook
```

For a local checkout, give the path of the checkout instead of `beyond10x/runbook`.

## Skills

| Skill | Use it to |
|---|---|
| `workflow-builder` | build a protocol-driven workflow to its first clean pass, then measure and improve it |
| `record` | turn a manual session into a draft runbook (a guided procedure, no automation) |
| `harden` | check a protocol with `canon check`, the route check, `canon simulate` and measured experiments |
| `delegation` | hand sub-agent work to Runbook in a Claude Code session with this plugin (below) |

`harden` and `record` work today inside a checkout of this repository: their commands are the
scripts and binaries here (`scripts/check-protocols.sh`, `scripts/check-bindings.sh`,
`sdc-check-routes`, `sdc-experiment`), and they need `runbook`, which links the pinned Canon (`runbook/Cargo.lock`).

## Delegation through Runbook (Claude Code)

With the plugin on, a Claude Code session's sub-agent work is served by Runbook: the daemon runs it as a
run on a named model route, records it and hands the result back. The design is `docs/model-routes.md`,
"Part two" (story `claude-plugin-serves-delegation`). Three parts, Claude Code only:

- `.mcp.json`: the MCP server `runbook`, started as `runbook mcp` with a per-server `timeout` of 3,600,000 ms
  (one hour per tool call) and no `env` of its own: it takes `RUNBOOK_DAEMON` from the session's environment
  (*inferred*: the Claude Code documentation does not say in words that a stdio server inherits it), and a
  session without it gets `runbook mcp`'s own "no daemon named" message. `runbook` must be on the session's
  `PATH` and `RUNBOOK_DAEMON` must name a running daemon (`docs/mcp.md`).
- `hooks/hooks.json`: a `PreToolUse` hook on `Agent|Workflow` that runs `hooks/delegation.sh pre-tool-use`
  (POSIX sh, `sed` and `tr` only, no network) with `onFailure: "block"`. It denies every Agent and every
  Workflow call with `permissionDecision: "deny"` and a reason naming the runbook `delegate` tool, the first
  non-blank line of the call's prompt (at most 200 characters) and the session's route: `RUNBOOK_MODEL_ROUTE`,
  or the daemon's default route when that is not set. A failed or timed-out hook blocks the call too, so no
  sub-agent runs on the session's own model.
- A `SessionStart` hook (`hooks/delegation.sh session-start`) and the skill `delegation` that say the same
  before the first delegation.

Why Workflow too: the Workflow tool runs a script whose `agent()` calls each start a sub-agent on the session's
model. The Claude Code documentation (code.claude.com/docs, workflows and hooks, read 2026-11-25) does not say
that an `agent()` call inside a workflow fires a `PreToolUse` hook for `Agent`; it says only that the workflow
agents' own tool calls go through the session's permission checks. So the hook denies the `Workflow` call itself.

`runbook mcp` serves `delegate` and `delegation_result` (`docs/mcp.md`, "The tools"): `delegate` runs the task
as a run on the route, waits for its end and answers the row of the view `delegation-result`, with the run's
`result.md` as `result_text`. Claude Code (2.1) hands the model that row, the tool's structured content, rather
than the text whose first line names the run. A route that is down is a tool error that names it: refused at the
submit as `route-unavailable` (HTTP status 503), or, when the route fails the run's call, a run that ended
`runtime-failure` whose `failure` names `route-unavailable` and the route, or a run that waited for its route and
ended `stopped: route`, the text naming the route and the last phase its gateway gave. The Agent and Workflow tools stay
denied either way. `describe_catalog` comes with the story `daemon-catalog-lists-what-a-delegation-may-name`.

A headless session delegates end to end in `runbook/tests/daemon/plugin_session.rs`: `claude -p` with
`--plugin-dir plugin` against a stand-in Messages server on loopback that answers the session's model calls (an
Agent call, then `delegate` as the deny reason says) and, as the daemon's gateway, the run's call through the
relay. The session runs with a fresh `HOME` and `CLAUDE_CONFIG_DIR`, `/` as its working directory,
`--setting-sources user`, an invented token and an HTTP proxy that refuses every connection: a `.claude`
directory above a working directory is read as the project's (its `settings.local.json` `env` can move
`ANTHROPIC_BASE_URL`), so the working directory is one with no such parent. Without a `claude` on `PATH` the two
cases print why and pass.

Enable it for one session only; nothing is installed and other sessions do not see it:

```console
RUNBOOK_DAEMON=http://127.0.0.1:<port> RUNBOOK_MODEL_ROUTE=<route> claude --plugin-dir <checkout>/plugin
```

or with a settings file passed as `claude --settings <file>`, which enables the plugin from the checkout's
marketplace for that session (the marketplace `runbook` is registered if Claude Code does not know it yet):

```json
{
  "extraKnownMarketplaces": {
    "runbook": { "source": { "source": "directory", "path": "<absolute path of the checkout>" } }
  },
  "enabledPlugins": { "runbook@runbook": true }
}
```

A launcher that starts a session for Runbook delegation also passes `--disallowedTools Agent Workflow`: the
model then never sees either tool, and the hook covers a session started with the plugin alone.

```console
claude --plugin-dir <checkout>/plugin --disallowedTools Agent Workflow
```

The hooks and the MCP configuration are Claude Code's; the Codex manifest lists the skills only.

## Planned

Not built yet, because they need a released `runbook` binary and commands that do not exist:

- `init`: set up a runbook in a project.
- `upgrade`: move a project to a newer runbook release.
- `ticket`: take a ticket through a runbook.

## Layout

- `.claude-plugin/marketplace.json` (repository root): the marketplace `runbook`, one plugin with
  source `./plugin`.
- `plugin/.claude-plugin/plugin.json` and `plugin/.codex-plugin/plugin.json`: the manifests. Their
  `version` is the repository's release version; `task release:check` fails when it differs.
- `plugin/skills/<skill>/SKILL.md`: the skills, loaded by both hosts.
- `plugin/.mcp.json`, `plugin/hooks/hooks.json` and `plugin/hooks/delegation.sh`: the delegation parts,
  Claude Code only. Their tests are in `driver/tests/plugin.rs`, run on the recorded hook input of
  `driver/tests/fixtures/plugin/`, and in `runbook/tests/daemon/plugin_session.rs` (a headless session, and
  the check that every tool, argument and result member the plugin names is one `runbook mcp` serves).
