# Decision requests

A decision request is everything the system asks of a person or a decider when a case cannot go
on without a choice: the question, the options, a recommendation, the grounds the recommendation
rests on, who may decide, and, once decided, who decided what and which Canon record that
produced. The operator, 2026-11-21: every approval or decision "should always come with a
recommendation", based on a vision, a previous decision, a PRD or a story.

Specification: `ess/` (domain `sdc.decision`, entity `DecisionRequest`). Grounding check:
`tools/decision-grounds.sh`. A decider record turned into a request: `tools/decision-from-record.sh`.
Examples: `decision/examples/`.

## What exists already

| where | what it has | what it lacks |
|---|---|---|
| Canon `canon-decision/1` (`canon/website/docs/reference/documents.md:60-78`) | per action `admissible`, `approval-required` or `blocked`, per outcome `legitimate` or `blocked`, with reasons | the pending decision is implicit in it; no options, recommendation or grounds |
| Canon `canon-authority/1` (`documents.md:110-132`) | a grant per capability: `granted` or `denied` | principal and case revision come with story:authority-revision-binding (active; `~/.local/state/worktree/trees/b10x/canon/canon-wave-2026-11-20-w1/.engineering/planning/story/authority-revision-binding.md:44-62`) |
| Canon `canon-decisions/1` (`documents.md:134-148`) | decision, outcome, principal, case revision | options, recommendation, grounds |
| Canon evidence binding (`documents.md:43-56`, `:107`) | a record applies only at its subject revision; `upstream_revisions` invalidate it | the same rule for the grounds of a recommendation |
| Canon story:judgement-marking (`judgement-marking.md:48-52`, beside the story above) | marks evidence a judgement decided (`decided_by: judgement`) | who asked, and why that answer |
| Canon stories' own "Decision needed" sections (`authority-revision-binding.md:93-102`, `:179-181`) | the shape this entity takes: lettered options with cost, a recommendation with its reason, a Decided section | prose in a story, not a record |
| AEP `approval-record` (no lifecycle; `canon/.engineering/planning/approval-record/wave-2026-10-29-w3.md`) | a decision about a planning artifact (`decides:` edge) | options, grounds |
| AEP `decision-blocker` (open → cleared; `canon/.engineering/planning/decision-blocker/outcome-decision-source.md:13-40`) | question, options seen, what it blocks, the decision | recommendation, grounds a program checks |
| Atlas ADR 0088, ADR 0079 | a decision is `decision_record` evidence under a capability; a decision record keeps the digests of the evidence it relied on | the request before the decision |
| Loom (`loom/AGENTS.md:28-32`, `:314-330`) | model-facing approval suspension; Commission's `AuthorityProvider` and the suspension port (`run_with_suspension`, `resume_after_grant`) | the question, options and grounds of what is suspended |
| `beyond10x/decider` (private) | learns which decisions a small model can take; its decision record is one choice in a session | not this: it consumes decisions |
| SDC `docs/autonomy-levels.md` | levels A0 to A5 (`:36-42`, `:205-213`), a holder per decision class (`:229-257`), escalation mode (`:385-387`), eval runs at A5 (`:454-459`) | the record of one decision |
| SDC `questions.yaml` (`runs/trial-todo-1/store/IDEA-1/work/questions.yaml:1-10`) | question, options, default, answer, `answered_by` | grounds; the default has no reason |
| SDC `decider.sh` (`driver/examples/sdc/tools/decider.sh:50-75`, `:119-133`, `:179`) | a reply with `reasons` and `cites` (item ids, checked to exist); since `:119-133` the decider sees only the claims of the action's precondition (approval) or the outcome's condition (close) | citations to a line or a revision; the claims it reads are not citable |
| SDC `runs/trial-todo-1/decisions.jsonl` and `decider/NNN-*/` | 5 real decider decisions (below), written by `run-trial.sh:135-185` | the request; options; a case revision for a close |

## The entity

A `DecisionRequest` holds:

- **What it unblocks**: case id, kind (`approval`, `question`, `outcome`), protocol action, and the
  capability, question id or outcome.
- **The subject**: the artifact and the revision under decision. The decision applies to that
  revision only.
- **The question** and **two or more options**. Each option has a letter, what it does, what it
  costs, and its consequence on the protocol: actions admitted, actions blocked, the grant it
  writes (approval) or the answer it writes (question), and the kind of Canon record it produces
  (`produces`; absent when it produces none). An outcome request has two options: the decision,
  and "leave it untaken", which produces no record.
- **The recommendation**: option, strength (`strong`, `moderate`, `weak`; `none` means there is
  none), whether silence takes it (`default_on_silence`, strong only), who proposed it (`rule`,
  `llm`, `person`, `prior_decision`), and its **grounds**. Or, instead, the reason there is none.
- **Each ground**: source kind (`idea`, `vision`, `requirement`, `prior_decision`, `prd`, `story`,
  `spec`, `policy`, `evidence_record`, `autonomy_rule`), a citation (`path_line` with a digest of
  the lines, `record_id` in a named store, or `artifact_revision`), the option it supports, and
  one sentence on how. `idea` was added because the decider cites dissection items (`d1` to `d4`).
- **Who decides**: the run's autonomy level (A0 to A5), the decision class (13 request classes,
  the rows of `autonomy-levels.md:244-256`; the `escalation` row is not a request class, see
  Escalate below), the **holder** for that class at that level, the eligible principals (handed
  in by the host), and whose inbox it is in (`addressed_to`).
- **The decision**: option, principal (`operator`, `decider:<kind>`, `rule:<name>` or `silence`), its
  kind, time, reasons, and, added afterwards by `RecordProduced`, the Canon record it produced.
- **Links**: `supersedes` and `superseded_by` (relations to another request).

Lifecycle:

```
Request -> Open --Recommend--> Open                         (Request refuses class escalation)
           Open | Escalated --Escalate--> Escalated         (refused under D*, R: end and R∧D*)
           Open | Escalated --Decide | TakeRecommendation--> Decided
           Decided --RecordProduced--> Decided              (only for an option that produces a record)
           Open | Escalated | Decided --Supersede--> Superseded   (terminal)
           Open | Escalated --Expire--> Expired                   (terminal; refused when silence has a default)
```

Commands: `Request`, `Recommend`, `Decide`, `TakeRecommendation`, `RecordProduced`, `Escalate`,
`Supersede`, `Expire`. `TakeRecommendation` (take the recommended option, never send one),
`RecordProduced` and `Expire` were added to the five asked for. Views:
`OpenDecisionsForPrincipal(principal)` (the inbox), `ArtifactDecisionHistory(artifact)`,
`DecisionRequests` (everything).

Escalate moves an existing request; it never opens one. It sets the holder to P, the cell of the
`escalation` row (`autonomy-levels.md:257`), so a decider can no longer decide it. From a decider
or a rule (holders D→P, R, R∧D→P) it is an escalation, and under R∧D→P the decider's veto. Under
P and P+ it is a person passing the request on to someone else, also from Escalated; a P+
request becomes P, which has the same guards, and keeps its recommendation.

`Expire`, and `TakeRecommendation` by `silence`, are fired by a host tick that passes the instant
as input (`at`, `decided_at`); neither reads a clock. Both are refused (`NotDue`) before
`expires_at` or where the request has none.

### Who decides: the holder

The run's level and the request's class give the holder (`autonomy-levels.md:242-257`; per-run
overrides and the order of holders `:297-320`). The request stores the holder; the guards read it,
not the level.

| decision rule | holder (`autonomy-levels.md`) | where it applies | enforced by |
|---|---|---|---|
| a person decides | P, `person` (`:233`) | every class at A0; scope, vision, roadmap at A2; merge, release at A2-A3 | `Decide` and `TakeRecommendation` outcome `person-only` |
| propose only | P+, `advised` (`:234`) | every class at A1 (`:209`) | the same; the decider's proposal goes through `Recommend` and the grounding check |
| take the rule's verdict or veto | R∧D, `rule_and_decider` and `rule_and_decider_final` (`:239`) | merge at A4 (R∧D→P) and A5 (R∧D*) (`:251`, merge rule `:261-267`) | the rule's verdict is recorded as a strong recommendation (`proposed_by: rule`); the decider takes it through `TakeRecommendation` and never grants against it. Its veto is `Escalate` under R∧D→P, and under R∧D* a `Decide` on the denying option (`consequence.grant: denied`), after which the run ends as refused with no `explicitly_abandoned`; any other decider `Decide` is `decider-under-rule` |
| take any option, escalate to a person | D→P, `decider` (`:235`) | most classes at A2-A4; `escalation.mode: person` (`:385-387`) | `Decide` allowed; `Escalate` moves the request to the person and sets the holder to P |
| escalation suppressed, best taken | D*, `decider_final` (`:236`) | every class at A5 (`:213`), so every eval run (`:41-42`, `:454-459`) | `Escalate` outcome `suppressed`; the decider's `best` goes through `Decide`; the driver records `escalation_suppressed` |
| a rule decides, then a person | R and "R, then P", `rule` (`:237`) | rollback from A3, budget at A3-A4, limits at A0-A4 (`:252-256`) | `Decide` outcome `rule-not-holder` keeps `rule:<name>` to R and R: end; an escalate goes to a person |
| a rule decides, the run ends | "R: end", `rule_final` (`:238`) | budget and limits at A5 (`:255-256`) | `Escalate` outcome `suppressed`; the run ends |

Under a final holder (`decider_final`, `rule_and_decider_final`) a decider result that is
missing, failed, or withheld by the grounding check decides the denying option, or ends the run
where no option denies; `escalation_suppressed` records the cause. A rule verdict withheld by the
grounding check under `rule_and_decider` counts as no recommendation and the request goes to a
person; under `rule_and_decider_final` the run ends (`autonomy-levels.md:277-282`).

A listed person may decide whatever the holder (lowering is always allowed, `:311`). Silence
taking a strong default is not in `autonomy-levels.md`; it is kept as a person-facing option and
can only take the recommendation. `TakeRecommendation` takes only a strong recommendation, for
every principal kind; a person who wants a weak or moderate one chooses its option through
`Decide`. Today `run-trial.sh` sends every class to one decider
(`autonomy-levels.md:448-450`): A0 is `SDC_DECIDER=person`, A5 is `SDC_DECIDER=haiku
SDC_DECIDER_ESCALATE=final`, and A1 to A4 need per-class routing that is not built.

## The grounding rule

Deterministic first. `tools/decision-grounds.sh <proposal.json> --case <case dir> [--root DIR]
[--log decisions.jsonl]` reads a request and a proposed recommendation and prints the
`Recommend` input with `check_verdict` and `check_report`. Exit 0 verified, 1 refused.

1. A recommendation names an option of the request, cites at least one ground, and at least one
   ground supports that option. A proposer that escalated proposes no option, so the request
   carries no recommendation.
2. Every citation resolves: the file exists and the lines are inside it (paths resolve against the
   case directory, then `--root`); a digest, when given, still matches the lines; a record id is
   in its store (case `evidence/`, `authority.json`, `decisions.json`, the decision log, or the
   AEP store under `work/aep/`); an `artifact@revision` is the current revision in `case.json`.
   The digest of each cited range is written into the ground, so a later check sees a change.
3. The subject revision is still current (for an outcome, the case revision); if not, the request
   must be superseded.
4. A Decided request in the log with the same `question_key` and subject revision is reused while
   all its grounds still resolve unchanged. Reuse is a strong recommendation, proposed by
   `prior_decision`, whose first ground cites the prior request. A prior whose grounds changed is
   named in `checks` and not reused.
5. An LLM's proposal goes through the same check. If any check fails, the request carries no
   recommendation and `no_recommendation_reason` lists every failure (`Recommend` outcome
   `withheld`).

The program checks that citations exist and are unchanged. Whether the cited text supports the
option is not computed: the `how` sentence states the claim, and the person reads it at the link.

Hand-made requests (case `runs/trial-todo-1/store/IDEA-1/children/IDEA-1.direction`, A0, class
roadmap, holder P):

| request | exit | result |
|---|---|---|
| `roadmap-valid.json` | 0 | verified, "5 grounds, all resolve" |
| `roadmap-dangling.json` | 1 | refused: `docs/phases.md` lines 460-460 outside the file (46 lines); no record `e9999-roadmap.approve` in canon_evidence; `roadmap@t0000000000000000` is not current |
| `roadmap-valid.json --log prior-decisions.jsonl` | 0 | reuses `...-0b`; `...-0a` not reused: its ground `vision@t0ld0ld0ld0ld0ld0` is not current |
| a ground with a wrong digest | 1 | "changed: lines 20-27 of work/aep/vision/product.md differ from the cited digest" |

### The 5 real decider records of `runs/trial-todo-1`

`tools/decision-from-record.sh runs/trial-todo-1/decider/NNN-*` turns a record into a request and
a proposal: options A approve and B reject (for a close, A the decision and B "leave it untaken"),
the decider's choice as the recommendation, each cite
as a `path_line` ground of the item in the case's `work/` files. The run used `SDC_DECIDER=haiku`
with escalation to a person, so the records are converted as A4, holder D→P. Read only; the
proposals were written to the session scratch directory.

| record | decision | check now | why |
|---|---|---|---|
| 001 roadmap.approve | reject | refused, exit 1 | its subject `roadmap@t652c58ef05f60e1c` is no longer current (`t2b3f8657120d0679`, the revision written after this reject). Its 3 grounds (r1-r3) resolve, so it would have passed when it was taken. They are about the epic, not about the reason (the roadmap body was empty): the reason has no ground |
| 002 roadmap.approve | approve | verified, exit 0 | r1-r3 resolve in `work/requirements.yaml` and the subject revision is current |
| 003 change.merge | approve | verified, exit 0 | the same, in `IDEA-1.todolist.storage` |
| 004 release.publish | escalate | refused, exit 1 | no option proposed: an escalation yields no recommendation, and the request goes to a person with the reasons as context. Grounds r1-r3 resolve |
| 005 close | approve | refused, exit 1 | the record carries no case revision (`revision: null` in `decide-request.json`), so the decision is not bound to what it closed. Its 10 grounds (r1-r3, q1-q3, d1-d4) resolve; with the case revision filled in (`c2`) it passes |

Two things the records show the decider cannot cite yet: the claims it is given (`decider.sh:119-133`,
such as `roadmap.valid`, `tests.pass`) are named in its reasons but are not citable ids; they
should become `evidence_record` grounds (`record_id` of the evidence behind each claim, or
`artifact_revision`). And a close needs the case revision in its request.

## How the parts use it

- **Driver** (`sdc-drive`). Where `run` stops with `awaiting_authority` or `awaiting_person`
  (`driver.md:119-127`), each entry of `next` becomes one `Request`: the action, the capability,
  the subject and its revision are in that entry. A handoff question (`driver.md:112-114`)
  becomes one request per open scope-changing question, its options and default taken from
  `questions.yaml`. The class comes from the action (`autonomy-levels.md:244-256`), the holder
  from the run's level. When the subject moves, the driver supersedes the open request. After a
  decision it follows the order of "What a decided request produces" below.
- **Decider** (`SDC_DECIDER`, `run-trial.sh:20-33`). `person` is holder P. `approve` and `reject`
  are rule deciders: they `Decide` the approving or refusing option where the holder allows a
  decider. `haiku` gets the subject, the requirements and only the claims the decision turns on
  (`decider.sh:119-133`), and returns an option, reasons and cites; the driver turns cites into
  grounds and runs the grounding check. Under P+ the result becomes the recommendation only;
  under R∧D the decider takes the rule's verdict through `TakeRecommendation` or vetoes it
  (`Escalate` under R∧D→P, `Decide` on the denying option under R∧D*); under D→P it may `Decide`
  or `Escalate`; under D* an escalation is suppressed and `best` is decided. The escalation reuse
  of `run-trial.sh:139-143` becomes rule 4, keyed by `question_key`. Each decided request is one
  `decisions.jsonl` line, which KPI H2 (`kpis.md:123`), R8 (`kpis.md:87`) and R21/R22
  (`autonomy-levels.md:499-500`) count and `beyond10x/decider` can learn from.
- **UI inbox** (`ui/decisions.mjs`, `ui/README.md:42-51`). The inbox is
  `OpenDecisionsForPrincipal(operator)`: per request the question, the options with cost and
  consequence, the recommendation with its strength, and each ground as a link that opens the
  cited file at the cited line, the record, or the artifact at that revision. One button per
  option (`Decide`), one for the recommendation (`TakeRecommendation`), and Escalate. The UI's
  revision check before every act is the `wrong-state` and supersede rule here. An artifact page
  shows `ArtifactDecisionHistory`.

## What a decided request produces

| unblocks | Canon record | AEP |
|---|---|---|
| approval of a `manual` action (`context.approve`, `vision.approve`, `roadmap.approve`) | the approval evidence (`canon-evidence/1`, kind `*_approval`, producer = principal, subject revision = the request's), through `sdc-drive approve --action` (`driver.md:216-220`) | `approval-record` with `decides: <artifact>` when the subject is a planning artifact |
| approval of a capability (`change.merge`, `release.approve`) | an `AuthorityDecision` (`canon-authority/1`): `granted` or `denied`, with `principal` and `case_revision` once story:authority-revision-binding lands (`authority-revision-binding.md:46-53`); a bound deny lapses at the next case revision (`:181`) | `approval-record` |
| outcome that requires a decision (close) | an `ExplicitDecision` (`canon-decisions/1`): decision, outcome, principal, case revision; the driver has no command for it yet (`driver.md:221-222`). The option "leave it untaken" produces no record | none |
| interview question | a new revision of `questions.yaml` with `answer` and `answered_by` (`run-trial.sh:209-212`); Canon sees it through the next `questions.check` evidence | none |
| an Open or Escalated request that blocks a planning artifact | none | `decision-blocker` (open), cleared when the request is Decided |

Record ids (`ProducedRecord.record`): the evidence id for `canon-evidence/1`,
`<capability>@<case_revision>` for `canon-authority/1`, `<outcome>/<decision>@<case_revision>` for
`canon-decisions/1`, `artifact@revision` for a `questions.yaml` revision.
`tools/decision-grounds.sh` resolves these forms, and until story:authority-revision-binding lands
also the bare forms `<capability>` and `<decision>@<outcome>`; it matches the revision only where
the entry carries one.

The order after a decision, for an option whose consequence names a `produces` kind:

1. `Decide` or `TakeRecommendation` checks every guard and writes nothing to Canon.
2. The runtime writes the Canon record. The record id keys idempotence: the Canon entry carries
   no request id, and two open requests for the same record id must not exist. If the case
   revision moved between the decision and the write, the write is refused and the request is
   superseded.
3. `RecordProduced` stores the record as `produced_record`. The same record id again changes
   nothing; a different one is refused (`ProducedRecordConflict`); so is a request whose decided
   option produces no record (`NoRecordToProduce`).
4. The run resumes. Resume requires `produced_record`.

A crash after step 1 leaves a Decided request without `produced_record`; the retry writes the
Canon record and records it. For an option that produces no record ("leave it untaken"),
`RecordProduced` is refused and the run resumes or ends directly after the decision. Superseding
a Decided request does not revoke the Canon record it produced.

Canon records the principal as given and resolves no identity (`authority-revision-binding.md:57`).
A decision by `decider:haiku` is judgement-decided evidence in the sense of story:judgement-marking
(`judgement-marking.md:52`).

## Ownership

The operator, 2026-11-21: Commission is no longer a repository of its own; it moved into Loom
(`loom/AGENTS.md:8`, `:31`; Atlas ADR 0090, 0096). The entity splits across two components:

- **Canon** (kernel, deterministic; `canon/AGENTS.md:14-16`, `:22-26`):
  - the pending decision as a fact derived from a case: which approval or outcome waits on which
    capability, the subject and its revision, the legitimate options. `canon-decision/1` already
    gives `approval-required` per action and `legitimate` per outcome (`documents.md:76-77`);
  - the grounds as citations to artifact revisions and evidence records, which Canon invalidates
    when a revision moves, as it does for evidence (`documents.md:43-56`, `:107`);
  - the decided record: `canon-authority/1` and `canon-decisions/1` exist (`canon/AGENTS.md:32-36`).
- **Loom** (runtime; holds Commission's `AuthorityProvider` and suspension, `loom/AGENTS.md:28-32`,
  `:314-330`):
  - the types (`sdc.decision`, moved from `sdc/ess/`) and the resolver: preset ⊕ overrides ⊕
    admitted ceilings into one holder per class (`autonomy-levels.md:297-320`);
  - in Loom a decision class is an identifier validated against the classes document the host
    passes in; `budget_overrun` and `limit_hit` are Loom built-ins. The `escalation` row of the
    class table is not a request class: Escalate moves the existing request, and the row gives
    its holder after Escalate;
  - producing the recommendation: a decider executor, rule or LLM, behind the grounding check;
  - routing to the eligible principals, which the host hands in, and the inbox. The inbox is a
    store plus CLI and SDK calls, not a service;
  - expiry and silence, fired by a host tick that passes the instant; escalation.
- **SDC**: its protocols' use of both, and the documents it passes to Loom as host: the preset
  table (`protocol/autonomy-levels.yaml`), the classes document (`sdc.decision-classes/1`) and
  the admitted ceilings (`eval/admitted.yaml`). Every host passes its own; SDC ships these. The
  ESS specification in `sdc/ess/` is the shared contract until it moves;
  `tools/decision-grounds.sh` is the reference for the check.

Superseding a Decided request does not revoke the Canon record it produced.

Not owners: AEP mirrors decided planning decisions (`approval-record`) and open ones
(`decision-blocker`); Mandate resolves principals (`authority-revision-binding.md:89`);
`beyond10x/decider` learns from decided requests.

## Requests (sent 2026-11-21)

**To the Canon session.** For decision requests (`sdc/docs/decisions.md`, `sdc/ess/domains/decision.yaml`):

1. A derived view of pending decisions from `canon-decision/1`: per `approval-required` action the
   capability, the subject artifact and its current revision; per outcome that requires an
   explicit decision, the outcome and the case revision; the options as the decisions that change
   it (grant or deny; decide the outcome or not). Today SDC reconstructs this from the driver's
   `next` list.
2. A ground format, or a reuse of `canon-evidence/1` binding: a citation to `artifact@revision` or
   an evidence id, applied only while that revision is current and invalidated by the same rules
   as evidence (`documents.md:43-56`, `:107`), so a recommendation whose grounds moved reads as
   stale from the evaluation, not from a separate program.
3. Land story:authority-revision-binding, so the decided record carries `principal` and
   `case_revision`.

Canon's answer to 1 and 2: accepted as story:pending-decisions (canon wave 5) and
story:decision-grounds (canon wave 6). `pending_decisions` carries no subject artifact: the caller
maps action → artifact through the bindings file. A pending recommendation is not a Canon input.

**To the Loom session.** For decision requests, the runtime side (`sdc/docs/decisions.md`):

1. A decider executor port: given a pending decision and its inputs, return an option, reasons and
   grounds (rule or LLM); a recommendation is accepted only when every ground resolves
   (`sdc/tools/decision-grounds.sh` is the reference behaviour), otherwise none, with the reason.
2. The autonomy policy of `sdc/docs/autonomy-levels.md`: level A0 to A5, holder per decision class
   (`:242-257`), overrides and the order of holders (`:297-320`), escalation mode person or final
   (`:385-387`), recording `escalation_suppressed`.
3. Routing and inbox: eligible principals (handed in by the host, resolved by Mandate), the
   open-decisions query per principal, expiry with an optional default on silence (fired by a
   host tick that passes the instant), escalation to a named person, and resume of the suspended
   run (`resume_after_grant`) in this order: `Decide` (all guards, no Canon write), the Canon
   write keyed by the record id, `RecordProduced`, then resume, which requires `produced_record`
   where the decided option produces a record.
4. The entity itself (`sdc/ess/domains/decision.yaml`, commands `Request`, `Recommend`, `Decide`,
   `TakeRecommendation`, `RecordProduced`, `Escalate`, `Supersede`, `Expire`) as the contract,
   moved into Loom's ESS when it is taken.
