# Decisions of a `runbook run`: what the decider answers, and the run's copy of the bindings

`runbook run` runs the SDC cycle under Loom (`b10x-loom tree`). Loom stops at a step that waits
for a principal and prints it in `decisions[]` of the `tree run` result (exit 3). The decider
(`runbook/src/loom_run/decider.rs`) answers those entries; the copy of the bindings
(`runbook/src/loom_run/bindings_copy.rs`) is what makes Loom accept the decider as the principal
and carries the model, turn and budget settings of the run. Loom commit `5af3f9c2` (`b10x-loom`
0.58.0) is the one this page was checked against.

## The decisions table

The driver (`scripts/run-trial.sh`, `bindings/tools/decider.sh`) hands a decider these kinds of
decision. The last two columns say whether Loom can ask the kind as a `decisions[]` entry at the
pinned commit, and what a run does with it. A kind Loom cannot ask is not answered by a path made up
for it: the run ends or the step never exists.

| Driver kind | Inputs to the decider | Allowed answers | Loom `decisions[]` entry | What `runbook run` does |
|---|---|---|---|---|
| approval of a person step (`context.approve`, `vision.approve`, `roadmap.approve`, `merge.approve`) | `type approval`, `case_dir` (`case.json`, `work/`), `action`, `capability`, `subject`, `path`, `revision`, `position`, `record_dir` | approve, reject, escalate | Yes: kind `ApproveAction` (case, action, capability, subject, path, revision, command) | `decider.sh model` (or the rule kinds); approve runs the printed `tree approve` with `--result performed`; reject runs it with `--result rejected --reasons-file` holding the decider's text (Loom `5af3f9c2`); a rejection of an action that declares no `rejected` result stops the run `stopped: rejected` (exit 10, cause `rejected_without_rework_path`, the reason `<step> in <case>: …`), and an approval of a result the action does not declare ends it `error`. The driver's `approved_on_checks` (three invalid replies and every check true: approved) is not in the runner: Loom's `decisions[]` holds no claim values to check, and an approval nobody validly gave stays `CannotDecide`, so the run stops for a person |
| approval of a capability (`stories.activate`, `release.publish`, `release.rollback`, the spec and code acceptances that a person gives) | the same | the same | Yes: kind `Approve`; Loom accepts `decider:model` as the principal and records it as the actor (checked in a full stub cycle, 5 approvals) | the same |
| escalate (decider reply) | the reply's `best` | `best` approve or reject | not a Loom entry: a reply | `escalate: final` takes `best` (reject when absent), as the driver's `escalation_suppressed`; `escalate: stop` ends in `CannotDecide {reason: "reserved for a person: …"}` |
| question: scope-changing answer, `scope.answer` | `type question`, the question object | approve with an answer text, escalate | Reachable as an `ApproveAction` of `scope.answer`, but Loom's approval carries no answer and takes none: the action lists no results, so the printed command has no `--result` and no `--reasons-file` | with `tools/answers.yaml` declaring the action (the committed bindings do): one request per open question, one decider call each at A5, the answer written into the workspace by the file's `apply` program, then the file's `then` program takes the step and the printed `tree approve` runs with the decider's principal. The answer text is in the work file, in `decisions.jsonl` and in the decider's record, not in Loom's approval. Without the declaration: `CannotDecide` ("scope.answer is a step whose outcome is a person's text …") and the copy keeps `person:owner` |
| question: `questions.answer` | | | No: a model node (class `other`) in the bindings, not a decision | runs as a node |
| finding (`amend.upstream` handoff, the decider answers an option or its own text) | `type finding`, the finding object | approve with an answer, escalate | Reachable as the `Handoff` of `amend.upstream`: a finding that needs a person is a request of the answers file's `requests` program | one request per such finding, one decider call each; the `apply` program writes the answer as the finding's `decision` and `decided_by`, then the printed `tree run` runs. A finding the driver would re-raise (`reraised`) ends the run `error`, as it stops the driver. What Loom cannot do (refuse a finding) stays in `bindings/PENDING.md` |
| close (`sdc/close:operating`, choice between legitimate outcomes) | `type close`, `outcome` | approve, reject | Reachable: kind `Close` | at A5 a close through an outcome the close class lists (`operating`) is taken by `rule:autopilot`: no decider call, no model call, the printed `tree close` runs. Any other close is asked as `type close` with the printed outcome where the table gives the close to the decider (A4; A5 for an outcome the class does not list); approve runs the printed `tree close`, decline runs nothing and the same close that comes back ends the run `denied`; the other outcomes of that case are not asked after one ran |
| handoff | a writer's handoff text | none | Yes: kind `Handoff`, the printed command is `tree run --store … --confinement …` | a scripted approval runs the printed `tree run` through the ledger; `decider.sh` has no reply for it, so a program decider ends `CannotDecide` ("kind Handoff …"); a handoff that comes back after its command ran is an `error` |
| an unknown kind | | | whatever Loom adds | `CannotDecide` ("kind X not known") |
| withhold, run.unstick, keep-going | the driver's own routes after a decision | | Not reachable through `tree` today: Loom has no such steps | a run has none of them; the first stop is the stop |
| rework after a rejected person step | the driver's rework route | | Reachable at Loom `5af3f9c2`: the rejection is the step's named result and the writer's `retry_when` sends it back with the reasons in `LOOM_RETRY_FEEDBACK` | the rejection runs as above; the writer runs again (bindings `retry_when: {<evidence kind>: [rejected]}`, `llm-node.sh` writes the reasons into `in/feedback.md`) |
| `SDC_RULE_APPROVALS` (rule approvals with claims) | claims of the position | | Not reachable through `tree` today: Loom's `decisions[]` holds no claim values | not mapped |
| budget overrun, limit hit | | | not Loom entries: the runner's own stops (R1's ledger and caps) | outcome `stopped`, not a decision |

Notes.

- The decider runs inside the runner (`runbook/src/loom_run/decider/`; `bindings/tools/decider.sh` is
  no longer started and stays for the old driver's scripts). It has the driver's mode only. There is no
  Loom mode: the decider builds the request it reads (`case_dir` with `case.json` and a link `work` to the run's workspace) from the entry and
  Loom's case file. `case.json` holds the case and the artifacts Loom's store knows with their
  revisions.
- The decider's `claims` are not in Loom's output. The model prompt tells the model to read the
  claims of the position from `in/checks.json`; it gets an empty position. A model decider therefore
  runs only with `decider.without_claims: true` in the settings, the run's explicit acknowledgement;
  without it the answer is `CannotDecide` naming this. Every record of the run says so.
- The principal is `decider:model` (kind `model`), `decider:approve` or `decider:reject` (kind
  `rule`, the driver's `SDC_DECIDER=approve|reject`).
- Autonomy. At `autonomy: A5` the decider answers every decision, except the close through
  `operating`, which `rule:autopilot` takes; it reads `tools/autonomy-levels.json` first, as at
  every level, and with a table that is missing or unreadable it answers nothing (below). A0 and no level named end in `CannotDecide {reason:
  "reserved for a person: …"}`. A1 to A4 route by class: `tools/autonomy-levels.json` (the table of
  `docs/autonomy-levels.md` § 2.2, one cell per level and class; one loader,
  `runbook/src/loom_run/decider/routing.rs`, which the requester decider reads its classes
  through) names the holder of each class. Only a class the table gives to `decider` is answered
  (`context`, `spec_acceptance`, `story_activation` and `code_review` from A2; also the scope
  answers, `vision`, `roadmap` from A3; the close at A4). A class held by anyone else (`person`,
  `advised`, `rule`, `rule_and_decider`: no rule is built, so the decider alone never takes the merge
  at A4) and a class the table does not know end in `CannotDecide {reason: "reserved for a person:
  autonomy A<n> …"}` and call no model. So does a table that is missing, unreadable or
  inconsistent (a format, a level, a class or a holder that is not what it should be), at every
  level A1 to A5: the table is required, and no rule and no model decides a decision without it;
  at A5 the first decision of the run stops, the run ends the way a decision that waits for a
  person ends (exit 8). Below A5 `escalate:
  final` does not apply: an escalate stops for a person. In the copy the person steps the table
  gives to the decider get `decider:<kind>` and the others keep `person:owner`; without a table the
  copy is made all the same, every step keeps its person, and the first decision stops the run
  for a person. A rule or a model never records an approval or a close under a person's name:
  the principal is `rule:autopilot`, `decider:model`, `decider:approve` or `decider:reject`.
  `driver/examples/sdc/decision-classes.json` and `tools/autonomy-levels.json` are copied beside
  the run's bindings; the copy stays in the run directory as the record of what the run was routed
  by. A bound program can write that copy under `--confinement local`, so the runner reads both
  files once, right after the copy is made and before the first action of the run
  (`Decider::pin_routing`), and routes every decision of the run, the requester's classes
  included, from that read in memory: a rewrite of the copy during the run changes no routing. A
  table that cannot be read at that first read is kept as the error, and every decision stops for a
  person, at every level. A test reads the table of the document and compares it to the JSON.
  The driver's A3 routing (the decider rejects a release, the decider is asked for the close) is
  not ported: the table gives the release and the close at A3 to a person, and a level never widens
  what its table gives.
- Call limit. `decider.max_calls` (the driver's `SDC_MAX_DECIDER_CALLS`; `max_decider_calls` is
  accepted as the experiment record's name for it, one of the two) counts the decider's model calls:
  a record directory under `<run>/decider/` per call, retries included. At the limit the next decision
  is `CannotDecide` ("decider budget spent: n decider calls"). A rule decider makes no call and has
  no limit. Default 20.
- `escalate: final` applies at A5 to the decisions Loom can ask (the two approval kinds), so it is
  implemented, not only described. Default `stop`.
- The reason of a stop at a decision the decider cannot take names the step, the case and what could
  not be decided, and no person (`amend.upstream in IDEA-1: kind Handoff: …`).
- An invalid reply (`valid: false`) is asked again with its reason, `decider.retries` times (default 2,
  the driver's `SDC_DECIDER_RETRIES`), then `CannotDecide`.
- Anything missing, unparsable or with an exit code other than 0 is `CannotDecide`: no JSON object, no
  `decision`, no `cost_usd`, a non-zero exit (the last stderr lines are in the reason), a timeout
  (`timeout_seconds`, default 900; the adapter's whole process group is ended), an adapter that
  cannot start (its call directory then holds no `out/decision.json`: an invalid reply).
- The decider's `cost_usd` is the cost of the call; the retries add up. The per-call budget the
  runner reserved is handed to the adapter as `max_budget_usd` of its request, never more than
  `decider.max_budget_usd` (default 0.25).

## The decider's environment

The decider starts one program: the adapter of `decider.adapter` (`metaharness`, `claude-cli`,
`dry`, the default `metaharness`, or a path), from the run's bindings copy, with the call's
`request.json` as its only argument. It gets the environment built here and nothing inherited from
the runner or the shell (`env_clear`), in its own process group. The working directory is the call's
record directory; standard input is empty. The model, effort, turn limit and per-call budget are
members of `request.json`, not variables.

| Variable | Value | When |
|---|---|---|
| `PATH` | the `PATH` of the runner's `child_env` (R1), nothing else of it | always; a `child_env` without `PATH` is `CannotDecide` |
| `HOME` | `<run>/home` | always |
| `SDC_REPLY` | `decider.reply_bin`, the `sdc-reply` binary the launcher found on `PATH` | kind `model`, when one is found (a scripted adapter of a test may read it; the three adapters of the copy do not) |
| `SDC_CLAUDE_CREDENTIALS` | `<run>/login`, the link the copy made (the settings' path when there is no link) | only when the settings name a login file |

The adapters are bash with `jq` (`metaharness`, `claude-cli`, `dry`) and need `jq` on `PATH`. The
decider itself needs no `jq`, `yq`, `rsync` or `bash`: it renders the prompt, copies the inputs,
resolves the cites and checks the reply in the runner, with the library of `sdc-reply`
(`driver/src/reply/`). The files of the call (`request.json`, `prompt.md`, `reply-schema.json`,
`response.json`, `status.json`, `cwd/in/`, `cwd/out/decision.json`) and the reply are the bytes
`decider.sh` wrote; the slow lane `task test:decider-parity` compares them, the requester's too.
The `requester` kind of `decider.sh` is `decider.requester: true` (see "The requester decider"
below). The decider finds `instructions/decider/` by
walking up from the bindings copy's tools directory: the copy puts `instructions/` beside it. The
login's path is removed from every stderr line the decider keeps (`<login file>`), and it is in no
argument, no file of the run directory but the link, and no result.

## The requester decider

`runbook run --goals <file>` copies the requester's goals document unchanged to `work/goals.yaml` of
the run before the tree opens; a path that is no file is refused with exit 64. With `decider.requester:
true` (kind `model`; it needs `--goals`, else the run is refused before anything is written) the decider
answers the decisions of the requester-side classes of `driver/examples/sdc/decision-classes.json`
(`scope_changing_answer`, `context`, `vision`, `roadmap`, `release`, `close`, `budget_overrun`: the ones the file marks
`requester`) as principal `decider:requester`; the other classes stay with the model decider
(`decider:model`). One call of the adapter: its `cwd/in/` holds the request, the subject file, the files
the request's grounds cite, `idea.yaml`, `dissection.yaml`, `goals.yaml` and `decisions.json` (its own
earlier decisions of this run) and nothing else; it writes `cwd/out/reply.json`. The prompt and the
question and options of an approval or a close are the files `instructions/decider/requester*.md`. The
grounds go through the program `tools/decision-grounds.sh` (the run's copy carries the checkout's
`tools/` beside the bindings, and a scope question's request comes from `sdc-decision-request` beside
`sdc-reply`). Option A approves, B rejects; a free answer answers a question. A reply with
`no_ground_in_goals` is an escalation: under `escalate: final` the question's default (or `approve` for
an approval) is taken and marked, under `stop` the run waits for a person. A refused grounding check is
an invalid reply and is asked again with the reason (`decider.retries`).

An approval line of `decisions.jsonl` has the class `approval` or `close` whatever routed it; the
routed class and `side` are in `decider/<n>/decide-request.json`; the lines of a question have both.
Not reached under the real engine yet: the entries `requester-decider` and `requester-no-ground` of
`cycles/suite.yaml` stop at `scope.answer` of `root.intake`, a program on a person's action
(`docs/cycles-suite.md`), which the engine refuses today; the fake-Loom tests of
`runbook/tests/loom_run/goals.rs` hold the same checks.

## The answers file

`bindings/tools/answers.yaml` (format `runbook.loom-run-answers/1`, read by
`runbook/src/loom_run/answers.rs`) declares, per action, what the runner does before it runs the
command Loom printed. It sits in `tools/` so the scans of the top-level `*.yaml` bindings do not
read it as a binding. The Rust code knows the format of the file and of a request line; the names
of the actions and of the work files are the file's and its programs'.

| Key | Meaning |
|---|---|
| `action`, `on` | the action id and the kind of Loom's decision (`ApproveAction` or `Handoff`) |
| `requests` | a program that prints one JSON object per line: `type` (`question` or `finding`), `id`, a member of that name holding the object (`default`, and for a finding `options`), and optionally `subject`, `path`, `capability`, passed to `decider.sh` as the request members of the driver |
| `apply` | a program that writes one answer into the workspace; words may hold `{id}`, `{answer}`, `{principal}`, `{case}`, `{type}` |
| `then` | optional, run once after the last answer; words may hold `{principal}`, `{case}` |

A program word with a `/` is a file of the run's bindings copy; any other word is looked up on the
`PATH` of the runner's child environment. A program starts in the workspace with `PATH` and
`LOOM_CASE` and nothing else, no standard input, its own process group, ended after 300 s.
`sdc-answers` (`driver/src/bin/sdc-answers.rs`) is the Rust port of the driver's `yq` logic for
questions and findings.

What a run does for a declared action, at `autonomy: A5` with a model or rule decider:

1. Runs `requests`. No request: the printed command runs as before.
2. For each request: one decider call (a record directory `<run>/decider/NNN-<id>`), counted against
   `decider.max_calls`, the model-call cap and the spend ledger; one line per answer in
   `decisions.jsonl` (`type`, the id, `answer`, `principal`, `decision`). The summary counts them in
   the classes `scope_changing_answer` (questions) and `finding`: asked, approved, escalated, calls.
3. An invalid reply is asked again `decider.retries` times, then `escalate: final` takes the
   request's default (`fallback: invalid_reply_default`) and `stop` ends the run. An escalation
   under `final` takes the default of a question and the `best` of a finding (else its default),
   with `suppressed: true`; under `stop` the run ends for a person with the decider's reasons. A
   rejection ends the run. These are the driver's `decide` rules (`scripts/run-trial.sh`, lines
   804 to 905; `answer_questions`, 1122 to 1148; `decide_findings`, 1150 to 1227).
4. Runs `apply` for the answer, then `then`, then the printed command, with the decider's principal
   (`decider:<kind>`) in `--principal`. The same request coming back after three answers ends the
   run `error`: the `apply` program did not take the answer.

Where the answer text goes. Where the action lists `performed` (an `ApproveAction` that names
results) it is also in the reasons file of the approval. `scope.answer` lists no results, so the
printed `tree approve` takes neither `--result` nor `--reasons-file`: Loom's approval of it cannot
carry the answer, which is only in the workspace file, `decisions.jsonl` and the decider's record.
An action the file does not declare behaves as before: `CannotDecide`, and in the copy a person step
without a recorded result keeps `person:owner`.

## The run's copy of the bindings

`materialise(src, dst, root, settings)` copies `bindings/` to `dst` (an unused directory) and applies
the settings with `serde_yaml`, structurally, to the copy. The committed `bindings/*.yaml` are never
written (a test copies a read-only source and compares bytes). A file no setting touches is copied
byte for byte; a file one touches is re-serialised (comments are not kept). All 66 bindings are
in the copy; a test counts them before and after and compares every member except the rewritten
ones. Beside `dst` it puts `instructions/` and `driver/examples/` (the tools read their root, two directories above their own: the instruction files, the fixtures and `rule-ends.json`) and,
when a login file is named, the link `login`. An unknown settings key, at any level, is refused by
name before anything is written.

| Setting | Where it lands in the copy |
|---|---|
| `autonomy: A5` with `decider.kind` `model` or `rule` | `executor.principal` and each evidence `producer` of the person steps `context.approve`, `vision.approve`, `roadmap.approve`, `merge.approve` become `decider:model` (or `decider:approve`, `decider:reject`); so does a person step that lists no `from: effect` result when `tools/answers.yaml` declares it (`scope.answer` in the committed bindings); without a declaration such a step keeps `person:owner` |
| `models.<role>.model` | option `--model` of the `sdc-node` nodes of the role (writer, implement, review, other, by `bindings/tools/node-classes.json`) |
| `models.<role>.max_turns` | option `--max-turns` |
| `models.<role>.max_budget_usd` | option `--max-budget-usd` |
| `adapter` (`metaharness`, `claude-cli`) | option `--adapter` of every model node |
| `login_file` | option `--claude-credentials <run>/login` of every model node; `<run>/login` is a link to the file |

`budget.max_runs`, the only budget member a tree acts on, stays as committed: `max_turns` and
`max_budget_micro_usd` are refused on program bindings by Loom.

## The strong variant of `experiments/release-0.1.2.yaml`

Fixed settings plus the strong variant's own, each with the `SDC_*` variable `sdc-experiment` sets
and what it becomes here.

| `SDC_*` setting (value) | In the copy or the settings |
|---|---|
| `SDC_LLM_MODEL_WRITER`, `_IMPLEMENT`, `_REVIEW`, `_OTHER` (claude-sonnet-5-5) | expressed in the copy as `--model` (`models.<role>.model`, the committed bindings) |
| `SDC_LLM_MODEL_DECIDER` (claude-opus-5-5) | the decider's environment, `decider.model` (not the copy) |
| `SDC_DECIDER` (model) | `decider.kind: model` (settings) |
| `SDC_DECIDER_ESCALATE` (final) | `decider.escalate` (decider environment) |
| `SDC_MAX_DECIDER_CALLS` (40) | `decider.max_calls`, counted by the decider (not an environment variable) |
| `SDC_DECIDER_ADAPTER` (claude-cli) | `decider.adapter` (decider environment) |
| `SDC_LLM_ADAPTER` (metaharness) | expressed in the copy as `--adapter metaharness` |
| `SDC_REVIEW_ADAPTER` (claude-cli) | the default of `sdc-node` when the writers run on metaharness; no option exists to set another, so only `review.adapter: claude-cli` is accepted. Not expressible under Loom today for any other value |
| `SDC_AUTONOMY` (A5) | `autonomy` (settings): the decider's gate and the person steps' principal in the copy |
| `SDC_MAX_LLM_CALLS` (150), `SDC_MAX_WALL_SECONDS` (5400) | the runner's caps (R1); not in the copy |
| `SDC_LLM_EFFORT_WRITER`, `_IMPLEMENT`, `_REVIEW`, `_OTHER` (not set; node-classes default high) | expressed in the copy as `--effort` of `sdc-node` (`efforts.<role>.effort`, `bindings_copy.rs`); a role with none runs at the default of `node-classes.json`, which `sdc-node` passes to the adapter as it always did. The decider's effort is `decider.effort` |
| `SDC_UNSTICK` / `SDC_LLM_MODEL_UNSTICK` (not launch settings) | not expressible: run.unstick does not exist under Loom; a stop stays the end of the run |
| `SDC_DECIDER_RETRIES` (default 2) | `decider.retries`, the decider |
| `SDC_RULE_APPROVALS`, `SDC_CONFORMANCE` (not set) | not expressible: see the table above (no claims in `decisions[]`) |

Per-call budgets (`--max-budget-usd`) and turns (`--max-turns`) are not part of the strong variant's
record: the committed bindings carry the node's own values, which the copy keeps unless a run's
settings change them.

## Checks

- Fast: `cargo test --locked --bin runbook loom_run::decider` and `loom_run::bindings_copy` (the
  fake adapter is a script the test writes, the rules run in the runner; no model). The slow lane
  `task test:decider-parity` runs `decider.sh` and the Rust decider on the same cases and compares
  the bytes; it goes with `decider.sh`.
- That Loom reads the copy (slow lane, needs the pinned `b10x-loom`): `RUNBOOK_LOOM_BIN=<b10x-loom>
  cargo test --locked --bin runbook -- --ignored loads_in_loom`. The test registers the protocols,
  makes a copy with the model, turn, budget, adapter and principal rewrites, runs `tree open` on it
  and `tree verify` on the store, with an environment of `PATH` and `HOME` alone. Result on Loom
  `5af3f9c2`: passes (2.2 s; the gate lane runs 216 tests in 30 s).
