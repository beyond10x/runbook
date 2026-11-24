# The engine

`runbook::engine` is the second implementation of the daemon's `Runtime` trait
(`runbook/src/runtime.rs`). It runs a case tree through Loom's library calls inside the daemon's
process. It starts no `b10x-loom`, `loom` or `canon` process; only the bound programs of
the bindings run as processes, started by Loom's runner. Loom is linked as three git dependencies
pinned by commit (`runbook/Cargo.toml`); Canon comes through Loom (tag `0.11.0`, in `Cargo.lock`).
The pin and the rows it was read at: `docs/library-entries.md`, "States at `5af3f9c2`".

The code that calls Loom (the opening of a tree, one `run_tree`, the answer to a decision, status
and verify, the runner of a level) is one module, `runbook/src/engine/calls.rs`, used by the engine
and by `runbook run` with no `--loom` (`docs/loom-run.md`, "The two ways to Loom"); there is no
second copy.

`runbook serve --principal <person:id | agent:id> --runtime loom|stub` selects it (`--principal` is
required with either runtime). `stub` stays the default: a runtime that starts
programs is not made the default before the daemon's Host and Origin check exists (decision of the
coordinator, 2026-11-22). `--runbook-home DIR` (default: the asset root) is the directory with
`protocol/` and `bindings/`; a runbook `name` is `protocol/<name>.protocol.yaml` with
`bindings/<name>.yaml`.

## A run

`start` runs on the daemon's run thread, never on a tokio task (Loom blocks inside `run_tree`).

1. Refuse, before anything is written: a confinement level the engine cannot give, an unknown
   runbook or version, a runner Loom refuses.
2. Write the intake as the idea artifact of the root bindings (`idea.yaml` in the workspace), read
   the protocol directory, `open_tree` with the store under the run's directory of `--state-dir`
   (`runs/<run id>/tree`). Nothing is written into the repository or the runbook directory.
3. `run_tree` until it stops. Every run-log line reaches `EventSink::record` unchanged, as the
   line the store holds (`kind` is the line's `kind`).
4. On a wait (`AwaitingPerson`, `AwaitingAuthority`) one request per decision goes to the sink;
   each answer is one library call; then `run_tree` again. On `Terminal` the outcome of the root case
   is the run's outcome.

## Decisions and answers

A decision is read from its structured members (`case`, `kind`, `action`, `capability`, `outcome`);
the printed `command` is neither parsed nor run.

| Loom kind | answer `approve` | answer `deny` | a result word, or `{"result":"..","reasons":".."}` |
|---|---|---|---|
| `Approve` | `approve` (allow the capability) | `approve` with deny | refused by Loom |
| `ApproveAction` | `approve` with the action, result `performed`, as the answering principal | refused by Loom | `approve` with that result and the reasons |
| `Close` | `close` through the decision's outcome | declines | refused |
| `Handoff` | the text was done: run again | the same | the same |

A refused answer (the daemon has committed it already) is asked again as a new request that quotes
Loom's refusal. A decision that lacks the member its call needs is unanswerable: its request says
so and names the kind and Loom's `file:line`; the run waits for a cancel. The answering principal is
the daemon's own, the one `runbook serve --principal <person:id | agent:id>` was started with (required,
no default); the `answered_by` label a client types stays in the daemon's record of the answer and
never reaches Loom. A person action needs the binding's principal, which `NextDecision` does
not carry (Loom `tree/src/person.rs:65`).

## Confinement

Passed unchanged, never lowered. The engine offers `local`, and `linux` when the process sits in a
cgroup it may delegate (`Engine::new`); anything else is refused before the store exists. The
`linux` level was not exercised by the tests (no delegated cgroup in the gate lane).

## Caps

The engine is made with a caps policy (`Engine::new(home, state_dir, caps)`): the daemon's three caps
and their ceilings. A run is driven one step at a time (`max_steps` of 1 for each `run_tree`), and
before each step `engine/gate.rs` holds the wall time, the model calls and the run's own spend against
their caps; the runner every bound program starts through reserves, as the program starts, the per-call
budget that program names (none: nothing) in the ledger of the runbook
(`<state dir>/ledgers/<runbook>.json`, entries and reservations keyed by the run), against the run's own
cap; after the step the cost records are booked. A run at a cap ends `stopped: wall time`, `stopped: llm calls` or
`stopped: spend cap`. A wall cap ends a sleeping step through a flag of the run's own that the
cancel handle of `run_tree` reads, and a client's cancel wins over it. The options, the request, the
ceilings and the view are in `docs/daemon-api.md`, "Caps of a run".

## Cancel and recover

Cancel while waiting ends the run at once. Cancel while a program runs ends the program too: the
cancel handle reaches the running bound program (Loom commit `f9a80257`, "A cancel handle ends a
running bound program"; row L15), no later step starts, and the run ends `Cancelled`. The daemon
shows `Cancelled` at once. The gate test is
`a_cancel_while_a_program_runs_ends_the_program_and_the_run_and_starts_no_later_step`.

A `handoff_when` check of a binding holds the same cancel (Loom `5b3142c7`, `run_handoff_check` takes
`cancel: Option<&Cancel>`; the engine has no call of it, Loom's `run_tree` makes the checks and the
engine passes its cancel flag to `run_tree`). A cancel during a check ends the check within seconds,
the `handoff checked` line holds the refusal `Cancelled: action <action>: the check <program> was
cancelled` and no exit, the action's program never starts, and the run ends `Cancelled` (status 5,
`next: [<action>]`). The gate test is
`a_cancel_while_a_handoff_check_runs_ends_the_check_and_the_run_and_starts_no_later_step`.

`recover` is always `Gone`: a run lives on one thread of one process, so a restarted daemon has no
run, whatever the store holds. The daemon then ends it as `orphaned`. The store stays on disk;
`Engine::store_state(run)` tells `Absent`, `Idle`, `StaleHolder` or `Held` (Loom's lock is a `flock`,
so a dead holder lets go by itself; `tree/src/lock.rs:82`). Resuming an orphaned run from its store
is not built.

## Loom gaps seen

All at `5af3f9c2`, paths under `crates/` of Loom unless given. A cancel ends a running program
since `23f1f4b3`, and a cancel ends a running `handoff_when` check since `5b3142c7`; the other entries
were not looked at again at that pin.

- `NextDecision` has no principal for a person action: `generated/rust/loom/src/tree.rs:102`,
  `loom-tree/src/person.rs:65`.
- The principal Loom prints for a decision is not a member of `NextDecision`; it is only inside
  `command` (`loom-tree/src/run.rs:886-899`), and the default for an action that binds none,
  `DEFAULT_PRINCIPAL` (`loom-tree/src/run.rs:70`), is not re-exported (`loom-tree/src/lib.rs:70`).
  `runbook run` with no `--loom` reads the bound principal from the bindings copy and repeats the
  default (`operator`) rather than parse the command.
- `b10x-loom-tree` re-exports neither `SubstrateRunner` nor the line encoder, and not
  `ProtocolCatalog` (`loom-tree/src/lib.rs:48-75`): the engine names `b10x-loom-runlog` and
  `b10x-loom-protocols` as dependencies too, and takes the runner through `EngineRunner`.
- `DecisionKind` has four kinds (`generated/rust/loom/src/tree.rs:78`); none is unanswerable today.

## Tests

Gate lane: `runbook/tests/daemon/engine.rs` over the fixture runbook
`runbook/tests/fixtures/engine-runbook/`. Slow lane: `task test:engine-real` (the `sdc` stub cycle
through the engine on the repository's own runbook).

## Not built

Resume of an orphaned run from its store; a stream of decisions to a person while `run_tree` runs
(decisions are read at each wait); the `linux` level under test; the spend ledger and the decider
(no model call here).
