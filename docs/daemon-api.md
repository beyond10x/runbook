# The daemon API

For a person who will call `runbookd` (the same binary as `runbook serve`). The API is specified in
ESS before any handler exists: the domain is `ess/domains/daemon.yaml`, and the HTTP contract is
generated from it, never written by hand: [`ess/generated/openapi/openapi/runbookd.yaml`](../ess/generated/openapi/openapi/runbookd.yaml)
(JSON Schemas of every type: `ess/generated/schema/`). `task check:ess-generated` holds the two
equal. Where this page and the specification differ, the specification wins.

Who may call is decided by the bearer token alone (a non-loopback listener needs one). A session's
`client` is a label the caller gives for display; it grants nothing.

## Entities and their states

| Entity | States | What it is |
|---|---|---|
| Run | Submitted, Running, WaitingOnDecision, Ended, Cancelled, FailedToStart | One run of a runbook. `Ended` carries an `outcome` (open text: the protocol's outcome name, or a cause of the daemon's own, `orphaned` or `runtime-failure`), `FailedToStart` a `failure`. |
| Session | Open, Closed | A named group of runs. A closed session takes no new run and keeps the runs it has. |
| DecisionRequest | Open, Answered, Withdrawn | A question a waiting run raised, mirrored from the runtime. Cancelling the run withdraws its Open requests. |
| RunEvent | Recorded | One record of a run, with `sequence` (per run, from 1) and `position` (per daemon, from 1, over all runs). |
| Daemon | Up | What the daemon was started with: its confinement floor. |

Confinement levels, weakest first: `local` (not confined), `linux`. The default of a submit is
`linux`, the daemon's default floor is `linux`; `local` must be asked for by name and is refused
under a `linux` floor.

## Client commands

Every command is a `POST` to `/daemon/commands/<name>` with a JSON body.

| Command | Refusals |
|---|---|
| `open-session` | `SessionExists` |
| `close-session` | `NoSuchSession`, `SessionStateConflict` (already Closed) |
| `submit-run` | `NoSuchRunbook`, `IntakeTooLarge` (over 1,048,576 bytes), `ContextOutsideRoots`, `NoSuchDaemon`, `BelowConfinementFloor`, `NoSuchSession`, `SessionClosedToRuns` |
| `cancel-run` | `NoSuchRun`, `RunAlreadyEnded` |
| `answer-decision` | `NoSuchDecisionRequest`, `DecisionNotOfRun`, `DecisionWithdrawn`, `NoSuchRun`, `RunNotWaiting`, `DecisionAlreadyAnswered` |

Notes for a caller:

- `submit-run` takes the intake inline (text), the runbook by name with an optional version, the
  context as an absolute path under one of the daemon's context roots, and the settings as a map
  from text to text. The daemon mints the run id (a UUID, version 7) and returns it; a client never
  chooses it.
- A client that wants a safe retry sends an `idempotency_key` (at most 200 characters, unique per
  daemon). A second submit with a key already used creates no run and answers the first run: the
  outcome is `already-submitted`, status 202 (not a refusal: a retry after a lost answer gets the
  answer it lost). The first run's id is data in the body, the `run_id` of the event it publishes
  (`published[0].payload.run_id`, event `SubmitReplayed`), and the `Location` header names the run
  too (`/daemon/views/run?run_id=<id>`, the same header a first submit has).
- The intake limit is 1,048,576 bytes of UTF-8, counted by the daemon's handler. The specification
  counts characters (`intake.count`), which is the weaker bound: a text of at most 1,048,576
  characters can still be over the limit in bytes, and the handler refuses it `IntakeTooLarge`.
- The first answer to a decision request stands. The request's state is judged before its run's:
  a second answer is refused `DecisionAlreadyAnswered` whether its run still waits, has resumed or
  has ended; an answer to a request withdrawn by a cancel is `DecisionWithdrawn`; `RunNotWaiting`
  is left for an Open request whose run does not wait.

The runtime's reports (`StartRun`, `FailRunStart`, `WaitOnDecision`, `RequestDecision`,
`WithdrawDecision`, `ResumeRun`, `RecordRunEvent`, `EndRun`), `StartDaemon` and `EndOrphanedRun` (the
daemon's own, actor `DaemonProcess`) are issued inside the process. They are in the specification
and not in the HTTP contract: the generator gives a route to the commands of the actor `Client` and
to no other.

## Views

Every view is a `GET` to `/daemon/views/<name>`, its parameters in the query.

| View | Parameters | Rows |
|---|---|---|
| `runs` | none | every run, newest first |
| `runs-of-session` | `session_id` | the runs of a session |
| `runs-in-state` | `state` | the runs in a state |
| `runs-of-session-in-state` | `session_id`, `state` | both filters |
| `run` | `run_id` | one run with everything it was submitted with |
| `open-decisions` | `run_id` | the run's Open decision requests (not the Withdrawn ones) |
| `decisions` | `run_id` | every decision request of the run, with its state |
| `sessions` | none | every session |
| `configuration` | none | the daemon's configuration |

One run with its open decision requests is two reads, `run` and `open-decisions`: a view has one
source entity, so the language cannot join them in one row.

## Event streams

Three views page the events by `position`:

| View | Parameters | Events |
|---|---|---|
| `events` | `run_id`, `after_position` | one run |
| `session-events` | `session_id`, `after_position` | every run of a session, interleaved in position order |
| `daemon-events` | `after_position` | every run of the daemon |

Each answers the rows with a position above `after_position`, in position order, as a JSON page.
Sent with `Accept: text/event-stream`, the same path streams them as server-sent events: every
row with a position above `after_position`, then each new event as it is recorded. The SSE `id:` of
an event is its `position` on all three streams. A client that lost the connection reconnects with
`Last-Event-ID: n` (a position) and the stream continues after `n`; `after_position = 0` is
everything. The `sequence` of an event is only its place within its run.

An event's `kind` is open text, the `kind` of the runtime's run-log line; events the daemon writes
itself have a kind that starts with `runbook.` (`runbook.submitted`, `runbook.cancel-requested`,
`runbook.start-failed`, `runbook.run-orphaned`).

## Running the daemon

```
runbook serve [--listen ADDR] [--state-dir DIR] [--context-root DIR]... [--runbook-dir DIR]
              [--min-confinement local|linux] [--token-file FILE]
```

In this version the runtime behind the daemon is a stub: a run is scripted, it asks one question
and ends, and nothing is executed. The daemon says so on standard error when it starts. It prints
`runbook: listening on http://127.0.0.1:<port>` on standard output and runs until SIGINT or SIGTERM.
Event streams (`Accept: text/event-stream`) are answered `406` with a body that names the next part
of the story; the three event views are JSON pages only.

| Option | Default | Meaning |
|---|---|---|
| `--listen` | `127.0.0.1:7430` | Address to bind. Port 0 picks a free port; the line above names it. |
| `--state-dir` | `$XDG_STATE_HOME/runbook`, else `~/.local/state/runbook` | Where the daemon keeps its record. |
| `--context-root` | none | A directory a run's `context_path` may lie under; repeatable. With none, any `context_path` is refused. |
| `--runbook-dir` | `protocol/` of the located checkout or install | Where a submit may find a runbook besides `sdc` and `sdc-bugfix`. |
| `--min-confinement` | `linux` | The weakest level a submit may ask for; a weaker one is refused `BelowConfinementFloor`. The level is stored with the run; nothing is confined yet. |
| `--token-file` | none | File that holds the bearer token. |

Token. With a token, every route, an unknown one included, answers `401` unless the request has
`Authorization: Bearer <token>`. The token comes from `--token-file` or from the environment value
`RUNBOOK_TOKEN`, the file first. A trailing newline is dropped. The daemon compares in constant
time, and it never logs or echoes the token. A token file that is readable by others (any bit of
`o+rwx`) is refused at start, exit 1, with the mode in the message: `chmod 600` it. Without a token
the daemon listens on loopback only: a listen address that is not loopback and no token exit 1
before anything is bound or written.

Limits. A body over 6 MiB + 64 KiB is refused `413` before it is read whole (the intake is at most
1,048,576 bytes; JSON writes a control character as six). Answers outside the specification have the
body `{"refused": <what>, "message": <why>}`: `400` for a body or query the daemon cannot read, `401`,
`404` for an unknown route, `405` for the wrong method, `406`, `413`.

State directory:

| Path | Holds |
|---|---|
| `daemon.lock` | Held while a daemon runs on the directory; a second daemon exits 1 with "in use by another daemon". |
| `journal.jsonl` | Every change of state as one JSON line, appended and flushed; replayed at start. It is the truth; a run killed mid-way is read back in its last state. |
| `runs/<run_id>/` | The run's own record, written by the runtime. |
| `workspaces/<run_id>/` | The run's workspace; a `context_path` is copied here, symbolic links are not followed. |

After a restart a run that was `Submitted` and never started becomes `FailedToStart` (event
`runbook.start-failed`). For each run the record holds as `Running` or `WaitingOnDecision` the daemon
asks the runtime (`Runtime::recover`): `resumed` leaves the run as it is; `gone` ends it with the
command `EndOrphanedRun`. The run is then `Ended` with the outcome `orphaned` (the cause, readable in
`outcome` of the `run` view), its Open decision requests are withdrawn one by one as a cancel does,
the event `RunOrphaned` is published and the run's event stream gets one event of kind
`runbook.run-orphaned`. A run that has ended is not asked about again, so a second start adds
nothing. The stub runtime always answers `gone`.

## Conformance

The scenarios of `ess/scenarios/daemon-*.yaml` are compiled to a suite document
(`ess/generated/conformance/authored.json`, written by `task ess-suite:authored`, compared byte for
byte by `task check:ess-suite`, which `task check:ess-generated` runs) and run against the real daemon
over HTTP by `runbook/tests/daemon/conformance.rs`, in the crate's one test binary. The test only
reads the committed file. It never synthesizes.

What the test does: the five client commands are `POST` calls and every view a `GET` over a real socket;
the runtime-side commands and the daemon's own `EndOrphanedRun`, which are not on the network, are the
calls of the command layer that a runtime's reports map to, made by a runtime double that reports
nothing on its own. The host facts of `submit-run` are made true or false in the host. What it cannot
ask is `skipped` with the reason, never `passed`: a command as an actor the daemon does not know (it
authenticates none), and a claim of an exact instant (the daemon stamps its own clock and takes no
`at`; the scenario still runs and every other claim is read).

The numbers are committed in the test (`GATE`: the scenarios, the executed, the passed, the skipped) and
may not move the wrong way; the failures are named in `KNOWN_FAILURES` with their reasons, so a different
failure cannot hide behind them. At the commit that made this section: 12 scenarios of the daemon (3 more
are of `sdc.decision`, out of the daemon's domain), 10 executed, 4 passed, 6 failed, 2 skipped. The six
failures are two differences between the specification and the daemon, both listed in the test: the
daemon writes its own `runbook.submitted` event at sequence 1 of a run, and `CancelRun` and
`EndOrphanedRun` withdraw the run's open decision requests themselves.

The synthesized suite (`ess verify conform synthesize`, `task ess-suite:record`) is not committed:
ess 0.97.1 took 1048 s on the specification of the first daemon commit and did not finish in 1800 s on
the current one, a known defect of that release. `CONFORMANCE_SUITE=<file> CONFORMANCE_REPORT=<file>
cargo test conformance` runs any suite document, a synthesized one included.

## Outside the language

- SSE. ESS 0.96.0 has no construct for a server-sent event stream; the three views above are the
  page a stream serves, and the OpenAPI document lists them as plain `GET` views with a JSON
  response. The `text/event-stream` binding is written and tested in the story
  `daemon-skeleton-on-a-runtime-trait`.
- The host checks of `submit-run`: that the runbook exists and that the context path lies under a
  context root are decided by the handler from the host. The specification takes them as facts
  (`runbook_known`, `context_inside_roots`) so that the refusals are specified and
  scenario-tested; the checks themselves are tested in the same story. The intake size limit is in
  the specification as a character count, the weaker bound; the handler holds the limit in bytes
  (1,048,576 bytes of UTF-8).
