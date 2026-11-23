---
format: aep.planning-md/3
id: story:decisions-routed-by-runbook-data
kind: story
status: proposed
title: Decision classes, their holders and the files a decider is shown are data of the runbook
summary: One classes document of the runbook carries the classes, the files a decider is shown and the holder of each class at each level; the runner keeps the levels, the holders and the routing rule and checks the table at load.
tags:
- generic-runner
relations:
- decomposes: epic:runner-knows-no-runbook
- depends_on: story:runner-source-names-no-runbook-ratchet
- depends_on: story:second-runbook-with-model-node
- depends_on: story:run-inputs-declared-by-runbook
- depends_on: story:conformance-and-root-case-from-runbook
scope:
- confidence: cited
  path: README.md
- confidence: cited
  path: docs/autonomy-levels.md
- confidence: cited
  path: docs/conductor-comparison.md
- confidence: cited
  path: docs/cycles-suite.md
- confidence: cited
  path: docs/decisions.md
- confidence: cited
  path: docs/driver.md
- confidence: cited
  path: docs/eval-design.md
- confidence: cited
  path: docs/kpis.md
- confidence: cited
  path: docs/loom-run-decisions.md
- confidence: cited
  path: docs/orchestration.md
- confidence: cited
  path: docs/requester.md
- confidence: cited
  path: docs/takt-comparison.md
- confidence: cited
  path: driver/examples/sdc/decision-classes.json
- confidence: cited
  path: driver/src/bin/sdc-check-routes.rs
- confidence: cited
  path: driver/src/bin/sdc-protocol-site.rs
- confidence: cited
  path: ess/domains/decision.yaml
- confidence: inferred
  path: ess/generated
- confidence: cited
  path: ess/scenarios/merge-a4-rule-and-decider.yaml
- confidence: cited
  path: runbook/src/loom_run/bindings_copy.rs
- confidence: cited
  path: runbook/src/loom_run/decider.rs
- confidence: cited
  path: runbook/src/loom_run/decider/ask.rs
- confidence: cited
  path: runbook/src/loom_run/decider/inputs.rs
- confidence: cited
  path: runbook/src/loom_run/decider/requester.rs
- confidence: cited
  path: runbook/src/loom_run/decider/routing.rs
- confidence: cited
  path: runbook/src/loom_run/decider/tests.rs
- confidence: inferred
  path: runbook/tests/fixtures/decisions
- confidence: inferred
  path: runbook/tests/fixtures/model-runbook
- confidence: cited
  path: runbook/tests/loom_run/decider.rs
- confidence: inferred
  path: runbook/tests/loom_run/decision_classes.rs
- confidence: cited
  path: runbook/tests/loom_run/decisions.rs
- confidence: cited
  path: runbook/tests/loom_run/mod.rs
- confidence: inferred
  path: runbook/tests/source_names/known/runbook/src/loom_run/decider/inputs.rs.txt
- confidence: inferred
  path: runbook/tests/source_names/known/runbook/src/loom_run/decider/requester.rs.txt
- confidence: inferred
  path: runbook/tests/source_names/known/runbook/src/loom_run/decider/routing.rs.txt
- confidence: cited
  path: tools/autonomy-levels.json
- confidence: cited
  path: website/docs/context.mdx
- confidence: cited
  path: website/src/routes/Evaluation.tsx
revision: 15
transitions:
- {from: "draft", to: "proposed", at: "2026-11-23T06:01:22Z", actor: "human:operator", revision: 15, executor: "agent:runbook", correlation: "epic-runner-knows-no-runbook-critics-3"}
---
## Outcome

The classes of decisions of a runbook, the holder of each class at each autonomy level and the files a decider is shown are data of the runbook. One classes document of the runbook, found beside its bindings, carries for each class its type, its side, its actions and its holder at each of the levels A0 to A5. The runner holds no name of a class, of a protocol or of a file of a runbook, and keeps what is its own: the list of levels, the list of holders and the routing rule (only a class whose holder is `decider` goes to the decider; what the document does not state goes to a person).

## Source

- `epic:runner-knows-no-runbook`, item 4 ("The classes of decisions, their holders and the files a decider reads are stated by the runbook; the runner keeps the routing rule and the autonomy levels").
- Read at `main` `826b09c7` (the cited lines hold at `3c0eb863`, release 0.7.0: of the paths read, only `runbook/src/commands.rs` and tests of the daemon differ):
  - The classes and holders are data already: `driver/examples/sdc/decision-classes.json` (`sdc.decision-classes/1`, 14 classes, each with `type`, `side` and `actions` written `<protocol>/<action>`; the class `close` lists `sdc/close:operating`) and `tools/autonomy-levels.json` (`sdc.autonomy-levels/1`, the levels A0 to A5, class to holder).
  - What Rust still holds, in `runbook/src/loom_run/decider/routing.rs` (127 lines): the path `driver/examples/sdc/decision-classes.json` (`:19`); the class of a request by its kind, with the names `scope_changing_answer`, `finding`, `close` and `approval` (`class_of()`, `:41-54`); the close by rule, which builds `sdc/close:<outcome>` (`closes_by_rule()`, `:122-126`). The rule itself is `decider_takes()` (`:116-118`); the principal of a close by rule is `rule:autopilot` (`:29`).
  - `runbook/src/loom_run/decider/inputs.rs` (321 lines): `collect()` (`:75-101`) names `idea.yaml`, `dissection.yaml`, `questions.yaml`, `requirements.yaml`, `context.yaml`, `published/README.md`, `aep/vision/product.md`, `aep/initiative/roadmap.md`, `aep/epic` and `aep/story/<story>.md`; `story_of()` (`:66-71`) cuts the story out of the third field of the case id. `runbook/src/loom_run/decider/requester.rs:239` names `idea.yaml`, `dissection.yaml` and `goals.yaml`.
  - The decider links the SDC library: `git grep -c "sdc_tools::"` prints 8 for `decider/inputs.rs`, 3 for `decider/requester.rs`, 2 for `decider/call.rs`, 2 for `decider/validate.rs`.
  - Other readers of the classes document: `driver/src/bin/sdc-check-routes.rs:433`, `driver/src/bin/sdc-protocol-site.rs:479`.
  - The runner already holds the two lists: `AUTONOMY_LEVELS` (`runbook/src/loom_run/decider.rs:27`, A0 to A5) and `HOLDERS` (`decider/routing.rs:24`, nine values with `none`). `Routing::load` (`routing.rs:63-96`) reads `tools/autonomy-levels.json` above the bindings copy and refuses a level that is missing, a class the classes document does not have, a holder outside the list and a class with no holder at a level. A class of a second runbook has no cell in that file.
  - `runbook/src/loom_run/decider/ask.rs:223-228`: `class_name()` names the classes `scope_changing_answer` and `finding`.
  - Tests that hold the routing and the requests today: `below_a5_only_the_person_steps_the_table_gives_the_decider_get_its_principal_and_no_table_leaves_every_step_a_person` (`runbook/src/loom_run/bindings_copy/tests.rs:205`); `the_prompt_of_each_model_decider_is_its_golden_byte_for_byte` (`runbook/src/loom_run/decider/golden.rs:80`, goldens under `instructions/golden/decider/` and `instructions/golden/requester-request/`); `runbook/src/loom_run/decider/requester_tests.rs`; `runbook/tests/loom_run/decisions.rs` and `decider.rs`.
  - `tools/autonomy-levels.json` is named in `docs/autonomy-levels.md`, `docs/decisions.md`, `docs/loom-run-decisions.md`, `docs/requester.md`, `docs/driver.md` and in records of past runs and releases (`docs/releases/`, `experiments/`).

## Domain relations

- Decision request -> decision class: exactly one; class at a level -> holder: exactly one cell, or none where the class does not arise. inferable. The three are typed in `ess/domains/decision.yaml`: `sdc.decision.AutonomyLevel` (`:169-174`), `sdc.decision.DecisionClass` (`:176-180`) and `sdc.decision.Holder` (`:182-186`); the document has no `relations:` entry for class and level to holder, so that part is inferred from `runbook/src/loom_run/decider/routing.rs:110-118` and `tools/autonomy-levels.json`.
- Runbook -> classes document: exactly one, owned by the runbook. inferable (inferred from `runbook/src/loom_run/decider/routing.rs:19`, one fixed path; the summary of `sdc.decision.DecisionClass` says that in Loom a class is an identifier validated against the classes document of the host).
- Decision class -> file a decider is shown: zero to many; the runbook owns the list. inferable (inferred from `runbook/src/loom_run/decider/inputs.rs:75-101`; no `ess/1` document declares it).
- One conflict to settle inside the story: `sdc.decision.DecisionClass` is a closed enum of the 14 classes of SDC. A class of another runbook is outside it.

## Scope

- `runbook/src/loom_run/decider/routing.rs`, `runbook/src/loom_run/decider/inputs.rs`, `runbook/src/loom_run/decider/requester.rs`, `runbook/src/loom_run/decider/ask.rs`, `runbook/src/loom_run/decider/tests.rs` (the assertion at `:564` reads the name of the levels file in a message; the constant `LEVELS_FILE` it uses at `:545` goes), `runbook/src/loom_run/decider.rs` (the comment that names the levels file, `:24`), `runbook/src/loom_run/bindings_copy.rs` (`COMPANIONS`, `:33`, what is copied beside the bindings).
- `driver/examples/sdc/decision-classes.json` (its place and its new members, the holders among them) and `tools/autonomy-levels.json` (goes; its cells move into the classes document); `driver/src/bin/sdc-check-routes.rs`, `driver/src/bin/sdc-protocol-site.rs`.
- `ess/domains/decision.yaml` and `ess/generated/` where the enum has to open.
- Not edited: `docs/progress.json`. The command `sdc-experiment progress` writes it from the experiment records (`docs/progress.md:3`), and its line `:5779` that names the old path is the note of `experiments/parity-loom-strong-4.yaml:57`, a record of a past run. The story changes no experiment record and does not run that command; the `git grep` of the acceptance leaves out both.
- `runbook/tests/loom_run/decisions.rs`, `runbook/tests/loom_run/decider.rs`, new `runbook/tests/loom_run/decision_classes.rs` (with its line in `runbook/tests/loom_run/mod.rs`), the recorded file `runbook/tests/fixtures/decisions/sdc-holders.txt`, `runbook/tests/fixtures/model-runbook/` (one class and its files).
- `docs/loom-run-decisions.md`, `docs/decisions.md`, `docs/autonomy-levels.md`, and the other files that name the path `tools/autonomy-levels.json` as a file of today: `README.md`, `docs/requester.md`, `docs/driver.md`, `docs/cycles-suite.md`, `docs/eval-design.md`, `docs/kpis.md`, `docs/orchestration.md`, `docs/conductor-comparison.md`, `docs/takt-comparison.md`, `website/docs/context.mdx`, `website/src/routes/Evaluation.tsx`, `ess/scenarios/merge-a4-rule-and-decider.yaml`; the shards of `decider/routing.rs`, `decider/inputs.rs` and `decider/requester.rs`.
- Overlap, by story:
  - `story:run-inputs-declared-by-runbook`: edge. Chosen, and written in both stories: the `git grep` of that story does not read `runbook/src/loom_run/decider/`, and this story owns the lines `decider/inputs.rs:78,97` and `decider/requester.rs:239` (`idea.yaml`, `dissection.yaml`, `goals.yaml`). The requester takes the place of the goals document from the input that story declares. Both type the fixture and `runbook/tests/loom_run/mod.rs`.
  - `story:conformance-and-root-case-from-runbook`: edge (the fixture, `runbook/tests/loom_run/mod.rs`).
  - `story:node-classes-file-names-no-action`: it has an edge to this story (`driver/src/bin/sdc-protocol-site.rs`, `docs/loom-run-decisions.md`). `story:runner-source-names-no-runbook`: it has an edge to this story.
  - `story:runner-finds-model-node-by-binding` (`bindings_copy.rs`): ordered through the edges to `story:run-inputs-declared-by-runbook` and `story:second-runbook-with-model-node`.
  - `story:node-program-runs-steps-a-binding-names`, `story:node-program-built-ins-removed`: no file in common; this story can be built at once with either.
  - `story:run-settings-are-toml` (active; `runbook/src/loom_run/decider.rs`, the settings of the decider): precondition, it is on `main` before this story starts; this story changes one comment there. `story:run-without-model-node-needs-no-caps` (active; `bindings_copy.rs`): ordered through the edge of `story:run-inputs-declared-by-runbook` to it.
  - Drafts with no typed scope, each not built in the same wave as this story: `story:decider-inputs-read-once` (`runbook/src/loom_run/decider.rs`, `runbook/src/loom_run/decider/`, `docs/loom-run-decisions.md`), `story:replay-plan-out-of-the-run-copy` (`runbook/src/loom_run/decider.rs`), `story:route-findings-name-loom-stops` (`driver/src/bin/sdc-check-routes.rs`), `story:driver-md-instruction-files` and `story:producer-id-llm-node` (`docs/driver.md`), `story:experiment-passes-efforts` (`docs/eval-design.md`), `story:held-rows-name-existing-tests` (`docs/cycles-suite.md`), `story:submitrun-outcome-order` and `story:submit-run-key-is-bound-to-its-content` (`ess/generated/conformance/DIGEST` and `ess/generated/conformance/suite.json`: both drafts change `ess/domains/daemon.yaml`, and every change under `ess/domains/` or `ess/scenarios/` records the two files again). Of this story and each of the last two, whichever lands second records the digest and the suite again after `main` is merged into it.

## Acceptance

- `git grep -n "driver/examples/sdc\|scope_changing_answer\|sdc/close\|idea.yaml\|goals.yaml\|dissection.yaml\|aep/\|autonomy-levels" -- runbook/src/loom_run/decider runbook/src/loom_run/decider.rs` prints nothing outside test code.
- The holder of a class at a level is data of the runbook: `git ls-files tools/autonomy-levels.json` prints nothing, and the classes document carries for each class a holder at each of the six levels. `docs/loom-run-decisions.md` says in one sentence that this table is data of the author of the runbook and is trusted like the bindings of the runbook: who can change the runbook can change who decides.
- The runner checks the table against the rule it keeps, when `runbook run` reads the runbook and before anything of the run is written. A table that breaks the rule is refused with a usage error and one line that names the document, the class and the level; no run opens (fail closed). The rule: every class has a holder at each of the six levels, every holder and every level is one the runner knows, and a level never widens against the level above it. That is, where a person holds a class at a level (`person` or `advised`), a person holds it at every lower level too; the holder `none` (the class does not arise at the level) is left out of the comparison. One test each in `runbook/tests/loom_run/decision_classes.rs`: `a_class_without_a_holder_at_a_level_is_refused`, `a_table_without_a_level_is_refused`, `a_holder_the_runner_does_not_know_is_refused`, `a_level_the_runner_does_not_know_is_refused`, `a_class_a_person_holds_at_a_level_and_a_decider_below_it_is_refused` and `a_class_that_does_not_arise_at_a_level_is_not_compared`; each reads the exit code, the line and that no run directory was written.
- The table of SDC passes that check as it is: the test `the_table_of_sdc_passes_the_check_of_the_runner` reads the classes document of SDC. Read at `3c0eb863`: the 14 classes of `tools/autonomy-levels.json` keep the rule; `escalation` is the one class with `none` (at A0 and A1), and `limit_hit` is held by `rule` at every level and by a person at none.
- A table that turns unusable after the open still fails closed for each request: `a_table_that_is_missing_or_unusable_is_a_person_s_at_every_level_for_every_class` (`runbook/src/loom_run/decider/tests.rs:543`) passes with one assertion changed, the name of the file its message holds (`:564`), and with the six broken tables written into the classes document. A runbook that has no classes document runs as today, every step with a person (`a_missing_classes_document_leaves_every_step_a_person`).
- No file of today names the old path: `git grep -l "tools/autonomy-levels.json" -- . ":!docs/releases" ":!experiments" ":!CHANGELOG.md" ":!docs/progress.json" ":!docs/trial-2.md" ":!.engineering"` prints nothing. The paths left out are records of the past (releases, experiments, the changelog, the trial) and `docs/progress.json`, which a command writes from the experiment records.
- The classes document states what Rust stated: the class of a question, of a finding, of a close and of an approval on an action no class lists, and the outcomes that close by rule.
- For SDC every request gets the same class and the same holder at each level as before. The recorded file `runbook/tests/fixtures/decisions/sdc-holders.txt` holds one row for each of the 14 classes at each of the 6 levels (class, level, holder; 84 rows) and one row for each kind of request the existing tests make (kind, action, class); the test `every_class_of_sdc_has_its_recorded_holder_at_each_level` reads both through the routing of the runner and compares.
- Recorded files: a file this story calls recorded is written in the first commit of the story, before any other change, by the named test started with `RUNBOOK_BLESS_EXPECTED=1` (the test writes the file when the variable is set and compares with it otherwise, the way `SDC_BLESS_EXPECTED` works in `driver/tests/sdc_node.rs:319-329`). That commit changes no file under `runbook/src` or `driver/src`, and `git log --oneline <base>..HEAD -- <file>` prints one line: no later commit of the story changes the file.
- The routing fails closed, one test each: `a_request_of_a_class_the_document_does_not_state_goes_to_a_person`, `a_classes_document_that_cannot_be_read_stops_for_a_person`, `a_missing_classes_document_leaves_every_step_a_person`.
- An autonomy level below A5 never widens, and a missing table fails closed: `below_a5_only_the_person_steps_the_table_gives_the_decider_get_its_principal_and_no_table_leaves_every_step_a_person` passes with its assertions unchanged, with the table read from the classes document, and `below_a5_the_class_of_the_second_runbook_goes_to_the_decider_only_where_its_document_says_decider` holds it for the second runbook.
- An approval or a close is never recorded under the name of a person by a rule or a model: a close by rule is recorded as `rule:autopilot`, a decision of the decider under the principal of the decider. The tests of `runbook/tests/loom_run/decisions.rs` and `decider.rs` that hold this pass with their assertions unchanged, and `a_decision_of_the_second_runbook_is_recorded_under_the_decider_and_never_a_person` holds it for the second runbook.
- The files a decider is shown come from the data of the runbook. A path that is absolute or leaves the work directory of the case is refused when the data is read (`a_shown_file_outside_the_work_directory_is_refused`). For SDC the request a decider gets is byte-identical: `the_prompt_of_each_model_decider_is_its_golden_byte_for_byte` and the tests of `runbook/src/loom_run/decider/requester_tests.rs` pass, `git diff --stat <base> -- instructions/golden/decider instructions/golden/requester-request` prints nothing, and no step of the story runs with `SDC_BLESS_PROMPTS=1`. This is what keeps the claims of `experiments/decider-haiku-vs-opus.yaml` and `experiments/decider-program-approvals.yaml`.
- The second runbook has one class with an invented name: at one level its request goes to the decider (stub adapter, no model call), at another to a person (`the_class_of_the_second_runbook_is_routed_by_its_own_document`). Where the closed enum `sdc.decision.DecisionClass` refuses that class (the daemon, a generated schema), `ess/domains/decision.yaml` is changed first: the type becomes an identifier checked against the classes document of the runbook, `ess specify validate` passes and `task check:ess-generated` is green with the files recorded again.
- The spend cap fails closed for the decider as before (the tests of `runbook/tests/loom_run/spend.rs` pass with their assertions unchanged); `--confinement` is required and has no default (the tests of `runbook/tests/loom_run/flags.rs` pass unchanged).
- The runner links no more of the SDC library in the decider than before: `git grep -c "sdc_tools::" -- runbook/src/loom_run/decider` prints for no file a higher count than at `826b09c7` (`inputs.rs` 8, `requester.rs` 3, `call.rs` 2, `validate.rs` 2).
- The prompts of the model nodes are not touched: `git diff --stat <base> -- driver/src/bin/sdc-node.rs driver/src/bin/sdc-node instructions/golden/llm-node` prints nothing.
- A `review-result` that reviews this story with the role security and has no open finding of severity blocker or warning exists in the store before the story moves to implemented.
- The cycles keep their results: `task test:cycles-loom` (slow lane, not part of `task check`; started in the background and without the `slot` prefix, as its description says; its task names its time with `SLOT_TIMEOUT=30m`) ends with exit 0 and reports each of the 54 cycles of `cycles/tree-suite.yaml` with the result that file commits for it. `git diff --stat <base> -- cycles/` prints nothing: no expected result is edited to get there.
- `task check` is green at the head of the story.
- The last commit of the unit, made after `main` is merged into it, adds one entry under `[Unreleased]` of `CHANGELOG.md`. No other commit of the story changes that file, and the file is not in the typed scope of the story, so two stories of one wave do not meet on it.
- Tests: no test over 60 s and the test step within 120 s; no model call and no network in a test (stub adapter, sh programs); what is expensive (a built binary, a prepared run) is made once per test run, never per test. Each new test is timed before it is added; a test over 5 s is named with its time in the report. Nothing that stays is Python (Rust or POSIX sh).
- A program, variable, file or format id this story creates for a generic thing carries no `sdc` in its name (epic item 6; the names that exist follow stage 2 of `story:project-rename`).
- The shards of `story:runner-source-names-no-runbook-ratchet` (`runbook/tests/source_names/known/<source path>.txt`) for the source files this story types lose every line this story removes from the source and gain none. `git diff --stat <base> -- runbook/tests/source_names/` lists only those shards, and the report names the lines removed.
- No measured claim of `docs/` or `experiments/` changes. Where a number would change (the time of a node, the count of processes it starts), the measurement is recorded under `experiments/` and listed in `experiments/INDEX.md` before the claim is edited.

## Not in this story

- The list of levels (A0 to A5), the list of holders and the routing rule: they stay with the runner. The format ids and type names that say `sdc` (`sdc.decision-classes/1`, `sdc.decision.Holder`): item 6.
- Records of past runs and releases that name `tools/autonomy-levels.json` (`docs/releases/`, `experiments/`): they are records and are not edited.
- The answers file (`bindings/tools/answers.yaml`) and the decider prompt texts.
- This is the largest story of the epic. If it has to be cut, the cut is between classes and holders on one side and the files a decider is shown on the other.
