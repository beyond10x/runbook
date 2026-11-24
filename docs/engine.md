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

## Not built

Resume of an orphaned run from its store; a stream of decisions to a person while `run_tree` runs
(decisions are read at each wait); the `linux` level under test; the spend ledger of the daemon (the
caps of one run are held in the run's judge, not shared between runs); a model call under test.
