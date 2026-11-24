# The daemon API

For a person who will call `runbookd` (the same binary as `runbook serve`). The API is specified in
ESS before any handler exists: the domain is `ess/domains/daemon.yaml`, and the HTTP contract is
generated from it, never written by hand: [`ess/generated/openapi/openapi/runbookd.yaml`](../ess/generated/openapi/openapi/runbookd.yaml)
(JSON Schemas of every type: `ess/generated/schema/`). `task check:ess-generated` holds the two
equal. Where this page and the specification differ, the specification wins.

Who may call is decided by the bearer token alone (a non-loopback listener needs one). A session's
`client` is a label the caller gives for display; it grants nothing. The principal of an answer is not
what a caller says: it is the daemon's own, given at its start (`--principal`) and fixed for the
process. `answered_by` stays a label of at most 200 characters, stored and shown, never passed to the
runtime.

## Entities and their states

| Entity | States | What it is |
|---|---|---|
| Run | Submitted, Running, WaitingOnDecision, Ended, Cancelled, FailedToStart | One run of a runbook. `Ended` carries an `outcome` (open text: the protocol's outcome name, or a cause of the daemon's own, `orphaned` or `runtime-failure`), `FailedToStart` a `failure`. |
| Session | Open, Closed | A named group of runs. A closed session takes no new run and keeps the runs it has. |
| DecisionRequest | Open, Answered, Withdrawn | A question a waiting run raised, mirrored from the runtime. Cancelling the run withdraws its Open requests. |
| RunEvent | Recorded | One record of a run, with `sequence` (per run, from 1) and `position` (per daemon, from 1, over all runs). |
| Daemon | Up | What the daemon was started with: its confinement floor and its principal. |

Confinement levels, weakest first: `local` (not confined), `linux`. The default of a submit is
`linux`, the daemon's default floor is `linux`; `local` must be asked for by name and is refused
under a `linux` floor.

## Client commands

Every command is a `POST` to `/daemon/commands/<name>` with a JSON body.

| Command | Refusals |
|---|---|
| `open-session` | `SessionExists` |
| `close-session` | `NoSuchSession`, `SessionStateConflict` (already Closed) |
| `submit-run` | `IntakeTooLarge` (over 1,048,576 bytes), `NoSuchRunbook`, `ContextOutsideRoots`, `NoSuchDaemon`, `NoSuchSession`, `BelowConfinementFloor`, `SessionClosedToRuns` |
| `cancel-run` | `NoSuchRun`, `RunAlreadyEnded` |
| `answer-decision` | `NoSuchDecisionRequest`, `DecisionNotOfRun`, `DecisionWithdrawn`, `DecisionAlreadyAnswered`, `NoSuchRun`, `RunNotWaiting` |

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
  too (`/daemon/views/run?run_id=<id>`, the same header a first submit has). A key over the limit is
  not an outcome of `submit-run`: it is refused `400` before every outcome, `IntakeTooLarge` included.
- The intake limit is 1,048,576 bytes of UTF-8, counted by the daemon's handler. The specification
  counts characters (`intake.count`), which is the weaker bound: a text of at most 1,048,576
  characters can still be over the limit in bytes, and the handler refuses it `IntakeTooLarge`.
- The first answer to a decision request stands. The request's state is judged before its run's:
  a second answer is refused `DecisionAlreadyAnswered` whether its run still waits, has resumed or
  has ended; an answer to a request withdrawn by a cancel is `DecisionWithdrawn`; `RunNotWaiting`
  is left for an Open request whose run does not wait (a state the daemon's own commands never
  leave, so a client does not meet it).
- An answer carries two names. `answered_by` is the label the client typed. `principal` is the daemon's
  own (`--principal`), the only one the runtime is given. Both are in the `DecisionAnswered` event the
  answer publishes (`published[0].payload`), in the row of the view `decisions` (and so in the answer
  of the MCP tool) and both stay in the journal; the view `configuration` names the principal. A
  `principal` member sent in the body of `answer-decision` is not read, and the MCP tool refuses it as
  an unknown argument. A journal written before the daemon had a principal still loads: the `principal`
  of such an answer is absent in the view.
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
| `open-decisions` | `run_id` | the run's Open decision requests (not the Withdrawn ones), each with the `case_id`, the `action` and the `capability` of the step that asks |
| `decisions` | `run_id` | every decision request of the run, with its state |
| `sessions` | none | every session |
| `configuration` | none | the daemon's configuration: its id, confinement floor, principal and start time |
| `runbooks` | none | the runbooks a `submit-run` of this daemon accepts, by `runbook_id`, `runbook_version` (absent when the host names none) and `runtime` (`stub` or `loom`), in order of name |

The view `runbooks` is the list a `submit-run` is checked against: every row names a runbook the
command accepts (`no-runbook` is its refusal for any other name). Under `--runtime loom` the rows are
the runbooks of `--runbook-home` that have a `protocol/<name>.protocol.yaml` and a
`bindings/<name>.yaml`, the version being the revision of the root protocol. Under the stub runtime
they are its scripts (the presets and `stub-decision`) and the entries of `--runbook-dir`: a
directory `<name>/` or a file `<name>.protocol.yaml` with a plain name (letters, digits, `-`, `_`,
`.`, no leading dot). A name is listed once. Under the stub runtime the view lists every entry of
`--runbook-dir` and every script of the stub, and an entry of `--runbook-dir` that the stub has no
script for is accepted: its run is `Submitted` and then ends `FailedToStart` with the reason `the stub
has no script for runbook <name>` (the stub plays scripts and reads no runbook of the directory). Only
the runtime `loom` runs what the view lists. A row has a name and a version only: no path of the host
is in it. The view sits behind the same token check as every route; it is a read, so the cookie of
the web UI's login opens it. It is a view and no new command: the language cannot read a file system,
so the specification declares the entity `Runbook` that no command creates, and the daemon fills the
rows from the host when asked.

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
`runbook.run-orphaned` (at the next start, or at the stop of a daemon that ends with runs live: see "Stopping the daemon"), `runbook.answer-undeliverable` (an answer the runtime could not take up) and
`runbook.runtime-failed` (before the run ends with the outcome `runtime-failure`). Each is declared
in the specification by the command that writes it, in its summary: the language cannot write a
run event from a command, so no row of the interpreted target stands for them. The runtime
`--runtime loom` writes one more of its own, `runbook.pinned` (record `runbook`, `protocol`,
`revision`, `bindings_sha256`, `copy_sha256`: the runbook the run copied at its submit,
`docs/engine.md`, "The run's copy of the runbook"); the specification does not declare it.

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
  stream of the daemon ends at shutdown only. When a stop ends (SIGINT or SIGTERM and no live run, or the
  end of the drain: see "Stopping the daemon") every open stream is closed and the daemon exits within
  2 s, also when a client does not read; during a drain the streams stay open. A client that gets the end of a
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
runbook serve --principal person:<id>|agent:<id> [--listen ADDR] [--state-dir DIR]
              [--context-root DIR]... [--runbook-dir DIR] [--min-confinement local|linux]
              [--token-file FILE] [--ui] [--mcp] [--drain-seconds SECONDS]
```

The runtime behind the daemon is a stub unless `--runtime loom` is given (`docs/engine.md`): on the
stub a run is scripted, it records one note and ends, and nothing is executed. One preset waits: the runbook `stub-decision` raises two decision
requests at once (the question is the run's intake, the options are A and B), waits until both are
answered, records `stub.answered` and ends `approved`; it is how a client or the web page is tried
against a run in `WaitingOnDecision`. The daemon says so on standard error when it starts. It prints
`runbook: listening on http://127.0.0.1:<port>` on standard output and runs until SIGINT or SIGTERM
and then until its live runs have ended (see "Stopping the daemon"); open event streams are closed
then (see "Event streams").

| Option | Default | Meaning |
|---|---|---|
| `--principal` | none, required | Who the daemon acts as: `person:<id>` or `agent:<id>`, `<id>` of `[a-z0-9][a-z0-9._-]{0,62}`. A missing or malformed one is refused at start, exit 1, before anything is bound or written. There is no environment variable. Fixed for the life of the process: no command, header, MCP argument or HTTP member changes it. The runtime is given this principal for every answer. |
| `--listen` | `127.0.0.1:7430` | Address to bind. Port 0 picks a free port; the line above names it. |
| `--state-dir` | `$XDG_STATE_HOME/runbook`, else `~/.local/state/runbook` | Where the daemon keeps its record. |
| `--context-root` | none | A directory a run's `context_path` may lie under; repeatable. With none, any `context_path` is refused. |
| `--runbook-dir` | `protocol/` of the located checkout or install | Where a submit may find a runbook besides `sdc` and `sdc-bugfix`. |
| `--min-confinement` | `linux` | The weakest level a submit may ask for; a weaker one is refused `BelowConfinementFloor`. The level is stored with the run; nothing is confined yet. |
| `--token-file` | none | File that holds the bearer token. |
| `--ui` | off | Serve the web UI, compiled into the binary, beside the API (see "The web UI"). Without it `GET /` is 404. |
| `--drain-seconds` | none: no bound | The most seconds a drain takes after the first SIGINT or SIGTERM. When they have passed with a run still live, the daemon stops at once as on a second signal (see "Stopping the daemon"). |

Token. With a token, every route, an unknown one included, answers `401` unless the request has
`Authorization: Bearer <token>`. The token comes from `--token-file` or from the environment value
`RUNBOOK_TOKEN`, the file first. A trailing newline is dropped. The token is at least 32 bytes: a
shorter one is refused at start, exit 1, with a message that names the minimum and `--token-file` and
shows neither the token nor its length (the client side takes the token it was given and has no minimum).
Make one with `head -c 32 /dev/urandom | base64 > token && chmod 600 token` (44 characters). The daemon compares in constant
time, and it never logs or echoes the token. A token file that is readable by others (any bit of
`o+rwx`) is refused at start, exit 1, with the mode in the message: `chmod 600` it. Without a token
the daemon listens on loopback only: a listen address that is not loopback and no token exit 1
before anything is bound or written.

Limits. A body over 6 MiB + 64 KiB is refused `413` before it is read whole (the intake is at most
1,048,576 bytes; JSON writes a control character as six). Answers outside the specification have the
body `{"refused": <what>, "message": <why>}`: `400` for a body or query the daemon cannot read, `401`,
`403` and `415` (see "Requests from a browser"), `404` for an unknown route, `405` for the wrong
method, `413`, and `503` for a submit while the daemon drains (see "Stopping the daemon"; the body
has `live_runs` too). None of `401`, `403`, `415` and `503` is in the specification (`ess/domains/daemon.yaml`,
the generated OpenAPI): the first three are the security floor of the realization, the fourth is the
daemon's own stop, and they are written here only. The
`403` the OpenAPI does declare, "not granted" with an `actor`, is another answer: this daemon
authenticates no actor and never sends it, and the floor's `403` has `"refused": "forbidden"` and a
`message`, never an `actor`.

## Stopping the daemon

A run lives on a thread of the daemon's process. A daemon that exits ends the runs it holds: the
runtime has no run of them at the next start and they end as `orphaned` (see "State directory"). So
the daemon does not exit on its first stop signal while a run is live; it drains.

- The first SIGTERM or SIGINT begins the drain. A submit is refused `503` with the body
  `{"refused": "draining", "message": ..., "live_runs": <n>}` (the MCP tool `submit_run` gives the
  same body, with `isError` true and `503` in `_meta`); no run is made. Everything else goes on: reads,
  event streams, `answer-decision`, `cancel-run`, and the live runs, which are not cancelled and not
  asked to stop. Every answer carries the header `X-Runbook-Draining: <n>`, `n` the count of live runs
  (`Submitted`, `Running` or `WaitingOnDecision`): a command, a view, a stream, the MCP endpoint, the
  UI files, a `404` or `405`, and the refusals of the guard (`401`, `403`, `415`), whose layer lies
  inside the one that adds the header. A daemon that does not drain sends no such header. The specification's rows are unchanged: a read of `runs` or
  `configuration` has no member for it.
- The journal records `daemon.draining` with the ids of the live runs, and when the last of them
  has ended, `daemon.drained`. The daemon then closes its streams and exits 0. With no live run at the
  first signal both lines are written and it exits at once.
- A run that waits on a decision keeps the drain open until the decision is answered or the run is
  cancelled: a drain waits as long as its runs do.
- A second SIGTERM or SIGINT, or `--drain-seconds <n>` seconds after the first (`n` is any number
  from 0 to 18446744073709551615; a value too large to be a point in time is no bound), ends the daemon at
  once. It exits 75 when at least one run was live and is orphaned, and 0 when none was left (the last
  run ended in the same moment). Each live run then ends as `orphaned` in that moment, with its decision requests
  withdrawn and one `runbook.run-orphaned` event whose `reason` is `the daemon was stopped while the
  run was live` (the reason after a start is `the daemon started again and the runtime holds no live
  run of this id`). `daemon.drained` is not written. A run in `Submitted` at that moment is
  `FailedToStart` at the next start, as after a kill.
- SIGKILL, a crash and a power cut give no drain: the runs end as `orphaned` at the next start, with
  the reason of a start.
- A stop that fails inside the daemon (a panic of the task that waits for the drain) ends the live runs
  as `orphaned` with the reason of a stop and exits 1, or 75 when it orphaned a run. A write to standard
  error that fails (a pipe whose reader is gone) is ignored and does not end the drain.

The exit codes of `runbook serve`: 0 the daemon stopped and no run was left live; 1 the start was refused
or the server failed; 75 the stop left at least one run orphaned (a task that stops the daemon reads this
as "a run was lost"); 2 and 64 usage errors of the command line.

A person or a task stops the daemon so that no run is lost by sending SIGTERM and waiting for the
process to exit, for example `kill -TERM <pid>` and then waiting on that process id. A restart that
installs a new runbook is: stop and wait, install, start. Waiting is the point: a new daemon cannot
open the state directory while the old one holds `daemon.lock`.

Under systemd the unit must not cut the drain short. systemd.service(5), `TimeoutStopSec=` (the
text of systemd 261 as the host's `man 5 systemd.service` prints it): "Second, it configures the time to
wait for the service itself to stop. If it does not terminate in the specified time, it will be forcibly
terminated by SIGKILL (see KillMode= in systemd.kill(5)). Takes a unit-less value in seconds, or a time
span value such as "5min 20s". Pass "infinity" to disable the timeout logic. Defaults to
DefaultTimeoutStopSec= from the manager configuration file (see systemd-system.conf(5))." On a host where
`systemctl show -p DefaultTimeoutStopUSec` prints `DefaultTimeoutStopUSec=1min 30s` that default is 90
seconds, shorter than a step of a run, so a unit that keeps it sends SIGKILL in the middle of the drain.
And systemd.kill(5), `KillMode=`: "If set to control-group, all remaining processes in the control group
of this unit will be killed on unit stop (for services: after the stop command is executed, as configured
with ExecStop=). If set to mixed, the SIGTERM signal (see below) is sent to the main process while the
subsequent SIGKILL signal (see below) is sent to all remaining processes of the unit's control group."
`control-group` is the default, and it would also end the programs a run's step has started. So the unit
has

```
[Service]
KillMode=mixed
TimeoutStopSec=infinity
```

`TimeoutStopSec=infinity` waits for as long as the longest run; a value longer than the longest step of
the longest run does the same with an end. To bound the drain inside the daemon instead, give
`--drain-seconds <n>` in `ExecStart` and a `TimeoutStopSec=` of more than `n` seconds: the daemon ends
the drain at `n` itself, and its live runs end as `orphaned` with the reason of a stop. `systemctl stop`
and `systemctl restart` of such a unit return when the drain has ended.

## Requests from a browser

A page of another site can send a request to a daemon on loopback, and a name that was rebound to
`127.0.0.1` makes a page of another site the same origin as the daemon. One middleware checks every
request, on every route (commands, views, event streams, `/mcp`, the UI files, `/ui/login`), with the
UI or without it, with a token or without one, in this order, and the first refusal ends it:

- Host. On a loopback listener the `Host` header is `127.0.0.1:<port>`, `localhost:<port>`, `[::1]:<port>`
  or the listen address as written (a port of 80 may be left out); anything else, or no `Host`, is
  `403` before any other work. A listener that is not loopback needs a token and has no `Host` check.
- Origin. A request with an `Origin` header is `403` unless the origin is `http://` and has the host
  and the port of the request's `Host`; `Origin: null` is refused. A request without `Origin` (curl,
  `runbook runs`, an MCP client) is not from a page and passes.
- Token. Without it `401`, as above. `POST /ui/login` is the one request that carries its token in its
  body: the middleware lets it through and the login checks it; every other method on `/ui/login`
  without the token is `401`.
- Content type. `POST /daemon/commands/<name>` and `POST /mcp` take `Content-Type: application/json`
  only (parameters such as `charset=utf-8` are fine), else `415`. A page cannot send that type to
  another origin without a preflight, which the daemon does not answer.

The bodies of `403` and `415` are fixed: `{"refused": "forbidden", "message": ...}` and
`{"refused": "unsupported media type", "message": ...}`. They name no header and no value of one.

State directory:

| Path | Holds |
|---|---|
| `daemon.lock` | Held while a daemon runs on the directory; a second daemon exits 1 with "in use by another daemon". |
| `journal.jsonl` | Every change of state as one JSON line, appended and flushed; replayed at start. It is the truth; a run killed mid-way is read back in its last state. A drain adds two lines of its own: `daemon.draining` (the ids of the live runs) and `daemon.drained` (see "Stopping the daemon"). |
| `runs/<run_id>/` | The run's own record, written by the runtime. |
| `workspaces/<run_id>/` | The run's workspace; a `context_path` is copied here, symbolic links are not followed. |

After a restart a run that was `Submitted` and never started becomes `FailedToStart` (event
`runbook.start-failed`). For each run the record holds as `Running` or `WaitingOnDecision` the daemon
asks the runtime (`Runtime::recover`): `resumed` leaves the run as it is; `gone` ends it with the
command `EndOrphanedRun`. The run is then `Ended` with the outcome `orphaned` (the cause, readable in
`outcome` of the `run` view), its Open decision requests are withdrawn in the same step, as a cancel
does, the event `RunOrphaned` is published and the run's event stream gets one event of kind
`runbook.run-orphaned`. A run that has ended is not asked about again, so a second start adds
nothing. The stub runtime always answers `gone`. A daemon that was stopped by its signal has no such
run unless its drain was cut off: it ended them itself, as `orphaned`, with the reason `the daemon was
stopped while the run was live` (see "Stopping the daemon").

## The web UI

`runbook serve --ui` serves the web UI beside the API. The files of `ui/web/` and `ui/daemon/` (the code that answers a decision, which the inspector `ui/server.mjs` does not serve) are compiled into the
binary at build time (`runbook/build.rs` lists the two directories, so a file added there is embedded by the
next build); the daemon reads nothing from disk to serve them, and it needs no `ui/` directory beside it.
Without `--ui`, `GET /` is `404`.

- Routes. `GET /` is `index.html`; every other file is at its path below `ui/web/` or `ui/daemon/` (`/app.js`,
  `/fonts/inter-variable-latin.woff2`). A path is looked up exactly as sent: a directory, `..` and
  any encoded form of either answer `404`, and so does a path that is not in the list. A write method on a
  file answers `405`.
- Types and headers. The content type follows the extension. Every file carries
  `X-Content-Type-Options: nosniff`, `Referrer-Policy: no-referrer`, `Server-Timing: runbookd` (how the page knows the
  daemon served it), `Cache-Control: private, no-cache`
  and an `ETag` that is the SHA-256 of its bytes (`If-None-Match` with it answers `304`), and the policy
  `default-src 'none'; script-src 'self' '<hash of the inline script of index.html>'; style-src 'self';
  style-src-attr 'unsafe-inline'; img-src 'self' data:; font-src 'self'; connect-src 'self'; base-uri 'none';
  form-action 'self'; frame-ancestors 'none'`. The page has one inline script (the phone breakpoint in
  `index.html`), allowed by its hash and no other; its code writes `style="..."` attributes into its markup,
  so attributes may carry inline styles and a `<style>` element may not; `data:` images are for the arrow
  in `style.css`.
- What a step produced. The page has a Products tab and a product line (`ui/web/products.js`,
  `ui/web/markdown.js`, embedded like every file of `ui/web/`). Served by the inspector (`ui/server.mjs`) they list
  the evidence records, the actions and the files of a run and open each one; the daemon's API has no view of
  the files, evidence records or action output of a run yet, so served here the tab and the line say "the
  daemon serves no files of a run yet" and list nothing. Nothing in them is made up. Run content never becomes
  markup of the page: the page builds nodes from the text, and the policy above (`script-src 'self'`,
  `connect-src 'self'`, `img-src 'self' data:`) stays as it is (`ui/README.md` "What a step produced").
- Token. The files are behind the same middleware and the same check (`Auth::admits`) as the API: with a
  token, a request without it is `401`, `/` included, on every listener. A browser cannot send an
  `Authorization` header with a page load, so it hands the token over once: the `401` of a page load
  (`GET /` with `Accept: text/html`) is a form that posts the token in its body to `POST /ui/login`
  (behind the same middleware as every route; a token never stands in a URL, so no log has it). A right
  token is answered `303` to `/` with a cookie `runbook_ui` (`HttpOnly`, `SameSite=Strict`, `Path=/`),
  a wrong one `401` with the form again, and a form from another origin `403`. The cookie does not hold
  the token: its value is the lowercase hex of the SHA-256 of the bytes `runbook-ui-read/1`, one NUL
  byte and the token, and a browser shows it to every server on the host. With `--ui`, the middleware
  compares a presented cookie with that value in constant time, for `GET` and `HEAD` only; the value is
  no bearer token, and a cookie never authorizes a command (`POST`). The cookie has no `Secure`
  attribute because the daemon speaks plain http only, and a login over a non-loopback address sends the
  token in clear: the page is meant for loopback or a tunnel (ssh, a VPN).
- A command from the page. The page answers a decision with `answer-decision`, the command `runbook
  decide` sends: `POST /daemon/commands/answer-decision`, `Content-Type: application/json`, and
  `Authorization: Bearer <token>` when the daemon has a token, from the page's own origin, with no
  cookie (the page sends the command without credentials). It goes through the same guard as every
  route: a request from another origin is `403` with a token and without one, a cookie alone is `401`,
  a body that is not JSON is `415`. The token is typed into the page and lives in the memory of the
  page only: in no cookie, not in `localStorage`, `sessionStorage` or `IndexedDB`, not in a URL, the
  history state or the DOM (the field is emptied as soon as it is read). A reload forgets it: the page
  reads again (the cookie) and asks for the token before its next command, once. A wrong token is
  answered `401`, the page shows the daemon's message and sends nothing again by itself. A daemon with
  no token (loopback only) takes the command without a bearer, and the page asks for none; it finds out
  with one read of `GET /daemon/views/configuration` without credentials (`401` means a token is
  needed). The form asks for a name and sends it as `answered_by`: a label, as with `--by`, never the
  principal (the daemon's own, see "Running the daemon"). The daemon bounds it like `client` and `idempotency_key`: at
  most 200 characters, and a longer one is refused `400` with the message `answered_by is longer than
  200 characters`, before the request is looked up (it stays open). The form allows 100.
- Two modes. The header names the mode of the page: "watch" when the inspector `ui/server.mjs` serves
  it (it only reads: no start form, no token field, every `POST` is `405`) and "operate" when the daemon
  serves it (start a run, take a decision). Operate adds the button "Start a run" (`ui/daemon/start.js`,
  loaded only when the daemon named itself, like `decide.js`). Its form takes a runbook from
  `GET /daemon/views/runbooks`, the intake as text and the confinement, and Start sends `submit-run`:
  `POST /daemon/commands/submit-run`, JSON, `Authorization: Bearer <token>` when the daemon has a token,
  no cookie, `runbook_id`, `intake`, `confinement` and an `idempotency_key` that belongs to one filled form: the page
  makes it when the form is first sent and sends the same key for a press repeated after an answer that is not
  the command's whole answer (a network error, a reset, a body that is cut or is not JSON, an answer of the
  HTTP layer such as `401`, `403`, `400` or `415`, a `5xx`), so the daemon's replay rule (`already-submitted`)
  returns the first run and no second one starts; the key changes only when the daemon answered the command
  with a `2xx` or `4xx` status and a named outcome (`submitted`, `already-submitted` or a refusal such as
  `no-runbook`), read whole (a `5xx` keeps the key whatever its body names), and
  when the runbook, the intake or the confinement changed (the page then says that the earlier press may have
  started a run: look in the list of runs). The levels offered start at the daemon's floor
  (`confinement_floor` of `GET /daemon/views/configuration`) and none is selected: the person picks the
  level, Start stays disabled until then, and the form says what the floor is. The form presets no level; the
  command does: a `submit-run` without the member `confinement` runs at `linux` (the specification says so in
  the summary of `sdc.daemon.ConfinementLevel`, `ess/domains/daemon.yaml`: "The default of a submit is linux
  (the handler fills it)", and the handler fills it in `submit_run_for`, `runbook/src/commands.rs`). A level
  the person picked while the form was still reading the floor and the list stays picked when the floor just
  read still offers it, and is dropped (Start disabled) when it is below that floor; a level below the floor is not sent (the daemon refuses it anyway:
  `below-floor`). The page checks the level against the floor it read when the form opened; when the daemon
  comes back with a higher floor while the form is open, a start with the lower level can leave the page, and the
  daemon refuses it with `409 below-floor` and starts no run: the daemon's check is the guard
  (`ui/test/security-start-ready-window.spec.mjs`, "measured: the daemon comes back with a higher floor while the form is open and has read its floor", which asserts `409` and 0 runs).
  From the moment the form opens until it has read the floor of that opening, Start is
  disabled and a press sends nothing, whatever level an earlier opening offered. A runbook the person
  picked that the list just read no longer has is not replaced by another: none is selected, Start is
  disabled and the form says that the picked runbook is no longer listed. The answer of a press belongs to
  the opening that sent it: it shows the run and changes nothing that was typed into the form opened
  again. A token that a header cannot carry (a character above U+00FF, a control character) is
  forgotten at once, the form asks for it again and says that the token cannot be sent; closing the form
  (Close, Escape, a successful start) empties the token field.
  The token follows the rules of the answer above and is held in its own variable of the page's memory (the
  form asks for it once beside its own button). The new run is read back from the answer
  (`published[0].payload.run_id`), appears in the run list and is selected. A refusal is shown as its
  name and status (`no-runbook (422)`, `intake-too-large (422)`, `401`) with its text, and the form keeps
  what was typed. Under the stub runtime (the `runtime` of the rows) the form says that the run is
  scripted and nothing is executed. The text of an intake, a runbook name and a refusal reaches the
  page as text nodes (`textContent`), never as markup.
- What the page shows. Served by the daemon, it reads `GET /daemon/views/runs` for the list of runs and
  `GET /daemon/views/daemon-events` as a stream (`Accept: text/event-stream`, resumed by position) for
  the events of every run; the feed of the selected run is the events of that run. The Decisions drawer
  lists the open decision requests of the selected run, read from `GET /daemon/views/open-decisions`:
  the question and each option as the view has them (letter, what it does, cost, what it admits and
  blocks), and an answer is one option or free text. Each card names the step that asks: the case, the
  action and the capability of the request, as text. The daemon records no event
  when a run starts to wait or goes on, so the page reads the runs and the open decisions again after
  its own answer and every two seconds while it is visible. The case tree, the graph and the phases need
  views the daemon does not have yet: the inventory is in the story `ui-embedded-in-the-binary`, section
  "Slice 1". Served by `ui/server.mjs`, the page reads that server as before.

## MCP

The MCP tools `read_run` and `read_events` ([`mcp.md`](mcp.md)) answer a run id the daemon does not know
with an error: `isError` is true and `_meta["runbook/http-status"]` is `404`. The HTTP views `run` and
`events` answer the same id with `200` and no rows; both stay as they are.

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
authenticates none). A claim of an exact instant is read like any other: the daemon reads one clock
(`runbook/src/clock.rs`; the system clock when it is started by `runbook serve`) and the test builds
it with a settable clock and moves it to the `at` of each step before the step runs. That clock is
for tests only: no option, environment variable or request sets it.

The numbers are committed in the test (`GATE`: the scenarios, the executed, the passed, the skipped) and
may not move the wrong way; the failures are named in `KNOWN_FAILURES` with their reasons, so a different
failure cannot hide behind them. Now: 13 scenarios of the daemon (10 more are of `sdc.decision`, out of
the daemon's domain), 13 executed, 13 passed, 0 failed, 0 skipped, and `KNOWN_FAILURES` is empty
(before the clock: 10 executed, 10 passed, 2 skipped for an exact instant each). Before part C4 it was
4 passed and 6 failed: two differences between the
specification and the daemon, which the specification now states (the daemon's first event of a run,
and the withdrawal of a run's open decision requests by `CancelRun` and `EndOrphanedRun`; see
"Client commands"). The same suite passes on the interpreted target of ESS
(`ess verify conform run --target interpreted --path ess --suite ess/generated/conformance/authored.json
--report-format 2`): 23 of 23 scenarios (13 of `sdc.daemon`, 10 of `sdc.decision`).

The synthesized suite (`ess verify conform synthesize`, `task ess-suite:record`) is committed as
`ess/generated/conformance/suite.json` with a digest of its sources, which `task check:ess-suite:synthesized` compares; ess 0.98.0 makes it in under a minute (0.97.1 took 1048 s and did not finish in 1800 s).
`CONFORMANCE_SUITE=<file> CONFORMANCE_ONLY=sdc.daemon cargo test conformance` runs any suite document, the synthesized one included, and prints its counts.

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
