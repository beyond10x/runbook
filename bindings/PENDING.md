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

- `sdc.bindings.json:55` `epics.open` `{dir}` in `args_from` — D13 — G1, wave 2 — none: the argv already names `tools/sdc-tool.sh`, a path relative to `bindings/` (step 2, `story:loom-tools-program-contract`). Every file that binds a program names `program_root: bindings`, which makes Loom resolve the path beside the bindings file (`resolve_programs`; Loom at `af8de2c9`).
- `sdc-intake.bindings.json:11` `*` `{dir}` in every `argv` (9 bindings) — D13 — G1, wave 2 — none, as above.
- `sdc-direction.bindings.json:12` `*` `{dir}` in every `argv` (6 bindings) — D13 — G1, wave 2 — none, as above.
- `sdc-epic.bindings.json:13` `*` `{dir}` in `argv` and `args_from` (2 bindings) — D13 — G1, wave 2 — none, as above.
- `sdc-spec.bindings.json:20` `*` `{dir}` in every `argv` and `args_from` (8 bindings) — D13 — G1, wave 2 — none, as above.
- `sdc-plan.bindings.json:16` `*` `{dir}` in every `argv` (7 bindings) — D13 — G1, wave 2 — none, as above.
- `sdc-story.bindings.json:14` `*` `{dir}` in every `argv` (6 bindings) — D13 — G1, wave 2 — none, as above.
- `sdc-release.bindings.json:7` `*` `{dir}` in every `argv` (5 bindings) — D13 — G1, wave 2 — none, as above.
- `sdc-bugfix.bindings.json:10` `*` `{dir}` in every `argv` (8 bindings) — D13 — G1, wave 2 — none, as above.
- `sdc-intake.bindings.json:11` `*` program-error exits 75, 126, 127 (driver `program_error`, `driver/src/main.rs:579-581`) on all 9 program bindings — D36 — G1, wave 2 — the member is drafted in `story:bound-program-contract` (`exits: {error: [...]}`), not in Loom's loader; the list each binding states: `[75, 126, 127]`.
- `sdc-direction.bindings.json:12` `*` program-error exits on all 6 program bindings — D36 — G1, wave 2 — as above; list `[75, 126, 127]`.
- `sdc-epic.bindings.json:13` `spec.gate` program-error exits — D36 — G1, wave 2 — as above; list `[75, 126, 127]`.
- `sdc-spec.bindings.json:20` `*` program-error exits on all 8 program bindings — D36 — G1, wave 2 — as above; list `[75, 126, 127]`.
- `sdc-plan.bindings.json:16` `*` program-error exits on all 7 program bindings — D36 — G1, wave 2 — as above; list `[75, 126, 127]`.
- `sdc-story.bindings.json:14` `*` program-error exits on all 6 program bindings — D36 — G1, wave 2 — as above; list `[75, 126, 127]`.
- `sdc-release.bindings.json:7` `*` program-error exits on all 5 program bindings — D36 — G1, wave 2 — as above; list `[75, 126, 127]`.
- `sdc-bugfix.bindings.json:10` `*` program-error exits on all 8 program bindings — D36 — G1, wave 2 — as above; list `[75, 126, 127]`.
- `sdc-story.bindings.json:75` `tests.run` exit 69, the tool's "no build slot within `--pool-wait`, nothing ran" (`sdc-tool.sh`, `pool_run`) — `story:pool-wait-is-not-a-failure` — G1, wave 2 — the driver takes 69 as a program error with up to 3 runs of the step and then the verdict `pool_wait_spent`; under Loom the YAML's `results` `"*": failed` still maps 69 to `failed` until the loader has `exits: {error: [...]}`: list `[69, 70, 75, 126, 127]` on this binding (70: the pool ended a started job at its run limit), which then ends the run `ProgramError` with nothing recorded (Loom `2c0a6f48`, `crates/loom-bindings/src/program.rs:21`), and Loom does not run the step again: an open item of the story.
- `sdc-release.bindings.json:31` `acceptance.run` exit 69 — `story:pool-wait-is-not-a-failure` — G1, wave 2 — as above; list `[69, 70, 75, 126, 127]`.
- `sdc-bugfix.bindings.json:68` `repro.run` exits 69 and 70 — `story:pool-wait-is-not-a-failure` — G1, wave 2 — as the `tests.run` line of `sdc-story.bindings.json`: the tool's "no build slot within `--pool-wait`, nothing ran" (69; `sdc-bugfix` `repro-run`, `tests-run` and `gate-run` bound the wait themselves) and "the pool ended a started job at its run limit" (70); list `[69, 70, 75, 126, 127]`.
- `sdc-bugfix.bindings.json:123` `tests.run` exits 69 and 70 — `story:pool-wait-is-not-a-failure` — G1, wave 2 — as above; list `[69, 70, 75, 126, 127]`.
- `sdc-bugfix.bindings.json:157` `gate.run` exits 69 and 70 — `story:pool-wait-is-not-a-failure` — G1, wave 2 — as above; list `[69, 70, 75, 126, 127]`.
- `sdc-intake.bindings.json:24` `idea.dissect` not called (`sdc-node` writes `SDC_NOT_CALLED`, exit 0) — D37 — G1, wave 2 — drafted as `exits: {not_called: [...]}` in `story:bound-program-contract`. Under Loom (`LOOM_CASE` set) `sdc-node` prints the reason and exits 0 as in the driver; it takes a not-called exit code once the loader has the member and the binding names one.
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
- `sdc-bugfix.bindings.json:89` `change.fix` not called — D37 — G1, wave 2 — as above.

## Agent nodes (Loom `story:bound-agent-node`, wave 4; stub adapter `N3`, wave 5)

Until then every agent node is a `program` running `sdc-node`, and a `program` executor
takes no agent member (`MemberNotForExecutor`). Since step 2 the binding's `llm` object is argv of
that program: `--model`, `--max-turns`, `--max-budget-usd`, `--operations` and `--stub <argv>`
(`docs/loom-bindings.md`, "Tool changes"). The text replaces the binding's `executor` and `budget`
and drops that argv; `inputs` and `outputs` follow the form `story:bound-agent-node` fixes. The
`llm.stub` argv (D21) has no member in any Loom story; the stub overlay takes it with `N3`'s
fixture adapter.

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
- `sdc-story.bindings.json:98` `change.review` `llm.operations` — D22 — bound-agent-node, wave 4 — `tools: {allow: [dir.list, file.read, search]}` (the names as `sdc-node` gives them; the adapter's own tool names are fixed with the adapter).
- `sdc-story.bindings.json:103` `change.review` `llm.stub` `[review-change]` — D21 — N3, wave 5 — syntax not fixed.
- `sdc-bugfix.bindings.json:14` `bug.dissect` `llm` — D18, D19, D20 — bound-agent-node, wave 4 — `executor: {kind: agent, adapter: claude-code, model: claude-haiku-5-5}`, `budget: {max_runs: 3, max_turns: 20, max_budget_micro_usd: 1000000}`.
- `sdc-bugfix.bindings.json:18` `bug.dissect` `llm.stub` `[stub-file, bugfix-pager/stub-dissection.json, bug-dissection.json]` — D21 — N3, wave 5 — syntax not fixed.
- `sdc-bugfix.bindings.json:51` `test.reproduce` `llm` — D18, D19, D20 — bound-agent-node, wave 4 — `executor: {kind: agent, adapter: claude-code, model: claude-haiku-5-5}`, `budget: {max_runs: 3, max_turns: 30, max_budget_micro_usd: 1000000}`.
- `sdc-bugfix.bindings.json:55` `test.reproduce` `llm.stub` `[stub-repro, bugfix-pager/stub-repro]` — D21 — N3, wave 5 — syntax not fixed.
- `sdc-bugfix.bindings.json:102` `change.fix` `llm` — D18, D19, D20 — bound-agent-node, wave 4 — `executor: {kind: agent, adapter: claude-code, model: claude-haiku-5-5}`, `budget: {max_runs: 3, max_turns: 40, max_budget_micro_usd: 2000000}`.
- `sdc-bugfix.bindings.json:106` `change.fix` `llm.stub` `[stub-tree, bugfix-pager/stub-fix, impl]` — D21 — N3, wave 5 — syntax not fixed.
- `sdc-intake.bindings.json:28` `*` `llm.instructions`, the files of the instruction text of the 14 agent actions (`instructions/<action>.md` and its parts), named by every binding with an `llm` object — none — no Loom story states the member — syntax not fixed; the YAML files name none, and `sdc-node` takes the paths from the JSON bindings until the agent executor has a member for them.

## Tool environment under Loom (step 2)

What the tools read from the driver's environment and Loom's program contract does not give them
(`docs/loom-bindings.md`, "Tool changes"). Under Loom (`LOOM_CASE` set) each takes the default
named here; the driver's path is unchanged. Line form: the tool and the variable, the Loom story.

- `sdc-tool.sh`, `sdc-node` `SDC_PRINCIPAL` — `story:bound-program-declares-its-environment` (wave 9), or a principal argument from G2 — the tools take `--principal <principal>` already; no binding states one, so the `aep` actor is `agent:<action>`.
- `sdc-tool.sh`, `sdc-node` `SDC_STUB_FIXTURES`, `SDC_STUB_COUNTER`; `sdc-node` `SDC_STUB_FINDINGS`; `sdc-findings` `SDC_STUB_REFUSE_DECISION` — N3, wave 5 — the tools take `--stub-fixtures` and `--stub-counter`; the stub overlay that states them is N3's.
- `sdc-node` `SDC_LLM_ADAPTER` — N3 (the stub overlay) and N9 (`story:engine-configuration-document`, the engine file) — `--adapter` is taken; the YAML names none, so the node runs its stub.
- `sdc-node` `SDC_LLM_NODES`, `SDC_RUN_SETTINGS`, `SDC_LLM_MODEL[_<CLASS>]`, `SDC_LLM_EFFORT[_<CLASS>]`, `SDC_REVIEW_RETRIES`, `SDC_COVERAGE_RETRIES`, `SDC_IMPLEMENT_MODE`, `SDC_REVIEW_MODE`, `SDC_IMPLEMENT_COVERAGE`; `sdc-reply` `SDC_<CLASS>_ADAPTER` — N9 (`story:engine-configuration-document`) — run settings, not binding text; the defaults apply.
- `sdc-node` `SDC_LLM_RECORD`, `SDC_LLM_LOG` — `story:agent-node-run-records` — the call record stays in `work/.sdc/llm/` of the workspace.
- `sdc-node` `SDC_CASE_DIR` (`events.jsonl`: a rejection no longer current; `change.implement` request mode) — `story:bound-agent-node` (the node's inputs) — no variable names the case store (`LOOM_RETRY_FEEDBACK` holds only the records about the subject's current revision), so the fallback over `events.jsonl` does not run, and `sdc-reply implement-run` reads the directory above the workspace. `change.review`'s inputs no longer wait: under Loom `sdc-reply` reads the tests log, the gate log and the earlier rejections from record files of the workspace (`.sdc/records/`, `docs/loom-bindings.md`), and the fixture adapter, which gets no `case_dir` in the request, names `<workspace>/..`.
- `sdc-tool.sh`, `sdc-node` `HOME` of `cargo`, `slot` and `aep` that they start — only for a host whose user has no passwd entry (they answered `--version` under `PATH` alone on the workstation, 2026-11-22; `docs/loom-bindings.md`, "Variables beyond `PATH` read under `set -u`") — `story:bound-program-declares-its-environment` (wave 9), or `--confinement local`.
- `sdc-node` `SDC_CLAUDE_CREDENTIALS` (the `claude-cli` adapter's login; `HOME` is unset under Loom) — a binding that names `--adapter claude-cli` also passes `--claude-credentials <file>` — N3, N9 — the tool takes the argv entry; without any login the adapter stops naming the variables.
- `LOOM_CASE` and `LOOM_RETRY_FEEDBACK` themselves — `story:bound-program-case-id` and `story:bound-program-retry-records` (both active, not merged into Loom's integration head) — the tools read both already. `LOOM_CASE_NAME` (the name the opening gave the case) and `LOOM_PARENT_CASE_NAME` (the parent's) are final names: epic case `LOOM_CASE_NAME=<epic>`; story case and the spec, plan and release children of an epic `LOOM_CASE_NAME=<own name>` and `LOOM_PARENT_CASE_NAME=<epic>`; the root case neither; a repeated case (`<id>.2`) takes the names of the first; set for an `arguments_from` program too. The tools read them in Loom mode for the epic and the story and fall back to the cut of `LOOM_CASE` when neither is set (Loom binaries before its wave 2); until Loom sets them a repeated case (`IDEA-1.billing.2.story-a`) gets the wrong story.

## Person actions (Loom G2, wave 3)

A `person` executor records the results its `results` table names (`from: effect`, Loom `5af3f9c2`); with an `argv` (Loom `70b0e6e0`, `story:person-action-runs-a-program`) it also runs that program when the approval is taken, with the approving principal in a whole argv entry `{principal}`. `scope.answer` is bound that way in `bindings/sdc-intake.yaml`; the JSON file keeps `manual: true` for the old driver. `context.approve`, `vision.approve`, `roadmap.approve` and `merge.approve` declare `rejected` beside `performed`. Nothing of this section is pending.


## Handoffs (Loom `N8`, wave 4)

The YAML `handoff` text says "in findings.yaml of the workspace" where the JSON says
`in {work}/findings.yaml`.

- `sdc-intake.bindings.json:184` `amend.upstream` `{work}` in `handoff` — D14 — N8, wave 4 — syntax not fixed; restore `in {work}/findings.yaml` once `{work}` is a placeholder.
- `sdc-direction.bindings.json:162` `amend.upstream` `{work}` in `handoff` — D14 — N8, wave 4 — as above.
- `sdc-spec.bindings.json:154` `amend.upstream` `{work}` in `handoff` — D14 — N8, wave 4 — as above.
- `sdc-plan.bindings.json:125` `amend.upstream` `{work}` in `handoff` — D14 — N8, wave 4 — as above.
- `sdc-story.bindings.json:143` `amend.upstream` `{work}` in `handoff` — D14 — N8, wave 4 — as above.

## Child cases

- `sdc.bindings.json:50` `epics.open` `child.revisions` `{"epic": "{name}"}` — D30 — G4, wave 10, only if `sdc.epic` cannot reach `passed` with `epic` at `none` (step 3 decides) — syntax not fixed.
