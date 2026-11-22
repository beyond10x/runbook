---
format: aep.planning-md/3
id: story:orphan-instruction-test-is-exact
kind: story
status: proposed
title: The test that a golden reaches every instruction file is exact
summary: A line counts as reached through any golden and lines under 12 characters never count (review rb2 S3-7).
relations:
- decomposes: epic:conductor-succession
scope:
- confidence: cited
  path: driver/tests/bugfix_prompt.rs
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-11-22T02:42:20Z", actor: "human:operator", revision: 2, executor: "agent:rb2-driver-fixes", correlation: "operator-standing-acceptance-2026-11-22"}
---
# Story: orphan-instruction-test-is-exact

## Outcome

The test that every instruction file is reached by a golden is exact: a file whose text is
reached only through another file's lines fails it.

## Context

Source: review rb2, driver, S3-7: `driver/tests/bugfix_prompt.rs:539`
(`every_instruction_file_is_reached_by_a_golden`) counts a line as reached when it is a substring of
any golden, all groups concatenated, and ignores lines under 12 characters.

## Acceptance

An instruction file whose every line of 12 characters or more occurs in the goldens of other files
and in none of its own golden group fails `every_instruction_file_is_reached_by_a_golden`.
