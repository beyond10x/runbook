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
| Run | Submitted, WaitingOnKey, WaitingForRoute, Running, WaitingOnDecision, Ended, Cancelled, FailedToStart | One run of a runbook. `Ended` carries an `outcome` (open text: the protocol's outcome name, or a cause of the daemon's own, `orphaned`, `runtime-failure`, `stopped: route` or `failed: step <action> <end>`), `FailedToStart` a `failure`. `WaitingOnKey`: a run with its serial key submitted before it is live (below, "Runs of one serial key"). `WaitingForRoute`: a route of the run could not answer at the submit (below, "A run starts only on a route that can answer"). |
| Session | Open, Closed | A named group of runs. A closed session takes no new run and keeps the runs it has. |
| DecisionRequest | Open, Answered, Withdrawn | A question a waiting run raised, mirrored from the runtime. Cancelling the run withdraws its Open requests. |
| RunEvent | Recorded | One record of a run, with `sequence` (per run, from 1) and `position` (per daemon, from 1, over all runs). |
| Daemon | Up | What the daemon was started with: its confinement floor and its principal. |
| ModelRoute | Declared | A model route of the daemon's `runbook.toml` with what its gateway last said of it (below, "Model routes"). |
| DaemonCatalog | Read | What a delegation may name, read when a client asks: the routes, the runbooks with the admission of their route entries, the terms of a delegation (below, "The catalog"). |

Confinement levels, weakest first: `local` (not confined), `linux`. A submit may leave out
`confinement`: it then runs at the daemon's default level (`--default-confinement`, else the
`confinement` of `runbook.toml`). The daemon has no built-in default: a daemon with none refuses a
submit that names no level, `400` and by name, and writes nothing. The run's row and its first
event (`runbook.submitted`) carry `confinement_source`: `submit` or `daemon-default`. The daemon's
default floor is `linux`; `local` must be asked for by name or stated as the daemon's default, and
is refused under a `linux` floor. The floor is never read as a default.

A submit may also carry `autonomy` (`A0` to `A5`) and `decider` (a kind name among those the operator
configured in `runbook.toml`); what it leaves out comes from `--autonomy` and `--decider`. A run with a
level and a kind has its decisions answered by that decider where the level gives them to it, and the
rest stay open for `answer-decision` (`docs/engine.md`, "Autonomy and a decider"). The run's row, the
view `run` and the `runbook.submitted` event carry `autonomy` and `decider`; a kind that is not
configured, an unknown level and a level with no kind are refused `400` before anything is written. An
answer of a rule or a decider is recorded as `rule:<name>` or `decider:<kind>` in `answered_by` and
`principal`. A client label is trimmed of ASCII whitespace and refused `400` when it holds a
non-ASCII or a control character, or starts, in any case, with `rule:`, `decider:` or `router:`
(the refusal prints no byte of it).

## Client commands

Every command is a `POST` to `/daemon/commands/<name>` with a JSON body.

| Command | Refusals |
|---|---|
| `open-session` | `SessionExists` |
| `close-session` | `NoSuchSession`, `SessionStateConflict` (already Closed) |
| `submit-run` | `IntakeTooLarge` (over 1,048,576 bytes), `NoSuchRunbook`, `ContextOutsideRoots`, `RouteRefused` (422 `route-refused`: below, "The route of a run"), `NoSuchDaemon`, `NoSuchSession`, `BelowConfinementFloor`, `SessionClosedToRuns` |
| `delegate` | `IntakeTooLarge`, `RouteRefused` (422 `route-refused`, as `submit-run`'s: below, "A delegation"), `NoSuchRunbook`, `ContextOutsideRoots`, `RepositoryRefused` (422 `repository-refused`: below, "A delegation that changes a repository"), `NoSuchDaemon`, `NoSuchSession`, `SessionClosedToRuns`; and `503` `route-unavailable` outside the language (below) |
| `cancel-run` | `NoSuchRun`, `RunAlreadyEnded` |
| `answer-decision` | `NoSuchDecisionRequest`, `DecisionNotOfRun`, `DecisionWithdrawn`, `DecisionAlreadyAnswered`, `NoSuchRun`, `RunNotWaiting`, `UnknownOption` (422: a request read from its case, an answer that names none of its options) |

An MCP client reaches `open-session` and `close-session` through the tools `open_session` and
`close_session` ([`mcp.md`](mcp.md)), with the same refusals and statuses.

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
- A submit (and a delegation, and `runbook run --detach --serial-key <k>`) may name a `serial_key`;
  see "Runs of one serial key" below.
- The intake limit is 1,048,576 bytes of UTF-8, counted by the daemon's handler. The specification
  counts characters (`intake.count`), which is the weaker bound: a text of at most 1,048,576
  characters can still be over the limit in bytes, and the handler refuses it `IntakeTooLarge`.
- The first answer to a decision request stands. The request's state is judged before its run's:
  a second answer is refused `DecisionAlreadyAnswered` whether its run still waits, has resumed or
  has ended; an answer to a request withdrawn by a cancel is `DecisionWithdrawn`; `RunNotWaiting`
  is left for an Open request whose run does not wait (a state the daemon's own commands never
  leave, so a client does not meet it).
- A request read from its case (it has the member `artifact`, below) takes the letter of one of its
  options, alone or followed by a colon and text (`B: too short`: the text goes with the option as its
  reasons), or any text where `artifact.free_answer` is true. The option `free` (a declared result) takes
  text: it is answered `free: <text>`, and its letter alone or with nothing after the colon names no
  option. Any other answer is refused
  `UnknownOption` (status 422, outcome `unknown-option`, payload `request_id`) after the request's and
  the run's state are judged and before anything is committed or handed to the runtime; the request
  stays Open. The run's own decider is held to the same rule: its answer that names no option is not
  saved, and the request stays Open for a person. A request without `artifact` takes any answer, as
  before. The specification states the
  check as the input `answer_offered`, which the handler fills (`ess/domains/daemon.yaml`,
  `AnswerDecision`); a client never sends it.
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

### Runs of one serial key

A submit or a delegation may name `serial_key`, 1 to 200 characters of the submitter's choice (the daemon never
derives one; an empty or a longer key is refused `400` before anything is written). Runs with one key run one at a
time, in the order they were submitted; runs with another key or none are not held. The key is recorded on the run
(`serial_key` of the views `runs` and `run`, and of the event `runbook.submitted`).

- A run whose key a run submitted before it holds (a live run: any state but `Ended`, `Cancelled` and
  `FailedToStart`; or one the runtime has not let go yet, below) is `WaitingOnKey` (command `WaitOnKey`, event `runbook.waiting-on-key`: serial_key, behind): nothing of it is
  started, its context is not copied and its routes are not asked. `key_wait` of the views says
  `waiting on key <k> behind run <id>`, the newest run of the key submitted before it that holds it, read when the view is
  read: after a cancel of that run it names the one ahead of that.
- The key is free as soon as each run ahead has left the live states: ended (also `stopped: route` and a failed
  step), cancelled, failed to start, or orphaned at a restart, and the runtime has let it go. A run that was taken
  up holds the key until its run thread in the daemon has returned: a cancelled holder is `Cancelled` at once, but
  its key is free only once the runtime ended its program in flight and its model call (the engine waits up to 5 s
  for one), and `key_wait` of the run behind names the cancelled run until then. A run cancelled while it waited
  was never taken up and holds nothing. The next run then takes it (command `TakeKey`,
  event `runbook.key-taken`), is `Submitted` again and is taken up as a submitted run is; its routes are asked
  again first (a gateway that is not ready or an alias it does not list fails its start, `route-unavailable: ...` or
  `route-refused: ...`; a model that is not callable makes it wait for its route).
- A cancel of a waiting run takes it out of the queue.
- The queue holds across a restart: its order is the journal's order of the submits. A run that was `WaitingOnKey`
  stays so and is taken up again when its key is free; a holder whose runtime is gone ends `orphaned` and frees the
  key. A waiting run whose context lies under no context root now, or whose routes the daemon no longer declares,
  fails to start with that reason. A stop that ends the live runs at once (a second signal, the drain bound) leaves
  the waiting runs waiting for the next start; a drain waits for them as for any live run.

## Views

Every view is a `GET` to `/daemon/views/<name>`, its parameters in the query.

| View | Parameters | Rows |
|---|---|---|
| `runs` | none | every run, newest first |
| `runs-of-session` | `session_id` | the runs of a session |
| `runs-in-state` | `state` | the runs in a state |
| `runs-of-session-in-state` | `session_id`, `state` | both filters |
| `run` | `run_id` | one run with everything it was submitted with, and `step_in_flight` while a program of it runs (below) |
| `open-decisions` | `run_id` | the run's Open decision requests (not the Withdrawn ones), each with the `case_id`, the `action` and the `capability` of the step that asks, and `artifact` for a request read from its case (below) |
| `decisions` | `run_id` | every decision request of the run, with its state, and `artifact` as in `open-decisions` |
| `sessions` | none | every session |
| `configuration` | none | the daemon's configuration: its id, confinement floor, principal and start time |
| `runbooks` | none | the runbooks a `submit-run` of this daemon accepts, by `runbook_id`, `runbook_version` (absent when the host names none) and `runtime` (`stub` or `loom`), in order of name |
| `products` | `run_id` | one row: the files the run wrote under its `work/` directory (`files`, each `path`, `size`, `modified_at`), `hidden`, `left_out` and `cut` (below) |
| `product` | `run_id`, `path` | one row: one file of the run's `work/` directory, its text up to 1 MiB (below) |
| `cases` | `run_id` | the case tree of the run, a row per case: `case_id`, `parent`, `name`, `protocol`, `case_state`, `outcome` (below) |
| `routes` | none | the model routes the daemon declares, in order of name: `route`, `gateway` (its name), `alias`, `kind`, `wires`, `status`, `phase` when the gateway gave one, `max_model_len` and `tool_calling` when `/v1/models` gave them, `read_at`, for a managed route with a wake cap or a pod price the day's wake and pod figures, and for every route the figures of its daily caps (below, "The day's figures of a route") |
| `delegation-result` | `run_id` | one row: what a delegation's run has come to, its result text and its calls (below, "A delegation") |
| `run-metrics` | `run_id` | one row: what the run took, its times, a row per step and the totals of its model calls, tokens and cost (below, "The metrics of a run") |
| `catalog` | none | one row: the declared routes with their prices, the runbooks with their purpose, inputs and the admission of their route entries, and the terms of a delegation (below, "The catalog") |

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

### Model routes

The daemon's `runbook.toml` (the file `--config` or `RUNBOOK_CONFIG` names, or the one the walk finds; a file a
program of a run finds has none of these tables) declares its gateways, its model routes and its policy
(`docs/model-routes.md`, section 1):

```toml
[gateways.default]
base_url = "http://127.0.0.1:<port>"
token_file = "gateway-token"         # optional; relative to the file

[routes.qwen]
gateway = "default"                  # a [gateways.<name>] table
model = "qwen"                       # the gateway's alias, as its /v1/models lists it
kind = "managed"                     # managed, hosted or local
start_wait_seconds = 600             # optional
price_in_micro_usd_per_mtok = 0      # optional
price_out_micro_usd_per_mtok = 0     # optional
wake_on_submit = false               # managed only; true needs the two keys below above 0
wake_daily_cap_micro_usd = 0         # managed only, optional: the most the route's wakes book per day
wake_cost_micro_usd = 0              # managed only, optional: what one wake books
price_per_hour_micro_usd = 0         # managed only, optional: the pod's price per hour, its time for runs booked

[routes.claude]
gateway = "default"
model = "claude"
kind = "hosted"

[route_policy]
default = "claude"                   # optional: a declared route
classifier = "claude"                # optional: a declared route

[delegation]
classify_threshold = 0.8             # optional: 0 to 1
copy_exclude = ["target/", "node_modules/"]   # optional: what the copy of a context leaves out
copy_max_entries = 50000             # optional: a whole number from 1
copy_max_bytes = 268435456           # optional: a whole number from 1
```

`[delegation] classify_threshold` is the confidence at or above which a classified delegation runs the runbook the
classifier picked (below, "A delegation"); without it 0.8. `copy_exclude`, `copy_max_entries` and `copy_max_bytes`
are the exclude list and the bounds of the copy of a run's context (below, "The copy of a context"); a key the file
names replaces its default, so `copy_exclude = ["build/"]` copies `target/`. A value that is no number from 0 to 1,
no list of names or relative paths, no whole number from 1, or another key of the table, refuses the start, exit 2,
naming the key (and for a list the entry's place, never its value).

A lone `[gateway]` table (the one gateway of a daemon's model decider, `docs/engine.md`, "A gateway") is the
gateway `default`: a route names it so, and the decider takes it as before. Both a `[gateway]` table and a
`[gateways.default]` table are refused. The daemon reads the token file of every `[gateways.<name>]` once when it starts, and the one of a lone `[gateway]` only when a route names it (else the model decider reads it, as before). It refuses to
start, exit 2, with a line that names the key and never its value, a part of it or a path: a route that names a
gateway the file does not declare, a gateway without `base_url`, a `token` (or any key that is not one of the
members above) in a gateway, a route or the policy, a `token`, `token_file`, `base_url` or `api_key` in a route,
`wake_on_submit = true` without a `wake_daily_cap_micro_usd` above 0 or without a `wake_cost_micro_usd` above 0
(the line names `routes.<name>.wake_on_submit` and the key it lacks), a `wake_cost_micro_usd` above the route's
`wake_daily_cap_micro_usd` (no wake would ever be sent; the line names both keys), a wake key or `price_per_hour_micro_usd` on a route that is not managed, a `kind` that is not one of the three, a
`model` that is no alias (an address), a policy that names a route the file does not declare, a name that is not
1 to 64 letters, digits, `.`, `_` or `-`, and a token file it cannot read. Nothing is written before.

The view `routes` (and the MCP tool `list_routes`, [`mcp.md`](mcp.md)) lists the declared routes by name. The
daemon reads each gateway a route names with `GET` requests only, with the gateway's token when it has one:
`GET /ready`, then `GET /v1/models` (the wires of each alias), then `GET /v1/status`, and for a managed or a local
route whose alias that does not cover, `GET /v1/models/{alias}/status`. It sends no model call and no wake, and
reads nothing of a gateway no route names.

Each `GET` ends within 3 s, its whole answer included, and one read of all gateways within 5 s (the gateways are
read side by side); a request still open then counts as no answer. A read is kept for 15 s of the daemon's clock;
`read_at` says when it was made. A request answers the rows of the last read at once and waits on no gateway: when
they are 15 s old or older, it starts one read in the background (never a second while one runs) and still answers
them; the next request after that read answers its rows. Only a request before the first read ended waits for it,
at most 5 s; a gateway that has not answered by then is `gateway-unreachable`. `status` is:

| Status | When |
|---|---|
| `gateway-unreachable` | `/ready` or `/v1/models` gave no answer, or the address is `https://` (the daemon speaks plain HTTP) |
| `gateway-not-ready` | `/ready` answered, not `200`, or `/v1/models` gave no list |
| `model-unknown` | the gateway lists no model of the route's alias |
| `ready`, `not-ready` | the gateway gave the model's status: `callable` of `/v1/status`, else the phase of the model's own status route (`ready` only for the phase `ready`); `phase` is the gateway's |
| `listed` | the gateway lists the alias and gave no status of it: a hosted route on a gateway without `/v1/status`, or a status route that did not answer in time or answered anything but `200` with a phase. Today's gateway answers `404` for `/v1/status` and for the status route of a model it does not start (a local model): such a route is `listed`, not `gateway-unreachable` |

A row names the gateway by name; no address, port, token, token file or price is in it. Of what a gateway answers, only words reach a row: a wire or a phase that is not 1 to 64 visible ASCII characters is left out, at most 16 wires are kept, and a status with neither such a phase nor a boolean `callable` counts as none.

#### The route of a run

A `submit-run` names the route of its model calls by `model_route` (a declared route name) and, for a role that
takes another, `model_routes` (a map from role to route name; the role `default` is the run's own). The run's
word wins; a run that names none takes `[route_policy] default`. A run may take a route only when it has a model
step: its decider kind is `model` and its autonomy is above `A0`, or a program the runner starts for its runbook (of its root
bindings or a child's: an `executor.argv`, an `arguments_from` or a `handoff_when` check) calls a model through the run's route file (`RUNBOOK_ROUTE_FILE`, below): an agent node, or a
program whose per-call budget, read as the run reserves it, is above 0 (its `--max-budget-usd`, else `max_budget_usd`
of its action, else of its file, else the largest of the runbook; `max_budget_usd: 0` is a program that calls no
model; `docs/model-routes.md`, section 2). A run with no model step takes no route, sends and books no wake and waits
for none, also when its submit names a route: such a submit is admitted, and its first event `runbook.submitted` holds
`model_route_not_taken` (`model_route` and `model_routes` as the submit named them, and `reason`, "the run makes no
model call" and why) in place of `model_route`. Only a run whose runbook runs no program at all and whose decider is
no model is refused for naming one (below). A run whose model calls come from its programs only and that names no route takes `[route_policy] default` only
when the file declares one and the daemon serves the relay (a loopback listener, below); else it takes none, is
not refused and runs as before routes. The interim route `default` of a lone `[gateway]` goes to a run whose
decider is `model` above `A0` only: a submit for any other run that names it is refused `route-refused`, the reason
naming the member (`model_route`, `model_routes.<role>`). A run on a route gives the route file to its
decider's adapter and to every bound program; a run on no route gives none. The run record and the views `runs`, `run` and the
other run views carry `model_route`, `model_route_alias` and `model_route_source` (`run` or `default`), and `run`
also `model_routes`; the event `runbook.submitted` says the same. The relay's table of the run is built from them:
the role `default` on the chosen route, each role of `model_routes` on its own, every call booked with the
route, the alias and the setting that chose it (`run` for a route the submit named, `default` for the policy's).

A submit is refused `422 route-refused` (error `RouteRefused`: `run_id`, `reason`, `declared`, the daemon's route
names), and nothing is written, when it names a route the daemon does not declare, an alias of a gateway's model
instead of a route, an address (a value with `/` or `:`), a value that is no route name (a token, or any other
text that is not 1 to 64 letters, digits, `.`, `_` or `-`), an empty name, the role `default` or a role that is
no name in `model_routes`, or a route at all for a run that makes no model call (its runbook runs no program and no
agent node, and its decider is not `model` above `A0`; the default is not applied to such a run; a run whose programs
call no model is admitted instead, above); and when a run
whose decider is `model` above `A0` names none while routes are declared and no `[route_policy] default` is. The `reason` names the member
(`model_route`, `model_routes.<role>`) and why, and never the value. `model_route_alias`, `model_route_source` and
`route_refused` are filled by the daemon: a submit that sends one is refused `400` by its name. A daemon with only
a `[gateway]` and a model decider declares the interim route `default` on that gateway (kind `hosted`, the
decider's model as its alias, no price) and takes it as its default: the view `routes` lists it, a model run that
names none is on it (source `default`), `model_route: "default"` names it, and any other name is refused with
`declared` `["default"]`.

A runbook may declare the routes it needs in its route entries, `bindings/<runbook>.routes.yaml` (below, "The
catalog"), which the daemon reads at each submit of the runbook and each delegation that names it as its protocol
(`docs/model-routes.md`, section 2). The precedence per role is the run's word, then the runbook's, then the default:
a role the submit names (`model_route` for the role `default`, `model_routes.<role>`) takes that route; a role it
does not name takes its entry's route, the one it names or, for `needs`, the run's `model_route` or
`[route_policy] default` when that meets them (the run's first, then the default: a run's route that misses them
leaves them to the default), else the one allowed route that does; the role `default` with no
entry takes `[route_policy] default`. The run's `model_routes` holds every role besides `default` with its route,
and the relay books a call of a role the runbook chose with the source `protocol` (`model_route_source` is
`protocol` when the runbook's entry for the role `default` chose it). The submit or delegation is refused `422
route-refused`, and nothing is written, when the file cannot be read or is not `runbook.routes/1`, an entry is not
one, a role's name starts with `runbook-` (the relay would take the model `runbook-<role>` for another role; the
reason names `model_routes.<role>`, and a submit's `model_routes` refuses such a role the same way), an entry names a route the daemon does not declare or that `routes_allowed` does not hold, no allowed route
meets an entry's `needs`, several do and neither the run's route nor the default is one of them, a route the
submit names for a role misses the role's `needs`, or any route of the run (the submit's, an entry's or the
default) is outside `routes_allowed`. The `reason` starts with `runbook <id>:` and names the role and the route or
the requirement, or the route and the member or setting that chose it (`model_route`, `model_routes.<role>`,
`route_policy.default`); an address, a token or another text of the file that is not a name is never echoed. A
runbook with no such file takes routes as above, unchanged.

#### A run starts only on a route that can answer

Before anything is written, a submit whose run takes declared routes asks the gateway of each of them (`docs/model-routes.md`, section 3)
`GET /ready`, `GET /v1/models` and the status of the route's model (`callable` of `GET /v1/status` when the gateway
serves it, else the phase of `GET /v1/models/{alias}/status` for a managed or a local route; a hosted route on a
gateway without `/v1/status` is read no further), within 5 s, with `GET` requests only:

| The gateway says | The submit |
|---|---|
| `/ready` not `200`, no list of models, no answer | refused `503` `{"refused": "route-unavailable", "message": ...}`, the message naming the route and `gateway not ready` (or that it does not answer); outside the specification, like the `503` of a drain; nothing is written |
| it does not list the route's alias | refused `422 route-refused`, the `reason` naming the route and `the gateway serves no model <alias>`; nothing is written |
| the model is not callable | accepted; the run is `WaitingForRoute` (command `WaitForRoute`, event `runbook.waiting-for-route`: route, phase, wait_seconds) and `route_phase` of the view `run` holds the phase |
| every route can answer | accepted; the run starts as before |

A run that waits sends no call to its routes, and wakes nothing unless a route's wake is on (below). The daemon reads their status again at most once
every 5 s of its clock: once every route can answer, the runtime starts the run; when the least
`start_wait_seconds` of the routes that cannot answer (none declared: no wait) is over first, the run ends `Ended` with the outcome `stopped: route`
(command `EndRouteWait`, event `runbook.route-wait-ended`: route, last phase), and `route_phase` holds the last
phase. A gateway that does not answer, or no longer lists the alias, during the wait leaves the run waiting. A cancel
ends the wait (`Cancelled`); a run that waits keeps a drain open until it starts or its wait is over; a run that
still waits when the daemon ends (a second stop signal, the drain bound) or that was `WaitingForRoute` at a restart is
`FailedToStart`. The interim route `default` of a daemon with only a `[gateway]`
is not asked, as before routes. A submit that repeats an idempotency key already used is not asked either: it answers
the first run (`already-submitted`) whatever the gateway says.

#### The wake of a managed route

The wake ships off (`docs/model-routes.md`, section 3 point 4, D-5): a pod is started by a person until the operator
sets `wake_on_submit = true` on a managed route, and that is accepted only with `wake_daily_cap_micro_usd` and
`wake_cost_micro_usd` both above 0 (else the daemon refuses to start, above). With the wake on, a run the daemon
holds `WaitingForRoute` on that route while the gateway gives the phase `stopped` sends one
`POST /v1/models/{alias}/wake` with the gateway's token, once for the run, before its first read of the wait, unless
it joins a wake already sent:

- A wake of the same route sent less than the route's `start_wait_seconds` before (by any run, also before a restart
  of the daemon: the journal holds it) is still starting the model: the run joins it, sends nothing and books nothing.
  Only a wake that may still start the model is joined: one the gateway took (a `2xx`) or answered `spend-ceiling`,
  and one whose run still lives while its answer is out or was a `5xx` or none (that run sends it again). A wake
  answered `4xx`, or whose run ended without a `2xx` (it gave up, was cancelled), is not joined: the run sends its own.
  Its event `runbook.route-wake` says `{"route", "sent": false, "reason": "joined a wake sent within the wait",
  "joined": "<run id of that wake>", "joined_at": "<its instant>"}`, and the run waits as any run that waits. The
  answer to that wake is its answer too: when the gateway answered it (or answers it while the run waits)
  `spend-ceiling`, the run ends `stopped: route` at once, its `runbook.route-wait-ended` naming the ceiling.
- The wake is booked before it is sent, under the lock of the daemon's state, in the run event `runbook.route-wake`:
  `{"route", "sent": true, "day", "cost_micro_usd", "booked_micro_usd", "cap_micro_usd"}`. `day` is the date (UTC) of
  the daemon's clock; `booked_micro_usd` is what the route's wakes of that day that the gateway took or has not
  answered yet add up to with this one. The booking is the journal's, so a daemon started again counts the wakes of
  the day it booked before.
- A wake the rest of the day's cap does not cover is not sent: the event says `"sent": false, "reason": "wake cap
  reached"` with what the day booked so far, and the run waits as any run that waits.
- The gateway's answer is the run event `runbook.route-wake-answered`: `{"route", "status"}` (`status` null when it
  did not answer within 5 s) and `code`, the gateway's error code, when it gave one. A run closed while its wake was
  out (cancelled, ended) takes no event, so its answer is the daemon's journal line `route.wake-answered`
  (`{"run_id", "record"}`, the same record); the view `daemon-events` does not show it.
- A wake books only when the gateway took it: a `2xx` answer (story `a-wake-is-booked-only-when-it-succeeds`). A `4xx`,
  a `5xx` (`target-unavailable` when the pod's start failed, `spend-ceiling`) or no answer within 5 s books nothing;
  the record of its send keeps the figure it held while it was out. A wake not answered yet counts as booked, so two
  runs cannot both pass the cap while their wakes are out. The answer of a run closed before it came books the same
  way: a `2xx` is booked, anything else is not. A wake out when the daemon was killed has no answer and stays booked
  after the restart: the gateway may have taken it, and the cap counts more, never less (decision D11 of the unit's
  coordinator).
- A wake answered with a `5xx` other than `spend-ceiling`, or not answered within 5 s, is sent again 30 s of the
  daemon's clock later (`WAKE_AGAIN_AFTER_MILLIS`, `runbook/src/commands.rs`) while the gateway still gives the model
  as `stopped`, and again after each such answer, but only before the run's wait for its route is over (the least
  `start_wait_seconds` of its routes that cannot answer): none is sent at or past its end. Each try is one more
  `runbook.route-wake` and `runbook.route-wake-answered` of the run, booked and capped as above; a try joins no wake.
  A `4xx` is not sent again.
- The answer `spend-ceiling` (a pod start would pass a cap of the gateway) ends the run at once: `Ended`, outcome
  `stopped: route` (command `EndRouteWait`), and its `runbook.route-wait-ended` names the ceiling:
  `{"route", "phase", "ceiling": "spend-ceiling"}`. Any other answer leaves the run waiting for its route.
- A model in another phase (`starting`, `stopping`), a hosted or a local route, and a route without the wake are not
  woken.

#### The pod's time of a managed route

A managed route that declares `price_per_hour_micro_usd` (a whole number of micro-USD, the figure of the gateway's
own `price_per_hour_usd`, which no route of the gateway serves) books the time its pod ran for its runs at that price
against `wake_daily_cap_micro_usd`, beside its wakes (`docs/model-routes.md`, section 5, D-3; story
`a-managed-route-books-its-pods-measured-time`). Without the key the pod's time books nothing, as before.

- A run uses the pod from its start (`started_at`: the daemon starts a run only once the route was read ready) to its
  end. While a run that started on the route lives, the daemon reads the route's status every 5 s of its clock,
  the first time 5 s after the earliest start of those runs, with the `GET` requests of a submit; a read that the
  pod cannot answer (`stopped`, `stopping`, or any phase but `ready`) ends the use of every such run, and a later
  read of it ready opens it again. Each change is the run event `runbook.route-pod-phase`: `{"route", "phase"}`. Only a
  status the gateway answered gives a phase (`RouteTable::pod_phase`): a gateway that does not answer, is not ready or
  no longer lists the alias, and a status route that fails, change nothing.
- The split rule: the pod's time is cut at every start and end of a run's use, and each piece is split equally over
  the runs that used the pod in it; a millisecond a piece cannot split goes to the runs first in submit order, one
  each (`runbook/src/pod_time.rs`). Two runs that share 30 minutes at 3.00 USD per hour book 0.75 USD each; two that
  overlap 15 of 45 minutes book 15 minutes alone and 7.5 shared, 1.125 USD each. The time after the last run's end,
  while the pod waits for its idle stop, is booked to no run. A run whose pod never became ready books no time, and so
  does a run that takes no route (`model_route_not_taken`). A run the daemon ends `orphaned`, at its own stop or at a
  restart that found it live, used the pod up to that mark: the time no daemon read the pod counts against the cap,
  never less (decision D10 of the unit's coordinator, 2026-11-27).
- Just before a run on the route ends (`EndRun`, a cancel, `EndOrphanedRun`) the daemon writes its share in the run event
  `runbook.route-pod-time`: `{"route", "pod_millis", "share_millis", "price_per_hour_micro_usd", "cost_micro_usd"}`,
  `pod_millis` the time the pod ran while the run used it, `share_millis` its share by the rule, `cost_micro_usd`
  that share at the price, rounded up to the micro-USD.
- The day's cap counts wakes and pod time together: the pod's time inside the day of the daemon's clock (UTC), each
  run's share of it at the price, also of runs that still live (up to now). A wake's `runbook.route-wake` on such a
  route also holds `pod_micro_usd`, the part of `booked_micro_usd` that is pod time. A wake that the rest of the day's
  cap does not cover is not sent (`wake cap reached`) and the run waits as any run that waits; the cap stops no pod
  and no run already on it. The booking is the journal's: a daemon started again counts the same figures.

#### The day's figures of a route

A row of the view `routes` (and a route of the view `catalog`, and of the MCP tool `list_routes`) answers what the
cap of a managed route counts on the day of the daemon's clock (UTC), so a client reads them in one request instead of
from the events of `daemon-events` (story `the-routes-view-answers-the-days-wake-figures`). They are read from the
journal when the view is asked, by the functions the cap of a wake reads (`wake_figures`, `day_booked`,
`runbook/src/commands.rs`), so the view and the cap never differ; the state keeps each route's wake events in an index
(`State::wake_events_of`), so neither walks the whole record.

| Member | When present | What |
|---|---|---|
| `wake_booked_micro_usd` | managed, `wake_daily_cap_micro_usd` declared | what the route's wakes booked today: those the gateway took (a `2xx`) and those still out (above) |
| `wake_cap_micro_usd` | the same | `wake_daily_cap_micro_usd` |
| `wake_cost_micro_usd` | the same, and the cost declared | `wake_cost_micro_usd` |
| `wake_last_sent_at` | the same, and a wake of the route was sent | the `at` of the newest sent `runbook.route-wake` of the route, on any day, whatever its answer |
| `wake_needed` | the same | true when the wake is on (`wake_on_submit`), the phase of the row is `stopped`, a run that takes the route (or a delegation the daemon classifies, on the classifier route) is `WaitingForRoute`, no wake of the route is out (sent, not answered yet, its run live, also past the wait) and none may be joined (a wake within the route's `start_wait_seconds` that the gateway took or answered `spend-ceiling`, or that is out, failed or not answered while its run lives): a run waits and the daemon sends no wake for it, because the wake would pass the day's cap, was answered `4xx`, or the wake the run joined is past its wait. Otherwise false, also with the wake off |
| `ceiling_refused_at` | the same, and a wake of the route was answered `spend-ceiling` today | the `at` of the newest such answer: a `runbook.route-wake-answered`, or the line `route.wake-answered` of a run closed before it came |
| `pod_booked_micro_usd` | managed, `price_per_hour_micro_usd` declared | the pod's time of the day up to now at the price, each run's share, also of runs that still live (above) |

A member that does not apply is absent, as every optional member of the daemon's rows is (a client that indexes the
row reads null). A wake is not sent when `wake_booked_micro_usd + pod_booked_micro_usd + wake_cost_micro_usd` is above
`wake_cap_micro_usd`; its `runbook.route-wake` holds `booked_micro_usd`, the first two added, and `pod_micro_usd`,
the second. The phase is the one of the row (the last read of the gateway, at most 15 s of the daemon's clock old);
the figures are read when the view is asked.

Every row also answers the figures of the route's daily caps on the same day (story
`any-route-kind-takes-a-daily-cap`; `docs/model-routes.md`, section 5). They are read from the figures the relay holds
each call against (`RouteDays`, `runbook/src/route_day.rs`), so the view and the refusal never differ:

| Member | When present | What |
|---|---|---|
| `daily_booked_micro_usd` | every route | what the relay booked on the route today: each call sent today at the cost of its ledger line (its reservation when its cost is not known), counted when its answer has ended |
| `daily_cap_micro_usd` | `daily_cap_micro_usd` declared | the route's `daily_cap_micro_usd` |
| `daily_llm_calls` | every route | the calls the relay sent on the route today, each counted as it was admitted |
| `daily_max_llm_calls` | `daily_max_llm_calls` declared | the route's `daily_max_llm_calls` |

A route of any kind may declare either cap in `runbook.toml` (`docs/model-routes.md`, section 1): a whole number above
0, else the daemon refuses to start and names the key; `daily_cap_micro_usd` only on a route that declares a price
(a route without one books a call's reservation, not its cost, so its calls are capped by `daily_max_llm_calls`).
Before a call is sent the relay holds it against its route's day: it is refused `429 cap-reached`, with `cap_key`
`route.<name>.daily_max_llm_calls`, when `daily_llm_calls` has reached the cap of calls, and with `cap_key`
`route.<name>.daily_cap_micro_usd` when `daily_booked_micro_usd` has reached the spend cap; the cap of calls is read
first. Nothing is sent, and the call is not taken against the run's caps. A call is counted on its route only after
the run's caps took it, so a call its run refuses holds no count of the route's day meanwhile; one the route refuses
at that count is given back to the run's caps. The spend cap holds the booked figure, so
the third of three calls of 1000 micro USD under a cap of 3000 is sent and the fourth is refused; a call is booked
before its caller sees the end of its answer, so no call is sent once the figure has reached the cap, and the last
call sent below it (with any sent beside it) may take the figure above the cap by its cost. The day is the UTC date of
the daemon's clock, the day a wake books on; a call counts on the day it was admitted. Two runs on one route share its
figures. The relay writes that day into each call's ledger line and its `runbook.model-call` (`day`), and a daemon
started again rebuilds the figures from the model-call lines of every ledger under `ledgers/` (a call in flight when
the daemon was killed has no line and is not counted; a ledger that cannot be read refuses every call on a route with
a daily cap). A run whose call a route's daily cap refused ends as a run at a cap ends ("Caps of a run"):
`stopped: llm calls` for `daily_max_llm_calls`, `stopped: spend cap` for `daily_cap_micro_usd`, after the step that
made the call, at once for a call of its decider (it does not wait for a person) and for a classify call of a
delegation (it ends `Ended`, not `FailedToStart`), and its `runbook.cap-reached` names `route` and `cap_key` beside
`cap` and `detail`. An answer with no usage at all books its reservation on a priced route too, so a gateway that
reports no usage still fills `daily_cap_micro_usd`.

### The copy of a context

Before a run starts the daemon copies its context (`context_path` of `submit-run`, `workspace` of `delegate`) into
the run's workspace (`runbook/src/context_copy.rs`). It copies directories and regular files only:

- an entry named `.git` is left out always, a directory or a file (a linked worktree's `.git` is a file);
- an entry of the exclude list is left out and not entered: the daemon's `[delegation] copy_exclude` (default
  `target/` and `node_modules/`), and what the submit or delegation adds in `workspace_exclude` (at most 64
  entries). A run adds entries; it cannot remove one of the daemon's;
- a symbolic link, and anything that is neither a directory nor a regular file, is skipped, so a link inside a
  context root carries no file from outside it into a run. Every directory is opened below the context root one
  name at a time without following a link, and every file through its directory: a directory or a file swapped for a
  link while the copy runs fails the start (`the context could not be copied into the workspace: ...`) and carries
  nothing in.

An entry is a name, matched at any depth (`target/` leaves out `target` and `a/b/target`), or a relative path from
the context root (`docs/big/` leaves out `docs/big` and not `x/docs/big`). One that ends in `/` matches a directory
only; one without it a directory or a file. A name matches a whole name: `targets/`, `target_x` and `node_modules_x`
are copied. An entry that is empty, absolute, holds an empty, `.` or `..` part or a NUL, or is longer than 200
characters, is refused `400` naming `workspace_exclude` and the entry's place, before anything is written.

The copy reads at most `copy_max_entries` entries (default 50,000; each entry it copies or skips counts, an excluded
directory once) and takes at most `copy_max_bytes` bytes of files (default 256 MiB). This repository's checkout
without `.git`, `target/` and `node_modules/` holds 4,212 entries and 32.7 MB (2026-11-27). The bounds are checked
before the first write: a context over one fails the run's start with the `failure`
`context-over-bound: the context is over the bound of <n> entries ([delegation] copy_max_entries); nothing of it was
copied` (or `bytes`, `copy_max_bytes`), and the workspace holds nothing of it. A file that grows past the byte bound
while it is copied fails the start the same way, and what was written is removed. A copy records the run event
`runbook.context-copied`: `exclude` (the list it applied: `.git`, the daemon's entries, then the run's own),
`entries` and `bytes` (what it copied) and `skipped` (how many entries it left out, counted, never listed). The
run's `runbook.submitted` names `workspace_exclude` when the submit or delegation named one.

### A delegation

`POST /daemon/commands/delegate` asks for a piece of agent work on a named model route
(`docs/model-routes.md`, section 8):
`{"request": "<text>", "model_route": "<route>", "protocol": "<runbook>", "workspace": "<dir>", "workspace_exclude": ["<name or path>"], "settings": {}, "session_id": "<id>", "idempotency_key": "<key>", "serial_key": "<key>"}`,
every member but `request` optional. A delegation is a run, at the daemon's default confinement level (a daemon
with none refuses it `400` and writes nothing), with the request as its intake; `cancel-run`, the run views, the
events and the web UI work on it unchanged.

- With `protocol`, that runbook runs (the path `named`). With `protocol` `delegation-free`, or without one on a
  daemon that has no classifier route, the free run (the path `free`): the daemon's own
  runbook `delegation-free`, one agent node (`sdc-node delegation.free` on the claude-cli adapter, the instruction
  `instructions/delegation.free.md`) whose instruction is the request, with the files of the workspace as its inputs,
  writing `result.md`. The daemon writes that runbook under its state directory at every start
  (`runbook/src/delegation.rs`); a `submit-run` does not find it. The free run's agent node reads the request
  whole, as the session wrote it, also when it reads as YAML.
- The agent node of a delegation (the free run's, each action of a generated protocol, `delegation.change`) gets no
  more turns than its run has model calls left. Its bindings give it `--max-turns 40`; as it starts, the daemon
  names in `RUNBOOK_CALLS_LEFT` the run's `max_llm_calls` (the delegation's `settings`, else the daemon's
  `--max-llm-calls`) less the calls the run has used, the classify and build calls included, at least 1, and
  `sdc-node` takes the smaller of the two (`runbook/src/engine/gate.rs`, `Reserving`;
  `driver/src/bin/sdc-node/setup.rs`, `calls_left`). The call's `request.json` names the bound. A cap of 30 gives
  30 turns, 50 gives 40, 1 gives 1; a run with no calls left starts no step (`max_llm_calls` in the table of the
  caps below). So a node that makes one model call per turn ends on its own bound and not on the relay's
  `429 cap-reached`; a call the CLI sends again after a failed answer, or one it makes on its own (a compaction), is
  counted by the relay and is no turn, so such a node can still reach the cap. A node that spends every call left
  ends the run `stopped: llm calls` after its step, its `result.md` written or not: the caps are held before the
  tree is read again. A cap of 0 is refused at submit. Other runbooks' agent nodes keep the `--max-turns` of their
  bindings.
- Without `protocol` on a daemon with `[route_policy] classifier`, the run takes its path when it starts
  (`docs/model-routes.md`, section 9; `runbook/src/engine/classify.rs`). It is submitted under the runbook
  `delegation`, and `RunDelegated` holds no `path`. One Messages call on the classifier route, through the relay as
  the role `classifier` (`instructions/delegation.classify.md`: the request, and the id, title, description and
  inputs of each runbook of the view `catalog`, a runbook another runbook opens as a child included: `idea` when its bindings place one), answers `{"protocol", "confidence", "inputs"}`; at a confidence of
  `[delegation] classify_threshold` (default 0.8) or above, when that runbook takes every input the answer's `inputs` names, it runs with the request as its intake
  (`classified`); an answer whose inputs it does not take goes on to the build call. Below it a second call
  (`instructions/delegation.build.md`) answers a `protocol/1` document and its `loom.bindings/1` as YAML texts (or
  `{"free": true}`, the free run): the daemon runs it (`generated`) only when every action is the agent node
  `sdc-node delegation.free`, every evidence is read from the exit, and nothing but `format`, `artifacts` and
  `actions` is named (an action may carry `instruction`, the text of its own step: the daemon writes it to
  `work/delegation/steps/<action>.md` and runs the agent node with `--step` on it and
  `instructions/delegation.step.md`); it writes each executor's arguments, exits, budget and timeout from the free run's, puts the
  artifact `idea` at `request.yaml`, and checks the result with the loader of a runbook (the layout, the budgets,
  Loom's reader of the protocol directory, the opening of the tree). Both calls are the run's: taken against its
  caps, booked in its ledger and recorded as its events `runbook.model-call`. An answer that is not the JSON asked
  for is asked again, with the validator's error, at most twice. A cancel is read before every classify, retry and
  build call: none goes out after it, and the run ends `Cancelled`; a call in flight at the cancel is booked in the
  ledger and, the run being closed, is said on the daemon's standard error rather than as an event. The run then fails to start with `failure`
  `classifier-answer: ...`; a call the route does not serve `route-unavailable: ...` (`cap-reached: ...` past a
  cap); a generated protocol the loader refuses `protocol-refused: <the loader's line>`. The run event
  `runbook.delegation-resolved` records `path`, `protocol` (the id the protocol declares), `confidence`,
  `classifier_route` and, for a generated protocol, `protocol_digest` (`sha256:<hex>` of the document); the
  classifier's answer (`classify.json`) and the generated documents (`protocol.yaml`, `bindings.yaml`) are products
  of the run under `work/delegation/`. The view `run` (and the MCP tool `read_run`) shows those members for every
  delegation's run.
- The route is chosen as a run's (above, "The route of a run"): `model_route`, by the name the daemon declares it
  under (`[routes.<name>]`, above, "Model routes"); none named: `[route_policy] default` (source `default`). The
  refusals of a named route are the run's: `RouteRefused` (422 `route-refused`, `run_id`, `reason`, `declared`) for
  a route the daemon does not declare, an alias, an address, a token or an empty name, and for none named on a
  daemon without a default route; nothing is written. A delegation needs no model decider: it always takes a
  route. The run record, its views and `runbook.submitted` carry `model_route`, `model_route_alias` and
  `model_route_source` as a submit's do, and every model process of the run reaches the route through the relay as
  the role `default` (below, "The relay of a run's model calls").
- The route takes the readiness check of a run (above, "A run starts only on a route that can answer"), before
  anything is written: a gateway that is not ready or does not answer refuses the delegation `503`
  `route-unavailable`, naming the route and never its address or token; an alias it does not list refuses it `422
  route-refused`; a model that is not callable leaves the run `WaitingForRoute` until it can answer or its
  `start_wait_seconds` are over. A delegation whose path is still open (it names no protocol on a daemon with a
  classifier route) takes the same check for the classifier route: no classify call goes to it before it can answer.
  A replay of an idempotency key is not asked about.
- `caller` and `caller_label` (each at most 200 characters, else `400`) say who asked: `runbook mcp` fills them
  with the name the MCP client gave in `initialize` and the session's `RUNBOOK_CALLER`; they are no argument of
  the MCP tool. The run records them in its event `runbook.delegated` (not in `RunDelegated`).
- A replay (`202` outcome `already-submitted`) names the first run with the key. Its `Location` is that run's
  `delegation-result`, or `/daemon/views/run?run_id=<id>` when the key was a `submit-run`'s.
- A `result.md` that comes with the workspace is moved to `.result.from-workspace.md` before the run starts
  (a dot name, which the products leave out): the result is only a `result.md` the run writes.
- The accepted answer, `202` outcome `delegated`, publishes `RunSubmitted` (`published[0].payload.run_id` is the
  run) and `RunDelegated` (`route`, `path`, `protocol`), and the `Location` header names the run's result:
  `/daemon/views/delegation-result?run_id=<id>`. The run holds two events before the runtime records any:
  `runbook.submitted` and `runbook.delegated` (`route`, `path`, `protocol`).

`GET /daemon/views/delegation-result?run_id=<id>` answers one row: `run_state`, and once the run ended its
`outcome` (and `failure`: why it failed to start, the reason of `runbook.runtime-failed`, or for the outcome
`failed: step <action> <end>` the reason of `runbook.step-failed`, which names the step and how it ended); `route`, `path`, `protocol` (both absent while the run classifies); `confidence`
(`null` for `named` and `free`), `classifier_route` and `protocol_digest`; `result_text`,
the text of `result.md` of the run's `work/` directory read as the view `product` reads a file (absent while there
is none, and for a file that is not text), `result_truncated` and `result_limit` (1,048,576 bytes); `products`, the
paths the view `products` lists; `calls`, `input_tokens`, `output_tokens` and `cost_micro_usd`, the sum of the
run's events `runbook.model-call`; `cost_known`, false when any call was booked at its reservation. A run id the
daemon does not know is `404` `no such run`; a run no delegation created is `404` `not a delegation`.

`runbook delegate [--route <r>] [--protocol <p>] [--workspace <dir>] [--repository <name>] [--set K=V] <request-file|->` sends one
delegation and prints the run's id (`--json`: the route's answer); the MCP tools `delegate` and
`delegation_result` are in [`mcp.md`](mcp.md).

### A delegation that changes a repository

The first slice of story `a-delegation-changes-a-repository-on-its-own-branch` (its `## Decision`;
`runbook/src/handback.rs`). A delegation may name `repository`: a repository the operator lists in the daemon's
`runbook.toml`, by the name of its entry, never a path. The list is empty by default:

```toml
[repositories.demo]
path = "../demo"                      # the operator's checkout; relative to the file
base = "main"                         # the revision a run's branch starts from
remote = "https://forge.example/org/demo.git"   # https://<host>/... or file:///..., no '@' in it
default_branch = "main"               # never pushed to; a branch outside runbook/
commit_name = "runbook-bot"           # the identity of the daemon's commit
commit_email = "runbook-bot@example.invalid"
push_token_file = "demo-push-token"   # optional; relative to the file; a one-line token
deny_paths = [".github/", ".gitlab-ci.yml", ".gitmodules"]   # optional; this list is the default
```

A key the table does not have, a remote with an `@` anywhere (a user or password hides behind one, also after a
third slash) or with no host, a `base` or `default_branch` that is an option, a `deny_paths` entry that is not a
path as git names one (absolute, `.`, `..` or an empty segment such as `src//`), or a `push_token_file` that is
no readable one-line token refuses the start, exit 2, naming the entry and the key.
A file found by a program of a run (not named with `--config`) may not list repositories.

- The delegation takes the path `change` and the daemon's protocol `delegation-change` (written beside
  `delegation-free` under the state directory, `runbook/src/delegation.rs`): one agent node (`sdc-node
  delegation.change`, the instruction `instructions/delegation.change.md`) whose instruction is the request. It
  needs the daemon's default level to be `local`.
- `RepositoryRefused` (422 `repository-refused`, `run_id`, `reason`), before anything is written: a name that is not
  an entry (a path is never one), a `protocol` other than `delegation-change` beside it, a default level that is not
  `local`, or a `base` that names no commit of the checkout. It comes after `context-outside-roots`.
- At the submit the daemon resolves `base` to a commit; the event `runbook.delegated` records `repository` and
  `base` (that commit). `RunDelegated` keeps `route`, `path`, `protocol`.
- When the run starts, before its runtime does, the daemon makes the run's worktree T with the host's `worktree`
  command: `worktree create --repo <path> --base <commit> --id runbook-<run-id> --purpose "runbook run <run-id>"`
  (T is the last line it prints), then `worktree hook session-start --session runbook-<run-id> --path T`. It records
  T, T's `.git` file and the worktree's git directory in `worktree.json` of the run's directory; no view and no
  event names T's path. A tree that cannot be made fails the start `repository: worktree create: ...`.
- The agent node works in T: every program of the run gets `RUNBOOK_WORKTREE=T`, and `sdc-node
  delegation.change` runs the model with T as its working directory, copies nothing into it and writes nothing of
  the call into it (the call's record stays under `work/.sdc/llm/`). Its result, `result.md` of the run's
  workspace, is the model's last reply, its summary.
- After the outcome `delivered`, and before the run ends, the daemon hands the change back: it refuses when T's
  `.git` file differs from the one it recorded, when T holds no change, when a changed path is in `deny_paths`
  (a path ending in `/` denies what is under it; another path itself and what is under it), or when the change
  adds a symbolic link whose target is absolute or climbs above T. Otherwise it commits T's change (`git --git-dir
  <recorded> --work-tree T`) as one commit on the base under `commit_name` and `commit_email`. It pushes from a
  bare repository of its own in the run's directory (no configuration but the daemon's, the checkout's objects
  through an alternate, removed after the push), so the checkout's own configuration (URL rewrites, push URLs,
  http settings) does not steer the push. It asks the remote first (`git ls-remote`): a branch `runbook/<run-id>`
  there already refuses the hand-back. Then it pushes with `git -c http.followRedirects=false push <remote>
  <commit>:refs/heads/runbook/<run-id>`: no `+`, no `--force`. Every git call of the daemon runs with
  `GIT_CONFIG_GLOBAL=/dev/null`, `GIT_CONFIG_NOSYSTEM=1`, `GIT_TERMINAL_PROMPT=0`, `core.hooksPath=/dev/null`, no
  credential helper and no signing: the operator's identity, hooks and credential helpers are never used. Each git
  and `worktree` call is killed after 120 s, with its process group, and the hand-back is refused with that reason.
  The push credential is `push_token_file`: git asks the `runbook` binary itself as its askpass (`GIT_ASKPASS`, with
  `RUNBOOK_ASKPASS_FILE` naming the file and `RUNBOOK_ASKPASS_ORIGIN` the remote's `https://<host>`), which reads the
  token then and answers only a prompt for that host; the token is in no argument, no log, no file of the run and no
  environment of a process of the run.
- Right before the push, under the state's lock and only while the run is live and not cancelled, the daemon writes
  the run event `runbook.pushing` (`repository`, `branch`, `commit`). A cancel that comes before it: nothing is
  pushed. A cancel accepted after it does not stop the push that runs; that event names the branch.
- The run event `runbook.handed-back` (`repository`, `branch`, `commit`, `base`, `tree_finished`) or
  `runbook.hand-back-refused` (`repository`, `reason`, at most 300 characters) records it; the run's outcome stays
  `delivered`. After a push the daemon ends the lease (`worktree hook session-end`), then calls `worktree finish
  runbook-<run-id>` and `worktree gc --apply --repo <path> --id runbook-<run-id>`.
- A run that ends any other way (a failed step, a cancel, a cap, a runtime failure) commits and pushes nothing: the
  daemon ends the lease and keeps T for inspection.
- The view `delegation-result` adds `repository`, and once the run delivered `branch` and `commit`, or
  `handback_refused` (the reason). A run cancelled while its push ran shows the `branch` and `commit` of its
  `runbook.pushing`. The view `catalog` lists the path `change` when the file lists a repository.

Not in this slice (each comes back to the operator first): the draft merge request and any forge client or forge
token, the level `linux` and its Loom request, the hand-back as a step the bug-fix runbook shares, a base a
delegation chooses, a fetch before the worktree is made, and the removal of a kept tree after `keep_failed_days`.

### The catalog

`GET /daemon/views/catalog` (the MCP tool `describe_catalog`, the command `runbook catalog [--json]`) answers one row
with what a delegation may name on this daemon (`docs/model-routes.md`, section 10):

- `routes`: each declared route, in order of name, as the view `routes` reads it (`route`, `gateway` by name,
  `alias`, `kind`, `wires`, `status`, `phase`, `read_at`, and `max_model_len` and `tool_calling` when the gateway's
  `GET /v1/models` gives them for the alias), with what the daemon's file declares beside the address:
  `price_in_micro_usd_per_mtok`, `price_out_micro_usd_per_mtok` and `start_wait_seconds`, each when declared, and the
  day's figures of the route as the view `routes` answers them ("The day's figures of a route").
- `runbooks`: each row of the view `runbooks`, with `title` (the root protocol's title, else the first sentence of its
  description), `description` (at most 2000 characters), `inputs` (`idea` when the root bindings place it: the request
  is the run's intake), and the admission of its route entries.
- `delegation`: `members`, the members of `delegate` a client sends as the OpenAPI document gives them (`name`,
  `required`, `summary`, `max_length`); `paths` (`named` when a runbook is listed, `free` when the daemon runs the free
  run, and on a daemon with a classifier route `classified` when a runbook is listed and `generated` when it runs
  the free run); `default_route` and `classifier_route` (`[route_policy]`); `confinement`, the daemon's default level, which
  every delegation runs at; `request_limit`, 1,048,576 bytes.

A runbook's route entries are the file `bindings/<runbook>.routes.yaml` beside its bindings (format
`runbook.routes/1`, Runbook's own file, not part of Loom's bindings):

```yaml
format: runbook.routes/1
model_routes:
  writer: {route: qwen}
  review: {needs: {wire: messages, min_context: 100000, tool_calling: true}}
routes_allowed: [qwen, claude]
```

An entry that names a route is admitted when the daemon declares it and `routes_allowed`, when given, holds it. An
entry with `needs` (`wire`, `min_context`, `tool_calling`, `kind`, each optional) is admitted with the daemon's
default route when that is allowed and meets them, else with the one allowed route that meets them; none or several
is a refusal. A need is checked against what the gateway listed for the route's alias (`wires`, `max_model_len`,
`tool_calling`) and the route's `kind`; a fact the gateway did not give meets no requirement. Each entry is listed
with `role`, `route` (the one that serves it), `needs`, `admitted` and the `reason` of one that is not; a runbook
whose entries cannot be admitted is listed with `admitted: false` and `refusal`, every reason joined by `; ` (or what
is wrong with the file as a whole: not `runbook.routes/1`, an unknown member, a link, more than 64 KiB). A reason
names the role and the route or the requirement; an address, a token or another text that is not a name is never
echoed. A runbook with no such file is admitted with no entries: every declared route may serve it. The catalog
admits against the default route; a submit, a delegation that names the runbook and a delegation whose runbook the
classifier picks (once it is picked, before its first model call) admit against the run's own
word first and refuse what cannot be admitted (above, "The route of a run"). The routes a picked runbook's entries
give take the readiness check of a run's routes before its first call: a model that cannot answer yet holds the
run up to the route's `start_wait_seconds` (the run event `runbook.waiting-for-route`; the run stays `Running`), and
past it, or on a gateway that does not answer or is not ready, the run ends `FailedToStart` with
`route-unavailable: ` and the route (an alias the gateway does not list: `route-refused: `). The run's record then
names the routes it took (`model_route`, `model_route_alias`, `model_route_source`, `model_routes`).
No address, token, token file or path of the host is in the row.

### The products of a run

The `work/` directory of a daemon run is its workspace, `<state dir>/workspaces/<run id>/`, which the
cases of the run share. The views `products` and `product` (and the MCP tools `list_products` and
`read_product`, [`mcp.md`](mcp.md)) list and serve the files under it, through one reader
(`runbook/src/engine/products.rs`, `docs/engine.md`, "The products of a run"). A path in both is
relative to `work/` (`draft.txt`, `notes/a.md`), never absolute and never a path of the host.

`products` answers one row: `files`, the regular files with one name by path, each with `size` (bytes)
and `modified_at`; `hidden`, the entries whose name starts with a dot (a tool's cache or login: neither
listed nor entered); `left_out`, the links and the files with more than one name (neither listed nor
served); and `cut` when the walk stopped at a bound, the inspector's: `files` (500), `directories`
(2000), `entries` (20000) or `depth` (6 levels). A directory is read an entry at a time, so one with a
million entries costs no more than the bound.

`product` answers one row for the file at `path`: `kind` `text` with `text`, the first 1,048,576 bytes
(`limit`) as UTF-8 (a partial character at the cut is dropped), or `binary` (a NUL byte or a sequence
that is not UTF-8) with no text; `size`, `shown` (the bytes of `text`), `truncated` (true when the file
holds more than `limit` bytes) and `modified_at`.

The guards are those of the inspector's file route, held through directory descriptors: the `work/`
directory is opened without following a link at its name, each directory on the path relative to the one
before with `O_NOFOLLOW | O_DIRECTORY`, the file with `O_NOFOLLOW | O_NONBLOCK`, and the type and the
link count are read from the open file before a byte is read. The file is read in the daemon's process;
no program opens it. A refusal is the daemon's `{"refused", "message"}` answer, names the path as it was
asked, and holds no byte of a file and no path of the host:

| Status | `refused` | When |
|---|---|---|
| 400 | `bad path` | the path is empty, absolute, holds a NUL or a backslash, or has a `..` component |
| 404 | `not served` | a component starts with a dot |
| 404 | `no such file` | nothing is at the path |
| 403 | `link` | a link lies at a component of the path (also one that points into `work/`: no link is followed) |
| 403 | `more than one link` | the file has a second name (a hard link may be a file outside the run) |
| 415 | `not a regular file` | a directory, a pipe, a socket or a device: never opened |
| 404 | `no such run` | the daemon has no run of that id (both views) |
| 400 | `bad request` | the query lacks `run_id` or `path` |

Both are reads: they sit behind the one guard (`runbook/src/api/auth.rs`) as every view does, so a
request without the token, with a foreign `Host` or `Origin`, gets the answer any other read gets, and
the cookie of the web UI's login opens them. The specification declares the entities `RunProducts` and
`RunProduct`, which no command creates, as it does `Runbook`: the daemon fills the rows from the host
when asked.

### The case tree of a run

The view `cases` answers a row per case of the run's tree, in the order the runtime lists them: `case_id` (the case's id
in the tree), `parent` (absent for the root), `name` (the link name the opening action gave the case; absent for the
root), `protocol` (the protocol id of the case's bindings, `engine-draft`), `case_state` and `outcome` (absent while the
case has not ended). No path of the store or of a case file is in a row. The Loom runtime reads the run's store with
Loom's tree status (`docs/engine.md`, "The case tree of a run"), which does not hold the store, so the view answers
while the run goes on; the stub runtime has no tree, and a run of it has no row.

`case_state` is where the case stands when the daemon reads the tree:

| `case_state` | When |
|---|---|
| `Ended` | the case has an outcome |
| `SpendCapReached`, `CallCapReached`, `WallCapReached` | the daemon ended the run at that cap: the run is `Ended` with `stopped: spend cap`, `stopped: llm calls` or `stopped: wall time` ("Caps of a run"), whatever the case's last stop; the case did not get stuck, its run was ended. Limit: a tree left without a move by the step that reaches a cap shows the cap word too, since telling it from a tree with a move left needs Loom to evaluate without a step |
| `AwaitingPerson`, `AwaitingAuthority` | its last stop is that one and the run is `WaitingOnDecision` |
| `Stuck` | its last stop is `Stuck` (no move) or `BudgetSpent` and the run ended by its tree (`Ended`, not `orphaned`, not at a cap); a wait or a cancel that ended with the run, or an orphaned run, leaves the case `Open` |
| `Running` | the run's step in flight (`docs/daemon-api.md`, "The step in flight") names the case |
| `Open` | none of these |

The member is `case_state`, not `state`: in the specification `state` is an entity's lifecycle, and no command moves a
case (the entity `RunCase`, like `Runbook`, is filled from the host when asked). A run id the daemon does not know is
`404` `no such run`; a tree that cannot be read at that moment (Loom refused the read) is `503` `tree not readable`, with
no word of Loom's reason: ask again. The view is a read behind the one guard, like every view.

### The metrics of a run

`GET /daemon/views/run-metrics?run_id=<id>` answers one row of what the run took, read from the run's own events in one
walk (`runbook/src/run_metrics.rs`). A number the events do not hold is left out of the row, never written 0.

- The run: `run_id`, `run_state`, `outcome` (once it ended), `submitted_at`, `started_at` (absent for a run that never
  started), `ended_at`, and `duration_ms` (start to end) for a run that ended, or `running_ms` (start to the daemon's
  clock when the view is read) for a live run.
- `steps`: a row per step, in the order the steps first started. A step is one action of one case in one role
  (`program` or `check`), over all its tries: `case_id`, `action`, `role`, `tries`, `step_state`, `started_at` (its
  first start), `ended_at` and `exit` (of its last try), `duration_ms` (its tries' times summed, once every try has an
  end) or `running_ms` (a running step: since its last try started), and `calls`.
- `step_state`: `ended` when Loom's line `ActionFinished` (a program) or `HandoffChecked` (a check) of the action followed
  its last start; `running` when it has none yet, the run is `Running` and the step is the newest to start; `unfinished`
  otherwise (a cancel, an orphaned run, a try that a newer step overtook).
- `calls` of a step and `totals` of the run: `model_calls`, `input_tokens` and `output_tokens` (the sum of what the calls'
  answers reported; absent when calls were made and none reported any, 0 when there was no call), `calls_without_tokens`,
  `cost_micro_usd` (the calls of known cost only), `unknown_cost_calls` and `booked_micro_usd` (what the spend ledger
  booked, reservations of calls of unknown cost included). A relay call (`runbook.model-call`) is the newest step's whose
  action (else case) its `step` names; the calls of `runbook.cost-booked` (a run on no route, booked from its programs'
  cost records) are the newest program step's, and count as calls without tokens and of unknown cost, since that event
  says neither. `calls_outside_steps` counts the calls of `totals` that no listed step holds: the decider's, a
  delegation's classify and build calls, and those of a step that is not listed.
- The bound: at most `step_limit` (500) steps are listed; `steps_left_out` counts the rest, and their calls stay in
  `totals`. The walk reads the run's events through the daemon's index of them, never the whole record, so its time is
  linear in the run's own events.

The times are the daemon's: a start is the `at` of `runbook.action-started` (before the program starts), an end the `at`
of the event that recorded Loom's line. A run id the daemon does not know is `404` `no such run`; a query without
`run_id` is `400`. The page's pane for the row is `ui/daemon/metrics.js` (embedded like every file of `ui/daemon/`):
the run page has the tab Metrics when the view answers a row for the run on screen, and none when it refuses or answers
no row (`tabsOf` in `ui/web/source.js`); the pane says the row's times in the page's clock words (`ui/web/clock.js`)
and its durations, counts and costs in the step list's (`ui/web/runsteps.js`). The page's step list takes a step's model calls, tokens and cost from the row
where the run's events name none (a run on no route books them in `runbook.cost-booked`; `withViewCalls` in
`ui/web/runsteps.js`) and says a count the row leaves out as not known, never 0. This view gives no count per route:
the view `routes` answers today's calls and booked spend of each route (`daily_llm_calls`, `daily_booked_micro_usd`,
"The day's figures of a route").

### A request read from its case

A person action whose case holds a decision request in the contract's shape
(`sdc.decision.DecisionRequest`, `docs/decisions.md`) opens a request whose `question` and `options` are
the case's and whose rows carry the member `artifact`: `recommended_option` and `recommended_answer` (each
where the case names one), `recommendation_strength` (`none` to `strong`), `grounds` (each with its
citation) and `free_answer`. Where the bindings name a question member for the action instead, the
`question` is that member of the subject and the `options` are the results the action declares, each with
the result as its letter; `artifact` then has `recommendation_strength` `none`, no grounds and
`free_answer` false. The engine reads both and writes neither; how it finds them is `docs/engine.md`, "A
request read from the case". A request that read nothing has no `artifact`, and its `options` keep the
shape `{answer, effect}` of a request the runtime builds itself: the specification's
`List<sdc.decision.DecisionOption>` describes the options of a request read from its case. The journal
writes `artifact` only for a request that has one, and a journal written before the member existed loads.

### The step in flight

A step of a run (a lint, a ship) can take twenty minutes and more, and the events of the run say
nothing while it runs: Loom writes a line when an action finishes. So the runtime (the engine,
`--runtime loom`) tells the daemon before each bound program starts, through the hook Loom gives
`runbook run` too (`runbook/src/engine/announce.rs`, the `run_child` of the runner), and the daemon
records the run event `runbook.action-started` before the program starts. The record has `case`
(Loom's `LOOM_CASE` of the program), `action` (the action whose argv is the program's, when exactly
one is; when the bindings files of two runbooks of the home bind that argv, the one of the file whose `protocol` is the
protocol of the case), `role` (`program`, or `check` for a `handoff_when` check of the action) and `try` (the nth
start of the action in the case, for a program); a member that is not known is absent. One event
for each program, in the order the programs start, between the `ActionFinished` lines of the run
log; an action that runs no program (an `open` or a `person` action) has none.

While the program runs, the view `run` (and the MCP tool `read_run`) has one more member,
`step_in_flight`: `action`, `case_id`, `started_at` (the `at` of the event) and `seconds` (whole
seconds since, by the daemon's clock, read with the view). It is absent when no program runs: between
steps, while the run waits on a decision, after a cancel, when the run is over, and after a restart
of the daemon (a run that was live is ended as `orphaned`; the event stays in its events). The step
is kept in the daemon's process and not in the journal, which holds the event only. The language has
no command that sets `step_in_flight`, so the specification declares the member of `Run` and states
where its value comes from, as for `Runbook`. `step_in_flight` is a new optional member of a run
row: a client that validates rows strictly against the schema of 0.9.3 refuses a row that has it and
must take the new schema. The stub runtime runs no program and tells nothing.
The daemon page reads the views `runs` and the event stream and not `run`; it shows the event as
any other `runbook.*` event and not the step (a separate change).

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
`runbook.run-orphaned` (at the next start, or at the stop of a daemon that ends with runs live: see "Stopping the daemon"), `runbook.answer-undeliverable` (an answer the runtime could not take up),
`runbook.runtime-failed` (before the run ends with the outcome `runtime-failure`),
`runbook.step-failed` (record `case`, `action`, `end`, `exit`, `signal` and `timeout_seconds` (each of the last three
when it applies), `reason`: before a run ends with the outcome `failed: step <action> <end>` (`<end>`: `exit <n>`, `signal <n>`, `timed out after <s> s` or `ended with no exit`), when the last program of its tree failed (it ended with an exit other than 0, was killed at its timeout or ended with no exit status, or Loom refused it for an exit its `exits.error` lists or a signal, an agent node or any other program) and the tree has no other action to take;
the views `run` and `delegation-result` give its `reason` as `failure`, as they give the reason of `runbook.runtime-failed`) and
`runbook.action-started` (before a bound program starts; "The step in flight"). Each is declared
in the specification by the command that writes it, in its summary: the language cannot write a
run event from a command, so no row of the interpreted target stands for them. The runtime
`--runtime loom` writes one more of its own, `runbook.pinned` (record `runbook`, `protocol`,
`revision`, `bindings_sha256`, `copy_sha256`: the runbook the run copied at its submit,
`docs/engine.md`, "The run's copy of the runbook"); the specification does not declare it. It writes
`runbook.step-ended` too, also undeclared (record `case`, `seq`): the engine runs the tree one step
at a time, so Loom ends every step with a run-log line `Stopped` whose `stop` is `BudgetSpent` and
whose `budget` is `MaxSteps`, written to the log of the case named by `case`, at `seq`; the engine
takes that stop as the end of one step and goes on, and records `runbook.step-ended` after the line
and before the next run-log line. A `Stopped` line with no `runbook.step-ended` after it is a wait or
the end of the tree. A run ends by its state and outcome (`Ended`, `Cancelled`), never by a
`Stopped` line: a run that a cap ends after a step has the step's `runbook.step-ended` and then its
outcome (`docs/engine.md`, "Caps").

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
              [--default-confinement local|linux] [--autonomy A0..A5] [--decider KIND] [--token-file FILE] [--ui] [--mcp]
              [--drain-seconds SECONDS]
              [--runtime loom --runbook-home DIR
               --max-wall-seconds N --max-llm-calls N --cap-micro-usd N
               [--max-wall-seconds-ceiling N] [--max-llm-calls-ceiling N] [--cap-micro-usd-ceiling N]]
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
| `--context-root` | none | A directory a run's `context_path` (a delegation's `workspace`) may lie under; repeatable. It adds to the `context_roots` of `runbook.toml` (see "Context roots"). With none from either, any `context_path` is refused. |
| `--runbook-dir` | `protocol/` of the located checkout or install | Where a submit may find a runbook besides `sdc` and `sdc-bugfix`. |
| `--min-confinement` | `linux` | The weakest level a submit may ask for; a weaker one is refused `BelowConfinementFloor`. The level is stored with the run; nothing is confined yet. |
| `--default-confinement` | the `confinement` of `runbook.toml` (`local` is the level `local`, `substrate` is `linux`), else none | The level of a submit that names none. The option wins over the file. It is never the floor: a default below `--min-confinement` stops the daemon at start, exit 1, with a message that names the two values and nothing else of the file. With none, a submit without a level is refused `400` by name and writes nothing. |
| `--autonomy` | none | The autonomy level (`A0` to `A5`) of a submit that names none. With none, such a run has no level and every decision of it waits for a client. It needs `--decider`, or the daemon does not start (exit 1). |
| `--decider` | none | The decider kind of a submit that names none: a kind of `[deciders.<kind>]` in `runbook.toml` (`rule`, `model`). A name that is not configured stops the daemon at start. Both need `--runtime loom`: the stub has no decider. |
| `--token-file` | none | File that holds the bearer token. |
| `--ui` | off | Serve the web UI, compiled into the binary, beside the API (see "The web UI"). Without it `GET /` is 404. |
| `--max-wall-seconds`, `--max-llm-calls`, `--cap-micro-usd` | none, each required with `--runtime loom` | The caps of a run that names none (see "Caps of a run"). A daemon started without one refuses to start, exit 1, naming each missing option, before anything is bound or written. There is no binary default. A value is a whole number of 1 or more. The stub takes none. |
| `--max-wall-seconds-ceiling`, `--max-llm-calls-ceiling`, `--cap-micro-usd-ceiling` | the cap itself | The most a submit may name for that cap. A ceiling below its cap is refused at start, naming the cap. |
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

### Context roots

A run's `context_path` and a delegation's `workspace` must lie under a context root of the daemon, or they are
refused `context-outside-roots`. The roots are the union of the `context_roots` of `runbook.toml` and every
`--context-root`: the option adds to the file's list and replaces none of it (story
`context-roots-are-keys-of-runbook-toml`).

```toml runbook.toml
context_roots = ["ctx", "/srv/agent-contexts"]
```

- The key is a list of at most 64 paths as text. A relative path is taken against the directory of the file, as a
  `token_file` is, never the working directory; `~` is not expanded. A relative `--context-root` is taken against the
  working directory, as before. A file that a program of a run finds by the walk has no `context_roots`.
- `runbook serve` prepares the roots after its security floor holds (the principal, the token, the listen address) and
  before it opens its state directory, and names each on standard error with its source:
  `runbook: context root <path> (context_roots of runbook.toml)` or `(--context-root)`.
- A root that does not exist is created with mode 0700, its missing parents too, and the line ends `: created with
  mode 0700`.
- A root that exists is never changed. One with a group or other write bit stops the start: `context_roots <path>: is
  writable by group or others (mode 0777)`. One wider than 0700 without those bits is taken and named:
  `... has mode 0755, wider than 0700: it is not changed`.
- A path that exists and is no directory stops the start (`context_roots <path>: is not a directory`), and so does a
  root that cannot be read or created, with the OS error (`... cannot be created: Permission denied (os error 13)`).
  The line names the key, or `--context-root` for a root of the option, and the path, and the daemon exits 2 as it
  does for a refusal of `runbook.toml`, before anything is bound or written into its state directory.
- Each root is canonicalized once; a root named twice counts once. `runbook config` prints the file's list with its
  origin (`context_roots = [...]  (file <path>)`).
- `Daemon::start`, the library's start, still refuses a root that is missing: creating one is the act of `runbook
  serve`.

## Caps of a run

A run of the Loom runtime is never unbound: it has a wall time, a number of model calls and a spend
cap (`runbook/src/caps.rs`, `runbook/src/engine/gate.rs`).

Where a request names them. `SubmitRun` has no member for a cap (the request is generated from
`ess/domains/daemon.yaml` and gains none), so a submit names them in its `settings`, as it names
anything else: `max_wall_seconds`, `max_llm_calls` and `cap_micro_usd`, each a string of a whole number
of 1 or more (`"max_llm_calls": "50"`; every value of `settings` is a string, over HTTP and through the
MCP tool `submit_run`). A cap the submit does not name is the daemon's option of that name. A cap over the
daemon's ceiling is refused at submit as an invalid request (`400`), naming the cap, the value and the
ceiling, and no run is made. A value that is no whole number of 1 or more is refused the same way, and
so is a value of another JSON type than a string (`"max_llm_calls": 50` is a number): the refusal names the
cap and says a cap is a string. A
submit may name a lower cap or a higher one, up to the ceiling. The caps of a run are stored with it
in `settings`, so the run view shows the three that applied, and a run taken up again has the same ones.

A submit names no gateway, address or token for the model programs: the daemon's operator names the gateway
in the daemon's `runbook.toml` (`docs/engine.md`, "A gateway"). A setting or a member of the request named
`gateway`, `base_url`, `token`, `token_file` or `gateway.<member>`, in any case, is refused as an invalid
request (`400`) naming it and never its value, and no run is made.

What each cap stops. Before each step of a run the engine holds the use against the caps; a run that
has reached one ends with an outcome and an event `runbook.cap-reached` (`cap`, `detail`), the same as
any run:

| Cap | Outcome | Rule |
|---|---|---|
| `max_wall_seconds` | `stopped: wall time` | Counted from the start of the run, waits on a decision included. A step that is running, or a wait on a decision, is ended when the time is up; what the run asked is withdrawn. A cancel by a client wins over the cap. |
| `max_llm_calls` | `stopped: llm calls` | A step is not started when the calls made reach the cap. A call is a cost record the bound programs write (`.sdc/llm/calls.jsonl`). |
| `cap_micro_usd` | `stopped: spend cap` | A step is not started when this run's spend has reached its cap, and a program is not started when the reservation for it is less than it may cost. |

Spend. The cap is the run's own: it is the most that one run spends, and what other runs of the
runbook spent, or hold while they run, never reduces it. Spend is booked in a ledger under the state
directory, `ledgers/<runbook id>.json`, one file per runbook with each entry and each reservation
keyed by the run id: the module of `runbook run` (`runbook/src/ledger.rs`), with the cost reader and
the call-budget reader of that command. A run's admission and its reservations are held against its cap
less what it has spent and holds itself. A reservation of a run that no longer lives counts against no
run: it is keyed by that run's id, and a run that starts again under its id drops what its id held.
What a program reserves as it starts is what it may cost, never the whole cap. A run is never
unbound in money:

- A program that names its per-call budget (`--max-budget-usd <v>` or `--max-budget-usd=<v>` in its
  own arguments, the largest when it names it more than once) reserves that budget. The arguments come
  from the bindings the run reads (the root's bindings file and the children it names) and nowhere
  else.
- A binding states the per-call budget of its programs without the argv, for a program that does not
  take the option (`docs/loom-bindings.md`, "The per-call budget of a program"): the member
  `max_budget_usd: <number>` of a program action (an action with `executor.argv` or `arguments_from`),
  and the same member at the top of a bindings file for every program action of that file that names
  none itself; it does not reach a child file an `open` action names (each file names its own). It is
  never passed to the program; `0` says the program calls no model and reserves nothing. The order:
  the argv's budget, then the action's member, then the file's member, then the largest budget below.
- A program whose budget none of these names may call a model all the same: it reserves the largest
  per-call budget the runbook's own bindings name (in an argv or as a member), root and children. A
  step near the cap whose program makes no model call may therefore be stopped; a program that names
  `--max-budget-usd 0` or `max_budget_usd: 0` reserves nothing.
- A runbook with a program whose budget no source names and no budget in any of its bindings is
  refused at submit as an invalid request (`400`), naming the reason and the action; no run is made. A
  runbook whose every program has a budget from one of the sources needs no budget elsewhere. Refused
  the same way: a budget that is no number of USD of 0 or more (`abc`, `-1`, `NaN`, `inf`, nothing
  after the option; for a member also text such as `"0.3"`, `null`, `true`, a list), naming the action
  and the bindings file, or the bindings file for its own member; an action whose argv and member name
  two different budgets, naming the action; a member on an action that runs no program. A run checks
  the same on its own copy before it opens; the copy's files hold no member (Loom refuses a member it
  does not know) and the members are kept beside them in `bindings/.call-budgets.json`, inside the
  copy's digest.
- A program a person's answer starts (a person action whose binding names an `argv`) reserves the
  same way before it starts and is booked after, also when the run ends right after it (a wall cap
  spent while it ran); when its reservation is short it does not start and the run ends at the spend
  cap. A run that reaches a cap while it takes the last answer of a wait ends like one that reached
  it while it waited.

After a step or such an answer the cost records are booked at their cost; a failed call with no
cost is booked at the largest reservation it held (nothing when it held none), and a record that
cannot be read books the reservations and ends the run as a runtime failure, unless the run is on a route and the
relay booked or still takes calls of the step: then the step costs those calls ("The relay of a run's model calls"). The reason
of a call that left no cost record names the call, its adapter (of its `response.json`, else of its `request.json`), the
adapter's exit (`adapter_exit`, else `exit` of its `response.json`) and the client's own error from its stderr; a value
the call's record does not hold is written `not recorded`. Claude Code's `unrecognized_model` is said in words: the model
it asked for and the roles of the run's route file with their models (`runbook-<role>`), or that the run has no route
file (`runbook/src/cost.rs`). A cap sent as a JSON
number (`"max_llm_calls": 50`, or `1e999`, which is out of range) is refused naming the cap, over HTTP
and through `submit_run`.

The view. `read_run` shows each cap and what is used so far as `settings` members: `max_wall_seconds`,
`max_llm_calls` and `cap_micro_usd`, and `used_wall_seconds`, `used_llm_calls` and
`spent_micro_usd`. A running run shows its use live; an ended run shows its last. The use is kept in
`runs/<run id>/caps-used.json`. `spent_micro_usd` is the spend of this run; the ledger holds the entries
of every run of the runbook, each under its run id.

What is not done. The stub runtime takes no caps. A run is not taken up again after a restart
(`docs/engine.md`, "Cancel and recover"), so the use a restart would carry over is kept and not yet read
by a live run.

## Stopping the daemon

A run lives on a thread of the daemon's process. A daemon that exits ends the runs it holds: the
runtime has no run of them at the next start and they end as `orphaned` (see "State directory"). So
the daemon does not exit on its first stop signal while a run is live; it drains.

- The first SIGTERM or SIGINT begins the drain. A submit is refused `503` with the body
  `{"refused": "draining", "message": ..., "live_runs": <n>}` (the MCP tool `submit_run` gives the
  same body, with `isError` true and `503` in `_meta`); no run is made. Everything else goes on: reads,
  event streams, `answer-decision`, `cancel-run`, and the live runs, which are not cancelled and not
  asked to stop. Every answer carries the header `X-Runbook-Draining: <n>`, `n` the count of live runs
  (`Submitted`, `WaitingOnKey`, `WaitingForRoute`, `Running` or `WaitingOnDecision`): a command, a view, a stream, the MCP endpoint, the
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
| `journal.jsonl` | Every change of state as one JSON line, appended and flushed; replayed at start. It is the truth; a run killed mid-way is read back in its last state. A drain adds two lines of its own: `daemon.draining` (the ids of the live runs) and `daemon.drained` (see "Stopping the daemon"). A wake answered after its run was closed adds `route.wake-answered` (see "The wake of a managed route"). |
| `runs/<run_id>/` | The run's own record, written by the runtime. |
| `workspaces/<run_id>/` | The run's workspace; a `context_path` (a delegation's `workspace`) is copied here without `.git`, the exclude list and symbolic links, within the bounds of the copy (above, "The copy of a context"). |

After a restart a run that was `Submitted`, or `WaitingForRoute`, and never started becomes `FailedToStart` (event
`runbook.start-failed`); a run that was `WaitingOnKey` keeps its place in the queue of its key (above, "Runs of one
serial key"). For each run the record holds as `Running` or `WaitingOnDecision` the daemon
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
  the evidence records, the actions and the files of a run and open each one. Served here, the tab lists the files of
  the run's `work/` directory, which its cases share, from the view `products`, and opens one through the view
  `product` (text, Markdown rendered by the page's renderer, JSON indented, a file over 1 MiB with the part shown
  said, bytes that are not text by their size); the line counts the files and stands with the run's steps. The daemon
  serves no evidence record and no action output of a case, and the page has no tab for them. Nothing in them is made up. Run content never becomes
  markup of the page: the page builds nodes from the text, and the policy above (`script-src 'self'`,
  `connect-src 'self'`, `img-src 'self' data:`) stays as it is (`ui/README.md` "What a step produced"); the
  inspector's hostile cases run against this page too (`ui/test/daemon-run-tree-security.spec.mjs`).
- A run. The centre lists the run's steps in the order they started, read from the run's events (`runbook.action-started`,
  `ActionFinished`, `HandoffChecked`, `runbook.model-call`; `ui/web/runsteps.js`): step, role, state, start, duration
  and try, the step in flight marked and its duration counting. A line above says what runs now or how the run ended,
  and the counts (steps done, exits other than 0, steps ended with no exit, model calls, tokens, cost). A program whose
  `ActionFinished` holds a refusal and no exit is `ended` with the refusal's cause (`ended by a signal`, `ended: timed
  out`), never done; a handoff check (role `check`) ends at its own `HandoffChecked` with its exit, and
  `ActionFinished` ends no check. A step ends at the run's `runbook.step-ended`, never the
  run; a `Stopped` with `BudgetSpent`/`MaxSteps` that no `runbook.step-ended` names is said as a wait or the end of the
  tree and ends no step. A run with no `runbook.step-ended` at all (a runtime before it) has its steps ended by that
  `Stopped` line (`stepEnds` in `ui/web/runsteps.js`). The list draws the latest 200 steps and the step in flight, the
  feed and the Events tab the latest 200 events, each saying how many earlier ones it left out. The side panel has only the tabs the daemon serves data for
  (`tabsOf` in `ui/web/source.js`): Products and Events, every event of the run in words, and Metrics when the view
  `run-metrics` answers a row for the run ("The metrics of a run"). Times are the browser's local time, 24 h, with the
  zone's name (`19:54:50 CET`), the ISO date in front when not of today (`ui/web/clock.js`).
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
  command does: a `submit-run` without the member `confinement` runs at the daemon's default level, else at
  `linux` (the specification says so in the summary of `sdc.daemon.ConfinementLevel`,
  `ess/domains/daemon.yaml`, and the handler fills it in `submit_run_for`, `runbook/src/commands.rs`; the page
  sends no level it was not given, so the one it picked is the one that runs). A level
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
  its own answer and every two seconds while it is visible. The sidebar's case tree is the view `cases` of the
  selected run: each case under its parent with the depth guides, labelled by its link name (the root by its id),
  with its protocol and its state ("open", "running a step", "awaiting a person", "stuck", "ended
  <outcome>"); it is read again on each event of the run and on the two-second tick, so a case that starts to
  wait and a run that ends while the page shows it change in place. A run of the stub runtime has no tree, and the
  sidebar says so. The daemon serves no graph, phases, timeline or explanation of a case: those panes say so.
  The address of a run is `?run=<run id>`: it opens that run after a reload and in another tab, and choosing a
  run writes its id there; an id the daemon does not have shows a note naming it and no other run. Served by
  `ui/server.mjs`, the page reads that server as before (its `?run=` names a run directory), and the inspector
  has no route to the daemon.
- Model routes (`docs/model-routes.md`, section 6; `ui/daemon/routes.js`, loaded only when the daemon named
  itself). The header line of a run on a route names it after the runbook, the state (`WaitingForRoute` while the
  run waits for its route), the level and the kind: `route qwen · managed · starting · alias qwen · named by the
  run` (`the daemon default` for a route of `[route_policy] default`). The route, the alias and the setting are the
  run's row of the view `runs`; the kind and the status are the route's row of the view `routes` (the model's
  `phase` when the gateway gave one, else the row's `status` in words), or "status not read yet", or the reason the
  read failed. The button "Routes" opens the Routes view: each route of the view `routes` with its alias, kind,
  gateway, status, phase, wires and `read_at`, as text nodes. The page reads the view `routes` at most once every
  15 s, whatever the answer, and only while it shows a run on a route or the Routes view is open (every 15 s
  while it is open); a read that fails keeps the rows of the last read that answered and says so. The Routes
  view does not show the day's figures the view `routes` answers ("The day's figures of a route"). With the Routes view open, a
  daemon that answers `403` to every request is asked at most 4 times a second, as without it
  (`ui/test/daemon-routes.spec.mjs`).

## MCP

The MCP tools `read_run` and `read_events` ([`mcp.md`](mcp.md)) answer a run id the daemon does not know
with an error: `isError` is true and `_meta["runbook/http-status"]` is `404`. The HTTP views `run` and
`events` answer the same id with `200` and no rows; both stay as they are. The views `products` and
`product` refuse such an id themselves (`404`, `no such run`), and the tools `list_products` and
`read_product` carry that answer.

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

## The relay of a run's model calls

A daemon with declared routes, or with a `[gateway]` and the decider kind `model`, serves, on its own listener when that
is loopback, the relay of the model calls of its runs on a route (above, "The route of a run") and of every
delegation's run (above, "A delegation") (`docs/engine.md`, "A gateway"; `docs/model-routes.md`, section 11).
On a listener that is not loopback the relay is not served: these paths take the daemon's token there and are `404`, and
a run on a route does not start (a run whose model calls come from its programs only takes no default route there).
On a loopback listener:

- `POST /daemon/relay/<run>/v1/messages`, `/v1/chat/completions` and `/v1/responses`, the gateway's three wires.
  A call carries the run's own token, from the run's route file (`RUNBOOK_ROUTE_FILE`, `runbook.route/1`), as
  `Authorization: Bearer <token>` or `x-api-key`, and never the daemon's token: the daemon's guard checks `Host`
  and `Origin` of these paths and leaves the token to the relay. A run that is not live, or another token, is
  `401`.
- The call's `model` names a role of the run: the role's `model` of the route file (`runbook-<role>`, taken for the
  role `<role>` first) or the bare role, a trailing bracketed suffix (`runbook-default[1m]`) dropped. No role's name
  starts with `runbook-` (above, "The route of a run"). The relay answers in the gateway's shape,
  `{"error": {"code", "message"}}`: `400 route-refused` for a role the run has no route for (the message names
  the run's roles), `429 cap-reached` for a call past the run's `max_llm_calls` or spend cap (nothing is sent), and
  for a call past its route's daily cap with `cap_key` `route.<name>.daily_cap_micro_usd` or
  `route.<name>.daily_max_llm_calls` beside `code` (nothing is sent; "The day's figures of a route"),
  `400 bad-request` for a body that is no JSON object, `503 run-not-ready` before the run takes calls, and
  `502 route-unavailable` when the gateway gives no answer. Every other answer is the gateway's, passed back
  unchanged and streamed as it comes (its `content-type`, `x-llm-gateway-call` and other headers but those of
  the connection).
- The relay sets `model` to the route's alias, sets `stream_options.include_usage` on a streamed chat call,
  adds the gateway's token and sends the call, one request per connection, to `<base_url>/v1/<wire>`. It reads
  the tokens from the answer's `usage` (or the usage of a stream's events) and books the call before the caller
  sees the end of the answer. The line has `input_tokens` and `output_tokens`, and `cache_read_input_tokens` and
  `cache_creation_input_tokens` when the answer counts input read from or written to the prompt cache (the
  messages wire counts that input beside `input_tokens`); a declared input price takes all three. A call whose
  cost is not known (no price, or no tokens reported) books its reservation with `cost_known: false`: the step's
  largest reservation while its step runs, else what one call of the decider may cost, at most what the cap has
  left, fixed as the call is admitted, so a call answered after its step's program was killed books what its step
  held. An answer with no usage at all is a call of unknown cost on a route of any kind, priced or not, and
  books its reservation. A step whose calls the relay booked or still takes costs those calls; its own
  reservation is released. A `stream_options` that is `null` or no object is replaced by
  `{"include_usage": true}`. A call that was sent is booked once. A caller that hangs up before the answer has ended
  ends the call at the gateway: the relay closes its request to the gateway within a second and books the call with
  the `cause` `call-abandoned` and the `code` `caller-hung-up`, at the tokens the answer reported so far, else at its
  reservation (`cost_known: false`); an answer that ends within 200 ms of the hang-up is booked as answered. An
  abandoned call is no failed call of its step (`docs/engine.md`, "A gateway").
- A call the gateway refused carries the gateway's `code`, its `detail` and its error text as `error`
  (`<type or code>: <message>` of the answer's JSON `error`, at most 300 bytes, the gateway's token replaced; the
  `code` at most 64 bytes, the token replaced). A streamed answer that ends in an error event is a failed call too. A
  `4xx` answer that is no refusal of the route by the gateway (not `model-unknown`, `wire-not-served`,
  `credential-*` or `spend-ceiling`) has the `cause` `call-refused`; every other failed call, a gateway that does
  not answer included, `route-unavailable`. A failed call does not end the run by itself: the run ends as its step
  ends, and a step that went on is named in the run's event `runbook.model-calls-failed` (`docs/engine.md`, "A
  gateway"). The gateway's refusals of the route and its `spend-ceiling` fail their step at once.

## Outside the language

- The relay of a run's model calls: its paths are the gateway's wires, not commands or views of the
  specification, so the OpenAPI document does not list them.
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
- The readiness of a delegation's route: `route-unavailable` (503) is the daemon's `{refused, message}` answer, as a
  draining daemon's refusal is; the facts `route_declared`, `runbook_known` and `context_inside_roots` of `Delegate`
  are read from the host as those of `submit-run` are. `delegation-result`'s two refusals are the daemon's answers
  too (a view has no outcome).
- The refusal of the view `run-metrics` (`404` `no such run`) is the daemon's answer, as for `cases`; its rows are read
  from the run's events, which no command of the language reads into a row.
- The refusals of the views `products`, `product` and `cases`: a view has no outcome in the language, so the
  refusals are the daemon's answers, written in the views' summaries, in "The products of a run" and in "The case
  tree of a run". Where a case stands (`case_state`) is read from the runtime, not moved by a command.
