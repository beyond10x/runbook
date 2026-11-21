# SDC protocol notes

The SDC is a family of eight Canon `protocol/1` documents under `protocol/`, one case per stage,
composed with child cases. Sources: `docs/phases.md` (phase model), `docs/intake.md` (intake
stages and evidence names), Canon 0.9.0 at commit 8f1f452.

| Protocol | File | Opens | Success outcome |
|---|---|---|---|
| `sdc` (root, per idea) | `sdc.protocol.yaml` | `sdc.intake`, `sdc.direction`, one `sdc.epic` per roadmap epic | `operating` |
| `sdc.intake` | `sdc-intake.protocol.yaml` | none | `passed` |
| `sdc.direction` | `sdc-direction.protocol.yaml` | none | `passed` |
| `sdc.epic` | `sdc-epic.protocol.yaml` | `sdc.spec`, `sdc.plan`, one `sdc.story` per AEP story, `sdc.release` | `passed` |
| `sdc.spec` | `sdc-spec.protocol.yaml` | none | `passed` |
| `sdc.plan` | `sdc-plan.protocol.yaml` | none | `passed` |
| `sdc.story` | `sdc-story.protocol.yaml` | none | `passed` (delivered) |
| `sdc.release` | `sdc-release.protocol.yaml` | none | `passed` (operating) |

No case stores a phase. Where a case stands is what its evidence makes true; `canon evaluate`
lists the open obligations (what is missing) and the admissible actions (what is next) at every
stage, as `cases/*/decision.json` show.

The root case's artifacts are `idea`, `goals` and `roadmap` (`sdc.protocol.yaml:7-14`). `goals` is
the requester's resolved goals document, `work/goals.yaml`, put there by `scripts/run-trial.sh
--goals FILE` (`docs/requester.md`, "How a run receives it"); `sdc.intake` and `sdc.direction`
declare it too (`sdc-intake.protocol.yaml:10`, `sdc-direction.protocol.yaml:23`) and read the same
file through the shared `work/`. It is an input only: no claim, invalidation or child opening names
it, so its revision changes no state count of `canon check` and sets no record aside. Without
`--goals` its revision is `none`, as in the fixture cases.

Commands (from `../canon`; `cargo` is the slot shim):

```console
scripts/check-protocols.sh   # validate, compile and check all eight, the ESS gate probes, then simulate
scripts/evaluate-cases.sh    # canon evaluate every case run, compare with expected.json
```

The scripts run `cargo run --locked -q -p canon-cli -- validate|compile|check --path <file>` and
`... evaluate --ir <ir> --case <case> --evidence <dir> [--authority <file>]`. The CLI takes
`--path`, not a positional path.

## Cases

Each run directory under `cases/` holds `case.yaml`, `evidence/`, optionally `authority.yaml`,
`expected.json` (claim values and statuses) and `decision.json` (the real `canon evaluate`
output). `scripts/evaluate-cases.sh` compares them; output in `docs/evaluate-output.txt`.

| Run | Protocol | What it shows |
|---|---|---|
| `01-idea-only` | sdc | only `intake.open` (and `roadmap.read`) admissible |
| `01-idea-only/intake` | sdc.intake | idea valid; next `idea.dissect`; dissection, interview, requirements open |
| `02-spec-revised-tests-stale/spec` | sdc.spec | spec revised s1 to s2, reused source, gate passes at s2: `passed` legitimate; s1 records `revision_mismatch`, the author's own review `not_independent` |
| `02-spec-revised-tests-stale/story` | sdc.story | conformance run and implementation record against s1 `invalidated`: `tests.pass` and `change.built` unknown, merge blocked, `change.implement` admissible |
| `03-ready-to-release` | sdc.release | gate, provenance and release conformance true: `release.publish` approval-required |
| `03-ready-to-release/approved` | sdc.release | same with `release.approve` granted: `release.publish` admissible |
| `04-roadmap-awaiting-approval/direction` | sdc.direction | vision revised after the roadmap approval: approval `invalidated`, `roadmap.approve` approval-required |
| `04-roadmap-awaiting-approval/epic` | sdc.epic | no roadmap approval: every epic action blocked except `roadmap.read` |
| `05-roadmap-read-once/{root,epic,spec,plan}` | sdc, sdc.epic, sdc.spec, sdc.plan | a roadmap approval record at r1, roadmap at r1: `roadmap.read` blocked (`roadmap.copied` true) |
| `05-roadmap-read-once/*/revised` | same | same record, roadmap at r2: the record is set aside, `roadmap.read` admissible again |
| `06-approval-once/{context,vision,roadmap}/approved` | sdc.direction | approval `approved`: the approve action blocked, the next stage's writer admissible |
| `06-approval-once/{context,vision,roadmap}/rejected` | sdc.direction | approval `rejected`: approve blocked, the writer (`context.default`, `vision.write`, `roadmap.write`) admissible |
| `06-approval-once/{context,vision,roadmap}/revised` | sdc.direction | rejected at revision 1, artifact at revision 2: the check due (`context.check`, `roadmap.validate`; vision has none, so `vision.approve` admissible) |
| `06-approval-once/{context,roadmap}/rechecked` | sdc.direction | as revised with the check valid at revision 2: approve admissible again |
| `06-approval-once/merge/{approved,rejected,revised,rechecked}` | sdc.story | merge approved: `change.merge` admissible; rejected: only `change.implement`; change at c2 built: `tests.run` due (`change.review` after it passed); tested and reviewed: `merge.approve` admissible |
| `07-plan-check-failed` | sdc.plan | plan covered, its validation invalid: `plan.decompose` admissible, the only action that revises the plan |
| `08-intake-idea-refused` | sdc.intake | idea record invalid: no action revises the idea, outcome `refused` legitimate |
| `09-child-ended-otherwise/{root,epic}` | sdc, sdc.epic | intake ended `refused`: root `refused` legitimate; spec ended `abandoned`: epic `not_delivered` legitimate |
| `10-release-refused/{release,release/withheld,epic,root}` | sdc.release, sdc.epic, sdc | publication refused: `withheld` legitimate with its decision; epic `not_released`; root `not_delivered` |
| `11-review-after-tests/{untested,passed,failed}` | sdc.story | `change.review` needs `tests.pass` at the current change revision: blocked before the run, admissible after a pass, blocked after a fail with `change.implement` admissible |
| `11-review-after-tests/{rejected,revised,retested}` | sdc.story | after a rejecting review: `change.implement`; at c2 `tests.run`, review blocked; tests passed at c2: `change.review` admissible again |

The other cases show the same rule for the checks: `01-idea-only/intake` (`idea.check` blocked
by its record), `02-spec-revised-tests-stale/spec` (every spec observation blocked at s2; the
author's own review does not block `spec.review`, the independent one does),
`02-spec-revised-tests-stale/story` (`tests.run_recorded` unknown: the run against s1 is
invalidated), `03-ready-to-release` (`spec.gate`, `acceptance.run` blocked),
`04-roadmap-awaiting-approval/direction` (`context.check`, `roadmap.validate` blocked).

## An observation runs once per revision

An observation that records a result is admissible only without a current record for the
artifact's revision. Each such action (`effect: read`, producing a kind some claim reads with
`result:`) carries a precondition `{claim: <kind>-recorded, is: unknown}` on a claim that reads its
kind without a result about the same subject, for example `roadmap.copied`
(`protocol/sdc-plan.protocol.yaml:45-50`) guarding `roadmap.read`
(`protocol/sdc-plan.protocol.yaml:138-146`). Without it a second record with another result made
the claim `unknown` (records that disagree, `../canon/website/docs/reference/evaluation.md:39-42`),
which blocks every action that needs it, and only a new revision got the case out: `canon simulate`
reported that as 1113 `cycle` findings on sdc.plan, sdc.spec and sdc.epic.

- A record about an older revision is not of the claim, so a new revision makes the action
  admissible again (`cases/05-roadmap-read-once/*/revised`).
- Where an invalidation rule sets a check's result aside, the guard claim is invalidated with it
  (`protocol/sdc-plan.protocol.yaml:252-256`, `protocol/sdc-intake.protocol.yaml`
  `dissection.revised`, `questions.revised`; sdc-direction `requirements.revised`; sdc-release and
  sdc-story `spec.revised`), so an upstream revision makes the check admissible again too.
- A review demanding independence is guarded by a claim with the same `independent_on`
  (`spec.reviewed`, `change.review_recorded`): the author's own review neither counts nor blocks.
- `release.observe` reads `health_observation` (max_age 30m): an expired observation is not of
  `release.observed`, so the release is observed again after 30 minutes.
- `questions.check` records two kinds in one run; it is admissible while either has no current
  record (`protocol/sdc-intake.protocol.yaml` `questions.check`). Canon has no form "no current
  record of kind K" other than a claim; a guard over both kinds (`all`) leaves `passed`
  unreachable in `canon simulate`, whose step adds one record.
- Guarded: sdc `roadmap.read`; sdc.epic `roadmap.read`, `spec.gate`; sdc.spec `roadmap.read`,
  `spec.validate`, `spec.compile`, `spec.scan`, `spec.synthesize`, `spec.trace`, `spec.review`;
  sdc.plan `roadmap.read`, `spec.gate`, `plan.validate`, `plan.cover`, `plan.scope`,
  `plan.status`; sdc.intake `idea.check`, `dissection.check`, `questions.check`,
  `requirements.check`; sdc.direction `context.check`, `roadmap.validate`; sdc.release
  `spec.gate`, `acceptance.run`, `release.observe`; sdc.story `spec.gate`, `tests.run`,
  `change.review`.
- Order between two observations is the protocol's, not the driver's: the driver takes
  admissible observations in identifier order, so `change.review` ran before `tests.run`.
  `change.review` now also needs `tests.pass` (the conformance run passed at the current change
  revision), so a failed run sends the implementer back before any review, and after a rejecting
  review the loop is `change.implement`, `tests.run`, `change.review` at the new revision. Cases
  `11-review-after-tests/*`.
- Not guarded: `release.publish` and `release.rollback` (`effect: external`, acts under
  `release.approve`). The approvals follow their own rule below.

## An approval is decided once per revision

An approval kind has two results, `approved` and `rejected`; `<artifact>.approved` is true only on
`approved`, `<artifact>.decided` reads the kind without a result. Every approval action is
admissible only while `<artifact>.decided` is unknown, so it is decided once per revision of the
artifact it approves. After `rejected` the artifact's writer is admissible; its new revision leaves
the checks and the rejection behind (revision binding), the checks run again, and the approval is
admissible again. Nothing else waits on the rejection itself. How many times an artifact was
rejected is not the protocol's business.

| Protocol | Approval action (authority) | Kind | Writer after `rejected` | Checks due at the new revision |
|---|---|---|---|---|
| sdc.direction | `context.approve` (`owner.approve`) | `context_approval` | `context.default` | `context.check` |
| sdc.direction | `vision.approve` (`owner.approve`) | `vision_approval` | `vision.write` | none |
| sdc.direction | `roadmap.approve` (`owner.approve`) | `roadmap_approval` | `roadmap.write` | `roadmap.validate` |
| sdc.story | `merge.approve` (`change.merge`) | `merge_approval` | `change.implement` | `tests.run`, then `change.review` once it passed |

`change.merge` now needs `merge.approved` instead of the capability: the capability `change.merge`
is decided once per case (Canon refuses it twice), so a rejection could not be a capability
denial. A merge decision is not in `spec.revised`'s invalidation: it is bound to the change
revision, `change.merge` also needs `change.built`, and an entry would give `merge_approval` an
upstream (16 values), putting sdc.story at 196608 states. `roadmap.decided` is invalidated with
`roadmap.approved` by a revised vision.

Bindings: each approval binding records `results: {"0": "approved", "6": "rejected"}`; the driver's
`approve --action <a> --result rejected --reasons <file>` records `rejected` and keeps the reasons
file as the step's log. Each writer's `retry_when` names its approval kind with `rejected`, so the
driver sends the writer back while the rejection is about the current revision and hands it the
log in `SDC_FEEDBACK` (the tools write it to the writer's `in/rejection.json`).

**Stated limit, sdc.release.** `release.approve` stays a capability of `release.publish`, with no
approval record and no `rejected` result: the same treatment (`release_approval` with a manual
`release.approve`) gives 98,304 states, past the bound of 65,536. Dropping the capability to fit
would leave authority resting on a manual record alone, which is not allowed. A refused release
therefore records nothing in the case; the tools stop such a run with a named cause
(`rejected_without_rework_path`), and the case's way out stays `withheld` on
`explicitly_withheld` (the release case has no writer that answers a rejection: `release.build`
rebuilds the same merged changes; the rework happens in the stories). Canon's state-space bound
per slice (unreleased) is what lifts this limit.

Cases: `06-approval-once/{context,roadmap,merge}/{approved,rejected,revised,rechecked}` and
`06-approval-once/vision/{approved,rejected,revised}`.

## Every child outcome has a way on in the parent

`children.passed` (`{children: {in: [passed]}}`) is `false` for good once any child ended with
another outcome, whatever revision it was about (limitation 4), so reopening a child cannot make
it `true` again. sdc and sdc.epic therefore end on that claim, with an outcome that says which
stage failed, legitimate without a person (a predicate, no decision). The stage is read from the
opening records, since a later stage opens only once every earlier child passed:

- sdc.epic `not_delivered`: `children.passed` `false` and `release.opened` unknown (a child
  before the release failed); `not_released`: `children.passed` `false` and `release.opened`
  (the release failed) (`protocol/sdc-epic.protocol.yaml:209-230`).
- sdc `refused`: `children.passed` `false` and `direction.opened` unknown (the intake failed);
  `not_delivered`: `children.passed` `false` and `direction.opened` (an epic failed)
  (`protocol/sdc.protocol.yaml:155-177`). `direction.opened` is bound to the idea revision: an
  idea revised after an epic failed would read as `refused`; a revised idea is a new root case in
  practice.

The outcomes read existing claims, so the state counts do not move (sdc 4608, epic 36864).

| Parent | Child protocol | Child outcome | Parent's way on |
|---|---|---|---|
| sdc | sdc.intake | `passed` | `direction.open` |
| sdc | sdc.intake | `refused` (idea failed its check) | ends `refused`; the next idea revision is a new root case |
| sdc | sdc.direction | `passed` | `epics.open` once the roadmap is approved |
| sdc | sdc.epic | `passed` | `operating` once every epic passed |
| sdc | sdc.epic | `not_delivered`, `not_released`, `abandoned` | ends `not_delivered` |
| sdc.epic | sdc.spec | `passed` | `plan.open` |
| sdc.epic | sdc.spec | `abandoned` | ends `not_delivered` |
| sdc.epic | sdc.plan | `passed` | `stories.open` |
| sdc.epic | sdc.story | `passed` | `release.open` once every story passed |
| sdc.epic | sdc.story | `dropped` | ends `not_delivered` |
| sdc.epic | sdc.release | `passed` | `passed` |
| sdc.epic | sdc.release | `withheld` (publication refused), `rolled_back` | ends `not_released` |

sdc.direction and sdc.plan have no outcome but `passed` (a plan is given up through the epic's
`abandoned`). Either parent can also end `abandoned` on its explicit decision at any point.

**A refused release.** sdc.release ends `withheld` on the decision `explicitly_withheld`
(`protocol/sdc-release.protocol.yaml:200-203`). Canon cannot tie that decision to a state: an
outcome requires either a predicate or a decision, not both (Canon reference, `OutcomeRequirement`),
and a `canon-decisions/1` entry is legitimate at any case revision it names. It needs no
capability either: the entry carries a `principal`, nothing else, and no action is involved. So
"only after the publication was refused" is the tools' rule: take `explicitly_withheld` when
`release.approve` was denied or the decider judged the release not publishable, with the
decider's reasons as the decision's log. A refusal record that the protocol could read would be a
new evidence kind; sdc.release cannot take one under the bound (98,304 states, see "An approval is
decided once per revision").

Cases: `09-child-ended-otherwise/root` (intake `refused`: root `refused`),
`09-child-ended-otherwise/epic` (spec `abandoned`: epic `not_delivered`), and the refused-release
chain `10-release-refused`: `release` (`release.approve` denied: `release.publish` blocked,
nothing legitimate), `release/withheld` (with `explicitly_withheld`: `withheld` legitimate),
`epic` (every earlier child passed, release `withheld`: `not_released` legitimate), `root` (intake
and direction passed, the epic `not_released`: `not_delivered` legitimate).

## What `canon simulate` still reports and why

Canon's classification of the findings on f364b6b (classes a1, a2, a3, c, d, e) is the reference
for this section; c is fixed by the approval rule above.

The 71 findings of Canon `a1a937ec` on `dfe4a3f`, each with a verdict: `docs/simulate-findings.md`.

- **A failed plan check (a3, fixed).** `plan.decompose` was guarded on coverage only, so with the
  plan covered but invalid, its scope unassessed or its stories not active, no action could
  revise the plan. It is now also admissible on `plan.valid`, `plan.scoped` or `plan.active`
  `false` (`protocol/sdc-plan.protocol.yaml:158-176`), as its binding's `retry_when` already
  expected (`plan_validation` invalid, `stories_scoped` unassessed, `story_states` draft).
  `amend.upstream` stays bound to an open finding (coverage `false`). Case:
  `07-plan-check-failed` (covered, validation invalid: `plan.decompose` admissible).
- **A failed idea check (a2 on `idea`, 18 findings, sdc.intake).** Nothing in an intake case
  revises the idea: a revised idea opens a new intake case at the root
  (`protocol/sdc-intake.protocol.yaml:6`). The case now ends `refused` once `idea.shaped` is
  `false` (`protocol/sdc-intake.protocol.yaml:253-259`; case `08-intake-idea-refused`). The root
  then ends `refused` (section above): for the idea's next revision `intake.open` is
  admissible again, but the refused child's `case_outcome` keeps `children.passed`
  (`protocol/sdc.protocol.yaml:61-65`, `in: [passed]`) `false` for good, also after the new
  intake case ends `passed` (limitation 4; probed with `canon evaluate` on a root case at i2
  holding both child outcomes), so a reopened intake cannot lead to `operating`.
- **A failed check another action revises (a1).** Left by a new revision, which the walk does not
  make; the writer is admissible in each such state. A rejected approval is the same.
- **A failed check only another case revises (a2 on `roadmap`, `spec`, 6 findings).** The child
  waits on a revision another protocol writes; no change.
- **A lasting denial (d).** The driver refuses a capability decided twice
  (`driver/src/main.rs:1590-1597`), so a denial stands for the case. Per denial, what a real
  case does next:
  - sdc.direction, `owner.approve` denied (2 findings): no other holder and no outcome in the
    case; the tree goes on through the root's `abandoned` decision, or a revised idea, which
    opens a new direction case with its own authority (`protocol/sdc.protocol.yaml:6`). A
    direction outcome of its own would need a decision, which doubles 49152 states past the bound
    (as intake's abandon did, limitation 1), so the root keeps that role.
  - sdc.story, `change.merge` denied (1): `merge.approve` stays approval-required; the case ends
    `dropped` on `explicitly_dropped`, and sdc.epic ends `not_delivered` (section above).
  - sdc.release, `release.approve` denied (1): the case ends `withheld` on
    `explicitly_withheld`, and sdc.epic ends `not_released`.
- **sdc `{intake_opened, roadmap_approval}` stuck (e, false).** Not a protocol defect, no
  protocol change; the walk is Canon's to fix. `intake.open` produces two records, its
  `intake_opened` and, through `case_open`, the child's `case_outcome`; its precondition
  `intake.opened is unknown` closes it after the first. The walk adds one record per step, so the
  state with `intake_opened` and no `case_outcome` has no step left. The runtime writes the
  `case_outcome` record (`open`) in the same step that opens the child (`open_children`,
  `driver/src/main.rs:770`) and rewrites its result when the child ends, so a real case never
  holds `intake_opened` without it. The same holds for `direction.open`, `epics.open` and
  sdc.epic's opening actions.

## Simulate

`scripts/check-protocols.sh` ends with a simulate step: `canon simulate --path` on each
`protocol/*.protocol.yaml`, its stdout compared byte for byte with the pinned report
`protocol/simulate/<name>.txt`. Any difference fails with a unified diff and
`simulate: <protocol>: findings changed (was N, now M)`; one line per protocol gives reachable,
terminable, waiting and findings. The released Canon 0.9.0 has no `simulate`, so the step uses
the unreleased commit in `protocol/simulate/CANON_REV` (Canon PR 27, decision D151), built once
by `cargo install --locked --root` into `${XDG_CACHE_HOME:-$HOME/.cache}/sdc/canon-simulate/<rev>/`
(outside the repository, not on `PATH`, not the binary of `CANON_DIR`); every other check keeps
the released binary.

- Re-pin after a deliberate protocol change: `scripts/check-protocols.sh --repin` (or
  `SDC_SIMULATE_REPIN=1`), review `git diff protocol/simulate/`, commit the reports with the
  protocol and the reason in the commit body.
- A new Canon commit: edit `CANON_REV`, then `--repin`; Canon's next two waves change simulate's
  output, so the reports move with them. A Canon release replaces the pin.
- `SDC_SIMULATE=off` skips the step with a printed line (no network).
- The pinned reports are a ratchet, not a verdict: every protocol still exits 1 with findings
  (the triage is on branch `plan/simulate-triage`). The limits Canon named: the walk adds one record per
  step (a step that produces two records, such as `case_open`, is walked as one), and it has no
  revise step, so a failed check on an artifact a protocol revises reads as a stuck state.

## Evidence kinds and the action that produces each

An LLM only writes artifacts (actions with no `may_produce`, such as `idea.dissect`,
`spec.author`, `change.implement`'s code). Every result below is written by a program or a person.

| Protocol | Evidence kind | Produced by | Program or person |
|---|---|---|---|
| sdc | `intake_opened`, `direction_opened`, `epics_opened` | `intake.open`, `direction.open`, `epics.open` | runtime |
| sdc | `case_outcome` | the three opening actions (`case_open`) | runtime, from the child case |
| sdc, sdc.epic, sdc.spec, sdc.plan | `roadmap_approval` | `roadmap.read` (copy) | runtime, copied from sdc.direction |
| sdc.intake | `idea_shape` | `idea.check` | `ess generate schema validate` |
| sdc.intake | `dissection_shape` | `dissection.check` | schema check plus span check |
| sdc.intake | `questions_shape`, `questions_settled` | `questions.check` | schema check, interview check |
| sdc.intake | `requirements_shape` | `requirements.check` | schema check, citation check |
| sdc.direction | `context_defaults` | `context.check` | schema check |
| sdc.direction | `context_approval`, `vision_approval`, `roadmap_approval` (approved, rejected) | `context.approve`, `vision.approve`, `roadmap.approve`, each `requires: owner.approve` | person |
| sdc.direction | `roadmap_validation` | `roadmap.validate` | `aep plan artifact validate` |
| sdc.epic | `spec_opened`, `plan_opened`, `stories_opened`, `release_opened` | `spec.open`, `plan.open`, `stories.open`, `release.open` | runtime |
| sdc.epic | `case_outcome` | the four opening actions | runtime, from the child case |
| sdc.epic, sdc.plan, sdc.story, sdc.release | `ess_gate` | `spec.gate` | the four ESS gate commands as one program |
| sdc.spec | `spec_source` (authored, reused, reconstructed) | `spec.author`, `spec.reuse`, `spec.reconstruct` | runtime, recording which action wrote the revision |
| sdc.spec | `spec_validation` | `spec.validate` | `ess specify validate --strict-requires` |
| sdc.spec | `spec_compile` | `spec.compile` | `ess specify compile` |
| sdc.spec | `open_markers` | `spec.scan` | `grep -rn 'UNMAPPED:'` |
| sdc.spec | `spec_synthesis` | `spec.synthesize` | `ess verify conform synthesize --suite-format 5` |
| sdc.spec | `scope_trace` | `spec.trace` | `ess verify diff` joined to the requirements list |
| sdc.spec | `spec_review` | `spec.review` | person or another agent, `independent_on: [different_principal]` |
| sdc.plan | `plan_validation` | `plan.validate` | `aep plan artifact validate` |
| sdc.plan | `decomposition_coverage` | `plan.cover` | join of compiled commands and scenarios to story bodies |
| sdc.plan | `stories_scoped` | `plan.scope` | `aep plan artifact waves --kind story` |
| sdc.plan | `story_states` | `plan.status` | `aep plan artifact list` |
| sdc.story | `implementation_change` | `change.implement` | version control, with the spec revision built against |
| sdc.story | `conformance_run` | `tests.run` | `ess verify conform run --target <implementation> --strict` |
| sdc.story | `code_review` | `change.review` | person or another agent, `independent_on: [different_principal]` |
| sdc.story | `merge_approval` (approved, rejected) | `merge.approve`, `requires: change.merge` | person |
| sdc.story | `merge_record` | `change.merge`, on `merge.approved` | version control |
| sdc.release | `build_provenance` | `release.build` | build system |
| sdc.release | `release_conformance` | `acceptance.run` | `ess verify conform run --target <release> --strict` |
| sdc.release | `release_record` | `release.publish`, `requires: release.approve` | deploy |
| sdc.release | `health_observation` (max_age 30m) | `release.observe` | monitoring |
| sdc.release | `rollback_result` | `release.rollback`, `requires: release.approve` | deploy |

Invalidation: idea → dissection check; dissection → interview check; interview → requirements
check (sdc.intake); requirements → context check; vision → roadmap approval (sdc.direction);
spec → plan validation and coverage (sdc.plan); spec → implementation record and conformance run
(sdc.story); spec → release conformance (sdc.release).

## Reconciliation with docs/intake.md

- Kept as named: `idea_shape`, `questions_shape`, `questions_settled`, `spec_validation`,
  `spec_compile`, `open_markers`, `spec_synthesis`, `scope_trace`, `spec_review`,
  `plan_validation`, `decomposition_coverage`, `stories_scoped`; artifacts `questions`, `plan`.
- Dropped `draft_validation`: phases.md replaces draft0 and its markers with the dissection; open
  items become questions. `dissection_shape` takes its place, and `questions_settled` is
  invalidated by a revised dissection instead of a revised draft0.
- Dropped `spec_self_conformance`: `sdc.spec` already has 65536 states without it (the bound),
  and intake.md G8 says it is the model checked against itself, not implementation conformance.
- `scope_trace` traces every added change id to the requirements list, not to idea spans and
  questions: in phases.md the requirements list is what the spec is written from, and a reused or
  reconstructed spec has no idea spans.
- `spec.valid` is not intake.md's proposed conjunction; the conjunction is `spec.complete` (the
  four gate checks), and `spec.done` adds source, trace and review.
- Outside `sdc.spec`, the gate is one kind, `ess_gate`: one program runs the four checks and
  records `passed` only when all four pass. Four kinds in every child do not fit the bound (see
  limitations).
- The roadmap is an AEP `initiative`: `aep plan artifact kinds` (aep 0.80.0) lists vision,
  product-requirements, initiative, epic, story and no roadmap kind.

## ESS gate property

`canon-properties/1` cannot say it: its only property is that a subject's status does not depend
on a claim (`independent_of`, documents.md), not that an outcome implies a claim. So
`scripts/check-protocols.sh` proves it with `canon check` itself: for each protocol whose success
outcome leads to implement, test or release (sdc.spec, sdc.plan, sdc.epic, sdc.story,
sdc.release) it writes `protocol/probes/<name>.gate-probe.yaml`, the protocol plus one outcome
`probe.passed_without_gate` that requires the success outcome's own requirement and the gate
evidence, read directly from the evidence match (four matches in sdc.spec, `ess_gate` elsewhere),
being `false` or `unknown`. The script passes only when `canon check` reports that outcome as
`unreachable-outcome` and nothing else. The full output is `docs/check-output.txt`.

The root `sdc` has no specification of its own (one per epic), so its `operating` outcome rests on
its epic cases' `case_outcome` records; that the gate holds there is the sdc.epic result above,
not something Canon checks across cases (inferred: Canon checks one protocol at a time).

## Canon limitations hit

Reproduce with `scripts/limitations.sh`; its output is `docs/limitations/output.txt`, the inputs
are in `docs/limitations/`.

1. **State-space bound, 65536.** It decides the shape of the whole family. Each evidence kind
   matched with one result costs 4 values, with an invalidation upstream 16, a capability 3, a
   decision 2, a `children` form with one listed outcome 8 times 9 for its history. Refusals hit:
   - one protocol for idea, spec, stories, release and operate (`monolith.protocol.yaml`):
     `error[state-space-bound]: protocol `sdc.probe.monolith` revision 1 has 2654208 states`;
   - the story with the four gate kinds instead of one `ess_gate`
     (`story-with-four-gate-kinds.protocol.yaml`): `... sdc.story revision 3 has 786432 states`;
   - intake with its own abandon decision (`intake-with-abandon.protocol.yaml`):
     `... sdc.intake revision 2 has 131072 states`.
   Consequences: eight protocols; the gate is one composite kind outside sdc.spec; abandon lives
   at the root for intake and plan; sdc.spec and sdc.plan sit exactly at 65536
   (states per protocol in `docs/check-output.txt`: sdc 4608, intake 49152, direction 49152,
   epic 36864, spec 65536, plan 65536, story 49152, release 24576); every child
   reports `passed`, so a parent's `children` form lists one outcome (two would cost 16 times 17).
2. **Invalidation reaches every match of a claim built on an invalidated claim.** A derived claim
   `change.verified = tests.pass AND change.reviewed` gave `code_review` upstream variants too:
   `canon check --path docs/limitations/story-with-derived-verified-claim.protocol.yaml` →
   `... has 196608 states ... evidence kind `code_review` about `change` with upstream `spec` with
   producers independent on [`different_principal`] 16`. Invalidated claims are therefore leaves,
   and conjunctions are written inline in preconditions and outcomes.
3. **A child's outcome cannot be invalidated.** `canon validate --path
   docs/limitations/child-outcomes-invalidated.protocol.yaml` → `error[inert-invalidation]:
   invalidation rule `spec.revised` invalidates claim `stories.delivered`, which reaches no
   evidence match`. A revised spec does not reopen delivered stories at the parent; the gate and
   invalidation inside each child carry it instead.
4. **The `children` form reads all children alike.** It cannot tell a story child from the release
   child, and a child that ended outside the listed set makes the claim `false` for good
   (protocol.md, `ChildOutcomes`). Stage order therefore needs one `*_opened` record per stage, and
   a stage that failed needs the parent abandoned and restarted (inferred: no way to retire a
   child outcome).
5. **No cross-case evidence.** Evidence produced in another case counts as produced by no action:
   `canon check --path docs/limitations/epic-imported-evidence.protocol.yaml` → 8 findings such as
   `unproduced-evidence: action `spec.open` reads evidence kind `roadmap_approval`, which no action
   may produce`. Workaround: an explicit `roadmap.read` action that copies the record; Canon cannot
   tell that copy from a forgery (producer is "recorded as given: Canon does not prove it",
   documents.md).
6. **Authority decisions are not bound to a revision.** A `canon-authority/1` entry is a capability
   and granted or denied, nothing more (documents.md). Approvals of context, vision, roadmap and
   merge are therefore evidence records bound to the decided revision, produced only by actions
   that require `owner.approve` or `change.merge`; `canon check`'s authority-bypass check finds no outcome that holds on
   them without authority (0 findings in every protocol).
7. **Every declared artifact needs a revision, even before it exists.** `canon evaluate` with a case
   that omits one → `error[missing-artifact]: case `IDEA-1` does not give the current revision of
   artifact `roadmap``. The cases use the revision `none`. There is no predicate on whether an
   artifact exists; `spec_source` serves as that fact for the spec.
8. **The evidence directory holds only records.** An empty directory cannot carry a `.gitkeep`:
   `error[unreadable]: .../evidence/.gitkeep: not an evidence record: the evidence directory holds
   only `*.yaml` and `*.json` files` (exit 2).
9. **One case per evaluation.** `canon evaluate` takes one `--case` (cli.md). Carrying the spec
   revision from parent to child, and the tree view of where an idea stands, are the runtime's job
   (inferred: no command evaluates a case tree).
10. **`canon check` time.** The two 65536-state protocols take minutes each to check (observed,
    not timed precisely); the full script runs in the background.

Not Canon: AEP 0.80.0 has no roadmap kind (`aep plan artifact kinds`), so the roadmap is an
`initiative`; the AEP story lifecycle is draft, proposed, active, implemented
(`aep plan artifact lifecycle story`), so `story_states` waits for `active`.
