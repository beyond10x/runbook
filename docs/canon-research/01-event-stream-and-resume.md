# Canon research 01: a run as an event stream, and resume after a crash

The operator, 2026-11-20: a protocol run is a stream of events and state snapshots through time; a crashed
run continues where it left off; the run is one path through the tree of possible futures. A
Canon (kernel) topic. Research only, from source; nothing was built or run.

## 1. Canon's state today

- Evaluation is a pure function of protocol IR, case snapshot, evidence set, authority decisions
  and an explicit instant (`canon/AGENTS.md:22-26`; `canon/docs/design/canon-protocol-calculus-design.md:681-707`),
  plus explicit decisions (`canon/website/docs/concepts/evaluation.md:24-26`).
- Canon has no run, no event and no history. It is "not a workflow engine … an event store"
  (`design.md:1462-1470`). The design leaves history to the governor (`design.md:1024-1034`) and
  asks only that every evaluation records what it was computed from (`design.md:1597-1611`).
- The case snapshot is written by the runtime, not derived by Canon: current artifact revisions, case
  revision, termination, inputs, independence (`canon/website/docs/reference/documents.md:18-35`).
  So a run is not today a pure fold of evidence. The state is the tuple (snapshot, evidence set,
  authority, decisions), and a runtime folds its own log into it (sdc: `sdc/driver/src/main.rs:158-177`).
- Evaluation reads a set, not a sequence. Input order does not matter (`evaluation.md:51-52`). Order
  comes only from `observed_at` (`canon/crates/canon/src/eval/order.rs:16-25`). Two notes:
  - Adding one whole-second record coarsens the resolution of every order form
    (`order.rs:16-24`), and `since` reads the latest record (`order.rs:3-7`). A decision cannot be
    updated incrementally. It must be recomputed from the whole set, which a pure function allows.
  - The tree of futures exists in abstract form: `canon check` enumerates the protocol's finite
    state space (`canon/crates/canon/src/check/mod.rs:1-13`). Those are states, not ordered paths
    (*inferred*).
- What would make a resume wrong:
  - **Clock.** Freshness depends on the instant (`canon/crates/canon/src/eval/freshness.rs:6-10`).
    A replay that takes a new instant changes the decision. sdc logs `at` per event
    (`main.rs:149`) and replays with the logged one (`main.rs:666-668`).
  - **One instant per run.** sdc gives every record of one `run` the same `observed_at`
    (`main.rs:632-635`, `main.rs:544`), and equal ticks are not ordered (`order.rs:19-20`). Order
    forms between records of one run are then `unknown` (*inferred*, not run).
  - **Excluded evidence.** Exclusions are derived, never stored (`documents.md:87`), so a replay
    recomputes them. They depend on the snapshot's revision, and that is where the trouble is. sdc
    writes the revision bump (`main.rs:530`) before the evidence (`main.rs:553`) and before
    `action_finished` (`main.rs:566`). A crash in between leaves a moved revision, no evidence, and
    an exit code that is lost.
  - **Writes cut short by a crash.** sdc's `write_json` and `event` are plain `fs::write`
    (`main.rs:88-92`, `main.rs:151`), and `event` rewrites the whole log each time. A crash
    mid-write can truncate `events.jsonl` (*inferred*). The reader drops unparsable lines without a
    word and never checks `prev` (`main.rs:140-143`).
  - **Duplicate evidence.** Canon refuses any evidence id given twice (`canon/crates/canon/src/eval/evidence.rs:90-97`).
    A runtime that delivers at least once and keeps the id stable stops every later evaluation.
  - **Started, result unknown.** Canon has no "in flight". An action is admissible, approval-required
    or blocked (`documents.md:76`). Effect class is an uninterpreted identifier
    (`canon/crates/canon/src/model/ids.rs:174-175`), so Canon cannot say whether repeating it is safe.
  - **Digest.** `computed_from` lists evidence ids only, not their content
    (`canon/crates/canon/src/explain/mod.rs:313-314`), and the protocol only by id and revision
    (`explain/mod.rs:306-307`). It is an explanation, not a digest. Decision bytes include the Canon
    version (`explain/mod.rs:310`), so they are byte-identical only under one Canon version.
  - **Authority.** It carries no principal and no revision (`documents.md:118-121`). A grant
    survives a revision move (`sdc/docs/driver.md:175`; this file was being edited by another session
    while this research ran, so its line numbers may move).

## 2. What each runtime does on a crash

- **Loom**
  - Durable state is current-state documents, not an event log: the case file and
    `observations.json` (`loom/crates/loom-governor/src/file_store.rs:4-11`), a suspension file
    (`file_suspension.rs:1-16`) and a workflow run record (`file_run_store.rs:1-19`). Each is
    written to a temp file, synced and renamed, under a directory lock (`file_store.rs:7-11`,
    `file_store.rs:29-32`).
  - Resume reads the record and continues after the last performed step, with no performed step
    proposed again (`loom/crates/loom-executor/src/workflow.rs:70-72`).
  - In flight: "A request proposed and not recorded performed is proposed again after a restart,
    so an effect must tolerate one repeat" (`workflow.rs:76-77`). That is at-least-once.
  - A port answer of `Pending` is invoked again under its own id and revalidated
    (`loom/crates/loom-commission/src/runtime.rs:151-162`). The loop itself never retries
    (`runtime.rs:110-111`). The frontier id hashes the decision bytes, kept stable across restarts
    (`file_store.rs:37-38`). `loom-sdk/src/runtime.rs` is a 153-line wrapper (`runtime.rs:1-5`).
- **takt**
  - `events.jsonl` is append-only and never rewritten (`takt/crates/takt/src/events.rs:1-8`). A
    line cut short is skipped and marked by a `Skipped` event (`events.rs:189-191`).
  - The state is the case files. `resume` re-reads them, and a moved `HEAD` sends every claim back
    to unknown and runs the validators again (`takt/crates/takt/src/case.rs:2707-2723`). That is a
    re-run, not a replay.
  - A crashed holder leaves `run.lock`, which must be removed by hand (`case.rs:417-418`).
  - `takt replay` re-runs one judgement step in a scratch tree, for verification, not recovery
    (`takt/crates/takt/src/replay.rs:1-17`).
- **eventlog**
  - State is a fold over an append-only log (`eventlog/crates/eventlog-core/src/lib.rs:3-8`).
  - Optimistic concurrency and idempotency keys: a retried command is answered, not decided again
    (`lib.rs:819-821`, `lib.rs:842-855`).
  - A snapshot is a cache, never the record. One that does not read is dropped and the fold starts
    from zero (`lib.rs:715-718`).
  - The file provider keeps a digest chain and rolls back an unpublished append intent
    (`eventlog/docs/design/file-provider.md:160-178`).
- **the driver**
  - Hash-chained `events.jsonl` (`main.rs:145-153`). An `evaluated` event carries a state digest and
    a decision digest (`main.rs:199-200`). The state digest is sha256 over IR, case, authority,
    decisions and the evidence tree (`main.rs:131-138`).
  - `replay` only re-evaluates the final store at the last logged instant and compares the two
    digests (`main.rs:662-676`). It folds no events, and the log is a record, not the source.
  - No resume command. An `action_started` without `action_finished` is not counted as a run
    (`main.rs:279`, `main.rs:287`), so the next `run` executes it again (*inferred*).
  - Each action is bounded by `max_runs`, default 8 (`main.rs:288`).
- **Atlas.** "Before every effect, the action is revalidated against current revision and
  authority" (Atlas ADR 0072; also ADR 0073).
  "Raw traces never automatically affect case truth" (Atlas ADR 0074). The
  governor is stateless per evaluation (Atlas ADR 0089). Durable
  suspend/resume is a roadmap item (Atlas's governed-autonomy roadmap). A
  branchable event tree already exists for planning stores (Atlas ADR 0063).

## 3. Kernel and runtime

- **Kernel (Canon, pure):**
  - the event vocabulary and its schema;
  - the fold from an event stream to run state;
  - the snapshot format and its digest;
  - the open-attempt rule, including the `in-doubt` action status;
  - refusal codes and the determinism claim.
  These are functions of bytes. They need no clock, IO or storage (`AGENTS.md:22-24`), and Canon
  still owns no store (`design.md:1469`).
- **Runtime (Loom, takt, sdc):**
  - atomic, synced appends and the hash chain written to disk;
  - cutting a torn tail;
  - locks;
  - reading the clock once per event;
  - executing actions and passing idempotency keys to external systems;
  - writing snapshots as caches;
  - the reconciliation observation that settles an in-doubt attempt.
- **What Canon's evaluation already gives a snapshot:**
  - deterministic bytes (`AGENTS.md:26`);
  - an explanation with `computed_from` (`explain/mod.rs:8-17`).
- **What it lacks:**
  - a content digest of the inputs;
  - an IR digest;
  - a stream position and chain head;
  - open attempts.

---

## Draft story body

**Title:** A run is a fold of its event stream, and resumes at its last event

**Outcome.** Canon defines the events a governed run emits and the pure fold from a stream of them
to the run's state. Folding the same stream gives the same bytes, a snapshot is a checked shortcut
of the fold, and an action started with no recorded result is reported `in-doubt` rather than
silently re-admitted. A runtime that crashed reopens its stream, folds it and continues from the
exact position, with every decision it took before reproducible at the instant it was taken.

**Event vocabulary** (`canon-run-event/1`, one JSON object per line). Every event has `seq`,
`prev` (sha256 of the previous line, `null` for the first), `at` (an `Instant`, written by the
runtime) and `event`:

- `case_opened`: case id, protocol id and IR digest, initial artifact revisions, inputs, independence. Only at seq 0.
- `action_admitted`: action, arguments digest, case revision, digest of the decision that admitted it.
- `action_started`: attempt id, action, arguments, case revision. The attempt id is unique in the stream.
- `action_finished`: attempt id, `performed | refused | failed`, evidence ids it recorded.
- `action_abandoned`: attempt id, principal, reason. Settles an in-doubt attempt without a result.
- `evidence_recorded`: one full `canon-evidence/1` record, optionally with the `attempt` that produced it.
- `authority_decided`: capability, `granted | denied`, principal, case revision.
- `decision_recorded`: one `canon-decisions/1` entry.
- `revision_changed`: artifact, from, to. The case revision advances by one.
- `outcome_taken`: outcome. It sets `termination`; no event may follow.
- `evaluated` (optional): the snapshot digest and the decision digest. It is derived from the
  events, and the fold checks it rather than reading it.

**Snapshot** (`canon-run-state/1`):

- the stream position `seq` and the chain head;
- the folded `canon-case/1`, the evidence set, the authority and the decisions;
- the open attempts;
- `inputs_digest`: sha256 over the canonical bytes of IR, case, every evidence record's content,
  authority and decisions.

A snapshot is a cache. It is used only when its chain head equals the stream's at its `seq`;
otherwise the fold starts from seq 0.

**Resume rule.**

1. The runtime removes a torn final line, recording that it did so, and verifies the chain.
2. Canon folds from the matching snapshot, or from 0.
3. Every `action_started` with no `action_finished` or `action_abandoned` is an open attempt.
   Its action evaluates `in-doubt` and is not admissible until one of these holds:
   - an `evidence_recorded` naming the attempt (an observation of whether the effect happened);
   - an `action_abandoned` for the attempt;
   - the protocol declares the action `repeatable`. Canon gives the effect class no meaning today
     (`ids.rs:174`); an explicit flag is clearer than giving the class one.
4. Evidence an attempt recorded before the crash counts like any other evidence.
5. The next evaluation uses an instant the runtime passes in. Positions before the crash are
   evaluated at their own `at`.

**Determinism claim.** For one Canon semantics version, the same stream bytes give byte-identical
`canon-run-state/1` and byte-identical `canon-decision/1` at every position. The fold depends on
order only for `revision_changed`, `outcome_taken` and attempt pairing. Evidence, authority and
decisions enter as sets, as evaluation reads them today (`evaluation.md:51-52`).

**Acceptance:**

1. `canon fold --events run.jsonl` run twice writes byte-identical `canon-run-state/1`
   (conformance `CANON-RUN-001`).
2. Folding events 0..n into a snapshot and then the rest from it gives the same bytes as folding
   0..end (`CANON-RUN-002`).
3. A stream with `action_started` and no result: `canon evaluate` on the folded state lists the
   action `in-doubt`, with a reason naming the attempt id, and not `admissible` (`CANON-RUN-003`).
4. Appending `evidence_recorded` naming that attempt, or `action_abandoned`, returns the action to
   the status it has without the attempt (`CANON-RUN-003`).
5. A stream whose `prev` breaks at line k is refused `broken-chain` naming k. An event after
   `outcome_taken` is refused `after-termination` (`CANON-RUN-004`).
6. Two streams that differ only in the order of `evidence_recorded` lines give identical decision
   bytes. Two that differ in the order of `revision_changed` lines give different case snapshots
   (`CANON-RUN-005`).
7. A second `evidence_recorded` with an id already folded is skipped when its content is identical
   and refused `conflicting-evidence` when it differs (`CANON-RUN-006`).
8. Changing one byte of an evidence record's content, its id unchanged, changes `inputs_digest`;
   today's `computed_from` does not change (`CANON-RUN-007`).
9. The decision at position n, recomputed from the stream alone, equals the decision digest the
   `evaluated` event at n logged. No clock is read (`CANON-RUN-008`).

**ESS first.** `ess/` declares:

- `canon-run-event/1` (the event union above);
- `canon-run-state/1` (the snapshot);
- the action status `in-doubt`;
- the protocol action flag `repeatable`;
- the refusal codes `broken-chain`, `after-termination` and `conflicting-evidence`.

The scenarios `conformance/scenarios/run-*.yaml` (`CANON-RUN-001`…`008`) are added in the same
first commit. Named red test: `crates/canon/tests/run_fold.rs::same_stream_same_bytes`, failing
because `canon fold` does not exist.

**Out of scope:**

- storage, durability, locks and the hash chain on disk (runtime; eventlog's file provider is the model);
- executing or reconciling an effect;
- choosing which admissible action comes next;
- branching streams. Forks are eventlog-tree's concern (ADR 0063).

## Not verified

- Nothing was built or run. The disk is nearly full.
- The sdc crash windows, the truncated-log risk and the "one instant per run makes order forms
  unknown" effect are read from code, not reproduced.
- Loom's commission runtime (3793 lines) was read only in its module docs (1-200).
- takt's `run.rs` behaviour for a judgement step interrupted mid-edit was not read.
- eventlog's recovery was read only in `file-provider.md:160-178`.
- ADR 0082 was only searched by keyword.
- Whether an evidence-order permutation test exists in Canon's suite was not confirmed. Only the
  doc claim (`evaluation.md:51-52`) and test file names were seen.
- `repeatable`, `in-doubt`, `canon fold` and every `CANON-RUN-*` id are proposals. None exists today.
