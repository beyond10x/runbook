# What `bindings/*.yaml` leaves out, and what brings it

The `bindings/*.yaml` files are `loom.bindings/1` as Loom's loader reads it at Loom commit
`8a8fad3b`. This list names every construct of `driver/examples/sdc/*.bindings.json` the format
cannot express yet. Nothing here is written in the YAML files. Delete this file when the list is
empty.

Line form: `` `<JSON file>:<line>` `<action>` `` construct, the difference number in Loom's
comparison of the two formats (`D<n>`), the Loom story and wave that bring it, and the text to add
once it exists. "Syntax not fixed" means no Loom story states the member yet. The waves are
Runbook's proposed order of Loom's open stories (`docs/loom-bindings.md`, "Pending").
`scripts/check-bindings.sh` reads the first two fields: an action bound in a JSON file and missing
from its YAML file must have a line here.

## Program contract (Loom G1, wave 2)

- `sdc.bindings.json:55` `epics.open` `{dir}` in `args_from` — D13 — G1, wave 2 — none: the argv already reads `../driver/examples/sdc/tools/sdc-tool.sh`, relative to `bindings/`, which G1 resolves.
- `sdc-intake.bindings.json:11` `*` `{dir}` in every `argv` (9 bindings) — D13 — G1, wave 2 — none, as above.
- `sdc-direction.bindings.json:12` `*` `{dir}` in every `argv` (6 bindings) — D13 — G1, wave 2 — none, as above.
- `sdc-epic.bindings.json:13` `*` `{dir}` in `argv` and `args_from` (2 bindings) — D13 — G1, wave 2 — none, as above.
- `sdc-spec.bindings.json:20` `*` `{dir}` in every `argv` and `args_from` (8 bindings) — D13 — G1, wave 2 — none, as above.
- `sdc-plan.bindings.json:16` `*` `{dir}` in every `argv` (7 bindings) — D13 — G1, wave 2 — none, as above.
- `sdc-story.bindings.json:14` `*` `{dir}` in every `argv` (6 bindings) — D13 — G1, wave 2 — none, as above.
- `sdc-release.bindings.json:7` `*` `{dir}` in every `argv` (5 bindings) — D13 — G1, wave 2 — none, as above.
- `sdc-bugfix.bindings.json:10` `*` `{dir}` in every `argv` (8 bindings) — D13 — G1, wave 2 — none, as above.
- `sdc-intake.bindings.json:11` `*` program-error exits 75, 126, 127 (driver `program_error`, `driver/src/main.rs:579-581`) on all 9 program bindings — D36 — G1, wave 2 — syntax not fixed; the list each binding states: `[75, 126, 127]`.
- `sdc-direction.bindings.json:12` `*` program-error exits on all 6 program bindings — D36 — G1, wave 2 — syntax not fixed; list `[75, 126, 127]`.
- `sdc-epic.bindings.json:13` `spec.gate` program-error exits — D36 — G1, wave 2 — syntax not fixed; list `[75, 126, 127]`.
- `sdc-spec.bindings.json:20` `*` program-error exits on all 8 program bindings — D36 — G1, wave 2 — syntax not fixed; list `[75, 126, 127]`.
- `sdc-plan.bindings.json:16` `*` program-error exits on all 7 program bindings — D36 — G1, wave 2 — syntax not fixed; list `[75, 126, 127]`.
- `sdc-story.bindings.json:14` `*` program-error exits on all 6 program bindings — D36 — G1, wave 2 — syntax not fixed; list `[75, 126, 127]`.
- `sdc-release.bindings.json:7` `*` program-error exits on all 5 program bindings — D36 — G1, wave 2 — syntax not fixed; list `[75, 126, 127]`.
- `sdc-bugfix.bindings.json:10` `*` program-error exits on all 8 program bindings — D36 — G1, wave 2 — syntax not fixed; list `[75, 126, 127]`.
- `sdc-intake.bindings.json:24` `idea.dissect` not called (`llm-node.sh` writes `SDC_NOT_CALLED`, exit 0) — D37 — G1, wave 2 — syntax not fixed; the not-called list is the exit `llm-node.sh` gets in step 2 in place of the file.
- `sdc-intake.bindings.json:63` `questions.write` not called — D37 — G1, wave 2 — as above.
- `sdc-intake.bindings.json:86` `questions.answer` not called — D37 — G1, wave 2 — as above.
- `sdc-intake.bindings.json:137` `requirements.normalise` not called — D37 — G1, wave 2 — as above.
- `sdc-direction.bindings.json:12` `context.default` not called — D37 — G1, wave 2 — as above.
- `sdc-direction.bindings.json:68` `vision.write` not called — D37 — G1, wave 2 — as above.
- `sdc-direction.bindings.json:102` `roadmap.write` not called — D37 — G1, wave 2 — as above.
- `sdc-spec.bindings.json:20` `spec.author` not called — D37 — G1, wave 2 — as above.
- `sdc-plan.bindings.json:29` `plan.decompose` not called — D37 — G1, wave 2 — as above.
- `sdc-story.bindings.json:27` `change.implement` not called — D37 — G1, wave 2 — as above.
- `sdc-story.bindings.json:83` `change.review` not called — D37 — G1, wave 2 — as above.
- `sdc-bugfix.bindings.json:10` `bug.dissect` not called — D37 — G1, wave 2 — as above.
- `sdc-bugfix.bindings.json:47` `test.reproduce` not called — D37 — G1, wave 2 — as above.
- `sdc-bugfix.bindings.json:86` `change.fix` not called — D37 — G1, wave 2 — as above.

## Agent nodes (Loom `story:bound-agent-node`, wave 4; stub adapter `N3`, wave 5)

Until then every agent node is a `program` running `llm-node.sh`, and a `program` executor takes
no agent member (`MemberNotForExecutor`). The text replaces the binding's `executor` and `budget`;
`inputs` and `outputs` follow the form `story:bound-agent-node` fixes. The `llm.stub` argv (D21)
has no member in any Loom story; the stub overlay takes it with `N3`'s fixture adapter.

- `sdc-intake.bindings.json:28` `idea.dissect` `llm` — D18, D19, D20 — bound-agent-node, wave 4 — `executor: {kind: agent, adapter: claude-code, model: claude-haiku-5-5}`, `budget: {max_runs: 3, max_turns: 15, max_budget_micro_usd: 500000}`.
- `sdc-intake.bindings.json:32` `idea.dissect` `llm.stub` `[stub-file, dissection.yaml, dissection.yaml]` — D21 — N3, wave 5 — syntax not fixed.
- `sdc-intake.bindings.json:67` `questions.write` `llm` — D18, D19, D20 — bound-agent-node, wave 4 — `executor: {kind: agent, adapter: claude-code, model: claude-haiku-5-5}`, `budget: {max_runs: 3, max_turns: 15, max_budget_micro_usd: 500000}`.
- `sdc-intake.bindings.json:71` `questions.write` `llm.stub` `[stub-file, questions-open.yaml, questions.yaml]` — D21 — N3, wave 5 — syntax not fixed.
- `sdc-intake.bindings.json:90` `questions.answer` `llm` — D18, D19, D20 — bound-agent-node, wave 4 — `executor: {kind: agent, adapter: claude-code, model: claude-haiku-5-5}`, `budget: {max_runs: 3, max_turns: 15, max_budget_micro_usd: 500000}`.
- `sdc-intake.bindings.json:94` `questions.answer` `llm.stub` `[stub-file, questions-answered.yaml, questions.yaml]` — D21 — N3, wave 5 — syntax not fixed.
- `sdc-intake.bindings.json:141` `requirements.normalise` `llm` — D18, D19, D20 — bound-agent-node, wave 4 — `executor: {kind: agent, adapter: claude-code, model: claude-haiku-5-5}`, `budget: {max_runs: 3, max_turns: 15, max_budget_micro_usd: 500000}`.
- `sdc-intake.bindings.json:145` `requirements.normalise` `llm.stub` `[stub-file, requirements.yaml, requirements.yaml]` — D21 — N3, wave 5 — syntax not fixed.
- `sdc-direction.bindings.json:16` `context.default` `llm` — D18, D19, D20 — bound-agent-node, wave 4 — `executor: {kind: agent, adapter: claude-code, model: claude-haiku-5-5}`, `budget: {max_runs: 3, max_turns: 15, max_budget_micro_usd: 500000}`.
- `sdc-direction.bindings.json:20` `context.default` `llm.stub` `[stub-file, context.yaml, context.yaml]` — D21 — N3, wave 5 — syntax not fixed.
- `sdc-direction.bindings.json:72` `vision.write` `llm` — D18, D19, D20 — bound-agent-node, wave 4 — `executor: {kind: agent, adapter: claude-code, model: claude-haiku-5-5}`, `budget: {max_runs: 3, max_turns: 15, max_budget_micro_usd: 500000}`.
- `sdc-direction.bindings.json:76` `vision.write` `llm.stub` `[stub-vision]` — D21 — N3, wave 5 — syntax not fixed.
- `sdc-direction.bindings.json:106` `roadmap.write` `llm` — D18, D19, D20 — bound-agent-node, wave 4 — `executor: {kind: agent, adapter: claude-code, model: claude-haiku-5-5}`, `budget: {max_runs: 3, max_turns: 15, max_budget_micro_usd: 500000}`.
- `sdc-direction.bindings.json:110` `roadmap.write` `llm.stub` `[stub-roadmap]` — D21 — N3, wave 5 — syntax not fixed.
- `sdc-spec.bindings.json:34` `spec.author` `llm` — D18, D19, D20 — bound-agent-node, wave 4 — `executor: {kind: agent, adapter: claude-code, model: claude-haiku-5-5}`, `budget: {max_runs: 4, max_turns: 40, max_budget_micro_usd: 2000000}`.
- `sdc-spec.bindings.json:38` `spec.author` `llm.stub` `[stub-tree, billing-spec, spec]` — D21 — N3, wave 5 — syntax not fixed.
- `sdc-plan.bindings.json:33` `plan.decompose` `llm` — D18, D19, D20 — bound-agent-node, wave 4 — `executor: {kind: agent, adapter: claude-code, model: claude-haiku-5-5}`, `budget: {max_runs: 3, max_turns: 15, max_budget_micro_usd: 500000}`.
- `sdc-plan.bindings.json:37` `plan.decompose` `llm.stub` `[stub-decompose]` — D21 — N3, wave 5 — syntax not fixed.
- `sdc-story.bindings.json:40` `change.implement` `llm` — D18, D19, D20 — bound-agent-node, wave 4 — `executor: {kind: agent, adapter: claude-code, model: claude-haiku-5-5}`, `budget: {max_runs: 3, max_turns: 40, max_budget_micro_usd: 2000000}`.
- `sdc-story.bindings.json:44` `change.implement` `llm.stub` `[stub-tree, impl, impl]` — D21 — N3, wave 5 — syntax not fixed.
- `sdc-story.bindings.json:94` `change.review` `llm` — D18, D19, D20 — bound-agent-node, wave 4 — `executor: {kind: agent, adapter: claude-code, model: claude-haiku-5-5}`, `budget: {max_runs: 3, max_turns: 30, max_budget_micro_usd: 1000000}`.
- `sdc-story.bindings.json:98` `change.review` `llm.operations` — D22 — bound-agent-node, wave 4 — `tools: {allow: [dir.list, file.read, search]}` (the names as `llm-node.sh` gives them; the adapter's own tool names are fixed with the adapter).
- `sdc-story.bindings.json:103` `change.review` `llm.stub` `[review-change]` — D21 — N3, wave 5 — syntax not fixed.
- `sdc-bugfix.bindings.json:14` `bug.dissect` `llm` — D18, D19, D20 — bound-agent-node, wave 4 — `executor: {kind: agent, adapter: claude-code, model: claude-haiku-5-5}`, `budget: {max_runs: 3, max_turns: 20, max_budget_micro_usd: 1000000}`.
- `sdc-bugfix.bindings.json:18` `bug.dissect` `llm.stub` `[stub-file, bugfix-pager/stub-dissection.json, bug-dissection.json]` — D21 — N3, wave 5 — syntax not fixed.
- `sdc-bugfix.bindings.json:51` `test.reproduce` `llm` — D18, D19, D20 — bound-agent-node, wave 4 — `executor: {kind: agent, adapter: claude-code, model: claude-haiku-5-5}`, `budget: {max_runs: 3, max_turns: 30, max_budget_micro_usd: 1000000}`.
- `sdc-bugfix.bindings.json:55` `test.reproduce` `llm.stub` `[stub-repro, bugfix-pager/stub-repro]` — D21 — N3, wave 5 — syntax not fixed.
- `sdc-bugfix.bindings.json:99` `change.fix` `llm` — D18, D19, D20 — bound-agent-node, wave 4 — `executor: {kind: agent, adapter: claude-code, model: claude-haiku-5-5}`, `budget: {max_runs: 3, max_turns: 40, max_budget_micro_usd: 2000000}`.
- `sdc-bugfix.bindings.json:103` `change.fix` `llm.stub` `[stub-tree, bugfix-pager/stub-fix, impl]` — D21 — N3, wave 5 — syntax not fixed.

## Person actions (Loom G2, wave 3)

A `person` executor runs no program and records only `performed` (`from: effect`).

- `sdc-intake.bindings.json:192` `scope.answer` a person's action runs a program (`sdc-tool.sh scope-answer`) — D11 — G2, wave 3 — syntax not fixed; the argv: `[../driver/examples/sdc/tools/sdc-tool.sh, scope-answer, <principal>]`.
- `sdc-intake.bindings.json:195` `scope.answer` `{principal}` in `argv` — D15 — G2, wave 3 — syntax not fixed.
- `sdc-intake.bindings.json:198` `scope.answer` `subject` with no evidence — D38 — G2, wave 3 — syntax not fixed; subject `questions`.
- `sdc-direction.bindings.json:64` `context.approve` result `rejected` (exit 6, `approve --result rejected --reasons`) — D12 — G2, wave 3 — syntax not fixed; the YAML records `performed: approved` only.
- `sdc-direction.bindings.json:98` `vision.approve` result `rejected` — D12 — G2, wave 3 — as above.
- `sdc-direction.bindings.json:151` `roadmap.approve` result `rejected` — D12 — G2, wave 3 — as above.
- `sdc-story.bindings.json:120` `merge.approve` result `rejected` — D12 — G2, wave 3 — as above.

## Handoffs (Loom `N8`, wave 4; `G3`, wave 8)

The YAML `handoff` text says "in findings.yaml of the workspace" where the JSON says
`in {work}/findings.yaml`.

- `sdc-intake.bindings.json:184` `amend.upstream` `{work}` in `handoff` — D14 — N8, wave 4 — syntax not fixed; restore `in {work}/findings.yaml` once `{work}` is a placeholder.
- `sdc-intake.bindings.json:185` `amend.upstream` `handoff_when` — D17 — G3, wave 8 — syntax not fixed; the argv: `[../driver/examples/sdc/tools/sdc-tool.sh, findings-wait]`.
- `sdc-direction.bindings.json:162` `amend.upstream` `{work}` in `handoff` — D14 — N8, wave 4 — as above.
- `sdc-direction.bindings.json:163` `amend.upstream` `handoff_when` — D17 — G3, wave 8 — as above.
- `sdc-spec.bindings.json:154` `amend.upstream` `{work}` in `handoff` — D14 — N8, wave 4 — as above.
- `sdc-spec.bindings.json:155` `amend.upstream` `handoff_when` — D17 — G3, wave 8 — as above.
- `sdc-plan.bindings.json:125` `amend.upstream` `{work}` in `handoff` — D14 — N8, wave 4 — as above.
- `sdc-plan.bindings.json:126` `amend.upstream` `handoff_when` — D17 — G3, wave 8 — as above.
- `sdc-story.bindings.json:143` `amend.upstream` `{work}` in `handoff` — D14 — N8, wave 4 — as above.
- `sdc-story.bindings.json:144` `amend.upstream` `handoff_when` — D17 — G3, wave 8 — as above.

## Child cases

- `sdc.bindings.json:50` `epics.open` `child.revisions` `{"epic": "{name}"}` — D30 — G4, wave 10, only if `sdc.epic` cannot reach `passed` with `epic` at `none` (step 3 decides) — syntax not fixed.
