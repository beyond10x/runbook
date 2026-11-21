# Canon research 02: branching, universes, and how to reduce them

The operator, 2026-11-20: a protocol is a graph through time. At each fork (a decision, a condition, a
different evaluation of evidence) a run can take another branch. Canon should know which edges are
deterministic and which are not (an LLM step), how many possible futures ("universes") a protocol
explodes into, globally and per node, and how to lower that number. A Canon (kernel) topic.

Research only. Read: `canon/AGENTS.md`, `canon/website/docs/reference/cli.md`,
`canon/crates/canon/src/check/mod.rs` (canon at `8f1f452`), `canon/crates/canon/src/model/mod.rs`,
`canon/crates/canon/src/eval/{actions,outcomes}.rs`, `sdc/docs/{canon-simulation-request,boundary,
loom-sdk-and-agent-node}.md`, `sdc/docs/limitations/output.txt`, and the drafted stories
`check-subject-slices`, `canon-simulate`, `missing-evidence` in the tree
`canon-wave-2026-11-20-w1/.engineering/planning/story/` (cited below as `story/<name>.md`). Nothing
was built or run. *(inferred)* marks a statement not read in a source; a formula marked so is my
construction. A source listed in § 5 was opened; one that could not be opened is not cited.

## 0. Short answer

- Canon already has the right "universe": a state is a set of evidence records, authority decisions
  and decisions (`check/mod.rs:8-45`), and evaluation is a pure function of it
  (`canon/AGENTS.md:22-26`). Two runs that reach the same set are the same universe for everything
  Canon decides, whatever the order. So the number to report is reachable states and their outcome
  classes, not paths. Paths overcount by every interleaving of independent steps *(inferred)*.
- Canon cannot tell a deterministic edge from an LLM edge today: `may_produce` lists kinds only
  (`model/mod.rs:245-251`) and `effect` carries no meaning (`loom-sdk-and-agent-node.md:123-126`).
  The `author` / `decided_by` fields proposed in `loom-sdk-and-agent-node.md:150-154` are the
  prerequisite for every metric that separates the two.
- Canon's step graph only adds records, so it is a DAG apart from self-loops
  (`story/canon-simulate.md:50`). Every count below (paths, non-deterministic edges per path,
  worst-case universes, entropy bound) is then one dynamic-programming pass over the reachable graph
  that `canon simulate` already builds, linear in its edges *(inferred)*. Counting simple paths in a
  general graph is #P-complete (Valiant 1979), so the DAG property matters.
- Compute first: the static judgement fields and checks (cheap, no state bound), then the counts as a
  by-product of `canon simulate`. Leave probabilities outside the kernel: export to PRISM/Storm.

## 1. Definitions in Canon's terms

**State.** One value per dimension: evidence dimensions (records of one kind about one artifact),
capabilities (undecided, granted, denied), decisions (taken or not), case inputs, and for order
forms history, later-record, last-record and order dimensions (`check/mod.rs:10-45, 104-122`). A
record whose result no match reads behaves as one with no result (`check/mod.rs:24-29`): the space
is already a quotient of all possible records by what the predicates can see.

**Step** (edge), as `story/canon-simulate.md:43-50` defines it:
- *action step*: from a state where the action is `admissible`, add one record of a kind it may
  produce, one per (dimension, class, producer profile);
- *authority step*: a capability goes from undecided to `granted` or `denied`;
- *decision step*: a decision goes from not taken to taken.
Steps only add, so the graph has no cycle except a step that returns its own state
(`story/canon-simulate.md:50`). Action statuses are `admissible`, `approval-required`, `blocked`
(`eval/actions.rs:104-131`); outcome statuses `legitimate`, `blocked` (`eval/outcomes.rs:5-14`).

**Fork.** A state with two or more distinct successors. Five kinds, by who picks the branch:

| Fork | Successors | Who picks | Deterministic? |
|---|---|---|---|
| result fork | one per class (× profile) of each kind the action may produce | the producer of the record | if the producer is a program: yes, given the artifact revision; if a model: no |
| choice fork | one per admissible action | the runtime's selector (`boundary.md:72`) | deterministic selector: yes; LLM chooser (`loom-sdk-and-agent-node.md:105`): no |
| authority fork | `granted` / `denied` | a principal | not a machine edge; an external choice |
| decision fork | taken / not taken | a principal | same |
| input fork | one initial state per input value (`story/canon-simulate.md:52`) | the case opener | fixed per case |

Today the result variants of an action are not declared: `may_produce` names a kind, no result
(`model/mod.rs:248-251`), so the variants are the checker's classes: each result some match reads,
plus one "no result / any other" class (`check/mod.rs:24-29`; `story/canon-simulate.md:92`).

**Deterministic edge.** A step whose successor is a function of the inputs it was computed from (the
protocol, the case snapshot, the artifact revision, the evidence set): running it again on the same
inputs gives the same successor. Canon's own evaluation is always such a function
(`canon/AGENTS.md:22-26`). A program-decided record (`decided_by: program`,
`loom-sdk-and-agent-node.md:154`) is one, provided the runtime honours the binding, which Canon
cannot prove (`loom-sdk-and-agent-node.md:170-172`).

**Non-deterministic edge.** A step whose successor can differ between two runs on the same inputs:
a record whose result a model decides (`decided_by: judgement`), an artifact revision whose content a
model writes (`author: judgement` with `revises`, `loom-sdk-and-agent-node.md:152-153`), and the
choice fork when an LLM selects. "Deterministic" model settings do not make it deterministic: five
LLMs configured to be deterministic varied in accuracy by up to 15 % across runs on the same input
(Atil et al. 2024).

**The key distinction.** A result fork after a program has *epistemic* branching: before the run we
do not know which class comes, but a re-run gives the same one. A fork after a model has *aleatoric*
branching: a re-run can give another. A static fan-out `b` counts both; only the second makes a
repeated run land in another universe. Every metric below keeps them apart *(inferred framing; it
is the MDP split of nondeterministic choice versus probabilistic branching, PRISM manual "Actions
and nondeterminism")*.

**A judgement-authored artifact.** Its content space is unbounded, but Canon sees it only through
the records later steps write about its revision; a record of the earlier revision is set aside
(`loom-sdk-and-agent-node.md:163`). So in Canon's quotient the branching of a judgement edge is the
number of distinct evidence classes the downstream checks can give the new revision, not the number
of possible texts *(inferred, from `check/mod.rs:24-29`)*.

**Universe.** Four levels, each a quotient of the one before:

| Level | One universe is | Count |
|---|---|---|
| path | a maximal path in the step graph | `U_path` |
| state | a reachable sink state (no successor but itself) | `U_state` |
| observation | a sink's vector of action and outcome statuses | `U_obs` |
| outcome | the set of outcomes legitimate at a sink | `U_out` |

`U_path >= U_state >= U_obs >= U_out` *(inferred)*. Canon decides on the state, so `U_state` is
"the number of universes"; `U_obs` is the number that differ in anything Canon reports; `U_out` is
the number a user sees. `U_path` counts histories, which matter for an audit log, not for meaning.

**Branching factor.** Local: `b(v) = |succ(v) \ {v}|`, split by fork kind. Per action `a` at state
`v`: `b_res(a, v) = sum over kinds k in may_produce(a), dimensions d of k, of |classes(d)| *
|profiles(d)|` minus successors equal to `v` *(inferred, from `story/canon-simulate.md:44`)*.
Global effective branching: `b* = U_path^(1/L)` with `L` the mean path length *(inferred)*.

## 2. Metrics Canon could report

`G = (V, E)` is the reachable step graph from the initial states `I` (`canon simulate`). `succ(v)`
excludes `v`. `nd(e) = 1` when edge `e` is non-deterministic, else 0. Sinks have no successor. All
recursions run once per node in reverse topological order, so each costs `O(|V| + |E|)` on top of
the walk *(inferred: `G` is a DAG, `story/canon-simulate.md:50`)*.

### Per protocol

| # | Metric | Formula | Exists |
|---|---|---|---|
| P1 | product space | `prod_d radix(d)`; evidence `2^c`, with `m` upstream artifacts `2^(c*2^m)`, with profiles `n^(c*2^m)`; capability 3; decision 2 (`check/mod.rs:24-82`, `space.rs:847-851`) | yes, `canon check` |
| P2 | slice per subject | product of the radices a subject reads (`story/check-subject-slices.md:51-57`) | drafted |
| P3 | reachable states | `|V|` | drafted (`story/canon-simulate.md:64`) |
| P4 | terminal outcomes | per outcome `o`: `|{v : o legitimate at v}|`; `U_out` = distinct sets of legitimate outcomes over sinks | partly drafted (terminable per outcome) |
| P5 | stuck, cycle, dead, unreached | as `story/canon-simulate.md:57-62` | drafted |
| P6 | path universes | `P(v) = 1` if sink, else `sum_{w in succ(v)} P(w)`; `U_path = sum_{i in I} P(i)` | no *(inferred)* |
| P7 | state universes | `U_state` = number of sinks; `U_obs` = distinct status vectors over sinks | no |
| P8 | max non-deterministic edges on a path | `N(v) = 0` if sink, else `max_{w} (nd(v,w) + N(w))`; `ND_max = max_{i} N(i)` | no *(inferred)* |
| P9 | replay universes (worst case) | `R(v) = 1` if sink; `max_{w} R(w)` at a deterministic fork; `sum_{w} R(w)` at a non-deterministic fork; `U_replay = max_i R(i)`: how many sinks repeated runs of one case in one fixed world can reach. `U_replay = 1` means the protocol is replay-deterministic | no *(inferred)* |
| P10 | bound on replay universes | `U_replay <= prod_{ND forks on the worst path} b_e <= b_max^ND_max` | no *(inferred)* |
| P11 | path entropy bound | `H <= log2 U_path` (`H` is largest, `log n`, when all `n` choices are equally likely: Shannon 1948, Part I, property 2 of `H`); non-deterministic part `H_ND <= log2 U_replay <= ND_max * log2 b_max` | no *(inferred)* |
| P12 | expected universes | with a probability `p_e` on each non-deterministic edge (estimated from logged runs, as Pro2Guard learns a DTMC from traces): `Pr(sink s)` by forward propagation; expected distinct sinks over `k` runs `sum_s (1 - (1 - Pr(s))^k)`; `Pmin/Pmax` of each outcome over the deterministic choices, which an MDP checker computes over all strategies (PRISM manual, P operator) | no *(inferred)*; probabilities are run data, not Canon input |
| P13 | path entropy (with `p_e`) | absorbing chain: `H = sum_v Pr(visit v) * H(p_v)`, `H(p_v) = -sum_w p_vw log2 p_vw`; Ekroot and Cover give the closed form for an irreducible chain with the same single-step entropies `H*` | no *(inferred for the DAG form)* |
| P14 | compounding reliability | per non-deterministic edge success `q_e`: `Pr(no error on path) = prod q_e = q^n`, `n = ND(path)`. Errors that move the run off the states it was checked on can grow as `T^2 * eps` over `T` steps (Ross, Gordon, Bagnell 2011, § 1). Consistency over `k` runs: `Pr(o)^k`, Canon's analogue of `pass^k` (Yao et al. 2024) | no *(inferred)* |
| P15 | growth rate (only once steps can remove records) | a graph with cycles has `N(T) ~ W^T` paths of length `T`, `W` the largest real root of the characteristic equation, capacity `log W` (Shannon 1948, Theorem 1); `A^n[i][j]` counts walks of length `n` (Wikipedia, Adjacency matrix) | not applicable today *(inferred: `revises` would add cycles, `story/canon-simulate.md:184-190`)* |

### Per node (state `v`) and per action `a`

| Metric | Formula |
|---|---|
| local branching | `b(v)`, split into result, choice, authority, decision |
| action fan-out | `b_res(a)` (static, from dimensions and classes; no walk needed) |
| futures from here | `P(v)`, `R(v)`, sinks reachable from `v` |
| non-determinism behind | `max` non-deterministic edges on any path from `I` to `v` (forward pass of P8) |
| non-determinism ahead | `N(v)` |
| entropy to go | `H(v) = H(p_v) + sum_w p_vw H(w)` (needs `p_e`) |
| option to complete | whether some legitimate outcome is reachable from `v` (workflow-net soundness, below) |
| check coverage | whether every path from each non-deterministic edge to a legitimate outcome passes a program-decided record about the same artifact: the run-time form of `unchecked-judgement` (`loom-sdk-and-agent-node.md:163`) *(inferred)* |

All counts can exceed `u128`: report `log2` or a saturating count *(inferred)*.

## 3. Reductions

### 3a. Proven by the checker (Canon computes; the protocol's meaning does not change)

| Reduction | What it computes | Cost | Applies to evidence-set states? |
|---|---|---|---|
| **Order-free state semantics** (partial order reduction, done by construction) | POR explores one representative of interleavings of independent transitions, which "can neither disable nor enable each other" and commute (Flanagan and Godefroid 2005, § 2.2). Canon's evaluation reads a set, so every interleaving of the same steps reaches one state; `U_state` already is the reduced count | free; nothing to build | Yes, without order forms. With order forms, order is a dimension (`check/mod.rs:92-133`) and interleavings differ. Report `U_path / U_state` as the interleaving overcount *(inferred)* |
| **Cone of influence / slicing** | keep only the variables a property depends on, through a dependency graph rooted at the property's variables (Telbisz et al. 2025, § 1, on classical COI) | linear in the protocol | Yes: `story/check-subject-slices.md` Option A. Admits 3 of 4 refused SDC protocols by its own reading (`:84-93`) |
| **Quotient by what predicates read** | classes per dimension (`check/mod.rs:24-29`) | done | Yes; it is why a judgement artifact's text does not explode the space |
| **Bisimulation minimisation** | the coarsest partition where equivalent states have the same labels and the same futures; partition refinement in `O(|P| log |S|)` for a Markov chain (Katoen et al. 2007, § 2); quotienting plus checking often beats checking the original | `O(|E| log |V|)` on the walk graph | Yes: label a state by its status vector; `U_obs` and the minimised graph are the "distinct universes" a user can tell apart *(inferred)* |
| **Symmetry reduction** | one representative per orbit of a symmetry group `G`; at most `|S|/|G|` states, `|G|` up to `n!`; finding the representative is the orbit problem (Miller, Donaldson, Calder 2006, § 4.2, 4.4) | the orbit problem per state | Rarely: only when artifacts or producers are interchangeable in every predicate *(inferred)* |
| **Abstraction refinement (CEGAR)** | an over-approximation whose spurious counterexamples drive refinement (Clarke et al. 2000, abstract) | iterations of check and refine | Only for "some state" findings; not for "no state" ones, which Canon reports (`story/check-subject-slices.md:106`) *(inferred)* |
| **DAG counting** | P6-P11 by dynamic programming | `O(|V|+|E|)` | Yes, because steps only add (`story/canon-simulate.md:50`); simple-path counting in general graphs is #P-complete (Valiant 1979, problem 14) |
| **Workflow-net soundness** | option to complete, proper completion, no dead transitions; for a net without reset or inhibitor arcs, sound iff the short-circuited net is live and bounded; decidable, polynomial for free-choice nets, undecidable with reset arcs (van der Aalst et al. 2011, § 5 Lemma 5.1, § 6) | one backward reachability pass on the finite walk graph *(inferred)* | Yes. `dead-action` is "no dead transitions"; `stuck-state` is weaker than "option to complete" (a state with admissible actions can still reach only stuck states). Add `no-completion` *(inferred)*. General Petri-net reachability is Ackermann-complete (Czerwiński and Orlikowski 2021), but Canon's space is finite by construction, and a net is bounded iff its reachability graph is finite (Wikipedia, Petri net) |

### 3b. A protocol author can apply

| Change | Effect on the numbers | Source |
|---|---|---|
| **Deterministic verifier after each agent node** (`unchecked-judgement`) | every universe an agent edge opens must pass a program-decided class before an outcome; the agent's open content space reaches outcomes only as the gate's classes. With an automatic verifier, more samples become more solved tasks; without one, selection plateaus (Brown et al. 2024) | `loom-sdk-and-agent-node.md:163` |
| **Independent producer for judgement-decided evidence** (`self-judged-evidence`) | an LLM verdict counts only from a producer the case lists as independent; it removes the universe where the author grades itself | `loom-sdk-and-agent-node.md:164` |
| **Narrow result variants** | each result a match reads is a class; a dimension of `c` classes and `m` upstream artifacts has `2^(c*2^m)` values. `c = 2, m = 1`: 16 (the 16s in `output.txt:2,10`); `c = 1`: 4 | `check/mod.rs:47-48` |
| **Collapse gate kinds into one program** | four ×4 kinds become one; costs the explanation naming which check failed | `boundary.md:109` |
| **Order stages by preconditions** | `k` unordered stages of none/pass/fail give `3^k` reachable combinations; "stage `i+1` only after stage `i` passed" gives `2k + 1` *(inferred)* | |
| **Split into child cases** | product becomes per-case spaces: monolith 2,654,208 states (`output.txt:6`), its slices at most 3,456 (`story/check-subject-slices.md:90`) | ADR 0081 via `boundary.md:106` |
| **Deterministic selector** | removes the LLM choice fork at every step; Loom's test selector and the driver's policy exist | `boundary.md:72`; `loom-sdk-and-agent-node.md:77, 89` |
| **Fewer agent edges per path** | `Pr(no error) = q^n`: each agent edge removed or checked raises it geometrically | P14 *(inferred)* |
| **Constrain the agent's output** (grammar, automaton) | narrows content, not the non-determinism: Formal-LLM supervises plan generation with an automaton (Li et al. 2024); AgentSpec enforces trigger/predicate/enforcement rules at run time (Wang, Poskitt, Sun 2025) | runtime, not Canon |
| **Sample and vote** | majority over sampled reasoning paths raises accuracy (Wang et al. 2022): it lowers `1 - q` on one edge | binding, not Canon |
| **Human decision or authority after an agent edge** | a principal's explicit choice replaces a model's at that fork | Canon has both (`check/mod.rs:30-32`) |

Field evidence that per-step checks alone are not enough: one review reports that blocking 94 % of
unsafe actions left fewer than 5 % of tasks done safely and correctly in most settings, and that
run-time monitors guarantee safety only per step or short horizon (Dantas et al. 2026, abstract).
Whole-path properties are what Canon's checker can add. Static verifiers of agent workflow graphs
report dead-end nodes and unreachable exits, and check policies as a graph-by-DFA product
(Agentproof, Xavier et al. 2026); CaMeLoT checks CTL policies over a plan's transition system with
nuXmv before any tool runs (Nikolaou et al. 2026). Canon's `canon check` and `canon simulate` are of
this kind, over evidence rather than control flow *(inferred comparison)*.

## 4. Recommendation: what Canon computes first

Costs are *(inferred)* from the cited code and stories; nothing was built.

1. **`author`, `revises`, `decided_by` in `protocol/1`, with the static checks `unchecked-judgement`
   and `self-judged-evidence`** (`loom-sdk-and-agent-node.md:150-166`). Without them every metric
   shows zero non-deterministic edges. Static over predicates, so the 65,536 bound does not apply.
   Cost: one model and ESS change, one validate/check rule each, fixtures; defaults keep existing
   protocols valid (`:156`). One story.
2. **Static per-action numbers in `canon check`**: `b_res(a)` and whether `a` is judgement-authored
   or decided, from the dimensions the check already builds. No walk. Cost: small, the radices exist
   (`space.rs:723, 847`).
3. **Counts in `canon simulate`**: P3, P4, P6 to P11, per-node `N(v)` and `R(v)`, and `no-completion`.
   One reverse-topological pass over the graph the walk already holds; the walk (at most 65,536
   evaluated states, `story/canon-simulate.md:69`) stays the dominant cost. Report counts as `log2`.
   Cost: an extension of a drafted story, or a follow-up story right after it.
4. **Bisimulation quotient of the walk graph by status vector** (`U_obs`). `O(|E| log |V|)`. Useful
   once protocols are large enough that `U_state` hides how few distinct universes exist.
5. **Do not build in Canon**: probabilities, expected universes and entropy with `p_e`. They need run
   data, which Canon must not read (`boundary.md:25, 70`). Export the walk graph as a PRISM `mdp` or
   Storm model (choice, authority and decision forks as nondeterminism, judgement edges as
   probabilistic choices), and let Loom or a separate tool fill `p_e` from run logs, as Pro2Guard
   learns a DTMC from traces (Wang et al. 2025). Not worth it now: explicit POR (state semantics
   already does it) and symmetry reduction (little symmetry expected).

Prerequisite outside this list: `check-subject-slices` raises what `canon check` can see;
`canon simulate` is what this document's counts hang on.

## 5. Sources

Opened on 2026-11-20. "Full text" means the PDF or HTML body was read; "abstract" means only the
abstract page.

- Atil et al., "Non-Determinism of 'Deterministic' LLM Settings", 2024. Abstract.
  https://arxiv.org/abs/2408.04667
- Brown et al., "Large Language Monkeys: Scaling Inference Compute with Repeated Sampling", 2024.
  Abstract. https://arxiv.org/abs/2407.21787
- Clarke, Grumberg, Jha, Lu, Veith, "Counterexample-guided Abstraction Refinement", CAV 2000. Full
  text (course copy). http://web.stanford.edu/class/cs357/cegar.pdf
- Czerwiński, Orlikowski, "Reachability in Vector Addition Systems is Ackermann-complete", 2021.
  Abstract. https://arxiv.org/abs/2104.13866
- Dantas, Cordeiro, Nowroozi, Tihanyi, "Toward Safe LLM Agents: A Survey of Specification,
  Verification, and Enforcement", 2026. Abstract. https://arxiv.org/abs/2608.14590
- Ekroot, Cover, "The Entropy of Markov Trajectories", IEEE Trans. Inf. Theory 39(4), 1993. Full
  text (author copy). https://isl.stanford.edu/~cover/papers/transIT/1418ekro.pdf
- Flanagan, Godefroid, "Dynamic Partial-Order Reduction for Model Checking Software", POPL 2005.
  Full text. https://escholarship.org/content/qt47c9f29c/qt47c9f29c.pdf
- Hensel, Junges, Katoen, Quatmann, Volk, "The Probabilistic Model Checker Storm", 2020. Abstract.
  https://arxiv.org/abs/2002.07080
- Katoen, Kemna, Zapreev, Jansen, "Bisimulation Minimisation Mostly Speeds Up Probabilistic Model
  Checking", TACAS 2007. Full text.
  https://mrmc.moves.rwth-aachen.de/downloads/Paper/KatoenKZJ_TACAS07.pdf
- Li, Hua, Wang, Zhu, Zhang, "Formal-LLM", 2024. Abstract. https://arxiv.org/abs/2402.00798
- Miller, Donaldson, Calder, "Symmetry in Temporal Logic Model Checking", ACM Computing Surveys
  38(3), 2006. Full text (author copy). https://www.doc.ic.ac.uk/~afd/papers/2006/ACMSurvey.pdf
- Nikolaou et al., "CaMeLoT: CaMeL orchestrated with Temporal logic for static verification and
  liveness", 2026. Abstract. https://arxiv.org/abs/2609.18674
- PRISM manual, model types: https://www.prismmodelchecker.org/manual/ThePRISMLanguage/ModelType ;
  actions and nondeterminism:
  https://www.prismmodelchecker.org/manual/ThePRISMLanguage/ActionsAndNondeterminism ; P operator
  (Pmin/Pmax over strategies): https://www.prismmodelchecker.org/manual/PropertySpecification/ThePOperator
- Ross, Gordon, Bagnell, "A Reduction of Imitation Learning and Structured Prediction to No-Regret
  Online Learning", AISTATS 2011 (DAgger). Full text; the `T^2 eps` statement is in § 1.
  https://proceedings.mlr.press/v15/ross11a/ross11a.pdf
- Shannon, "A Mathematical Theory of Communication", 1948. Full text; Theorem 1 (capacity `log W`).
  https://people.math.harvard.edu/~ctm/home/text/others/shannon/entropy/entropy.pdf
- Telbisz, Bajczi, Szekeres, Vörös, "On-the-fly Cone-of-Influence Reduction for Model Checking
  Concurrent Software", SPIN 2025. Full text. https://csanadtelbisz.github.io/publications/spin25coi.pdf
- Valiant, "The Complexity of Enumeration and Reliability Problems", SIAM J. Comput. 8(3), 1979.
  Full text (course copy); problem 14, S-T PATHS.
  https://www.math.cmu.edu/~af1p/Teaching/MCC17/Papers/enumerate.pdf
- van der Aalst et al., "Soundness of Workflow Nets: Classification, Decidability, and Analysis",
  Formal Aspects of Computing 23(3), 2011. Full text (accepted manuscript).
  https://www.vdaalst.com/publications/p628.pdf
- Wang, Poskitt, Sun, "AgentSpec: Customizable Runtime Enforcement for Safe and Reliable LLM
  Agents", 2025. Abstract. https://arxiv.org/abs/2503.18666
- Wang, Poskitt, Wei, Sun, "Pro2Guard" (v1 title; later "ProbGuard"), 2025. Abstract.
  https://arxiv.org/abs/2508.00500
- Wang et al., "Self-Consistency Improves Chain of Thought Reasoning in Language Models", 2022.
  Abstract. https://arxiv.org/abs/2203.11171
- Wikipedia, "Adjacency matrix", section Matrix powers. https://en.wikipedia.org/wiki/Adjacency_matrix
- Wikipedia, "Petri net" (reachability complexity, boundedness). https://en.wikipedia.org/wiki/Petri_net
- Xavier et al., "Agentproof: Static Verification of Agent Workflow Graphs", 2026. Abstract.
  https://arxiv.org/abs/2603.20356
- Yao, Shinn, Razavi, Narasimhan, "τ-bench", 2024. Abstract (pass^k named, not defined there).
  https://arxiv.org/abs/2406.12045

Could not open, so not cited: Peled CAV 1993 and Godefroid LNCS 1032 (Springer 503/404), Ip and Dill
1996, van der Aalst 1997 (Springer 503), Paige and Tarjan 1987 (403); its bound is cited through
Katoen et al. Dziri et al. 2023 (arXiv 2305.18654) was opened but its abstract does not state
compounding, so it is not used.

## Not verified

- No count above was computed on a real protocol; `canon simulate` does not exist yet.
- Whether the walk graph of a protocol with order forms stays a DAG (`canon simulate` refuses them,
  `story/canon-simulate.md:73`).
- The claimed `3^k` versus `2k + 1` for ordered stages is a hand count on an idealised protocol.
