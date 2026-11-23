# Loom as the workflow SDK, and how a workflow declares an agent node

Question (the operator, 2026-11-20): can Loom be the SDK in which people build condition- and
evidence-driven workflows on Canon, programmatically (Rust core, later JavaScript and Python), and
how does a workflow say where its non-deterministic part (an agent or LLM node) runs?

Read on 2026-11-20: Loom at `abdb0c73` (0.53.0): `AGENTS.md` (§ Boundary, Rules, Commission,
Governor, Agent spec, Session executor, Workflow definitions), `crates/loom-sdk/{Cargo.toml,
src/lib.rs, src/runtime.rs, examples/governed_loop.rs}`, `ess/domains/workflow.yaml`,
`crates/loom-executor/src/{workflow.rs, selection.rs}` (parts), `crates/loom-commission/src/ports/
{executor,effect,evidence,connector}.rs`, `crates/loom-governor/src/lib.rs` (parts),
`crates/loom-session-executor/src/lib.rs:1-60`, `crates/loom-cli/src/lib.rs:222-242`,
`examples/agents/operator.yaml`, `website/docs/reference/sdk-stability.md`, the story
`canon-governor.md`. Canon at `8f1f452` (0.9.0): `crates/canon/src/{lib.rs, model/mod.rs,
model/properties.rs, model/ids.rs, check/mod.rs:245-304, validate/mod.rs:498-512}`,
`website/docs/reference/protocol.md:1-200`, `CHANGELOG.md`. Atlas ADRs 0072, 0073, 0074, 0082, 0083,
0090, 0096. SDC: `docs/{boundary,driver,canon-requests,phases}.md`, `docs/intake.md:1-24`,
`protocol/sdc-spec.protocol.yaml`, `driver/examples/sdc/bindings.json`. ADR 0089 was not read.
Nothing was built. The only program run was the installed `b10x-loom --help`. Statements marked
*inferred* were not observed.

## Verdict

Loom is the right home for the SDK, and its runtime half largely exists. It is not yet the SDK
the operator describes, for four reasons:

1. **Rust only, and embed only.** `b10x-loom-sdk` is "one crate an application depends on to embed
   a governed agent" (`loom-sdk/src/lib.rs:3`). It is `publish = false` and declares no
   `crate-type` (`loom-sdk/Cargo.toml:8-11`), and is consumed by git tag
   (`sdk-stability.md:22-27`). A search of Loom and Canon (`Cargo.toml` and `*.rs`) for `napi`,
   `pyo3`, `wasm-bindgen`, `uniffi`, `crate-type` and `cdylib` found nothing. The only
   `wasm-bindgen` in `loom/Cargo.lock` is a transitive dependency of a time crate (lock
   lines 1118-1126).
2. **Protocols are written as YAML.** A host registers a protocol as YAML text
   (`CanonGovernor::with_protocol_yaml`, `loom-governor/src/lib.rs:823-829`) or as a
   `b10x_canon::model::Protocol` value (`:836-840`). That struct has public fields and
   `Declarations::new` is public (`canon/.../model/mod.rs:70-98, 293-296`), so a Rust program can
   build one in memory. But the model derives only `Deserialize` (`model/mod.rs:68-69`), so
   nothing can write it out as `protocol/1`. No builder exists, and the docs of
   `with_protocol_yaml` say YAML keeps "embedders independent of Canon crate identity"
   (`lib.rs:821-822`).
3. **Loom's "workflow" is a sequence, not an evidence-driven graph.** A `loom.workflow`
   definition is "an ordered list of steps" (`ess/domains/workflow.yaml:10-15`). The executor
   takes "the first step after the last performed one whose `when` holds" and "never skips to a
   later step on its own" (`loom-executor/src/workflow.rs:14-24`). The evidence-driven mode is the
   frontier loop (`run_until_blocked`, `loom-sdk/src/runtime.rs:94-110`) with a selector. Its
   production selector is an LLM; the deterministic `FirstAdmissibleSelector` is "used only for
   tests/examples" (`loom-executor/src/selection.rs:139-142`).
4. **The agent node is not data tied to a protocol action.** A run has one executor
   (`runtime.rs:30-37`). A workflow's judgement steps all go through one host factory
   (`WorkflowExecutor::with_executor`, `workflow.rs:938-954`), and the factory tells steps apart
   only by the step name it receives (`Judgement`, `workflow.rs:765-776`). Turning a result into
   evidence is host code (`EvidenceAdapter::interpret`, `ports/evidence.rs:49-52`). Nothing
   declares "this action runs on an agent, under this budget and sandbox, and these programs check
   what it wrote".

The architecture already puts this layer in Loom: L3 governed runtime with loom-sdk
(ADR 0096), "Applications build on loom-sdk" (ADR 0096), and Loom as the one runtime
repository with "an SDK facade" (ADR 0090). So build the missing pieces in Loom (and two in
Canon), not in a new repository.

## What exists and what is missing

| Wanted SDK part | State | Where |
|---|---|---|
| Register a protocol | exists (YAML or Canon struct) | `loom-governor/src/lib.rs:823-872`; catalog `ProtocolCatalog::add_yaml` (`loom-protocols/src/lib.rs:183`, from grep) |
| Build a protocol in code and emit `protocol/1` | **missing** | Canon model is `Deserialize` only (`canon/.../model/mod.rs:68-69`); ids are `Serialize` (`model/ids.rs:36`), the documents are not |
| Open a case with inputs and independence | exists | `AGENTS.md:486-500`; `loom-governor/src/lib.rs:31-42, 283` |
| Evaluate a snapshot (pure) | exists | `loom_governor::evaluate`, `b10x-loom evaluate` (`AGENTS.md:466-475`); Canon library is IO-free (`canon/crates/canon/src/lib.rs:1-6`) |
| Run loop: frontier, revalidation, authority, suspension, resume, budgets, stop policy, events | exists | `AGENTS.md:310-423`; `loom-sdk/src/runtime.rs:94-152` |
| Effects (action performed by a trusted adapter) | exists | `EffectPort` (`ports/effect.rs:41-77`); action → Connector operation bindings (`ports/connector.rs:5-9`) |
| Evidence from observations, producer set by the trusted caller | exists as host code | `EvidenceAdapter`, `submit_evidence` (`ports/evidence.rs:9-16, 49-52, 102-130`) |
| Declarative result table (exit code or report field → evidence) | **missing** | host writes an `EvidenceAdapter`; the SDC driver has one as data (`bindings.json:10-23`) |
| Agent session that proposes an artifact and never writes it | exists | `SessionExecutor` (`loom-session-executor/src/lib.rs:1-5, 22-30`; `AGENTS.md:1376-1385`) |
| Agent's model, budget, tools, egress, confinement as data | exists, per agent | `loom.agent/1` `session:` (`examples/agents/operator.yaml:13-23`; `AGENTS.md:705-715`) |
| Bind an agent (or a program) to a protocol action | **missing** | one executor per run (`runtime.rs:30-37`); one judgement factory (`workflow.rs:938-954`) |
| Deterministic, evidence-driven selector (useful actions first) | **missing** (test-only exists) | `selection.rs:139-142`; policy in `driver.md:89-98`; usefulness is a Canon request (`canon-requests.md:18-22`) |
| Child cases (`case_open`, `case_outcome`) | **missing**, and blocked by the Canon pin | Loom pins Canon `0.8.0` (`loom-governor/Cargo.toml:15`; `AGENTS.md:502-505`); `case_open`, `case_outcome` and `children` arrived in Canon 0.9.0 (`canon/CHANGELOG.md:3, 12, 27, 32`); the governor story defers child cases to "later stories" (`canon-governor.md:86-88`); no Loom source names `case_open` or `case_outcome` (grep) |
| A command that runs a workflow or binding file | **missing** | source CLI: `run`, `protocols`, `evaluate`, `plugin`, `agent`, `approve`, `resume` (`loom-cli/src/lib.rs:222-242`); the installed `b10x-loom` is 0.4.0 with `run` and `protocols` only (`--help`) |
| JavaScript / Python bindings | **missing** | grep above; Loom's surface is generic over borrowed ports (`Runtime<'p, G, E, A, F, R, C>`, `runtime.rs:30`), which does not cross a binding as is (*inferred*) |

Order, by value per cost (*inferred*):

1. Repin Loom to Canon ≥ 0.9.0 (together with the engineering-protocols tag, `AGENTS.md:502-505`).
   Until then Loom cannot load an SDC protocol that has `case_open`.
2. Action bindings as data (`loom.bindings/1`, below): program, connector or agent per action,
   with result tables. This is the SDC driver's `bindings.json` moved into Loom. It also replaces
   the host's hand-written `EvidenceAdapter` for the common case.
3. A deterministic selector: Canon usefulness, then reads before writes, then identifier order
   (`driver.md:89-98`). It becomes the default for binding-driven runs.
4. Child-case runtime (`boundary.md` top change 1).
5. Canon: `Serialize` on the `protocol/1` model plus a builder that emits it. It belongs in Canon
   because the model is Canon's; Loom re-exports it.
6. Bindings. Bind at the document boundary: `protocol/1`, `canon-case/1`, `canon-evidence/1`,
   `canon-decision/1` and the bindings file in, decisions and run events out, with effects and
   evidence interpretation as host callbacks. Canon's pure `evaluate`/`check` is the first and
   easiest to bind. A new npm or PyPI toolchain in a foundation repository is the operator's
   decision (Atlas `AGENTS.md` § Language) (*inferred* scope).

## Where the agent node lives today

Loom has three executor forms, all on the proposal side. None of them performs an effect (ADR
0082):

- **Chooser** (`ModelSelector`): an LLM picks an admissible action and its arguments (ADR
  0073). This is a decision, not authored work.
- **Judgement step** (`loom.workflow.JudgementStep`, `workflow.yaml:58-70`): a model or the host's
  executor proposes one action among the step's actions, with instructions and typed inputs
  ("introduced as data and never as instructions", `AGENTS.md:1563-1569`).
- **Session executor**: a Claude Code, Codex or b10x-harness session works in a scratch copy, and
  "the change returns as a `ProposedAction` whose arguments are `{files}` only"; an effect applies
  it after Commission revalidates it (`AGENTS.md:1376-1381`). It "applies no budget, stop policy
  or egress" itself (`AGENTS.md:1384-1385`). Those come from the run and the agent spec.

The session executor is already the right shape for an agent node: the agent authors, a
deterministic effect applies, and its summary and transcript digest are "never evidence"
(`loom-session-executor/src/lib.rs:24-30`). Loom's rules already say "A trace is not evidence …
never from what a model says happened" (`AGENTS.md:89-90`). The SDC driver reaches the same rule
by binding the LLM as a tool with `revises` and no `produces` (`driver.md:157-166`).

What Canon has today:

- `effect` is a free identifier ("Names the class of effect an action has", `protocol.md:377`).
  `canon validate` only collects it as a name (`validate/mod.rs:505-507`), and the checker gives
  it no meaning: `read` and `write` are conventions. `effect: judgement` would be legal today, and
  Canon would prove nothing from it.
- `may_produce` lists kind ids only (`EvidenceProduction { evidence }`, `model/mod.rs:248-251`).
  It cannot say who decides a record's result.
- Usable as is: `independent_on` on an evidence match plus the case's `independence` map
  (`protocol.md:181-186`); the `authority-bypass` analysis (`check/mod.rs:269-285`), which is the
  pattern for the new check below; and `canon-properties/1` `independent_of: claim`
  (`properties.rs:1-17`). That last one can prove that an outcome does not depend on a claim at
  all. It cannot prove "depends on it only through an independent match", which is what a review
  needs.

## Design: the agent node

Split by ADR 0083, which puts capabilities in Canon and runtime binding in Commission (now Loom),
with "no tool names above the connection" (ADR 0083):

- **Canon protocol (semantic, provable).** Canon gets *that* an action's work product is
  non-deterministic, *which* artifacts it may move, and *who decides* each record's result. These
  are classes, not tools or models.
- **Loom binding (runtime, trusted).** Loom gets *which* agent runs it: harness kind and model
  role, inputs, budget, sandbox, the principal it acts as, and the programs and result tables of
  the checking actions.

### Canon: three optional fields

| Field | On | Values | Meaning |
|---|---|---|---|
| `author` | `Action` | `program` (default) \| `judgement` | `judgement`: the action's arguments and the artifact content it writes come from a non-deterministic executor |
| `revises` | `Action` | list of `ArtifactId` | the artifacts the action may move to a new revision; a runtime moving any other is refused |
| `decided_by` | `EvidenceProduction` | `program` \| `judgement`; default: the action's `author` | who decides the record's result. `program` on a judgement action is only for facts the runtime reads from the deterministic effect report (provenance, such as `spec_source: authored`), never from the agent's output |

Defaults keep every existing protocol valid (*inferred*: no current protocol changes meaning).

What `canon validate` / `canon check` could then prove (*inferred* designs; the last row follows
`authority-bypass`, `check/mod.rs:269-285`):

| Finding | Rule | Kind of check |
|---|---|---|
| `unchecked-judgement` | An outcome that reads evidence about an artifact some `judgement` action `revises` must positively read at least one `decided_by: program` match about that artifact. Revision binding already sets aside every record about an earlier revision (`driver.md:163-164`), so after the agent moves the artifact no outcome is legitimate until a program has observed the new revision. | static, over predicates: not affected by the 65,536-state bound |
| `self-judged-evidence` | Every evidence match on a kind that is `decided_by: judgement` must carry `independent_on`. An LLM's verdict counts only from a producer the trusted context lists as independent. | static |
| `judgement-bypass` | An outcome legitimate in a state where taking away one positively read judgement-decided dimension blocks it, and that dimension's match demands no independence. | state search, as `authority-bypass`; subject to the bound |
| `revises` checks | Each id in `revises` is a declared artifact. A `judgement` action with neither `revises` nor a production does nothing. | validate |

The headline property is then checkable per protocol: **no outcome is legitimate on evidence an
agent produced, or on an artifact an agent wrote, without a deterministic check of the current
revision or an independent producer.** Canon cannot prove that the runtime honours the binding:
that the program is deterministic, or that the independent principal is really another one. That
stays Loom's trusted context, as independence already is (`loom-governor/src/lib.rs:31-37`).

### Loom: what a binding must declare

| Item | Declared where | Existing piece |
|---|---|---|
| Executor class and agent (harness kind, model role, brief) | binding → `loom.agent/1` document | `session.kind`, `model.roles` (`operator.yaml:8-14`) |
| Principal the agent acts as | binding | `submit_evidence` takes the producer from the trusted caller (`ports/evidence.rs:102-110`) |
| Allowed inputs | binding: files copied into the scratch copy, plus earlier outputs as a data block | session executor named files (`AGENTS.md:1376-1380`); judgement `inputs` (`workflow.yaml:58-70`) |
| Output artifact, path, revision rule | binding; artifact must be in Canon `revises` | proposal `{files}` (`loom-session-executor/src/lib.rs:24-26`); revision digest choice (`driver.md:267-268`) |
| Budget | binding → agent `session.budget`, run budget | `operator.yaml:15-16`; Commission run budget (`AGENTS.md:353-369`) |
| Sandbox | binding → agent `session.confinement`, `tools.allow/deny`, `egress` | `operator.yaml:17-23` |
| Deterministic checks | **protocol** (the program-decided matches about the artifact, proven by `unchecked-judgement`); binding gives each check's program and result table | driver `results` tables (`bindings.json:10-23`) |
| Who must be independent | **protocol** `independent_on`; **case** independence map, written by Loom at open | `CanonGovernor::open_with_independence` (`lib.rs:35-37`) |

Checks Loom runs when it loads a binding (*inferred*, cheap): a `judgement` action binds only to an
agent or model executor, and a `program` action only to a program or Connector; the output artifact
is in `revises`; result-table kinds are in `may_produce` (`boundary.md` row 16); the case's
independence map never lists a judgement action's own principal as independent on a dimension
that a match about the artifact it revises demands.

### Example: SDC `spec.draft`

The driver binds `spec.draft` (`bindings.json:2-9`). `sdc-spec.protocol.yaml` calls the same
action `spec.author` (lines 139-154) and has it produce `spec_source`. Protocol side:

```yaml
independence_dimensions:
  different_principal:
    description: the producer is another principal than the one who wrote the specification
actions:
  spec.draft:
    description: an agent writes the ESS specification from the requirements list
    author: judgement
    effect: write
    revises: [spec]
    precondition: {claim: roadmap.approved}
    may_produce:
      - {evidence: spec_source, decided_by: program}
    arguments:
      type: object
      properties: {domain: {type: string, minLength: 1}}
      required: [domain]
      additionalProperties: false
  spec.validate:
    effect: read
    precondition: {claim: spec.sourced}
    may_produce:
      - {evidence: spec_validation}
  spec.review:
    author: judgement
    effect: read
    precondition: {claim: spec.complete}
    may_produce:
      - {evidence: spec_review}
claims:
  spec.approved:
    true_when:
      evidence: {kind: spec_review, result: approved, subject: spec, independent_on: [different_principal]}
```

With this protocol, `canon check` proves that `passed` needs program evidence (`spec_validation`,
`spec_compile`, `open_markers`, `spec_synthesis`, `scope_trace`) at the spec revision the agent
wrote. It also proves that `spec_review`, which is judgement-decided, counts only through
`independent_on`. Removing `independent_on` from `spec.approved` would raise
`self-judged-evidence`.

Loom side (proposed `loom.bindings/1`; the registration name form is *inferred* from
`AGENTS.md:59-63`):

```yaml
format: loom.bindings/1
protocol: sdc-spec@5
actions:
  spec.draft:
    executor:
      kind: agent
      agent: agents/spec-author.yaml
      principal: spec-author
    inputs:
      files: [requirements.md, spec]
    output:
      artifact: spec
      path: spec
      revision: ess-spec-digest
    budget: {max_tokens: 400000, max_budget_micro_usd: 2000000, max_runs: 2}
    sandbox:
      confinement: substrate
      tools: {allow: [Read, Write, Edit, Grep], deny: [Bash, WebFetch], decisions: ask}
      egress: []
    evidence:
      - {kind: spec_source, from: effect, result: authored}
  spec.validate:
    executor:
      kind: program
      argv: [ess, specify, validate, --path, spec, --strict-requires]
      timeout_seconds: 120
    evidence:
      - {kind: spec_validation, subject: spec, producer: ess, results: {"0": valid, "*": invalid}}
  spec.review:
    executor:
      kind: agent
      agent: agents/spec-reviewer.yaml
      principal: spec-reviewer
    inputs:
      files: [requirements.md, spec]
    evidence:
      - {kind: spec_review, subject: spec, producer: spec-reviewer, from: verdict, results: {approved: approved, rejected: rejected}}
case:
  independence:
    spec-reviewer: [different_principal]
```

`from: effect` means that Loom writes the record from the deterministic effect's report (it
applied a proposal of `spec.draft`), never from the session's text. `from: verdict` means the
reviewer's structured verdict. It is judgement-decided, so Canon admits it only through
`independent_on`, and the case lists only `spec-reviewer`, never `spec-author`.

## Open

- Whether `author` should be a closed enum in Canon or reuse `effect` with a declared meaning. A
  separate field keeps `effect` free as today (*inferred* preference).
- Whether provenance records (`decided_by: program` on a judgement action) are worth their
  exception, or whether `spec_source` should come from a separate program action. The separate
  action costs a state dimension (`boundary.md` § 2 pattern).
- Where `loom.bindings/1` sits beside `loom.workflow` and `loom.agent/1`. Proposal: bindings are
  per protocol action and evidence-driven; workflows stay for scripted sequences.
- ADR 0089 (governor component) was not read; nothing here depends on it (*inferred*).
