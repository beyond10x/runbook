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

`harden` and `record` work today inside a checkout of this repository: their commands are the
scripts and binaries here (`scripts/check-protocols.sh`, `scripts/check-bindings.sh`,
`sdc-check-routes`, `sdc-experiment`), and they need `runbook`, which links the pinned Canon (`runbook/Cargo.lock`).

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
