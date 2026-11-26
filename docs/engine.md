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
`MAX_BYTES`). It is private to the
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

The copy is checked against `copy_sha256` before each round, before each answer that is a library
call and before the final `tree status`; `open` reads the copy right after it is made and nothing runs
between the two. A round is the steps between two waits on decisions; the engine takes each step as a
`run_tree` of one step, and the check stands before the round's first, not before each step. A copy
that changed (a file edited, added, removed or made executable) ends the run at the next check:
`runbook.runtime-failed`, reason `RunbookCopyChanged: ...` (an entry named relative to the copy, never a
path of the host), outcome `runtime-failure`, and the round, answer or status that would have read or
run the changed file is not started. A later step of the same round is not checked before it: at
confinement `local` a program that changes a program of its own round makes that later step run the
changed bytes, and the run ends at the next check. A program an answer starts that changes a program
of the next round ends the run at the check before that round. A run that the runtime fails on while it waits on
decisions (this one, or a failure to read the run's log) has its open requests withdrawn, as a cancel
does, and ends the same way; before, such a run stayed `WaitingOnDecision` for good. An answer to a run
whose runtime has failed and whose end the daemon has not recorded yet is refused as `run-not-waiting`:
it is not recorded as given. The daemon writes an answer to its journal before it hands it to the run,
and a run is handed only an answer the daemon recorded: an answer the journal cannot take is an error
to the client and the run has not seen it.

No reason, record or question that leaves the engine toward a client (a refusal, a failure at the start,
an event of the run, the text of a question) names a path of the daemon's state directory or of the
runbook home: an entry is named relative to the run's copy or to the home, and a directory itself is
named `.`. This holds whatever the directory's name holds: a path is taken out as the file system names
it, with each control character read as a space (a failure is cleaned before it leaves) and as Rust's
`Debug` escapes it (Loom names a refused file that way), and a directory given as a relative word is taken
out by its absolute form only, so a word of a reason that spells it stays (`engine::hide`,
`daemon::copy_integrity::names_of_paths`).

The daemon makes `--state-dir` and `--runbook-home` absolute once, at its start, before anything is built
from them (the current directory joined, no link resolved): Loom takes only an absolute store, so every path
the engine builds and passes to it is absolute. A directory that cannot be made absolute (the daemon has no
current directory) refuses the start, and the refusal names no path
(`daemon::copy_integrity::relative_state_directory`).

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

## A request read from the case

Before a request opens the engine reads the decision's case (`runbook/src/engine/decision.rs`,
`read_case`); it starts no program for it and calls no model.

- **The artifact.** `decision-request.json`, a decision request in the contract's shape
  (`sdc.decision.DecisionRequest`), written by the case's bound program into the case's `work/` (the run's
  workspace), in the directory of the decision's `path` (its subject's file). A decision whose `path` is
  absent or outside the workspace reads none. The file is read without following a link, at most 1 MiB;
  each directory from the workspace down to it is opened without following a link, and a directory
  reached through one reads no artifact. It is the request of the decision whose action its `unblocks.action` names (for a close, whose outcome
  `unblocks.outcome` names); any other decision does not take it. No file: the request is the one of
  before, byte for byte.
- **The check.** The artifact has a question and at least two options (the contract's `too-few-options`;
  an artifact of none is taken for its question alone where the declared results below give the
  options), each with a letter of its own, `does`, `cost` and `consequence`; an option of an action's
  approval whose `consequence.blocks` names the action has a `consequence.answer` (taken without one it
  would perform the action it blocks); its `subject_revision` is the decision's `revision` (Loom's revision of the subject:
  `t` and the first 16 hex digits of the SHA-256 of its bytes); a `recommended_option` is one of the
  letters; every `path_line` citation of its `grounds` names a file below the workspace (each component
  opened without following a link, so a citation below a link resolves nowhere), lines inside it,
  and, where it names a digest, the lines still have it (the digest `tools/decision-grounds.sh` writes).
  The other citation forms are not checked here; `tools/decision-grounds.sh` stays the full check of the
  producer. An artifact that fails ends the run (`runtime-failure`) by the cause
  `DecisionArtifactRefused: <file>: <what failed>`, and no request opens with its words.
- **The binding's question member.** `bindings/person-questions.json`, beside the bindings files
  (Loom's loader refuses a member of a binding it does not know), copied into the run's copy with them
  and read through the held directory: `{"format": "runbook.person-questions/1", "actions": {"<action>":
  "<member>"}}`. For a person action it names, the request's `question` is the text of that top-level
  member of the subject (read as JSON or YAML), and, where no artifact gives options, its `options` are
  the results the action declares (`shape::declared_results`), each `{letter: <result>, does, cost: "",
  consequence: {admits: [], blocks: [], answer: <result>}}`; the option `free` takes text and is answered
  `free: <text>` (its letter alone, or with nothing after the colon, names no option). A subject that
  cannot be read below the workspace, has no such member or an empty one leaves the generated question,
  the request opens, and the run event `runbook.question-field-unread` (record `case`, `action`, `cause`)
  says why. A member name in the file that is empty or not a text, and an action that declares no result
  (or whose declared results cannot be read), are recorded the same way, and the request is the one of
  before, byte for byte. An artifact with options wins, and the declared results are never added to it.
- **What an answer is.** The request carries `artifact` (`docs/daemon-api.md`, "A request read from its
  case"), and an answer names an option by its letter, alone or followed by a colon and text that goes
  with it as the reasons. The letter's effect is the option's (`parse_option`):

| Loom kind | the option's effect |
|---|---|
| `Approve` | approves when its `consequence.grant` is `granted`, denies otherwise |
| `ApproveAction` | the result `consequence.answer` where it has one, else `performed`; the text after the colon is the reasons |
| `Close` | closes through the outcome when it `produces: canon_decisions`, declines otherwise; no reasons |
| `Handoff` | the text was done, for every letter |

  Where `free_answer` is true (an artifact whose `unblocks.kind` is `question`) any other text is the
  reasons of an approval. The daemon refuses anything else before it is committed. A request that Loom
  refuses an answer of is asked again with the options and members of the first, not read again; an open
  request keeps what it opened with when the file changes. A rule or decider of the run answers through
  the same reading: its answer that names no option of a request read from its case (`approve`,
  `performed`) is not saved, and the request stays open for a person.

## Confinement

Passed unchanged, never lowered. The engine offers `local`, and `linux` when the process sits in a
cgroup it may delegate (`Engine::new`); anything else is refused before the store exists. The
`linux` level was not exercised by the tests (no delegated cgroup in the gate lane).

## Caps

The engine is made with a caps policy (`Engine::new(home, state_dir, caps)`): the daemon's three caps
and their ceilings. A run is driven one step at a time (`max_steps` of 1 for each `run_tree`), and
before each step `engine/gate.rs` holds the wall time, the model calls and the run's own spend against
their caps; the runner every bound program starts through, a step's and one an answer starts,
reserves, as the program starts, the per-call budget that program names (in its argv, else the
member `max_budget_usd` of its action or its bindings file; none: the largest the runbook's bindings
name; a runbook with such a program and no budget anywhere is refused at load) in
the ledger of the runbook (`<state dir>/ledgers/<runbook>.json`, entries and reservations keyed by the
run), against the run's own cap; after the step or answer the cost records are booked. A run at a cap ends `stopped: wall time`, `stopped: llm calls` or
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
run, whatever the store holds. The daemon then ends it as `orphaned`. A stop signal does not lead
there: the daemon drains first and exits when its runs have ended (`docs/daemon-api.md`, "Stopping the
daemon"); only a second signal, the drain bound, a kill or a crash orphans a live run. The store stays on disk;
`Engine::store_state(run)` tells `Absent`, `Idle`, `StaleHolder` or `Held` (Loom's lock is a `flock`,
so a dead holder lets go by itself; `tree/src/lock.rs:82`). Resuming an orphaned run from its store
is not built.

## The step in flight

The engine wraps the runner every bound program starts through in `engine::announce::Announcing`,
the wrapper `runbook run` uses too, when the sink gives a target (`EventSink::steps`, the daemon's
sink does). Before a program starts the target is told its case, action, role and try, and after it
ended it is told that; the daemon records the run event `runbook.action-started` and keeps the step
for the view `run` (`docs/daemon-api.md`, "The step in flight"). The gate tests are
`a_step_is_recorded_and_shown_while_its_program_runs_and_gone_when_it_ends`,
`actions_in_turn_give_one_event_each_in_the_order_they_started`,
`a_handoff_check_is_recorded_as_a_check_of_its_action` and
`after_a_restart_the_orphaned_run_shows_no_step_and_keeps_the_event` in `runbook/tests/daemon/engine.rs`.

## The products of a run

The run's `work/` directory is its workspace, `<state dir>/workspaces/<run id>/`: the bound programs of
every case of the run write there, and a stub run has one too. `runbook/src/engine/products.rs` is the
one reader of it: `list` walks it for the views `products` and `list_products`, `read` reads one file for
`product` and `read_product` (`docs/daemon-api.md`, "The products of a run"; `docs/mcp.md`). It runs in
the daemon's process and starts no program, so no name and no path reaches a shell.

It holds directories as descriptors (`record_write::Held`, the store of the run's record uses the same
walk): the workspace is opened without following a link at its own name, each directory below with
`O_NOFOLLOW | O_DIRECTORY` relative to the one above, and a file with `O_NOFOLLOW | O_NONBLOCK`, after
`fstatat` without following named a regular file with one link. The type and the link count are read
again from the open descriptor before a byte is read, so a file swapped for a link, a pipe or a second
name between the two is refused, and a pipe never blocks the read. A path is checked before anything
opens: not empty, not absolute, no NUL, no backslash, no `.` or `..` component, no component that starts
with a dot. A file is read up to 1 MiB and one more byte (to say `truncated`), never more.

The listing has the inspector's bounds (500 files, 2000 directories, 20000 entries, 6 levels; the first
reached is `cut`). It counts the names that start with a dot (`hidden`, never entered) and the links, the
files with a second name and the names that are not UTF-8 (`left_out`). The gate tests are
`runbook/tests/daemon/products.rs` (the views) and the `list_products` and `read_product` cases of
`runbook/tests/daemon/mcp.rs` (the tools).

## The case tree of a run

`Runtime::cases` (`runbook/src/runtime.rs`) answers the cases of a run's tree; the stub runtime has none (the default).
The engine answers it with Loom's tree status of the run's store, `calls::status` over `calls::catalog`
(`runbook/src/engine/calls.rs`), the call it makes at a run's end: Loom reads the manifest, the case files and the run
logs without taking the store's lock, so the tree answers while `run_tree` runs, and a run whose store has no manifest
yet has no case. A case gives its id, its parent, its link name, the protocol id of its bindings, its outcome and the
stop of its last `Stopped` run-log line; no path leaves the engine. The daemon makes the view `cases` from it
(`Daemon::cases_of_run`, `runbook/src/commands.rs`; `docs/daemon-api.md`, "The case tree of a run"): a waiting stop
counts while the run waits on a decision, a `Stuck` or `BudgetSpent` stop once the run ended by its tree (a wait or
a cancel that ended with the run, or an orphaned run, leaves the case `Open`: a case above one that waits records a
`Stuck` stop of its own), and the run's step in
flight marks its case `Running`. A read Loom refuses (a log line written at that moment) is the daemon's `503`; the
page keeps the tree it shows and reads again on the next tick. The gate tests are
`the_cases_of_a_run_are_its_tree_with_each_case_state_and_the_tree_follows_the_run_to_its_end`,
`a_case_whose_program_runs_is_running_while_the_run_goes_on` and
`a_tree_three_levels_deep_is_answered_to_its_last_case` (a middle protocol, `engine-section`, written by the test) in
`runbook/tests/daemon/engine.rs`, and `runbook/tests/daemon/cases.rs` (the stub's empty tree, the guard).

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

## Autonomy and a decider

A submit may carry `autonomy` (`A0` to `A5`) and `decider` (a kind name). The daemon fills what the
submit leaves out from `runbook serve --autonomy <level> --decider <kind>`; the submit wins. Both are
fixed when the run is submitted: the start event `runbook.submitted` records them, `get_run` and the
page show them as text, and after a restart they are read from the run's record, never from the
daemon's options of the day. A submit with neither, and a daemon with no default, is a run whose every
decision waits for a client, as before. `A0` is the same.

The kinds are the tables `[deciders.<kind>]` of `runbook.toml`: `rule` (`verdict = "approve"` or
`"reject"`, answered by `loom_run::decider::rules`) and `model` (the model decider of `runbook run`,
with its `model`, `adapter` and cap members). A kind that is not configured is refused at the submit,
before a run directory or a record exists, with the status 400 and a message that names the kind asked
for and the kinds configured, no others; so is a level with no kind to take it. A daemon whose own
default names a kind that is not configured does not start. A kind names a table of the file and never
a program or a path.

At each wait the engine asks the run's judge, in the order Loom printed the decisions, before it asks
a client. The judge is the decider of `runbook run`: `loom_run::arbiter::DaemonArbiter` implements
`engine::arbiter::{Arbiter, RunArbiter}` over `NativeDecider::decide` (the level, the table of who
decides in the runbook's `tools/autonomy-levels.json`, the rule or the model call) and
`bindings_copy::materialise` (the run's copy of the bindings, in which the person steps the level gives
to the decider name the decider's principal). The result a person action is answered with, and the
text of a rejection, come from `engine::shape::{person_result, clean_reasons, declared_results}`,
which `loom_run::run_loop` calls too. What differs is where the answer goes: `runbook run` hands it to
its host, the engine applies it with `calls::apply`, the call a client's answer takes.

A decision the level leaves to a person stays open for `answer_decision`: so does one the decider
cannot decide (a model decider with no instructions or no claims, a cap that is spent), a table of who
decides that cannot be read (fail closed: every decision waits), and a verdict Loom would refuse. At
`A5` only a close by rule is taken; below it only a class the table gives to `decider`.

Each answer records who gave it. A rule or the decider answers as `rule:<name>` or `decider:<kind>`,
in `answered_by` and in `principal`, never as a person and never as the daemon's principal; a client's
answer is under the label it typed and the daemon's principal. One function reads a client label: it
trims ASCII whitespace and refuses (400, naming the field and printing no byte of the label) a label
with a non-ASCII or a control character, and one whose lower case starts with `rule:`, `decider:` or
`router:`, so a client cannot sign as a rule, a decider or a router in any spelling.

The environment of a decider program is its own `PATH` and nothing of the daemon's. A `model` decider
needs the login the model's adapter reads: `login_file` of `runbook.toml`, a path, never content. The
daemon, like `runbook run`, makes a link to it in the private directory of each run
(`$XDG_RUNTIME_DIR/runbook/daemon-<process>-<run>/login`, mode 0700) when the run's kind is `model`, and
the adapter is given the link, never the file's own path; the daemon refuses to start that run when
`XDG_RUNTIME_DIR` is not set. The link and its directory go when the run ends (one case holds it, with
a stand-in adapter; the model call itself is not exercised by the tests: no model call here). `rule`
works with nothing of the sort. Money: the
caps are optional (`cap_micro_usd`, `max_llm_calls` of the file); a rule costs nothing and needs
none; when a cap is named and spent, the decider stops and the decision stays open.

### A gateway

The daemon's operator may name a gateway instead of a login: the `[gateway]` table of the `runbook.toml` the
daemon reads, with `base_url` and, for a gateway that takes a token, `token_file` (the file holds no token:
`gateway.token` there is refused, `docs/loom-run-settings.md` "`[gateway]`"). It is the one route `default` of
the runs that may take a route (decider kind `model` above `A0`, or a runbook that runs a program: `docs/daemon-api.md`, "The route of a run"), whose alias is the `model` of `[deciders.model]`,
until the daemon declares routes of its own (`docs/model-routes.md`, sections 1 and 11). Every model call of
such a run goes through the daemon's relay, never to the gateway (`runbook/src/relay.rs`,
`runbook/src/engine/route.rs`):

- The relay answers on the daemon's own listener, at `/daemon/relay/<run>/v1/messages`,
  `/v1/chat/completions` and `/v1/responses` ("The relay of a run's model calls", `docs/daemon-api.md`), when
  that listener is loopback. On any other (every interface included) the relay is not served, the daemon says so
  on stderr as it starts, and a run on a route fails to start by that name.
- When the run starts, the daemon mints a token for it (32 random bytes), void when the run ends, and writes
  `route.json` (`runbook.route/1`, mode 0600: the relay's address for the run, the token, the roles, the model a
  client sends for each (`runbook-<role>`) and the
  route of each) into the run's private directory (`$XDG_RUNTIME_DIR/runbook/daemon-<process>-<run>/`, mode
  0700). Its path is `RUNBOOK_ROUTE_FILE` for the decider's adapter and for every bound program of the run;
  nothing else of the route is given to a process. The file and its directory go when the run ends.
- The claude-cli adapter reads the file as it reads a gateway file: `claude` gets `ANTHROPIC_BASE_URL` (the
  relay) and `ANTHROPIC_AUTH_TOKEN` (the run's token) for that one start, and asks for the run's role (the
  request's model when the file names a route for it, else `default`); the relay sets the alias and adds the
  gateway's token. No process of the run gets the gateway's address or token, and no login is linked or copied.
- Each call is taken against the run's caps before it is sent: a role the run has no route for is refused
  `400 route-refused`, a call past `max_llm_calls` or past the spend cap `429 cap-reached`, and neither reaches
  the gateway. Each call that was sent is one line of the run's spend ledger (`what` `model call`; run, step,
  role, route, alias, the setting that chose the route as `source`, the gateway's `x-llm-gateway-call` as
  `gateway_call`, `input_tokens`, `output_tokens`, `cost_known`, `micro_usd`) and one event
  `runbook.model-call` of the run. With no price on the route the cost is not known, and the line books the
  call's reservation with `cost_known: false` (`docs/model-routes.md`, section 5): in a step, the largest
  reservation the step holds; otherwise (a call of the decider, or a step that holds none) what one call of the
  decider may cost (`max_call_micro_usd`, its `max_budget_usd`), at most what the run's spend cap has left.
- A call is exchanged with the gateway and booked apart from its caller: a caller that hangs up before the
  answer does not stop a call that was sent from being booked, once. An answer that breaks off after its head
  ran on the gateway: it is booked at its tokens and prices as read, else at its reservation.
- A gateway that refuses the call, or does not answer, fails the call: the refusal is passed back unchanged
  (a connection that fails answers `502` `route-unavailable`), and the line and the event carry a `cause`, the
  route, the gateway's `code` (`connection-failed` for no answer, `answer-broken` for an answer that broke off),
  the `detail` and the `source`, and, when the answer gave one, the gateway's error text as `error`
  (`<type or code>: <message>` of the answer's JSON `error`, at most 300 bytes, the gateway's token replaced by
  `<token>`). The `cause` is `call-refused` for a `4xx` answer that is no refusal of the route by the gateway,
  else `route-unavailable`. The `code` is cut to 64 bytes and holds no token either. An answer with a status below
  400 whose stream ends in an error event (`event: error` with `overloaded_error`, a chat chunk with an `error`,
  `response.failed`) failed too: `route-unavailable`, the event's type or code as `code`, `detail` "the answer
  ended in an error event". A call the gateway refused or did not answer costs 0. The relay never sends a call to
  another route.
- A failed call in a step does not end the run by itself: a `4xx` the client may recover from, a `5xx` answer
  (`upstream-failed` for an upstream that timed out), a connection that fails and an answer that broke off are
  passed to the client, which may retry, as Claude Code does. The run ends as the step ends: a step whose program
  exits 0 with its products goes on, and the run's record names the failed calls with one event
  `runbook.model-calls-failed` (the step, and per call the route, the `gateway_call`, the `cause`, the `code`, the
  `detail` and the `error`), one event per step: a call left in flight by an earlier step and booked while a later
  one runs is named under its own step, and one that ends after the last step was booked is named when the run's
  calls are closed; a step whose program failed ends the run with its own first failed call as its first cause, `<cause>: route <route> (chosen by <source>) failed the call: <code>: <detail>` (`refused the call` for
  `call-refused`) (story `a-recovered-call-does-not-fail-the-run`).
- What stops the run at once: the gateway's refusals of the route (`model-unknown`, `wire-not-served`,
  `credential-absent`, `credential-malformed`, `credential-rejected`) and its `spend-ceiling` fail the step they
  came in with `route-unavailable`, the route, the code and the `source`, whatever its program does after; a call
  past the run's caps is refused by the relay and the caps stop the run before the next step. A call of the decider
  that fails ends the decider's call as any failed call does: the decision stays open for a person.
- For a run on a route the relay's line is the call's cost: the cost records of its bound programs book
  nothing, and one with no `cost_usd` does not end the run (`runbook/src/cost.rs`). A line the ledger cannot
  write fails the step the call ran in (fail closed). The spend cap is held against what was booked when a
  call is taken: calls in flight at once may each pass it, by at most their own cost.
- When the run ends its token is void at once, and the calls still in flight are waited for up to 5 seconds
  and recorded. A call that outlasts that is booked in the ledger when it ends and said on the daemon's stderr;
  the run's record says how many were left with one event `runbook.model-calls-unfinished`.

A run whose kind is `rule`, or that has no kind, is on no route and writes no route file.

The daemon also declares named model routes, several gateways and routes side by side (`docs/model-routes.md`, section 1), and lists them in the view `routes` (`docs/daemon-api.md`, "Model routes"). A run whose kind is `model` takes one of them (`docs/daemon-api.md`, "The route of a run"): the route its submit names in `model_route` (and per role in `model_routes`), else `[route_policy] default`; the relay's table of the run is the role `default` on that route plus each named role on its own, and every call is booked with the route, its alias and the setting that chose it. A daemon that declares no route but a lone `[gateway]` (the gateway `default`) and a model decider declares the interim route `default` on that gateway (kind hosted, the decider's model as its alias, no price) as its default: the route above, now listed by the view `routes` too.

The token file is read once, when the daemon starts, and only when the `model` kind is configured. The daemon
does not start (exit 2, naming the member and never a value or a path) when the token file cannot be read as
one line of a token, when the gateway's shape is refused, and when the `model` kind's adapter is
`metaharness`, its default, which cannot take a gateway: name `adapter = "claude-cli"` in
`[deciders.model]`. A gateway and a `login_file` in one file are refused. A model run under a gateway without
`XDG_RUNTIME_DIR` fails to start by that name. The relay speaks `http://` to the gateway only: a gateway at an
`https://` address fails every call `route-unavailable`.

A submit names no gateway, address or token: a setting, or a member of the request, named `gateway`,
`base_url`, `token`, `token_file` or `gateway.<member>`, in any case, is refused (400) by its name and never
with its value, and no run is made. The cases are `runbook/tests/daemon/gateway.rs` and
`runbook/tests/daemon/relay_daemon.rs`: a stand-in gateway on a loopback port receives the configured token
from the relay, with the alias as the model, for a stand-in `claude` started by the committed claude-cli
adapter; no process of the run, no file of the daemon's state and no event of the run holds it.
`runbook/tests/daemon/relay.rs` holds the relay's own cases: the answer passed back unchanged, the usage of a
JSON answer and of a stream, a price, the refusals and a role on another route that is not called.

## Not built

Resume of an orphaned run from its store; a stream of decisions to a person while `run_tree` runs
(decisions are read at each wait); the `linux` level under test; the spend ledger of the daemon (the
caps of one run are held in the run's judge, not shared between runs); a model call under test.
