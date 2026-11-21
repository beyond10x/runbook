---
format: aep.planning-md/3
id: story:runbook-plugin
kind: story
status: proposed
title: Bundle an installable runbook plugin in the repository
relations:
- decomposes: epic:conductor-succession
scope:
- confidence: cited
  path: .claude-plugin
- confidence: cited
  path: AGENTS.md
- confidence: cited
  path: README.md
- confidence: cited
  path: Taskfile.yml
- confidence: cited
  path: docs/releasing.md
- confidence: cited
  path: plugin
- confidence: cited
  path: skills
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T19:36:21Z", actor: "human:operator", revision: 2, executor: "agent:runbook-plugin", correlation: "operator-standing-acceptance-2026-11-21"}
---
# Story: runbook-plugin

## Outcome

The repository carries an installable Claude Code and Codex plugin named `runbook` with the skills
that can be written today (`workflow-builder`, `harden`, `record`). People add the repository as a
marketplace and install the plugin from it.

## Context

The operator, 2026-11-21: "I would also like to bundle a Claude plugin with this project or repo
here, so people can actually install it from there and use the skills and such." `init`, `upgrade`
and `ticket` need a release binary and commands that do not exist; they are named as planned in
the plugin's README and not built.

## Acceptance

`.claude-plugin/marketplace.json` (marketplace and plugin `runbook`, source `./plugin`) and the
plugin manifests carry the repository's release version; `task release:check` fails when they
differ. A check fails when a repository path named in `plugin/skills/**/SKILL.md` does not exist.
`claude plugin validate` passes on the repository and on `plugin/`, and the plugin installs from
the local marketplace into a scratch configuration directory.

## Scope

`plugin/`, `.claude-plugin/`, `skills/workflow-builder/` (moved to `plugin/skills/`), the release
check in `Taskfile.yml` and `docs/releasing.md`, `README.md` (one section), `AGENTS.md` (one entry).

## Ambiguities

- None open. The repository address in the install commands changes with the rename
  (`story:project-rename`).

## Acceptance note

- Accepted on creation under `approval-record:operator-standing-acceptance-2026-11-21`.
