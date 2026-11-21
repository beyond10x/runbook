# Why a simulatable, evidence-driven workflow is good for agents: the evidence

Question (the operator, 2026-11-20): Canon sees a run as a stream of events and state snapshots. At each fork
the run can take another branch. Canon knows which edges are deterministic and which are not, and how
many outcomes a run can end in. More LLM participation means more uncertainty, errors and side
effects. Because the workflow can be simulated, it can be optimised by exhaustive analysis before any
run, not by trial and error. Why is that good for agents, and how strong is the evidence?

Method, 2026-11-20. Local files were read at their working-tree state; citations are `path:line`.
Web sources were opened on 2026-11-20. I opened three myself: the Anthropic article, Sinha et al.
and the BOBA abstract. Two research sub-agents opened the others, through a fetch tool that
summarises pages. "abstract" means only the abstract or landing page was read. Check a number in
the source before quoting it outside this file. Items marked *inferred* are my reasoning, not a
source's finding. Nothing was built or run.

## 0. What the project already claims, and on what evidence

| Claim in the project | Where | Evidence given |
|---|---|---|
| A protocol answers "what is legitimate", not "what runs next"; a workflow is one strategy for a protocol | `canon/docs/design/canon-protocol-calculus-design.md:365-407`, `:788-790` | none (design argument) |
| The agent loop is frontier-driven: the agent picks among admissible actions, and Canon re-evaluates after each piece of evidence | same file `:566-608` | none |
| A protocol gives "bounded discretion" and stops the model from silently owning correctness, authority or completion | same file `:1891-1895` | none |
| "Agents choose how to do the work. The organization defines what must be true before the work counts." | Atlas ADR 0066 | operator decision |
| The model sees only actions the frontier allows; each action is revalidated before it takes effect | Atlas ADR 0072 | none |
| A wrong selector choice "must not create an unauthorized action" | Atlas ADR 0073 | none |
| A raw trace never counts as evidence | Atlas ADR 0074 | none |
| Evaluation is pure and byte-identical for the same input | `canon/AGENTS.md:22-26` | conformance suite (`CANON-DETERMINISM-001`, design `:1454`) |

What Canon already does (shipped, from its reference documentation):

- `canon check` enumerates a protocol's whole finite state space. It reports unreachable outcomes,
  dead actions, evidence that no action produces, authority bypass, and each failing declared
  property with a counterexample. It refuses a space above 65,536 states (`canon/website/docs/reference/cli.md:19, 110`;
  `canon/crates/canon/src/check/mod.rs:339`).
- `canon generate` writes one minimal witness scenario per outcome and per blocked action (`cli.md:20`).

What Canon does not do yet, so the operator's idea is ahead of the code:

- It has no paths, stuck states, livelocks or invalidation loops. A state is a set of evidence, not a
  sequence of actions (`sdc/docs/canon-simulation-request.md:18-26`).
- It cannot tell a deterministic edge from a non-deterministic one. `effect` is a free name, and
  `may_produce` cannot say who decides a result. The `author`, `revises` and `decided_by` fields are
  only a proposal (`sdc/docs/loom-sdk-and-agent-node.md:121-134, 148-172`).
- Loom's production action selector is an LLM; the deterministic one is used only in tests
  (`loom-sdk-and-agent-node.md:46-48`).

Evidence the organisation holds:

- None of the governed-autonomy references is a reliability study
  (Atlas's governed-autonomy reference list).
- The experiment that would measure the effect is planned but not done: Atlas's governed-autonomy
  roadmap and task board plan to attempt one Commission case with several executors and compare
  the attempts.
- Agentic Principles has 0 supported principles (`agentic-principles/README.md:21`).
  "Bound autonomy with invariants" is a seed (`agentic-principles/docs/principles.json:6-9`). The
  only candidate is "contain the failure, continue the safe frontier" (`principles.json:98-101`).
- `research/literature` holds 113 papers and 1,194 findings (`research/literature/DISTILLATION.md:7-8`).
  The most relevant is BOBA (arXiv 2611.02207), used below.

## 1. The argument

**1. Errors compound with the number of steps, so per-step reliability sets the reachable task length. Supported.**
- Under a constant per-step accuracy, horizon length "grows hyperbolically with the step accuracy".
  Models also make more mistakes once their own errors are in the context, which the authors call
  self-conditioning. Sinha et al., https://arxiv.org/html/2509.09677 (full text, §2.1 and §3.2).
- The 50% time horizon of frontier models was about 50 minutes and has doubled about every 7 months
  (METR, https://arxiv.org/abs/2503.14499, abstract).
- τ-bench: gpt-4o succeeds on fewer than 50% of tasks, and pass^8 is below 25% in retail
  (https://arxiv.org/abs/2406.12045, abstract).
- When instructions arrive over several turns, performance drops 39% on average
  (https://arxiv.org/html/2505.06120, full text).
- BOBA: models that complete about 95% of requests one at a time complete about 51% of 20-request
  chains (https://arxiv.org/abs/2611.02207v1, abstract). The org's notes record a per-request failure
  rate flat at 3-6% along the chain (`research/literature/papers/2611.02207.md:32-37`).

**2. A mistake nobody verifies makes the next step fail more often. Supported, observational.**
- After one unverified mistake, failure of the next request's own tests rises by an average of 18
  points. The odds of failing roughly double with each additional open defect beneath a request
  (`papers/2611.02207.md:39-44`).
- The authors put about 9 of those points down to task difficulty, not to the mistake itself (same
  file, `:44`).

**3. Agents and LLM reviewers cannot be relied on to verify their own work. Supported.**
- Submissions that follow a passing local test run still fail the hidden contract 8.3% of the time,
  against 8.6% for all submissions (`papers/2611.02207.md:53-58`).
- LLM reviewers separate wrong from correct implementations with a Youden J of at most 0.15
  (`:60-65`).
- Intrinsic self-correction lowers GPT-4 on GSM8K from 95.5 to 91.5 to 89.0
  (https://arxiv.org/html/2310.01798, full text).
- GPT-4 plans alone: about 12% are executable (https://arxiv.org/html/2402.01817, full text).
- *Inferred* link to Canon: "trace is not evidence" (ADR 0074) and the proposed `self-judged-evidence`
  check (`loom-sdk-and-agent-node.md:164`) target this failure mode.

**4. An external check after an LLM step raises success. Supported, but the gain depends on how good the check is.**
- LLM-Modulo with external verifiers: Blocks World rises from about 12% to 82% within 15 rounds, and
  Logistics to 70% (https://arxiv.org/html/2402.01817, full text).
- BOBA: one external "that's wrong" plus one repair round lifts chain completion from 51% to 59%
  (https://arxiv.org/abs/2611.02207v1, abstract). That signal is an oracle
  (`papers/2611.02207.md:72`).
- MAST: adding a verification step gives "+15.6% improvement in task success"
  (https://arxiv.org/html/2503.13657, full text, first ~60%).
- Huang et al.: with ground-truth stopping, GPT-4 on HotpotQA goes from 49.0 to 59.0 (URL above).
- Partly supported for Canon. Canon does not verify anything itself; it demands that a program-decided
  record exists for the current revision before an outcome counts. The gain depends on that check
  (*inferred*).

**5. Limiting an agent to an allowed action set at runtime blocks most violations at millisecond cost. Supported, mostly by the systems' own authors.**
- AgentSpec prevents unsafe executions in "over 90% of code agent cases", at about 1-3 ms per check
  (https://arxiv.org/html/2503.18666, full text; Table 3 does not plainly reproduce the 90%).
- NOMOS on τ²-bench: violations fall from 66.3% to 2.6% (airline) and from 30.8% to 6.9% (retail)
  (https://arxiv.org/abs/2610.11030, abstract, preprint).
- Agent Behavioral Contracts: 88-100% compliance with hard constraints
  (https://arxiv.org/abs/2602.22302, abstract, preprint).
- Shielding originates with Alshiekh et al. (https://arxiv.org/abs/1708.08611, abstract).
- Canon's frontier projection (ADR 0072) is a shield in this sense (*inferred*).
- Counter-case: one static, task-independent deny list did not lower injected effects, 46/75 against
  47/75 (`research/literature/DISTILLATION.md:2109-2113`).

**6. Predefined paths beat free agents on well-defined tasks, and cost less. Partly supported.**
- Anthropic: "workflows offer predictability and consistency for well-defined tasks"; "The
  autonomous nature of agents means higher costs, and the potential for compounding errors."
  (https://www.anthropic.com/engineering/building-effective-agents, full text). This is guidance, not
  a measurement.
- AlphaCodium flow: pass@5 rises from 19% to 44% (https://arxiv.org/abs/2401.08500, abstract).
- Agentless: 32.00% of SWE-bench Lite at $0.70 per issue (https://arxiv.org/abs/2407.01489, abstract).
- AFlow: +5.7% on average, with smaller models at 4.55% of GPT-4o's cost on some tasks
  (https://arxiv.org/abs/2410.10762, abstract).
- Canon is not a fixed path (design `:365-407`), so these results support it only by analogy
  (*inferred*).

**7. A run that persists its events and snapshots can resume after a crash instead of starting over. Supported as practice.**
- Temporal: "A Workflow Definition must be deterministic to support replay"; the event history lets
  state survive crashes (https://docs.temporal.io/workflow-definition#deterministic-constraints and
  https://docs.temporal.io/workflow-execution/event, full).
- Event sourcing (https://martinfowler.com/eaaDev/EventSourcing.html, full).
- Agent frameworks do the same: DBOS (https://docs.dbos.dev/why-dbos), Restate
  (https://docs.restate.dev/concepts/durable_execution) and Pydantic AI
  (https://pydantic.dev/docs/ai/durable-execution/overview/).
- Measured for agents: resuming a failed run from its restored state solved 291 of 500 against 293
  for a restart, with 33% fewer Executor tokens (`DISTILLATION.md:2084-2089`).
- Canon's pure evaluator meets Temporal's determinism requirement by construction (`canon/AGENTS.md:22-26`).

**8. Binding evidence to a revision stops a stale check from counting. Partly supported.**
- The failure mode is documented. Regression of the agent's own earlier work is the most common
  first-failure class, 37-55% per model, labelled by an LLM judge (`papers/2611.02207.md:108-113`).
- A test run that ran no tests was read as a pass in 19 of 40 Sonnet checks (`DISTILLATION.md:869-874`).
- Unsupported for Canon: that revision binding prevents these failures is *inferred* and was not
  measured.

**9. Analysing the protocol before any run finds design defects that trial and error would only find by hitting them. Partly supported.**
- Tool capability exists: `canon check` finds unreachable outcomes, dead actions and authority bypass
  (`cli.md:19`).
- MAST names "unaware of termination conditions" (12.4%) and "disobeying task specification" (11.8%)
  among the largest multi-agent failure modes (URL above). An explicit outcome and obligation set
  targets these (*inferred*).
- Formal-LLM reports an "over 50%" gain from automaton-constrained planning, without naming the metric
  (https://arxiv.org/abs/2402.00798, abstract).
- No study was found that counts defects caught before a run against defects found during runs.

**10. Knowing which edges are non-deterministic bounds where uncertainty enters, and counting reachable outcomes makes the risk of a run visible. Unsupported (a belief).**
- Canon does not model edge determinism yet (section 0).
- No paper was found that model-checks agent workflows with LLM-estimated transition probabilities,
  or that translates them to Petri nets or TLA+ (research sub-agent, gaps list).
- AgentGuard learns an MDP of an agent's behaviour and model-checks it at runtime, and reports no
  numbers (https://arxiv.org/abs/2509.23864, abstract).

**11. Exhaustive analysis makes optimisation proactive and cheaper than trial and error. Unsupported (a belief).**
- No source measures it for agents.
- The classic limit is state explosion (Clarke, https://ntrs.nasa.gov/citations/20100024455,
  abstract), and Canon already refuses above 65,536 states.

## 2. Counter-evidence, and where Canon does not help

- **Minimal scaffolds overtake fixed pipelines as models improve.**
  - Anthropic's SWE-bench agent kept "the scaffolding minimal" and scored 49% on Verified against a
    previous best of 45% (https://www.anthropic.com/engineering/swe-bench-sonnet, full).
  - mini-swe-agent is about 100 lines and reports above 74% on Verified
    (https://github.com/SWE-agent/mini-swe-agent, self-reported). Agentless's 32% was on Lite, so the
    two are not directly comparable.
  - ReCreate reports that generated agents beat human-designed ones in 13 settings
    (https://arxiv.org/abs/2601.11100, abstract, no percentages).
- **A rigid workflow wrote for one task type hurt another.** A task-specific workflow prompt combined
  with network blocking took a SWE-Bench score from 96.0 to 66.0 and agent time from 245.5 s to
  821.5 s (`DISTILLATION.md:2213-2218`). Prompt and blocking were not separated.
- **Format constraints degrade reasoning.** "stricter format constraints generally lead to greater
  performance degradation" (https://arxiv.org/abs/2408.02442, abstract). This bears on typed action
  arguments (*inferred*).
- **Open-ended work.** "Agents can be used for open-ended problems where it's difficult or impossible
  to predict the required number of steps" (Anthropic article). A protocol for such work is either
  loose or wrong (*inferred*).
- **The check is the bottleneck, and Canon does not supply it.**
  - BOBA's gains use an oracle. Under handoff, a false-alarm rate of 0.5 gave back Opus 5.5's whole
    gain (`DISTILLATION.md:1053-1058`).
  - The repair round raised regression rates by 3.2 to 8.9 points and added 47 to 294 calls per chain
    (`papers/2611.02207.md:74-79`).
  - A pre-commit check covers only the pattern it encodes; a new form of drift appeared within 1 to
    9 days (`DISTILLATION.md:1605-1610`).
- **State explosion.** Model checking's main drawback (Clarke, above). `canon check` stops at 65,536
  states. A simulation over action sequences would grow faster than over evidence sets (*inferred*).
- **Where Canon does not help:**
  - the quality of what an agent writes inside one step;
  - tasks with no decidable check;
  - a protocol that is itself wrong;
  - whether the runtime honours the binding: "Canon cannot prove that the runtime honours the
    binding" (`loom-sdk-and-agent-node.md:170-172`);
  - the cost of writing and maintaining protocols.
  - For an LLM step, the reachable outcomes are classes of evidence, not of content. The number of
    outcomes is therefore a count over what the protocol can see (*inferred*).
- **The organisation's own research does not yet support the thesis:** 0 supported principles, and
  AP-001 is a seed (section 0).

## 3. What Canon can measure to prove the claim

Pre-register each experiment: hypothesis, number of runs, stopping rule, and a planted known-bad case
for every grader (`agentic-principles/AGENTS.md`, § Evaluations). Every design below is *inferred*.

| # | Experiment | Arms | Measures |
|---|---|---|---|
| E1 | Same task chains, free agent against Canon-governed. Source: BOBA-bench chains (public, `papers/2611.02207.md:21`); runner: the SDC driver or Loom | Same model, harness, budget. Arm A: free agent. Arm B: an outcome is legitimate only on program-decided evidence at the current revision, using only checks visible to the agent, not the hidden oracle | chain completion; carry-forward of unverified mistakes; regression rate; tokens and cost; wall time; human interventions |
| E2 | False "done" | as E1 | share of runs that declare done (A) or reach a legitimate outcome (B) while the hidden check fails |
| E3 | Out-of-protocol actions | as E1, with planted tempting actions (deletes, force-push in a fixture) | actions attempted against actions executed; selector overhead in ms |
| E4 | Analysis yield before any run | mutate real protocols (`sdc/docs/limitations/*.protocol.yaml`) by dropping a guard, adding an outcome or removing a producer | share of mutants `canon check` and `canon generate` catch, against share caught by N agent runs; cost of each |
| E5 | Resume after crash | kill at a random step; resume from the event log against a cold restart | success, tokens, duplicated side effects |
| E6 | State-space headroom | every protocol in ELS and SDC | state count against the 65,536 bound; protocol size at which it refuses |
| E7 | The "universes" claim | per protocol: number of judgement edges and reachable outcome classes (needs `author` and `decided_by`) | does a higher share of judgement edges predict a higher observed failure rate across E1 runs? |

E1 and E2 test the core claim. E4 tests the proactive-analysis claim. E7 is the only test of the operator's
universe-counting idea, and it needs the proposed fields first.

## 4. Positioning

Multi-step LLM agents lose reliability with every step. Their own tests and LLM reviewers catch few
of their mistakes, and an unverified mistake raises the chance that the next step fails. An external
check after a step, or a runtime limit on allowed actions, measurably improves success and safety.
How much depends on how good the check is. Canon is a deterministic layer that decides when evidence
from such checks is enough: an outcome counts only on evidence bound to the current revision, and
the agent sees only the actions that are currently allowed. Before any run, Canon can check a
protocol's finite state space, up to 65,536 states, for unreachable outcomes, dead actions and
authority bypass. Canon does not make a weak check strong or make an open-ended task predictable. It
has not yet been measured against a free agent on the same tasks. The planned comparison (E1-E2 above)
is what would turn this design argument into evidence.
