---
format: aep.planning-md/3
id: story:interview-question-context
kind: story
status: proposed
title: Interview questions carry excerpt, consequences and default reason
relations:
- decomposes: epic:decisions-a-person-can-take-cold
scope:
- confidence: inferred
  path: driver/examples/sdc/fixtures/questions-answered.yaml
- confidence: inferred
  path: driver/examples/sdc/fixtures/questions-open.yaml
- confidence: inferred
  path: driver/examples/sdc/fixtures/scope-question
- confidence: inferred
  path: driver/examples/sdc/tools/llm-node.sh
- confidence: inferred
  path: driver/examples/sdc/tools/sdc-tool.sh
revision: 5
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T01:30:36Z", actor: "human:operator", revision: 3}
---
# Story: interview-question-context

## Outcome

Every interview question says where it comes from and what each answer changes, so the answer can
be judged without reading the run: the excerpt of the idea and the dissection item that raised it,
a consequence per option (what is built or not built), and the reason for the default.

## Acceptance

`questions.check` exits 2 on a scope-changing question that lacks `excerpt`, a `consequences` entry
for every option, or `default_reason`, and exits as before on the same file with all three; the
interview prompt of `questions.write` asks for them, and a recorded questions fixture shows the
three keys on the scope-changing questions of the trial-2 idea.

## Scope

The interview prompt in `driver/examples/sdc/tools/llm-node.sh` (`questions.write`, `:136-147`), the
`check-questions` branch of `driver/examples/sdc/tools/sdc-tool.sh` (`:52-70`), and the questions
fixtures in `driver/examples/sdc/fixtures/` (`questions-open.yaml`, `questions-answered.yaml`,
`scope-question/`). Bound to `driver/`: waits for the trial in progress to end.

## Tooling note

- Changes the existing bash tool `driver/examples/sdc/tools/llm-node.sh`: allowed, a fix to prototype glue; no new bash tool.
- Changes the existing bash tool `driver/examples/sdc/tools/sdc-tool.sh`: allowed, a fix to prototype glue; no new bash tool.
(AGENTS.md, "Rules": anything new that runs is Rust; existing bash tools get fixes only.)
