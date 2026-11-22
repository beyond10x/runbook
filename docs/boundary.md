# Where Canon ends: the SDC driver as a case study

Question (the operator, 2026-11-20): which of the things the driver does should move to something more
general, Canon or a generic layer, and is it right to keep Canon "just the kernel, the engine"
with everything specific on top.

Read on 2026-11-20: `sdc/driver/src/main.rs` (395 lines), `sdc/docs/{driver,intake,phases,
protocol-notes,canon-simulation-request}.md`, every file under `sdc/docs/limitations/` and
`sdc/scripts/limitations.sh`, `canon/AGENTS.md`, Canon's reference docs and design document at
canon `8f1f452` (0.9.0), Atlas ADRs 0066-0075 and the later ADRs that amend them (0077, 0081-0084,
0089, 0090, 0094, 0096, 0098, 0104), and the Loom and takt sources cited below. Nothing was built or
run. Statements marked *inferred* were not observed.

## Recommendation

The boundary:

1. Canon owns every rule whose answer changes truth, admissibility or legitimacy, and is a pure
   function of protocol, case, evidence, authority, decisions and instant. Pure is the test, not
   small.
2. By that test three things the driver or its users compute belong in Canon: the usefulness of an
   admissible action (Canon's own design lists "useful missing evidence" in the frontier), the
   binding of an authority grant to a case revision and a principal, and exhaustive checking of a
   protocol that the 65,536-state bound does not distort.
3. Canon stays without a store, a log, a clock, a process, a tool name or a selection policy. Its
   design says so (`canon/docs/design/canon-protocol-calculus-design.md:1460-1476`: "not a
   workflow engine; … an event store; … a CI runner").
4. A generic runtime layer is needed, and it already exists by decision: Loom's L3 governed runtime
   (ADR 0090, ADR 0096). Do not create a new repository or a new crate in Canon for it.
5. Most of the driver is that runtime: case store, revisions, event log, evaluation calls,
   selection, budgets, tool invocation, observation-to-evidence, authority and decision recording,
   termination. Loom has almost all of it; the driver is the fourth copy beside Loom, takt and
   `metaharness aep drive` (ADR 0096, ADR 0104).
6. Loom lacks one generic piece the SDC cannot run without: opening child cases and writing their
   outcomes back as `case_outcome` records. Build it there.
7. SDC-specific and stays on top: the eight protocols (they belong in engineering-protocols, ADR
   0094), the action-to-tool bindings, the exit-code-to-result tables, which digest is an
   artifact's revision, the independence lists and the report checks (spec digest, clean tree).
8. The bindings' `until` claim is protocol logic hidden in the runtime; move it into action
   preconditions so `canon check` sees it.
9. Of the eight limitations, four are the checker's state bound, two are composition semantics
   (parent-to-child evidence, invalidating a child's outcome), one is the case document, one is
   case-store hygiene. None needs engineering vocabulary in Canon.

Top three changes by value per cost (reasons in "Top three changes" below):

1. Loom: open child cases and feed their outcomes back as `case_outcome` evidence.
2. Canon: `canon-authority/1` entries carry `principal` and `case_revision`, like
   `canon-decisions/1`.
3. Canon: a usefulness projection in `canon-decision/1`, with the bindings' `until` moved into the
   SDC protocols' preconditions.

## 1. What the driver does, and where each belongs

Classes: **(a)** Canon kernel, pure and generic; **(b)** generic runtime on Canon, no engineering
words; **(c)** engineering/SDC-specific. "Loom has" cites the existing code; nothing in (b) needs a
new home.

| # | Responsibility | `main.rs` | Class | Home and reason |
|---|---|---|---|---|
| 1 | Command surface: init, eval, tools, step, run, approve, close | 27-67 | b | Loom CLI. Canon's CLI "must remain a thin shell over pure library semantics" (design.md:1358). |
| 2 | Case store layout: `ir.json`, `case.json`, `evidence/`, `authority.json`, `decisions.json`, `bindings.json`, `work/` | 314-339 | b | Loom has `FileCaseStore`, atomic writes and a directory lock (`loom-governor/src/file_store.rs:1-30`). Canon is not a database (design.md:1468). |
| 3 | Compile the protocol once at open and pin the IR digest | 318-323, 340 | b | Loom registers protocols per host (`file_store.rs` header: "Protocol definitions are not stored: the host registers the same protocols"). Compilation itself is Canon's (exists). |
| 4 | Artifact revision = content digest of what the tool left; case revision counter `cN` | 86-102, 273-287 | b, rule; c, digest | Loom has the counter and per-artifact revisions (`loom-governor/src/lib.rs:188-192`, `update_revision` 1021-1043, same rule: unchanged revision changes nothing). Which digest counts (tree vs ESS `spec_digest`) is SDC's (driver.md:267). |
| 5 | State digest over all Canon inputs | 104-112 | b | A runtime replay key. Canon's guarantee it rests on is already Canon's (AGENTS.md:22-26). |
| 6 | Hash-chained event log | 114-127 | b | Canon is not an event store (design.md:1469). Loom has an `EventSink` (`loom-commission/src/runtime.rs:580`); takt has its own `events.jsonl` (`takt/crates/takt/src/events.rs:1-10`). Whether Loom's sink is hash-chained: not checked. |
| 7 | Call `canon evaluate`, keep every decision | 129-148 | b | Loom's `CanonGovernor` (driver.md:69, `loom-governor/src/lib.rs:701`). |
| 8 | Position: open obligations, admissible, approval-required, legitimate outcomes | 150-153, 200-208 | a (exists) | A projection of `canon-decision/1`; Canon's planned `canon frontier` (design.md:1350, `website/docs/status/where-this-stands.md:32`). Low value to move. |
| 9 | Usefulness, pure part: an action is useful while a kind it may produce has no applicable record at the current revision | 162-176, 190-191 | **a** | Canon's design puts "useful missing evidence" in the frontier (design.md:282-291) and states the agent's question as "admissible and useful" (design.md:604) and the milestone as "a useful frontier" (design.md:1757). The driver re-derives applicability (subject revision, `excluded_evidence`) that Canon already computed: two implementations of one rule (*inferred*: they drift when Canon adds a reason). |
| 10 | Usefulness, history part: `max_runs`, "succeeded against this snapshot" | 177-186, 192-193 | b | Needs run history, which Canon must not read. Loom has step, run and per-action budgets and an idle bound (`runtime.rs:543-563`). |
| 11 | `until` claim on a binding | 187-189 | **c → protocol** | Progress logic outside the protocol: `canon check` cannot see it. Canon's claim test `is` (protocol.md:144) expresses "while not true" as a precondition (*inferred*: `any` of `is: false` / `is: unknown`). |
| 12 | Selection policy: reads before writes, then identifier order | 195-197, 219-221 | b | ADR 0073 (`ActionSelector` in Loom). Loom's `FirstAdmissibleSelector` is "only for tests/examples" (driver.md:69); this deterministic policy is a better default for it. |
| 13 | `--action` must be admissible at this step's evaluation | 216-218 | b | Revalidation before effect: ADR 0072, ADR 0082. Loom has it (`AdmittedRequest`, `loom-commission/src/ports/effect.rs:3-6`). |
| 14 | Verdicts: terminable, awaiting_authority, stuck; step budget | 222-232, 354-365 | b (run time); a (static) | Loom's `NoUsefulAction`, `AwaitingApproval`, `IdleBound`, step budget (`runtime.rs:543-563`). Finding stuck states before running is Canon's (`docs/canon-simulation-request.md:66-72`). |
| 15 | Argument check (top-level `required`, `additionalProperties`) | 240-254 | b calls, a owns | Canon says "the runner … validates them against the schema" (protocol.md:92). The dialect is Canon's and `canon validate` already holds schemas to it (`canon/crates/canon/src/validate/arguments.rs`). Recommend: Canon exports a pure `validate_arguments` so each runner does not reimplement a partial dialect (driver.md:259). |
| 16 | Binding sanity: `produces` ⊆ `may_produce` | 255-261 | b | Binding is runtime (ADR 0083). Better checked once at binding load than per step. |
| 17 | Run the bound tool: argv, cwd `work/`, env `SDC_ACTION`/`SDC_ARGS` | 262-267 | b | `EffectPort::invoke` (effect.rs:41-77), Connectors behind it (ADR 0082, ADR 0083). No timeout here; Loom has command timeouts (`loom-intake-slice/src/executor.rs:23-24`, `loom-session-executor/src/lib.rs:617`). |
| 18 | Keep tool stdout/stderr as a content-addressed blob, never read | 268-272 | b | This is a trace (ADR 0074). In Loom it is the observation that evidence must cite (`loom-commission/src/ports/evidence.rs:1-15`: a record "that names no observation" is refused). |
| 19 | Exit code → result → `canon-evidence/1` with `subject_revision`, `observed_at = --at`, `producer` | 288-303 | b, mechanism; c, tables | Loom's `EvidenceAdapter::interpret` (evidence.rs:49-51) and `submit_evidence` (116). The tables (`ess … validate` exit 0 → `valid`) are SDC (driver.md:190-209). ADR 0074 allows "an independent verifier may interpret trace data", so a deterministic program's exit code is fine; an LLM's is not. |
| 20 | `action_started` / `action_finished` events with state before | 264, 304-306 | b | Loom run events. |
| 21 | Tools for an external chooser (name, description, input schema) | 344-352 | b | ADR 0072, projected catalogue in Loom. |
| 22 | Record an authority decision; keep the principal in the log only | 366-376 | **a**, format; b, intake | `canon-authority/1` has only `capability` and `decision` (documents.md:116-121), while `canon-decisions/1` already has `principal` and `case_revision` (documents.md:140-148). A grant given at spec r1 still holds at r2 (driver.md:145-147). Whether a grant applies decides admissibility, so it is semantics. Takt already works around it in its runtime ("the case's authority over the revisions that decision was computed at", `takt/crates/takt/src/replay.rs:7-9`). |
| 23 | Terminate through an outcome; roll back if Canon refuses | 377-386 | a (exists), b | Legitimacy is Canon's (`illegitimate-termination`, driver.md:238-239); the write and rollback are the store's. |
| 24 | Independence lists per producer in the case | 332-335 | a (exists), c (data) | Canon semantics exist (CANON-INDEPENDENCE-001); Loom stores them (`lib.rs:195-196`); who is independent is SDC data. |
| 25 | No clock: every instant is `--at` | 5, 40-66 | a (exists), b | Canon rule (AGENTS.md:22-24). Loom has a `Clock` port (`runtime.rs:594-596`) a host can feed from `--at`. |
| 26 | Explicit outcome decisions (`canon-decisions/1`) | file only, 338 | b | Not implemented in the driver (driver.md:264); Loom has `CanonGovernor::decide` (`lib.rs:1052`). |
| 27 | Child cases: open on `case_open`, write `case_outcome` back | absent | **b** | Canon has the primitive and says "Canon opens nothing itself; whoever takes the action opens the child case" (protocol.md:93). No `case_outcome`/`child` handling found in `loom-governor`, `loom-commission` or `loom-sdk` (grep, 2026-11-20). The SDC stops `stuck` at `story.open` because of it (driver.md:249-251, 257-258). |
| 28 | Copy a record from another case (`roadmap.read`) | absent | b, with a gap in a | See limitation 4 below. |
| 29 | Store lock; tool timeout | absent | b | driver.md:265; Loom has both (rows 2 and 17). |
| 30 | Replay check | manual (driver.md:95-97) | b | Run `canon evaluate` on the logged inputs and compare digests; takt has `takt replay` (`replay.rs:1-4`). |
| 31 | Report checks: spec digest, clean tree, timestamp window, no-write for observations | absent | c, with b hooks | driver.md:177-182. The no-write check (tree digest before = after for `effect: read`) is generic (b); the others name ESS and git (c). |

Count: 3 rows move to Canon or need a Canon change (9, 15's validator, 22), 1 row moves into the
SDC protocols (11), about 22 are runtime that Loom has or must have, and the rest is SDC data.

## 2. The limitations, one by one

The outputs are in `sdc/docs/limitations/output.txt`, reproduced by `sdc/scripts/limitations.sh`.
The checker's state space is the product of independent dimensions, refused above `STATE_BOUND`
65,536 (`canon/crates/canon/src/check/mod.rs:249-251, 339`). `protocol-notes.md` still holds the
placeholder `LIMITATIONS` at line 106; this section is what it should say.

| # | File | What happens | Class | Why |
|---|---|---|---|---|
| 1 | `story-with-derived-verified-claim.protocol.yaml` | `state-space-bound`, 196,608 states (output.txt:2) | Canon checker | The named factors are evidence kinds (16 each with an upstream), capability 3, decision 2; the derived claim `change.verified` adds no factor. The SDC dropped `plan` and `design` to fit (compare `sdc/protocol/sdc-story.protocol.yaml`). The bound, not the language, shaped the protocol. |
| 2 | `monolith.protocol.yaml` | 2,654,208 states (output.txt:6) | SDC modelling, correctly solved | Splitting into child cases is what ADR 0081 intends anyway. It does expose a Canon gap: a parent is checked without its children's protocols (*inferred*: no compositional check exists; none is listed in `canon-simulation-request.md:12-20`). |
| 3 | `intake-with-abandon.protocol.yaml` | 131,072 states (output.txt:10) | Canon checker | 4·16·16·4·16 = 65,536 exactly, and the `explicitly_abandoned` decision doubles it. That decision is read only by outcome `abandoned` (lines 171-174), so it is independent of every other dimension. The workaround changed the meaning: intake can no longer be abandoned on its own; "abandoning the idea is the root case's decision" (`sdc-intake.protocol.yaml:6`). *Inferred*: checking independent dimensions separately (cone of influence) brings this to 65,536, within the bound (`check/mod.rs:561`, `size <= STATE_BOUND`). `sdc.spec` sits at exactly the bound (protocol-notes.md:157-158). |
| 4 | `epic-imported-evidence.protocol.yaml` | 8 `unproduced-evidence` findings for `roadmap_approval` (output.txt:14-22) | Canon language (small) + runtime | Evidence flows child → parent (`case_outcome`) but has no declared way parent → child. The SDC workaround is an action that "copies" the record (`scripts/roadmap-read-action.yaml`, `sdc-epic.protocol.yaml:103-111`), which only the runtime can honour and which nothing checks is a copy. A declared imported kind (or case input) would make it explicit; the copy itself is the runtime's. Usable today as modelled. |
| 5 | `story-with-four-gate-kinds.protocol.yaml` | 786,432 states (output.txt:26) | Canon checker; SDC workaround acceptable | Four gate kinds at ×4 each. The SDC collapsed them into one `ess_gate` kind run by one program (protocol-notes.md:164-166). This is also reasonable modelling, at the cost of Canon's explanation naming which of the four checks failed. Cone of influence would not help here (*inferred*: the gate feeds every action); this needs a smarter search or the bounded walk requested in `canon-simulation-request.md:76-81`. |
| 6 | `child-outcomes-invalidated.protocol.yaml` | `inert-invalidation` (output.txt:30) | Canon language | Canon counts a child's outcome whatever its subject revision: "a record about an earlier revision of its subject, an expired one and an invalidated one still count" (protocol.md:194-196). A spec revised after its stories ended `passed` therefore leaves `children.passed` true in `sdc.epic`. The SDC has no workaround in its protocols (*inferred* from reading `sdc-epic.protocol.yaml`; `gate.passed` is re-checked on the current spec, delivery against it is not). Needs a Canon decision: either the `children` form gains a "current subject revision" option, or a protocol must reopen children itself. |
| 7 | `missing-artifact/` | `missing-artifact` refusal (output.txt:34) | Runtime convention; optional Canon change | Every declared artifact needs a revision even before it exists. The driver uses a sentinel (`spec none → t2a…`, driver.md:247). Fine as a runtime convention; a Canon "absent" revision would make it explicit (*inferred*, low value). |
| 8 | `evidence-with-gitkeep/` | `.gitkeep` refused as not an evidence record (output.txt:38) | Runtime (case-store hygiene) | Canon is right to refuse unknown files in its input. The store should not put placeholder files there. No Canon change. |

Pattern: four of eight (1, 3, 5, and 2's reason) are the checker's bound, and in two of them (1, 3)
the bound changed what the protocol means. ADR 0094 records the same for
`self.improvement/1` (5,971,968 states). That is the strongest evidence that a Canon change pays.

## 3. Where I disagree with the framing

- "Keep Canon abstract and low level" is right for vocabulary and effects, and Canon's own rules
  already enforce it (AGENTS.md:14-18, 22-24). It is wrong if it means "keep Canon small": the
  question to ask of each rule is whether it is pure and generic, not whether it is low level.
  Usefulness and revision-bound authority are pure and generic; leaving them out pushes them into
  every runtime, where takt, Loom and this driver each solve them differently (rows 9, 22).
- "Everything else specific on top" skips the middle layer, and that layer is the real risk. Most
  of the driver is neither kernel nor engineering (section 1). The architecture already names its
  owner, Loom L3 (ADR 0090, ADR 0096); ADR 0096's own context lists takt carrying "its own case,
  frontier and protocol runtime beside Loom's" (ADR 0096). A sixth runtime in sdc repeats that.
- So I agree with driver.md:54-57 (target home Loom), with one change of order: the missing piece
  is in Loom (child cases), so the driver should not grow it first.
- I disagree with Canon's own statement that argument validation is only the runner's
  (protocol.md:92). The dialect is Canon's; a pure validator in the Canon library costs little and
  removes a partial reimplementation per runner (*inferred* cost).

## 4. Top three changes

Ranked by value per cost; each reason is cited.

1. **Loom: child-case runtime** (open the child on a `case_open` action, keep the parent-child
   link, write the child's terminal outcome as a `case_outcome` record in the parent).
   Value: it blocks the SDC end to end; the run stops `stuck` at `story.open` (driver.md:249-251)
   and six of eight SDC protocols open or are opened as children (protocol-notes.md:7-16). Cost:
   no Canon change; Canon's primitive and its checker support exist (protocol.md:93, 188-196;
   `check/mod.rs:225-238`), and Loom already has the store, governor and loop to hang it on. ADR
   0081 put the links in AEP; ADR 0089 and 0090 have since moved the governor and
   runtime into Loom, so Loom is the owner (*inferred*: no ADR re-homes the link store explicitly).
2. **Canon: authority bound to revision and principal.** Add `principal` and `case_revision` to
   `canon-authority/1` entries and apply a grant only at its revision, as `canon-decisions/1`
   already does (documents.md:140-148). Value: closes a safety hole (a grant survives a spec
   revision, driver.md:145-147) that phases.md:45-46 requires closed, and removes takt's workaround
   (`replay.rs:7-9`). Cost: one document format and one evaluation rule, with the pattern already in
   the codebase. ADR 0083 already plans to change the same structure (capability scopes), so
   both can land in one story (*inferred*).
3. **Canon: usefulness in the decision, and `until` into the protocols.** Add, per admissible
   action, whether some kind it may produce lacks an applicable record at the current revision
   (driver rule, main.rs:155-161), as the design's "useful missing evidence" (design.md:282-291).
   Move each binding's `until` into its action's precondition in the SDC protocols. Value: every
   runtime needs it (the first driver run looped on an admissible, useless action,
   driver.md:91-92), it removes the driver's second applicability check (main.rs:167-176), and it
   is the run-time half of "an action that is admissible yet can never change any claim"
   (`canon-simulation-request.md:71`). Cost: a pure function over data Canon already computes;
   the `until` move is a protocol edit.

Next, not in the top three because its cost is higher: a checker reduction (evaluate independent
dimensions separately, then the bounded or exhaustive walk of `canon-simulation-request.md`).
Its value is the highest of all (section 2 pattern), its cost is a change to the checker's core
(*inferred*).

## Not verified

- Whether Loom's `EventSink` is hash-chained, and whether Loom's runtime has a no-write check for
  read actions.
- Whether any Loom or Canon story already plans child-case runtime, usefulness or revision-bound
  authority (planning stores were not read).
- Atlas `docs/design/governed-autonomy/` was listed, not read; the ADRs that cite it were.
