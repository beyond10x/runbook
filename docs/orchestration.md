# Orchestration as a protocol, parallelism, end conditions, brownfield

Question (the operator, 2026-11-21): can the orchestration the main `sdc` session does today (dispatch
agents, start sessions per repository, relay story ids, rank work, decide, ask the owner only on
real forks) be a Canon protocol; is parallelism in the protocols; is a run one task or a loop;
how does the SDC apply to an existing project; what does that do to the UI.

Read on 2026-11-21: `sdc/docs/*`, the eight `sdc/protocol/*.protocol.yaml`,
`sdc/driver/src/main.rs`, the trial and full-cycle scripts of `sdc/scripts/`, `sdc/scripts/check-protocols.sh`,
`sdc/ui/{README.md,web/index.html,web/now.js}`; Canon `website/docs/reference/protocol.md`,
`crates/canon/src/check/mod.rs`; Loom `crates/loom-governor/Cargo.toml`, a grep of `crates/` for
child cases, and the planning epics `protocol-tree-on-bindings` and
`tree-daemon-under-enforced-limits` (only on branch `wave/2026-11-21-w1`, worktree
`~/.local/state/worktree/trees/b10x/loom/loom-sdc-plan`, below `<loom-plan>`); agentplugins
`origin/main` (local `main` is 33 commits behind): `aep/skills/implementing/{SKILL.md,
references/{wave,drive,branch-and-merge,story-scoper}.md}` (below `W`, `D`, `BM`, `SS`),
`aep/skills/planning/references/{plan-critic-parallel-safety,reverse-engineer}.md` (`PC`, `RE`),
`ess/skills/retrofitting/SKILL.md` (`R`). Agentplugins, Canon internals and the Loom epics were
read by helper agents; their citations are relayed, the SDC ones were read directly. *Inferred*
marks what was not observed. The draft protocol is
`protocol/draft/orchestrate.protocol.yaml` (§ 6). Nothing outside `docs/orchestration.md` and
`protocol/draft/` was changed; the brownfield experiment only read `decider` and wrote its
compiled outputs to the session scratch directory.

## 0. Answers in brief

1. **Orchestration as a protocol: yes.** One standing case per area of activity (repository or
   repository group with one planning store), `sdc.orchestrate`: an LLM routes intake and proposes
   a ranked wave, programs admit and integrate it, the holder of `wave.approve` (person or decider
   by autonomy level) approves, the runtime opens one child case per work item. Checked: 49,152
   states, 0 findings (§ 2, § 6).
2. **Parallelism: declared, not executed.** The protocols open siblings together (`sdc.epic`
   `stories.open`, `sdc` `epics.open`), but the driver drives them one at a time, depth first,
   under one lock and one shared `work/`; Loom has no child cases at all yet, and its planned
   `tree run` is sequential too (§ 1).
3. **Scope: today one idea, ended once.** The root ends `operating` after one pass and never looks
   again. Proposed: bounded cases end `done`; a standing case never ends done, it is *quiescent*
   (nothing to triage, nothing accepted and undelivered, the release observed healthy within its
   max age) and becomes active again on new evidence; only `retired` ends it (§ 3).
4. **Brownfield: partly applies.** Entry points are idea, ticket, planning-store id, "continue";
   intake is reconstructed with citations from the store, spec and code instead of interviewed.
   Dry run on `decider`: the SDC would start at `sdc.spec` of `epic:decider-v1`, and its ESS gate
   fails today (strict ESS pin, 45 synthesis refusals); no roadmap exists; the three stories form
   a strict chain (§ 4). Rating 2 of 5.
5. **UI: from one tree to a portfolio board.** Lanes per work item under the area, an admission
   and queue strip, the orchestrator's own case and decisions, quiescence and health as the
   header state (§ 5).

## 1. Current state (verified)

### 1.1 Parallelism

| Where | What it does | Cite |
|---|---|---|
| Protocol `sdc` | `epics.open` opens one `sdc.epic` per roadmap epic from one action, argument `epics: [..]` | `protocol/sdc.protocol.yaml:115-137` |
| Protocol `sdc.epic` | `stories.open` opens one `sdc.story` per AEP story, "opened together" | `protocol/sdc-epic.protocol.yaml:6`, `:152-178` |
| Bindings | `child.each: "stories"` / `"epics"`: one child per argument entry | `driver/examples/sdc/sdc-epic.bindings.json:58-62`; `sdc.bindings.json:45-46` |
| Driver: open | `open_children` creates every child at once and writes one `case_outcome` `open` per child | `driver/src/main.rs:446-478` |
| Driver: run | `drive` walks `children()` in sorted order, recursing depth first; a child that is not terminable with exactly one outcome **stops the whole run**; only then one step in the parent | `main.rs:791-826`, sort `:288-293` |
| Driver: step | one action per step, the first `useful` one | `main.rs:602-614` |
| Driver: lock | one writer per case tree, a lock file with the pid | `main.rs:108-138` |
| Driver: workspace | all cases of a tree share the root's `work/` | `main.rs:235-237`; `docs/driver.md:505`, `:520-522` (gap 22: "stories run one after another") |
| Trial scripts | the trial script and the full-cycle script: a sequential loop of driver `run` rounds (40 / 30), no `&`, no `wait` | lines 238-240 of the trial script; lines 45-47 of the full-cycle script (both deleted since) |
| Canon | several children may be open; the `children` form is `unknown` while any is `open`, `false` for good once one ends outside the listed set | Canon `website/docs/reference/protocol.md:188-195`, `:328-339` |
| Loom today | no child cases: pins Canon 0.8.0 (`case_open` arrived in 0.9.0); no `case_open`/`case_outcome` in `crates/` | `loom/crates/loom-governor/Cargo.toml:15`; grep 2026-11-21 (3 hits, all unrelated names) |
| Loom planned | `story:child-case-runtime` opens several children; `tree run` drives "every open child before its parent's next step", one writer per store: sequential (*inferred* from wording). `story:tree-daemon`: many **trees** at once, run slots, a waiting tree holds no slot | `<loom-plan>/story/tree-run-command.md:61-81`; `tree-daemon.md:6-7`, `:23-27` |

Conclusion: siblings are a protocol fact, not an execution fact. No code anywhere runs two child
cases of one tree concurrently, and the shared `work/` would make it unsafe if it did. The only
parallelism that exists in practice is the AEP wave (one implementor per story, its own worktree,
N default 4: `W:408-414`, `:487-492`), run by instruction, outside Canon.

### 1.2 End conditions

- Root `sdc`: outcome `operating` when the roadmap is approved, epics opened and every child
  ended `passed`; or `abandoned` by decision (`sdc.protocol.yaml:143-154`). `operating` is a
  terminal outcome: after it the case is closed (`main.rs:486-511`) and nothing reads it again.
- `sdc.release` `passed` requires `release.healthy` (`sdc-release.protocol.yaml:184-195`), a
  `health_observation` with `max_age: 30m` (`:37`). The expiry is real, but nothing re-observes
  after the case closes: the 30 minutes only matter until `passed` is taken.
- Driver run verdicts: `terminable`, `awaiting_person`, `awaiting_authority`, `stuck`
  (`main.rs:617-630`), `step_budget_spent`, `wall_time_spent` (`:809-817`). Per action
  `max_runs` (default 8) counts every `action_finished` of the case's whole life (`main.rs:366-368`):
  a case that lives for months would exhaust it. Fine for one pass, wrong for a standing case.
- Canon has no standing-case construct: `termination` is optional and absent while open
  (Canon `documents.md:24`); outcomes are terminal (`protocol.md:97`); obligations are discharged
  only while `discharged_when` is true (`evaluation.md:228-233`).
- Loom's governor completes a case when exactly one outcome is legitimate
  (`loom/AGENTS.md:432-468`, relayed). A standing protocol must therefore never have a
  legitimate outcome except an explicit retire, or Loom would close it.

## 2. Orchestration as a protocol

### 2.1 What the orchestrator's case is

One `sdc.orchestrate` case per **area of activity**: a repository, or a repository group, with one
AEP planning store and its ESS specifications. Not one per idea (that is `sdc`), not one per
machine. The main session's work maps as follows:

| Today (main `sdc` session) | In the protocol |
|---|---|
| receives an idea, ticket, bug, story id | an entry appended to artifact `intake` (new revision) |
| decides what kind of work it is | `intake.route` (LLM) writes the routing; `intake.check` (program) records `triage` |
| starts a session per repository | one `sdc.orchestrate` case per area; a programme level above areas is *inferred*, not drafted (§ 2.6) |
| plans an idea | `ideas.dispatch` opens one `sdc` case per idea (intake, direction, epics) |
| ranks work across sessions | `wave.propose` (LLM) writes the ranked `wave` artifact |
| checks the work can run at once | `wave.admit` (program) records `admission` |
| asks the owner only on real forks | `wave.approve` requires capability `wave.approve`; who holds it is the autonomy level (`docs/autonomy-levels.md:208-213`); every other person decision stays inside the child that owns it |
| dispatches agents, relays story ids | `wave.dispatch` opens one `sdc.story` case per story, `arguments.stories` |
| merges, runs the gate | `wave.integrate` (program) records `wave_result` |
| decides it is finished | `backlog.survey` records `backlog_state`; outcome `done` (bounded) or claim set "quiescent" (standing) |

### 2.2 Elements (draft, `protocol/draft/orchestrate.protocol.yaml`)

- Input `horizon: bounded | standing`, fixed when the case opens.
- Artifacts: `intake` (entries not yet routed; digest revision, so every arrival is a new
  revision), `portfolio` (planning store + specs, store digest), `wave` (ranked story ids, reason
  per story, worktree count), `release` (the operated release; standing only).
- Evidence, claims, producers:

| Kind | Subject | Claim | Producer | Note |
|---|---|---|---|---|
| `triage` | intake | `intake.triaged` | program | written only when every entry is routed; no result field (state bound) |
| `ideas_dispatched` | intake | `ideas.dispatched` | runtime (`case_open sdc`) | |
| `admission` | wave | `wave.admitted` | program | `admitted` / `refused` |
| `wave_approval` | wave | `wave.approved` | person or decider through `wave.approve` | revision-bound record, since Canon grants are not (`docs/protocol-notes.md:227-231`) |
| `wave_dispatched` | wave | `wave.dispatched` | runtime (`case_open sdc.story`) | |
| `wave_result` | wave | `wave.delivered` | program | `delivered` / `failed`; one record per wave, whatever its size |
| `backlog_state` | portfolio | `backlog.drained` | program | `drained` / `pending` |
| `health` | release | `release.healthy` | program | `healthy` / `unhealthy`, `max_age: 1h` |

- Obligations: `triage.required`, `delivery.required` (backlog drained), `health.required`
  (`applicable_when: {input: horizon, in: [standing]}`).
- Outcomes: `done` (bounded, triaged, drained), `retired` (decision `explicitly_retired`). No
  other outcome, so a standing case is never closed by Loom's completion rule.

### 2.3 Where the LLM is and where it is not

- LLM, writing artifacts only, never evidence (the SDC rule, `docs/protocol-notes.md:105-106`):
  `intake.route` (classify, file drafts citing the entry), `wave.propose` (rank ready stories,
  give reasons). Inside children: every writer the child protocols already have.
- Deterministic: `intake.check`, `wave.admit`, `wave.integrate`, `backlog.survey`,
  `health.observe`, and Canon's admissibility of every action.
- Authority: `wave.approve`. This is the one "fork" the orchestrator itself owns. Scope, vision,
  roadmap, merge and release forks stay in the child protocols, where they are today.

### 2.4 Admission of parallel children (the `wave.admit` program)

Admitted only when all hold, otherwise `refused` with the failing rule:

1. every story is accepted (`active`) and in one wave of
   `aep plan artifact waves --kind story --status active --format json` (`W:53-59`), with no
   printed `collision` between two of them and none `unassessed` (a story without scope is never
   placed: `W:194-198`, `aep plan artifact waves --help`);
2. every `depends_on` is terminal or in an earlier delivered wave (`W:153-155`);
3. story count ≤ free run slots: the engine's configured ceilings (Loom
   `<loom-plan>/story/action-limits-enforced.md:18-27`: ceilings in engine config, a binding may
   only lower them) and free build slots (`slot status`), and the remaining model budget;
4. every story's ESS gate passes at the current spec revision (else the child would only stall
   at `change.implement`'s precondition, `sdc-story.protocol.yaml` `change.implement`);
5. no work-in-progress limit is exceeded (§ 2.4a).

Admission is global to the area, not per epic: two epics touching one crate collide. This
needs one change to the existing family (*proposed*): `sdc.epic` stops opening its own stories
(`stories.open`, `sdc-epic.protocol.yaml:152-178`); it hands its accepted stories to the
portfolio and waits for a copied `stories_delivered` record, the way `roadmap.read` copies today
(`sdc.protocol.yaml` `roadmap.read`; copy limitation `docs/protocol-notes.md:221-226`).

Each admitted child gets its own managed worktree from `worktree create` and its own build
directory (`BM:26-29`), replacing the shared `work/` (`main.rs:235-237`). Children of one wave
run concurrently; the runtime must drive siblings in parallel, which neither the driver nor the
planned `tree run` does (§ 1.1). The tree daemon's run slots are the natural place (*inferred*:
it schedules trees, not siblings; extend its slot to "one per running case").

### 2.4a Work-in-progress limits (decided, the operator 2026-11-21)

Parallelism must speed delivery up without leaving unfinished work behind (100+ open worktrees,
nothing finished, every start a reconcile). Flow KPIs: `docs/kpis.md` § 5.

- Limits, in the area's configuration, enforced by `wave.admit`, never by an LLM:
  `limits.wip.max_in_progress` (open story child cases), `limits.wip.max_open_worktrees`,
  `limits.wip.max_unmerged` (closed `passed`, branch not merged), `limits.wip.max_item_age`.
- Finish before start: a new wave is `refused` while any limit is exceeded; the refusal names the
  limit and the items that hold it. This is rule 5 of § 2.4.
- A story's worktree is removed by the merge step of that story; a worktree with no open case is
  an orphan and is filed into `intake` as a finding.
- Tunable during a run: a change is the recorded event `limits_changed` (who, old, new), by a
  person or a rule, read at the next admission and never retroactive. No LLM raises a limit
  (`docs/autonomy-levels.md:271-272`).
- The WIP limit is an eval variable: a sweep (1, 2, 4, 8) over the standard suite picks the
  default per level (`docs/kpis.md` F17).

### 2.5 Conflicts and late findings flow back through `intake`

Canon has child-to-parent evidence (`case_outcome`) and nothing in the other direction
(`docs/canon-requests.md:38-40`). The draft needs neither direction beyond the opening:

- merge conflict or red gate at `wave.integrate`: the story's branch is not merged, the failure is
  filed into `intake`; `wave_result` is `failed`; `intake` has a new revision, so `triage` no
  longer applies (revision binding) and `intake.route` is admissible again;
- a late finding inside a child (the finding loop of `docs/trial-2.md:23-41`): resolved inside the
  child if it can be; if it changes scope beyond the child, the child files it into `intake` and
  ends `dropped`;
- spec revised under running stories: each `sdc.story` invalidates its own
  `change.built`/`tests.pass` (`sdc-story.protocol.yaml:218-226`); the parent cannot (Canon
  `inert-invalidation`, `docs/protocol-notes.md:211-215`) and does not need to;
- a pre-existing finding becomes a backlog draft that waits for the next plan, exactly the AEP
  wave rule (`W:552-569`).

### 2.6 Staying under 65,536 states with N work items

Canon `check` multiplies one dimension per evidence kind × subject (`2^(results+1)` values), 3 per
capability, 2 per decision, one per input value (`canon/crates/canon/src/check/mod.rs:10-92`;
bound `:339`, enforced `:559-568`). The draft has 49,152:
`admission 4 · backlog_state 4 · health 4 · wave_result 4 · triage 2 · ideas_dispatched 2 ·
wave_approval 2 · wave_dispatched 2 · capability 3 · decision 2 · input 2`. The first version,
with results on `triage` and `wave_approval`, was refused at 196,608.

Why N does not appear:

- every item is a child case with its own state space; `check` sees a child only as its
  `case_outcome` record (`check/mod.rs:221-240`, relayed);
- the parent reads one aggregate per wave (`wave_result` about the `wave` revision) and one per
  portfolio revision (`backlog_state`), produced by programs that read the children and the store;
- each new wave or intake revision makes the previous records inapplicable by revision binding,
  so the history does not accumulate in the evaluated state.

Why not the `children` form: it reads all children alike (`docs/protocol-notes.md:216-220`); a
single child that ends `dropped` makes it `false` for good, and in a standing case children
accumulate for the life of the product, so one dropped story months ago would block every later
wave. It also costs 72 (`8 × 9`) per listed outcome against 4 for an aggregate record. The cost
of the aggregate: Canon cannot tell a program's `wave_result` from a forgery, as with any copied
record (`docs/protocol-notes.md:221-226`). Canon request: a `children` form scoped to the children
opened at the current revision of a subject (extends `docs/canon-requests.md:33-36`).

Canon's answer (2026-11-21): request 14 accepted as Canon `story:child-outcome-subject` (Canon
wave 8), after `story:child-outcome-revision` (wave 7). The draft protocol works without it,
through `wave_result`.

- Form: `{children: {in: [passed], subject: wave, revision: current}}` reads only the
  `case_outcome` records whose `subject` is `wave` at its current revision. Without `subject` it
  also reads the `sdc` children opened about `intake`, whose outcomes are never `passed`.
- `story:child-outcome-revision` already skips the children of a superseded wave: a story child's
  `case_outcome` carries the parent's revision of `wave` at opening.
- Limit: Canon counts only the `case_outcome` records it is given; a child opened without one is
  invisible (five stories, two `passed` records, three absent: `true`). The runner's duty
  (the driver then, Loom later): write an `open` `case_outcome` when a child is opened, `subject`
  set to the `case_open` subject. the driver did this then: `open_children` takes `subject` from
  `case_open.subject` and writes one `open` record per child with that subject and its current
  revision before the child runs (`driver/src/main.rs:491`, `:503-507`).
- The form does not replace `wave_result` where that record also states that the integration gate
  passed.

A programme above areas (several repositories ranked together) is the same pattern one level up:
one `sdc.orchestrate` child per area, one aggregate per area. *Inferred*, not drafted.

### 2.7 Compared with the AEP wave

| | AEP wave (`aep:implementing`) | `sdc.orchestrate` |
|---|---|---|
| plan | derived once up front from accepted stories (`W:53-63`) | derived per wave from the current store revision; a new accepted story is a new portfolio revision |
| overlap | `aep plan artifact waves` collisions by path; unassessed never placed (`W:194-206`) | the same verb inside `wave.admit` |
| approval | stops after the proposal page (`W:274-285`) | `wave.approve` capability, revision-bound record |
| dispatch | ≤ N implementors, one worktree each (`W:408-414`, `:487-492`) | one `sdc.story` case per story, one worktree each |
| per-item gates | adversary, two attacks, then a person (`W:538-545`, `:636-641`) | inside `sdc.story`: ESS conformance, independent review, merge authority |
| merge | serial `--no-ff` onto the wave branch, one gate (`BM:26-37`, `W:777-797`) | `wave.integrate`, same procedure as a program |
| findings | backlog draft, waits for the next plan (`W:552-569`) | into `intake`, re-triaged |
| enforced by | the coordinator following text (`SK:13-16`) | Canon admissibility + program evidence |
| ends | no undelivered wave (`W:81-82`) | `done` (bounded) or quiescent (standing) |

## 3. End conditions

| | Bounded case | Standing case |
|---|---|---|
| scope | one idea, one ticket, or one named backlog slice | a product that is operated |
| done | outcome `done`: `intake.triaged ∧ backlog.drained` at the current revisions | none |
| quiescent | same as done, just before closing | `intake.triaged ∧ backlog.drained ∧ release.healthy`, every obligation discharged, no useful action; the case stays open |
| re-opened by | n/a (closed) | a new `intake` revision (bug report, ticket, finding, advisory, alert); a new `portfolio` revision with an accepted undelivered story; `health` expired after `max_age` or `unhealthy` |
| ends | `done`, or `retired` by decision | only `retired` by decision |

Rules:

- "No bugs left" is not provable and not the end condition. Done means: every entry that arrived
  was routed, nothing accepted is undelivered, and (standing) the invariants hold now. An open bug
  is an entry; it keeps `triage` or `backlog.drained` from holding until it is delivered or
  explicitly dropped (filed and rejected in the store).
- Invariants of a standing case are claims with a `max_age` (health, advisory scan, error budget):
  Canon's evaluator takes an explicit instant, so expiry is deterministic and replayable.
- Quiescence is the runtime's resting state: no useful action, nothing awaiting a person. A
  standing tree in quiescence holds no run slot, like a waiting tree in the tree daemon
  (`<loom-plan>/story/tree-daemon.md:23-24`). It wakes on an artifact revision or an expiry.
- Runtime changes needed: per-action `max_runs` counted per subject revision, not per case life
  (`main.rs:366-368`); a wake-up source (file watch on `intake`, timer at the earliest `max_age`
  expiry) (*inferred*).
- Rename: the root `sdc` outcome `operating` means "delivered once". With a standing orchestrator
  it should be `delivered`; "operating" belongs to the standing case's quiescent state.

## 4. Brownfield

### 4.1 Entry points

| Entry | Lands in | Then |
|---|---|---|
| idea text | `intake` of the area's case | routed as idea → `ideas.dispatch` → `sdc` (today's path, `autonomy-levels.md:416`) |
| ticket ref | `intake`, text scanned before any agent reads it (`autonomy-levels.md:417`) | routed as idea, bug or story draft |
| planning-store id (`story:x`, `epic:y`) | `intake` as a reference | story: filed to the wave queue if `active`, else to the activation decision; epic: a `sdc.epic` case at the stage its facts make true |
| "continue where you left off" | no new entry | re-evaluate the area's case and every open child; facts decide the stage (`docs/phases.md:3-6`); today `--continue <run>` (`autonomy-levels.md:418`) |
| adopt a repository | first `intake` revision = "reverse-engineer this area" | § 4.2 |

### 4.2 Intake changes

Greenfield intake dissects 3 words into items and interviews (`docs/phases.md:10-26`). Brownfield
intake reconstructs instead, each item citing a source:

| Phase | Greenfield | Brownfield |
|---|---|---|
| dissection, interview | from the idea text | skipped for the existing scope; requirements derived from code, docs and store with `path:line` citations (`aep plan reverse scan`, byte-stable facts: `RE:28-29`; "no citation, no artifact": `RE:12-13`); interview only for what cannot be cited (`RE:116-117`) |
| context defaults | proposed | read from the repository (language, platform, deployment: Cargo.toml, CI, Taskfile); each still an assumption with a cite |
| vision, roadmap | written, approved | adopted if present (`vision:*`, initiative); approval imported from the store's transition record (`actor: human:…`) as a copied record, or re-approved |
| epics, stories | decomposed | adopted; `aep:migrating` for legacy trackers; `aep:reverse-engineer` drafts for uncovered code |
| ESS spec | authored | `reused` if present; `reconstructed` by `ess:retrofitting` from OpenAPI, cluster or code, every declaration cited, gaps `UNMAPPED:` (`R:9-23`) |
| ESS gate | must pass | ratchet gate (decision 3, § 7), mechanics in § 4.5; an adopted spec is usually not clean (§ 4.3) |
| stories already implemented | n/a | not re-run; their delivery evidence is imported, or they are left out of the portfolio's undelivered set |

### 4.3 Dry experiment: `decider` (read-only)

Commands run in a local checkout of the private `decider` repository (worktree unchanged, `git status --short` empty before and
after; compiled outputs written to the session scratch directory):

| Fact | Command | Result |
|---|---|---|
| store | `aep plan artifact list`, `validate` | 5 artifacts, valid: `vision:decider` draft, `epic:decider-v1` active, `story:decider-extract` implemented, `story:decider-pretrain` proposed, `story:decider-heads-eval` draft |
| waves | `aep plan artifact waves --kind story` | 3 waves of 1 story each; 6 collisions, all `(inferred)`, on its CLI crate and its specification; chain extract → pretrain → heads-eval by `depends_on` |
| spec | `ess specify validate --path spec` | valid, 6 files, 10 scenarios (ess 0.96.0; spec requires 0.74) |
| gate step 1 | `ess specify validate --strict-requires` | **fails**: "requires ess 0.74 … --strict-requires refuses a newer release" |
| gate step 2 | `ess specify compile` | 82 declarations, compiled |
| gate step 3 | `ess verify conform synthesize [--scenarios spec]` | 51 scenarios (10 authored), **45 refusals** (gate needs 0) |
| gate step 4 | `grep -rn UNMAPPED: spec` | 0 |
| conformance target | its Taskfile | `--target interpreted` only: the suite runs against the spec's interpreter, not against `decider` |
| delivery evidence | its evidence store | one record for the implemented story |

What the SDC intake would derive:

- requirements: from its design document and the epic's Outcome, cited;
  context: Rust CLI, private, offline, from `Cargo.toml`, `AGENTS.md`;
- vision: adopt `vision:decider` (draft: needs an approval decision); roadmap: **none**, the store
  has no initiative, so `sdc` root's `epics.open` (needs `roadmap.approved`,
  `sdc.protocol.yaml:115-137`) is blocked until a one-epic roadmap is reconstructed and approved;
- spec source `reused` (in-repository).

Where it would start: `sdc.epic` for `epic:decider-v1` at `spec.gate`, which fails, so the case
sits in `sdc.spec` with the spec writer admissible: fix the ESS pin and the 45 refusals. Then
`sdc.plan` needs every story `active` (`sdc-plan.protocol.yaml:79-84`): two activation decisions.
Then waves of one story each: no parallelism is possible in this repository as planned. Each
`sdc.story` would further need a conformance run against the implementation, which `decider` does
not have (it runs `interpreted` only).

### 4.4 Rating: 2 of 5 for brownfield today

- Works: facts-not-phases evaluation means "continue" and "start mid-way" need no special code;
  `spec.reuse` / `spec.reconstruct` exist (`docs/protocol-notes.md:123`); AEP and ESS have the
  reverse tools (`RE`, `R`).
- Gaps:
  1. no roadmap kind; the SDC requires an approved roadmap before any epic
     (`docs/protocol-notes.md:245-246`);
  2. the all-or-nothing ESS gate blocks every adopted spec that is not already clean (decider:
     pin + 45 refusals);
  3. `sdc.plan` needs every story active, so one draft story blocks the epic;
  4. `sdc.story` assumes a conformance target that is the implementation; many repositories have
     only the interpreter or nothing;
  5. no import of existing approvals (store transitions) or delivery evidence as Canon records;
  6. intake has no "reconstruct" path: `sdc.intake` starts from `idea.yaml` only.

### 4.5 Ratchet ESS gate (decision 3; refusal records per the ESS session, 2026-11-21)

The structured refusal list exists in ess 0.96.0; nothing is needed from ESS.

- Precondition of adoption (ruling, 2026-11-21): a failing ESS version pin does not fall under the
  ratchet. The baseline is taken only from a specification that validates strictly at the
  installed ess (`ess specify validate --strict-requires`), so fixing the pin (decider: requires
  0.74, installed 0.96) is the first adoption step, its own story action, before the baseline
  count is recorded.

- Command: `ess verify conform synthesize --path spec --suite-format 5 --out suite.json`.
- Ratchet count: `.coverage.counts.refused` (decider:
  `{"authored":0,"generated":41,"outside":0,"refused":45}`); the gate also requires
  `.coverage.knowledge` = `"complete_inventory"`. Records: `.coverage.refused`; identical records
  keep their multiplicity.
- Per record: `.code` (stable, e.g. `ESS-SYNTH-013`), `.subject` = `{kind, name}` with `kind` one
  of `entity`, `outcome`, `type`, `transition`; `name` is a qualified string for entity and type,
  `{command, outcome}` for an outcome, `{entity, transition}` for a transition; `.scenario` is the
  candidate scenario id.
- Domain of a record: the qualified name without its last segment
  (`decider.catalog.CatalogVersion` → `decider.catalog`), from `.subject.name.command` for an
  outcome and `.subject.name.entity` for a transition. A story's own domains are the domains of
  the declarations it touches, mapped the same way.
- Gate: `refused` ≤ the count recorded at adoption, and 0 records in the story's domains. No
  `file:line` per declaration (`.source` is null for generated candidates); the gate does not need it.
- Cautions: `--suite-format 5` writes `ess-conformance/59` for decider, not `/58`; use that file
  for the gate only, or for the conformance run too, but never mix the two files' digests. With
  `--format json` and an ordinary suite the refusals are printed on neither stream, so no refusal
  lines in JSON mode does not mean zero refusals.

## 5. UI impact

Today one store, one case tree, one selected protocol graph, lenses, a Decisions drawer
(`ui/README.md:26-52`; `web/index.html:21-44`). For an area case:

```
┌ header ─ area: decider ─ horizon: standing ─ QUIESCENT since 14:02 ─ health ✓ (exp 15:02) ─ Decisions 2 ┐
├ intake strip ──────────────────────────────────────────────────────────────────────────────────────┤
│ new: bug#41 (alert) · finding f7 (wave 3) │ routed: idea → IDEA-4 · story:x draft │ triage ✓ r12   │
├ admission strip ───────────────────────────────────────────────────────────────────────────────────┤
│ wave 4 r3: [pretrain] admitted · slots 2/4 · budget $12/$40 │ held: heads-eval (dep pretrain)      │
│ collisions: extract×pretrain crates/decider-cli │ unassessed: none │ approval: decider (A3) ✓     │
├ lanes (one row per open child, grouped planning / delivery / operate) ─────────────────────────────┤
│ IDEA-4   sdc     ▸intake ▸direction ○epics      awaiting person: roadmap.approve                   │
│ pretrain sdc.story ▸gate ▸implement ◉tests ○review ○merge   LLM running 6m   worktree w-pretrain     │
│ wave 3   integrate ✓ delivered  (2 merged, 1 dropped → f7)                                         │
├ orchestrator ─ its own graph (small, 11 actions) + decision log: rank reasons, admit/refuse, approvals ┤
└ timeline: area events (arrivals, waves, expiries) ─ drill-down into any lane opens today's view ───┘
```

- Lane = one child case; its phase pips come from that case's claims as today (phase strip,
  `README.md:28-29`), so today's whole view becomes the drill-down of one lane.
- Admission strip shows the `wave.admit` evidence: what is admitted, what is held and why
  (collision, dependency, unassessed, slots, budget). This is the "queue".
- Orchestrator panel: the LLM's ranking with reasons next to the program's admission verdict, so
  a reader sees where the model proposed and where a rule decided.
- Area of activity: header names the area, its repositories, the store and the horizon; a
  programme view lists areas with their state (active / waiting / quiescent).
- Decisions drawer: a read-only list (the inspector answers nothing; `ui/README.md`), grouped by lane; `wave.approve` is one more kind.
- Quiescent and expiry are first-class: what would wake the case, and when.
- Server: the store watcher must handle many concurrent writers (one per running child) instead
  of one driver lock (`main.rs:108-138`); `now.js` already derives in-flight per case
  (`web/now.js:20-30`) and needs no model change for concurrent lanes (*inferred*).

## 6. Draft protocol `protocol/draft/orchestrate.protocol.yaml`

```
$ canon validate --path protocol/draft/orchestrate.protocol.yaml
valid: protocol `sdc.orchestrate` revision 1
$ canon compile --path protocol/draft/orchestrate.protocol.yaml > protocol/draft/orchestrate.ir.json   # exit 0
$ canon check --path protocol/draft/orchestrate.protocol.yaml
checked: protocol `sdc.orchestrate` revision 1: 49152 states (input `horizon` 2 values), 0 properties, 0 findings
```

(`canon` = `../canon/target/debug/canon`.) It is outside `protocol/*.protocol.yaml`, so
`scripts/check-protocols.sh` does not pick it up. Not done: no bindings file, no probe (a probe
"done in a standing case" would be unreachable by construction), not run by the driver (its
`case_open` children `sdc` and `sdc.story` exist, but the driver drives them sequentially).
Headroom: 16,384 states; one more result kind (×4) does not fit.

## 7. Decisions (the operator, 2026-11-21)

Acceptance, the operator 2026-11-21: "I do accept the orchestration and parallelism design and upcoming
epics and stories. So write them, scope them, implement it, but when it's time for it. So we're
probably focusing first on the non-orchestration bits."

1. **Story delivery is admitted in the area case**, across epics. `sdc.epic` stops opening its own
   stories (`stories.open`) and waits for a copied delivery record. Overlap is a property of the
   repository, not of an epic.
2. **A standing case notices change through two channels**: the `intake` queue (every arrival a
   new revision) and a `health` claim with `max_age`, which Canon expires after its age limit at
   an explicit instant.
3. **An adopted spec gets a ratchet ESS gate**: the synthesis refusal count may not rise above the
   count at adoption, and each story's own domain must be clean.
4. **Work-in-progress limits** (added 2026-11-21): § 2.4a; flow KPIs in `docs/kpis.md` § 5.

Epics (planning store, all `draft`): `epic:orchestrate-area-case` (§ 2, § 3),
`epic:concurrent-child-cases` (§ 1.1, § 2.4), `epic:brownfield-entry` (§ 4),
`epic:area-board` (§ 5), `epic:flow-limits` (§ 2.4a). `aep plan artifact list --kind story`
lists their stories.
