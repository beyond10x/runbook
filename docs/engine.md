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
2. Copy the runbook into the run's directory (`runs/<run id>/runbook`, "The run's copy of the
   runbook"), write the intake as the idea artifact of the root bindings (`idea.yaml` in the
   workspace), read the protocol directory of the copy, `open_tree` on the copy with the store under
   the run's directory of `--state-dir` (`runs/<run id>/tree`). Nothing is written into the
   repository or the runbook directory.
3. `run_tree` until it stops. Every run-log line reaches `EventSink::record` unchanged, as the
   line the store holds (`kind` is the line's `kind`).
4. On a wait (`AwaitingPerson`, `AwaitingAuthority`) one request per decision goes to the sink;
   each answer is one library call; then `run_tree` again. On `Terminal` the outcome of the root case
   is the run's outcome.

## The run's copy of the runbook

A run takes its runbook at submit. Loom keeps the protocols in the run's store when the tree is opened
but reads the bindings files again at every `run_tree`, `approve`, `close` and `tree status` (the
root's file as the store names it, each child's through the `bindings` of an `open` executor), and a
bound program is started from the absolute path the bindings resolve it to. A run that named the files
of the runbook home would read a revision made while it waits: a bindings file at revision 4 under a
tree opened at revision 3 ends the run with `BindingsRefused ... ProtocolMismatch`. So the engine
copies, before it opens the tree, into `runs/<run id>/runbook/` (a directory of the run, beside its
`tree/` and outside its workspace `workspaces/<run id>/`, so nothing the run's product lists or serves
holds it):

- `protocol/` and `bindings/` of the home, whole (the root's bindings, the bindings of every child,
  the tools and every other file beside them);
- `instructions/`, `views/`, `driver/examples/` and `tools/` of the home when it has them (what the
  tools of the bindings read at `$here/../..`).

The copy is made by the routine `runbook run` copies its bindings with (`engine::copy::copy_dirs`; one
implementation, two policies for a link): the daemon copies regular files and directories whose real
path lies inside the home, follows a link that stays inside (the copy holds the file), and refuses a
link that leaves the home, a file that is neither regular nor a directory, and a link that loops. Every
refusal comes before the first write: the run ends `FailedToStart` with the refusal named and its
directory holds no copy. Nothing a client sends chooses the source or the target: a submit names a
runbook by name and version (a name with a separator or a leading dot is `no-runbook`), the source is
the daemon's `--runbook-home` and the target is the run's directory.

The daemon's copy is bounded: a runbook of more than 4096 entries (directories and files; a link
counts as what it is copied as, so links that lead to the same directory many times end at the
bound) or 16 MiB is refused before the first write, naming no path (`engine::copy::MAX_ENTRIES`,
`MAX_BYTES`; the home of this repository holds 661 entries and 1.1 MB, counted over `bindings/`,
`protocol/`, `instructions/`, `views/`, `driver/examples/` and `tools/`). It is private to the
daemon's user: `runs/<run id>/` and every directory of the copy are `0o700`, every file `0o600`, or
`0o700` when the source has an executable bit. The plan names each file it will copy by device,
inode and length; the copy opens each source without following a link and refuses the whole copy,
leaving nothing behind, when a source is no longer that file. `runbook run` keeps links and the
modes of its sources, as before, and has none of these bounds: no bound of entries, of bytes or of depth.

What a revision of the home does during a run: nothing, for that run. An edit, a replacement by a file
that does not parse, a removal and a revision of the protocols and bindings in the home change nothing
for a run that was submitted before it; a run submitted after takes the new files, and a submit that
names the old version is refused (`no-runbook`).

The run's record names its copy: the event `runbook.pinned` (the daemon's own kind, written once after
the run started and before the first line of Loom's log; open text, not one of the kinds ESS declares)
holds `runbook` (the name), `protocol` (the id the root protocol declares), `revision` (its revision),
`bindings_sha256` (sha256 over the copied `bindings/` directory) and `copy_sha256` (over the whole copy).
No path is in it. The digest is over every entry in name order: its kind, its relative path (the raw
bytes of the names) and, for a file, its length, whether it has an executable bit (any of the three, so
that the group and other bits of a source make no second digest for one runbook) and its bytes, read in
blocks, for a link its target (`engine::copy::digest`); `runs/<run id>/runbook-pin.json` holds the same
members. The value for an unchanged runbook is the one earlier daemons wrote.

The copy is checked against `copy_sha256` before each round (each `run_tree`), before each answer that
is a library call and before the final `tree status`; `open` reads the copy right after it is made and
nothing runs between the two. A copy that changed (a file edited, added, removed or made executable)
ends the run: `runbook.runtime-failed`, reason `RunbookCopyChanged: ...` (an entry named relative to the
copy, never a path of the host), outcome `runtime-failure`, and the round, answer or status that would
have read or run the changed file is not started. A run that the runtime fails on while it waits on
decisions (this one, or a failure to read the run's log) has its open requests withdrawn, as a cancel
does, and ends the same way; before, such a run stayed `WaitingOnDecision` for good. An answer to a run
whose runtime has failed and whose end the daemon has not recorded yet is refused as `run-not-waiting`:
it is not recorded as given. The daemon writes an answer to its journal before it hands it to the run,
and a run is handed only an answer the daemon recorded: an answer the journal cannot take is an error
to the client and the run has not seen it.

No reason, record or question that leaves the engine toward a client (a refusal, a failure at the start,
an event of the run, the text of a question) names a path of the daemon's state directory or of the
runbook home: an entry is named relative to the run's copy or to the home, and a directory itself is
named `.`.

The check is a read before a call, not a guard around each program. Inside one round a bound program at
confinement `local` is unconfined: it can change the copy, and a later program of the same round runs
the changed bytes. The next check ends the run, and the digest the run recorded is then not what ran in
that round. At confinement `linux` the copy is not in a bound program's view, by the pinned Loom's mount
list, except the bound program's own file: Loom binds it read-only at its host path. A check before each
program is Loom's, not the engine's.
`runbook run` has no such check; it limits the copy's budgets before each step and pins who decides.

`start` with a store that exists goes on from the store (resuming an orphaned run is not built, below):
it takes the run's copy and checks it as above. A run that has neither a copy nor a `runbook-pin.json` (a
store opened before the copy existed) reads the home as it always did. A copy that stands without a pin
file that is a regular file of one name, text, and what the submit wrote ends the run by the refusal a
changed copy has; the pin file is opened without following a link and without waiting on a pipe.

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
through the engine on the repository's own runbook). The cases of the run's copy are in `engine.rs`
(`a_revision_of_the_runbook_home_...`, `...garbled...`, `...removed...`, `a_second_submit_...`,
`...copy...`, `a_link_...`, `a_submit_names_a_runbook_by_name_...`) and, for the copy routine and the
digest, in `runbook/src/engine/copy.rs` and `pin.rs`.

## Not built

Resume of an orphaned run from its store; a stream of decisions to a person while `run_tree` runs
(decisions are read at each wait); the `linux` level under test; the spend ledger and the decider
(no model call here).
