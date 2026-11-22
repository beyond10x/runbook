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
| Run | Submitted, Running, WaitingOnDecision, Ended, Cancelled, FailedToStart | One run of a runbook. `Ended` carries an `outcome` (open text), `FailedToStart` a `failure`. |
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
| `submit-run` | `NoSuchRunbook`, `IntakeTooLarge` (over 1 MiB), `ContextOutsideRoots`, `NoSuchDaemon`, `BelowConfinementFloor`, `NoSuchSession`, `SessionClosedToRuns` |
| `cancel-run` | `NoSuchRun`, `RunAlreadyEnded` |
| `answer-decision` | `NoSuchDecisionRequest`, `DecisionNotOfRun`, `DecisionWithdrawn`, `NoSuchRun`, `RunNotWaiting`, `DecisionAlreadyAnswered` |

Notes for a caller:

- `submit-run` takes the intake inline (text), the runbook by name with an optional version, the
  context as an absolute path under one of the daemon's context roots, and the settings as a map
  from text to text. The daemon mints the run id (a UUID, version 7) and returns it; a client never
  chooses it.
- A client that wants a safe retry sends an `idempotency_key` (at most 200 characters, unique per
  daemon). A second submit with a key already used creates no run and answers the first run
  (outcome `already-submitted`).
- The first answer to a decision request stands; a second is refused `DecisionAlreadyAnswered`.

The runtime's reports (`StartRun`, `FailRunStart`, `WaitOnDecision`, `RequestDecision`,
`WithdrawDecision`, `ResumeRun`, `RecordRunEvent`, `EndRun`) and `StartDaemon` are issued inside the
process. They are in the specification and not in the HTTP contract.

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
itself have a kind that starts with `runbook.` (`runbook.submitted`, `runbook.cancel-requested`,
`runbook.start-failed`).

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
`runbook.start-failed`); a run that was `Running` or `WaitingOnDecision` stays so and gets the event
`runbook.orphaned`, because the stub runtime cannot resume it.

## Outside the language

- SSE. ESS 0.96.0 has no construct for a server-sent event stream; the three views above are the
  page a stream serves, and the OpenAPI document lists them as plain `GET` views with a JSON
  response. The `text/event-stream` binding is written above and tested in the story
  `daemon-skeleton-on-a-runtime-trait` (part C2).
- The host checks of `submit-run`: that the runbook exists and that the context path lies under a
  context root are decided by the handler from the host. The specification takes them as facts
  (`runbook_known`, `context_inside_roots`) so that the refusals are specified and
  scenario-tested; the checks themselves are tested in the same story. The intake size limit is in
  the specification (it counts characters; the handler holds the byte limit).
