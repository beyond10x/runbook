---
name: workflow-builder
description: Build a protocol-driven workflow (Canon protocol/1 cases run by a driver, with LLM nodes as executors of single actions and a decider model in place of a person) so that its first real unattended run passes with few attempts, then measure it and improve it by controlled experiments. Use when designing or changing such a workflow, before and during its first real trials, when trial runs keep stopping at new places, when a run must be classed as pass or defect, or when comparing settings (models, effort, caps) of a workflow that already passes. Not for writing a single protocol's syntax (Canon's own docs), not for UI iteration (ui-iteration-loop), and not for running a fixed benchmark of agent freedom (the eval design of the repository, docs/eval-design.md in the SDC).
---

# workflow-builder

Two parts. Part A takes a workflow from design to its first clean pass. Part B measures it and
improves it one variable at a time. The worked example (the SDC, eight protocols, five attempts and
a batch on 2026-11-21) is in `references/`; the rules below are generic.

Each rule carries a mark: **[observed]** the worked example shows it worked; **[inferred]** the
example shows that its absence cost time; **[untested]** neither.

## Terms

- **Step**: one admissible action of a case. **Actor**: who takes it (program, LLM node, decider).
- **No-point**: a step whose actor can say no: an approval, a review, a check with a failing result,
  a decision a program can refuse, a writer cap.
- **Route**: what the protocol does after a no: rework by a named writer, or an end by rule (an
  outcome such as `not_delivered`, `withheld`, `dropped`, `abandoned`).
- **Dead end**: a reachable state where no action is admissible and no outcome is legitimate.
- **Frontier**: the furthest stage and round a run reached.
- **Clean pass**: a fresh run that closes in its success outcome by itself, with no stop, no reopen
  and no forced step. Product faults found in it are listed, not hidden, and do not void it.
- **Defect**: anything that ended or forced a run. Recorded with case, step, cause, record line.

## Part A: to the first clean pass

### Phase 0. Frame the pass

Entry: an idea for the workflow. Exit: the four items below written in the repository.

1. Write the pass rule (definition above) and the defect rule: a reopened or forced run is defect
   evidence only, never the pass. [observed]
2. List the outcomes, success and failure. Every failure outcome must be reachable by rule. [observed: J5]
3. Choose the smallest input that still crosses every stage the pass needs. Keep a second, larger
   input for later. [untested]
4. Cut every stage the pass does not need (deployment, operation, anything endless or remote) into
   protocols of their own that the workflow does not open. [inferred: `frictions.md` F11]

### Phase 1. Shape the protocol

Entry: Phase 0 done. Exit: protocols pass `canon check` and the route table is complete.

1. Make each stage a protocol that opens as a root from recorded inputs, with one entry in the run
   script, so a late stage is tried in minutes, not after a full run. [inferred: F11]
2. Enumerate every no-point in every protocol into a route table: step, actor, result, route on the
   1st, 2nd and last no (`references/checklists.md` § 1). A no with no route is a defect before
   any run. [inferred: F2]
3. A rejected approval records `rejected` with reasons and makes the artifact's writer admissible;
   the approval is decided once per artifact revision. [observed: `jumps.md` J4]
4. Every loop has a bound and an exit the protocol can see: after the bound, an ending outcome
   becomes legitimate and the rework action stops being admissible. If the kernel cannot count,
   the driver records the verdict as evidence (`rework_budget: exhausted`) and the protocol reads
   it. [inferred: F6]
5. A refusal at a final gate ends the tree by rule with a named outcome; a terminable root closes by
   rule when its outcome needs no decision. [observed: J5]
6. Every requirement traces to checkable behaviour (a command, a view, a permitted actor); a
   requirement that traces to nothing is a finding, never resolved by default. [inferred: F5]

### Phase 2. Static gate

Entry: Phase 1 done. Exit: zero unexplained findings, reports pinned. No paid run before this exit.

1. Run the kernel's state-space walk (`canon simulate` where available) on every protocol. Triage
   each stuck state and cycle: real, or an artifact of a known walk limit, with the limit named.
   [inferred: F8]
2. Declare every rule that lives in the driver or scripts (rework limit, refusal ends, caps, how
   caps count) as data, and run a route checker over protocols, bindings and that data that prints
   one line per dead end. [untested]
3. Pin both reports in the repository; the check fails on any changed finding set; a deliberate
   change re-pins in the same commit. [untested]
4. Keep every protocol under the kernel's state bound; record the count per protocol.

### Phase 3. Node contracts

Entry: Phase 2 done. Exit: every LLM node and decision class has the items below, each with a test.

1. Every LLM output that is a data document has a response schema; a program validates the reply;
   an invalid reply is retried with the validator's error at most twice, then fails with a named
   cause or a declared fallback. [observed: J3]
2. Audit each decision class's input against what that decision needs, reading one real input
   per class (`references/checklists.md` § 2): [inferred: F3]
   - every id it may cite exists in its input and resolves (ids, `artifact@rev#field`, `file:line`);
   - claims and obligations that only a later step can meet are shown apart, as "met after";
   - every file the decision may doubt is in the input (lock files, built output, test logs);
   - tool metadata that looks like a verdict (front matter `status`) is labelled as such;
   - the prompt matches the run mode (no "escalate to a person" when no person exists).
3. A program check refuses a decision only for content that decision wrote; an older fault goes back
   to its writer with the checker's error. Check citations at write time. [observed once: no batch
   run stopped in intake after `102ce99`]
4. A decision that replaces an earlier answer marks it superseded and rewrites every artifact that
   cites it; citing a superseded answer is a fault at every stage gate. [inferred: F7]
5. Prompt examples are neutral to the product (no database, no domain the input does not name);
   one shared constraint text goes to every node that must obey it. [inferred: F5]
6. A review node is a real model call with a schema and program-checked citations before the first
   pass is claimed; a stub review leaves the decider as the only judge of the product. [inferred: F5]

### Phase 4. Deterministic cycles, then recorded replay

Entry: Phase 3 done. Exit: one red-then-green cycle per route and per fixed defect, all green.

1. Write a stub cycle for every route: red before the route exists, green after; the cycle script
   exits non-zero on any miss. [observed: cycles 16 to 53, each fix proven red then green]
2. Stub cycles prove routes and machinery only. A fixture adapter that ignores the prompt cannot
   find prompt or input defects. Do not count cycles as evidence that a real run will pass. [inferred: F4]
3. Once one real run exists, re-drive it through the real driver with its recorded model replies
   (provisional: adapter `recorded`, `run-trial.sh --recorded <run>`). Change one condition (a cap,
   a timeout, a retry count, an injected reject or fault at a named step) and read the same metrics.
   It answers how the machinery behaves given these replies, not what a model would answer under
   the new condition. A call with no recorded reply is a divergence: report it, never fill it
   silently. [untested]

### Phase 5. First real runs

Entry: Phase 4 done. Exit: one clean pass, or a defect batch for Phase 6.

1. Use the strongest setting for every node and the decider. Cost is not the constraint of a first
   pass; cheaper settings come in Part B. [observed: J1]
2. Run several samples in parallel (at least 2 of the main input, 1 or 2 of the small input) on the
   same tree. A defect may show in one sample only. [observed: J1; F1]
3. Keep going past stops: record each defect with its full reason and force a continuation under a
   named rule principal, up to a fixed number per run. A forced run is defect evidence. [inferred: F1]
4. Reopen a stopped run as soon as its fix exists on a branch; a reopen must re-apply stored refused
   decisions to the fixed check. [observed: `timeline.md` A5 reopen]
5. Stop at once, with reason and the fix it waits on, any run that cannot continue and that nobody
   is working on. [inferred: F12]
6. Every budget stop (calls, decider calls, wall seconds) names the loop that spent it: node,
   edge, count. Size budgets so a working run does not reach them. [inferred: two batch runs
   stopped `decider_budget_spent` with the reason text "autopilot: " and nothing after it]
7. An LLM fallback that picks a move where no step can run is defect evidence each time it acts and
   never makes a run clean; the static gate should have removed that state. [untested]

### Phase 6. Defect batch and fix loop

Entry: a defect batch. Exit: the next attempt starts.

1. Fix the class, not the step: for each defect, search every step of every protocol for the same
   class (missing route, shared cap, input fault) and fix all of them. [inferred: F2]
2. Add one red-then-green cycle per fix; add the route to the route table and its checker.
3. Run the next attempt from a trial branch holding the fixes; merge behind it. [observed: J6]
4. Every claim about a run cites a record line; read each exit code on its own. [inferred: F13]
5. Three attempts stopping in the same stage: stop dynamic runs, return to Phase 2 for that stage.
   [untested]

### Stop rules

- No paid run while the static gate has an unexplained finding.
- No claim of a pass from a reopened or forced run. A pass with a stub node is reported with
  that limit named.
- No attempt that changes the setting and the tree at once when a defect is being confirmed.
- A first pass with zero rejections has not exercised a reject route: say so, and run Part B's
  reject-path experiment before calling the workflow robust.

## What to record, from the first run on

Every run directory holds, written by programs:

- run settings per leg: tree commit, autonomy level, model and effort per node class and for the
  decider, caps; the decider's principal names the model that decided;
- per LLM call: case, action, class, model, effort, start, seconds, tokens, cost, exit, not-called;
- per decision: class, step, revision, decision, reasons, citations, retries, refusals, fallback;
- every stop and defect: at, case, step, cause, full reason, record line, forced or not;
- status: state, outcome, closed by; replay result per case;
- labels: real, stub, fixture or demo; a stopped run shows as stopped, never as running.

## Part B: measure and improve (after the first clean pass)

Programs compute every metric and verdict; no LLM judges. The repository's eval design defines the
base metrics and levels (SDC: `docs/eval-design.md` § 2, M1 to M11, and § 5); this part adds the
workflow-level metrics and the experiment procedure. Names follow the SDC's `sdc.run-metrics/1`
(binary `sdc-run-metrics`, full text in `docs/workflow-metrics.md` on `feat/run-metrics`).

### B1. Metrics

- **Node** = (stage, action). A take is one start of the action; a real take called its tool.
  `instances` = case instances with at least one real take.
  - `looping_factor` = real takes / instances (1.0 = no rework); `rework_takes` = real takes −
    instances; `first_pass_yield` = share of instances with exactly one real take.
  - `retries_by_cause`: each rework take has one cause `A:R`, the action and result that sent it
    back (`merge.approve:rejected`, `spec.validate:invalid`), or `arrival:<A:R>` /
    `arrival:outside` when it came back through an upstream amendment.
  - seconds, cost, tokens, model and effort per call.
- **Edge** `A:R -> W` (type check, approval, finding or arrival): count, `loop_cost_usd`,
  `loop_driver_s`, `loop_wall_s` = all spent in that case from the rework take to the next take of
  A, decider included. Finding edge `finding@S -> T`: findings raised at stage S that amended T.
- **Decision class**: asked, approved, rejected, escalation_suppressed, invalid_replies, retries,
  refused_by_check, cost_usd.
- **Run**: `furthest_stage`, `end` (`closed:<outcome>`, `stopped:<cause>`, `open`), `clean` (closed,
  no reopen, nothing forced or unstuck), `waiting_on_person_s`, cost, driver and model seconds,
  tokens; product faults listed beside.

### B2. Experiment procedure

1. Baseline: at least 3 runs of the current setting on a fixed tree and input.
2. Write the hypothesis before any run, in an experiment record (SDC: `experiments/<id>.yaml`,
   checked by `sdc-experiment check`, reported by `sdc-experiment report`): one node or edge, one metric, the expected direction and size, guard
   metrics that must not get worse (clean-pass rate, product faults, cost), n per variant, and the
   decision rule.
3. Change one variable (a model, an effort level, a cap, a prompt, a route). Effort sweeps: the same
   run 3 times per effort level.
4. Prefer the cheapest instrument that can answer: static walk, then recorded replay (Phase 4.3),
   then paid runs. Run variants in parallel on the same tree.
5. Compare groups by median, min, max and n. Default decision rule: n ≥ 3 per variant, direction
   registered before the runs, and complete separation of the two ranges in that direction (exact
   one-sided p = 1/C(2n,n) = 0.05 at n = 3). Overlapping ranges claim no effect (SDC:
   `sdc-experiment compare`). Verdict: held, contradicted or inconclusive; raw counts before rates.
6. Adopt (merge and take a new baseline) or drop; keep the record either way.
7. Exercise reject paths on purpose: inject a reject at each no-point in a recorded replay, then
   one paid run with a decider that rejects at least once.

With 1 or 2 runs per setting nothing is established; treat such comparisons as hypotheses for B2.

## References

- `references/timeline.md`: every attempt, stop, cause, fix, loop time and frontier, with sources.
- `references/frictions.md`: what cost time, ranked, with numbers and sources.
- `references/jumps.md`: what moved the frontier, ranked by a number.
- `references/measure-improve.md`: what the record already holds as raw material for Part B.
- `references/checklists.md`: the route table, the decider input audit and the run start check.
