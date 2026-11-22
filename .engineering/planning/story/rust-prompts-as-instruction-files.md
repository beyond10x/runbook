---
format: aep.planning-md/3
id: story:rust-prompts-as-instruction-files
kind: story
status: active
title: The model instructions of three Rust binaries are files outside the code
summary: implement-run request mode, sdc-conform and sdc-unstick read their prompts from instructions/ through one shared renderer; prompts byte-identical.
tags:
- conductor-succession
- loom
relations:
- decomposes: epic:conductor-succession
scope:
- confidence: cited
  path: CHANGELOG.md
- confidence: cited
  path: docs/driver.md
- confidence: cited
  path: driver/src/bin/sdc-conform.rs
- confidence: cited
  path: driver/src/bin/sdc-reply/implement.rs
- confidence: cited
  path: driver/src/bin/sdc-unstick.rs
- confidence: cited
  path: driver/src/instructions.rs
- confidence: cited
  path: driver/tests/bugfix_prompt.rs
- confidence: cited
  path: instructions
revision: 11
transitions:
- {from: "draft", to: "proposed", at: "2026-11-22T01:09:12Z", actor: "human:operator", revision: 2, executor: "agent:wave-rb2-u1-rust-prompts", correlation: "operator-standing-acceptance-2026-11-22"}
- {from: "proposed", to: "active", at: "2026-11-22T01:09:12Z", actor: "human:operator", revision: 3, executor: "agent:wave-rb2-u1-rust-prompts", correlation: "operator-standing-acceptance-2026-11-22"}
---
# Story: rust-prompts-as-instruction-files

## Outcome

The model instructions that three Rust binaries held as string literals are files under
`instructions/`, outside the code. The operator, 2026-11-21: "The instruction should come from the
outside and not be inside this driver binding." Three binaries are covered: `sdc-reply
implement-run` (the prompt of `SDC_IMPLEMENT_MODE=request`: head, build rules, task and reply,
rework, continuation and the retry texts), `sdc-conform` (the table node's prompt and its two
feedback texts) and `sdc-unstick` (the `run.unstick` prompt). One shared function,
`driver/src/instructions.rs`, reads and fills them with the rules of `render` in
`bindings/tools/llm-node.sh`; the binaries find `instructions/` above their own place, as the shell
tools do.

## Acceptance

- The prompt each of the three binaries writes, held in golden files under
  `instructions/golden/{implement,conform,unstick}/` before the text moved, is byte-identical after
  it (`git diff` of the goldens between the commit before the move and the head is empty).
- `sdc-reply/implement.rs`, `sdc-conform.rs` and `sdc-unstick.rs` hold no multi-line model
  instruction literal, and `bugfix_prompt` fails when an instruction line of `instructions/` is
  found in them.
- An instruction path that is not `instructions/<...>.md`, is absolute or has a `..` segment is
  refused by the shared renderer, and a placeholder without a value is an error (`bugfix_prompt`).
- An instruction file that no golden reaches fails `bugfix_prompt`, the new files included.
