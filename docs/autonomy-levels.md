# Autonomy levels for SDC runs

Question (the operator, 2026-11-21): a user says "run the process, here's a ticket", "continue where you
left off", or gives one sentence of an idea, and sets a level of autonomy for the run. Take the
levels-of-autonomy scale the swarm-like "hive" project points at, and use it to decide, per
decision, where a person is involved and how much an AI decides.

Addition (the operator, 2026-11-21): eval runs always run at the highest level, with no person at any
decision (§ 3.4).

Read on 2026-11-21: `sdc/docs/{driver,kpis,layers,trial-1,eval-design}.md`,
`sdc/scripts/run-trial.sh`, `sdc/bindings/tools/{decider.sh,sdc-tool.sh}`, the eight
`sdc/protocol/*.protocol.yaml` (authority lines), `research/literature/papers/{2609.00252,
2611.06946,2611.10058,2609.24234}.md`, `research/literature/DISTILLATION.md`, and the web sources
in § 1. Searched without hits for a levels scale: `swarm/`, `agency/`,
Atlas's governed-autonomy design documents, `governed-autonomy/` (the only "hive" matches there are
inside "archive"). Nothing was built or run. *(inferred)* marks my construction or anything not
observed.

## 0. Short answer

- **The hive project** is `hivecommons/hive` (first published as `kubestellar/hive`, IBM Research,
  CNCF sandbox, bee logo). It enforces a six-level **AI Codebase Maturity Model (ACMM)** in code,
  and its policy matrix cites the paper: Anderson, "The AI Codebase Maturity Model: From Assisted
  Coding to Fully Autonomous Systems", arXiv 2604.09388 (§ 1.5). That paper cites Dan Shapiro's
  Five Levels (which borrows the driving levels), not Feng et al. A second hive, `aden-hive/hive`
  (queens, colonies, HoneyComb), cites no paper.
- **The scale adapted** is Feng, McDonald and Zhang, "Levels of Autonomy for AI Agents" (arXiv
  2506.12469): levels named by the user's role, and autonomy as a design choice made apart from
  capability. Three things come from elsewhere. Parasuraman, Sheridan and Wickens contribute one
  level per decision type, which becomes the per-class override. Sheridan and Verplank contribute
  "act, then inform", which sets what is reported afterwards. SAE J3016 contributes the
  operational domain and the minimal-risk stop, so hard limits end the run.
- **Proposed scale, A0 to A5** (§ 2). The prefix is `A` because `eval-design.md` § 1 already uses
  L0 to L5 for an agent node's freedom.
  - A0 Manual: a person takes every decision.
  - A1 Advised: the decider proposes and a person decides.
  - A2 Internal: the decider takes revisable internal decisions.
  - A3 To merge: the decider also takes scope and direction, and a person merges and releases.
  - A4 To release: the decider also merges, and a person releases.
  - A5 Unattended: no person at all. The decider is final, escalations are suppressed and counted,
    and limits end the run. Eval runs always use A5.
- **First decision for the operator** (§ 6, D1). `sdc.intake` requires every scope-changing answer to come
  from a human, and `questions.check` hard-codes `answered_by == "human"` (`sdc-tool.sh:55-56`).
  An A5 run, and so every eval run, therefore stops at intake whenever any question is
  scope-changing. trial-1's three questions all were.

## 1. Survey

| # | Scale | URL (opens on) | Levels |
|---|---|---|---|
| 1 | Sheridan and Verplank 1978 | https://apps.dtic.mil/sti/citations/ADA057655 (the DTIC record. It returned 403 here, so nothing was reached. NTIS has the same record: https://ntrl.ntis.gov/NTRL/dashboard/searchResults/titleDetail/ADA057655.xhtml, not opened) | 10 |
| 2 | Parasuraman, Sheridan and Wickens 2000 | https://ieeexplore.ieee.org/document/844354 (IEEE record. It returned empty here; the abstract was reached through search result text only) | 4 types × 10 levels |
| 3 | SAE J3016 (April 2021) | https://saemobilus.sae.org/content/j3016_202104 (the standard's record, paywalled, not opened). Reached: Koopman's guide, https://users.ece.cmu.edu/~koopman/j3016/index.html | 0 to 5 |
| 4 | Feng, McDonald and Zhang 2025 | https://arxiv.org/abs/2506.12469 (abstract). Full text read in v2: https://arxiv.org/html/2506.12469v2 | L1 to L5 |
| 5 | ACMM, Anderson 2026 (hive's paper) | https://arxiv.org/abs/2604.09388 (abstract). Full text read in the PDF, v2 | 1 to 6 |

### 1.1 Sheridan and Verplank: levels of automation of decision and action

The wording below is as Parasuraman et al. 2000 reproduce it (Table 1). It is quoted from memory,
because neither primary text was reached *(inferred wording)*. The computer:

1. offers no assistance, and the human takes all decisions and actions;
2. offers a complete set of alternatives;
3. narrows the set down to a few;
4. suggests one alternative;
5. executes that suggestion if the human approves;
6. allows the human a restricted time to veto before it executes;
7. executes automatically, then necessarily informs the human;
8. informs the human only if asked;
9. informs the human only if it decides to;
10. decides everything and acts autonomously, ignoring the human.

- Evidence: a taxonomy from undersea teleoperation research. It is a classification, not tested
  as a scale.
- What the SDC takes: levels 4 to 9 differ only in **when the person hears about a decision**. That
  becomes the "reported afterwards" column of § 2.

### 1.2 Parasuraman, Sheridan and Wickens: types and levels

- Four types of automation: information acquisition, information analysis, decision selection,
  action implementation. Each type can sit at its own level. Search text quoting the abstract:
  "A particular system can involve automation of all four types at different levels."
- Evaluation: the primary criterion is the human-performance consequences. Secondary criteria are
  automation reliability and "the costs of decision/action consequences" (abstract, via search).
- Evidence: a conceptual model built on earlier human-factors studies. Those studies cover
  workload, situation awareness, complacency and skill loss (from memory of the paper,
  *(inferred)*).
- What the SDC takes: **the level is set per decision class, not once per run** (§ 2.3). The cost
  of a wrong decision sets how high a class may go.

### 1.3 SAE J3016

| Level | Who drives | Who is fallback |
|---|---|---|
| 0 | human | human |
| 1 | human; feature sustains speed or steering | human |
| 2 | feature sustains both, human monitors | human |
| 3 | system, inside its operational design domain (ODD) | a "fallback-ready user" |
| 4 | system, inside its ODD | system: it reaches a minimal risk condition |
| 5 | system, anywhere on public roads | system |

- Evidence: a consensus terminology standard. Koopman: "a terminology standard, not a safety
  standard"; "Higher level numbers have more automation, but they might not be safer". Level 3's
  takeover by a person is a known weak point.
- What the SDC takes, *(inferred)* for each:
  - **An ODD**: a level holds only inside declared bounds (paths, budgets, apertures).
  - **No "person as fallback" at the top level**: a run that leaves its bounds stops in a safe
    state. It does not wait for a takeover.
  - **The level-3 lesson**: a level whose safety depends on a person who is not watching is the
    worst design. In the SDC, escalation stops the run; it never waits silently.

### 1.4 Feng, McDonald and Zhang: Levels of Autonomy for AI Agents

| Level | User role | Definition (v2) |
|---|---|---|
| L1 | operator | "the user is in charge at all times while the agent is available to provide support on-demand" |
| L2 | collaborator | "Both the agent and the user can plan, delegate, and execute tasks" |
| L3 | consultant | "The agent takes initiative in task planning and execution over extended time horizons"; the user gives feedback, not takeover |
| L4 | approver | "The user is only required to interact with the agent when the agent encounters a blocker it cannot resolve on its own"; approvals before consequential actions, user-configurable |
| L5 | observer | "does not require, and comes with no means for, user involvement"; an emergency off switch, activity logs |

- Autonomy is "the extent to which it is designed to act without user involvement", and it is a
  design choice: "a capable agent can still act with a low level of autonomy".
- Autonomy certificates: a third party issues "the maximum level of autonomy at which an agent can
  operate". A change of model, tools, prompts or environment can invalidate the certificate.
- **Assisted evaluation**: the agent first runs with no user. If it misses the success threshold,
  user involvement is added in rounds (L4-style approvals, then L3-style consultation), and the
  involvement is logged and classified afterwards.
- Evidence: an essay, with example systems classified from public demos as of June 2025. It
  reports no experiment.
- What the SDC takes: the role names; levels as a design choice, separate from model capability;
  the off switch at the top level; and the assisted evaluation, which becomes § 5.2.

### 1.5 The hive project and ACMM

- `hivecommons/hive`, "AI agent orchestration … from brainstorming to full autonomy" (GitHub
  description). `src/docs/acmm-policy-matrix.md:10` says: "Background: [the ACMM
  paper](https://arxiv.org/abs/2604.09388)".
- The paper's levels, from its Table 1:

  | Level | Name | Human role |
  |---|---|---|
  | 1 | Assisted | executor |
  | 2 | Instructed | rule-writer |
  | 3 | Measured | analyst |
  | 4 | Adaptive | governor |
  | 5 | Semi-automated, "the system proposes, humans approve" | operator |
  | 6 | Fully Autonomous, "agents act, merge, and roll back without human gating" | strategist |

- A level is defined by the feedback loops the codebase has. "You cannot skip levels." Level 6
  criteria include:
  - risk assessment config ("prevents autonomous changes to high-risk areas");
  - a rollback drill ("Autonomy without a kill switch is recklessness");
  - a dashboard of autonomous activity.
- Hive enforces the levels as packs. Each agent gets a mode:

  | Mode | What the agent may do |
  |---|---|
  | advisory | record findings |
  | measured | file issues |
  | holdgated | open PRs with a `hold` label that a human lifts |
  | full | merge on green CI, L6 only |

  More hive rules:
  - Even at L6, high-risk decision questions escalate: permissions, security, irreversible
    actions, external side effects.
  - The ioscan fail mode turns `closed` at L5 and L6, accepting that "every Critical
    false-positive becomes a stalled queue item".
  - Automatic promotion and demotion are off by default. When on: three qualifying retro findings
    promote one level, and a rollback demotes one level immediately.
  - "Approval does not prove the code is right" (README).
- Evidence: one maintainer's 100-day experience report on one dashboard project, self-reported.
  L6 had been in production for about one week ("36+ issues closed autonomously with zero
  production incidents"). The paper states survivorship bias and no independent validation
  (§ 6.7). Its related-work table also lists Shapiro's Five Levels, AI-SDLC, AIDMM and AI-MM SET,
  none of them validated.
- What the SDC takes:
  - a level is **earned by gates and eval evidence**, not declared;
  - **merge on green plus review** is the step from "proposes" to "acts";
  - **demote on rollback** at once, promote only on evidence;
  - **at higher levels tighten fail-closed**, not only loosen permissions.

### 1.6 Local research

| Paper | Finding | Grade | Use here |
|---|---|---|---|
| 2609.00252 F9, autonomy-level-per-task-class | set autonomy per class of work as an explicit, versioned, revisable decision (dependency patches autonomous, auth changes with a human-approved plan) | E0 | the per-class override (§ 2.3) |
| 2611.10058 F1 | autonomy ratings vary more by developer (27.6% of variance) than by activity (2.1%) | E3 | the level is a per-run, per-person choice, not only a property of the class |
| 2611.10058 F5 | developers who give more autonomy also approve actions without close review more often (48 of 77 pairs) | E3 | higher levels must not rely on a person rubber-stamping: the person's decisions move to fewer, larger ones (§ 2.2) |
| 2611.10058 F11, `bounded-observable-reversible-autonomy` | 59% of 175 answers name bounded, observable or reversible as conditions for granting more autonomy | E2 | classes are ordered by reversibility; every level reports (observable) and has hard limits (bounded) |
| 2611.06946 | L1 to L5 autonomy levels | none | "the author's own and unvalidated" (§ 5.7, § 8); not used |
| 2611.06946 F8 | protect test and gate files from agent edits | E0 | protected paths in the A4+ merge rule |
| 2609.24234 | developers' supervision stages, prompted with Sheridan's framework | E1 | the authors note that introducing Sheridan's framework "may have anchored" participants (§ 9) |

Not opened: Anthropic, "Measuring AI Agent Autonomy in Practice" (Feb 2026), cited by 2611.10058
as auto-approval and interruption data from Claude Code sessions. It is a lead for § 5.

## 2. The SDC scale

### 2.1 Levels

The decider is the LLM decider of `tools/decider.sh` (`SDC_DECIDER=model`, principal `decider:model` whatever the model; `haiku` is its old name, D149). A rule is a program
that decides deterministically from evidence.

| Level | Name | Adapts | Departs | One line |
|---|---|---|---|---|
| A0 | Manual | Feng L1 operator; Sheridan 1 | Feng's operator also drives the agent; here the agent nodes run regardless, and only decisions are the person's | every decision is the person's; no decider call |
| A1 | Advised | Feng L2 and L3; Sheridan 4 | the decider never acts, it only proposes | the decider proposes each decision with reasons; the person accepts or overrides; agreement is recorded (shadow data for § 5) |
| A2 | Internal | Feng L3 consultant; Sheridan 7 for decided classes | Feng splits by planning horizon; here the split is by reversibility | the decider takes internal, revisable decisions (context, spec acceptance, story activation, code review); the person takes scope, vision, roadmap, merge, release and operate |
| A3 | To merge | Feng L4 approver; ACMM L5; hive `holdgated` | | the decider also takes scope answers, vision and roadmap; the person merges, releases and operates; rollback becomes a rule |
| A4 | To release | ACMM L6; hive `full` | ACMM L6 also releases autonomously; here release stays the person's | the decider merges when the merge rule holds; the person releases |
| A5 | Unattended | Feng L5 observer; SAE 4 (inside the ODD, minimal risk condition) | Feng's observer still monitors; here nobody needs to | no person: the decider's decision is final, escalations are suppressed and counted, and hard limits end the run. **Eval runs always use A5** |

Departures from every source *(inferred)*:

- **Canon chooses the action, not the agent.** The SDC driver picks the next action
  deterministically (`driver.md` "Fixed policy"). Autonomy here is only about who **decides** at a
  decision point, never about who chooses the next step. The freedom of the agent nodes is a
  separate axis: `eval-design.md` § 1, L0 to L5.
- **A level is a ceiling and a default, not a certificate.** Per class, the eval decides how high
  a class may go (§ 5.2). A run may set lower holders per class, and higher ones only up to what
  the eval admitted (§ 2.3).
- **The program checks never move.** The ESS gate, tests and validation are rules at every level.
  Autonomy changes only the judgement decisions that sit on top of them.

### 2.2 Who decides, per decision class

Holders:

| Symbol | `sdc.decision.Holder` | Meaning |
|---|---|---|
| **P** | `person` | a person decides |
| **P+** | `advised` | a person decides, with the decider's proposal shown |
| **D→P** | `decider` | the decider decides; an escalate stops the run and goes to a person |
| **D\*** | `decider_final` | the decider decides; an escalate is suppressed, the decider's `best` is final, and the event is `escalation_suppressed` |
| **R**, **R, then P** | `rule` | a rule decides; an escalate goes to a person |
| **R: end** | `rule_final` | a rule decides; an escalate is refused and the run ends |
| **R∧D** | `rule_and_decider` (R∧D→P), `rule_and_decider_final` (R∧D\*) | the rule's verdict is a strong recommendation; the decider takes it or vetoes it and never grants against the rule. Under R∧D→P a veto is an Escalate; under R∧D\* it is a Decide on the denying option, and the run ends as refused with no `explicitly_abandoned` |
| **—** | none | does not arise |

| Class | Canon action, capability (today) | A0 | A1 | A2 | A3 | A4 | A5 |
|---|---|---|---|---|---|---|---|
| scope-changing answers | intake `questions.answer` on a `scope_changing` question; the human rule is hard-coded in `questions.check` (`sdc-tool.sh:55-56`); no capability | P | P+ | P | D→P | D→P | D* |
| context | direction `context.approve`, `owner.approve` | P | P+ | D→P | D→P | D→P | D* (`SDC_RULE_APPROVALS=on`: R, `context.check` passed at this revision: `rule:autopilot`; else D*) |
| vision | direction `vision.approve`, `owner.approve` | P | P+ | P | D→P | D→P | D* |
| roadmap | direction `roadmap.approve`, `owner.approve` | P | P+ | P | D→P | D→P | D* (`SDC_RULE_APPROVALS=on`: R, `roadmap.validate` passed at this revision: `rule:autopilot`; else D*) |
| spec acceptance | spec `spec.review` → `spec_review`, `independent_on: [different_principal]` (`sdc-spec.protocol.yaml:108`); ESS gate always R | P | P+ | D→P | D→P | D→P | D* |
| story activation | none: the decompose stub moves stories to active (`driver.md` gap 11) | P | P+ | D→P | D→P | D→P | D* |
| code review | story `change.review` → `code_review`, `different_principal`; tests always R | P | P+ | D→P | D→P | D→P | D* |
| merge | story `change.merge`, `change.merge` | P | P+ | P | P | R∧D→P | R∧D* |
| release | release `release.approve`, `release.approve` | P | P+ | P | P | P | D* (`SDC_RULE_APPROVALS=on`: R, `spec.gate`, `release.build` and `acceptance.run` passed at this revision: `rule:autopilot`; else D*) |
| operate: rollback | release `release.rollback`, `release.approve` (unbound today) | P | P+ | P | R | R | R: end |
| operate: close | root close `operating` | P | P+ | P | P | D→P | R (D140: its claims are settled upstream; `rule:autopilot` closes) |
| budget overrun | driver caps: `--max-llm-calls`, `SDC_MAX_DECIDER_CALLS`, USD | P | P | P | R, then P | R, then P | R: end |
| limit hit | `--max-wall-seconds`, `--max-steps`, `timeout_s`, Substrate refusals | R, then P | R, then P | R, then P | R, then P | R, then P | R: end |
| escalation | decider reply `escalate` or an invalid reply | — | — | P | P | P | D* |

The rule for context, roadmap and release is built (`story:decider-program-approvals`,
`driver.md` "Approvals by program", data `rule-ends.json`) behind `SDC_RULE_APPROVALS=on`, off by
default until the experiment `decider-program-approvals` holds; vision has no program check and
stays D*.

Rules named in the table *(inferred: proposed, not built)*:

- **Merge rule (A4, A5).** All of these must hold for the rule's verdict to grant; the decider
  takes the verdict or vetoes it (R∧D above):
  - `conformance_run` is `passed`;
  - `code_review` is `approved` by an independent principal;
  - the diff stays inside the story's AEP scope paths (`aep plan artifact scope`);
  - no protected path is touched: test and gate files, the run's `protected` globs (2611.06946
    F8).
- **Rollback rule (A3 and up).** `release.observe` gives `health_observation` not `healthy` →
  `release.rollback`. This runs in the safe direction, so it is a rule even where release is the
  person's (Sheridan 7: act, then inform).
- **Budget rule (A3, A4).** On the first overrun, extend once by `limits.extend.by`, never past
  `limits.hard`. The second overrun stops the run for a person.
- **Limits are never the decider's.** A limit hit is enforced by Loom and Substrate. No LLM can
  raise one, at any level.
- **Reject at A5.** A decider `reject` ends the run as refused (`run-trial.sh` exit 6), with the
  decision recorded. It records no `explicitly_abandoned`: abandoning stays a person's decision.
- **No usable result under a final holder.** Under `decider_final` and `rule_and_decider_final`,
  a decider result that is missing, failed, or withheld by the grounding check decides the
  denying option; where no option denies, the run ends. `escalation_suppressed` records the cause.
- **A withheld rule verdict.** Under `rule_and_decider`, a rule verdict the grounding check
  withholds counts as no recommendation: the request goes to a person. Under
  `rule_and_decider_final` the run ends.

Person decisions per run, from trial-1's "Where you act" table, with k scope-changing questions,
e epics and s stories *(inferred counts)*:

| Level | Person decisions |
|---|---|
| A0, A1 | k + 3 + 2e + 2s + s + e + 1 (scope, direction, spec and activation per epic, review and merge per story, release, close) |
| A2 | k + 3 + s + e + 1 |
| A3 | s + e + 1 |
| A4 | e |
| A5 | 0 |

Escalations come on top of these at A2 to A4.

### 2.3 Per-run overrides (2609.00252 F9)

`overrides` maps a class to a holder, written as the `sdc.decision.Holder` value (`person`,
`advised`, `decider`, `decider_final`, `rule`, `rule_final`, `rule_and_decider`,
`rule_and_decider_final`). The holders form a partial order, lowest first:

- `person` < `advised` < `rule_and_decider` < `rule` and `decider` (these two are not comparable);
- each non-final holder < its final variant (`decider` < `decider_final`, `rule` < `rule_final`,
  `rule_and_decider` < `rule_and_decider_final`);
- `rule_and_decider_final` < `rule_final` and `decider_final`.

A move down in this order is a lowering. Every other move, including one between two holders
that are not comparable (such as `rule` to `decider`), is a raise. The rules:

1. **Lowering is always allowed.** For example, at A4 set `vision: person`.
2. **Raising a class above the level's preset is allowed only up to that class's admitted holder.**
   Admitted holders sit in `eval/admitted.yaml`, which the eval of § 5.2 writes and a person
   commits. Without an entry the ceiling is the level preset. Example: at A3, `merge: decider`
   only once merge has been admitted for D.
3. **`limit hit` has no override**: it is always the rule.
4. **Eval runs** (`--eval`, § 3.4) refuse any override naming a holder that is not final (`person`, `advised`, `decider`, `rule`, `rule_and_decider`), exit 64: at A5 every cell is a final holder, so no decision waits on a person.
5. **ODD by path, `escalate_when`.** A class decided by the decider is sent to a person when a
   deterministic condition holds. The F9 example is any change under `**/auth/**`. At A5 such a
   condition ends the run instead (minimal risk condition, SAE).

### 2.4 What is reported to the person afterwards

| Level | When | What |
|---|---|---|
| A0 | at each decision | the decision request (today's `next` message and `sdc-drive approve` command) |
| A1 | at each decision | the same, plus the decider's proposal, reasons and cites; after the decision, agreement yes or no in `decisions.jsonl` |
| A2 to A4 | at every stop for a person, and at the end | a **decision digest** since the last contact. Per decision: class, case, subject at revision, holder, decision, reasons, cites, cost. Plus escalations with their reasons, budget extensions and limit hits (Sheridan 7: act, then necessarily inform). The person can reverse a decision by revising the subject (`context.change`, a new roadmap revision); reversals are counted (R23) |
| A4 additionally | at the release stop | the merge report per story: diff stat, scope fence result, test and review records, decider reasons |
| A5 | at the end only | the run report: every decision, every suppressed escalation with the decider's `best` and reasons, every limit hit and budget event, the KPIs of § 5. Nothing is pushed during the run (Sheridan 8: on request, `sdc-drive status`) |

### 2.5 Loom and Substrate limits per level

| Limit | A0, A1 | A2 | A3 | A4 | A5 |
|---|---|---|---|---|---|
| Confinement of agent nodes and the decider (`revises` paths only, no egress, Substrate profile) | always | always | always | always | always |
| Run budget (LLM calls, USD, wall) | small: a person is there at each stop | small | medium, one extension by rule | medium, one extension by rule | hard and final; reaching it ends the run (`budget_spent`) |
| Kill switch (`sdc-drive stop`, lock) | yes | yes | yes | yes | yes (Feng L5 off switch) |
| Fail mode on a confinement refusal, or an injection finding in outside text (ticket, issue) | stop, ask | stop, ask | fail closed: branch stops, event | fail closed | fail closed: run ends |
| Merge aperture (write `mainline/`, push a branch) granted to the runtime's effect | after a person's grant | same | same | after the merge rule | after the merge rule |
| Protected paths enforced at merge | review by person | same | same | rule (Substrate sealed paths for agents, plus the merge rule) | rule |
| Release aperture (`published/`, deploy credentials through a secret slot) | after a person's grant | same | same | same | only to targets the run's profile names; eval: sandbox target only, never production |
| Rollback bound and drilled before release (ACMM L6 criterion) | no | no | required for the rollback rule | required | required |

The direction is *(inferred)*: permissions loosen with the level, and fail-closed behaviour
tightens with it (hive ioscan, § 1.5). Who decides never changes which effect is allowed. It only
changes who holds the grant that admits the effect.

## 3. As data: `sdc.autonomy/1`

### 3.1 Document

```yaml
format: sdc.autonomy/1
level: A3
decider:
  kind: llm
  model: claude-haiku-5-5
  adapter: metaharness
  max_turns: 8
  max_usd_per_decision: 0.25
  max_calls: 20
escalation:
  mode: person
  person: operator
  escalate_when:
    - decider: escalate
    - decider: invalid
    - paths_touched: ["**/auth/**", "**/migrations/**"]
    - decision_cost_usd_over: 1.0
overrides:
  vision: person
  merge: person
protected: ["**/tests/**", "Taskfile.yml", ".sdc/**"]
limits:
  max_llm_calls: 60
  max_wall_seconds: 14400
  max_usd: 10
  extend: {by: 0.5, times: 1}
  hard: {max_llm_calls: 120, max_usd: 20, max_wall_seconds: 28800}
report:
  at: stop
```

- `escalation.mode`: `person` (A2 to A4) or `final` (A5: suppress and take `best`). A0 and A1 have
  no decider escalation. Under `person` and `advised` an Escalate is a person passing the request
  on to someone else.
- `report.at`: `decision` (A0, A1), `stop` (A2 to A4), `end` (A5).
- The level presets of § 2.2 to § 2.5 are SDC data in a second document,
  `protocol/autonomy-levels.yaml` (`format: sdc.autonomy-levels/1`): per level, a holder per class
  plus the limit defaults *(inferred file)*. The runtime resolves preset ⊕ overrides ⊕
  `eval/admitted.yaml` ceilings into one grant table per run and logs it as the event
  `autonomy_resolved`.

### 3.2 Decision classes, as SDC data

```yaml
format: sdc.decision-classes/1
classes:
  scope_changing_answer: {type: question, actions: ["sdc.intake/questions.answer"], when: "question.scope_changing", capability: scope.answer}
  context:               {type: approval, actions: ["sdc.direction/context.approve"], capability: context.approve}
  vision:                {type: approval, actions: ["sdc.direction/vision.approve"], capability: vision.approve}
  roadmap:               {type: approval, actions: ["sdc.direction/roadmap.approve"], capability: roadmap.approve}
  spec_acceptance:       {type: review, actions: ["sdc.spec/spec.review"], independent: true}
  story_activation:      {type: approval, actions: ["sdc.plan/stories.activate"], capability: plan.approve}
  code_review:           {type: review, actions: ["sdc.story/change.review"], independent: true}
  merge:                 {type: approval, actions: ["sdc.story/change.merge"], capability: change.merge}
  release:               {type: approval, actions: ["sdc.release/release.approve"], capability: release.approve}
  rollback:              {type: approval, actions: ["sdc.release/release.rollback"], capability: release.rollback}
  close:                 {type: close, actions: ["sdc/close:operating"]}
  budget_overrun:        {type: control}
  limit_hit:             {type: control}
  escalation:            {type: control}
```

The keys are the `sdc.decision.DecisionClass` values (`ess/domains/decision.yaml`). `escalation`
is not a request class: Escalate moves an existing request, and the `escalation` row of § 2.2
gives the holder of a request after Escalate. `budget_overrun` and `limit_hit` are Loom
built-ins; SDC lists them so its table is complete.

The capabilities `scope.answer`, `context.approve`, `vision.approve`, `roadmap.approve`,
`plan.approve` and `release.rollback` and the action `stories.activate` do not exist yet. They are
the Canon-side proposals of § 4.

### 3.3 Run commands and entry modes

```console
$ sdc run --autonomy A3 --idea "I want to build a to-do list web application"
$ sdc run --autonomy A4 --ticket gh:owner/repo#123
$ sdc run --autonomy A2 --ticket ./tickets/DEV-42.md
$ sdc run --continue trial-todo-1
$ sdc run --continue trial-todo-1 --autonomy A4 --override merge=person
$ sdc run --autonomy-file run.autonomy.yaml --idea "…"
$ sdc run --eval --idea-file eval/ideas/billing.yaml
```

| Entry | Opens | Notes *(inferred)* |
|---|---|---|
| `--idea "<sentence>"` | root case with `idea.yaml` = the sentence | today: `scripts/run-trial.sh <name> "<idea>"` |
| `--ticket <ref>` | root case with `idea.yaml` from the ticket's title and body, plus `source: <ref>` | Ticket text is outside input: it is scanned for injection before an agent reads it, and the scan fails closed from A3 up. Answers the ticket already gives are recorded as `answered_by: ticket:<author>`, which counts as a person's answer only if that author holds `scope.answer` |
| `--continue <run>` | resumes the tree | Without `--autonomy` the stored level holds. A change is recorded as an event `autonomy_changed {from, to, by}`. Lowering applies at once. Raising applies only to decisions not yet taken. Grants already given stay, bound to their revision |
| `--eval` | as `--idea`/`--ticket` | forces A5, refuses `person` and `advised` overrides, and reports into the eval report |

Mapping to today's switches:

| Level | Switches |
|---|---|
| A0 | `SDC_DECIDER=person` |
| A5 | `SDC_DECIDER=model SDC_DECIDER_ESCALATE=final`; built 2026-11-21 as `SDC_AUTONOMY=A5` or `scripts/run-trial.sh … --autopilot`: refuses `SDC_DECIDER=person`, sets escalate final, the decider takes scope answers and findings, and a stop that needs a person ends the run (exit 7, stop record by `rule:autopilot`) |
| A1 to A4 | `run-trial.sh` does not route by class (`SDC_AUTONOMY=A1`..`A4` is recorded and the run behaves as `SDC_DECIDER` says). `runbook run` does: the decider takes a decision of a class the table of § 2.2 gives to `decider`, read from `tools/autonomy-levels.json` (`sdc.autonomy-levels/1`: per level a holder per class, kept equal to § 2.2 by a test), and every other class stops the run for a person. The merge at A4 (`rule_and_decider`) is not the decider's alone, because the merge rule is not built. A missing or unreadable table stops every decision for a person. See `docs/loom-run-decisions.md` |

`SDC_AUTONOMY=A3` is the environment form. The level, where it came from (`SDC_AUTONOMY`,
`--autopilot`, or derived: person A0, a final decider A5, otherwise none) and any stand-in are
written into each `run-settings.jsonl` line.

### 3.4 Eval runs: A5, no person

- Every eval run runs at A5. No decision, in any class, waits on a person.
- Where a lower level would escalate, the **decider's decision is final**: its `best` (approve or
  reject) is taken. An invalid reply is asked again (2 retries), then takes the fallback of its
  kind (`docs/driver.md`, "Decider replies"); it never counts as a reject by itself.
- **A rejected approval in autopilot is not an end** (operator's rule, 2026-11-21). The rejection
  is recorded with its reasons and citations; the action that writes the rejected artifact
  (`change.implement` for `merge.approve`; `context.default`, `vision.write` and `roadmap.write`
  for the direction approvals) runs again with the reasons in its inputs (`in/rejection.json`),
  its checks run again on the new revision, and the approval is asked again. At most 2 rejections (the rework cap, D184:
  `SDC_MAX_REJECTIONS` when set, else `rework_limit` of rule-ends.json; recorded in run-settings.jsonl) are reworked
  per approval per case: the next one (by default the third) stops the run by `rule:autopilot` with cause `rework_spent`,
  naming the approval and every reason. A rejected `release.publish` (the capability
  `release.approve`, no writer) ends the tree instead: the release is withheld, the epic ends
  `not_released`, the root `not_delivered`, and the run is closed, not stopped (`docs/driver.md`,
  "A refused release"). A reject that has neither a writer nor such an end stops with
  `rejected_without_rework_path`. No message says a person decides.
- **Every would-be escalation is recorded** as the event `escalation_suppressed`, with the class,
  the subject at revision, the reasons and `best`. It is counted as the KPI R21, escalations
  suppressed (§ 5).
- **Hard limits end the run instead of a person**:
  - the budget (`max_llm_calls`, `max_usd`, decider calls);
  - wall time;
  - the step budget;
  - Substrate limits (confinement refusals, egress denials, timeouts).

  The run's verdict names the limit, and the limit hit is counted (R24). No limit is extended.
- An `escalate_when` condition (§ 2.3) also ends the run, as the minimal risk condition.
- Release goes only to the eval's sandbox target.
- Blocker today: the intake human rule (§ 6, D1).

## 4. Canon, runtime, SDC: who owns what

| Part | Owner | Content |
|---|---|---|
| Which capability each decision needs; which reviews need independence | **Canon** (the protocols' `requires: capability` and `independent_on`) | Canon knows principals and capabilities only, never "person" or "LLM" or a level. Requested: grants bound to `case_revision` and `principal` (`driver.md` gap 1). Proposed: split `owner.approve` into `context.approve`, `vision.approve` and `roadmap.approve`, so a level can hand out one without the others. Today one grant admits all three. Also proposed: a `scope.answer` capability on `questions.answer` in place of the hard-coded `answered_by == "human"` (D1); a `stories.activate` action under `plan.approve` (gap 11); a separate `release.rollback` capability, so the rollback rule can hold it without holding `release.approve` |
| Who holds each capability at a level; issuing grants; escalation; budgets and limits; events | **Runtime**: `sdc-drive` plus `run-trial.sh` today, Loom later (Mandate owns approval, Substrate enforces limits, `layers.md` § 5) | Resolves `sdc.autonomy/1` into a grant table. Grants as principal `decider:<model>`, `rule:<id>` or `person:<id>`. Runs the decider confined. Enforces the merge and rollback rules. Records `autonomy_resolved`, `decided`, `escalated`, `escalation_suppressed`, `limit_hit` and `autonomy_changed`. Chooses the Substrate profile and apertures per level (§ 2.5) |
| Decision request types and the resolver (preset ⊕ overrides ⊕ admitted ceilings into one holder per class) | **Loom** | the `sdc.decision` types (`ess/domains/decision.yaml`) move into Loom. In Loom a decision class is an identifier validated against the classes document the host passes in; `budget_overrun` and `limit_hit` are built-ins. Eligible principals are handed in by the host. The inbox is a store plus CLI and SDK calls, not a service |
| Level presets, decision classes, decider prompts per class, `escalate_when` defaults, KPIs, admitted ceilings | **SDC** (data) | the preset table, the classes document and the admitted ceilings are documents the host passes to Loom; SDC ships its own: `protocol/autonomy-levels.yaml`, `sdc.decision-classes/1`, `eval/admitted.yaml`. Also `tools/decider.sh` prompts and `docs/kpis.md` rows |

Independence *(inferred)*: Canon's `different_principal` is satisfied by `decider:haiku` against
an implementing `llm-node` session running the same model. A review by the same model family is
weaker independence. A later dimension `different_model` would let a level require it for spec
acceptance and code review.

## 5. KPIs per level

### 5.1 What should move

New rows proposed for `kpis.md` § 1b *(inferred definitions)*:

| id | KPI | Formula | Source | dir |
|---|---|---|---|---|
| R18 | autonomous run time | the longest interval, and the share of wall time, in which the run proceeds without waiting on a person | `awaiting_person` and `awaiting_authority` events against run start and end | ↑ with the level |
| R19 | person decisions | decisions with holder P or P+ per run (the § 2.2 count plus escalations); splits R8 into required and unplanned | `decisions.jsonl` `principal` | ↓ with the level |
| R20 | escalation rate | decider escalates / decider decisions, per class | `decisions.jsonl` | ↓; a class with a high rate is not ready for D |
| R21 | escalations suppressed | `escalation_suppressed` events per run, per class (A5 and eval) | events | ↓; reported always, gated never |
| R22 | decider agreement | decider proposal = person decision / proposals, per class (A1 shadow, and the eval's gold decisions) | `decisions.jsonl` | ↑ |
| R23 | decisions reversed | decider decisions a person later reversed (subject revised, merge reverted, rollback) / decider decisions | events, revisions | ↓ |
| R24 | limit hits | runs ending on a hard limit / N, by limit | verdict events | ↓ |
| R25 | decider cost | Σ decider `cost_usd` per run, and its share of R6 | `decisions.jsonl` | ↓, advisory |

| Rising from A0 to A5 | Should move | Must not move beyond margin |
|---|---|---|
| R8 and R19 person decisions | ↓ to 0 at A5 | — |
| R18 autonomous run time | ↑ to the whole run at A5 | — |
| R7 wall time | ↓ (no waiting) | — |
| R20, R21 | appear from A2 | — |
| R25 | ↑ (decider calls) | — |
| R1 verified success, R2 false done, R3 regression, R10 stuck, W4 review rejection | — | held; this is what "safe" means |
| R23 reversals, R24 limit hits | — | held at a low level per class |

### 5.2 The comparison that admits a class to a holder

This is Feng's assisted evaluation, turned around: start at A5 and step down per class *(inferred
design)*.

1. **Gold decisions.** Every eval idea or ticket gets a gold answer per decision point: scope
   answers, approve or reject per artifact revision, merge or no merge. The gold comes from
   fixtures and the hidden oracle (`eval-design` § 4). The adversary variants
   (`run-adversaries.sh`: `story-readme`, `spec-overreach`, …) must be rejected.
2. **Paired cells.** Class c is decided by the decider (D*) in one cell and by the gold
   ("scripted person") in the other. Every other class is held at gold. Same ideas, same seeds,
   N ≥ `trials` (takt rule).
3. **Admit c to D→P** when all of these hold:
   - the Wilson lower bound of R22 against gold is at least 0.9 for c;
   - zero approvals of an adversary variant in c;
   - R1, R2, R3 and R10 are no worse than the gold cell beyond the margin;
   - escalations fall on the disagreements: the share of R20 escalations on cases where the
     decider's `best` differs from gold is at least ½.
4. **Admit c to D\*** when the same holds with escalation suppressed. That is the A5 cell itself.
5. **Irreversible classes** (merge outside a sandbox, release, operate) also need a zero false
   approval rate over at least 20 adversarial cases *(inferred threshold)*. Before release goes to
   the decider, the rollback drill must have run.
6. **Write `eval/admitted.yaml`** {class → highest admitted holder, model, prompt digest, protocol
   digests}. A change of model, decider prompt or protocol invalidates the entry, as Feng's
   certificates do. A rollback or a person's reversal (R23) in a real run demotes that class by
   one holder at once (hive rule); promotion happens only through a new eval.
7. **A1 shadow data**: real runs at A1 give R22 against a real person at no extra cost to the
   person. These are the field data that confirm or contradict the eval cells.

Cost: one decider call per decision point per run, at most $0.25 by cap. 13 classes × 5 runs ×
2 cells × (5 to 10 decisions) stays within the pilot budget order of `eval-design` D7
*(inferred)*.

## 6. Open decisions for the operator

| # | Decision | Options | Recommendation |
|---|---|---|---|
| D1 | Scope-changing answers at A3 and up, and in every eval run: `questions.check` accepts only `answered_by: human` (`sdc-tool.sh:55-56`, mirrored in the `sdc.intake` description), so a decider's answer leaves the interview `needs_human` and an A5 run stops at intake | **a** Canon: replace the human rule with a capability `scope.answer` on `questions.answer`; the check accepts any answer whose `answered_by` principal holds it; the runtime grants it per level (protocol revision of sdc.intake plus the check); **b** keep the human rule; eval runs answer scope questions from gold fixtures as a scripted person, so evals are not person-free for this class; **c** the check reads the run's autonomy document and accepts `decider:*` at A3 and up (protocol unchanged, runtime data leaks into a check) | **a**: it is the same pattern as every other approval, it keeps Canon level-agnostic, and it makes the operator's "no person at any decision" true for evals |
| D2 | The level prefix | **a** `A0` to `A5` (here); **b** `L0` to `L5` as in the brief, renaming `eval-design`'s freedom ladder to `F0` to `F5` | **a**: no renaming of a written design |
| D3 | Default level when a user gives none | **a** A0; **b** A2; **c** A3 | **b** until § 5.2 admits classes; then the highest level whose classes are all admitted |
| D4 | Split `owner.approve` into three capabilities (§ 4) | **a** yes, a Canon request with the revision binding; **b** keep one, the runtime routes by action | **a**: A2 hands out context without vision |
