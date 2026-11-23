# Canon research 04: techniques that shrink an agent's branches and surface

Question (the operator, 2026-11-20): an SDC is a set of Canon protocols, and some of its nodes are agent
nodes. He will test from very open nodes down to very restricted ones with few tools. The goal is
fewer reachable universes and a smaller agent surface. His empirical idea: run a node at a given
freedom many times, see which tool the successful runs use at each place, then grant exactly what
the working runs used. Which other techniques exist, deterministic ones included?

Read on 2026-11-20, not repeated here: `02-branching-and-universes.md` (universe levels, metrics
P1-P15, reductions), `03-why-good-for-agents.md`, `../loom-sdk-and-agent-node.md`, `../layers.md`,
`../boundary.md`. Also read: Canon at `8f1f452` (`website/docs/reference/cli.md`,
`website/docs/concepts/protocols.md`); the stories `canon-simulate`, `check-subject-slices`,
`missing-evidence`, `canon-mutate`, `semantic-diff` in the tree `canon-wave-2026-11-20-w1`
(cited `story/<name>.md`); Loom `AGENTS.md`, `examples/agents/operator.yaml`,
`crates/loom-commission-testkit/src/fake_executor.rs`; Atlas ADRs 0072 and 0083; the `AGENTS.md` or
`README.md` of mcp, gates, mandate and governor; `agentic-principles/docs/principles.json`;
`research/literature/` (`DISTILLATION.md`, `papers/2609.08371.md`). Nothing was built or run.
*(inferred)* marks a statement not read in a source. Three research sub-agents opened the web
sources (§ 8); "abstract" means only the abstract or landing page was read. Check a number in its
source before quoting it elsewhere.

## 0. Short answer

- An agent node has three surfaces, cut in three places *(inferred framing)*: the **protocol
  surface** (which actions are admissible, which results and artifacts they may produce: Canon),
  the **execution surface** inside one action (tools, arguments, files, hosts, budget: the Loom
  binding, enforced by Substrate and Mandate), and the **token surface** (what the model can emit:
  the decoder). ADR 0072 already composes the first two: "visible candidates = available
  integrations ∩ protocol-admissible actions ∩ environment capabilities"
  (Atlas ADR 0072).
- Deterministic techniques prove things about the protocol and binding and cost no tokens, but
  never see what an agent does inside a step. Empirical techniques see that, but guarantee nothing
  about an unobserved run: their guarantee is a statistical bound over the sampled run
  distribution. Trace-based profiling "is not sound" and misses paths not exercised in training;
  static analysis is sound but over-approximates (Confine, § 4).
- They compose: Canon's graph lists the places (states, result classes) that exist; the eval
  harness must sample each one; a mined restriction must pass Canon's checks before adoption (D9,
  § 6) *(inferred)*.
- Best value for cost (§ 5): (1) usefulness and dead-action tightening in Canon, (2) a tool
  allow-list per protocol action, not per agent, (3) strict argument schemas with constrained tool
  calls, (4) scripted-agent simulation plus the reduction gate, (5) the operator's mined allow-list,
  validated on fresh runs under the restriction.
- "Grant what 100 % of the working runs used" guarantees in-sample coverage only. With zero
  tool-shortage failures in `n` fresh runs under the restricted set, the 95 % upper bound on that
  failure rate is `1 - 0.05^(1/n)`, about `3/n` (§ 6).

## 1. What already restricts an agent here

| Mechanism | What it cuts | Where |
|---|---|---|
| Frontier projection | the model "sees and can execute only what the current frontier admits", authority rechecked before every effect | `loom/AGENTS.md:20-22`; ADR 0072 |
| Judgement-step narrowing | a judgement step runs narrowed to its `actions`, "never widening it" | `loom/AGENTS.md:1509-1511` |
| Agent tool policy | `tools.allow`, `deny`, `egress`, `decisions: ask`, per agent | `loom/examples/agents/operator.yaml:17-23`; Claude Code is launched "offering only `allow` less `deny`" (`loom/AGENTS.md:1381-1383`) |
| Per-intent action and source lists | each state of the operator agent names its actions and sources | `operator.yaml:27-30` |
| Authority equals the policy | a served spec's authority is exactly `policy.capabilities` | `loom/AGENTS.md:141`; `operator.yaml:37` |
| Forced decisions | the narrowed catalogue in one forced `select_action` request, then one forced `action_arguments` request with the action's schema | `loom/AGENTS.md:1283-1286` |
| Per-call decision record | `SessionReport::tool_decisions`: each call's decision and decider from `trace-ir/1`; unknown, never empty, when the census disagrees | `loom/AGENTS.md:1427-1436` |
| Egress apertures | a confined child reaches only its named apertures, at most four | `loom/AGENTS.md:155` |
| Capability ceilings | an agent may do "the intersection of every input"; exchange "never widens authority" | `mandate/README.md:20`; `mandate/docs/architecture/combined.md:9` |
| MCP annotations | "never grant either consumer anything" | `mcp/README.md:15` |
| Scripted executor | a fake `AgentExecutor` that returns scripted outcomes and logs each frontier it was handed | `loom/crates/loom-commission-testkit/src/fake_executor.rs:1-2, 21-26` |
| Principle | AP-001 "Bound autonomy with invariants", freedom only inside the envelope; maturity `seed` | `agentic-principles/docs/principles.json:6-9` |

Missing for the operator's ladder: a tool policy per protocol action. Today it is per agent, and a run has
one executor (`../loom-sdk-and-agent-node.md:76`).

## 2. Deterministic techniques

None needs a model call. "Guarantees" is relative to the protocol model: Canon cannot prove that
the runtime honours it (`../loom-sdk-and-agent-node.md:170-172`).

**D1. Dead-action and unproduced-evidence elimination.**
- Does: delete actions admissible in no state and evidence no action produces.
- Needs: the protocol. `canon check` reports both today (`canon/website/docs/reference/cli.md:19`);
  `canon simulate` adds actions that no order of steps from the empty case reaches
  (`story/canon-simulate.md:63, 91`).
- Guarantees: removing a dead action changes no reachable state, so no universe is lost; dead code
  "does not affect the program results" (Wikipedia, Dead-code elimination). Does not: shrink what an
  agent does inside a live action.
- Cost: none for `check`; `simulate` is an active story. Lives: Canon kernel.

**D2. Usefulness tightening (admissible but useless actions leave the frontier).**
- Does: offer an action only while some kind it may produce lacks an applicable record; move each
  binding's `until` into the action's precondition.
- Needs: protocol and case. `missing_evidence` in `canon-decision/1`
  (`story/missing-evidence.md:60-65`); the `until` move (`../boundary.md:39-40, 154-162`).
- Guarantees: the choice fork at a state shrinks to actions that can still change a dimension.
  Does not: predict that the action succeeds (`story/missing-evidence.md:65`).
- Cost: one pure function (active story) and a protocol edit. Lives: Canon kernel; SDC protocols.

**D3. Narrow result variants and `may_produce`.**
- Does: declare the results each produced kind may carry; drop kinds an action never needs.
- Needs: a language change; `may_produce` holds kinds only (`story/canon-simulate.md:95`).
- Guarantees: the result fork `b_res(a)` falls with the class count (02 § 1, § 3b). Does not: stop a
  runtime writing another result; a binding check at load must refuse it
  (`../loom-sdk-and-agent-node.md:187-191`).
- Cost: one model and ESS change. Lives: Canon kernel.

**D4. Argument schema tightening.**
- Does: enums instead of free strings, `additionalProperties: false`, bounded lengths in each
  action's `arguments`.
- Needs: the protocol; Canon holds schemas to its dialect (`../boundary.md:75`).
- Guarantees: every proposal outside the schema is refused before its effect. Does not: constrain
  content inside an allowed string. Cost: protocol edit. Lives: Canon protocol; checked by the runner.

**D5. Constrained decoding and forced tool calls.**
- Does: mask tokens so the model can only emit a call to an offered tool with schema-valid
  arguments (Geng et al. 2023; Outlines; Anthropic strict tool use; OpenAI Structured Outputs).
- Needs: the argument schema; a provider or decoder that supports it.
- Guarantees: syntax only, and only for the schema subset a framework accepts; acceptance varies
  widely between frameworks (JSONSchemaBench § 5). A refusal or `max_tokens` can still break the
  schema (OpenAI and Anthropic limitation sections). Does not: pick the right tool or value. Masking
  "distorts the LLM's learned language distribution" (Grammar-Aligned Decoding § 1). Format
  restrictions lowered reasoning scores in Tam et al. 2024; a rerun by the Outlines maintainers
  found no loss.
- Cost: low; Loom already forces both requests (`loom/AGENTS.md:1283-1286`). Anthropic's docs list
  `tool_choice: any|tool` as returning HTTP 400 on Opus 5.5 and Sonnet 5.5, with `auto` plus
  `strict` as the replacement; whether llm's forced request is affected was not checked. Lives:
  Loom, through llm.

**D6. Minimal action set that preserves every reachable outcome.**
- Does: find the smallest set of actions `A'` such that each outcome reachable with all actions is
  still reachable with `A'` alone. A set cover: each action covers the outcomes it lies on a path
  to *(inferred formulation)*.
- Needs: the `canon simulate` graph, walked again per candidate set.
- Guarantees: outcome reachability, exactly, in the model. Does not: keep alternative routes, so a
  node whose one remaining action fails at run time becomes stuck; it trades redundancy for
  surface *(inferred)*.
- Cost: set cover is NP-hard; greedy is within `H(n) <= ln n + 1` of optimal, and `(1 - o(1)) ln n`
  is the best possible unless P = NP (Wikipedia, Set cover problem). With the few actions of an SDC
  protocol, enumerating subsets is feasible *(inferred)*. Lives: a Canon-side analysis command;
  `canon simulate --without <action>` would be its primitive *(inferred)*.

**D7. Least restrictive shield (supervisory control).**
- Does: per state, remove only the actions from which every continuation is stuck. Ramadge and
  Wonham's supremal controllable sublanguage is unique and gives the "least restrictive
  controller" (libFAUDES docs; Wonham and Ramadge 1987). A shield "restricts the agent as little as
  possible and forbids actions only if they could endanger safe system behavior" (Alshiekh et al.
  2018, § 1).
- Needs: the `canon simulate` graph plus the `no-completion` finding proposed in 02 § 3a.
- Guarantees: every remaining choice keeps the option to complete, and no completing run loses a
  step. Result forks the agent does not control enter as uncontrollable events.
- Cost: one backward pass over the walk graph *(inferred)*. Lives: Canon analysis, written back as
  preconditions so Canon's own frontier enforces it.

**D8. Input slicing.**
- Does: give an agent node only the artifacts and earlier evidence that its precondition and its
  produced kinds' readers depend on: a static slice, "the set of program statements … that may
  affect the values at some point of interest" (Wikipedia, Program slicing). The binding's
  `inputs.files` becomes derived, not hand-picked.
- Needs: the protocol (the slice code of `story/check-subject-slices.md:51-57`).
- Guarantees: nothing a downstream check reads is withheld *(inferred)*. Does not: know what the
  agent needs to do good work inside the step. Cost: low. Lives: Canon analysis → Loom binding.

**D9. Reduction gate: semantic diff plus simulate.**
- Does: apply every proposed restriction, deterministic or mined, as a protocol diff; accept it only
  when `canon diff` classes it `TIGHTENING` (no `RELAXATION`, `EXPANSION`, `BREAKING`) and
  `canon simulate` shows no new `unreached-outcome` or `stuck-state`.
- Needs: semantic diff (implemented, `story/semantic-diff.md:34, 75-79`) and simulate (active).
- Guarantees: a restriction never widens anything and never strands an outcome in the model. Does
  not: cover the binding half, which Loom checks at load. Cost: two commands in CI. Lives: SDC eval
  harness, calling Canon.

**D10. Scripted-agent simulation with stubbed tools and fault injection.**
- Does: run the real binding and runtime with a scripted or seeded random executor instead of a
  model, stub tool results, and force each result class, failures included, to find stuck states,
  missing bindings and loops before spending tokens.
- Precedent: FoundationDB simulates a whole cluster deterministically in one thread with "perfect
  repeatability" and injected faults (FoundationDB testing docs); Antithesis: "bugs can be reliably
  reproduced". Test doubles for models exist off the shelf: Pydantic AI `TestModel` and
  `FunctionModel`, Inspect AI `mockllm`, VCR-style cassettes. Stateful property tests search for
  "sequences of those actions that result in a failure" and shrink them (Hypothesis stateful docs).
- Needs: Loom's `ScriptedExecutor` (`fake_executor.rs:21-26`) or `FirstAdmissibleSelector`
  (`../loom-sdk-and-agent-node.md:46-48`); the result classes per node from `canon simulate`.
- Guarantees: each state the script reaches is checked against the real runtime, replayable from a
  seed. Does not: say what a model will choose. Measured: with the model stubbed and the tool live,
  cut-point replay killed 51 of 192 mutants that a stub-everything test killed none of
  (`research/literature/DISTILLATION.md:759-764`, paper 2609.20625, authors' own incidents).
- Non-deterministic variant: emulate the tools with an LM (ToolEmu: 68.8 % of found failures valid
  in the real world, abstract).
- Cost: low; no tokens. Lives: Loom testkit; SDC eval harness.

**D11. Least privilege in the binding and the substrate.**
- Does: deny by default, "Base access decisions on permission rather than exclusion"; give each
  action "the least set of privileges necessary to complete the job" (Saltzer and Schroeder 1975,
  § I.A.3): tools, apertures, writable roots, secret slots.
- Agent systems: Progent checks "symbolic rules over tool names and arguments" deterministically;
  attack success on AgentDojo 39.9 % → 1.0 % (§ 8.1). CaMeL solves 77 % of AgentDojo tasks "with
  provable security" against 84 % undefended (abstract).
- Needs: the binding (`../loom-sdk-and-agent-node.md:183`) and Substrate profiles
  (`../layers.md:148-150`).
- Guarantees: exactly what the enforcer enforces. A tool list did not enforce GitHub permissions in
  hand-built setups (`DISTILLATION.md:3359-3361`); a static task-independent deny list let the
  injected effect through in 46/75 runs against 47/75 with none (`papers/2609.08371.md:60-66`).
- Cost: low per binding; the per-action binding is not built. Lives: Loom binding; Substrate; Mandate.

## 3. Empirical techniques

Each needs runs and an oracle that decides whether a run worked. The oracle caps every guarantee:
a false success puts what that run used into the allow-list *(inferred)*.

**E1. Mined allow-list by profiling (the operator's rule).**
- Does: run the node at a freedom, record the tool calls of successful runs, grant their union.
- Precedent: AppArmor `aa-genprof` puts a profile in complain mode, asks the user to "exercise its
  functionality" and builds the profile from the logs (man page); `oci-seccomp-bpf-hook` allows "all
  the syscalls made" and denies the rest (README); AWS IAM Access Analyzer grants "permissions that
  the entity used in your specified date range", up to 90 days back, and warns "Do not use policy
  generation for auditing purposes" (docs). AgenTRIM infers an agent's tool interface from code and
  recorded executions and filters tools per step (abstract, no numbers).
- What profiling misses: "dynamic analysis is not sound, and thus can miss system calls along
  execution paths that were not exercised during the training phase"; for Nginx, "Extending the
  training phase for a longer duration would not solve the problem"; error and exception paths are
  the typical misses; static analysis found 160 syscalls where a dynamic study found 76 (Confine,
  §§ 4, 7, 9.1). Logs "provide only a lower bound on the entitlements" (Xu and Stoller § 1).
- Needs: per-call records (`SessionReport::tool_decisions`, `loom/AGENTS.md:1427-1436`), an oracle,
  `n` runs. Guarantees: in-sample coverage; out of sample, § 6. Cost: `n` runs at the open freedom
  plus validation runs. Lives: SDC eval harness; output into the protocol or binding (§ 7).

**E2. Per-node, per-state mining.**
- Does: one allow-list per protocol action (and per frontier state), not per agent.
- Evidence: a tool subset per sub-agent, fixed before untrusted text is read, let the injected
  effect through in 3/75 runs, against 33/75 for one task-wide policy and 47/75 with none; repairs
  68/75 against 72/75; mean time 316 s against 145 s (`papers/2609.08371.md:30-49`; 5 tasks, one
  small model, authors' own system).
- Needs: as E1, with each call tagged by action and state. Guarantees: as E1, per node; each node
  needs its own sample *(inferred)*. Cost: E1 times the node count. Lives: eval harness → binding
  per action.

**E3. Argument invariant mining.**
- Does: infer likely invariants over tool arguments (paths under one root, file kinds, command
  shapes) and turn them into schemas or path rules. Daikon infers "likely invariants" and filters
  coincidences by a statistical test, which "is not a confidence on the likelihood that the reported
  invariants are correct over all possible inputs" (Ernst et al. 2001, § 3.1).
- Why: a name-level list over a universal tool (`Bash`) cuts nothing *(inferred)*. Needs argument
  traces; whether `tool_decisions` carries arguments was not checked. Cost: low once traces exist.
  Lives: eval harness → D4 or the binding.

**E4. Sequence automaton inference.**
- Does: learn the allowed order of tool calls from successful traces (k-tails, Synoptic, L*) and
  enforce it as a sequence shield.
- Guarantees: Synoptic's model "satisfies exactly the temporal invariants mined from the log" but
  "may accept traces that were not present in the log" (FSE 2011, § 3.3). From positive examples
  alone, some language families are not learnable at all (Gold 1967, via Wikipedia); L* needs both
  membership and equivalence queries (Wikipedia, Induction of regular languages), and no run can
  answer an equivalence query *(inferred)*. A fixed automaton monitor over tool calls covered 68-75 %
  of attacks at low attack entropy and 6-13 % at high (arXiv 2608.01388, abstract).
- Cost: moderate. Lives: Loom runtime. Inside a Canon node the order of actions is fixed by the
  protocol already, so it pays only for long free-form steps *(inferred)*.

**E5. Conformance and precision of runs against the protocol.**
- Does: replay case logs over the protocol and measure fitness and precision, two of the "four
  competing quality dimensions" (Process Mining Manifesto, C6). Low precision marks actions and
  results no run uses: candidates for D1-D3 and D7.
- Needs: case event logs. Guarantees: a measurement; it restricts nothing. Logs hold "only example
  behavior" (Manifesto). Cost: low. Lives: SDC eval harness.

**E6. Tool ablation by delta debugging.**
- Does: shrink the tool set with ddmin until removing any single tool breaks success.
- Guarantees: "ddmin(c) is 1-minimal" (Zeller and Hildebrandt 2002, Prop. 11), for a deterministic
  test; with a model, each test needs `k` repeats and carries their statistical error *(inferred)*.
- Cost: `|c|² + 3|c|` tests worst case, `2 log2 |c|` best (Props. 12, 13), each times `k` runs.
- Side benefit: fewer tools can raise accuracy. Selection accuracy 43.13 % with tool retrieval
  against 13.62 % without (RAG-MCP); OpenAI advises "fewer than 20 functions" per turn; SWE-agent's
  interface ablations moved resolution between 10.3 % and 18.0 % (Table 3). Lives: eval harness.

**E7. Restriction ladder with a Pareto front and a bandit.**
- Does: the operator's open-to-restricted ladder scored on (success, cost, surface), keeping levels no other
  level beats on all three; successive or median elimination stops sampling levels that are clearly
  worse, in `O((n/ε²) log(1/δ))` samples (Even-Dar et al. 2006); Pareto active learning predicts the
  front with fewer evaluations (Zuluaga et al. 2013, abstract). AFlow searches workflows by MCTS
  (abstract).
- Guarantees: the chosen level is `ε`-close to the best tested one with probability `1 - δ`, on the
  tested tasks. Cost: below a fixed `n` per level. Lives: eval harness.

**E8. Edge probabilities from runs.**
- Does: estimate `p_e` per non-deterministic edge (02 P12) and spend restriction where the failure
  mass is. ProbGuard learns a DTMC with an `(ε, δ)` PAC statement, and notes its 30 traces per
  scenario are "well below the sample sizes required for formal PAC certification". Lives: outside
  Canon (02 § 4 item 5).

**E9. Model-proposed ceiling (neither class).**
- Does: one trusted model call proposes the task's capabilities before untrusted text is read;
  host code freezes them (`papers/2609.08371.md:33`); Progent and Conseca generate policies the same
  way. Guarantees: no completeness: 2 of 75 runs failed because the ceiling omitted source write
  (`papers/2609.08371.md:63-70`). Cost: one call per task. Lives: Loom runtime.

## 4. Summary

| # | Technique | Kind | Needs | Lives |
|---|---|---|---|---|
| D1 | dead action, unproduced evidence | deterministic | protocol | Canon kernel (exists) |
| D2 | usefulness, `until` → precondition | deterministic | protocol, case | Canon kernel (active story); SDC protocols |
| D3 | narrow result variants | deterministic | language change | Canon kernel |
| D4 | argument schema tightening | deterministic | protocol | Canon protocol; runner |
| D5 | constrained decoding, forced calls | deterministic | schema, decoder | Loom via llm |
| D6 | minimal action set (set cover) | deterministic | simulate graph | Canon analysis command |
| D7 | least restrictive shield | deterministic | simulate graph, `no-completion` | Canon analysis → preconditions |
| D8 | input slicing | deterministic | protocol | Canon analysis → Loom inputs |
| D9 | reduction gate (diff + simulate) | deterministic | both commands | SDC eval harness |
| D10 | scripted simulation, fault injection | deterministic | testkit, stubs | Loom testkit; eval harness |
| D11 | least privilege in binding, substrate | deterministic enforcement | binding, profiles | Loom; Substrate; Mandate |
| E1 | mined allow-list (100 % rule) | empirical | runs, oracle, call records | eval harness → § 7 |
| E2 | per-node mining | empirical | runs tagged by action | eval harness → binding |
| E3 | argument invariant mining | empirical | argument traces | eval harness → D4 |
| E4 | sequence automaton | empirical | traces | Loom runtime |
| E5 | conformance, precision | empirical | case logs | eval harness |
| E6 | delta-debugging ablation | empirical | oracle, `k` repeats | eval harness |
| E7 | ladder, Pareto front, bandit | empirical | runs per level | eval harness |
| E8 | edge probabilities | empirical | runs | outside Canon |
| E9 | model-proposed ceiling | generated | one model call | Loom runtime |

## 5. Ranked by value for cost

1. **D2 + D1, usefulness and dead-action tightening.** Value: removes choices at every agent node
   at once; the first driver run looped on an admissible, useless action
   (`story/missing-evidence.md:72`). Cost: `canon check` reports dead actions today (`cli.md:19`);
   `missing_evidence` is one pure function in an active story (`story/missing-evidence.md:5, 58-66`);
   `until` is a protocol edit (`../boundary.md:154-162`).
2. **E2 + D11, a tool allow-list per protocol action.** Value: per-agent subsets let 3/75 injected
   effects through against 33/75 for one task-wide policy, with repairs 68/75 against 72/75
   (`papers/2609.08371.md:30-41`); a static deny list did nothing (`:60-66`). Cost: the tool policy
   exists per agent (`operator.yaml:17-23`); per action needs the proposed `loom.bindings/1`
   (`../loom-sdk-and-agent-node.md:239-283`).
3. **D4 + D5, strict argument schemas and constrained calls.** Value: the token surface becomes the
   schema; with Structured Outputs a model scored 100 % on a complex-schema eval against under 40 %
   for an older model without (OpenAI announcement). Cost: Loom already forces both requests
   (`loom/AGENTS.md:1283-1286`); schema tightening is a protocol edit. Keep the constraint on the
   call, not on free reasoning (Tam et al. 2024).
4. **D10 + D9, scripted simulation and the reduction gate.** Value: finds stuck states and broken
   bindings with zero tokens and refuses any restriction that strands an outcome; stubbed-model
   replay killed 51/192 mutants where stub-everything killed 0 (`DISTILLATION.md:759-764`). Cost:
   `ScriptedExecutor` exists (`fake_executor.rs:21-26`); semantic diff is implemented
   (`story/semantic-diff.md`, status `implemented`); simulate is active (`story/canon-simulate.md:5`).
5. **E1, the operator's mined allow-list, validated under the restriction.** Value: the only technique here
   that sees what an agent does inside a step; in Android, about one third of 940 apps were
   over-privileged when permissions were declared by hand (Felt et al. 2011, abstract). Cost: the
   per-call data is already recorded (`loom/AGENTS.md:1427-1436`); the cost is runs, about 60 clean
   validation runs per node for a 5 % bound at 95 % (§ 6).

Not ranked higher: D6 and D7 need `canon simulate` first and trade fallback routes for surface
*(inferred)*; E4 and E6 cost many runs for weak guarantees (Gold; Zeller Prop. 12).

## 6. What "grant the 100 % set" does and does not guarantee

Read the rule as the **union**: the allow-list `S` holds every tool any successful run used. The
intersection (tools every successful run used) fails each run that needed an uncommon tool
*(inferred)*.

It guarantees: every observed successful run would have run unchanged under `S`. Nothing else.

It does not guarantee, with the statistics:

1. **Unseen tools.** The share of tool use that falls on tools outside `S` is the unseen mass,
   estimated by Good-Turing as `N1/N`, the share of observations seen exactly once (Wikipedia,
   Good-Turing). Coverage with a bias correction: `Ĉ = 1 - (f1/n)·[(n-1)f1 / ((n-1)f1 + 2 f2)]`,
   `f1`, `f2` the counts seen once and twice (Chao et al. 2014, Eq. 12). With runs as sampling
   units, `Q1/n` (`Q1` = tools used in exactly one of `n` runs) estimates the expected number of new
   tools in the next run, which bounds the chance that it needs any *(inferred adaptation)*. Many
   singletons means the sample has not saturated.
2. **Validation bound.** Run `n` fresh cases under `S` only. With zero tool-shortage failures, the
   one-sided 95 % upper bound on that rate is `1 - 0.05^(1/n)`, about `3/n`; at 99 %, about
   `4.61/n`; independent trials assumed, good for `n > 30` (Wikipedia, Rule of three). 60 runs bound
   it at 5 %, 300 at 1 %. With `x > 0` failures, use Clopper-Pearson (Wikipedia, Binomial proportion
   confidence interval). Runs at the open freedom are not this test: restriction changes behaviour
   (refusals and retries took mean time from 145 s to 316 s, `papers/2609.08371.md:43-49`).
3. **Out-of-time validation.** Mine on one period, validate on the next; a longer observation
   window gave "higher TPR but also higher FPR", and random k-fold ignores how use drifts (Sanders
   and Yue 2019, §§ 3.2, 5.3).
4. **Survivorship.** Only runs that "passed a selection process" are read (Wikipedia, Survivorship
   bias): a run that failed for lack of a tool it never tried leaves no trace *(inferred)*.
5. **Oracle error.** A false success adds its tools to `S`; a false failure drops a needed one
   *(inferred)*. With a weak reviewer, false alarms gave back a model's whole gain
   (`03-why-good-for-agents.md:191-193`).
6. **Rare branches.** Error paths are what profiling misses, and longer profiling does not fix it
   (Confine §§ 4, 9.1). A protocol result class taken in 1 % of runs is absent from most samples of
   60 *(inferred)*. Fix: stratify by the result classes `canon simulate` lists per node, and force
   each with stubbed results (D10) *(inferred)*.
7. **Distribution shift.** The bound holds for the sampled tasks, model, prompt, harness and
   protocol version; any change is a new population *(inferred)*.
8. **Granularity.** A tool name hides its arguments: `Bash` in `S` admits every command
   *(inferred)*. Mine arguments too (E3).
9. **Safety is not success.** `S` holds whatever successful runs used, a dangerous tool included;
   the rule bounds failures for lack of a tool, not harm *(inferred)*. Over-privileged tool choice is
   common and grows after tool failures (ToolPrivBench, abstract).
10. **Dependence.** Runs sharing a seed, cache or fixture are not independent, so `n` overstates the
    evidence (the rule of three assumes independence).

Precise statement *(inferred, from the rule of three)*: after mining `S` from successful runs and
seeing zero tool-shortage failures in `n_v` independent validation runs under `S`, drawn from the
same task distribution and judged by the same oracle, a new case from that distribution fails for
lack of a tool outside `S` with probability below `1 - 0.05^(1/n_v)` at 95 % confidence. It says
nothing about other task distributions, about harm done with tools in `S`, or about runs the oracle
misjudged.

## 7. How a mined allow-list is expressed

ADR 0083 puts semantic capabilities in Canon and the runtime binding below, with "no layer above
the connection" naming a tool (Atlas ADR 0083).
Per mined fact *(inferred application of ADR 0083)*:

| Mined fact | Expressed in | Form |
|---|---|---|
| no successful run took action `a` in states where `p` holds | Canon protocol | a tighter precondition, through D9 |
| action `a` never produced kind `k` | Canon protocol | drop `k` from `may_produce` |
| kind `k` only ever carried results `r1, r2` | Canon protocol | per-kind result variants (D3, language change) |
| action `a` only ever moved artifact `x` | Canon protocol | `revises: [x]` (`../loom-sdk-and-agent-node.md:153`, proposed) |
| action `a` only touched one resource class | Canon protocol | capability scope (ADR 0083) |
| tool names, argument patterns | Loom binding | `sandbox.tools.allow/deny` per action (`../loom-sdk-and-agent-node.md:260`) |
| files read | Loom binding | `inputs.files` |
| hosts reached | Loom binding | `egress` by aperture name; a binding selects among configured apertures, never declares one (`../layers.md:136-139, 222-225`) |
| tokens, cost, steps used | Loom binding | `budget` at an observed quantile |
| credentials used | Mandate | a narrower ceiling (`mandate/README.md:20`) |

Rule: a fact that changes what is admissible or legitimate goes in the protocol, where Canon can
prove things about it; a fact that names a tool, path, host, model or number goes in the binding
(ADR 0083). A protocol change passes D9; a binding change passes Loom's load checks
(`../loom-sdk-and-agent-node.md:187-191`).

## 8. Sources

Opened on 2026-11-20 by research sub-agents. Most pages came through a fetch tool that summarises;
the Confine, Felt, Sanders-Yue and Chao PDFs were read as text. One sub-agent hit the fetch tool's
limit and read its sources (Daikon through Grammar-Aligned Decoding below) with `curl` instead.

Least privilege and agents
- Saltzer, Schroeder, "The Protection of Information in Computer Systems", 1975, § I.A.3. Full.
  https://web.mit.edu/Saltzer/www/publications/protection/Basic.html
- Debenedetti et al., CaMeL, "Defeating Prompt Injections by Design", 2025. Abstract.
  https://arxiv.org/abs/2503.18813
- Shi et al., Progent, 2025. Full HTML, first part (§ 8.1). https://arxiv.org/html/2504.11703
- MiniScope, 2025. Abstract. https://arxiv.org/abs/2512.11147
- Conseca, "Contextual agent security", HotOS 2025. Abstract. https://arxiv.org/abs/2501.17070
- AgenTRIM, 2026. Abstract. https://arxiv.org/abs/2601.12449
- ToolPrivBench, 2026. Abstract. https://arxiv.org/abs/2606.20023

Profiling and policy mining
- `aa-genprof(8)`. Full. https://manpages.ubuntu.com/manpages/noble/man8/aa-genprof.8.html
- `aa-logprof(8)`. Full. https://manpages.ubuntu.com/manpages/noble/man8/aa-logprof.8.html
- oci-seccomp-bpf-hook README. Full. https://github.com/containers/oci-seccomp-bpf-hook
- Ghavamnia et al., "Confine", RAID 2020. Full. https://par.nsf.gov/servlets/purl/10219922
- Felt et al., "Android Permissions Demystified", CCS 2011. Full.
  https://comp97109.doc.ic.ac.uk/reading/felt_ccs_11.pdf
- Xu, Stoller, "Mining Attribute-based Access Control Policies from Logs", 2014. Full (first part).
  https://ar5iv.arxiv.org/html/1403.5715
- Sanders, Yue, least-privilege policies from audit logs, ACSAC 2019. Full.
  https://par.nsf.gov/servlets/purl/10175651
- AWS IAM Access Analyzer policy generation. Full.
  https://docs.aws.amazon.com/IAM/latest/UserGuide/access-analyzer-policy-generation.html

Statistics
- Rule of three. Full. https://en.wikipedia.org/wiki/Rule_of_three_(statistics)
- Clopper-Pearson, section of. Full. https://en.wikipedia.org/wiki/Binomial_proportion_confidence_interval
- Good-Turing frequency estimation. Full. https://en.wikipedia.org/wiki/Good%E2%80%93Turing_frequency_estimation
- Chao et al., "Rarefaction and extrapolation with Hill numbers", Ecol. Monogr. 84, 2014, Eq. 12.
  Full. https://www.uvm.edu/~ngotelli/manuscriptpdfs/ChaoHill.pdf (Chao and Jost 2012 itself
  returned 403.)
- Survivorship bias. Full. https://en.wikipedia.org/wiki/Survivorship_bias

Specification mining
- Ernst et al., "Dynamically discovering likely program invariants", TSE 2001, § 3.1. Full.
  https://homes.cs.washington.edu/~mernst/pubs/invariants-tse2001.pdf
- Beschastnikh et al., Synoptic, FSE 2011. Full. https://homes.cs.washington.edu/~mernst/pubs/synoptic-fse2011.pdf
- Beschastnikh et al., CSight, ICSE 2014. Full. https://homes.cs.washington.edu/~mernst/pubs/concurrent-models-icse2014.pdf
- Induction of regular languages (L*). Full. https://en.wikipedia.org/wiki/Induction_of_regular_languages
- Language identification in the limit (Gold). Full. https://en.wikipedia.org/wiki/Language_identification_in_the_limit
- Process Mining Manifesto, challenge C6. Full. https://www.tf-pm.org/upload/1580737614108.pdf
- ProbGuard (formerly Pro2Guard), 2025, Def. 3.3, Thm 3.4. Full. https://arxiv.org/abs/2508.00500
- Fixed-invariant monitors over tool calls, 2026. Abstract. https://arxiv.org/abs/2608.01388

Constrained decoding
- Geng et al., "Grammar-Constrained Decoding for Structured NLP Tasks without Finetuning", 2023.
  Full. https://arxiv.org/abs/2305.13971
- Willard, Louf, "Efficient Guided Generation for LLMs" (Outlines), 2023. Full. https://arxiv.org/abs/2307.09702
- JSONSchemaBench, 2025, § 5. Full. https://arxiv.org/abs/2501.10868
- OpenAI, "Introducing Structured Outputs in the API", 2024. Full.
  https://openai.com/index/introducing-structured-outputs-in-the-api/
- Anthropic, strict tool use. Full. https://docs.claude.com/en/docs/agents-and-tools/tool-use/strict-tool-use
- Anthropic, implement tool use (`tool_choice`). Full.
  https://docs.claude.com/en/docs/agents-and-tools/tool-use/implement-tool-use
- Tam et al., "Let Me Speak Freely?", 2024, § 4.1. Full. https://arxiv.org/abs/2408.02442
- .txt, "Say What You Mean" (rebuttal). Full. https://blog.dottxt.co/say-what-you-mean.html
- Park et al., "Grammar-Aligned Decoding", NeurIPS 2024, §§ 1-2. Full. https://arxiv.org/abs/2405.21047

Program analysis and control
- Dead-code elimination. Full. https://en.wikipedia.org/wiki/Dead-code_elimination
- Program slicing. Full. https://en.wikipedia.org/wiki/Program_slicing
- Set cover problem (greedy bound, inapproximability). Full. https://en.wikipedia.org/wiki/Set_cover_problem
- Wonham, Ramadge, "On the supremal controllable sublanguage of a given language", SIAM J. Control
  Optim. 1987. Abstract. https://collaborate.princeton.edu/en/publications/on-the-supremal-controllable-sublanguage-of-a-given-language-2/
- libFAUDES, controllability and synthesis. Full. https://fgdes.tf.fau.de/faudes/reference/synthesis_controllability.html
- Alshiekh et al., "Safe Reinforcement Learning via Shielding", AAAI 2018, § 1. Full (pp. 1-3).
  https://arxiv.org/pdf/1708.08611

Search and ablation
- Zeller, Hildebrandt, "Simplifying and Isolating Failure-Inducing Input", TSE 2002, Props. 11-13.
  Full (pp. 4-7). https://www.st.cs.uni-saarland.de/papers/tse2002/tse2002.pdf
- RAG-MCP, 2025, § 5. Full. https://arxiv.org/html/2505.03275
- "Less is More", tool reduction on edge devices, 2024. Abstract. https://arxiv.org/abs/2411.15399
- Yang et al., SWE-agent, 2024, Table 3. Full. https://arxiv.org/html/2405.15793
- OpenAI function-calling guide ("fewer than 20 functions"). Full.
  https://developers.openai.com/api/docs/guides/function-calling
- Even-Dar, Mannor, Mansour, JMLR 7, 2006. Abstract plus pp. 1082-1084.
  https://www.jmlr.org/papers/v7/evendar06a.html
- Zuluaga et al., Pareto Active Learning, ICML 2013. Abstract. https://proceedings.mlr.press/v28/zuluaga13.html
- Zhang et al., AFlow, 2024. Abstract. https://arxiv.org/abs/2410.10762

Simulation and test doubles
- FoundationDB, Simulation and Testing. Full. https://apple.github.io/foundationdb/testing.html
- Antithesis, deterministic simulation testing. Full.
  https://antithesis.com/docs/resources/deterministic_simulation_testing/
- Pydantic AI, testing guide. Full. https://pydantic.dev/docs/ai/guides/testing/
- Inspect AI `mockllm` provider source. Full.
  https://github.com/UKGovernmentBEIS/inspect_ai/blob/main/src/inspect_ai/model/_providers/mockllm.py
- VCR.py usage. Full. https://vcrpy.readthedocs.io/en/latest/usage.html
- Temporal Python testing suite. Full. https://docs.temporal.io/develop/python/testing-suite
- Hypothesis, stateful testing. Full. https://hypothesis.readthedocs.io/en/latest/stateful.html
- Ruan et al., ToolEmu, 2023. Abstract. https://arxiv.org/abs/2309.15817

Opened but not used: IsolateGPT, FIDES, AgentSpec, ShieldAgent, GuardAgent, Gorilla, ToolLLM,
AnyTool, DSPy, Trace, XGrammar, ATLAS (2608.14352), TigerBeetle safety page, LangChain fake model,
Hughes FMCAD 2011 and "Experiences with QuickCheck", the Princeton PAC lecture notes. Not opened,
so not cited: Angluin 1987, Gold 1967, Biermann and Feldman 1972, Chao and Jost 2012 (403),
Hanley and Lippman-Hand 1983, the Security Profiles Operator docs (404).

## Not verified

- No technique was run; no count, cost or bound here was measured on an SDC node.
- Whether `canon diff` classes every precondition narrowing as `TIGHTENING`; the story lists the
  `all`-conjunct case (`story/semantic-diff.md:78`).
- Whether `tool_decisions` carries call arguments, which E3 needs; the text names tool, decision,
  reason and decider (`loom/AGENTS.md:1427-1430`).
- Whether llm's forced `select_action` uses `tool_choice: any|tool`, which Anthropic's docs list as
  refused on Opus 5.5 and Sonnet 5.5.
