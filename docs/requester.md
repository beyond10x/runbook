# The requester side and its goals

Requirement (the operator, 2026-11-21), after trial 2 stopped on two scope questions he had no context
for: "Somehow we do need to inject company goals, or higher level context […] which would, for
example, drive these interview decisions. So the implementing or planning side would find these
ambiguities, and then the requester, in that case also played by an agent — or not really played,
it's a valid real-life use case — would then answer these questions based on its own values and
this maybe higher level context."

Two sides. The **delivering side** (intake, planning, implementation) finds ambiguities and raises
questions and decisions. The **requesting side** answers them: a person, or a **requester agent**
that holds the requester's values and a **goals document**. Read on 2026-11-21; nothing was built
or run. *(inferred)* marks design not observed.

## 0. Decisions

| # | Decision | Reason |
|---|---|---|
| 1 | Format `sdc.goals/1`, one "goals document"; entry kinds `goal`, `principle`, `constraint`, `preference` | The operator's word; `context` is taken (`context.yaml`, `phases.md:21-26`) |
| 2 | Run input: artifact `goals` of the root case, revision = sha256 of the resolved file | artifacts cost no state (`trial-2.md:90`); a revision makes it citable |
| 3 | A citation rests on one entry (`goals@<rev>#<id>` plus the entry's digest), not on the whole document | one new preference must not reopen every answer |
| 4 | A goals change sets answers aside through the existing `finding` result, not a Canon invalidation | `sdc.intake` is at 49,152 states and cannot take a dimension (`trial-2.md:85-88`, `:128`) |
| 5 | Requester agent = a decider configuration, principal `decider:requester`; no new `PrincipalKind` | every guard and holder (D→P, D\*) applies as is (`decision.yaml:180-190`); Canon sees only the string (`decisions.md:238`) |
| 6 | Each decision class gets a `side` in `sdc.decision-classes/1`; the host hands in eligible principals per side | the P of a scope question is the requester, the P of a merge the maintainer; `eligible` is host input already (`decision.yaml:237`) |
| 7 | Requester inputs: the request, its subject, the files its grounds cite, idea, dissection, goals, its own earlier decisions; nothing else | independence by construction, checkable as an input list |
| 8 | No ground in the goals: D→P escalates to the requester person; D\* takes the request's default, marked `no_ground_in_goals` | the existing holder semantics (`autonomy-levels.md:235-236`) |
| 9 | Answers that a goal states are `answered_by: derived` with a `basis`; scope-changing ones only where the class holder is D (A3 up) | keeps the A0-A2 rule "scope is the person's" (`autonomy-levels.md:244`); see question 1 |
| 10 | Persona per eval item: goals given to the run, `truth.yaml` hidden beside the oracle | the oracle rule (`eval-design.md:174-177`) |

## 1. What exists and is reused

| Exists | Where | Used for |
|---|---|---|
| Decider gets `in/` = idea, dissection, questions, requirements, context, `aep/vision/product.md`, `aep/initiative/roadmap.md`, `aep/epic`, the story, the subject, `checks.json`, `decision.json` | `decider.sh:127-133`, `:146-149` | **no goals or requester context among them**; vision and roadmap are the run's own outputs. Its prompt acts "in place of the product owner" (`:175`) with no values to act on |
| Decision contract: options, recommendation, strength, grounds, holder, eligible | `decisions.md:35-59`; `decision.yaml:157-165` | the requester's reply and the person's view |
| Grounding check: citations resolve, digests match, prior reuse | `tools/decision-grounds.sh:17-55`, `:68-81` | goals citations (one new resolver case) |
| Holders, classes, levels, eval at A5 | `autonomy-levels.md:242-257`, `:397-413`, `:454-472` | requester classes |
| Gold decisions per eval item (proposed, not built) | `autonomy-levels.md:520-523` | the persona's truth |
| Ranks, `answered_by: derived`, findings, `amend.upstream`, `scope.answer` | `trial-2.md:57-68`, `:109-121` | derived answers, stale-basis findings |
| Consistency rules (`sdc-consistency-rules/1`) | `bindings/tools/consistency-rules.json` | rules against constraints |
| AEP `vision` = an objective; active work must `serves` one once a store declares any | `aep plan artifact relations` (`serves`); `aep/crates/edge/aep-cli/tests/planning_cli.rs:2374-2441` | product-layer source |
| AEP `initiative` (draft → proposed → active → implemented), `product-requirements` | `aep plan artifact kinds`, `lifecycle initiative` | time-bound goals, constraints |
| Brownfield reads context from the repository and adopts `vision:*` | `orchestration.md:318-319` | where an existing repository's goals come from |
| Cold-read epic: questions carry excerpt, consequences, default reason | `epic:decisions-a-person-can-take-cold` | the request the requester receives |
| Eval: hidden oracle, no persona, no simulated requester | `eval-design.md:174-177`; `eval/` has none (searched) | built here |

Gaps found: `check-questions` accepts a scope-changing answer under any `answered_by` except empty
and `non-interactive` (`sdc-tool.sh:65-67`), so `derived` passes with no basis checked. The
contract had no free answer: `Decide` took an option letter only, yet both of the operator's
trial-2 answers were "neither option as asked" (`questions.yaml:11`, `:33`). Closed by
story:decision-contract-goals-grounds: `Decide` and `Recommend` take a free `answer` on a question
request (`decisions.md`, "The decision"; scenario `ess/scenarios/question-free-answer.yaml`).

## 2. The goals document, `sdc.goals/1`

File fields: `format: sdc.goals/1`, `layer` (required: `company`, `product` or `request`; every id
prefix must match it), optional `requester`, `entries`. Nothing else.

Entry fields: `id` (stable, never reused, qualified by layer: `company/c2`), `statement` (one
sentence), `kind`, `priority` (an integer 1 to 5, 1 highest; required on goal, principle and
preference, forbidden on a `constraint`, which is hard), `sources` (a non-empty list; each item is
exactly one of `person:<id>`, `aep:vision:O1@<rev>`, `path:line` or `decision:<request id>`: no
`+`, no parenthesis, no trailing note), `valid_from`, optional `valid_until` (strictly after
`valid_from`; `YYYY-MM-DD` or `YYYY-MM-DDTHH:MM:SSZ`, a date meaning 00:00:00Z), optional `stated`
(`{by, at}`: who said it, on which date; not part of a source), optional `derived_from` (a list of
entry ids that exist in the given layers), optional `overrides` (a higher-layer id) and `rule`
(§ 4, a constraint only). A single `source:` string is refused (`source_form`: use `sources`).
- **Digest**: `sha256:<hex>` over the entry as canonical JSON: the fields as parsed, keys sorted,
  no insignificant whitespace, UTF-8, strings as written. `sdc-goals digest <file> <id>` prints it;
  it is what a citation `goals@<rev>#<id>` carries (decision 3).
- **goal**: an outcome to move toward. **principle**: how to choose between options.
  **constraint**: a limit no answer may cross. **preference**: a taste default.
- **Layers** `company`, `product`, `request`, one file each. A lower layer adds entries and may
  override a higher `principle` or `preference` by naming it in `overrides`; the entry replaces the
  target for the requests that layer covers, and the target stays in its own file. It never
  overrides a `constraint` or a `goal`. The crate `goals/`, binary `sdc-goals`, does the rest:
  `check <file>...` (pass every layer: an `overrides` or `derived_from` target outside the given
  files is a finding) refuses that, unknown kinds, duplicate ids, a missing source, a prefix that
  differs from `layer`, and bad priorities or dates. Findings go to stderr, one per line,
  `<file>:<id>: <code>: <message>`; `--format json` adds a report with a digest per entry; exit 0
  valid, 1 findings, 2 usage or unreadable file.
- **Resolve**: `sdc-goals resolve --at <instant> <layer files>` prints one JSON document: the entries
  in their window at the instant (`valid_from` <= instant < `valid_until`; passed in, no clock),
  without a target that an entry in the window overrides, each with `layer`, `sources` and
  `digest`, ordered company, product, request. Ties between entries: priority, then the nearer layer
  (request over product over company).
- **Where it lives** *(inferred)*: company in the requester organisation's own repository, pinned by
  commit; product in the product repository as `.sdc/goals.yaml` (already a protected path,
  `autonomy-levels.md:374`, so no delivering agent edits it); request with the idea
  (`sdc run --goals FILE`, or a `goals:` block in a ticket). Brownfield: with no `.sdc/goals.yaml`,
  the product layer is imported from approved `vision:*`, active `initiative:*` and
  `product-requirements:*` of the repository's AEP store, each `sources: [aep:<id>@<rev>]`; imports
  count as rank 1 (a proposed default) until a person approves the layer.
- **How a run receives it**: `scripts/run-trial.sh --goals FILE` copies the file unchanged to the
  root case's `work/goals.yaml` when the run starts (`run-trial.sh:41-55`, `:142`); a goals
  document or the output of `sdc-goals resolve` both work. It is artifact `goals` beside `idea`
  (`protocol/sdc.protocol.yaml:10`, `sdc.bindings.json:4`), and also of `sdc.intake` and
  `sdc.direction` (`sdc-intake.protocol.yaml:10`, `sdc-direction.protocol.yaml:23`, their bindings
  `:4`). A child's `work/` is a link to its parent's (`driver/src/main.rs:506`), so intake and
  direction read the same file, and each case records the same revision: `t` and the first 16 hex
  digits of the file's sha256 (`main.rs:191-199`), `none` without `--goals`. No claim reads it and
  no invalidation names it: a revised goals document makes no intake or direction record stale.
  Ground: source `goals`, citation `artifact_revision` with an `entry` id; displayed
  `goals@<rev>#<id>`.
- **Invalidation**: an answer or decision carries the entry digest. If the entry's digest changed or
  the entry is gone, `questions.check` and `requirements.check` return `finding` (stale basis) and
  `amend.upstream` answers again (`trial-2.md:50-52`); open requests citing it are superseded and a
  prior is not reused (`decision-grounds.sh:68-81`). Other entries changing touch nothing.

## 3. The requester agent

| Side | Classes (`autonomy-levels.md:244-257`) | Person (P) | Agent (D cells) |
|---|---|---|---|
| requester | scope-changing answers, findings needing a person with `scope_changing: true`, context, vision, roadmap, release, close, budget overrun | the requester | `decider:requester` |
| delivering | spec acceptance, story activation, code review, merge, rollback, limit hit | the maintainer | `decider:reviewer`, `rule:<name>` |

- Context is the requester's: audience, platform, deployment and licence describe its world. Spec
  acceptance stays delivering: the ESS spec is checked against requirements the requester already
  decided, and the requester's acceptance points are release and close.
- One person can be both sides (the operator today); the split is in the data, not in who exists.
- **Receives** `in/request.json` (the contract's shape, from `story:decision-request-from-question`
  or `tools/decision-from-record.sh`), the subject file, every file a ground of the request cites,
  `idea.yaml`, `dissection.yaml`, `goals.yaml`, and `decisions.json` (its own and the requester
  person's decided requests of this run). Not requirements, spec, code or `checks.json` unless the
  request cites them.
- **Returns** `{option, answer, reasons, grounds, strength, no_ground_in_goals}`: an option letter,
  or a free `answer` for a question; every ground has source `goals`, `idea` or `prior_decision`
  and a `how` sentence. It goes through `decision-grounds.sh` like any LLM proposal (`decisions.md:138-140`).
- **Strength**: strong when an entry states the answer; moderate when constraints or principles rule
  out every other option; weak otherwise. An option crossing a `constraint` is never chosen.
- **No ground** (weak, none, or two entries of equal priority in conflict): D→P (A3, A4) escalates to
  the requester person; D\* (A5) takes the request's default, `no_ground_in_goals: true`, event
  `escalation_suppressed`. Under P and P+ the proposal is shown as the recommendation.
- **Independence**: its own record directory and principal, confined like the decider today (writes
  `out/` only); delivering agents never see its prompt, the requester never sees `impl/` or `spec/`
  except as a cited subject. A different model per side is `different_model`
  (`autonomy-levels.md:483-486`), optional.
- **Prompt** *(inferred)*: "You answer for the requester. Its goals are in in/goals.yaml. Choose the
  answer its entries support; cite each entry you rely on; never cross a constraint; say
  `no_ground_in_goals` when nothing covers the question." The decider's clause "escalate when the
  decision would change scope" (`decider.sh:185-187`) does not apply: scope is its job.

## 4. Goals in intake and the other stages

- **Interview**: a new intake action `questions.derive` *(inferred)*, run by the requester agent right
  after `questions.write`. For each open question an entry answers with strong or moderate grounds,
  it writes `answer`, `status: accepted|changed`, `answered_by: derived`, `derived_by:
  decider:requester`, `basis: [{entry, digest}]`. Non-scope questions: at every level (they are
  writer answers today, `llm-node.sh:148-155`). Scope-changing: only where the class holder is D;
  below, the derivation becomes the request's recommendation.
- **Check**: `check-questions` refuses `derived` with no basis, and a basis whose entry is missing
  or changed (`finding`). This closes the `sdc-tool.sh:65-67` gap.
- **Writers**: `questions.write` reads `goals.yaml` and does not set a default that crosses a
  constraint; `context.default` cites entries for each field it fills.
- **Consistency rules**: a `constraint` may carry `rule` in the `sdc-consistency-rules/1` shape; its
  side is `goals.<id>`, matched against requirement texts and context fields. `sdc-findings
  consistency` reads `goals.yaml`. Stage agents may raise a `contradiction` citing `goals.<id>`.
  Rank: a constraint is never amended; the other side is, by the agent when it is rank 0 or 1, by a
  person when it is rank 2 (a person's own answer, `trial-2.md:57-68`).

## 5. The person's place

The same request reaches the requester person at A0 to A2 (and on escalation). It shows, beyond the
cold-read epic's fields: the recommendation with its strength and `proposed_by: llm`
(`decider:requester`; none at A0, which calls no decider, `autonomy-levels.md:208`); per ground,
the entry statement, kind, layer and source, linked to its line in `goals.yaml`; "no goal covers
this" when `no_ground_in_goals`; and the answers derived without them since the last stop, with
their entries, in the digest (`autonomy-levels.md:328`). Reversing one is an edit of the answer (H3).

## 6. Eval and KPIs

- Per suite item `eval/personas/<item>/goals.yaml` (given to the run) and `truth.yaml` (hidden,
  outside the sandbox): per topic `about` (a span of the idea), `accept` (patterns), `derivable_from`
  (an entry id or null). Questions are matched to topics through their dissection item's span; an
  unmatched question is `unknown`, counted apart. Every eval runs at A5, so the requester is D\*.
- Proposed `docs/kpis.md` § 6 "Requester side" (not edited):

| id | KPI | formula | source | dir | gate |
|---|---|---|---|---|---|
| G1 | answered from goals without a person | questions `derived` or decided by `decider:requester` on a strong or moderate goals ground / questions answered | `questions.yaml`, `decisions.jsonl` | ↑ | advisory |
| G2 | requester agreement with the persona | answers matching `accept` / matched answers; covered (`derivable_from` set) and uncovered apart | eval report | ↑ | covered: Wilson lower bound ≥ 0.9 to admit scope answers to D (`autonomy-levels.md:527-532`) |
| G3 | answers with no ground | `no_ground_in_goals` / requester answers | `decisions.jsonl`, `questions.yaml` | ↓ | advisory; each is a missing entry |
| G4 | asked though a goal answered it | requests opened to any holder whose decided answer rests on a strong goals ground; in eval also `derivable_from` questions asked | same; `truth.yaml` | ↓, target 0 | ratchet |
| G5 | answers set aside by a goals change | stale-basis findings / answers resting on goals | `findings.yaml` | n/a | advisory |
| G6 | constraint findings | findings citing `goals.<id>`, by raiser and stage | `findings.yaml` | program share ↑ | advisory |

## 7. Layers

| Layer | Gets |
|---|---|
| Canon | nothing new: `goals` is one more artifact; a ground is an `artifact@revision` citation, which story:decision-grounds (canon wave 6) already covers (`decisions.md:294-296`) |
| Loom | the host passes the resolved goals document and, per decision class, a decider configuration (principal, inputs, prompt) and eligible principals per side: `upstream-blocker:loom-decider-per-class` |
| SDC | `sdc.goals/1`, `goals/` (`sdc-goals check|digest|resolve|ground`), `side` in the classes document, the requester prompt, `questions.derive`, the basis check, rules over constraints, the persona format, G1 to G6, the UI |

Contract additions in `ess/domains/decision.yaml` (story:decision-contract-goals-grounds):
`GroundSource` value `goals`; `ArtifactRevisionCitation.entry` and `.digest`; `Ground.supports`
optional (absent: the free answer); an optional free `answer` on `Decide` and
`recommended_answer` on `Recommend`, for question requests only (guards `option-or-answer`,
`answer-not-allowed`). The grounding check calls `sdc-goals ground` for a goals entry
(`decisions.md`, "The grounding rule").

## 8. Worked example: trial 2

Idea: "A to-do list web app for me and my partner that also works on the train without a
connection". Goals *(constructed for the example)*:

```yaml
format: sdc.goals/1
layer: company
requester: operator
entries:
  - {id: company/c1, kind: principle, priority: 1, statement: "Build the simplest thing that serves the named users; nothing for users nobody named.", sources: ["person:operator"], valid_from: 2026-11-21}
  - {id: company/c2, kind: constraint, statement: "Data stays on devices and servers we run; no third-party cloud, account or mail service.", sources: ["person:operator"], valid_from: 2026-11-21}
  - {id: company/c3, kind: principle, priority: 2, statement: "Every user signs in to an own account; no anonymous or link-based access.", sources: ["person:operator"], valid_from: 2026-11-21}
```

```yaml
format: sdc.goals/1
layer: product
requester: operator
entries:
  - {id: product/p1, kind: goal, priority: 1, statement: "Each of us keeps our own to-dos, organised in several lists.", sources: ["person:operator"], valid_from: 2026-11-21}
  - {id: product/p2, kind: constraint, statement: "Everything works offline; no edit is lost, also when both of us edit offline.", sources: ["person:operator"], valid_from: 2026-11-21}
```

Questions from `runs/trial-2-train/store/IDEA-1/children/IDEA-1.intake/work/questions.yaml`:

| q | scope | Requester answer | Grounds, strength | A2 | A3, A4 | A5 |
|---|---|---|---|---|---|---|
| q1 one shared list or separate shareable lists (`:2-12`) | yes | neither: "each user has several lists of their own; no shared list" | p1 states own lists; c1: sharing was not named. Strong | person, with this recommendation | derived | derived |
| q2 offline conflicts (`:13-22`) | no | "keep both changes and ask us to pick"; the default "last change wins" loses an edit | p2 rules out A and C (item merge loses a same-item edit). Moderate | derived | derived | derived |
| q3 sign-in (`:23-34`) | yes | "name and password from a fixed list of the two accounts; no registration, no mail" | c3 rules out the link; c2 the mailed code; c1 no registration. Moderate | person, with this recommendation | decides | decides |
| q4 browsers, installable (`:35-44`) | no | the default, "current browsers; not installable" | none covers it: `no_ground_in_goals` (G3 +1) | default | default | default |

q1 and q3 match the operator's own answers (`:11`, `:33`). Intake stops for no person at A3, and at A2
once, with recommendations naming the entries the operator lacked. Without c2, q3 is weak (only the link
is ruled out): A3 escalates to the operator, A5 takes the mailed-code default, `no_ground_in_goals`.

## 9. Plan

`epic:requester-side-and-goals-context`; stories (store-wide wave of `aep plan artifact waves
--status draft`, 2026-11-21): `goals-format-check` 1, `decision-contract-goals-grounds` 2,
`goals-root-input` 3, `requester-decider` 4, `constraint-consistency-rules` 5, `ui-goal-grounds` 6,
`interview-derived-from-goals` 7, `eval-requester-persona` 8, `requester-decider-loom` 8 (blocked by
`upstream-blocker:loom-decider-per-class`), `requester-kpis` 9; those in `driver/`, `scripts/`,
`protocol/` wait for the running trial.

## 10. Questions for the operator

1. Scope-changing answers a goal states, below A3. **a** shown to the person as a strong
   recommendation, one click (the design; the autonomy table unchanged); **b** derived without the
   person from A2, reported in the digest; **c** at every level. Recommendation: **a** until G2's
   covered agreement has admitted scope answers to D, then **b**.
2. Where trial 3's goals come from. **a** the operator writes about ten company and product entries;
   **b** the requester agent imports them from the AEP visions and `AGENTS.md` of his repositories,
   the operator approves; **c** start empty and grow from answered questions. Recommendation: **a**: small,
   and it tests the agent against goals a person actually holds.
