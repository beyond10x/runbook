# Library entries of Loom and Canon for Runbook

What Runbook can call in process from Canon and Loom, and what is missing. Runbook is to link both
as cargo dependencies and start no `canon`, `b10x-loom` or `loom` process
(`epic:runbook-one-binary-and-daemon`, design decisions 1 and 2). Collected by
`story:library-entries-from-loom-and-canon`. Both neighbour sessions read their own code on
2026-11-22 and answered; this page copies their references and marks. Nothing here was built or run
by Runbook, and no `cargo check` of a crate that depends on them was run (see "Not done").

## States and commits

States of a row:

- `exists`: public, in a release tag or `main` of that repository, read there. No row has this state
  today.
- `on a branch`: public at the named commit of the named branch; not in a tag. Whether `main` has it
  was not checked unless the row says so.
- `requested`: not built. Agreed with the neighbour session on 2026-11-22 (marked "agreed"), or
  planned in a story of that repository (named in the row; its status as the session reported it),
  or asked of the session with no story id yet (marked "asked"; no row has this mark now).
- `not planned`: no story that is more than a draft and nothing agreed.
- `not checked`: the answers do not cover it.

Commits the rows were read at. Paths are under `crates/` of that repository.

| label | repository | ref | commit | note |
|---|---|---|---|---|
| C | Canon | `wave/2026-11-21` | `7d1a28a20` | read, not run; also on `origin/wave/2026-11-21` |
| H | Loom | plan tree `wave/2026-11-21-w7` | `50ddc47d` | in no tag and in no remote branch; the integration head of the same branch was `54204526` when Loom reported the parity path |
| B | Loom | `impl/tree-run-command` | `c8ca4ad6` (base `5751f164`) | unmerged; holds `loom-tree` |
| P | Loom | plan commit | `df62eaec` | holds the stories `concurrent-confined-runs`, `cancel-a-running-bound-program` and `per-run-program-path` (proposed); their `file:line` are copied from the stories and were not re-read in the code |
| main | Loom | `origin/main` | `b4b5de28` | has no `tree` command |
| 0.58.0 | Loom | tag | `1378f8fe` (2026-11-21, in `main`) | has neither `loom-tree` nor `loom-decision`; pins Canon `0.9.0` |

Answers: Canon answered on 2026-11-22; Loom answered on 2026-11-22 (six questions with `file:line`)
and agreed the items marked "agreed" below with the Runbook session after those answers.

## Canon

Canon is the kernel. `b10x-canon` and `b10x-canon-expr` are meant for embedders (`README.md:33`);
the `canon-cli` library is not. Canon has no written version rule for its public functions, and
`0.10.0` and the next release are not interchangeable. The line numbers below are those of C; the
same lines in tag `0.10.0` hold other code (`ir/mod.rs:294`, `validate/mod.rs:806`,
`eval/mod.rs:503` and `simulate/mod.rs:1000` are not the function heads there), and the public items
at `0.10.0` were not checked.

| id | capability Runbook needs | crate and public item | read at (paths under `crates/canon/src/`) | state |
|---|---|---|---|---|
| C1 | parse a protocol | `b10x-canon`: `model::parse(&str) -> Result<Protocol, ParseError>` | C `model/parse.rs:25` | on a branch (`wave/2026-11-21`) |
| C2 | validate | `validate::validate(&Protocol) -> Result<(), Vec<Problem>>` | C `validate/mod.rs:806` | on a branch (`wave/2026-11-21`) |
| C3 | compile (and compose with imports) | `ir::compile(&Protocol) -> Result<Ir, Vec<Problem>>`; with imports `ir::compose(&protocol, &imported)` first; the `ir.json` form is `Ir::canonical_json()` | C `ir/mod.rs:294`, `ir/mod.rs:674`; the line of `compose` was not recorded | on a branch (`wave/2026-11-21`) |
| C4 | evaluate | `eval::evaluate(&Ir, &Case, &[EvidenceRecord]) -> Result<Decision, Refusal>`, `eval::evaluate_with(...)`; readers `eval::read::read_ir`, `eval::case::read_case`, `eval::evidence::read_evidence`; text `eval::decision::render(&Decision)` (not checked byte-equal to the CLI's stdout) | C `eval/mod.rs:503`, `:509` | on a branch (`wave/2026-11-21`) |
| C5 | check | `check::check(&Ir, Option<&Properties>) -> Result<Report, Refusal>`; properties `check::properties::read_properties` | C `check/mod.rs:605` | on a branch (`wave/2026-11-21`) |
| C6 | simulate | `simulate::simulate(&Ir) -> Result<Report, Refusal>` | C `simulate/mod.rs:1000` | on a branch (`wave/2026-11-21`) |
| C7 | the exit-status mapping of `check` | private to `canon-cli` | not read as code; reported by the Canon session | not planned |
| C8 | import handling | CLI-only (the library has `ir::compose`, row C3, which takes the imported protocols as values) | reported as CLI-only | not planned |

Facts that bear on use:

- A check stops at 65536 states (`check/mod.rs:291-297`). One call `check(&ir, Some(&properties))`
  decides the plain subjects and the properties from one enumeration. If the gate probe is a
  properties file, one call replaces Runbook's two calls (halves the slow lane of `check-protocols`).
- Comparing a `simulate` report with a pinned report is Runbook's, not Canon's.
- Counts: 6 `on a branch`, 2 `not planned`, 0 `exists`, 0 `requested`.

## Loom

Runbook's bindings on `main` hold the actions `program` (50), `open` (7), `person` (5) and `copy`
(4), and no `agent`. A `person` action stops the run with exit 3 and needs `tree approve`. Loom's
`tree` command group (`loom-cli/src/tree.rs` on B) only parses, calls and prints; the logic is in
`crates/loom-tree` as public functions with plain-data arguments, and no library function reads the
current directory or an environment variable except as listed under "Limits".

"Blocker" names the upstream blocker a `requested` row is filed under. Every Loom `requested` row
is filed under `upstream-blocker:loom-library-entry`; a second blocker is named where one already
covers it. "Blocks" names stories of the epic.

| id | capability Runbook needs | crate and public item | read at (paths under `crates/loom-`) | state | blocks |
|---|---|---|---|---|---|
| L1 | load protocols (from YAML text) | `b10x-loom-protocols`: `ProtocolCatalog::default()` and `add_yaml(&mut self, name: &str, yaml: &str, source: ProtocolSource) -> Result<(), String>` (compiles with Canon) | H `protocols/src/lib.rs:241` | on a branch (`wave/2026-11-21-w7`) | |
| L2 | load protocols from a directory | `--protocols`, the `Directory` source kind and a public `registration_for` (private today) | H `protocols/src/lib.rs:318`; Loom story `tree-run-command` § Outcome | requested (Loom story; blocker `loom-library-entry`) | `engine-on-loom-library-entry` (whether `add_yaml` alone is enough: not checked) |
| L3 | load and check bindings | `b10x-loom-bindings`: `decode(text: &str) -> Result<BindingsDocument, Refusal>`, `check(&BindingsDocument, registration: &str, &b10x_canon::model::Protocol) -> Result<(), Refusal>`; `ProtocolEntry.model` is a public Canon `Protocol` | H `bindings/src/decode.rs:56`, `bindings/src/load.rs:30`, `protocols/src/lib.rs:60-65` | on a branch (`wave/2026-11-21-w7`) | |
| L4 | open a case tree | `b10x-loom-tree`: `open_tree(&OpenRequest<'_>) -> Result<OpenedTree, Refused>`; `OpenRequest` holds store, workspace, protocol registration, bindings path, revisions `Vec<(String, String)>`, case, `TreeTime` and `&ProtocolCatalog` | B `tree/src/open.rs:89`, `:41-58` | on a branch (`impl/tree-run-command`, unmerged; its `Cargo.toml` pins Canon `0.9.0`) | |
| L5 | open a tree from an intent | `open_tree` takes a registration, not an intent; routing an intent to `tree open` (a model call) is in no story Loom read | B `tree/src/open.rs:41-58` | not planned | |
| L6 | run a tree in process | `run_tree` in `crates/loom-tree`: agreed 2026-11-22, plain-data arguments, an optional observer called per run-log line, a cancel handle checked between steps. Loom's story `tree-run-command` § Acceptance (line 303) names it and states no signature. Its parts exist on H: `run_until_blocked(governor, executor, authority, effects, commission, runs, context) -> Result<LoopEnd, LoopError>`, `ProgramEffects::new(governor, case, document, workspace, store, runner) -> Result<Self, ConfinementError>`; `terminate` is on `impl/child-case-runtime` | H `commission/src/runtime.rs:1194`, `bindings/src/program.rs:132`; `governor/src/children.rs:643` on `impl/child-case-runtime` | requested (agreed 2026-11-22, not built; needs `child-case-runtime`, `bound-program-retry-records`, `bound-program-contract`, all unmerged; blocker `loom-library-entry`, also `loom-tree-run`) | `engine-on-loom-library-entry`, `run-in-process-and-cli-as-daemon-client`, `parity-run-on-loom`, `presets-embedded-and-runbook-test` |
| L7 | records as they are written | the observer of L6, one call per run-log line (agreed 2026-11-22, not built). Today no callback, channel or stream exists or is planned. Commission's `EventSink::emit(&mut self, &RuntimeEvent)` carries runtime events, not run-log lines; a bridge from it into the run log is only "inferred" in `tree-run-command` § Scope. A stream of evidence records: not known | H `commission/src/ports/events.rs:26`; B `tree/src/log.rs:136-169` (the tree's writer `Logs` is `pub(crate)`) | requested (agreed, not built; blocker `loom-library-entry`) | `engine-on-loom-library-entry` (acceptance: every record reaches the sink), `daemon-skeleton-on-a-runtime-trait` only through its stub |
| L8 | records by reading | `RunLog::append(&self, case, at, RunLogEvent)` and `read(&self, case)` in `b10x-loom-runlog`; `tree_status` and `verify_tree` take no lock, so they can be polled | H `runlog/src/lib.rs:193`, `:208`; B `tree/src/lib.rs:18-19` | on a branch (`wave/2026-11-21-w7`; `impl/tree-run-command`) | |
| L9 | the status reading for the parity comparison | `tree status` prints `format: loom.tree-status/1` with, per case: evidence, runs, approvals, copies, link name, parent, protocol id and revision, outcome or stop, open obligations. The same value is the return of a public library function (agreed 2026-11-22, not built). Runbook's Loom-side reader reads that one output and no store file. Today `tree status` prints JSON per case with id, protocol, outcome or stop, open obligations and the paths of the case file and run log; `status_json` and `tree_status` are on B | B `cli/src/tree.rs:11-14`, `:60-143`; `tree/src/lib.rs:18-19` | requested (agreed, not built; blocker `loom-library-entry`). Today's narrower reading: on a branch (`impl/tree-run-command`) | `parity-run-on-loom` |
| L10 | approve a `person` action | `approve(&ApproveRequest<'_>) -> Result<CaseApproval, Refused>` | B `tree/src/approve.rs:48` | on a branch (`impl/tree-run-command`) | |
| L11 | decisions: list open requests for a principal, answer, take the recommendation | `b10x-loom-decision`: `open_decisions_for_principal(&S, &Principal)`, `decide(&mut S, Decide) -> Result<DecideOutcome, DecisionError>`, `take_recommendation`, trait `DecisionStore`, `InMemoryDecisionStore`. Not in `main` or `0.58.0` | H `decision/src/views.rs:56`, `apply.rs:400`, `apply.rs:517`, `store.rs:119`, `store.rs:193` | on a branch (`wave/2026-11-21-w7`) | |
| L12 | a file-backed decision store | `FileDecisionStore::open(dir)` | `governor/src/file_decision.rs:108`, `:122` on `impl/decision-request-file-store` (commit not recorded) | on a branch (`impl/decision-request-file-store`) | |
| L13 | a tree wait becomes a decision request; `loom_sdk::decisions` | Loom stories `tree-waits-open-decision-requests` and `decision-inbox-commands` (both proposed) | Loom story files | requested (Loom stories, proposed, not agreed; blocker `loom-library-entry`) | `engine-on-loom-library-entry` (answer a decision), `run-in-process-and-cli-as-daemon-client` (`runbook decide`) |
| L14 | cancel a run | a cancel handle of `run_tree`, checked between steps (agreed 2026-11-22, not built). The type is `Cancel`, defined in the intake slice and re-exported by `b10x-loom-tree` (L15). The hook exists: `HostControl::host_stop(&mut self) -> Option<HostStop>` is asked before each step, and `--max-wall-seconds` maps to it (`tree-run-command` § Scope). `tree stop` is daemon-only (Loom story `tree-daemon`, draft) | H `commission/src/runtime.rs:641-648` | requested (agreed, not built; blocker `loom-library-entry`) | `engine-on-loom-library-entry`, `run-in-process-and-cli-as-daemon-client` (`runs cancel`) |
| L15 | cancel a running bound program | Loom story `cancel-a-running-bound-program` (proposed, Loom plan commit `df62eaec`; depends on `tree-run-command` and `concurrent-confined-runs`). `Cancel` in `b10x_loom_intake_slice::confinement`: `Cancel::new()`, `cancel(&self)`, `is_cancelled(&self) -> bool`, cloneable, not undoable; it is the handle `run_tree` takes and is re-exported by `b10x-loom-tree`. `ChildCommand::with_cancel(cancel)`, `ProgramEffects::with_cancel(self, cancel) -> Self`; `ProgramEffects::new` keeps its six parameters. A cancel ends a running program within about 7 s under either adapter: `linux` kills at the next read step (`READ_STEP` 1 s), `local` polls every 100 ms and kills the process group (at most `KILL_SETTLE`, 5 s). The step ends `Cancelled` (`Cancelled: action <action>: <detail>`, `ends_run: true`); its `action finished` run-log line holds a refusal `Cancelled`, and `tree status` gives the case `stop` `Cancelled`, `stop_next` with the action, and the cancelled run carries `cancelled: true`. Until this story lands a cancel shows as stop `BudgetSpent`. The step has no evidence record, no result-table row, no blob and no retry; the observation is kept and counts against `budget.max_runs`. A cancel set before the start is refused before any driver open. The timeout path is unchanged. Today `run_child` and `ProgramEffects` take no cancel argument; only `spawn_child` returns a `ConfinedChild` with `signal()` | as cited in the story, not re-read: `intake-slice/src/confinement.rs:96-106`, `:746-761`, `:763-777`, `:825`, `intake-slice/src/confinement/engine.rs:570-585`, `bindings/src/program.rs:132-150`, `:177-434`, `:323-330`; today `confinement.rs:593-627`, `:643-669`, `confinement/child.rs:198`, `program.rs:264`, `:506` | requested (Loom story, proposed; ordered after the stub-cycle path, `per-run-program-path` and `concurrent-confined-runs`; blocker `loom-library-entry`) | none yet: cancel in D and E takes effect at the next step boundary and a running program ends at its timeout until this is built |
| L16 | a confinement level per run | `EngineConfinement { adapter: Linux \| Local, local_reason, cgroup_root }` passed to `EngineRunner::open`. The CLI has `--confinement substrate\|local\|none` (`none` deprecated) and `--local-reason`, required for `local`. `local` is unconfined, run as the operator, and needs a non-blank reason. For bound programs `admit_bound_program_runner` refuses no runner (`NoRunner`) and a `local` runner whose reason is not exactly `UNCONFINED_FLAG_REASON` (`CommandUnconfined`); the fixed text stays (agreed 2026-11-22) | H `intake-slice/src/engine.rs:89-117`, `:205-219`; `intake-slice/src/bindings.rs:79-80`, `:382-401` (called from `bindings/src/program.rs:139`); `cli/src/lib.rs:786-790`, `:1001-1010` | on a branch (`wave/2026-11-21-w7`) | |
| L17 | the list of levels | no function lists them. Planned in Loom drafts: `loom.engine/1` with `confinement: linux\|local` and `local_reason` read by `--engine` (`engine-configuration-document`); levels `local`, `linux`, `oci`, `pod` (`confinement-level-required`); `adapter: daemon` through `b10x-substrate-sdk` (`substrate-daemon-runner`) | Loom story files | not planned (drafts only) | `confinement-level-per-run` (`--list-confinement`) |
| L18 | several runs in one process | Loom story `concurrent-confined-runs` (proposed, Loom plan commit `df62eaec`). The form is one table of opened Substrate drivers for the whole process. Today every `SubstrateRunner` keeps its own `Drivers` map (a new `Arc::default()` per runner), so the second run's runner opens a second driver under the same cgroup root; opening a driver ends every confined run under that root, so the open is refused while the first run's launch runs. The table is keyed by the canonical cgroup root, the directory that holds the workspace and the `DriverEgress`; it holds each driver weakly, each runner keeps strong references, and a driver closes when the last runner that used it is dropped. Launches with an equal key share one driver and run side by side. A shared driver runs at most 16 executions at once and tracks 128. Not the form: one cgroup root per run, the form Runbook preferred and the Loom session first agreed. It fails in Substrate, whose probe proves `limits` only for a root that holds the calling process's own cgroup; one process sits in one leaf, so sibling roots per run hold no process and every run under them is refused `CgroupUndelegated`. Changing that is a change to Substrate, out of scope there. The caller's four duties: "What the daemon must do for confined runs". Separate store, workspace and catalog per call and a per-store lock (`tree.lock`, `tree.holder` with the process id) exist on B; a direct `tree run` is sequential | story: `intake-slice/src/confinement.rs:249-252`, `:422-427`, `:508-519`, `:274-278`, `:380-385`, `:401-419`, `:485-486`, `:3441-3462`; Substrate `crates/substrate-host/src/probe.rs:931-948` at rev `adfde402` (pinned by `intake-slice/Cargo.toml:30-31`) and `crates/substrate-host/src/lib.rs:529-530`; B `tree/src/open.rs:41-58`, `tree/src/lock.rs:14-18`, `:38` | requested (Loom story, proposed; blocker `loom-library-entry`, also `loom-confinement`) | `confinement-level-per-run`; `engine-on-loom-library-entry` once two runs overlap at the `linux` level |
| L19 | a launch that needs another directory or another egress set while a launch runs under the same root | stays refused `CapabilityUnserved` with today's text under L18. Bound programs name no egress, so for them only the directory rule applies. Loom story `mixed-egress-launches` (proposed, filed after Runbook's request of 2026-11-22; depends on `concurrent-confined-runs` and `cancel-a-running-bound-program`) changes it: a launch that needs a driver for another directory or egress set waits for it, first come first served per root, bounded by the launch's own timeout. On that timeout the refusal is `DriverBusy` (not `CapabilityUnserved`) and names the holder's egress set; a wait that runs out records nothing. A cancel handle ends a wait. Each wait writes two run-log lines. No Substrate change: Loom takes the form "wait in Loom", because only opening a driver ends other launches and a launch on an already open driver reconciles nothing. The other form, several drivers serving at once under one root, needs a Substrate driver that ends only its own orphaned executions; Loom does not take it and files it upstream only if the measured waits are too long. Ordered after `cancel-a-running-bound-program` and before `named-egress-per-action`, the first story that gives a tree step egress under Substrate | story `mixed-egress-launches` § Outcome: `intake-slice/src/confinement.rs:510-519`, `:510-526`; Substrate `crates/substrate-host/src/process.rs:479-500`, `:504-542` at rev `adfde402` (as cited in the story, not re-read); `intake-slice/src/bindings.rs:414-450` | requested (Loom story `mixed-egress-launches`, proposed; blocker `loom-library-entry`) | `driver-runtime-removed` (agent nodes with egress in the daemon, L20); no other story in the epic |
| L20 | the agent executor and the decider configuration that replace `bindings/tools/llm-node.sh` and `decider.sh` | the six Loom answers do not cover it | none | not checked (see `upstream-blocker:loom-decider-per-class` and `bindings/PENDING.md`, "Agent nodes") | `driver-runtime-removed` |
| L21 | the suite runner behind `runbook test` | `crates/loom-tree/src/suite/{decode,select,check,availability,run}.rs` with `decode_suite`, `evaluate_checks`, `run_suite`, `SUITE_AVAILABILITY` (all "inferred", `tree-suite-format` § Scope lines 431-463; no signatures). Each step runs through the function of its `tree` command, in process; `tree test` starts no `b10x-loom` process. `impl/tree-suite-format` holds only the generated `loom::tree` suite types (`1619e7b3`) and no runner | Loom story `tree-suite-format` (active), lines 58-62, 234, 250 | requested (Loom story, active, not built; blocker `loom-library-entry`) | `presets-embedded-and-runbook-test` |
| L22 | Canon through one Loom crate, so Runbook names no `b10x-canon` dependency | a re-export of Canon in `loom-protocols` (agreed 2026-11-22). Today no crate has `pub use b10x_canon`; Canon types are public in Loom's API (`ProtocolEntry.model`, `check(.., &Protocol)`); only `loom_governor::gates` re-exports some types | H `protocols/src/lib.rs:60-65` | requested (agreed, not built; blocker `loom-library-entry`) | `engine-on-loom-library-entry` (the form of `runbook/Cargo.toml`) |
| L23 | the `PATH` of a run's bound programs | Loom story `per-run-program-path` (proposed, Loom plan commit `df62eaec`; depends on `tree-run-command` and `bound-program-contract`). The settings value `run_tree` takes gains `program_path: Option<String>` (directories separated by `:`) and `program_path_source` (`library` by default); the CLI gets `--program-path <dirs>`. Absent means the process `PATH`, as today. Every bound program and every `arguments_from` program of the run starts with it (`ProgramEffects::with_program_path`). The check is one public function of `b10x-loom-bindings`, `program_path(value, workspace, local) -> Result<ProgramPath, ProgramPathRefusal>`. Refused before any process starts and without a run-log line, as `ProgramPathUnusable` (a new reason of `loom.tree.TreeRefusalReason`): longer than 4096 bytes, an empty entry, an entry that is not absolute, has a `..` component or is not an existing directory. Under a runner that is not `local`, also refused: an entry reached through a symbolic link, an entry that is, holds or lies inside the run's workspace, and more than 4 entries outside the system view (`/usr`, `/bin`, `/lib`, `/lib64`); each such entry is mounted read-only at its own path, at most 4 (`ChildCommand::MAX_READ_ONLY_ROOTS`). The run log gets one `program path` line before the first step (`loom.runlog.ProgramPath`) with `source` (`command line`, `library` or `process`), `entries` (the count) and `sha256` (of the value as given, or of the process `PATH`); it holds no directory and no value | as cited in the story, not re-read: `bindings/src/child.rs:33-53`, `:47-70`, `bindings/src/program.rs:132-150`, `intake-slice/src/bindings.rs:75`, `:86`, `:414-450`, `intake-slice/src/confinement.rs:1233`, `:1242-1259`, `:1255`, `:1297-1306` | requested (Loom story, proposed; blocker `loom-library-entry`) | `engine-on-loom-library-entry` (Loom's reason: a daemon with several runbooks, each with its own tool directory, would otherwise put every directory on one process `PATH`) |

Counts (23 rows): 8 `on a branch` (L1, L3, L4, L8, L10, L11, L12, L16), 12 `requested` (L2, L6,
L7, L9, L13, L14, L15, L18, L19, L21, L22, L23), 2 `not planned` (L5, L17), 1 `not checked` (L20),
0 `exists`. L9 also has a narrower reading `on a branch`; it is counted once, as `requested`.
Three rows rest on Loom stories filed on 2026-11-22 (plan commit `df62eaec`, status proposed):
`concurrent-confined-runs` (L18), `cancel-a-running-bound-program` (L15) and
`per-run-program-path` (L23); `mixed-egress-launches` (L19, proposed later the same day) follows
Runbook's request.

### Where the parity summary's members live

The parity summary of `story:parity-run-on-loom` has these members. Loom's reading of each, at its
integration head `54204526`. Runbook reads the `loom.tree-status/1` output (L9), not the store.

| member | where Loom holds it |
|---|---|
| per case: name | the case link's `name` (`CaseLink {parent, child, name, action}`) |
| per case: protocol | `protocol`, the registration string `<id>@<rev>`; the case's own integer is `revision` |
| per case: outcome | `termination` |
| per evidence record: kind, result, subject artifact, upstream revisions, order | `evidence[]` entries `{data, applies}`; `data.facts` holds the `canon-evidence/1` record. Field names inside it were not checked here; the order is the order of the array (not stated by Loom) |
| decision requests and answers | story `decision-request-file-store`; not in the stub-cycle path (L12, L13) |
| file list of the product tree | the workspace directory is the product tree; its path is recorded beside the root case |
| program runs (for the attempt count) | `observations.json`, source `loom/bound-program`, payload `action`, `exit`, `timed_out`, `runner_refusal`, `retry_of`, `blob_sha256`; attempt = length of the `retry_of` chain |

Store layout until the status format exists: one file per case `case-<hex of case id>.json`
(`case-sha256-<hex>.json` for ids over 100 bytes; members of `StoredCase` in Loom's
`ess/domains/governor.yaml`); `observations.json` (program runs and `loom/open`); `.lock`; beside
each case file `runlog-<stem>.jsonl` (hash-chained lines with `seq`, `at`, `kind` and the event's
members); beside the root case `tree-protocols.json` (per protocol id: revision, file name, sha256)
and the recorded workspace path (`governor/src/file_store.rs:1-30`).

### Order of the stub-cycle path in Loom

Still to merge into the integration branch, in order: `child-case-runtime`,
`record-copy-between-cases`, `bound-program-contract`, `open-children-from-arguments-from`,
`tree-run-command` (`tree open/run/approve/close/status/verify`). The first runnable commit is the
head of `impl/tree-run-command` after it has merged the four; Loom names the sha. The integration
branch already holds the Canon `0.10.0` pin, retry records, `LOOM_CASE`, artifact revisions, the
program result table (`bindings/src/program.rs`), the default selector and the hash-chained run log.
After that path, in the order Loom set (2026-11-22, not built): `per-run-program-path` (L23) and
`concurrent-confined-runs` (L18), then `cancel-a-running-bound-program` (L15), then
`tree-waits-open-decision-requests` (L13).

## Dependency form and pin

- Form: git dependencies in `runbook/Cargo.toml`, `git = "https://github.com/beyond10x/loom"` with
  `rev = "<commit>"`, for the crates the engine names (`b10x-loom-tree`, `b10x-loom-protocols`,
  `b10x-loom-bindings`, `b10x-loom-decision`, `b10x-loom-runlog`, as the rows list them). The lock
  file in `runbook/` holds the resolved sources; cargo ignores the lock file of a dependency, so
  Loom's `Cargo.lock` does not bind Runbook.
- Canon: Runbook names no `b10x-canon` dependency once L22 exists. Until then it declares
  `b10x-canon` with the same git source and the same tag as Loom, so that cargo unifies it; two
  spellings link two Canons and their `Protocol` types do not match. Loom's crates pin tag `0.10.0`
  on the integration branch (`loom-bindings/Cargo.toml:14`, `loom-governor/Cargo.toml:15`,
  `loom-protocols/Cargo.toml:14`; `Cargo.lock:188-190`). B's `loom-tree/Cargo.toml` pins tag `0.9.0`;
  the merge must change it, or two Canons are linked.
- Pin: Runbook pins the commit Loom names, the first with `run_tree` (L6), and then the tag that
  contains it. No commit has `run_tree` yet. No tag contains `loom-tree` or `loom-decision`; `0.58.0`
  does not, and `50ddc47d` is in no tag and no remote branch, so nothing complete can be pinned
  today.
- Canon rows are read at `7d1a28a20` on a branch; the Canon version Runbook links is the one Loom
  pins (today `0.10.0`), so the Canon rows still have to be confirmed at that version (the public
  items at tag `0.10.0` were not checked).
- Toolchain: Runbook pins rustc `1.99.1` in `rust-toolchain.toml`. Loom has no `rust-version` in
  any manifest and no `rust-toolchain.toml`; edition 2024 (`[workspace.package]`); its story
  `rust-1-100-toolchain` is a draft. Whether Loom's crates build with Runbook's rustc: not checked.
- Features: no Loom crate has a `[features]` table, so every dependency is hard and nothing can be
  switched off from Runbook.
- Layout: every internal dependency in Loom is a path dependency. The generated `loom`, `intake`
  and `commission` crates are path dependencies outside the workspace (`Cargo.toml` `exclude`), each
  with an empty `[workspace]`. There is no `[patch]` and no extra registry; external crates come
  from crates.io and ten git sources (`Cargo.lock`). `.cargo/config.toml` and `publish = false` (24 of
  28 crates) do not affect a git dependency. Whether cargo resolves those path crates from a git
  checkout of Loom: not checked.
- Hard-linked weight, from the dependency lists of the crates:
  - `loom-protocols`: canon, the generated `intake`, serde_json, sha2, tempfile, url.
  - `loom-governor`: protocols, canon, commission, executor (llm-core, tokio), the generated `loom`,
    serde_yaml_ng, sha2.
  - `loom-bindings`: canon, commission, governor, intake-slice, the generated `loom`, rustix, serde,
    sha2, tempfile. `loom-intake-slice` brings intake-router, intake-references, executor,
    llm-routing, llm-tool-call (which brings llm-http, llm-credentials, llm-providers),
    substrate-host (git2, gix, curl, tokio), substrate-wire, tokio `rt-multi-thread`, nix, chrono and
    jiff.
  - `loom-tree` (on B): canon `0.9.0`, bindings, commission, governor, intake-slice, protocols,
    runlog, the generated `loom`, serde_json, sha2.
  - Substrate: `b10x-substrate-host` (feature `git`) and `b10x-substrate-wire` at rev `adfde402…`,
    version `=0.21.0`, in `loom-intake-slice`; every crate that depends on `loom-bindings` gets them.
    `b10x-substrate-sdk` is not linked today.
  - Not in this set: Slack, connectors, A2A, laya and conformance. They come only through `loom-cli`
    (a2a, plugin, plugin-slack) and `loom-sdk` (connectors, session-executor, metaharness). The LLM
    HTTP stack and Substrate are inside the set, hard-linked.

## Limits Runbook designs around

- Calls block. `run_until_blocked` is a plain `fn`; `loom-commission` has no async code. Inside, each
  launch builds a current-thread tokio runtime and calls `block_on` (`intake-slice/src/confinement.rs:692`,
  `confinement/engine.rs:474`, `:771`, `:799-803`; `executor/src/model_selector.rs:369`;
  `intake-slice/src/run.rs:1230`). A nested `block_on` panics on a runtime thread. Agreed: Runbook
  calls the runtime from a plain thread per run, never from a tokio task. The tree-run function
  blocks until the tree stops or ends.
- One writer per tree: the lock is per store, in the store. A direct run is sequential; running
  several cases of one tree at once is the daemon's job in Loom (`tree-run-command` § Out of scope,
  line 358; `concurrent-cases-in-the-tree-daemon`, proposed), not a library call.
- Process-global confinement state: at the `linux` level every runner keeps its own drivers, and a
  second tree is refused `CapabilityUnserved` while a launch runs under the same cgroup root (L18).
  Until L18 is built, Runbook runs at most one `linux`-confined run at a time per process. After
  L18 (one driver table for the process, per-run cgroup roots do not work in Substrate) the daemon
  follows "What the daemon must do for confined runs". `linux` needs a delegated cgroup v2, else
  `CgroupUndelegated` (`confinement.rs:501-506`); `local` goes through
  `substrate_host::confinement::open(AdapterRequest::Local)` (`engine.rs:111`). The text of the
  unconfined reason is fixed (`UNCONFINED_FLAG_REASON`); an embedder cannot choose another.
- A launch that needs another directory or another egress set than the driver of a launch running
  under the same root stays refused `CapabilityUnserved` after L18. Bound programs name no egress,
  so for them only the directory rule applies. Loom's story `mixed-egress-launches` (L19, proposed)
  replaces the refusal for a launch that needs another directory or egress set: it waits for the
  driver, first come first served per root, bounded by the launch's own timeout; on that timeout
  the refusal is `DriverBusy` and names the holder's egress set; a cancel handle ends a wait; two
  run-log lines per wait; no Substrate change. It is ordered after
  `cancel-a-running-bound-program` and before `named-egress-per-action`, the first story that
  gives a tree step egress under Substrate. It is needed before agent nodes with egress run in the
  daemon.
- Cancel is at step boundaries: the runtime asks `host_stop` before each step (L14). A bound program
  already running is not cancelled; it ends at its timeout, the binding's `timeout_seconds` (default
  60, `bindings/src/program.rs:215`), when Substrate ends the child and Loom sends `Signal::Kill`
  after a drain margin (`confinement.rs:771`, `:808-820`). After L15 a cancel ends a running
  program within about 7 s; the step ends `Cancelled` with no evidence record, no result-table row
  and no retry.
- Child processes go through `TestRunner::run_child`, which blocks to the end (`confinement.rs:78-103`).
  The only programs Loom starts are `git`, `ess`, `cargo` and the connectors program; it starts no
  `substrate` binary (no `Command::new("substrate…")` in any `crates/*/src`).
- Environment read at run time: `PATH`, the only variable a bound program inherits
  (`intake-slice/src/bindings.rs:75`, `:423`; one `PATH` for the whole process until L23 gives each
  run its own); `XDG_DATA_HOME` and `HOME` in `InstallStore::user`
  (`protocols/src/store.rs:23-36`; the CLI builds the catalog from it, a library caller does not
  have to); `PATH`, `HOME` and `SSH_AUTH_SOCK` for `git` (`protocols/src/git.rs:125-130`); the
  baseline variables (`confinement/engine.rs:712`); `RUSTUP_HOME`, `CARGO_HOME`, `HOME` and `PATH`
  for the test runner (`confinement.rs:3286-3308`, `:3415`). Also read: `/proc/self/cgroup`
  (`confinement.rs:3608`) and the system temporary directory for `loom-substrate-*`
  (`confinement.rs:480`).
- Current directory and signals: no `set_current_dir` outside the CLI; `current_dir` is read only to
  resolve a relative install path (`protocols/src/store.rs:248`). Signal handlers are in the CLI
  only (`cli/src/main.rs:191`, `:760`).
- Statics: a `OnceLock` of the compiled-in engineering catalog (`governor/src/lib.rs:1956`);
  counters `NEXT_TEMPORARY` (`governor/src/locked_dir.rs:34`, `intake-slice/src/state_dir.rs:35`),
  `ISSUED` (`bindings/src/program.rs:909`), `NEXT_EXEC` (`confinement.rs:70`) and `MADE`
  (`executor/src/lib.rs:278`); the `RUNNING` map (`confinement.rs:381`); a `RECORDINGS` mutex
  (`intake-slice/src/git.rs:129`, use not read); a `thread_local` (`executor/src/metering.rs:346`).
  No global store and no lock at a fixed path.
- Canon: a check stops at 65536 states; no version rule for the public functions, so a Canon change
  can break a call; Runbook's tests of C1 to C6 are the detection.

## What the daemon must do for confined runs

Required by Loom's story `concurrent-confined-runs` (L18, proposed, not built) of a library caller
that runs several `linux`-confined trees in one process. The four duties are the story's "What a
library caller does":

1. Run the process in a delegated cgroup v2 subtree with the process in a leaf below the root.
   `prepare_delegated_cgroup` (`confinement.rs:3631-3641`) does this inside a scope that
   `systemd-run --user --scope -p Delegate=yes` started.
2. Give every run the same `cgroup_root` in `EngineConfinement`, or none (the discovered root,
   `discover_cgroup_root`, `confinement.rs:3615-3628`).
3. Give each run its own workspace directory, and make all these directories children of one
   shared directory.
4. Open one `EngineRunner` per run, or share one. Both work.

Runbook's decisions that follow (2026-11-22, not built):

- Workspaces are at `<state>/workspaces/<run>`, children of the one shared directory
  `<state>/workspaces` (duty 3). Each run's store and logs are at `<state>/runs/<run>`. `<state>` is
  `$XDG_STATE_HOME/runbook`.
- The daemon passes no `cgroup_root` per run: it gives every run the same one, or none (duty 2).
- The daemon's systemd unit sets `Delegate=yes` (duty 1). Started by hand, outside a delegated
  scope, it re-executes itself under `systemd-run --user --scope -p Delegate=yes`.
- The daemon opens one `EngineRunner` per run (duty 4).

What stays refused, whatever the daemon does: a launch that needs another directory or another
egress set than the running driver's (L19).

## What each `requested` row blocks

| story | waits on |
|---|---|
| `story:engine-on-loom-library-entry` | L6, L7, L13, L14, L22, L23; L2 in part; L18 once two runs overlap at the `linux` level; both upstream blockers; no Canon row is `requested` |
| `story:run-in-process-and-cli-as-daemon-client` | L6, L13, L14 |
| `story:parity-run-on-loom` | L6, L9 |
| `story:confinement-level-per-run` | L18; L17 is `not planned` and `--list-confinement` has no Loom list to print |
| `story:presets-embedded-and-runbook-test` | L21 |
| `story:driver-runtime-removed` | L20 (not checked); L19 (`mixed-egress-launches`) for agent nodes with egress |
| `story:daemon-skeleton-on-a-runtime-trait` | none: it runs on a stub runtime |
| no story yet | L15: cancel in D and E works at step boundaries until it is built |

## Not done

- The scratch `cargo check` of a crate that depends on Loom (and Canon) by git is not done: nothing
  complete can be pinned yet (see "Dependency form and pin"), and whether the crates build with
  rustc `1.99.1` is unknown.
- No Loom or Canon function was run. Every row is read from source by the neighbour session.
