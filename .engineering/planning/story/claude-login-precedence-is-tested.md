---
format: aep.planning-md/3
id: story:claude-login-precedence-is-tested
kind: story
status: proposed
title: The claude-cli login precedence is tested and disagreement reported
summary: SDC_CLAUDE_CREDENTIALS silently beats CLAUDE_CONFIG_DIR beats HOME and no test shows it (review rb2 S3-5).
relations:
- decomposes: epic:conductor-succession
scope:
- confidence: cited
  path: bindings/tools/llm-adapter-claude-cli.sh
- confidence: cited
  path: bindings/tools/llm-node.sh
- confidence: cited
  path: driver/tests/questions_check.rs
revision: 5
transitions:
- {from: "draft", to: "proposed", at: "2026-11-22T02:42:21Z", actor: "human:operator", revision: 2, executor: "agent:rb2-driver-fixes", correlation: "operator-standing-acceptance-2026-11-22"}
---
# Story: claude-login-precedence-is-tested

## Outcome

The login of the claude-cli adapter is chosen by a tested precedence, and two sources that
disagree are reported.

## Context

Source: review rb2, driver, S3-5: `bindings/tools/llm-adapter-claude-cli.sh:23-31`
(`SDC_CLAUDE_CREDENTIALS` silently beats `CLAUDE_CONFIG_DIR` beats `HOME`; with `ANTHROPIC_API_KEY`
set a wrong `--claude-credentials` is never looked at), `bindings/tools/llm-node.sh:91` (an empty
`--claude-credentials ""` reads as not given), `driver/tests/questions_check.rs:596-630` (proves
only "no variable" and "file exists"; the stand-in `claude` checks that `.credentials.json` exists,
not that it is the file named).

## Acceptance

A test per source order (each variable alone, two that disagree, a directory, a missing file, a
symlink) shows which login the adapter copies, and the stand-in `claude` fails when the copy is not
the file the winning source named.
