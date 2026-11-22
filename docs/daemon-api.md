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
  is left for an Open request whose run does not wait (a state the daemon's own commands never
  leave, so a client does not meet it).
- `cancel-run` and the daemon's own end of an orphaned run withdraw every Open decision request of
  the run in the same step: the requests are `Withdrawn` when the answer comes (view `decisions`,
  state `Withdrawn`; the view `open-decisions` no longer lists them), and an answer to one is
  refused `DecisionWithdrawn`. No `withdraw-decision` follows, and the cancel's answer publishes
  `RunCancelled` only, no event per request. `withdraw-decision` stays for the runtime's own
  withdrawal of an Open request of a run that goes on; after a cancel it answers `not-open`.
- The first event of every run is written by the daemon at the submit: kind `runbook.submitted`,
  `sequence` 1, `position` the next of the daemon. The next sequence of the run starts at 2, so the
  first event the runtime records for a run has `sequence` 2. A repeated submit (`already-submitted`)
  and a refused submit write no event.

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
The `sequence` of an event is only its place within its run.

An event's `kind` is open text, the `kind` of the runtime's run-log line; events the daemon writes
itself have a kind that starts with `runbook.`: `runbook.submitted` (the run's first event, sequence
1, at the submit), `runbook.cancel-requested` (at a cancel), `runbook.start-failed`,
`runbook.run-orphaned`, `runbook.answer-undeliverable` (an answer the runtime could not take up) and
`runbook.runtime-failed` (before the run ends with the outcome `runtime-failure`). Each is declared
in the specification by the command that writes it, in its summary: the language cannot write a
run event from a command, so no row of the interpreted target stands for them.

### Event streams

Sent with `Accept: text/event-stream`, the same path is a stream of server-sent events. There is no
other route.

```
curl -N -H 'Accept: text/event-stream' 'http://127.0.0.1:7430/daemon/views/daemon-events?after_position=0'
curl -N -H 'Accept: text/event-stream' -H 'Last-Event-ID: 41' 'http://127.0.0.1:7430/daemon/views/events?run_id=<run>'
```

- Message. `id:` is the event's `position`, `event:` its `kind`, `data:` the row the JSON page holds
  for it, as one line. A stream sends every recorded event after the asked position in position
  order, then stays open and sends each new event as it is recorded. An event is read from the
  daemon's record, so none is sent twice and none is skipped, also when it is recorded while the
  stream connects.
- Where it starts. The `Last-Event-ID` header, when given, is the position the stream continues after.
  Without it the parameter `after_position` is that position, and without both the stream starts at
  0, the whole record. When both are given the header wins and the parameter is not read. The id
  parameter of the view (`run_id`, `session_id`) is required as in the page form (`400`). A
  `Last-Event-ID` that is not a position (digits only, at most 18,446,744,073,709,551,615), or that
  is given twice, is refused `400` with the body `{"refused": "bad request", "message": ...}`, as
  is an `after_position` that is not a position when it is the one in use. A position beyond the last
  recorded one is not an error: the stream waits and sends the events whose position is above it.
- Unknown ids. The page form of `events` and `session-events` answers `200` with no row for an id
  the daemon does not know; so does the stream: it answers `200`, has nothing to send and ends at once.
- Where it ends. The stream of a run ends (the daemon closes it) when the run is `Ended`,
  `Cancelled` or `FailedToStart` and its last event has been sent. The stream of a session ends when
  the session is closed, every run of the session is terminal and the last event has been sent. The
  stream of the daemon ends at shutdown only. On SIGINT or SIGTERM every open stream is closed and
  the daemon exits within 2 s, also when a client does not read. A client that gets the end of a
  stream before it expected one reconnects with `Last-Event-ID`.
- Keep-alive. A comment line, `: keep-alive`, is sent every 15 s on a stream that has nothing else to
  send, so that a proxy does not close it; a client ignores it.
- Slow clients. Recording an event never waits for a stream: a stream reads the record when its
  connection takes data, and a client that reads slowly holds back its own stream and nothing else.
  A live stream that finds more than 1,024 events waiting for it (more than it has sent, not counting
  what was recorded before it connected) is closed, after a comment line that says where to resume:
  the client reconnects with `Last-Event-ID` set to the last id it read and gets the rest.
- Token. A stream is a route like any other: with a token, a request without
  `Authorization: Bearer <token>` is refused `401` before any stream starts.

## Running the daemon

```
runbook serve [--listen ADDR] [--state-dir DIR] [--context-root DIR]... [--runbook-dir DIR]
              [--min-confinement local|linux] [--token-file FILE]
```

In this version the runtime behind the daemon is a stub: a run is scripted, it asks one question
and ends, and nothing is executed. The daemon says so on standard error when it starts. It prints
`runbook: listening on http://127.0.0.1:<port>` on standard output and runs until SIGINT or SIGTERM;
open event streams are closed then (see "Event streams").

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
`404` for an unknown route, `405` for the wrong method, `413`.

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
`outcome` of the `run` view), its Open decision requests are withdrawn in the same step, as a cancel
does, the event `RunOrphaned` is published and the run's event stream gets one event of kind
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
failure cannot hide behind them. Now: 12 scenarios of the daemon (3 more are of `sdc.decision`, out of
the daemon's domain), 10 executed, 10 passed, 0 failed, 2 skipped (an exact instant each), and
`KNOWN_FAILURES` is empty. Before part C4 it was 4 passed and 6 failed: two differences between the
specification and the daemon, which the specification now states (the daemon's first event of a run,
and the withdrawal of a run's open decision requests by `CancelRun` and `EndOrphanedRun`; see
"Client commands"). The same suite passes on the interpreted target of ESS
(`ess verify conform run --target interpreted --path ess --suite ess/generated/conformance/authored.json
--report-format 2`): 15 of 15 scenarios, as before.

The synthesized suite (`ess verify conform synthesize`, `task ess-suite:record`) is not committed:
ess 0.97.1 took 1048 s on the specification of the first daemon commit and did not finish in 1800 s on
the current one, a known defect of that release. `CONFORMANCE_SUITE=<file> CONFORMANCE_REPORT=<file>
cargo test conformance` runs any suite document, a synthesized one included.

## Outside the language

- SSE. ESS 0.96.0 has no construct for a server-sent event stream; the three views above are the
  page a stream serves, and the OpenAPI document lists them as plain `GET` views with a JSON
  response. The `text/event-stream` binding is written above and tested in the story
  `daemon-skeleton-on-a-runtime-trait` (part C2).
- The host checks of `submit-run`: that the runbook exists and that the context path lies under a
  context root are decided by the handler from the host. The specification takes them as facts
  (`runbook_known`, `context_inside_roots`) so that the refusals are specified and
  scenario-tested; the checks themselves are tested in the same story. The intake size limit is in
  the specification as a character count, the weaker bound; the handler holds the limit in bytes
  (1,048,576 bytes of UTF-8).
