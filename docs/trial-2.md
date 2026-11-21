# Trial 2: an ambiguity found late, and how the cycle repairs itself

Goal (the operator, 2026-11-21): see how the protocols behave, and repair themselves, when an ambiguity
is discovered on the road and was not defined upfront. The more is defined upfront, or decided
by an agent on the way, the fewer human interventions; the KPIs optimise for fewer interventions
and longer autonomous runs per task.

Built and checked in a working copy, then merged into this tree (protocols, compiled IR, bindings,
tools, scripts, cases) on 2026-11-21. `sdc-drive` was unchanged by the merge (changed later for the run notes R3, R4: observation run cap, program errors). The programs are two binaries of
the driver crate, `sdc-findings` and `sdc-finding-kpis` (`driver/src/bin/`), built with
`cargo build --bin sdc-findings --bin sdc-finding-kpis`, which does not relink `sdc-drive`.
Lines marked *(inferred)* were not observed.

## Result (2026-11-21)

- State: stopped by the coordinating session at finding f16 in `IDEA-1.todo.lists` (scope-changing, raised by `change.implement`): r4 asks users to view their lists, the spec has no read command; option 1 (default) the browser's local copy only, option 2 add a read command.
- Cases: closed intake, direction, spec, plan, access story (merged); open lists (f16), offline (opened, never evaluated), release (not opened).
- Why stopped: option 2 changes the specification and no story action re-derives it (R7); covered by `story:story-finding-reopens-spec`.
- Calls: 27 work calls, $0.503 (`llm-calls.jsonl`, all Haiku), 5 decider calls, $0.024 (`decisions.jsonl`); the driver counts 30 (it counts "not called" starts).
- Time: 18 driver legs, 1009 s in the driver (`rounds.jsonl`); wall clock 5274 s (00:53:37 to 02:21:31), 4265 s (81 %) outside the driver.
- Findings raised: 17 (intake 3, context 4, spec 2, story 8), by LLM 16, program 1, person 0; 14 of them late (after intake).
- Findings resolved: 16, 14 by agent default, 2 by a person (f1, f9, taken by the coordinating session for the operator); defaults accepted: questions 16 of 20, context 6 of 7, findings 14 of 16.
- Rework: 58 revisions (requirements 15, findings 17, change 9, questions 7, plan 4, spec 4, context 2).
- Replay of the root (R8, not re-run): exit 1, 7 of 9 match; `IDEA-1.todo.offline` never evaluated, `IDEA-1.todo` state mismatch after access's `child_closed` rewrote its `case_outcome` (event 34).
- T1 not hit: no approval while a rule fired or a finding was open (context, vision, roadmap approved by the decider after consistency exit 0).
- T2 not hit: the planted contradiction was raised by program rule `offline-hosted` (`p-offline-hosted-context.deployment`, `raised_by: program`), no human finding.
- T3 hit: that finding was classed agent, and its first resolution reached the decider's reject and a person's context change (R5); fixed in R5, its record now reads agent, default taken.
- T4 not hit by wording (no finding open at any stuck or spent stop); hit in substance 5 times (R2, R4, R6, R7, R8).
- T5 not hit at the end (consistency exit 0 on the plan case's `work/`); hit in substance at leg 5, where the amended context contradicted q8, averted by the decider's reject (R5).
- T6 open: hit if the 5 operator steps count as interventions (R2, R4, the plan step, R8 twice), at the limit if not.
- Success L4 met: 14 late findings, the planted one at the context stage.
- Success L5 met: caught by LLM 16, program 1, person 0.
- Success L1 not met: 2 finding decisions (f1, f9) against at most one; the floor is not comparable once the decider took the approvals (L1 9).
- Success L3 met in the record: all 14 agent-classed findings resolved by their default; T3 above is the exception on the way.
- Success L6 not met: vision 0 and roadmap 0, spec 4 revisions after the first (limit 2).
- Success L2 met by its wording, not clean: longest round 268 s (19 % of wall time), no stop on an agent-classed finding after the roadmap, but 4 of the 8 later legs ended `stuck` or `writer_spent`.
- Success replay not met: exit 1, 7 of 9 cases.
- Works: the planted contradiction (offline against a hosted server) was raised by a program rule, not by a person.
- Works: 17 findings raised, 14 resolved by agent default, none raised by a person.
- Works: the decider refused a context that contradicted an earlier decision (q8; `decider/001-context.approve`).
- Does not work: person stops could not be decided cold: `epic:decisions-a-person-can-take-cold`.
- Does not work: caps and stale evidence after a person's decision (R2, R4, R7), fixed in the driver: the cap rule, `docs/driver.md:104` ("The loop", "Run caps").
- Does not work: the wrong amendment (f11's default emptied f9's decision, R7): `story:amendment-keeps-person-decisions`.
- Does not work: story-stage findings cannot reach the specification (f9, f16): `story:story-finding-reopens-spec`.
- Does not work: 8 of 14 late findings arose in the story stage: `story:intake-asks-what-stories-will-need`, `story:late-findings-per-stage-kpi`.
- Did not work: the decider could not take a finding decision (f1, f9, f16 went to a person): `story:decider-finding-decision`, built since (cycles `decider-finding*`).
- Does not work: the driver counts 30 calls, the record 27: `story:llm-call-count-matches-record`.
- Does not work: replay of the root fails after a child closes: `story:replay-after-child-close`.
- Fixed R1: edit-then-approve refused; `sdc.intake` r6, cycle `scope-changed`.
- Fixed R2: glob compare in `llm-node.sh` refused every `questions.answer` apply; cycle `scope-changed`.
- Fixed R3: a check that could not run was recorded as `invalid`; tools exit 127 with no evidence; cycle `intake-decision`.
- Fixed R4: flat observation cap, interview rewritten after the f1 decision; cycle `intake-decision`.
- Fixed R5: `offline-hosted` fired where both sides hold, verification ignored cited decisions; cycles `offline-sync`, `offline-decision`.
- Fixed R6: `aep` took the repository's lifecycles for the case store; cycle `plan-in-project`.
- Fixed R7: flat run cap per case, `stuck` after a person's decision; cycles `story-decision`, `story-spent`.
- Fixed R8: 429s counted as runs, a spent writer hid a due handoff; cycle `story-late-person`.
- Next: trial 3 (`story:clean-unattended-run-compare`): the same idea run unattended twice in two model settings.

## What trial 1 showed

`runs/trial-todo-1` (read only). The person changed the context to "a daemon serving the web UI and
a JSON API … its own port and data file" while requirement r3 (from q3's accepted default) said
"kept in browser storage". The vision repeated both (`superseded/vision-product-r1.md`: "Keeps
to-do items in browser storage" next to "Runs as a server-like daemon … serves … a JSON API"). Only
the coordinator noticed, at the vision approval: q3 and r3 edited by hand, the vision moved aside,
three more rounds (`continue-4.log` to `continue-6.log`). No check saw it: `context.check` only
checked the shape.

## The late-finding loop

### In one paragraph

Any stage can raise a typed **finding** (`ambiguity`, `contradiction`, `missing_decision`) citing
the items it is about (`r3`, `q3`, `d2`, `context.deployment`). Findings live in the artifact
`findings.yaml`. A **program** raises the ones it can see (consistency rules); a **stage agent**
(LLM) may raise others in a declared output, and they count only after a program check accepts
their shape and citations. While a finding is open, the stage's own check returns `finding`, so
its claim is false and nothing downstream is admissible. The action `amend.upstream` then
resolves it: a program classifies it as an **agent** decision (no scope change, or the other
side is a person's stronger decision) or a **person** decision; an agent decision takes the
default at once, recorded as an answered question with its reason; a person decision waits (the
run stops with the exact file to edit). The decision **amends upstream**: a new question in
`questions.yaml` and the cited requirements in `requirements.yaml`. That new requirements
revision is what Canon re-derives from: invalidation sets aside the context check, revision
binding sets aside every record about the derived artifact the amendment marks, and the writer
re-derives it.

### Parts

| Part | Where | What it does |
|---|---|---|
| `findings.yaml` | artifact `findings` in `sdc.intake`, `sdc.direction`, `sdc.spec`, `sdc.plan`, `sdc.story` | `{findings: [{id, type, stage, raised_by, cites, says, options, default, scope_changing, status, scope, basis, decision, decided_by, resolution}]}`; status `open`, `needs_person`, `resolved`, `dismissed`, `rejected` |
| consistency rules | `driver/examples/sdc/tools/consistency-rules.json` (`sdc-consistency-rules/1`) | deterministic keyword rules between context fields and requirement texts: `storage-location` (server context vs browser storage: trial 1's case), `interface-kind` (CLI vs web UI), `user-count` (single user vs shared), `offline-hosted` (offline vs server, unless the context gives the client an offline path, for example the optional `offline` field `context.default` now fills), `shared-local` (shared between people vs one user's machine, same `unless`; added 2026-11-21). Each names its two sides, two options and the text that replaces the losing side |
| program | `sdc-findings --rules <consistency-rules.json>` (`consistency`, `gate`, `promote`, `amend`; source `driver/src/bin/sdc-findings.rs`), called from `tools/sdc-tool.sh` and `tools/llm-node.sh` (override the binary with `SDC_FINDINGS=`) | no clock, no network, no LLM |
| finding gate | folded into one existing check per stage: `requirements.check` (intake), `context.check` (direction), `spec.trace`, `plan.cover`, `tests.run` (story) | exit 5 → result `finding`, while a finding is open, a rule fires, or `requirements.yaml` holds `open_findings`. Not in any writer's `retry_when`, so no writer is sent back for it |
| stage agents | `tools/llm-node.sh`: `requirements.normalise`, `context.default`, `vision.write`, `roadmap.write`, `spec.author`, `plan.decompose`, `change.implement` | the prompt asks for `out/findings.yaml` (type, cites, says, options, default, scope_changing), reads `in/findings.yaml` (resolved ones are decisions to follow); `sdc-findings promote` accepts or rejects each. While a finding waits for a person no stage agent is called. A derived tree (`spec/`, `impl/`, `aep/`) gets `.derived-from` (the requirements digest), so a re-derivation is always a new revision |
| `amend.upstream` | a new writer action in the five protocols; binding `sdc-tool.sh amend <stage>` with a `handoff` | admissible only while the stage's gate claim is false; promotes rule hits, classifies, amends, verifies, marks the derived artifact |
| `scope.answer` | `sdc.intake` r5, capability `scope.answer` (coordinator, `autonomy-levels.md` D1 option a) | see "Scope-changing answers" |

### Classification: who decides (program, never the raiser's word alone)

Rank of an item: 2 a person's own decision (`status: changed` by a person, or `derived` from one),
1 a person accepted a proposed default, 0 an agent's default. A requirement takes the highest rank
of the questions it cites.

1. A rule contradiction with sides of different rank: the higher side stands, the agent amends
   the lower side (`answered_by: derived`, reason names both ranks). Trial 1's case: the context
   (rank 2, the person's own words) against r3 via q3 (rank 1, an accepted default): the agent
   resolves it, no person.
2. Same rank above 0, or rank 0 on both sides resting on a scope-changing question: a person.
3. A raised finding marked `scope_changing: true`, or a contradiction that would overturn a
   person's decision: a person.
4. Anything else: the agent takes the default.

After an agent amendment, a program verifies it: every requirement cites a fact, a wish or a
question; every scope-changing answer names a principal; the requirement count and the set of
specification domains are unchanged (otherwise the amendment changed scope); the rule no longer
fires. A failed verification is reverted and the finding goes to a person.

Added 2026-11-21 (Run notes R5), for the agent's amendments only (a person's decision stands):
no other rule fires that did not fire before; and after a rule decision, no decision (an answered
question) cited by a requirement outside the losing side states the losing side's content (the
losing side's pattern matches its answer). This covers decisions taken by earlier findings of the
same run, which are questions too (q7, q8). The basis names the decision and shows both sides as
they were. A rule may name an `unless` (items and a pattern): it does not fire while one of them
states what makes both sides hold (`offline-hosted`, `shared-local`: the client keeps a local copy,
works offline, syncs). Two sides that can both hold therefore no longer reach the ranking; when the
context leaves no such path, the ranking still picks a side, and the verification sends the pick
to a person if a cited decision holds the other side. No third outcome ("amend to state both") is
built: the agent cannot know how the two meet (local copy and sync is a design choice).

### The person's part

When a finding needs a person, `sdc-drive run` stops `awaiting_person` (exit 3) with the handoff:
in `work/findings.yaml` set `decision:` (one of its options or free text), or `status: dismissed`;
then run again. The edit moves the `findings` revision, `amend.upstream` runs again and amends
with `decided_by: human`. A person may also add a finding by hand (`raised_by: human`) in place of
editing questions and requirements as in trial 1.

### Canon terms and the state budget

Canon counts an evidence kind as `2^(classes × 2^upstreams)` and a capability as ×3
(`canon/crates/canon/src/check/space.rs`). `sdc.intake`, `sdc.spec` and `sdc.plan` were at 65,536,
`sdc.direction` at 3 × 2^14: none can take a new evidence kind, an invalidation or a capability.
So the loop adds no dimension:

- artifacts and actions are free: `findings`, `questions`, `requirements` declared where needed,
  `amend.upstream` everywhere (state counts unchanged, checked);
- the finding gate is a new **result** (`finding`) of an existing check, which falls into the
  class `None` of its kind (the claim is false, as for `invalid`);
- re-derivation uses what exists: `requirements.revised` invalidates `context.valid`
  (`sdc.direction`), `questions.revised` invalidates `requirements.valid` (`sdc.intake`); elsewhere
  the amendment gives the derived artifact a new revision (`.derived-from`, or the vision set
  aside), and revision binding sets aside every record about the old one;
- `sdc.direction` r4: `vision.write`, `vision.approve`, `roadmap.write` and `roadmap.approve` need
  `context.valid`, so no vision or roadmap is written or approved while a finding is open; a
  finding raised at the vision or roadmap enters `requirements.yaml` `open_findings`, which
  re-runs the context check. `context.default` stops once the context is valid (it re-ran on
  every person edit in trial 1).

A child case was considered and not used: the stage that finds the problem must stop, and only
the root (4,608 states) has room to open a child, but the root acts only when every open child
has ended, and a stage cannot end on a finding without a new outcome condition. A `sdc.finding`
child would need the stages restructured (for example `sdc.direction` split in two).

### Scope-changing answers (coordinator's addition, D1 option a)

`sdc.intake` r5: the human-only rule is gone. `questions.answer` (the agent) answers only the
questions that change no scope and may not touch the others (`llm-node.sh` refuses to apply a
change to one). The new action `scope.answer` requires the capability `scope.answer`; it is a
`manual` binding, taken with `sdc-drive approve --capability scope.answer --principal P --action
scope.answer`, and runs `sdc-tool.sh scope-answer P`: every scope-changing question still open
takes its default, every scope-changing answer is recorded with `answered_by: P`. The runtime
grants it per autonomy level (a person at A0 to A2, a decider at A3 and up). `check-questions`
now refuses a scope-changing answer only when it names no principal or the agent's
`non-interactive` (exit 3, result `needs_scope_answer`). To afford the ×3, `sdc.intake` dropped
the invalidation `idea.revised`: a revised idea opens a new intake case at the root, so it was
never used inside one. Result: 49,152 states (was 65,536).

`sdc.intake` r6 (2026-11-21, after `runs/trial-2-train` stopped, see "Run notes"): `scope.answer`
is also admissible while `questions.shaped` is `unknown` (it needs `dissection.valid`, and not
`questions.shaped` false). A person's edit of an answer moves `questions` to a revision no check
has seen, so under r5 the approval after the edit was refused. `scope-answer` refuses a
scope-changing question with `status: changed` and no `answer`, and a missing or unparsable
`questions.yaml`, with what to fix; `questions.check` runs on the revision it writes. State count
unchanged.

### State counts (`canon check`, 0 findings each)

| Protocol | before | after |
|---|---|---|
| `sdc` r4 | 4,608 | 4,608 (unchanged) |
| `sdc.intake` r4 → r5 → r6 | 65,536 | 49,152 |
| `sdc.direction` r3 → r4 | 49,152 | 49,152 |
| `sdc.epic` r2 | 36,864 | 36,864 (unchanged) |
| `sdc.spec` r3 → r4 | 65,536 | 65,536 |
| `sdc.plan` r4 → r5 | 65,536 | 65,536 |
| `sdc.story` r3 → r4 | 12,288 | 12,288 |
| `sdc.release` r3 | 24,576 | 24,576 (unchanged) |

### Limits

- A finding at the plan or story stage amends the requirements and re-derives its own stage, but
  not the specification, whose case has already ended: `sdc.epic` (36,864 = 9 × 2^12) has no room
  to reopen `sdc.spec` on a requirements revision.
- A person's decision on a finding after intake goes through the handoff edit, not through
  `scope.answer`: no protocol after intake has room for a second capability. Under a decider
  (A3+) `run-trial.sh` takes that edit itself (added 2026-11-21, `story:decider-finding-decision`):
  each finding waiting for a decision goes to the decider as a decision request (its text,
  options, default, cites, and the requirements, prior decisions and context fields it touches);
  the answer is written as `decision:` with `decided_by: decider:<kind>`, `amend.upstream` applies
  it through the same program check as the agent's amendments, and a refused one is reverted and
  kept as `refused_decision`, and the finding waits for a person.
- Keyword rules see only what their patterns name; the LLM findings cover the rest, and those are
  only as good as the agent's attention.
- An approved vision is not re-derived when a later stage amends the requirements
  (`sdc.direction` has ended by then).
- `amend.upstream` is admissible whenever the stage check is false, so it also runs once, as a
  no-op, when the check fails for its own reason: 3 more driver steps in the stub cycle (R9 65 →
  68).
- `amend.upstream` makes no LLM call: the amendment is a program (a new answered question and a
  `Decision qN: …` clause on the cited requirements, or the rule's replacement text). The stage
  agents re-derive from it. The requirement text is clumsy but explicit.

## The trial

### Idea: two candidates

| | Idea text (verbatim for the run) | Planted ambiguity | Where it should surface |
|---|---|---|---|
| **A (recommended)** | "A to-do list web app for me and my partner that also works on the train without a connection" | "web app" plus "without a connection": where does the list live, and how do two people's copies meet? The interview will likely ask about sharing and storage; the context agent then has to pick a deployment, and any server-side choice (`server`, `hosted`, `cloud`, `web service`) contradicts the offline requirement | `context.check` (program rule `offline-hosted`), or, if the context agent picks a local deployment, at `vision.write` or `spec.author` as an LLM finding (sync between two devices: a missing decision) |
| B | "A command-line tool for my team to see who is on call this week; they check it from their phones" | a command-line tool that is read from phones | `context.check` (rule `interface-kind`, if the context says CLI and a requirement says web page or browser), else `spec.author` |

A is recommended: it is the same family as trial 1 (where the data lives), two of the four rules
can fire on it, and if no rule fires the LLM path is tested instead. Either way it is the
direction or the spec stage, the two stages where the loop re-derives fully. B is more likely to be
caught at intake already (both "command-line" and "phones" are in the text, so the dissection
should mark it an ambiguity), which tests intake, not a late finding.

### Expected path (A)

1. Intake: about 4 dissection items; questions on sharing, sign-in, storage, offline; the
   scope-changing ones wait for `scope.answer` (one person action, accepting or changing the
   defaults); requirements include "works without a network connection".
2. Direction: `context.default` picks a deployment. If it names a server: `context.check` returns
   `finding` (`p-offline-hosted-context.deployment`, cites the requirement and
   `context.deployment`). The requirement rests on a person's answer (rank 1 or 2), the context is
   an agent default (rank 0): `amend.upstream` replaces the deployment with "on the user's own
   machine", records q-new `answered_by: derived`, and `context.check` re-runs valid. No person.
3. The person approves context, vision, roadmap (3 actions, as in trial 1).
4. Spec: `spec.author` may raise "how do two devices exchange the list" as a missing decision. If
   it marks it scope-changing (a sync component), the run stops once for the person (edit
   `findings.yaml`), else the default is taken.
5. Story, merge, release: 1 merge per story, 1 release, as before.

### Refutation criteria (written first)

The trial refutes the loop if any of these holds. One run is a pilot (n = 1, no statistics): each
criterion is a hard condition on that run.

| # | Refuted if | Read from |
|---|---|---|
| T1 | a person approves a context, vision or roadmap revision while a consistency rule fires on the work files of that revision, or while a finding is open | `events.jsonl` `action_finished` of `*.approve`; `sdc-findings consistency` and `gate` replayed on the revision's files (`blobs/`, `.sdc/superseded/`) |
| T2 | the planted contradiction is found by a person (a `raised_by: human` finding, or a hand edit of `questions.yaml` or `requirements.yaml` as in trial 1) | `findings.yaml`; `revised` events of `questions`/`requirements` not following an `action_finished` |
| T3 | a finding that the program classed `agent` reaches a person, or a finding classed `person` is resolved without one | `findings.yaml` `scope`, `decided_by` |
| T4 | the run ends `stuck`, `step_budget_spent` or `llm_budget_spent` in a case with an open finding | the last verdict event; `findings.yaml` |
| T5 | a resolved finding's rule fires again at the end, or a final requirement contradicts a resolved decision | `sdc-findings consistency` on the final `work/` |
| T6 | the loop costs more person interventions than trial 1's repair did (1 hand edit + 3 re-runs + 1 vision re-approval): L1 − required floor > 2 | L1; floor = 3 direction approvals + merges + 1 release + `scope.answer` actions |

### Success (all must hold)

| KPI | Success | Failure |
|---|---|---|
| L4 late findings | ≥ 1 (the planted one surfaced after intake) | 0 with the contradiction present in the final files (missed) |
| L5 caught by | program or LLM ≥ 1, human 0 | human ≥ 1 |
| L1 human interventions | ≤ floor + 1 (at most one finding decision, and only for a scope-changing finding) | > floor + 2 (T6) |
| L3 defaults accepted (findings) | every `agent` finding resolved by its default | an `agent` finding needed a person |
| L6 rework | vision ≤ 1, roadmap ≤ 1, spec ≤ 2 revisions after the first | more |
| L2 autonomous run time | longest round from roadmap approval to the merge stop without a finding stop | a finding stop after the roadmap that the program classed `agent` |
| replay | `sdc-drive replay` exit 0 on every case | any mismatch |

## Commands

Nothing here rebuilds `sdc-drive`; the scripts use `driver/target/debug/sdc-drive` (override with
`DRIVE=`), and the trial writes only to its own run directory, `runs/trial-2-train/`. Run every
command here from the repository root.

```console
$ scripts/run-trial.sh trial-2-train \
    "A to-do list web app for me and my partner that also works on the train without a connection"
```

After each stop (exit 3) do what it prints, then resume with the name only:

```console
$ scripts/run-trial.sh trial-2-train
```

The stops have these forms:

| Stop | What you decide | How |
|---|---|---|
| intake `scope.answer` (awaiting authority, or awaiting person once granted) | the scope-changing answers: edit `work/questions.yaml` (`answer`, `status: changed`; `scope.answer` sets `answered_by`) or leave them to accept the defaults | 1. edit (or not); 2. the printed `sdc-drive … approve --capability scope.answer --principal operator --action scope.answer`, once; 3. resume. Needs `sdc.intake` r6 in the case (`jq -r .protocol.revision <case>/ir.json`): under r5 step 2 is refused after an edit (`questions.shaped` unknown), and the order is edit, resume, approve, resume |
| direction approvals | context, vision, roadmap, as in trial 1 | the printed `approve … --action context.approve` (then `vision.approve`, `roadmap.approve`) |
| a finding (`amend.upstream`, awaiting person) | in `work/findings.yaml` set `decision:` on each `needs_person` finding (an option or free text), or `status: dismissed` | edit, then resume |
| merge, release, close | as in trial 1 | the printed commands |

Afterwards:

```console
$ driver/target/debug/sdc-finding-kpis runs/trial-2-train
$ driver/target/debug/sdc-drive --canon ../canon/target/debug/canon \
    --case-dir runs/trial-2-train/store/IDEA-1 replay
```

Free rehearsal first (stub idea, every LLM command printed, none run, approvals scripted):
`SDC_RUNS_DIR=<scratch>/runs scripts/run-trial.sh --dry trial-2-dry`.

## Verification (2026-11-21, stubs only, no LLM call)

Scratch: `S` is the session scratch directory. Run three times with
the same exit codes and KPI values: in the working copy with the first (interpreted) programs,
again after the port to the Rust binaries, and on this tree after the merge
(`$S/check-all.sh "$PWD" main`, outputs in `$S/check-main/`). The port was also
compared against the first programs directly: the trial 1 replay leaves identical
`requirements`, `questions`, `findings` and `context` files, and the KPIs of the five recorded
cycles are equal. `sdc-drive`'s digest is unchanged (`9c7ee798…`).

| Command | Exit | Result |
|---|---|---|
| `scripts/check-protocols.sh` | 0 | 8 protocols valid and checked, 0 findings each (counts above); the ESS gate property holds for spec, plan, epic, story, release |
| `scripts/check-bindings.sh` | 0 | 8 of 8 pass; unbound only `context.change`, `release.rollback`, `spec.reuse`, `spec.reconstruct` (as before) |
| `scripts/evaluate-cases.sh` | 0 | 8 of 8 pass. First run: exit 1, because the case fixtures did not name the new artifacts (`missing-artifact: findings`); after adding them (`findings: none`, plus `questions` and `requirements` where newly declared) the only differences were the new actions, all `blocked` (`amend.upstream` in 4 cases, `scope.answer` in intake), written into the 4 `expected.json` files |
| `scripts/run-finding-cycles.sh $S` (the six full stub cycles below through `run-full-cycle.sh`, `RUNS=$S/runs`; `scope-answer` and `finding-program` were run with the same command lines before they were added to the script) | see rows | |
| `stub-cycle` | 0 | root `operating`, replay 8 of 8 match; 68 driver steps (65 before: 3 no-op `amend.upstream`); L1 5 |
| `finding-agent` (`SDC_STUB_FINDINGS=fixtures/finding-spec-agent.yaml`) | 0 | `spec.author` raises f1 (missing decision on r1, not scope-changing); `spec.trace` exit 5 `finding`; `amend.upstream` resolves it by the default (agent), adds q3, amends r1, marks `spec/`; `spec.author` re-derives, every spec check re-runs, `spec.trace` 0. No stop for a person: L1 5 (the floor), L3 findings 1 of 1 by default, L4 spec 1, L5 llm 1, replay 8 of 8 |
| `finding-person-stop` (`finding-spec-person.yaml`, no decision) | 3 | stops `awaiting_person` in `IDEA-1.billing.spec` with the `amend.upstream` handoff (edit `work/findings.yaml`) |
| `finding-person` (same, `FINDING_DECISION=option:2`) | 0 | after the person's decision: amended r2 ("Decision q3: the customer and the billing clerk"), q3 `answered_by: human`, spec re-derived, `operating`; L1 7 (floor 5 + 1 decision + its question), replay 8 of 8 |
| `scope-answer` (`SDC_STUB_FIXTURES=fixtures/scope-question`) | 0 | intake stops `awaiting_authority` for `scope.answer`; `approve --capability scope.answer --principal operator --action scope.answer` records q2 `answered_by: operator` with its default; `questions.check` settled; `operating`; L1 7 (6 actions + 1 answer) |
| `finding-program` (`SDC_STUB_FIXTURES=fixtures/storage-conflict`: trial 1's case on the billing idea) | 0 | direction: `context.check` exit 5 (rule `storage-location`: the person's daemon with a data file against r3's browser storage), `amend.upstream` resolves it by agent (rank 2 over 0), r3 now "kept on the server, in the service's data file", `context.check` 0, then the person approves the context: the first approval already sees a consistent context. L4 context 1, L5 program 1, L1 6 (5 + the simulated context edit) |
| `scope-changed` (added 2026-11-21, `SDC_STUB_FIXTURES=fixtures/scope-question`, `SDC_LLM_NODES=questions.answer` through `tools/llm-adapter-fixture.sh`, `SCOPE_ANSWERS={"q2": …}`) | 0 | `questions.answer` goes through `llm-node.sh`'s apply step (no LLM); at the `scope.answer` stop the person changes q2 in `questions.yaml`, then takes `scope.answer` once; `operating`, replay 8 of 8; q2 keeps the person's answer, `answered_by: operator`, `requirements.normalise` after `scope.answer`. Before the fix: exit 1, approve refused (`questions.shaped` unknown); with only r6: exit 1, `stuck` in intake after three refused applies (`$S/sc-before/`, `$S/sc-preconly/`) |
| `intake-decision` (added 2026-11-21, R4: as `scope-changed`, plus `NO_YQ_STEP=questions.check`, `SCOPE_RESUME_FIRST=1`, `fixtures/finding-intake-person.yaml`, `FINDING_DECISION=option:3`) | 0 | the step without `yq` exits 127 with no evidence; edit, resume, approve; the person picks option 3 of a scope-changing intake finding; `questions.write` runs once; q2 `answered_by: operator`, the decision `answered_by: human`. Before: exit 1, `stuck`, interview rewritten |
| `offline-sync` (added 2026-11-21, R5: `fixtures/offline-sync`) | 0 | hosted server, context `offline: yes: …`, r3 offline: `offline-hosted` does not fire, deployment kept. Before: exit 0 with the deployment amended to "on the user's own machine" |
| `offline-decision` (added 2026-11-21, R5: `fixtures/offline-decision`) | 3 | r3 offline cites q3 ("a hosted web server"); the rule fires, the agent's amendment contradicts q3: reverted, `needs_person`, basis names q3 and both sides. Before: exit 0, amendment stood |
| `plan-in-project` (added 2026-11-21, R6: case directory inside a project with profile `development.standard`, `plan.decompose` through `llm-node.sh`'s apply with `fixtures/plan-stories.json`) | 0 | `plan.decompose` applied once, `operating`, replay 8 of 8. Before: exit 1, `stuck` in the plan case, `plan.decompose` exit 1 |
| `story-decision` (added 2026-11-21, R7: `fixtures/finding-story-rounds.yaml`, `FINDING_DECISION=option:2`) | 0 | `change.implement`'s stub raises a scope-changing finding and an agent one, then another agent one; the person's decision is an arrival, `change.implement` runs 4 times, `operating`. Before (round-3 driver): exit 1, `stuck` in the story case after 3 runs |
| `story-spent` (added 2026-11-21, R7: `fixtures/finding-story-spent.yaml`) | 1 | a new agent finding on every `change.implement` run: the run ends `writer_spent` naming `change.implement`, 3 runs. Before: `stuck` with no action named |
| `story-late-person` (added 2026-11-21, R8: `fixtures/finding-story-late-person.yaml`, `FINDING_DECISION=option:2`) | 0 | `change.implement`'s third run raises a scope-changing finding: `awaiting_person` (due `handoff_when`) rather than `writer_spent`, then a fresh allowance, `operating`. Before: exit 1, `stuck` after the person's decision |
| `decider-scope-answer` (added 2026-11-21, `runs/trial-2b-fresh`'s first stop: `scripts/run-trial.sh` with `SDC_DECIDER=haiku` on `tools/llm-adapter-fixture.sh`, `fixtures/scope-two`) | 0 | two scope-changing questions open at `scope.answer`; each goes to the decider on its own (text, options, default, dissection item, idea excerpt; `questions.settled` marked `settled_by_this_decision`): q2 by its default (`accepted`), q3 a free answer (`changed`), both `answered_by: decider:haiku`, class `scope_changing_answer` in `decisions.jsonl`; `scope.answer` taken as `decider:haiku`; closed, replay 8 of 8. Before: exit 3, the decider escalated the approval of `scope.answer` reading `questions.settled` false as a failed check |
| `decider-scope-final` (added 2026-11-21, as above, `SDC_DECIDER_ESCALATE=final`, the decider escalating q3) | 0 | q3 `escalation_suppressed`, its default taken. Before: exit 6 (the escalated approval had no `best`, counted as reject) |
| `decider-finding` (added 2026-11-21, `story:decider-finding-decision`: `fixtures/finding-spec-person.yaml` under the decider) | 0 | f1 (scope-changing) goes to the decider as a finding request; it answers option 2; f1 resolved with `decided_by: decider:haiku`, the amendment's question `answered_by: decider:haiku`, class `finding` in `decisions.jsonl`; closed, replay 8 of 8. Before: exit 3, "nothing for decider:haiku at amend.upstream" |
| `decider-finding-final` (as above, `SDC_DECIDER_ESCALATE=final`, the decider escalating with no `best`) | 0 | `escalation_suppressed`, f1's default taken. Before: exit 3 |
| `decider-finding-refused` (`fixtures/offline-decision` under the decider) | 3 | the decider takes the default the agent's check already refused; the same check refuses the decider's amendment (it states against decision q3): reverted, `refused_decision` kept, the decider not asked again, a person decides. Before: exit 3 with no decider request and no `refused_decision` |
| `scripts/run-trial.sh --dry trial-2-dry` (`SDC_RUNS_DIR=$S/runs`) | 0 | 10 LLM commands printed, none run; scripted approvals; `operating`; replay 8 of 8 match |
| `$S/t1-replay.sh` (trial 1's own files, copied; q3/r3 put back to browser storage) | 0 | `sdc-findings consistency` exit 5 (`p-storage-location-r3`, cites `context.deployment`, `context.platform`, `r3`); `amend context` resolves it by agent; gate exit 0; `check-questions` 0, `check-requirements` 0. Trial 1's contradiction would have been found at the context check and repaired with no person |

## Run notes

`runs/trial-2-train`, 2026-11-21. Intake case `C=runs/trial-2-train/store/IDEA-1/children/IDEA-1.intake`
(started under `sdc.intake` r5; its `ir.json` stays r5). Copy of the stuck state:
`$S/trial-2-train.stuck/`. Logs: `$S/trial-2-leg1.log` to `-leg4.log`; answer scripts
`$S/trial2-answer.sh`, `$S/trial2-answer2.sh`; continuation `$S/trial2-continue.sh`.

**R1: the documented order (edit, then approve) was refused.** The operator's answers to q1 and q3
(`status: changed`) were written into `work/questions.yaml`, then `approve --capability
scope.answer --action scope.answer`: `action scope.answer is not admissible:
{"reasons":[{"claim":"questions.shaped","value":"unknown"}]}` (`-leg2.log`). The edit is `revised`
seq 64 (questions `t15a4d7e…` → `ta48e586…`) with no action behind it; evaluation `0065.json` has
`questions.shaped` unknown (no record at the new revision), and r5's `scope.answer` required it
true. Resume (questions.check exit 3), approve, resume did take it (seq 77-79). An earlier refusal
with the same message at 00:55:08 had another cause: the live view ran `step --action
questions.check` (`ui-actions.jsonl`) in an environment without `yq`, so the check exited 2
(`invalid`, blob of seq 60: `yq: Kommando nicht gefunden`) against the valid record e0038 at the
same revision, and the two made the claim unknown (`0061.json`). Not fixed here (`ui/`): an
environment failure of a check is recorded as evidence. Fix: `sdc.intake` r6 (above), the
`scope-changed` cycle, the stop table.

**R2: stuck after the changed answers.** `round 1: stuck` (`-leg3.log`), `0089.json`:
`questions.settled` false, `requirements.normalise` blocked by it, `questions.answer` admissible but
not useful. Cause: `tools/llm-node.sh` (apply of `questions.answer`) compared the scope-changing
questions with `[[ $(…) == $(…) ]]`; the unquoted right side is a glob pattern and JSON never
matches itself as one. Haiku answered q2 and q4 and left q1 and q3 alone three times
(`llm/003-` to `005-questions.answer`, in/out identical on the scope-changing questions), and every
apply was refused (`apply.log`: `changed a scope-changing question: not applied`, exit 1, seq
46/50/54). That spent `questions.answer`'s `max_runs` 3, so q2 and q4 stayed open for good. No
requirements were ever written (`requirements` stays `none`; the three invalidated
`requirements.check` records are about `none`); `questions.check` (7 runs of 8) and the
`scope.answer` revision (seq 78) are not the cause. Fix: both sides quoted; the stub adapter never
reached that line, so the `scope-changed` cycle sends `questions.answer` through it
(`tools/llm-adapter-fixture.sh`).

**Continued, not restarted.** No event log, evidence or case file was edited. One operator step,
`sdc-drive step --action questions.answer` (an explicit step is not bound by `max_runs`), with
run-trial.sh's LLM environment and logged in `run.log`: Haiku answered q2, q4, applied (seq 93).
Then `scripts/run-trial.sh trial-2-train` (`-leg4.log`): interview settled, `requirements.normalise`
wrote r1-r6 from the operator's answers and raised three findings; `amend.upstream` took the defaults of f2
(item states) and f3 (the fixed user map is for the test version) and stopped on f1 for a person
(exit 3, `awaiting_person` in the intake case: f1, scope-changing ambiguity citing q1 and r2,
whether the partner can see or edit a user's lists). LLM calls: 7 Haiku calls in
`llm-calls.jsonl` (about $0.02); the driver counts 8, because the second `requirements.normalise`
(seq 131) was started and returned "not called: findings wait for a person".
q1 and q3 keep the operator's answers, `answered_by: operator`. Replay of the tree: exit 0, both cases match.
Children opened from now on compile the protocols in `protocol/` at opening, so they take any
later fix; the intake case keeps its r5 `ir.json`, which no longer matters there (`scope.answer`
is taken).

**Who decided what (2026-11-21).** q1 and q3: the operator, in chat, as free text ("neither option as
asked"); written to `questions.yaml` by the coordinating session. Finding f1 (raised by
`llm:requirements.normalise`, scope-changing: may the partner see or edit a user's lists):
decided "Each list can be shared with the partner" by the coordinating session under the operator's
delegation ("decide that for me"), not by the operator himself. `amend.upstream` records it as
`decided_by: human`; for T2, T3, T6 and L1 count it as one person intervention taken by a
stand-in. The operator could not decide the first request as presented (no context); see
`epic:decisions-a-person-can-take-cold`.

**Decider from the direction stage on (2026-11-21).** At the `context.approve` stop the operator asked
whether the decider should take such decisions ("shouldn't this be done by the agent, or the
decider"). The run was resumed with `SDC_DECIDER=haiku` (escalate or an invalid reply stops for
a person). So L1 and the T6 floor of this run are not comparable with the design above from
that point: read person interventions per stage, and take the decider's votes from
`decisions.jsonl`. The decider has no goals document yet (`docs/requester.md`).

**R3: a check that could not run was recorded as `invalid`.** The live view's `questions.check`
without `yq` (R1) exited 2 and became a `questions_shape: invalid` record and one spent run.
Fix: every tool in `driver/examples/sdc/tools/` checks the programs it needs and exits 127
(`sdc-tool.sh` `need`, a nested call passes 126/127 on); `sdc-drive` records no evidence for exit
126/127, does not count it as a run and stops with exit 2 (`docs/driver.md`, "Fixed policy"). The
driver's digest changes with this (first change since trial 1).

**R4: stuck again after the f1 decision; the interview was rewritten.** Copy:
`$S/trial-2-train.stuck2/`; decision script `$S/trial2-f1.sh`; `-leg4.log` (last lines).
`amend.upstream` did what it should (event 139, log "f1 resolved by human … (amends q7, r2)"):
q7 `answered_by: human`, r2 "Decision q7: Each list can be shared with the partner". The `yq -i`
edit of `findings.yaml` changed only indentation plus `decision:` (`fs/changes.jsonl` seq 42), and
the agent-amendment verification (§ Classification) does not apply to a person's decision. Cause:
the default `max_runs` 8 also bound the observations. `questions.check` had run 8 times (events
12, 29, 38, 60 (R1's run without `yq`), 68, 83, 98, 124) and `requirements.check` 8 times, so
neither could check the amended revision. `questions.shaped` stayed unknown (`0141.json`), which
admits `questions.write`, and the policy took it: Haiku wrote a fresh interview (`llm/008-`, event
148, q1-q4 all open) over the operator's answers, f1's q7 and the agent's q5/q6. With both checks spent,
`stuck`. Fixes: observations default to 32 runs (`src/main.rs` `useful`); `questions.write`
(`llm-node.sh`) does not rewrite an existing `questions.yaml` unless a check found it invalid
(a retry); cycle `intake-decision` (before: exit 1, `stuck`, interview rewritten; after: exit 0).
Continued: `questions.yaml` put back from sdc-watch's record (`fs/blobs/d8cb7470…`, the content of
revision `td8cb7470df6cdfc0` that event 139 names; `$S/trial2-restore.sh`). This is the one hand
edit of a work file in the run: a `revised` event with no action, content equal to
`amend.upstream`'s output, so not a person's finding for T2. Then `scripts/run-trial.sh
trial-2-train` (`-leg5.log`): `questions.check` settled, intake `passed`; direction:
`context.default` raised f4-f6 (agent defaults; f4 chose "Single hosted web server reachable over
the internet"), `context.check` exit 5 on rule `offline-hosted` (the planted contradiction, found
by the program), `amend.upstream` resolved it by agent (deployment "on the user's own machine"),
`context.check` 0. Stop: `awaiting_authority`, `context.approve` of context `t2a77fb422aa60348`.
For the person to see there: f4's decision and the amended deployment disagree, and two partners
sharing lists on one user's machine is not settled. LLM calls: 9 in `llm-calls.jsonl` (about
$0.027), the driver counts 10. Replay: 3 of 3 cases match.

**R5: the decider rejected the context; the rule and the verification were wrong.** Copy:
`$S/trial-2-train.rejected/`. At that stop the coordinator resumed with `SDC_DECIDER=haiku`, which
rejected `context.approve`: the deployment "on the user's own machine" contradicts r4, which cites
"Decision q8: Single hosted web server … Decision q11: works offline". Two defects.
(1) `offline-hosted` fired on any hosted deployment beside an offline requirement, although a
hosted server and an offline client both hold when the client keeps a local copy and syncs. Fix:
rules may name `unless` (items, pattern) in `consistency-rules.json`; `offline-hosted` and the new
`shared-local` stay silent while `context.offline`, `context.deployment` or `context.platform`
states such a path; `context.default` now writes `offline` (`no`, or `yes: ` and how), and
`check-context-shape` checks it. The finding still fires when the context leaves no offline path.
(2) The verification checked only the rule and the requirement count, so it accepted an amendment
against q8, a decision of the same run (f4) that r4 cites. Fix: § Classification, "Added
2026-11-21". When both sides can hold, the rule no longer fires and the ranking is not reached;
when only one can, the ranking picks and the verification hands any pick against a cited decision
to a person with both sides. D-T2-1 option a needs no third outcome. Cycles `offline-sync`
(0) and `offline-decision` (3); before the fix both exit 0 with the deployment amended.
Continued (`$S/trial2-context.sh`, `-leg7.log`): the coordinator's deployment decision was written
as a person's context change (`status: changed`, reason "operator (via coordinator), 2026-11-21"), as
`ui/decisions.mjs` writes one. Consistency exit 0, r4 unchanged with q8 and q11. Then
`SDC_DECIDER=haiku scripts/run-trial.sh trial-2-train`. The decider approved context
`t5c79ee05b85f08ba`, vision `t2d0f52431cae8c09` and roadmap `tedcc307fa9dc6871` (reasons in
`-leg7.log`, `decider/001-004`). The spec stage raised f7 and f8, and the agent resolved both by
default (f8: PostgreSQL). The run then stopped in the plan case (R6).

**R6: plan.decompose refused by the repository's own aep profile.** Stop: exit 3, `awaiting_person`
in `IDEA-1.todo.plan`, with `amend.upstream`'s handoff. No finding is open (f1-f8 and
`p-offline-hosted-…` are resolved), so nothing there is for a person. The real stop is
`plan.decompose`: exit 1 three times (`llm/016-` to `018-`, `max_runs` 3 spent),
`apply.log`: "story:access is draft; a story may move to: proposed, archived". `aep` takes the
lifecycles of the nearest project above its working directory. `runs/` lies inside this
repository, whose `.engineering/project.yaml` names `development.standard` (draft → proposed
only). The stub cycles and `--dry` ran in scratch, outside any project, so they never saw this.
Fix: `llm-node.sh` and `sdc-tool.sh` call `aep … --root .`, so the case store keeps aep's own
lifecycles wherever it lies. Cycle `plan-in-project` (before: exit 1, `stuck`; after: exit 0).
Not continued: the next move is one `sdc-drive step --action plan.decompose` (one Haiku call; an
explicit step is not bound by `max_runs`), an operator's call. The driver also reports this
spent writer as `awaiting_person` on a no-op `amend.upstream`, not as `stuck`: that handoff is
misleading. LLM calls so far: 18 in `llm-calls.jsonl` (about $0.109), and the driver counts 19.
The decider made 4 further calls (about $0.017), so 22 of the 30 are used.

**Verdict against T1-T6, as far as the run has gone (spec stage done, plan stage stopped).**
- T1: not hit. No approval was taken while a rule fired or a finding was open. The approvals since
  R5 are the decider's, not a person's: context, vision, roadmap, with consistency exit 0 on the
  approved context.
- T2: not hit. The planted contradiction (hosted against offline) was found by the program:
  `context.check`, rule `offline-hosted`, in leg 5. No `raised_by: human` finding was raised, and
  no hand edit was made to the questions or the requirements to find it.
- T3: hit. `p-offline-hosted-context.deployment` was classed `agent`, and its resolution reached a
  person: the decider's reject, then the coordinator's deployment decision. Fixed by R5 (1) and (2).
- T4: not hit by its wording: the run is not over, and the plan case has no open finding. The run
  has stopped three times on spent `max_runs` (R2, R4, R6), and those stops are what T4 is for.
- T5: not hit by its own words, which name "a final requirement" and measure on the final
  `work/`; hit in substance at leg 5's stop, where the amended context contradicted the resolved
  decision q8 (f4) and r4, which cites q8. The decider's refusal kept that state from being
  approved, so the end state is clean: consistency exit 0 on the plan case's `work/`, and r4 holds
  with q8 and q11. That is the loop's last guard working. The program part of the loop (rule,
  ranking, verification) failed, and it took a person's decision (the coordinator's) to repair it.
  So T5 counts as averted at the end, not as passed by the loop.
- T6: open. It is hit if the operator's repairs of tool defects count as interventions: f1 and the
  context change count +1 each above the floor, plus the R2 step and the R4 restore, 4 > 2. If they
  do not count, the total is 2: at the limit, not over it.

**R7: stuck in the story case after a person's decision (the fourth stop of one family).** Copy:
`$S/trial-2-train.stuck4/`. The coordinator took the operator step `plan.decompose` (stories
access, lists, offline) and resumed with the decider. Then, in `IDEA-1.todo.access`:
- `change.implement` ran at events 8, 26 and 46. Run 26 was "not called: findings wait for a
  person (f9)" (exit 0), and it counted.
- `tests.run` returned `finding` (exit 5) twice, at 17 and 34. `sdc-tool.sh story-tests` runs
  `findings gate story` first, and the gate fails while a finding is open: f9 and f10, raised by
  `change.implement` itself, then f9 waiting for a person.
- `amend.upstream` (41) applied the coordinator's f9 decision ("Server sets a session cookie on
  sign-in", amends q15, r1, r2) and marked `impl/` derived from an earlier requirements revision.
  `change.implement` (46) then raised f11, and `amend.upstream` (59) resolved it by default ("The
  token is issued and held nowhere; no command checks it yet").
- `impl/` was marked again. `change.implement` had used its 3 runs, which counted over the whole
  case, so it was not useful. `tests.run` and `change.review` were blocked on `change.built`
  unknown (`0062.json`: every record `revision_mismatch`). Result: `stuck` (event 63).

The class: a flat count per case ignores that a person's decision (event 38, a `revised` event
with no action behind it) is new input. Fix in `src/main.rs` (`docs/driver.md`, "Run caps"):
- an observation runs once per case snapshot, and only while due;
- a writer has `max_runs` per arrival;
- a spent writer ends the run `writer_spent`, naming the action, its runs and its last failure.

Cycles `story-decision` and `story-spent`. Before the fix: exit 1, `stuck`, both times. After:
exit 0, and exit 1 `writer_spent` naming `change.implement`.

Spec and conformance after a story-stage decision: f9 amended r1 and r2 (a session cookie), but
no action in `sdc.story` writes `spec`. The conformance suite therefore stays the one derived
before the decision, and it does not test the cookie. Re-deriving it needs the spec case
reopened, which is a gap (a story-stage finding that changes the spec should send the run back
to `IDEA-1.todo.spec`). f11's default ("held nowhere") also empties f9's decision: the
verification checks rules only, not the meaning of a person's decision.

**R8: three 429s spent the lists story's writer; the late finding f16 waits for a person.**
- Resumed (`-leg9.log`). The decider approved `change.merge` of access (`decider/005`), and access
  closed. In `IDEA-1.todo.lists`, `change.implement` ran 3 times (`llm/024-026`). Each run ended
  with `terminal_reason: api_error`: the model API returned 429 (a usage limit). They were
  recorded as exit 3, so the stop was `writer_spent` after 3 runs.
- Fix: `llm-node.sh` maps an `api_error` session to exit 75, which the driver treats as "could
  not run" (no evidence, no run counted, stop, resume later on the same model). The already
  recorded runs stay as they are.
- Copy: `$S/trial-2-train.spent5/`. The operator step `change.implement` (`$S/trial2-leg10.sh`)
  raised f13-f16. `amend.upstream` resolved f13-f15 by default (f13: "Lists are kept in a local
  data file, one per daemon instance", which overrides f8's PostgreSQL) and marked f16 for a
  person. With its allowance spent (counting the 429s), `change.implement` stopped as
  `writer_spent`, although a person was the real next step.
- Fix: `handoff_when` on `amend.upstream` (`sdc-tool.sh findings-wait`). With a spent writer, a
  due handoff stands even while `amend.upstream` is not admissible. Cycle `story-late-person`:
  before, exit 1 `stuck` after the person's decision; after, exit 0.
- The lists case was opened with the old bindings, which have no `handoff_when`. So one more
  operator step `change.implement` was taken (`$S/trial2-leg12.sh`; "not called" while f16
  waits), then `change.review` and `tests.run` (exit 5).

**Stop (exit 3, `awaiting_person` in `IDEA-1.todo.lists`).** Finding f16 (`missing_decision`,
scope-changing, raised by `llm:change.implement`, cites r2 and r4) says:

> r4 requires users to view their lists and items, but the spec has no command that returns lists
> or items, so the only view is the browser's local copy.

Options:
1. (the default) "Viewing comes from the browser's local copy only, with no read command".
2. "Add a read command that returns the signed-in user's lists and items".

Option 2 changes the spec, which R7 says cannot be re-derived from the story case. The decider
cannot take a finding decision.

LLM calls: 27 in `llm-calls.jsonl` (about $0.50). The driver counts 30 of 45, because it also
counts the "not called" starts. The decider made 5 calls. `sdc-finding-kpis`:
- L1: 9 (2 person actions; answers: 4 questions, 1 context field, 2 finding decisions);
- L2: 18 rounds, 1009 s autonomous, 19 % of wall time;
- L3 defaults accepted: questions 16 of 20, findings 14 of 16, context 6 of 7;
- L4 late findings: 14 (context 4, spec 2, story 8);
- L5: caught by an LLM 16, by the program 1, by a person 0; resolved by agent 14, by a person 2;
- L6 rework: 58.

Replay of the root: exit 1. 7 of 9 cases match. `IDEA-1.todo.offline` was opened but never
evaluated (listed, not failed). `IDEA-1.todo` has a state mismatch only: access's `child_closed`
(event 34) rewrote its `case_outcome` record after the epic case's last evaluation (event 33),
and the epic case is evaluated again when it is next driven. Older runs with the new driver:
`.rejected` 3 of 3, `.stuck4` exit 0, cycle stores 8 of 8.

Verdict update:
- T4: not hit by its own wording: no finding was open at any of the stuck or spent stops (R2,
  R4, R6, R7, R8's `writer_spent`). It was hit in substance five times: the run stopped with
  work left and nothing it could do.
- T6: the operator steps grow to 5 (R2, R4, the plan step, R8 twice), so it is hit if they count.
- T1-T3 and T5: unchanged.

## Decision D-T2-1 (decided: option a)

**D-T2-1: when two decisions contradict, which one stands without asking?** Decided 2026-11-21
by the coordinator, acting for the operator: option a (the higher-ranked decision wins; a person is asked
only on a tie or a scope-changing finding). Built as such.

- **a** (decided) rank rule: a person's own decision (status `changed`) outranks a
  default a person accepted, which outranks an agent's default; the higher one stands and the
  agent amends the other side, recorded `answered_by: derived` with both ranks as its reason; a
  person is asked only on a tie between person decisions or when the raiser marks it
  scope-changing. Trial 1's case resolves with no person.
- **b** ask a person whenever a contradiction touches any person's answer (trial 1's case: one
  finding decision).
- **c** as a, and at A3 and up let the decider take the ties too. Needs `scope.answer` in the
  stages after intake, which have no state room (Limits); a Canon or protocol split first.

Why **a**: it applies a decision a person already took rather than taking a new one,
it is reversible (the finding, both ranks and the amendment stay in `findings.yaml` and
`questions.yaml`), and it is the lever that turns trial 1's late contradiction into zero human
interventions.
