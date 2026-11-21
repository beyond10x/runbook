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
scripts/check-protocols.sh   # validate, compile and check all eight, then the ESS gate probes
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
| sdc.direction | `context_approval`, `vision_approval`, `roadmap_approval` | `context.approve`, `vision.approve`, `roadmap.approve`, each `requires: owner.approve` | person |
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
| sdc.story | `merge_record` | `change.merge`, `requires: change.merge` | version control |
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
   at the root for intake and plan; sdc.intake, sdc.spec and sdc.plan sit exactly at 65536
   (states per protocol in `docs/check-output.txt`: sdc 4608, intake 65536, direction 49152,
   epic 36864, spec 65536, plan 65536, story 12288, release 24576); every child
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
   and granted or denied, nothing more (documents.md). Approvals of context, vision and roadmap are
   therefore evidence records bound to the approved revision, produced only by actions that
   `require: owner.approve`; `canon check`'s authority-bypass check finds no outcome that holds on
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
