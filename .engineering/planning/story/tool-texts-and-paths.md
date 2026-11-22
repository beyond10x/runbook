---
format: aep.planning-md/3
id: story:tool-texts-and-paths
kind: story
status: active
title: The shell tools hold no model text, no home path and no unset variable under Loom
summary: in/feedback.md and the requester request texts move to instructions/; the three beyond10x/sdc fallbacks go; the claude-cli adapter runs with PATH and LOOM_* alone.
relations:
- decomposes: epic:conductor-succession
scope:
- confidence: cited
  path: CHANGELOG.md
- confidence: cited
  path: bindings/PENDING.md
- confidence: cited
  path: bindings/tools/decider.sh
- confidence: cited
  path: bindings/tools/llm-adapter-claude-cli.sh
- confidence: cited
  path: bindings/tools/llm-node.sh
- confidence: cited
  path: bindings/tools/sdc-tool.sh
- confidence: cited
  path: docs/loom-bindings.md
- confidence: cited
  path: driver/tests/bugfix_prompt.rs
- confidence: cited
  path: driver/tests/questions_check.rs
- confidence: cited
  path: instructions
- confidence: cited
  path: scripts/run-finding-cycles.sh
revision: 14
transitions:
- {from: "draft", to: "proposed", at: "2026-11-22T00:57:52Z", actor: "human:operator", revision: 2, executor: "agent:rb2-u2-tool-texts", correlation: "operator-standing-acceptance-2026-11-22"}
- {from: "proposed", to: "active", at: "2026-11-22T00:57:52Z", actor: "human:operator", revision: 3, executor: "agent:rb2-u2-tool-texts", correlation: "operator-standing-acceptance-2026-11-22"}
---
# Story: tool-texts-and-paths

## Outcome

The shell tools under `bindings/tools/` are tied to nothing the old driver or one machine
provided. The model-facing text of `in/feedback.md` and of the requester request is in files
under `instructions/`, rendered by `render`; no tool falls back to a home path that no longer
exists; and the real adapter works when Loom gives a program `PATH` and `LOOM_*` alone.

## Acceptance

- The `in/feedback.md` of a failed check and of a writer's own failed run, and the requester
  request of an approval and of a close, are byte-identical before and after the text moves
  out of `llm-node.sh` and `decider.sh` (goldens under `instructions/golden/`, written from the
  code first and compared by `driver/tests/bugfix_prompt.rs`).
- `llm-node.sh`, `sdc-tool.sh` and `scripts/run-finding-cycles.sh` name no
  `beyond10x/sdc` path default; a missing `sdc-findings` or `sdc-finding-kpis` stops the tool
  with a message that names the binary and where it looked.
- `llm-adapter-claude-cli.sh` run with an environment of `PATH` and `LOOM_*` alone (no `HOME`)
  does not fail on an unset variable: it takes the login from `SDC_CLAUDE_CREDENTIALS`,
  `CLAUDE_CONFIG_DIR` or `HOME`, or stops with a message naming those variables; a test in the
  Loom-mode module of `driver/tests/questions_check.rs` shows it, and fails when the fix is
  reverted.
- `driver/tests/bugfix_prompt.rs` and `driver/tests/questions_check.rs` pass, and
  `scripts/check-bindings.sh` exits 0.

## Scope

`bindings/tools/llm-node.sh`, `bindings/tools/decider.sh`, `bindings/tools/sdc-tool.sh`,
`bindings/tools/llm-adapter-claude-cli.sh`, `scripts/run-finding-cycles.sh` (the fallback only),
`instructions/`, `driver/tests/bugfix_prompt.rs`, `driver/tests/questions_check.rs`,
`docs/loom-bindings.md`, `bindings/PENDING.md`, `CHANGELOG.md`.
