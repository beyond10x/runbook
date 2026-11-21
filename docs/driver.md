# SDC driver

The driver runs one SDC case on top of Canon. Canon decides every status. The driver reads the
`canon-decision/1`, runs the tool bound to one admissible action, turns the tool's result into a
`canon-evidence/1` record, and evaluates again. An LLM is only ever one action's executor. Its
output becomes an artifact revision, never evidence and never the next state.

Everything below was checked on 2026-11-20 and 2026-11-21. Lines marked *inferred* were not
observed.

## Status (2026-11-21, third pass: ready for a real LLM)

- An LLM executor for the agent nodes (`tools/llm-node.sh`, see "LLM executor") behind a
  pluggable adapter: metaharness (default for a trial), `claude -p`, dry, or any program with the
  same contract. Model `claude-haiku-5-5`, a binding value. The stub stays the default, so
  `scripts/run-full-cycle.sh` runs unchanged with no LLM.
- Closed: stops on `context.default` and `vision.write` (sdc.direction r3); a per-action tool
  timeout (`timeout_s`); a store lock; evidence and `action_finished` before the revision bump;
  atomic document writes and an appended log; caps on LLM calls (`run --max-llm-calls`, over the
  tree) and on wall time (`run --max-wall-seconds`). New for a real LLM: `retry_when` (a failed
  check sends the writer back, with the check's log as feedback), `handoff` (a writer that did
  what it may hands over to a person), `args_from` (epics and stories come from the planning store,
  not a fixed list), and `run` names the exact command when it waits on a person.
- Verified 2026-11-21: `scripts/run-full-cycle.sh` exit 0, root `operating`, replay 8 of 8
  match; `scripts/run-trial.sh --dry trial-dry` exit 0, 10 LLM commands printed and none run,
  replay 8 of 8 match; `check-protocols.sh`, `check-bindings.sh`, `evaluate-cases.sh` exit 0.
- The trial itself: `docs/trial-1.md` (`runs/trial-todo-1/`: 3 real Haiku calls through
  metaharness, stopped for the scope-changing interview answers).

## Status (2026-11-20, second pass)

One full loop runs end to end: idea → intake → direction → epic (spec, plan, story, release) →
root outcome `operating`, over all eight protocols, with stub LLM executors, real `ess` (gate,
validate, compile, synthesize, conformance run) and real `aep` (vision, roadmap, epic, story,
validate, waves, list, moves) in the case tree's own planning store. Run:
`runs/full-cycle-1/` (see "Full-cycle run"). What made it work:

- Bindings: one file per protocol, `driver/examples/sdc/<protocol>.bindings.json`, checked
  against the compiled protocols by `scripts/check-bindings.sh` (all 8 pass).
- `until` is gone from the driver and the bindings. Where a writer needs a stop, it is now the
  action's precondition (sdc.intake r4, sdc.direction r2, sdc.plan r4; see "Preconditions that
  replaced `until`").
- Child cases (`case_open` → `case_outcome`) and the `roadmap.read` copy are built, as a
  **temporary copy of what Loom must provide** (boundary.md rows 27 and 28). They are marked
  `TEMPORARY` in `src/main.rs` and are deleted here once Loom has them.

## Decision

- Prototype: `sdc/driver/`, one Rust binary `sdc-drive` (clap derive, three dependencies, 943
  lines with the child-case runtime; 685 before the third pass). It calls `canon evaluate` as a
  subprocess and keeps the case as plain files.
- The first version was 395 lines. The child-case runtime, the copy action, `$artifacts`
  sync, person's actions and `replay` added about 290 lines, most of which is the temporary
  Loom copy.
- Target home (*inferred*): Loom. Loom already has the Canon governor, the
  `run_until_blocked` loop, effect ports and a deterministic workflow executor (see "What
  exists"). The prototype should move there once Loom has a CLI that runs a workflow
  definition with only deterministic steps.

## What exists

| Repo | What it does with Canon (verified) | Missing for this driver |
|---|---|---|
| metaharness | Nothing in code. `AGENTS.md:61`: "`metaharness aep drive` is kept as the governed-run driver until Loom's session executor…". `drive.rs:2782` hands `run`/`resume` to AEP's driver. | No Canon at all. It runs on AEP's own engine (`aep/crates/drive/aep-driver/src/run.rs:594` `drive()`, loop at `:835`, engine evaluate at `:872`). It does have what to copy: a digest-sealed run directory (`run.rs:46-50`), visit, iteration and retry budgets (`:882`, `:887`), operator pauses, and per-call tool policy. |
| aep (`aep drive`) | Nothing (all matches are "canonical"). | Same loop shape as wanted (`aep-driver/src/lib.rs:1-25`: "restore → evaluate → select step → execute → submit evidence → transition"; "Gates are evaluated only by the engine"), on AEP's engine and a static step map. Its rule that an `llm` step carries no `evidence:` key (`drivers/development/default.yaml:17-18`) is the rule used here. |
| conductor | Nothing (matches are "canonicalize"). | Deterministic daemon loop with bounded LLM decisions (`docs/design/conductor-run.md:122-131`). No case, evidence or tool binding. |
| commission | No dependency: `crates/commission-testkit/tests/executor_port.rs:45` refuses Loom and Canon. Has ports (`Governor`, `EvidencePort`, `AuthorityProvider`) and `run_until_blocked`. | "no request is executed. Commission has no effect port yet" (`crates/commission/src/runtime.rs:35-36`). |
| engineering-protocols | A library of Canon protocols that `loom-governor` consumes (`AGENTS.md:15`). Actions are tool-agnostic (`AGENTS.md:23`). Fixtures replay a `canon-case/1` through scripted evidence states. | No executor and no binding, by design. |
| Atlas ADR 0066-0075 and Atlas's governed-autonomy design | 0072: visible tools are integrations ∩ protocol-admissible ∩ capabilities, revalidated before each effect. 0073: the selector picks only from the frontier, then arguments are schema-validated, revalidated and executed by a trusted adapter. 0074: a trace is not evidence. | Design only. Atlas's delivery-loop design note (dated 2026-10-04) lists Commission's loop and effects, triggers and the approval inbox as missing. |
| loom (extra) | `CanonGovernor` (`crates/loom-governor/src/lib.rs:701`) maps `admissible`/`approval-required` (`:1899-1900`). The Commission loop with effects is at `crates/loom-commission/src/runtime.rs:1148`. Each action binds to one Connector operation (`ports/connector.rs:1-9`). `WorkflowExecutor` runs deterministic steps conditioned on Canon claims (`crates/loom-executor/src/workflow.rs:1-20`). | The production selector is an LLM (`ModelSelector`, `model_selector.rs:184`). `FirstAdmissibleSelector` is "used only for tests/examples" (`selection.rs:140`). `b10x-loom` 0.4.0 offers only `run <INTENT>` (the intake slice) and `protocols`, with no way to run a workflow definition. |

Canon: no `canon` on PATH (`command not found`). `../canon/target/debug/canon`
0.9.0 exists, and every run below used it. That answers intake gap G7.

Per-node, per-edge and per-run metrics of a recorded run (`sdc-run-metrics`, `sdc-experiment`): `docs/workflow-metrics.md`.

## The loop

Each step does the following, with nothing else as input:

| # | Step | Input | Output |
|---|---|---|---|
| 1 | Evaluate | `ir.json`, `case.json`, `evidence/`, `authority.json`, `decisions.json`, `--at` | `evaluations/NNNN.json` (`canon-decision/1`), event `evaluated` {state digest, decision digest} |
| 2 | Position | the decision plus the case store | admissible, approval-required, useful actions, legitimate outcomes, open obligations |
| 3 | Select | the useful actions | one action: `--action` (must be admissible), or the fixed policy. With none, a verdict. |
| 4 | Check the call | the IR action, the binding, the arguments | refuse when the arguments do not fit the schema, or the binding produces a kind the action's `may_produce` does not list |
| 5 | Run | the binding's `argv` (none: the driver's own action), cwd `work/`, stdin empty, its own process group, killed after the binding's `timeout_s` (default 1800; exit `timeout`); env `SDC_ACTION`, `SDC_ARGS`, `SDC_CASE` (case id), `SDC_CASE_DIR` (absolute case directory), `SDC_LLM_NODE` (the binding's `llm`), `SDC_FEEDBACK` (on a retry) | exit code, plus stdout and stderr as a content-addressed blob |
| 6 | Revise (pending) | the bindings' `$artifacts` {artifact → path in `work/`} | every artifact whose content digest changed gets revision `t<sha256(tree)[..16]>` (`none` while absent), next case revision `cN`; computed, not yet written |
| 7 | Record | the binding's `results` {exit code or `*` → result; for several `produces`, an object kind → result} | `evidence/<id>.json`: subject revision from the pending snapshot, `observed_at` = `--at`, `producer` from the binding, `upstream_revisions` for the binding's `upstream` artifacts. An observing action (`effect` not `write`) that changed an artifact records nothing (event `discarded`) |
| 8 | Open children | a `case_open` action that exited 0 | see "Child cases" |
| 9 | Finish, then commit | | event `action_finished` (exit, log blob, evidence ids, `revises`, digest of the pending `case.json`), then `case.json` and event `revised`. A crash before the commit leaves evidence about a revision `case.json` does not hold yet (Canon sets it aside), never a moved revision without its step. Pending revisions are also committed before every evaluation, so a child's writes and a person's edits reach the case |
| 10 | Re-evaluate | as step 1 | new position |

**Fixed policy** (`useful()` in `src/main.rs`):

- Only bound, admissible actions are candidates. Admissible alone is not enough: `inspect` stays
  admissible after its evidence exists, and the first run looped on `attempt_falsification`.
- An action that may produce evidence is useful while one of its kinds has no applicable record
  at the subject's current revision.
- An action that produces nothing is useful until it has succeeded against the current
  `case.json`. When a writer must stop for good is the protocol's precondition, not the
  binding's (`until` was removed).
- A `copy` binding is useful while its source case holds a record this case lacks.
- A `manual` binding is a person's action: never taken by the policy, only by
  `approve --action A`, with the binding's `{principal}` producer set to the approving principal.
- Run caps (2026-11-21, `trial-2.md` Run notes R4 and R7: a person's decision moved a revision,
  then a flat cap left nothing admissible, four times). An observation (any `effect` but
  `write`) has no cap: it runs at most once per case snapshot (`case.json`, which holds the
  revision of every artifact it reads), and only while it is due (a kind it may produce has no
  applicable record at its subject's revision; with no kind, until it ran at this snapshot);
  its binding's `max_runs` is ignored. A writer has `max_runs` (default 8) per arrival: only
  its runs after the last arrival count. An arrival is a `revised` event that no agent action
  of this case produced: the opening, a person's edit in `work/` (a finding decision, an
  answer), a person's `approve --action`, a write from another case of the tree. So a
  person's decision gives every writer a fresh allowance.
- A run that did nothing is not a run (2026-11-21, `runs/trial-3-haiku-4`: a "not called" run
  of `change.implement` took the last of its 3, and the real failure after it ended
  `writer_spent`). The driver gives each tool `SDC_NOT_CALLED`, a file; a tool that exits 0
  without doing its work (`llm-node.sh`'s "not called" exits) writes why there, and the run's
  `action_finished` carries `not_called` with that text. Such a run does not count toward
  `max_runs`. A writer whose not-called run is the case's last step, at the current snapshot,
  is not useful (it would see the same inputs); when nothing else is useful the run ends with
  the verdict `writer_not_called` (exit 4, event `writer_not_called` with the action, the run
  and the reason; `run-trial.sh` stops an autopilot run with that cause).
- `llm-node.sh` re-derives with the LLM whenever a rejection of the writer's subject is pending:
  the case's last rejected approval with no run of the writer after it that exited 0 and was not
  `not_called`. It is given as `in/rejection.json` even when `SDC_FEEDBACK` does not carry it,
  and the own-default skip ("the decisions since ... are the defaults it raised") does not apply.
- A writer that would be useful but has spent its allowance ends the run with the verdict
  `writer_spent` (exit 4, event `writer_spent` with `spent`: action, runs, `max_runs`, the
  arrival's event index, last exit and last log line, and `sent_back`: per counted run, the
  first other action after it that exited non-zero and its last log line, also in the message;
  `runs/trial-3-haiku-2` stopped with "last exit 0:" and nothing else). Order of the stops when nothing is
  useful: a legitimate outcome; a person's own action (`manual`) or a due `handoff`; with a
  spent writer, a pending approval (`scope.answer` after `questions.answer` refused a changed
  answer), else `writer_spent`; an approval; `stuck`. A `handoff` is due only while its
  binding's `handoff_when` argv (run in `work/`) exits 0 (`amend.upstream`: `sdc-tool.sh
  findings-wait`, a finding waits for a person), so a spent writer is never reported as
  `awaiting_person` on an `amend.upstream` with nothing to decide; with a spent writer a due
  handoff stands even while its action is not admissible (the person's decision is what gives
  the writer a new allowance). Cases opened before `handoff_when` keep the old bindings: their
  handoff is taken as given. The new event is appended to the hash chain like `stuck`;
  `replay` reads only `evaluated` events and the state digest, so replay of runs written
  before the change still matches. `replay` lists a child that was opened but never evaluated
  as `evaluated: false` instead of failing.
- A tool exit 126 or 127 (a program it needs is missing, `need` in `sdc-tool.sh`) or 75 (a
  temporary failure: `llm-node.sh` maps a session the model's API ended, such as a 429, to 75)
  could not run: no evidence, not counted as a run, and `step`/`run` stop with exit 2 naming
  the tool's last log line. A check that could not run is not an `invalid` result; resume
  later on the same model.
- Observations come before writes (`effect: write` sorts last), then identifier order.
- `retry_when` {kind → [results]} on a writer: the writer is useful again while a record of one
  of those results exists about the current revision of what it wrote (not about `none`: nothing
  was written yet). It gets those records and their logs as `SDC_FEEDBACK`. Bounded by
  `max_runs` per arrival (3 on every LLM node; 4 on `spec.author`). Without it a real LLM's first invalid
  draft left the case `stuck`: a writer is not useful again until something changes `case.json`.
- `handoff` on a writer (`questions.answer`): when nothing is useful and that writer is
  admissible, the verdict is `awaiting_person` with the binding's text, not `stuck`. The person
  edits `work/` (the scope-changing answers) and runs again; the edit is a new revision.
- An action whose binding has `llm` is an agent node. Before it runs, `run --max-llm-calls N`
  counts the `action_started` events marked `llm` in the whole tree and stops at N
  (`llm_budget_spent`).

**Verdicts and exit codes**: an action was taken (0), `terminable` (0: a legitimate outcome
exists and nothing useful is left), `awaiting_person` (3: an admissible `manual` action is
useful, or a `handoff`), `awaiting_authority` (3), `writer_spent` (4), `stuck` (4), `step_budget_spent` (5,
`run --max-steps`, default 200, counted over the whole case tree), `llm_budget_spent` (5),
`wall_time_spent` (5, `run --max-wall-seconds`; the driver reads a monotonic clock only to stop,
never for a record). `run` reports the case directory that stopped, `llm_calls`, and on a wait
`message` and `next`: per decision the action, what is decided (subject, revision, path to read)
and the exact `sdc-drive approve` (or `run`) command; on a terminal the message also goes to
stderr.

**Lock.** Every command but `init` and `replay` takes `<tree root>/.sdc-drive.lock` (the pid,
created exclusively, removed on exit; a lock whose pid is gone is stale and taken over). A second
driver on the same tree exits 2 naming the pid. Beside it, `driver.json` names the holder (see
"Observation records").

**Tools for a chooser**: `sdc-drive tools` prints the admissible actions as `{name,
description, input_schema}`, with the input schema taken from the IR's `arguments`. That list
is for a person or an external selector. The driver itself never asks an LLM which action
comes next.

## Case store

```text
case/
  ir.json            canon compile output, fixed at open
  case.json          canon-case/1: current artifact revisions, case revision cN, independence, termination
  evidence/          one canon-evidence/1 per file, append-only, id e<seq>-<action>
  authority.json     canon-authority/1 (granted|denied per capability)
  decisions.json     canon-decisions/1 (explicit outcome decisions)
  bindings.json      action → tool (argv, args, produces, subject, upstream, producer, results,
                     manual, copy, child, max_runs), `$artifacts`, `$independence`; `{dir}`
                     resolved to the bindings file's directory at open
  evaluations/       every canon-decision/1, named by event seq
  blobs/<sha256>     tool stdout and stderr, content-addressed; never read back by the driver
  work/              the artifacts the tools act on (prototype; a managed worktree in the SDC);
                     in a child, a link to the parent's, so the whole tree shares the root's
  children/<id>/     child case stores, same layout (TEMPORARY, Loom)
  link.json          in a child: the parent (`../..`) and its `case_outcome` evidence file
  events.jsonl       hash-chained log: seq, prev = sha256(previous line), at, event, body
  observed.jsonl     wall time of each event (see "Observation records"); never replayed
  .sdc-drive.lock    at the tree root, while a driver runs: its pid
  driver.json        at the tree root, while a driver runs (see "Observation records")
```

- Events: `opened`, `evaluated`, `action_started`, `revised`, `action_finished` (with
  `principal` for a person's action), `discarded`, `child_opened`, `child_closed`,
  `authority_recorded` (with principal), `terminable`, `awaiting_person`, `awaiting_authority`,
  `writer_spent`, `writer_not_called`, `stuck`, `step_budget_spent`, `terminated`.
- Revisioned: each artifact revision is a content digest computed by the driver, and the case
  revision is a counter. Evidence and evaluations are append-only, except one rewrite per
  child: its `case_outcome` record goes from `open` to the child's outcome (Canon reads one
  record per child). The parent is evaluated right after that rewrite (`child_closed`, then
  `evaluated`), so a run that stops in a sibling still replays (`runs/trial-3-haiku-4`'s
  `IDEA-1.todo` failed replay without it).
- Replay: `sdc-drive replay` re-runs `canon evaluate` on every case of the tree with the `at`
  of its last `evaluated` event and compares the state and decision digests with the logged
  ones. Full-cycle run: all 8 cases match. Canon's guarantee: the same inputs give the same
  bytes (`AGENTS.md:22-26`).

## Observation records (2026-11-21)

Every event of one `run` carries that run's `--at`, by design, so replay is exact. For a live
view three records add wall time. Replay and replay-match never read them, and a missing one is
not an error.

- `observed.jsonl`, beside each case's `events.jsonl`: one line per appended event, written right
  after the event line (`event()` in `src/main.rs`):
  `{"format":"sdc.observed/1","index":<0-based line index in events.jsonl>,"event":"<name>","action":<action or null>,"observed_at":"<RFC 3339 UTC, ms>","pid":<driver pid>}`.
  A failed sidecar write does not stop the step.
- `driver.json`, beside `.sdc-drive.lock` at the tree root, while a driver holds the lock:
  `{"format":"sdc.driver/1","pid":…,"started_at":"<RFC 3339 UTC, ms>","argv":[…]}`. Removed on
  normal exit (before the lock); one left by a killed driver (pid not alive) is overwritten when
  the next driver takes the lock. `init` and `replay` take no lock and write none.
- `status.json` in each agent-node call record (`llm-node.sh`, not in stub mode, which makes no
  record) and each LLM decider call record (`decider.sh`): `{"format":"sdc.call-status/1",
  "state":"running","pid":…,"started_at":"…"}` once the record directory exists, rewritten on
  every exit with `"state":"finished"|"failed"`, `"finished_at"` and `"exit"` (llm-node: the
  node's exit; decider: the script's, else the adapter's). Written through a temp file and `mv` in
  the record directory. The call's `request.json` also names `case` (the case id) and `case_dir`
  (llm-node: `SDC_CASE`, `SDC_CASE_DIR`; decider: the request's `case_dir` and its `case.json`).

## Child cases (TEMPORARY: a copy of what Loom must provide, boundary.md row 27)

- A `case_open` action's binding names `child: {protocol, bindings, name | each, revisions}`.
  On exit 0 the driver opens one child per entry of the argument `each` names (`epics`,
  `stories`), else one child `name`, at `children/<parent id>.<name>/`, compiled from the child
  protocol, with the child bindings. Artifacts with a path in `$artifacts` take their revision
  from the shared `work/`; others take `revisions` (`epic: "{name}"`), else `none`.
- On opening, the parent gets one `case_outcome` record per child: `child_case`, `result: open`,
  `subject` = the `case_open` subject, `subject_revision` = the parent's revision of that subject
  at the moment the child was opened, `observed_at` = `--at`, `producer: sdc-drive`.
- Superseded convention, still what this driver does (coordinator, 2026-11-20). Rule from
  2026-11-21, given to Loom for its child-case runtime and to Canon's story child-outcome-revision:
  the terminal record's `subject_revision` is the revision the child's outcome was computed
  against. (a) The child protocol declares the subject artifact: the child's own revision of it at
  its terminating evaluation (sdc.spec authors the spec, so it ends at the new revision).
  (b) The child protocol does not declare it: the parent's revision at the moment the child was
  opened, because the child never saw a later one; if the parent's subject moved meanwhile, the
  outcome no longer counts and the parent opens a new child.
  Old text: a record keeps that opening revision for good.
  When the child terminates (`close`, or `run` closing a terminable child with exactly one
  legitimate outcome), only `result` and `observed_at` (= the explicit `--at`) change. After a
  revision of the subject the parent opens a NEW child with a new id (`<id>.2`, `<id>.3`, …);
  the old child's record is never reused or rewritten.
- Observed consequence for option A: the epic opens `sdc.spec` with subject `spec` at revision
  `none` (the spec does not exist until that child writes it), so its record says `none` while
  the epic's spec is `tf05b4d3…`. Under `revision: current` that child would no longer count and
  `sdc.epic` could never reach `passed`. Either `spec.open`'s subject becomes `epic` (as for
  `plan.open` and `release.open`) or option A must exempt it. Not changed here.
- `run` drives the tree depth first: every open child before the parent's next step; a child's
  `awaiting_*` or `stuck` stops the run and names that child's directory, where `approve` is run.
- `roadmap.read` (`copy: {kind, from}`, boundary.md row 28): copies the latest record of `kind`
  at its subject's current revision from the case of protocol `from` in the same tree. Not a
  literal copy: `upstream_revisions` entries for artifacts the receiving protocol does not
  declare are dropped, or Canon refuses the record (`undeclared-artifact`). In the run the
  direction's `roadmap_approval` lost `upstream_revisions: {vision: …}` in sdc, sdc.epic, sdc.spec
  and sdc.plan. Canon cannot tell this copy from a forgery (limitation 5).

**Evaluation instant.** Every command takes `--at`, and the driver reads no clock. Canon reads
none either: "Without an instant… nothing expires: Canon reads no clock"
(`evidence-and-revisions.md:101`). The caller passes UTC once per step, and that same instant
becomes the evidence's `observed_at`. So a tool's own timestamp is never used.

**Authority.**

- `sdc-drive approve --capability C --principal P [--deny] [--action A]` appends to
  `authority.json` (once: Canon refuses a capability decided twice) and logs the principal.
  `canon-authority/1` has no principal field (`documents.md:116-121`). With `--action`, it then
  takes `A` as `P`: the approval record (`context_approval`, `vision_approval`,
  `roadmap_approval`) is produced by `P` about the current revision.
- Outcomes that need an explicit decision go to `decisions.json` (`canon-decisions/1`, with
  principal and `case_revision`). The prototype has no command for this yet.
- **Open, a Canon request, not worked around**: a grant is not bound to a revision. In the run
  one `owner.approve` grant made `context.approve`, `vision.approve` and `roadmap.approve`
  admissible for every later revision; only the driver's `manual` rule kept each approval a
  separate step by a person. One `release.approve` grant also admits `release.rollback`
  (unbound in the example). A `change.merge` grant no longer admits merging a later change
  revision: `change.merge` needs a `merge_approval` record of the current change revision
  (docs/protocol-notes.md, "An approval is decided once per revision"). Fix belongs in Canon: `principal` and `case_revision` on `canon-authority/1`
  entries, as `canon-decisions/1` has (boundary.md top change 2).

**Stuck detection.**

- Nothing is useful, no outcome is legitimate, no writer is spent, and nothing is approval-required: `stuck`.
- `max_runs` per writer and arrival, once per snapshot per observation, and `--max-steps` per run bound every loop.
- Before a case opens, `canon check` reports outcomes no state reaches and evidence no action
  produces (`canon --help`). The SDC should run it on every protocol revision (*inferred*).
- A timeout per tool: the binding's `timeout_s` (default 1800 s), the whole process group is
  killed, exit `timeout`, which maps through `results` like any exit.

## LLM actions: output is only an artifact

- An LLM action binds like any tool (`argv` = a harness call, for example `metaharness run …`).
  Its binding has no `produces` (exception: `spec.author`'s `spec_source`, which records only
  which action wrote the revision). Its only effect is a new content digest of an `$artifacts`
  path. It cannot name a revision, write evidence or change the case.
- The next useful actions are observations by programs (`ess …`, `task check`, `aep plan
  artifact validate`). Their exit codes become evidence about the new revision. Revision binding
  sets aside every older record (seen: `revision_mismatch` on both records after `r1 → r2`).
- A review by an LLM can produce `spec_review`. Its `producer` must then be a principal that the
  case's `independence` lists. Canon drops records from other producers as `not_independent`.

What the driver must check before a tool result counts as evidence:

| Check | Done in the prototype |
|---|---|
| The action was admissible at the step's own evaluation | yes |
| The arguments fit `arguments` (top-level `required`, `additionalProperties: false`) | yes; full dialect: gap |
| The produced kind is in the action's `may_produce` | yes |
| `observed_at` = `--at`, never a time the tool reports (intake G9: the interpreted report says 2023-11-14) | yes |
| `subject_revision` = the case's current revision, computed by the driver | yes |
| An observing action left every `$artifacts` path unchanged, otherwise discard | yes (event `discarded`; 0 in the full-cycle run). Files outside artifact paths (`.sdc/`, AEP's lock file) are not checked |
| A report's `provenance.spec_digest` equals the current spec revision, otherwise discard | no, gap |
| A report's own timestamp, if read at all, is within a window of `--at`, otherwise discard | no, gap |
| A result read from a report field (for example `conformance_status`) is in the binding's allowed set | no; exit codes only |
| AEP lifecycle state is never evidence: `conforming` was reached on an interpreted self-check (intake G8) | by design: AEP moves are effects |
| Implementation evidence names a clean commit (`git status --porcelain` empty), revision = `HEAD` sha | no, gap |

## LLM executor (2026-11-21)

Every agent node binds `argv: ["{dir}/tools/llm-node.sh", "<action>"]` plus an `llm` object:
`model` (`claude-haiku-5-5`), `max_turns`, `max_budget_usd`, optional `operations`, and `stub`
(the `sdc-tool.sh` subcommand that writes the fixed draft). Agent nodes: `idea.dissect`,
`questions.write`, `questions.answer`, `requirements.normalise` (intake); `context.default`,
`vision.write`, `roadmap.write` (direction); `spec.author`; `plan.decompose`;
`change.implement` (story). `change.review` is a model call too (see "The code review" below); `spec.review` stays a program stub.

What one call does (`llm-node.sh`):

1. Copies the node's declared inputs from `work/` into `<record>/cwd/in/` (an output edited in
   place, such as `questions.yaml`, `spec/`, `impl/`, is also seeded into `out/`), plus
   `in/feedback.md` on a retry (the failing checks and the last 60 lines of each log).
2. Writes `prompt.md` from those inputs only, and a `request.json`.
3. Calls the adapter, which runs the session in `cwd/` with only the admitted operations
   (`file.read`, `file.write`, `file.edit`, `dir.list`, `search`; no shell, no web).
4. Copies back only the declared output: `out/<artifact>` for files and trees; for the AEP nodes
   (`vision.write`, `roadmap.write`, `plan.decompose`) the LLM writes a JSON proposal and the
   script makes the `aep plan artifact new|scope|move` calls. Whatever else the session wrote stays
   in the record.
5. Appends one line to `SDC_LLM_LOG`: action, case, attempt, retry, adapter, model, allowed
   operations, offered tools, every tool call (name, operations, input, decision), exit, tokens
   (`usage`), cost, turns, duration, record directory.

The driver then sees a new revision; only the bound programs produce evidence about it.

**Model per node** (2026-11-21, `story:model-per-node-variable`). The model is a run setting, by
node class (`tools/node-classes.json`): `SDC_LLM_MODEL_WRITER` (`spec.author`, `plan.decompose`),
`SDC_LLM_MODEL_IMPLEMENT` (`change.implement`), `SDC_LLM_MODEL_REVIEW` (`change.review`;
`spec.review` is still a program stub), `SDC_LLM_MODEL_OTHER` (every other
node), else `SDC_LLM_MODEL` for every node, else the binding's `llm.model`. The decider:
`SDC_LLM_MODEL_DECIDER`, else `SDC_DECIDER_MODEL`, else `claude-haiku-5-5` (`SDC_LLM_MODEL` does
not reach it). Each call line names its `model` and `class`; `run-settings.jsonl` records
`models` per class (`model`, `source`, and per node).

**Reasoning effort per node** (2026-11-21). The same way as the model: `SDC_LLM_EFFORT_WRITER`,
`_IMPLEMENT`, `_REVIEW`, `_DECIDER`, `_OTHER`, else `SDC_LLM_EFFORT` for every node and the decider
(per class wins), else the class default in `tools/node-classes.json` (`default_efforts`): `high`
for writer, implement, review and other, `xhigh` for the decider and the requester (which takes
`SDC_LLM_EFFORT_REQUESTER`, then `_DECIDER`, first). A level is always passed, so a run is the same
on any host and no adapter falls back to its own default (the `high` the `metaharness` adapter
used, the `xhigh` the decider's `claude -p` took from this host's `~/.claude/settings.json`).
Accepted: `low`, `medium`, `high`, `xhigh`, `max` (`claude --help`; list in
`tools/node-classes.json`). `scripts/run-trial.sh` refuses any other value with exit 64 and the
list, because both CLIs ignore an unknown one with a warning and run at their own default. The
value reaches `request.json` (`effort`) and each adapter as `--effort <level>`
(`tools/llm-adapter-metaharness.sh:38` to `metaharness run claude --effort`, which passes it to
Claude Code's `--effort`; `tools/llm-adapter-claude-cli.sh:35` to `claude -p --effort`). Each call
line, decider record and adapter response names its `effort`; `run-settings.jsonl` records
`efforts` per class and for the requester (`effort`, `source`: the variable, or `default`). A model
may silently lower a level it does not offer.

**Hermetic `claude-cli` adapter** (ruling D153: a decision does not depend on the machine's personal
settings). A recorded decider call of the old adapter (`claude -p` with no `--setting-sources`,
`trial-3-strong-2/decider/001-q1/events.jsonl`) shows the host's setup in its `init` event: 6
`SessionStart` hooks fired, 60 skills, 11 plugins (7 the user installed, 4 built in),
26 agents, output style `Operator Report`, 14 tools
(`SendMessage`, `PushNotification`, `ToolSearch` ...), and the user's global instructions load from
`~/.claude/CLAUDE.md` (*inferred* from that: the transcript does not print the system prompt). The
adapter now runs as the `metaharness` adapter does (`../metaharness/crates/metaharness-claude/src/launch.rs:1353`):
`--setting-sources ""`, a fresh `CLAUDE_CONFIG_DIR` per call holding only a copy of the login file
`.credentials.json` (mode 600, removed on exit; or `ANTHROPIC_API_KEY`), `--strict-mcp-config`,
`--disable-slash-commands`, `--tools` limited to the request's operations, auto-memory and the git
status block off, `CLAUDE_CODE_EFFORT_LEVEL` unset (`tools/llm-adapter-claude-cli.sh:1-14`). A call
with the new adapter lists tools `Glob`, `Read`, `StructuredOutput`, no hook, no skill, output style
`default`, only the built-in plugins and agents, and authenticates (probe, 2 Haiku calls). The
login copy shares the host's refresh token, as the `metaharness` adapter's does.

**Adapters** (`SDC_LLM_ADAPTER`; contract in `tools/llm-adapter-metaharness.sh`: one argument,
`request.json` {model, prompt_file, cwd, operations, max_turns, max_budget_usd, record_dir, node,
attempt}; one response JSON on stdout; the session's exit):

| Value | What runs |
|---|---|
| `stub` (default) | the binding's `llm.stub`, no LLM |
| `dry` | builds the metaharness frame and argv, appends `would run: metaharness …` to `SDC_LLM_DRY_LOG`, runs nothing, then the stub |
| `metaharness` | `metaharness run claude --frame <sealed metaharness.frame/1> --decisions frame --cwd <cwd> --scratch-cwd --model M --max-turns N --max-budget-usd B --retain-dir <record>/transcripts -p <prompt>`: Claude Code with the operator's login, its tool calls decided per call against the frame's operations. Record: `frame.json`, `argv.json`, `events.jsonl` (`metaharness.event/1`), `transcripts/` |
| `claude-cli` | `claude -p … --tools <operations> --allowedTools <operations> --disallowedTools Bash,WebFetch,WebSearch,Task,Skill --setting-sources "" --strict-mcp-config --permission-mode acceptEdits --output-format stream-json` in a fresh config home, hermetic (below), the fallback |
| a path | any program with the same contract |
| `driver/target/debug/sdc-replay` (a path, set by `run-trial.sh --recorded`) | a recorded run's reply at this place, for work nodes and the decider ("Recorded replay" below) |
| `tools/llm-adapter-fixture.sh` (a path) | tests, no LLM: writes the node's `stub-file` output into `out/`, so the apply step runs as on a real reply. With `SDC_LLM_NODES="<node> …"` only the named nodes go to the adapter, the rest run their stub (`scope-changed` cycle). `SDC_FIXTURE_STUB='{"<node>": [<fixture>, <artifact>]}'` gives a node without a `stub-file` stub one (`plan-in-project` cycle). `llm-node.sh` and `sdc-tool.sh` call `aep … --root .`: the case store keeps aep's own lifecycles even inside a project with another profile |

metaharness 0.30.0 can do one bounded non-interactive call: `run claude -p` with `--max-turns`,
`--max-budget-usd` and a frame. Checked without spending: against a dead `--model-endpoint` the
sealed frame was accepted (`session.started`) and the run only failed at the API. It warns
`version_outside_pin`: Claude Code 2.1.330 is not one of its pins (2.1.304, 2.1.305, 2.1.309);
`metaharness doctor claude` says every flag it sends is declared by the binary.

**Decider** (2026-11-21, `scripts/run-trial.sh`, `tools/decider.sh`; usage in `trial-1.md`,
KPIs H1-H4 in `kpis.md` § 1d). The person's decisions (approvals, scope-changing questions, the
root's close) go to `SDC_DECIDER`: `person` (default), the rules `approve` and `reject`, or `haiku`
(one bounded call through the adapters above, `SDC_DECIDER_ADAPTER` else `SDC_LLM_ADAPTER`; the LLM
writes `out/decision.json` {decision, answer, reasons, cites, best}, a jq program validates it,
invalid counts as escalate). The script, not the driver, applies a decision: `sdc-drive approve
--principal decider:<kind>` (grant and approval record carry that principal) or `answered_by:
decider:<kind>` in `questions.yaml`; `decisions.jsonl` keeps every decision with its reasons and
its `class`. No driver change. `scope.answer` is put to the decider one open scope-changing
question at a time (class `scope_changing_answer`), then taken as `decider:<kind>`; a finding
waiting at `amend.upstream` is put to it as a finding request (class `finding`) and its answer
written as `decision:` with `decided_by: decider:<kind>`, which `sdc-findings amend` checks as it
checks the agent's amendments (`docs/trial-2.md`, "Limits"). `check-questions` takes any named
principal for a scope-changing answer (`sdc.intake` r5).

**What comes after the decision** (2026-11-21, `runs/trial-3-mixed-3`: the decider refused
`release.publish` because `health.required` was open, though only `release.observe`, admissible
after the publish, can meet it). `sdc-reply later --ir <ir.json> --action <a>` derives from the
compiled protocol the actions that need a claim only `a` (or such actions) can make true, the
claims only those actions make true, and the obligations only those claims discharge. `decider.sh`
leaves them out of `claims` and `open_obligations` in `in/checks.json` and lists them under
`met_after_this_action` ("met after <a>, not a condition of this decision"); the prompt says so.
Moved per approval action of the eight protocols: `release.publish` health.required
(release.healthy, release.observed, rollback.complete); `context.approve` vision.required,
roadmap.required; `vision.approve` roadmap.required; `merge.approve` no obligation (claim
change.merged); none for `roadmap.approve`, `scope.answer`, `release.rollback`.

**What the decider reads** (2026-11-21, `runs/trial-3-haiku-5`'s close: rejected because the
roadmap's aep front matter said `status: draft` beside `roadmap.approved` true, and because eight
decisions were "answered by decider:haiku, not a person"). Tree inputs keep `Cargo.lock` (the copy
dropped it, and the decider doubted the `--locked` build the checks had recorded). A `close` reads
`published/README.md` and no `aep/` file: it judges the result, and the roadmap's approval is a claim
in `in/checks.json`. A prompt with `aep/` inputs says that `status` and `revision` in their front
matter belong to the planning tool and say nothing about approval. Under `SDC_DECIDER_ESCALATE=final`
the prompt says that a decider takes every decision a person would take, so an answer by
`decider:<kind>` is the product owner's own and never a reason to reject or escalate; and in place
of "escalate to a person" it says that no person can be asked: write `escalate`, say in `reasons`
what cannot be told, and give `best`. Cycle `decider-input` checks the records of the `autopilot`
cycle.

**Decider replies** (2026-11-21, `driver/src/bin/sdc-reply.rs`). `sdc-reply citable` lists what a
decider call may cite, in the citation forms of `decisions.md`: ids (requirement, dissection,
question and finding ids, `context.<key>`, the claims in `in/checks.json`), artifact revisions with
an entry (`context@<rev>#platform.value`; YAML and JSON artifacts in `in/`) and lines of a file
(`context.yaml:13-14`). The prompt lists each form with one example from the call's input. `sdc-reply
resolve` checks every cite (an artifact revision must be the current one, an entry and a line must
exist); the check refuses only what does not resolve, and a valid reply records each cite's form in
`citations`. `sdc-reply schema` builds the reply's JSON schema per request (`decision` and `answer`
as enums where the request fixes them, `cites` as an enum of the citable set); `reply_schema` and
`citable_set` are the parts other LLM nodes can reuse. The claude-cli adapter passes it as `claude -p
--json-schema` (the CLI validates the reply and asks again on a mismatch); `metaharness run claude`
has no response-schema option, so through it the reply is not constrained. Each record says which
(`schema_constrained`, `schema_note`). An invalid reply is asked again with the check's reason, at
most 2 retries, each a decider call counted against `SDC_MAX_DECIDER_CALLS` (`invalid_reply` lines in
`decisions.jsonl`). Still invalid under `SDC_DECIDER_ESCALATE=final`: a question or a finding takes
its default (`fallback: invalid_reply_default`); an approval is approved when every claim of its
precondition is true (`approved_on_checks`), a close when its outcome's claims are true and no
obligation is open, else rejected with the failing checks (`rejected_on_checks`). `--autopilot`
exports `SDC_DECIDER_ESCALATE=final`, so the decider is asked for `best` and an escalate without
it is invalid.

**The requester agent** (2026-11-21, `docs/requester.md` § 3, story:requester-decider). Each
decision class has a `side` in `driver/examples/sdc/decision-classes.json`
(`sdc.decision-classes/1`, the classes of `autonomy-levels.md` § 3.2 with today's action names).
With `SDC_REQUESTER_DECIDER=haiku`, `run-trial.sh` sends the requester-side classes
(scope-changing answers, context, vision, roadmap, release, close) to `decider.sh requester`,
principal `decider:requester`; the delivering side (merge, findings, …) keeps `SDC_DECIDER`. Its
`in/` holds exactly `request.json` (`sdc-decision-request` for a question; built from the stop for
an approval or a close, options A approve and B reject), the subject file, the files the
request's grounds cite, `idea.yaml`, `dissection.yaml`, `goals.yaml` and `decisions.json` (its own
earlier decisions of the run). It writes `out/reply.json` (`option`, `answer`, `reasons`,
`grounds`, `strength`, `no_ground_in_goals`); the grounds go through `tools/decision-grounds.sh`,
and a refused or malformed reply is asked again like the decider's. `no_ground_in_goals` is an
escalate: under D→P (A3, A4) the run waits on the requester person, under D* (A5) the question's
default is taken and the decision line says `no_ground_in_goals: true`. decisions.jsonl records
each decision's `side`, `strength` and grounds; run-settings.jsonl records `requester_decider`.
The fixture adapter answers node `requester.<question or action>` from `SDC_REQUESTER_REPLIES`
(cycles `requester-decider`, `requester-no-ground`). Findings and code review are not requests in
this pipeline: a finding stays with `SDC_DECIDER`, code review is the reviewer node.

**The code review** (2026-11-21, `story:review-node-is-a-model`). `change.review` is an LLM call
of class REVIEW (`SDC_LLM_MODEL_REVIEW`) by `llm-node.sh change.review`, producer `llm-review`,
which the story bindings list as independent (`different_principal`): the implementer is
`llm-node`. A fresh session with no implementer context and read-only operations; the model
replies only through the response schema, never by writing a file. Its adapter is the review
class's own (`sdc-reply adapters`): `SDC_REVIEW_ADAPTER`, else `claude-cli` when `SDC_LLM_ADAPTER`
is `metaharness` (which has no response-schema option), else `SDC_LLM_ADAPTER`. `run-trial.sh`
checks at start that every class in `schema_reply` (`tools/node-classes.json`) is on an adapter
that takes a schema, else refuses with exit 64 naming class and adapter, and records `adapters` in
`run-settings.jsonl`. A review that stays invalid after its retries stops an autopilot run with
cause `review_invalid`, naming case and call, not `stuck` (2026-11-21: `runs/trial-3-strong-3` and
`-4` stopped `stuck` because on metaharness the review could only reply by file). An observation
runs once per case snapshot; `run-trial.sh --reopen` marks a reopen in every case (`sdc-drive
reopen`, event `reopened`), and a run before it that recorded nothing (an invalid review, a writer
that was not called) no longer holds the snapshot, so it runs once more on the unchanged state;
invalid again, the stop names the new call. A run that recorded evidence still holds it. Its inputs are
built by `sdc-reply review-inputs`: the story, its acceptance lines (the summary and an
`## Acceptance` section), the requirements it delivers (`spec/traces.json`: a covered command, or a
view or actor of a covered command's domain; without a trace, `spec_domains`), `in/uncovered.json`
(requirements no story covers: findings of kind `outside`, never a reason to reject), the covered
commands' IR entries, the change tree, its diff against `mainline/` (empty for the first story),
the gate log, and the log of the conformance run that passed on the revision under review (sdc.story
revision 7 admits `change.review` only after `tests.pass`; without that log the node records
nothing). The reply is constrained by
`sdc-reply schema --type review` (claude-cli `--json-schema`): `decision` approved or rejected,
`findings` each `{kind, at: impl/<path>:<line>, violates, says}`, `summary`. `sdc-reply
check-review` checks it: every `at` a line of the change tree, every `violates` in the story's set
(or, for `outside`, in the uncovered set), a rejection with at least one violation. An invalid
reply is asked again with the reason (`SDC_REVIEW_RETRIES`, default 2); still invalid, or a failed
call: exit 2, nothing recorded, so the case is stuck on that revision; never an approval. A valid
rejection exits 1 with only its `sdc.rejection/1` on stdout (approval `change.review`, principal
`llm-review:<model>`, reasons, citations with `in_diff`, count); that log reaches `change.implement`
through `retry_when: {code_review: [rejected]}` as `in/rejection.json`. The bound is
`change.implement`'s `max_runs` 3 per arrival, shared with failed conformance runs: the third
rejection ends `writer_spent` with `sent_back` naming `change.review`. Cycles `review-reject-once`,
`review-invalid`, `review-spent` (fixture adapter, `SDC_REVIEW_REPLIES`).

**A rejected approval** (2026-11-21, `autonomy-levels.md` § 3.4). Under autopilot `run-trial.sh`
writes the rejection beside the decider record (`sdc.rejection/1`: approval, case, subject,
revision, principal, reasons, citations, count) and records it: `sdc-drive approve … --action
<approval> --result rejected --reasons <file>` writes the plain `canon-evidence/1` record with
result `rejected` (refused unless the action's binding is a manual approval whose `results` name
`rejected`) and keeps the file as the step's log blob. A writer whose binding names the approval's
kind in `retry_when` (`"retry_when": {"merge_approval": ["rejected"]}`) is then useful while the
rejected record is about the current revision; the driver passes the record in SDC_FEEDBACK, and
`llm-node.sh` gives the writer the blob as `in/rejection.json` with a prompt to address each reason
and cite it back. A model writer writes the artifact itself; only the stub, which writes the same
output every time, appends the rejection to the subject (`.rework` in a tree, a comment in a YAML or
Markdown file) so its rework is a new revision (cycle `decider-rework-model`: none on a model
adapter). The new revision leaves the record behind and the approval is asked again; the
decider sees the earlier rejections in `prior_rejections` (`sdc-reply rejections`, derived from the
evidence and the event log). The third rejection stops the run with cause `rework_spent`; a
rejection that cannot be recorded, or that sends no writer back, and has no end of its own (below)
with `rejected_without_rework_path` (exit 6). An open case keeps the `ir.json` and `bindings.json`
it was opened with, so a run started before a protocol change does not gain the new actions.

**A refused release** (2026-11-21, `docs/protocol-notes.md`). `release.publish` is approved by the
capability `release.approve`: no `rejected` result, no writer. The release's other end is
`withheld`, which requires the decision `explicitly_withheld`; Canon admits that decision from any
principal in any state, so the tools hold the rule. Under autopilot only, directly after the
decider rejected `release.publish`, `run-trial.sh` records the refusal as the protocol represents
it, `approve --capability release.approve --deny` by the decider, then `sdc-drive decide
--decision explicitly_withheld --outcome withheld --principal rule:autopilot --after-denial
release.approve --reasons <rejection>`, which appends the `canon-decisions/1` entry (decision,
outcome, principal, case revision) with the rejection (the decider's reasons and citations) as its
log blob, and refuses before writing unless the outcome requires that decision alone and the
case's last step was that denial. The release is closed `withheld`; the driver's round closes the
epic `not_released`, and the root, terminable `not_delivered`, is closed without a decider: the
decider closes only outcomes that require a decision, and `operating` below A5. The run is
closed, not stopped: exit 0, `closed IDEA-1: "not_delivered" (release withheld: <first reason>)
by rule:autopilot`, and `status.json` (`sdc.run-status/1`: state, outcome, `delivered`, note,
`closed_by`), no stop record. Under A0–A4 the reject stops the run as before and nothing is
recorded in the release case.

**The close in autopilot** (D140, 2026-11-21, `runs/trial-3-haiku-5`: the decider rejected the
close of a terminable root, and the run stopped `rejected_without_rework_path`). Under A5 the
close of the root is not a decision: `operating` requires the claims `roadmap.approved`,
`epics.opened` and `children.passed` (`protocol/sdc.protocol.yaml`, `outcomes`), which the
approvals upstream already settled, and binds no capability. `run-trial.sh` closes a terminable
root through any outcome that requires no decision as `rule:autopilot` (`sdc-drive close
--principal rule:autopilot`, recorded on the `terminated` event and as `closed_by` in
`status.json`) and asks the decider nothing. Children were never asked: the driver closes a
terminable child with one legitimate outcome in its round. Below A5 the decider still closes
`operating` (class close, `autonomy-levels.md` § 3.2).

**Keep-going** (D142, 2026-11-21, trial and eval runs only: `scripts/run-trial.sh <run>
--autopilot --keep-going`; refused without `--autopilot`). Where autopilot stops with a cause,
keep-going first writes a line to `<run>/defects.jsonl` (`sdc.run-defect/1`: at, case, action,
cause, the full reason, the decision or reply that led to it, the case's last event seq, LLM
calls so far, `forced`, `principal`, `continuation`), then takes a forced continuation under the
principal `rule:keep-going`, never a decider's or a person's:

| Stop cause | Continuation |
|---|---|
| `rejected_without_rework_path` (an approval or close; reached also by an invalid reply under final, `rejected_on_checks`) | `rule:keep-going` approves (closes); a question or a finding takes its default; none: stops |
| `rework_spent` | `rule:keep-going` approves |
| `decision_not_takeable`, a refused decider decision on a finding | the finding's default, `decided_by: rule:keep-going`, applied by `amend.upstream` through the same check; refused again: stops |
| `decision_not_takeable`, a handoff with nothing for the decider | stops |
| `writer_spent`, `writer_not_called` | `sdc-drive allow --action <writer> --principal rule:keep-going` (event `allowance_granted`): one more `max_runs`, the failing check records fed back as on any retry; spent again: stops |
| `decider_budget_spent`, `llm_budget_spent`, `step_budget_spent`, `wall_time_spent`, `stuck`, an approval the driver refuses, "pending again", a tool that crashes | stops |

At most 12 forced continuations per run, then the cause `keep_going_spent`; the same cause at the
same case and action is forced once, and a second time stops. Every stop also gets a line
(`forced: false`, `continuation: stops`). `status.json` carries `forced: N` and the closed line
ends `(forced N)`; `sdc-finding-kpis` (`forced`) and `sdc-run-compare` ("forced continuations",
"defects") print the count and the defects. An invalid decider reply after its retries does not
stop under final (`approved_on_checks`, `rejected_on_checks`, or the default), so it needs no
continuation of its own. Without `--keep-going` nothing changes.

**Unstick** (D147, 2026-11-21, every `--autopilot` run; `SDC_UNSTICK=off` turns it off, and it is
off where its adapter calls no model, `stub` or `dry`). Before autopilot stops for any cause but a
cap (`decider_budget_spent`, `llm_budget_spent`, `step_budget_spent`, `wall_time_spent`) or a
tool that crashed, the node `run.unstick` (class `unstick` in `tools/node-classes.json`, model
`SDC_LLM_MODEL_UNSTICK`, default `claude-opus-5-5`; adapter `SDC_UNSTICK_ADAPTER`, else the
decider's) is asked first. A program, `sdc-unstick moves`, lists the legal moves of the stopped
state; the model picks one id and never invents state:

| Move | Offered when | Carried out (principal `agent:unstick`) |
|---|---|---|
| `rerun:<W>` | W is the spent or not-called writer of the stop or of the position | `sdc-drive allow --action W`: one more `max_runs`, the failing check records fed back as on any retry |
| `reask:<D>` | a question or finding request; an approval or close with no recorded rejection | the decider asked D again with the diagnosis as `note` in its `in/decision.json` (a refused finding decision is cleared first) |
| `default:<F>` | finding or question F has a default | the default taken, `decided_by`/`answered_by: agent:unstick` |
| `approve:<A>` | an approval or close with no recorded rejection | approved (closed) as `agent:unstick` |
| `end:<O>` | an approval whose case has an outcome O that requires one decision alone (`dropped`, `withheld`, `abandoned`) | the refusal recorded (`approve --deny` when the capability was never decided, else the decider's rejection recorded), `sdc-drive decide --after-denial <capability>` directly after it, the case closed O; its parents end through what their evaluation then offers (a dropped story: the root `not_delivered`) |
| `stop` | always | the run stops as it would have |

The node's input (`<run>/unstick/NNN-<cause>/cwd/in/stop.json`): the cause and reason, the stopped
case and action, the decider request that led to it, the claims, the blocked actions with their
unmet preconditions, the open obligations, the case's last 20 events, the last failing action's
log, and the moves. The reply (`out/unstick.json`, schema `reply-schema.json`, passed to an
adapter that constrains replies): `move` (an id), `diagnosis` (two or three sentences: which
route, check or input is missing or wrong), `defect_class` (`missing_route`, `cap_too_low`,
`writer_error`, `decider_error`, `contradiction_in_inputs`, `tool_defect`, `other`), `reasons`.
`sdc-unstick validate` checks it; an invalid reply is asked again with the check's reason, at
most 2 retries, then no move is taken: the keep-going table under `--keep-going`, else the stop.
Each use is one line in `defects.jsonl` (`unstick: true`, `unstuck`, `move`, `diagnosis`,
`defect_class`, `reasons`, `attempts`, `record`; `principal: agent:unstick` for a move taken). At
most 6 moves per run, and the same cause at the same case and action once; then the run stops
with the cause `unstick_spent` (keep-going takes no continuation after it). A third rejection is
not recorded in the case while run.unstick or keep-going may still take the approval; a run that
stops records it before the stop, as without them. `status.json` carries `unstuck: N`, the
closed line ends `(unstuck N)` (beside `forced N`), `run-settings.jsonl` `unstick: {on, adapter,
model}`; `sdc-finding-kpis` (`unstuck`, with each diagnosis) and `sdc-run-compare` ("unstuck",
"defects") print them. A run with `unstuck` above 0 closed unattended, not clean: each diagnosis is
a defect to fix. `sdc-drive decide --after-denial <capability>` admits the decision also directly
after a recorded `rejected` result of an action that requires the capability: once `merge.approve`
has recorded a rejection, `change.merge` is granted and can no longer be denied.

**Stopping a run** (2026-11-21). `scripts/run-trial.sh <run> --stop "<reason>"` writes
`runs/<run>/run-stopped.json`, `{"format": "sdc.run-stopped/1", "at": "<RFC 3339 UTC>", "by":
"<principal>", "reason": "…", "verdict_at_stop": "<last round's verdict>", "case": "<its case id
or null>"}` (`by` from `SDC_PRINCIPAL`, required), and a `# stopped at … by …: …` line in
`run.log`; the driver and the event logs are not touched. A resume of a stopped run is refused
(exit 8) until `--reopen`, which renames the record to `run-stopped.<at>.json`, logs it and goes
on. `sdc-finding-kpis` reports the time the run waited on a person as L7 (`kpis.md` § 4), with the
stops that caused it, up to the stop record's instant; with a decider set (A3 and up) a value
above 0 is printed as `FAILED`. Under `--autopilot` a stop the decider cannot take is written by
`rule:autopilot` with a `cause` (`decision_not_takeable`, `rejected_without_rework_path`, `rework_spent`,
`decider_budget_spent`, `writer_spent`, `stuck`, …) and a reason that names the action, the cause,
the case and the fix it waits on; the interval before a `rule:` stop is not counted in L7.

**Where another model source plugs in** (not built):

- A gateway: `SDC_LLM_ENDPOINT=<root URL>` makes the metaharness adapter add `--model-endpoint
  <root> --credentials none`; Claude Code then speaks Anthropic messages at `<root>/v1/messages`.
  That fits `llmgw` (private; its harness configuration is one URL and one token) and
  `llm-gateway` (relays messages requests, README; 0.18.0, "nothing is deployed or qualified").
- A plain model call without an agent loop (`llm`'s `Model::turn`, `inference`), or Bedrock
  through Substrate: a new adapter program set as `SDC_LLM_ADAPTER=<path>`, same request and
  response. Such an adapter needs its own file tool loop or a single-shot output format, since
  `llm` "never runs a tool" (README).

## Recorded replay (2026-11-21, `story:recorded-run-replay`)

The operator, 2026-11-21: "If the LLM nodes are successful for a certain eval, we could also just
record it and then replay it, very quickly and fast, and simulate: given the LLM would respond in
that way at that place, change conditions, timeouts, retries, stuff like that, and see how the
system behaves and whether it would produce better metrics or not."

`scripts/run-trial.sh <new run> --autopilot --recorded <source>` drives a fresh case tree with the
real driver and the real programs. At every model call the reply comes from what the source
recorded at that place. The source is a run directory (`runs/<name>`) or a recording made by
`sdc-replay pack` (`driver/examples/sdc/fixtures/recorded/stub/`). `sdc-replay prepare` writes
`<run>/replay/plan.json` and names the run's environment: `driver/src/bin/sdc-replay.rs` is
`SDC_LLM_ADAPTER` and `SDC_DECIDER_ADAPTER` (an adapter program, one argument `request.json`, like
any other), and the run takes the source's decider kind, per-class models, caps, `SDC_LLM_NODES`,
the principal its aep store names, the idea and the goals. A variable the caller sets wins. The
first line of the console names the source's tree and the re-driving tree (`run-settings.jsonl`
now records `tree: {commit, dirty}`). If they differ, it says that code changes alone can make a
call diverge. `sdc-replay` is not `sdc-drive replay`: that re-evaluates a finished store against
its log, this re-runs the run.

**What is real and what is recorded.** Real: the driver, Canon, every bound program (the gates,
`tests.run` with its cargo build and tests, the checks, `aep`, `sdc-findings`, `sdc-reply`, the
decider's validation), `llm-node.sh`'s input building and apply step. Recorded: the model's reply
(the call's `out/` tree, the decider's and the review's structured output) and its duration,
cost, tokens and turns. `--take-recorded <action,…|all>` also takes a program's exit and log from
the source (the n-th run of the action in the case) instead of running it. It reproduces no file
the program wrote, so an action whose output files later steps read (`spec.gate`'s
`.sdc/gate/ir.json`) must run for real. It needs a run directory: a recording holds no logs.

**The key.** A work call is keyed `case/node@arrival.try[.attempt]`. The arrival is counted as
the driver counts it for `max_runs`: `revised` events no agent action of the case wrote (the case
opening, a write from another case, a person's edit). The try is the node's runs since that
arrival plus one, counted as `max_runs` counts them: no program error, no `not_called`. The
attempt is `change.review`'s `review_attempt`. A decider call is keyed `case/decision@asking.try`:
the decision is the question id, finding id, action or `close`, the asking is the n-th time it is
asked in the case, the try is the decider's attempt. All of these come from the records:
`request.json` (`case`, `node`, `review_attempt`), the case's `events.jsonl` (for a source call,
the k-th record of a node in a case is its k-th run that reached the adapter), and
`decide-request.json`. A retry the source never made, or a re-ask after a rework, gets a key of
its own instead of a neighbour's reply. An injection counts `#n` as the n-th call of a name in its
case (with `<case>/`) or in the run.

**The input check.** Each call keeps one sha256 per file of its `in/` and the request without its
paths (`prompt_file`, `cwd`, `record_dir`, `case_dir`, `json_schema`) and ordinals (`attempt`,
`review_attempt`). Before hashing, the run directories become `<run>`, home paths `<path>`, RFC 3339
and `diff` instants `<instant>`, `t<16 hex>` revisions `<rev>` (aep `move` writes the wall time into
the stories' `transitions`, so the aep tree's revision differs in every run), and seconds `<s>`.
Test harness lines (`test x ... ok`) are sorted in place, because threads finish in any order.
A call whose key exists but whose digests differ is a divergence that names each file and
request field that differs. A changed prompt is noted (`prompt_changed`, counted in the status),
not a mismatch: prompts change with code, and an experiment that changed a prompt must not pass for
one that changed a condition. In a recorded decider reply, each revision the source was shown
(`citable_artifacts`, the requester's subject) is replaced by the current revision of the same
artifact (`revisions_mapped` in the call record).

**Divergence** (`--on-divergence`):

| Policy | When the source has no call at the key, or the digests differ |
|---|---|
| `stop` (default) | `replay/diverged.json` (`sdc.replay-divergence/1`: place, difference, summary); a work node exits 75 (`terminal_reason: recording_diverged`), so the driver records nothing and stops. The decider gets no reply. The run ends with exit 9 and, under autopilot, a stop record by `rule:autopilot` with cause `recording_diverged` naming the place and the difference |
| `nearest` | the recorded reply of the same node or decision in the same case: same arrival at the nearest try (an earlier one first), else the latest earlier arrival; marked `reused`. Nothing to reuse: as `stop` |
| `live` | the real adapter for this one call (`--live-adapter`, default `metaharness`; `dry` for tests), marked `live`, counted and capped by `--live-cap` (default 5); a spent cap ends the run as `stop` |

Every call record has `replay.json` (`sdc.replay-call/1`: key, mode `recorded`, `reused`, `live`
or `diverged`, source record, differences, fault, virtual milliseconds, `prompt_changed`). The
response's `adapter` is the mode (`source_adapter` keeps the recorded one). At the end
`sdc-replay seal` writes `replay/summary.json` (`sdc.replay-status/1`) and copies it as `replay`
into `status.json` and `run-stopped.json`: label (`re-driven: recorded replies only`, `… with reused
replies`, `… with live model calls`), counts (`recorded`, `reused`, `live`, `diverged`, `paid`,
`faults`, `prompt_changed`), program runs (real, recorded, check failures), both trees, policy,
injections, conditions, virtual seconds. `sdc-replay compare <run>` lists each case's actions
(action, exit, result) and the end against the source, exit 1 when they differ.

**Clock.** The model's share of the time is virtual: a recorded call takes no time, and its
recorded `duration_ms` stays in `duration_ms` of the response, `llm-calls.jsonl` and
`decisions.jsonl` (each line gets `replay: {mode, key, clock: "virtual", virtual_ms, …}`).
`rounds.jsonl` gets `real_seconds`, `virtual_seconds` (the recorded work calls of the round; decider calls fall between rounds, as in a real run),
`seconds` = both and `clock: "virtual"`; the rounds' epochs stay wall time. `cost_usd` stays the
recorded cost, what the call would have cost; `calls.paid` counts the live calls that reached a
model (0 for `dry`). A work call whose recorded duration reaches the action's `timeout_s` times out
on the virtual clock. The driver passes `SDC_TIMEOUT_S` and `SDC_VIRTUAL_TIMEOUT`; the adapter writes
nothing and puts the reason in that file, and the driver records exit `timeout`, "killed after N s
(virtual clock: …)", as for a real kill.

**Changed conditions** (`--set <k>=<v>`, recorded in `run-settings.jsonl` `replay.conditions`):
`max_runs.<node|class>` and `timeout.<node|class|action>` (classes of `tools/node-classes.json`)
go into `replay/conditions.json` (`sdc.conditions/1`), which `sdc-drive` applies over each case's
`bindings.json` when `SDC_CONDITIONS` names it (`bindings_of`). `max_llm_calls`,
`max_decider_calls`, `max_wall_seconds`, `max_rejections` (`SDC_MAX_REJECTIONS`, the rework limit,
default 2), `decider_retries` (`SDC_DECIDER_RETRIES`, default 2) and `review_retries`
(`SDC_REVIEW_RETRIES`) go into the environment.

**Injected faults** (`--inject`, repeatable; place `[<case>/]<name>#<n>`):

| Fault | Effect |
|---|---|
| `timeout@<case>/<node>#<n>` | the call times out on the virtual clock: nothing written, exit `timeout`, the action's `timeout_s` on the clock |
| `invalid-reply@…` | a work node writes nothing (its apply step fails); `change.review` and the decider get `{"invalid": …}`, which their program checks refuse, so their retry paths run |
| `exit@…=<code>` | the adapter exits `<code>` with nothing written; 75 is an API error (the driver stops, resume later) |
| `reject@<decision>#<n>[=<reason>]` | the decider's recorded reply becomes `reject` with the reason (default: "a reject injected by sdc-replay …") |
| `check-fail@<action>#<n>` | the program runs for real, then its run is recorded as exit 1 with a line saying so (`sdc.conditions/1` `wrap` puts `sdc-replay program` before the binding's argv) |

**What cannot be replayed.** A call that the source made under other code: the prompt can differ
(noted), but changed inputs or a node that was a program then (`change.review` before
`story:review-node-is-a-model`) diverge. A reused reply cannot address a rejection or a failed
check: it writes the same revision, so the writer is sent back until `max_runs` (`writer_spent`).
Programs that call no model run for real, so a cargo build or test can differ on another machine.
The source's own timeouts reproduce only when the source recorded the call. Live calls cost money
unless the live adapter is `dry`.

**Proofs.** Cycles in `scripts/run-finding-cycles.sh` re-drive the committed recording
`fixtures/recorded/stub` (a stub run through the fixture adapters, packed: digests, `out/` trees,
structured outputs, event logs with `<run>`/`<path>`, no prompt, transcript or absolute path):
`replay-unchanged`, `replay-reject-stop`, `replay-reject-nearest`, `replay-timeout`,
`replay-live-dry`, `replay-check-fail`, `replay-recording-clean`. Re-recording it after a change to
a node's inputs: `scripts/run-trial.sh` with the fixture adapters (the cycle's comment names the
environment), then `sdc-replay pack runs/<run> driver/examples/sdc/fixtures/recorded/stub`.

## Action-to-tool bindings

One file per protocol in `driver/examples/sdc/`: `sdc`, `sdc-intake`, `sdc-direction`,
`sdc-epic`, `sdc-spec`, `sdc-plan`, `sdc-story`, `sdc-release` (`.bindings.json`). Every action
name, produced kind and placed artifact is checked against the compiled protocol by
`scripts/check-bindings.sh`. Unbound on purpose: `context.change` (a person's edit),
`spec.reuse`, `spec.reconstruct`, `release.rollback`. Tools are subcommands of
`driver/examples/sdc/tools/sdc-tool.sh`; agent nodes run `tools/llm-node.sh` (see "LLM executor"), whose stub mode runs the `stub-*` subcommands, each writing
a fixed draft from `fixtures/` (or a fixed `aep plan artifact new` call) and producing no
evidence, except `spec.author`'s `spec_source=authored`, which records which action wrote it.

| Protocol | Action | Tool | Result → evidence |
|---|---|---|---|
| sdc | `intake.open`, `direction.open`, `epics.open` | driver: open child(ren); `epics.open` takes `epics` from `args_from: list-epics` (the planning store) | `*_opened`, `case_outcome` per child |
| sdc, epic, spec, plan | `roadmap.read` | driver: `copy` from the sdc.direction case | the `roadmap_approval` record |
| intake | `idea.check`, `dissection.check`, `requirements.check` | yq/jq checks (schema shape, spans are substrings of the idea, citations exist) | `valid`/`invalid` |
| intake | `questions.check` | yq checks | exit 0 `valid`+`settled`, 1 `valid`+`open`, 3 `valid`+`needs_human`, else `invalid`+`open` |
| intake | `idea.dissect`, `questions.write`, `questions.answer`, `requirements.normalise` | `llm-node.sh` (stub: fixed file) | revises the artifact |
| direction | `context.default`, `vision.write`, `roadmap.write` | `llm-node.sh` (context: file; vision, roadmap: proposal → `aep plan artifact new vision/initiative/epic`) | revises the artifact |
| direction | `context.check` | yq check | `valid`/`invalid` |
| direction | `roadmap.validate` | `aep plan artifact validate --strict --store aep` | `valid`/`invalid` |
| direction | `context.approve`, `vision.approve`, `roadmap.approve` | `manual`: `approve --action`, producer = principal | `approved` |
| epic, plan, story, release | `spec.gate` | `ess specify validate --strict-requires`, `ess specify compile`, no `UNMAPPED:`, `ess verify conform synthesize` with 0 refusals | `passed`/`failed` |
| epic | `spec.open`, `plan.open`, `stories.open`, `release.open` | driver: open child(ren); `stories` from `args_from: list-stories` (stories that deliver the epic) | `*_opened`, `case_outcome` |
| spec | `spec.author` | `llm-node.sh`, `domain` from `args_from: spec-domain` (stub: copies `fixtures/billing-spec`, ESS `examples/billing`) | `spec_source=authored` |
| spec | `spec.validate`, `spec.compile`, `spec.synthesize` | the `ess` commands above, one each | `valid`, `compiled`, `clean` |
| spec | `spec.scan`, `spec.trace` | `grep -rn UNMAPPED:`; every `system.yaml` domain cited by a requirement | `none`, `traced` |
| spec | `spec.review` | reviewer stub (`test -f`), producer `reviewer-stub`, listed independent | `approved` |
| story | `change.review` | `llm-node.sh change.review`, class REVIEW, producer `llm-review`, listed independent (see "The code review"); stub `test -s impl/README.md` | `approved`/`rejected` |
| plan | `plan.decompose` | `llm-node.sh`: proposal → `aep plan artifact new story`, `scope --add`, `move --to active` (epic, story) | revises `plan` |
| plan | `plan.validate`, `plan.scope`, `plan.status` | `aep plan artifact validate --strict`; `waves --kind story` with 0 unassessed; `list` all active | `valid`, `scoped`, `active` |
| plan | `plan.cover` | every compiled command named by exactly one story file | `covered` |
| story | `change.implement` | `llm-node.sh` (stub: fixed `impl/`) | `implementation_change` |
| story, release | `tests.run`, `acceptance.run` | `story-tests impl` (story: `conform-run impl`, then `impl-gate impl`, see "Interim story gate"), `conform-run release` (release): `ess verify conform run --strict` against `billing` for the billing spec, else `interpreted` plus the tree's `slot task check` (gap 16) | `passed`/`failed` |
| story | `merge.approve` | manual (`argv: ["true"]`), taken by `approve --capability change.merge --action merge.approve`, `--result rejected --reasons <file>` for a rejection | `merge_approval` approved or rejected |
| story | `change.merge` | copy `impl/` to `mainline/` (once `merge.approved`) | `merge_record` |
| release | `release.build`, `release.publish`, `release.observe` | copy `mainline/` to `release/` with a spec digest; copy to `published/`; compare | `build_provenance`, `release_record`, `healthy` |

### Interim story gate (2026-11-21)

`tests.run` was `passed` for an `impl/` holding only a README with `status: done` (eval D1,
`story-readme`), because the conformance run targets ESS's `billing`, not `impl/`. Until ESS can
run the suite against an outside implementation, `sdc-tool.sh impl-gate impl` runs after
`conform-run`, and `tests.run` is `passed` only when both exit 0. Covered commands: the compiled
specification's commands named after `covers` in the story summary (`aep/story/<story>.md`; every
command when the case has no story file). Checks, each logged `impl-gate: ok|FAILED <check>` in
the tool log:

- `source`: `impl/Cargo.toml` and at least one `.rs` file under `impl/src`;
- `tests`: at least as many `#[test]` as covered commands, in the test code (`impl/tests/*.rs`,
  and each `src` file from its first `#[cfg(test)]` on);
- `names`: each covered command's name after its last dot (`CreateTask`) or its snake_case
  (`create_task`) appears in the test code;
- `run`: `cargo test --offline` (through `slot`) in a copy of `impl/`, target directory inside
  the copy, the copy removed afterwards: exit 0, at least 1 test ran, 0 ignored.

The check language is Rust only; a missing `Cargo.toml` fails `source`. Another language is a
branch that sets the same four things. `conform-run`'s `task check` builds into
`.sdc/target-check`, removed afterwards, so neither check writes into `impl/`.

Limits: it checks that the agent's own tests exist, name the covered commands and pass. It does
not check behaviour against the specification: an empty test that names the command passes. It is
replaced by an ESS conformance run against an outside target (`impl/`) when ESS has one (gap 16).
The stub implementer's `fixtures/impl/` is a minimal in-memory Rust library with one `#[test]`
per command, so the stub run passes the gate.

Note: `aep plan artifact create` does not exist; the verb is `new`. `aep` refused to activate a
story that serves no vision objective (`empty_declaration … serves no objective`); the decompose
stub relates the story `serves:vision:product`.

## Preconditions that replaced `until`

A writer that produces no evidence stops through its precondition. Added (any of `is: "false"`,
`is: unknown` on the claim): sdc.intake r4 `idea.dissect` until `dissection.valid`,
`questions.write` until `questions.shaped`, `questions.answer` until `questions.settled`,
`requirements.normalise` until `requirements.valid`; sdc.direction r2 `roadmap.write` until
`roadmap.approved`; sdc.plan r4 `plan.decompose` until `plan.covers_spec` (`plan.valid` would not
do: the store from direction already validates before any story exists). State counts did not
change (intake 65536, direction 49152, plan 65536); `canon check` 0 findings on all eight.

**Closed 2026-11-21 (sdc.direction r3)**: `context.default` until `context.approved`,
`vision.write` until `vision.approved` (each `is: "false"` or `is: unknown`). Before, in the
full-cycle run `context.default` ran 3 times and `vision.write` twice; now once each. A person
changes an approved context through `context.change` (unbound). `canon check`: 49152 states, 0
findings. `cases/04-roadmap-awaiting-approval/direction/expected.json` changed: `context.default`
and `vision.write` `admissible` → `blocked`, because that case has an approved context (`xa`) and
an approved vision (`va`), and an approved revision must not be rewritten; the old expectation
recorded the missing stop.

## Full-cycle run

```console
$ scripts/run-full-cycle.sh full-cycle-1 ~/.cache/scratch/<session>/store-full-cycle-1
```

Output in `runs/full-cycle-1/`: `console.txt`, `run.log` (every `sdc-drive` command and its
output), `decision.json` (final `canon evaluate` of the root), `replay.json`, `events/<case>.jsonl`,
`store/` (the whole case tree). The script plays the person, principal `operator`; each round is one
`run` at its own `--at`:

| Round | Stopped at | Recorded |
|---|---|---|
| 1 | `IDEA-1.direction` awaiting_authority | `approve --capability owner.approve --action context.approve` |
| 2, 3 | `IDEA-1.direction` awaiting_person | `approve … --action vision.approve`, then `roadmap.approve` (grant reused) |
| 4 | `IDEA-1.billing.invoices` awaiting_authority | `approve --capability change.merge` |
| 5 | `IDEA-1.billing.release` awaiting_authority | `approve --capability release.approve` |
| 6 | `IDEA-1` terminable | `close --outcome operating` |

- Every case ended through its success outcome: intake, direction, spec, plan, story, release
  and epic `passed`, root `operating`. 0 `discarded` events.
- Final decision: `operating` legitimate, `abandoned` blocked, every obligation discharged;
  sha256 `63a292f4f23b84a489c22035bb12a1969ceb6b88343dd01d9c79168b837eafff`, the same bytes as an
  earlier independent trial run.
- `sdc-drive replay`: exit 0, state and decision digests match the log in all 8 cases.
- Conformance: `tests.run` and `acceptance.run` each `32 scenarios: 32 passed`. The target is
  ESS's built-in `billing` reference implementation, not the stub's `impl/`: the run proves the
  loop, not an implementation.

Earlier runs (investigation fixture, first SDC attempt) are summarised in the first version of
this document: `close` before the evidence existed was refused (`illegitimate-termination`,
exit 2) and `case.json` restored; a blocked `--action` was refused with Canon's reasons;
moving an artifact put its records under `revision_mismatch`; `{}` for an action with required
arguments was refused at the missing key. The first SDC attempt stopped `stuck` at `story.open`.

## Gaps

Every reject point and writer cap in autopilot, with its route and pending cycles: `reject-routes.md`.

Status 2026-11-21: 2, 3, 7, 13 closed; 12 and 14 partly; 16-23 new.

1. **Authority is not bound to a revision.** A Canon request (see "Authority"); not worked
   around. There is no `decide` command for `canon-decisions/1`.
2. *Closed:* tool timeout (`timeout_s`, process group killed).
3. *Closed:* store lock (`.sdc-drive.lock` at the tree root). Not a generation pointer: a
   crashed writer's half-written step is not rolled back (see 15).
4. **Report checks are not built.** Results come from exit codes only: no report field read,
   no `provenance.spec_digest` match, no timestamp window, no clean-tree check for
   implementation evidence. The no-write check for observations is built (artifact paths only).
5. Child cases and the copy action are TEMPORARY copies of Loom's job (boundary.md rows 27-28).
   One shared `work/` per tree, so two stories of one epic would share `impl/` (the run has one
   story). A failing writer's partial output still moves the revision (seen once in a trial:
   `plan.decompose` exit 1 after creating the story).
6. `spec.open`'s subject `spec` is `none` at opening, which breaks under Canon's option A (see
   "Child cases").
7. *Closed:* stops on `context.default` and `vision.write` (sdc.direction r3).
8. Argument validation covers only the top level, not Canon's whole argument dialect.
9. The spec revision is a tree digest, not the ESS `spec_digest`. *Inferred*: use the ESS
   digest, so `ess` reports can be matched against it.
10. `run` uses one `--at` for every step it takes; the script advances it per round.
11. Plan activation (`aep … move --to active`) is done by the decompose stub: sdc.plan has no
    action for it, so no person approves the stories.
12. A real LLM executor exists (see "LLM executor"); its first real run is `docs/trial-1.md`.
    Several checks are still yq/jq scripts, not `ess` or `aep` (intake, context, `spec.trace`,
    `plan.cover`).

Recorded from `docs/canon-research/01-event-stream-and-resume.md`, not fixed. Its `main.rs`
line numbers were re-checked against the current 685-line file and all match:

13. *Closed:* evidence and `action_finished` are written before `case.json` and `revised` (step
    9 of "The loop"). A crash between the tool's exit and `action_finished` still loses the exit
    code; the next evaluation then commits the tool's revision with no step behind it (see 15).
14. *Partly closed:* documents are written through a temporary file and a rename, and `event`
    appends instead of rewriting. The reader still silently drops an unparsable (cut) last line
    and never checks `prev`.
15. **No resume.** `replay` (`:662-676`) only re-evaluates the final store at the last logged
    instant and compares the two digests; it folds no events, so the log is a record, not the
    source. An `action_started` without `action_finished` is not counted as a run (`:279`,
    `:287`), so *inferred*: the next `run` executes that action again.

New with the LLM executor (2026-11-21):

16. **No conformance of the implementation.** ESS 0.96.0's `conform run` targets are `billing`,
    `oracle-fixture` and `interpreted`; none runs an outside program. For any spec but ESS's
    billing example, `tests.run` and `acceptance.run` run the suite against `interpreted` (which
    checks the spec against itself) plus the tree's own `task check` (tests the LLM wrote). Next:
    ESS's generated Go/TypeScript runner or `conform report` with a runner that drives `impl/`.
    Interim: the story's `impl-gate` (see "Interim story gate").
17. **The spec review is a stub.** `spec.review` is a `test -f` check by `reviewer-stub`.
    `change.review` is a model call by `llm-review` since 2026-11-21 ("The code review").
18. **No per-call narrowing of a shell.** The frame admits operations, not commands, so the
    agents get no shell at all: they cannot run `ess specify validate` or their tests and learn
    only through `retry_when` feedback. Narrowing (`ess specify validate` only) needs metaharness
    `--decisions ask` or `claude --allowedTools "Bash(ess specify validate:*)"`.
19. **AEP actor.** The `aep` calls made from an LLM's proposal are stamped `human:$USER`;
    metaharness's `--actor` covers only writes inside the session.
20. **The implementation language is not a context field.** `change.implement` says Rust 2021,
    standard library only, with a `Taskfile.yml`; the context names human languages only.
21. **A rewrite of an AEP artifact deletes the file first** (`vision.write`, `roadmap.write`,
    `plan.decompose` on a retry): the store has no supersede step for these kinds here.
22. **Shared `work/`** (gap 5): with 2 or 3 stories each story's `change.implement` extends one
    `impl/`; stories run one after another, so it holds, but a story's evidence is about the
    whole tree.
23. **Unvetted vendor pin.** Claude Code 2.1.330 is outside metaharness 0.30.0's pins; its
    event mapping may turn new shapes into `opaque` records, so a tool call could be missing
    from `tool_calls`.
