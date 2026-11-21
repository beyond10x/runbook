# `canon simulate` findings on the SDC protocols

What Canon's unreleased `canon simulate` prints for the eight protocols, and for each finding
whether a real run can reach the state and not leave it, or whether the walk's limits produce it.
The operator's question (2026-11-21): static analysis should find the states "where nothing is
pointing to a next step" before a real run does.

## Run

- Canon commit `a1a937ec` (branch `wave/2026-11-20`, unreleased), run by the Canon session as
  `canon simulate --path <protocol.yaml>` over the protocols of SDC `main` at `dfe4a3f`. Those
  protocol files are byte-identical to `protocol/*.protocol.yaml` at `dfe4a3f`.
- The printed lines quoted below come from that run's reports. The reports are not committed.
- Re-running `canon simulate` on a scratch copy of `sdc-plan.protocol.yaml` gives the same summary.

| Protocol | Reachable | Terminable | Waiting | Findings | Stuck | Cycle | Decision line |
|---|---|---|---|---|---|---|---|
| `sdc` | 78 | 54 | 10 | 1 | 1 | 0 | 1 |
| `sdc.epic` | 122 | 91 | 15 | 2 | 2 | 0 | 2 |
| `sdc.direction` | 41 | 1 | - | 10 | 0 | 10 | 0 |
| `sdc.intake` | 1215 | 408 | - | 17 | 0 | 17 | 0 |
| `sdc.plan` | 85 | 1 | - | 17 | 2 | 15 | 0 |
| `sdc.release` | 60 | 36 | 1 | 3 | 1 | 2 | 3 |
| `sdc.spec` | 120 | 61 | - | 16 | 1 | 15 | 16 |
| `sdc.story` | 54 | 28 | - | 5 | 1 | 4 | 5 |
| total | | | | 71 | 8 | 63 | 27 |

"Decision line" counts the findings that end in "decision `d` taken makes outcome `o`
legitimate". A decision step does not make a state open in the walk.

## What a finding means (Canon `a1a937ec`, `crates/canon/src/simulate/mod.rs`)

- A state is open when action, supplied, child-outcome, expiry and authority steps lead from it to
  a legitimate outcome. Decision steps do not count.
- `stuck-state`: the state is not open, and no action is admissible or approval-required.
- `cycle`: one state that is not open, where no action is approval-required, some action is
  admissible, and every step of every admissible action returns that same state.
- A loop that has an exit is not printed. Simulate checks that an outcome can be reached, not that
  every path ends, so a rework loop is no finding, bounded or not.
- Limits of this binary:
  - One record per step. An action that records two kinds in one taking is walked as two steps.
    Fixed by Canon's `simulate-record-groups`.
  - No revise step. A writer with no evidence, or a writer whose second record is the same as its
    first, returns the same state, so rework on a new revision prints as a `cycle`. Fixed by
    `simulate-revise-step`, which needs the protocol to declare `revises`
    (`story:protocols-declare-revises-and-authors`).

## Verdicts

- `real`: a real case can sit in the state with nothing admissible and nothing waiting on an actor.
- `left only by a decision`: as `real`, but an explicit decision ends the case (in autopilot that
  needs a rule end or the decider).
- `left by revision`: a real case leaves the state by revising an artifact. The walk has no
  revise step.
- `left by a second record of the same step`: the record-groups limit.
- `cannot tell`.

Every finding of the `left by revision` kind is a rework loop that the protocol does not bound. A
real run that keeps failing the check leaves it only at a cap in the driver, marked
`bounded only by the driver`:

- `max_runs`: writer runs per arrival, default 8 (`driver/src/main.rs:700`).
- The autopilot's 2-rejection limit: the third rejection ends the run `rework_spent`
  (`scripts/run-trial.sh:521-555`, `:551`).

| Verdict | Stuck | Cycle | Total |
|---|---|---|---|
| `real` | 1 | 4 | 5 |
| `left only by a decision` | 3 | 3 | 6 |
| `left by revision` (all `bounded only by the driver`) | 4 | 56 | 60 |
| `left by a second record of the same step` | 0 | 0 | 0 |
| `cannot tell` | 0 | 0 | 0 |

Trial runs: no `runs/trial-3-*` run stopped on any of these states. The `run-stopped*.json` files
and `console.txt` show no `stuck` verdict. Across `runs/trial-3-*` there are 0 roadmap rejections,
and all 49 gate records in `sdc.plan`, `sdc.story` and `sdc.release` cases are `passed`
(61 of 61 across all of `runs/`). Two runs
hit a driver cap of a rework loop:

- `runs/trial-3-haiku-2`: `spec.author` spent, max_runs 4.
- `runs/trial-3-haiku-4`: `change.implement` spent, 3 runs.

## Stuck states (8)

A state "evidence `K` about `S`" with no result is a record whose result does not unlock the claim
(not `approved`, not `passed`).

| # | Protocol | State in plain words | Protocol text | Verdict | Story |
|---|---|---|---|---|---|
| 1 | `sdc` | Intake and direction ended `passed`, but the root's copy of the roadmap approval is not `approved`. | `protocol/sdc.protocol.yaml:81-89` (`roadmap.read`), `:115-141` (`epics.open` needs `roadmap.approved`, `:123`), `:142-177` | `left by revision` | `story:sdc-root-roadmap-copy-not-approved` |
| 2 | `sdc.epic` | Roadmap approval copy not `approved`; nothing else. | `protocol/sdc-epic.protocol.yaml:103-111`, `:112-135`, `:195-230` | `left by revision` | `story:sdc-epic-record-without-unlocking-result` |
| 3 | `sdc.epic` | Spec child ended `passed`; the epic's gate record about the same spec revision is not `passed`. | `protocol/sdc-epic.protocol.yaml:125-135` (`spec.gate` once per revision), `:136-151` (`plan.open` needs `gate.passed`) | `left only by a decision` (`abandoned`) | `story:sdc-epic-record-without-unlocking-result` |
| 4 | `sdc.plan` | Roadmap approval copy not `approved`. | `protocol/sdc-plan.protocol.yaml:138-146`, `:147-233` | `left by revision` | none (not reached by a run) |
| 5 | `sdc.plan` | Roadmap approved; gate record about the current spec not `passed`. | `protocol/sdc-plan.protocol.yaml:147-157` (`spec.gate`), `:158-221` (every other action needs `gate.passed`), `:235-246` (`passed` is the only outcome) | `real` | `story:sdc-plan-gate-not-passed-has-no-outcome` |
| 6 | `sdc.release` | Gate record about the current spec not `passed`. | `protocol/sdc-release.protocol.yaml:121-137`, `:184-203` | `left only by a decision` (`withheld`) | in #5's story |
| 7 | `sdc.spec` | Roadmap approval copy not `approved`. | `protocol/sdc-spec.protocol.yaml:183-191`, `:192-242` (writers need `roadmap.approved`) | `left by revision` | none (not reached by a run) |
| 8 | `sdc.story` | Gate record about the current spec not `passed`. | `protocol/sdc-story.protocol.yaml:150-169`, `:230-241` (`amend.upstream` needs `tests.pass` false), `:243-258` | `left only by a decision` (`dropped`) | in #5's story |

Why:

- **Roadmap copies (1, 2, 4, 7).**
  - A copy that is not `approved` is set aside by the direction case's next roadmap revision. Every
    case reads the same `aep/initiative/roadmap.md` digest; for example the root, direction and
    epic `case.json` of `runs/trial-3-strong-2` all hold `t48bf2260334d9815`. After the new
    revision, `roadmap.read` is admissible again.
  - A real run does not reach 2, 4 or 7. An epic opens only on the root's `roadmap.approved`
    (`protocol/sdc.protocol.yaml:123`). The copy takes the latest record at the current revision
    (`driver/src/main.rs:474-492`), so it is `approved`.
  - State 1 needs the direction case to pass at the same roadmap revision it rejected. Only a
    vision revision re-opens that decision (`protocol/sdc-direction.protocol.yaml:319-324`). With
    the roadmap rejected, no action of the direction case can revise the vision:
    - `vision.write` needs the vision not approved (`:218-231`);
    - `amend.upstream` needs a requirements revision that no running case makes (`:289-299`).
    So only an outside edit of the vision gets there.
- **Gate records (3, 5, 6, 8).**
  - The gate program runs the same four commands as `sdc.spec`'s own checks
    (`driver/examples/sdc/tools/sdc-tool.sh:143-149` against `:150-155`). It can therefore fail on
    a spec revision whose spec case passed only when the program itself fails: a crash, a timeout,
    or a newer `ess` between the stages.
  - Every non-zero exit except 126, 127 and 75 records `failed`: `"*": "failed"` in
    `driver/examples/sdc/sdc-plan.bindings.json:25`, `sdc-story.bindings.json:23`,
    `sdc-release.bindings.json:16` and `sdc-epic.bindings.json:22`. The exceptions are in
    `driver/src/main.rs:549-554`.
  - After that, nothing in the case revises the spec: the spec case has ended.
  - 21 epic gate records in `runs/` are `failed`, all at spec revision `none`, before the spec
    exists. The spec child's first revision sets each aside (`runs/trial-3-strong-2`: `e0008`
    failed at `none`, `e0020` passed). That is not state 3.
  - **Route for 5:** a `sdc.plan` outcome that requires `gate.passed` `false`.
    - Probed on a scratch copy: the stuck state no longer prints, 16 findings remain, and 2 states
      are terminable.
    - The epic then ends `not_delivered`, because its `children` form lists only `passed`
      (`protocol/sdc-epic.protocol.yaml:209-219`).
    - The same outcome in `sdc.story` and `sdc.release` turns 8 and 6 from a decision into a
      predicate.

Printed text of the eight:

```
stuck-state: state {evidence `case_outcome` result `passed`, evidence `direction_opened` about `idea`, evidence `intake_opened` about `idea`, evidence `roadmap_approval` about `roadmap`}: no outcome is legitimate and no action is admissible or approval-required; path: action `intake.open` produces evidence `intake_opened` about `idea`, then action `roadmap.read` produces evidence `roadmap_approval` about `roadmap`, then a child of action `intake.open` records evidence `case_outcome` result `passed`, then action `direction.open` produces evidence `direction_opened` about `idea`, then a child of action `direction.open` records evidence `case_outcome` result `passed`; decision `explicitly_abandoned` taken makes outcome `abandoned` legitimate
stuck-state: state {evidence `roadmap_approval` about `roadmap`}: no outcome is legitimate and no action is admissible or approval-required; path: action `roadmap.read` produces evidence `roadmap_approval` about `roadmap`; decision `explicitly_abandoned` taken makes outcome `abandoned` legitimate
stuck-state: state {evidence `case_outcome` result `passed`, evidence `ess_gate` about `spec`, evidence `roadmap_approval` about `roadmap` result `approved`, evidence `spec_opened` about `epic`}: no outcome is legitimate and no action is admissible or approval-required; path: action `roadmap.read` produces evidence `roadmap_approval` about `roadmap` result `approved`, then action `spec.gate` produces evidence `ess_gate` about `spec`, then action `spec.open` produces evidence `spec_opened` about `epic`, then a child of action `spec.open` records evidence `case_outcome` result `passed`; decision `explicitly_abandoned` taken makes outcome `abandoned` legitimate
stuck-state: state {evidence `roadmap_approval` about `roadmap`}: no outcome is legitimate and no action is admissible or approval-required; path: action `roadmap.read` produces evidence `roadmap_approval` about `roadmap`
stuck-state: state {evidence `ess_gate` about `spec`, evidence `roadmap_approval` about `roadmap` result `approved`}: no outcome is legitimate and no action is admissible or approval-required; path: action `roadmap.read` produces evidence `roadmap_approval` about `roadmap` result `approved`, then action `spec.gate` produces evidence `ess_gate` about `spec`
stuck-state: state {evidence `ess_gate` about `spec`}: no outcome is legitimate and no action is admissible or approval-required; path: action `spec.gate` produces evidence `ess_gate` about `spec`; decision `explicitly_withheld` taken makes outcome `withheld` legitimate; 2 other stuck states contain it
stuck-state: state {evidence `roadmap_approval` about `roadmap`}: no outcome is legitimate and no action is admissible or approval-required; path: action `roadmap.read` produces evidence `roadmap_approval` about `roadmap`; decision `explicitly_abandoned` taken makes outcome `abandoned` legitimate
stuck-state: state {evidence `ess_gate` about `spec`}: no outcome is legitimate and no action is admissible or approval-required; path: action `spec.gate` produces evidence `ess_gate` about `spec`; decision `explicitly_dropped` taken makes outcome `dropped` legitimate; 2 other stuck states contain it
```

## Cycles (63), grouped by cause

In each group the admissible action is a writer. A writer either produces no evidence or produces
a record identical to its first. In the walk its step returns the same state; in a run it makes a
new revision, and the failed check runs again on that revision.

| Group | Protocol | Loop in plain words | Findings | Protocol text | Verdict | Cap |
|---|---|---|---|---|---|---|
| D1 | `sdc.direction` | Context check failed: `context.default` (or `amend.upstream`) rewrites the context. | 2 | `protocol/sdc-direction.protocol.yaml:159-178`, `:289-299` | `left by revision` | max_runs 3 (`driver/examples/sdc/sdc-direction.bindings.json:34`), 12 (`:160`) |
| D2 | `sdc.direction` | `owner.approve` denied: only `context.change` is admissible. It has no binding (`driver/src/main.rs:648-650`), and a new context still needs the denied capability. | 2 | `:179-192`, `:204-217` | `real` | none; the driver stops `stuck` (`driver/src/main.rs:1210-1211`) |
| D3 | `sdc.direction` | Context approval rejected: `context.default` writes a new context. | 2 | `:159-178`, `:204-217` | `left by revision` | max_runs 3 (`sdc-direction.bindings.json:34`); 2 rejections (`scripts/run-trial.sh:551`) |
| D4 | `sdc.direction` | Vision approval rejected: `vision.write`. | 2 | `:218-246` | `left by revision` | max_runs 3 (`sdc-direction.bindings.json:80`); 2 rejections |
| D5 | `sdc.direction` | Roadmap validation failed, or roadmap approval rejected: `roadmap.write`. | 2 | `:247-288` | `left by revision` | max_runs 3 (`sdc-direction.bindings.json:122`); 2 rejections |
| I1 | `sdc.intake` | Dissection check failed: `idea.dissect` (also `questions.answer` while the interview is not settled). | 8 | `protocol/sdc-intake.protocol.yaml:138-157` | `left by revision` | max_runs 3 (`driver/examples/sdc/sdc-intake.bindings.json:43`) |
| I2 | `sdc.intake` | Interview shape failed: `questions.write`. | 4 | `:158-169` | `left by revision` | max_runs 3 (`sdc-intake.bindings.json:82`) |
| I3 | `sdc.intake` | Requirements check failed: `requirements.normalise` or `amend.upstream`. | 1 | `:213-242` | `left by revision` | max_runs 3 (`:156`), 12 (`:182`) |
| I4 | `sdc.intake` | Interview not settled, `scope.answer` granted: `questions.answer` or `scope.answer`. | 2 | `:170-199` | `left by revision` | max_runs 3 (`:100`), 8 (`:197`) |
| I5 | `sdc.intake` | Interview not settled, `scope.answer` denied: `questions.answer` leaves every scope-changing question open (`:172`), so the interview never settles. | 2 | `:170-199` | `real` (while a scope-changing question is open) | `bounded only by the driver`: max_runs 3 (`:100`) |
| P1 | `sdc.plan` | One or more of the four plan checks failed: `plan.decompose` (or `amend.upstream` when not covered). 15 = 2^4 - 1 combinations. | 15 | `protocol/sdc-plan.protocol.yaml:158-233` | `left by revision` | max_runs 3 (`driver/examples/sdc/sdc-plan.bindings.json:55`), 12 (`:123`) |
| R1 | `sdc.release` | Release conformance failed: only `release.build` is admissible, and it rebuilds the same merged changes. The driver does not take it again (no `retry_when`, `driver/src/main.rs:661-677`). | 1 | `protocol/sdc-release.protocol.yaml:131-149`, `:200-203` | `left only by a decision` (`withheld`) | none |
| R2 | `sdc.release` | `release.approve` denied (`docs/protocol-notes.md:253-254`). | 1 | `:150-161`, `:200-203` | `left only by a decision` (`withheld`) | none |
| S1 | `sdc.spec` | A spec check failed or the review rejected: `spec.author`, `spec.reuse` or `spec.reconstruct` (or `amend.upstream` when untraced). | 15 | `protocol/sdc-spec.protocol.yaml:192-242`, `:312-323` | `left by revision` | max_runs 4 (`driver/examples/sdc/sdc-spec.bindings.json:61`), hit in `runs/trial-3-haiku-2`; 12 (`:149`) |
| T1 | `sdc.story` | Conformance run failed: `change.implement` or `amend.upstream`. | 1 | `protocol/sdc-story.protocol.yaml:160-169`, `:230-241` | `left by revision` | max_runs 3 (`driver/examples/sdc/sdc-story.bindings.json:61`), hit in `runs/trial-3-haiku-4` |
| T2 | `sdc.story` | Review rejected: `change.implement`. | 1 | `:160-169`, `:182-196` | `left by revision` | max_runs 3 |
| T3 | `sdc.story` | `change.merge` denied (`docs/protocol-notes.md:251-252`). | 1 | `:197-228`, `:255-258` | `left only by a decision` (`dropped`) | none |
| T4 | `sdc.story` | Merge rejected: `change.implement`. | 1 | `:160-169`, `:197-214` | `left by revision` | max_runs 3; 2 rejections |

Totals: 10 direction, 17 intake, 15 plan, 2 release, 15 spec, 4 story = 63.

Notes:

- **D2 and I5 are one cause.** A capability denial lasts for the case: the driver refuses a
  capability decided twice (`docs/protocol-notes.md:243-245`). Neither protocol has an outcome
  that a denial reaches, so the parent waits on the child.
  - Route, probed on a scratch copy of `sdc.direction`: a supplied kind `authority_withheld`, and
    an outcome that requires it with `context.valid` and `context.decided` unknown. The two D2
    findings no longer print; the other 8 do.
  - An outcome that requires the supplied record alone makes every state open. The probe then
    printed 0 findings, so the gate would see nothing. The outcome must be narrowed to the stuck
    state's claims.
  - A new evidence kind multiplies the state count. `sdc.direction` and `sdc.intake` hold 49152
    states each (`docs/check-output.txt:2`, `:6`), and the old bound is 65,536. *Inferred*: this
    needs Canon's per-slice bound, which the `a1a937ec` `check` help text describes as unreleased.
  - Story: `story:sdc-lasting-denial-has-no-outcome`.
- **S1, rejected review (2 of 15).** In the protocol `spec.author` is admissible. Its binding's
  `retry_when` does not name `spec_review` (`driver/examples/sdc/sdc-spec.bindings.json:44-60`),
  so the driver would not take it, and a run would stop `stuck`.
  - Not reachable today: `review-spec` is a stub, `test -f spec/system.yaml`
    (`driver/examples/sdc/tools/sdc-tool.sh:291`), and all 61 `spec_review` records in `runs/` are
    `approved`.
  - This is a binding gap for when the reviewer becomes real, not a protocol finding.
- **Record groups.** `questions.check` records two kinds (`protocol/sdc-intake.protocol.yaml:200-212`).
  Every intake cycle state holds both kinds, so no finding comes from the one-record-per-step
  limit. The limit only adds intermediate states to the 1215.

State of each group, as printed (one finding each; the other findings of a group differ only in
the result of the failed records):

- D1 `{context_defaults, roadmap_validation result valid}`
- D2 `{context_defaults result valid, roadmap_validation result valid, capability owner.approve denied}`
- D3 `{context_approval, context_defaults result valid, roadmap_validation result valid, capability owner.approve granted}`
- D4 `{context_approval result approved, context_defaults result valid, roadmap_validation result valid, vision_approval, capability owner.approve granted}`
- D5 `{context_approval result approved, context_defaults result valid, roadmap_approval, roadmap_validation result valid, vision_approval result approved, capability owner.approve granted}`
- I1 `{dissection_shape, idea_shape result valid, questions_settled result settled, questions_shape result valid, requirements_shape result valid}`
- I2 `{dissection_shape result valid, idea_shape result valid, questions_settled result settled, questions_shape, requirements_shape result valid}`
- I3 `{dissection_shape result valid, idea_shape result valid, questions_settled result settled, questions_shape result valid, requirements_shape}`
- I4 and I5 `{dissection_shape result valid, idea_shape result valid, questions_settled, questions_shape result valid, requirements_shape result valid, capability scope.answer granted | denied}`
- P1 `{decomposition_coverage result covered, ess_gate result passed, plan_validation result valid, roadmap_approval result approved, stories_scoped result scoped, story_states}`
- R1 `{build_provenance, ess_gate result passed, release_conformance}`
- R2 `{build_provenance, ess_gate result passed, release_conformance result passed, capability release.approve denied}`
- S1 `{open_markers result none, roadmap_approval result approved, spec_source, spec_validation}`
- T1 `{conformance_run, ess_gate result passed, implementation_change}`
- T2 `{code_review by a producer independent on different_principal, conformance_run result passed, ess_gate result passed, implementation_change}`
- T3 `{code_review result approved ..., conformance_run result passed, ess_gate result passed, implementation_change, capability change.merge denied}`
- T4 `{code_review result approved ..., conformance_run result passed, ess_gate result passed, implementation_change, merge_approval, capability change.merge granted}`

## Why `sdc.direction` and `sdc.plan` have one terminable state

- A terminable state is a state in which some outcome is legitimate.
- `sdc.direction` and `sdc.plan` have no outcome but `passed` (`docs/protocol-notes.md:197-198`;
  `protocol/sdc-direction.protocol.yaml:301-310`, `protocol/sdc-plan.protocol.yaml:235-246`).
  `passed` needs every check recorded once with its passing result. The guards admit one record
  per kind per revision, and the walk makes no revision, so exactly one reachable state has them
  all: 1 of 41 and 1 of 85.
- The other protocols count every state with a decision taken as terminable: `abandoned`,
  `dropped`, `withheld`. For example `sdc.spec` has 61 terminable states, of which `abandoned` is
  legitimate in 60 and `passed` in 2.
- This is expected. Giving up a direction or a plan is the parent's decision: the root's
  `abandoned`, or the epic's `abandoned` and `not_delivered`.
- The count changes when an outcome is added: with the route for stuck state 5, `sdc.plan` has 2
  terminable states.

## What becomes checkable when Canon releases the command

- `story:simulate-as-a-protocol-gate`: `scripts/check-protocols.sh` prints each `simulated:` line
  and fails when a protocol's finding count rises above `protocol/simulate-expected.json`. The
  counts to commit are the table above.
- With `simulate-revise-step` and `revises` declared (`story:protocols-declare-revises-and-authors`),
  the 60 `left by revision` findings should stop printing (*inferred*). What is left is the 5
  `real` and 6 `left only by a decision` findings. The gate can then fail on any finding that is
  not on a committed list of decision-only states.
- With `simulate-record-groups`, `questions.check` is walked as one step. A guard over both of its
  kinds (`all`), which today leaves `passed` unreachable (`docs/protocol-notes.md:100-103`), can be
  checked.
- Each `real` finding has a story whose acceptance is that the finding no longer prints:
  - stuck state 5: `story:sdc-plan-gate-not-passed-has-no-outcome`;
  - D2 and I5: `story:sdc-lasting-denial-has-no-outcome`.
  - Stuck states 1 to 3: `story:sdc-root-roadmap-copy-not-approved` and
    `story:sdc-epic-record-without-unlocking-result`.
- Not checkable by simulate: whether a run can reach a state at all. Roadmap copies 2, 4 and 7 are
  unreachable only because of the runtime's copy source (`driver/src/main.rs:474-492`), and Canon
  sees no other case (`docs/protocol-notes.md`, limitation 5). The driver's caps (`max_runs`, the
  2-rejection limit) are outside the protocol too.
