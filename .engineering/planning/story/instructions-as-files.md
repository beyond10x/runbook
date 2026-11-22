---
format: aep.planning-md/3
id: story:instructions-as-files
kind: story
status: active
title: The instructions of the agent nodes and the decider are files outside the binding
summary: instructions/<action>.md per agent action, instructions/decider/ and shared/, named by the llm object of each JSON binding; llm-node.sh and decider.sh only read and fill placeholders; prompts byte-identical.
tags:
- conductor-succession
- loom
relations:
- decomposes: epic:conductor-succession
scope:
- confidence: cited
  path: CHANGELOG.md
- confidence: cited
  path: bindings/PENDING.md
- confidence: cited
  path: docs/driver.md
- confidence: cited
  path: driver/examples/sdc/sdc-intake.bindings.json
- confidence: cited
  path: driver/examples/sdc/tools
- confidence: cited
  path: driver/tests/bugfix_prompt.rs
- confidence: cited
  path: instructions
revision: 10
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T22:49:57Z", actor: "human:operator", revision: 9, executor: "agent:wave-rb1-instructions", correlation: "operator-standing-acceptance-2026-11-21"}
- {from: "proposed", to: "active", at: "2026-11-21T22:49:57Z", actor: "human:operator", revision: 10, executor: "agent:wave-rb1-instructions", correlation: "operator-standing-acceptance-2026-11-21"}
---
# Story: instructions-as-files

## Outcome

The instructions of the agent nodes and of the decider are files outside the driver binding. The
operator, 2026-11-21: "The instructions, they should be part of what is specified in the YAML. The
instruction should come from the outside and not be inside this driver binding." Each of the 14
agent actions has `instructions/<action>.md`, the decider's prompts are under
`instructions/decider/`, the shared frame under `instructions/shared/`; variable parts are
`{{name}}` placeholders. `llm-node.sh` and `decider.sh` only read a file and fill the placeholders.
Each JSON binding names its file in the `llm` object (`instructions`).

## Acceptance

- The prompt each of the 14 actions writes on the stub inputs, and each decider prompt, is
  byte-identical before and after the move (golden files under `instructions/golden/`, compared by
  a test of the driver crate).
- `llm-node.sh` and `decider.sh` hold no instruction prose.
- `scripts/check-bindings.sh`, `aep plan artifact validate` and the driver tests that touch the
  tools pass; three finding cycles give the same verdict lines before and after.
- `bindings/PENDING.md` has a line for the `instructions` field, which `loom.bindings/1` cannot hold.

## Scope

`instructions/`, `driver/examples/sdc/tools/llm-node.sh`, `driver/examples/sdc/tools/decider.sh`,
`driver/examples/sdc/*.bindings.json`, `driver/tests/bugfix_prompt.rs`, `bindings/PENDING.md`,
`docs/driver.md`, `CHANGELOG.md`.
