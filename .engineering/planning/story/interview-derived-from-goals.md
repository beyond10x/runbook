---
format: aep.planning-md/3
id: story:interview-derived-from-goals
kind: story
status: proposed
title: Interview answers derived from goals
summary: A question a goal entry answers is answered derived with its basis, and the check refuses a derived answer without a valid one.
tags:
- requester
relations:
- decomposes: epic:requester-side-and-goals-context
- depends_on: story:requester-decider
scope:
- confidence: inferred
  path: driver/examples/sdc/sdc-intake.bindings.json
- confidence: inferred
  path: driver/examples/sdc/tools/llm-node.sh
- confidence: inferred
  path: driver/examples/sdc/tools/sdc-tool.sh
- confidence: inferred
  path: protocol/sdc-intake.protocol.yaml
revision: 5
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T01:30:36Z", actor: "human:timo", revision: 3}
---
# Story: interview-derived-from-goals

## Outcome

The interview does not ask what the goals already answer: a question an entry answers with strong
or moderate grounds is answered `derived` with its basis (entry and digest), non-scope questions at
every level and scope-changing ones where the class holder is the decider; a derived answer without
a valid basis does not count.

## Acceptance

On the trial-2 fixture with the example goals at A3 and the recorded requester reply fixture,
`questions.derive` writes q1 and q2 with `answered_by: derived` and a basis naming `product/p1` and
`product/p2`, and `check-questions` exits 3 on a copy where a scope-changing answer says `derived`
with no basis and returns `finding` on a copy where `product/p2`'s statement changed after the
derivation.

## Scope

New action `questions.derive` in `protocol/sdc-intake.protocol.yaml` and its binding, the
`check-questions` branch of `driver/examples/sdc/tools/sdc-tool.sh` (`:52-69`; closes the gap that
any `answered_by` but empty or `non-interactive` passes, `:65-67`), and the `questions.write` prompt
in `driver/examples/sdc/tools/llm-node.sh` (no default that crosses a constraint). Waits for the
running trial to end.

## Tooling note

- Changes the existing bash tool `driver/examples/sdc/tools/llm-node.sh`: allowed, a fix to prototype glue; no new bash tool.
- Changes the existing bash tool `driver/examples/sdc/tools/sdc-tool.sh`: allowed, a fix to prototype glue; no new bash tool.
(AGENTS.md, "Rules": anything new that runs is Rust; existing bash tools get fixes only.)
