---
format: aep.planning-md/3
id: story:finding-loop-leaves-node-program
kind: story
status: proposed
title: Whether an SDC node is called and what happens to its findings is decided by programs its bindings name
summary: Steps of sdc-step at before, ready and stored do the three skips, the findings input and the promotion of findings; findings.rs no longer runs for the eight nodes.
tags:
- generic-runner
relations:
- decomposes: epic:runner-knows-no-runbook
- depends_on: story:runner-source-names-no-runbook-ratchet
- depends_on: story:binding-states-model-node-inputs-and-output
- depends_on: story:node-program-runs-steps-a-binding-names
- depends_on: story:node-reply-stored-by-bound-programs
- depends_on: story:node-inputs-prepared-by-bound-programs
- depends_on: story:review-reply-leaves-node-program
- depends_on: story:runner-finds-model-node-by-binding
scope:
- confidence: cited
  path: bindings/sdc-direction.yaml
- confidence: cited
  path: bindings/sdc-intake.yaml
- confidence: cited
  path: bindings/sdc-plan.yaml
- confidence: cited
  path: bindings/sdc-spec.yaml
- confidence: cited
  path: bindings/sdc-story.yaml
- confidence: cited
  path: driver/examples/sdc/sdc-direction.bindings.json
- confidence: cited
  path: driver/examples/sdc/sdc-intake.bindings.json
- confidence: cited
  path: driver/examples/sdc/sdc-plan.bindings.json
- confidence: cited
  path: driver/examples/sdc/sdc-spec.bindings.json
- confidence: cited
  path: driver/examples/sdc/sdc-story.bindings.json
- confidence: cited
  path: driver/src/bin/sdc-node/implement.rs
- confidence: cited
  path: driver/src/bin/sdc-node/rework.rs
- confidence: inferred
  path: driver/src/bin/sdc-step/findings.rs
- confidence: cited
  path: driver/src/reply/mod.rs
- confidence: inferred
  path: driver/src/reply/rejection.rs
- confidence: inferred
  path: driver/tests/steps/findings.rs
- confidence: inferred
  path: runbook/tests/source_names/known/driver/src/bin/sdc-node/implement.rs.txt
revision: 11
transitions:
- {from: "draft", to: "proposed", at: "2026-11-23T06:01:20Z", actor: "human:operator", revision: 11, executor: "agent:runbook", correlation: "epic-runner-knows-no-runbook-critics-3"}
---
## Outcome

Whether a node of SDC is called at all, and what happens to the findings it raised, is decided by a program of SDC that the bindings of the SDC actions name at three points of `story:node-program-runs-steps-a-binding-names`. At `before`, for every adapter: a node is not called while a finding waits for a person, and `questions.write` is not called while an interview exists. At `ready`, only when a model is to be called: a node whose derived tree is current is not called, and the findings a node is given are an input of its prompt. At `stored`: the findings a node raised go through the program check into `findings.yaml`, and a derived tree records what it was derived from. The bindings of the eight actions mark the concern `findings` as moved, so the list of nodes that raise findings and the names of the trees in `findings.rs` are no longer read; `story:node-program-built-ins-removed` deletes them.

## Source

- `epic:runner-knows-no-runbook`, item 2 ("the promotion of findings ... becomes bound programs of SDC actions").
- Read at `main` `826b09c7` (the cited lines hold at `3c0eb863`, release 0.7.0: of the paths read, only `runbook/src/commands.rs` and tests of the daemon differ):
  - `driver/src/bin/sdc-node/findings.rs` (163 lines): `raises()` is a list of seven actions (`:14-20`); `derived()` maps three actions to the trees `spec`, `impl` and `aep` (`:23-30`); `wait_for_person()` reads `findings.yaml` and ends the node with exit 0 and the text of `instructions/shared/node.not-called.findings.md` (`:34-54`); `post()` runs the helper `sdc-findings` and writes `<tree>/.derived-from` (`:60-120`); `derived_skip()` (`:127`).
  - `driver/src/bin/sdc-node.rs:176-182`: `questions.write` is not called while `questions.yaml` exists and no feedback is given.
  - `driver/src/bin/sdc-node/call.rs:177-182`: the findings a node is given as an input.
  - The two places before the call differ: the skip of `questions.write` and `wait_for_person` run before the stub branch (`driver/src/bin/sdc-node.rs:176-182,213`), `derived_skip` runs after it has returned for the stub adapter (`:239-241`) and returns early when the node raises nothing, when a retry feedback is given, when a rejection was found or when the marker or `findings.yaml` is missing (`findings.rs:127-131`).
  - `Ctx::post` is called from `call.rs:340-342` with `out/findings.yaml`, from the stub branch (`sdc-node.rs:236`) and from the implementer (`driver/src/bin/sdc-node/implement.rs:212,237`).
  - A rejection is found by `find_rejection` (`driver/src/bin/sdc-node/rework.rs:30`, format `sdc.rejection/1`), a function of the binary.

## Domain relations

- Statement of a model node -> program that decides whether the node is called: zero or one; -> program that runs after the store: zero or one. inferable (inferred from `driver/src/bin/sdc-node/findings.rs:14-30` and `driver/src/bin/sdc-node.rs:176-182`, lists of action ids in code; no `ess/1` document declares it).

## Scope

- `driver/src/bin/sdc-step/findings.rs` (the subcommands `sdc-step findings before|ready|stored <action>`) and `driver/tests/steps/findings.rs`; both files exist empty.
- `driver/src/bin/sdc-node/implement.rs`: the two calls of `Ctx::post` (`:212`, `:237`) reach the point `stored` in both modes of the implementer; where one does not after the mechanism story, this story changes it, so that the tree is green with this story alone.
- `driver/src/bin/sdc-node/rework.rs` and new `driver/src/reply/rejection.rs` (with its line in `driver/src/reply/mod.rs`): `find_rejection` moves into the library of SDC, so that the step and, until `story:implement-reply-leaves-node-program`, the node program call one function.
- The bindings of the seven actions of `raises()` and of `questions.write`: `bindings/sdc-intake.yaml`, `bindings/sdc-direction.yaml`, `bindings/sdc-spec.yaml`, `bindings/sdc-plan.yaml`, `bindings/sdc-story.yaml` and their JSON copies.
- The shard of `implement.rs`, which this story leaves as it is. No file under `docs/`, not `driver/tests/sdc_node.rs`. Of `driver/src/bin/sdc-step.rs`, `driver/src/bin/sdc-step/` and `driver/tests/steps/` this story edits only its own module, `driver/src/bin/sdc-step/findings.rs` and `driver/tests/steps/findings.rs`.
- Overlap, by story:
  - `story:node-program-runs-steps-a-binding-names`: edge.
  - `story:node-reply-stored-by-bound-programs`: edge; the point `stored` of this story runs after the store step of that one, and both edit argv lines of `bindings/sdc-intake.yaml`, `bindings/sdc-direction.yaml` and `bindings/sdc-plan.yaml`.
  - `story:node-inputs-prepared-by-bound-programs`: edge; both edit the argv lines of `spec.author` in `bindings/sdc-spec.yaml` and of `plan.decompose` in `bindings/sdc-plan.yaml`.
  - `story:review-reply-leaves-node-program`: edge; both edit `bindings/sdc-story.yaml` (this story the line of `change.implement`, that one the line of `change.review`; one file, so they are ordered).
  - `story:runner-finds-model-node-by-binding`: edge; it marks argv lines of `bindings/sdc-spec.yaml`, `bindings/sdc-plan.yaml` and `bindings/sdc-story.yaml`.
  - `story:implement-reply-leaves-node-program` (`implement.rs`, `rework.rs`, `bindings/sdc-story.yaml`) and `story:node-program-built-ins-removed`: each has an edge to this story.
  - `story:one-copy-of-the-bindings` (draft, no typed scope): not in one wave with this story.
  - `story:bindings-state-their-exits` and `story:producer-id-llm-node` (drafts, no typed scope; the five bindings files of this scope and, for the second, their JSON copies): each not built in the same wave as this story.

## Acceptance

- The test `the_eight_bindings_name_their_findings_steps` (`driver/tests/steps/findings.rs`) reads the bindings: `questions.write` names a `before` step; each of the seven actions of `raises()` names a `before`, a `ready` and a `stored` step; all eight have `findings` in `--moved`.
- A node that is not called makes no model call and no adapter starts; the record of the run states 0 calls for it. One test for each reason: `a_finding_that_waits_for_a_person_stops_the_node` passes unchanged and `questions_write_is_not_called_while_an_interview_exists` (`driver/tests/steps/findings.rs`) passes, both with the stub adapter (point `before`); `a_node_whose_derived_tree_is_current_is_not_called` runs with an sh adapter written for the test, not the stub, and reads its empty call log (point `ready`); `the_derived_skip_does_not_run_for_the_stub_adapter` holds that the stub cycles are as they were.
- The step gets what the skip of a derived tree reads: the retry feedback is the member of its input; a rejection it finds itself with `find_rejection` of `driver/src/reply/rejection.rs`. The tests `a_retry_feedback_calls_a_node_whose_derived_tree_is_current` and `a_rejection_calls_a_node_whose_derived_tree_is_current` hold both, and `a_derived_tree_is_renewed_when_every_decision_is_a_default_the_node_raised` and `a_decision_of_a_person_calls_the_node` (`driver/tests/sdc_node.rs:637,644`) pass unchanged.
- The findings a node is given are a contribution of the `ready` step (`call.rs:177-182` no longer runs for a moved node): the golden prompt test below passes.
- The point `stored` is reached from every place that called `Ctx::post`: the test `the_findings_step_runs_after_the_store_in_every_mode` starts `change.implement` on the stub, in the session mode and in the request mode, and `spec.author` with the stub and with an sh adapter (the action reaches the point through `sdc-node.rs:236` and through `call.rs:340-342`), and reads the marker of the step in each of the five runs.
- The first step of the implementor, before any other change of this story: show with the two `spec.author` runs of `the_findings_step_runs_after_the_store_in_every_mode` (a marker step at `stored`, the stub and an sh adapter) that the action reaches `Ctx::post` through `sdc-node.rs:236` on the stub and through `call.rs:340-342` with an adapter. The decomposer read this in the source and did not run it. If it does not hold, stop and report.
- An approval or a close is never recorded under the name of a person by a rule or a model: after the `stored` step ran, no finding has a `decision` it did not have before, and a finding with `status: needs_person` stays without one (the test `promoting_findings_decides_nothing` compares `findings.yaml` before and after).
- `git grep -n "env::var\|env_set" -- driver/src/bin/sdc-step/findings.rs` prints only lines that read a `LOOM_*` name.
- The steps run under the contract of `story:node-program-runs-steps-a-binding-names` and change nothing of it: `git diff --stat <base> -- docs/loom-bindings.md driver/src/bin/sdc-node.rs driver/src/bin/sdc-node/steps.rs driver/src/node driver/tests/steps/mechanism.rs` prints nothing. The dispatcher, the signature of a module, the library of a step and the helpers of the step tests are final since that story: `git diff --stat <base> -- driver/src/bin/sdc-step.rs driver/src/bin/sdc-step driver/src/node/step.rs driver/tests/steps` lists `driver/src/bin/sdc-step/findings.rs` and `driver/tests/steps/findings.rs` and no other file.
- The prompts of the 14 model nodes are byte-identical before and after: the test `the_prompt_of_each_agent_node_is_its_golden_byte_for_byte` (`driver/tests/bugfix_prompt.rs:214`) passes, `git diff --stat <base> -- instructions/golden/` prints nothing (25 files under `instructions/golden/llm-node/`), and no step of the story runs with `SDC_BLESS_PROMPTS=1`. Where the test has to start a node in a new way, only its harness changes, never a golden file.
- The cases of `driver/tests/sdc_node.rs` keep their committed expectations (`expect()`, `:319-329`, 146 files under `driver/tests/expected/`): `git diff --stat <base> -- driver/tests/expected/` prints nothing, and no step of the story runs with `SDC_BLESS_EXPECTED=1`.
- The cycles keep their results: `task test:cycles-loom` (slow lane, not part of `task check`; started in the background and without the `slot` prefix, as its description says; its task names its time with `SLOT_TIMEOUT=30m`) ends with exit 0 and reports each of the 54 cycles of `cycles/tree-suite.yaml` with the result that file commits for it. `git diff --stat <base> -- cycles/` prints nothing: no expected result is edited to get there.
- `task check` is green at the head of the story.
- The last commit of the unit, made after `main` is merged into it, adds one entry under `[Unreleased]` of `CHANGELOG.md`. No other commit of the story changes that file, and the file is not in the typed scope of the story, so two stories of one wave do not meet on it.
- Tests: no test over 60 s and the test step within 120 s; no model call and no network in a test (stub adapter, sh programs); what is expensive (a built binary, a prepared run) is made once per test run, never per test. Each new test is timed before it is added; a test over 5 s is named with its time in the report. Nothing that stays is Python (Rust or POSIX sh).
- A program, variable, file or format id this story creates for a generic thing carries no `sdc` in its name (epic item 6; the names that exist follow stage 2 of `story:project-rename`).
- No shard of `story:runner-source-names-no-runbook-ratchet` changes: `git diff --stat <base> -- runbook/tests/source_names/` prints nothing.
- No measured claim of `docs/` or `experiments/` changes. Where a number would change (the time of a node, the count of processes it starts), the measurement is recorded under `experiments/` and listed in `experiments/INDEX.md` before the claim is edited.

## Not in this story

- Deleting `findings.rs`, the skip of `questions.write` in `sdc-node.rs` and the findings lines of `call.rs` (`:177-182`, `:340-342`): `story:node-program-built-ins-removed`. Until then the lines of their shards stay.
- The rework of the implementer after a rejected approval (`stub_rework`, the rework text): `story:implement-reply-leaves-node-program`.
- The helper `sdc-findings` itself and the format of `findings.yaml`.
