# Model routes: a daemon run on a named route through the LLM gateway

Status: design, 2026-11-25. Not built. Decisions open for the operator are marked **D-n** at the end.

## The goal

The operator's ask (2026-11-25): the LLM gateway, with a self-hosted model on a RunPod pod behind it, is wired into
Runbook, and the Runbook daemon (`runbook serve`) dispatches a run on a chosen model route.

A run names a **model route** by name. The daemon resolves the name to a gateway endpoint and a model alias that the
daemon's operator declared, checks that the route can answer before the run starts, gives every model step of the run
that route, books and caps every call, and shows the route on the daemon page. A route that cannot answer refuses the
run. No run ever falls back to another route on its own.

## What exists today

- `[gateway]` (`docs/loom-run-settings.md`, "`[gateway]`"): one gateway address and token for the model programs of
  `runbook run`, instead of a login file. The daemon takes the `[gateway]` of its own `runbook.toml` and gives it
  to the model decider of a run whose kind is `model` (`docs/engine.md`, "A gateway"). A submit that names a
  gateway, an address or a token is refused (`docs/daemon-api.md`).
- The bound programs of a runbook get neither a login nor a gateway (`docs/engine.md:400`).
  `story:model-entry-for-bound-programs` (active) gives them one documented entry; not built.
- Caps: `max_llm_calls`, `cap_micro_usd` and a spend ledger per run (`runbook run`); the daemon's Loom runtime
  does not take them yet (`story:daemon-run-names-its-caps`, proposed).
- A consumer runs on the gateway today outside Runbook's settings: its own tool sets the model process's base URL to
  the gateway and its model to the gateway's alias, chosen per run by an intake word. Its gaps: the route is no
  Runbook setting, a gateway call has no booked cost, nothing reads the route's health before a submit, and the
  daemon offers no list of routes.

What the gateway gives (`b10x-llm-gateway`, its `docs/gateway.md` and `docs/hosting.md`):

- An OpenAI-compatible and Anthropic-compatible relay on loopback; a client names the model by its **alias**
  (`/v1/models` lists them, without a credential).
- `GET /ready`: the gateway serves (`200 {"status":"ready"}`), or not (`503`). It says nothing about a model's pod.
- Two kinds of upstream for an alias:
  - a **hosted endpoint** (`[hosted.<alias>]`, a fixed base URL such as a pod's proxy address): no lifecycle; when
    the pod is stopped, calls fail at the upstream;
  - a **managed RunPod model** (`[models.<alias>]`): the gateway starts the pod on the first request or on
    `POST /v1/models/{alias}/wake`, reports its phase at `GET /v1/models/{alias}/status` (`stopped`, `starting`,
    `ready`, `stopping`), stops it after an idle timeout, and refuses a start past the provider's daily or total
    spend ceiling (`spend-ceiling`) or while cold (`model-cold-start`, with `retry-after`).
- One usage record per call with input, output and cache tokens; prices per pod hour for a managed model
  (`price_per_hour_usd`), no price per token.

## The design

### 1. A route is declared once, by the daemon's operator

The daemon's `runbook.toml` (or the file `--config` names) declares its routes. A route holds what only the
operator may state: where the gateway is, which alias, how to read its readiness, what a call costs.

```toml
[routes.qwen]
gateway = "default"           # a [gateways.<name>] table below
model = "qwen"                # the gateway's alias, as /v1/models lists it
kind = "managed"              # "managed" (the gateway starts and stops the pod) or "hosted" (fixed upstream)
wake_on_submit = true         # managed only: ask the gateway to start the pod when a run on this route is accepted
start_wait_seconds = 600      # how long a run waits for the pod to be ready before it ends `stopped: route`
price_in_micro_usd_per_mtok = 0   # optional: what the ledger books per million input tokens
price_out_micro_usd_per_mtok = 0  # optional: per million output tokens

[routes.claude]
gateway = "default"
model = "claude"
kind = "hosted"

[gateways.default]
base_url = "http://127.0.0.1:<port>"
token_file = "<path>"          # only for a gateway that takes an ingress credential; none on loopback without it

[route_policy]
default = "claude"             # the route of a run that names none; absent: a run must name one
```

- `[gateways.<name>]` replaces the single `[gateway]` table; `[gateway]` stays accepted as `[gateways.default]`
  for one release, then is refused by name.
- A route names an alias, never an upstream URL, a pod id or a key: those stay in the gateway's own document.
- A non-loopback `base_url` is the operator's choice, as for `[gateway]` today.

### 2. Which route a run uses: allowed by default, narrowed per protocol or per run

The operator's rule (2026-11-25): "generally, it must all be allowed, but it should be just configurable per protocol or
per run config".

- **Allowed by default:** every route the daemon declares may serve every runbook.
- **Per protocol:** a runbook's bindings may name `model_route` per model node or role (`writer`, `review`, …),
  and may name `routes_allowed = [...]` to narrow which routes may serve it.
- **Per run:** a submit may name `model_route = "<name>"` (and per role, `model_routes.<role>`). It names a
  declared route by name only; an address, a token or an alias that no route declares is refused by name.
- **Precedence:** the run's word, then the protocol's, then `route_policy.default`. A route outside the protocol's
  `routes_allowed`, or not declared by the daemon, is refused before the run starts (422 `route-refused`, naming the
  route and the rule that refused it).
- `runbook run` reads the same tables from `runbook.toml` and takes `--model-route <name>`.

### 3. Before the run starts: the route must be able to answer

At submit, after the route is resolved and before anything is written:

1. `GET <gateway>/ready` answers 200; else the submit is refused `503 route-unavailable` naming the route and
   "gateway not ready".
2. `GET <gateway>/v1/models` lists the alias; else `422 route-refused` "the gateway serves no model `<alias>`".
3. For a `managed` route, `GET /v1/models/{alias}/status`:
   - `ready`: the run starts.
   - `stopped` or `starting`: the run is accepted and starts in the state `waiting-for-route`; with
     `wake_on_submit` the daemon sends one `POST /v1/models/{alias}/wake`. The run waits up to
     `start_wait_seconds` for `ready`, polling the status at most once every 5 s, then starts; past the wait it
     ends `stopped: route` with the last phase. A `spend-ceiling` answer to the wake ends it `stopped: route`
     naming the ceiling.
   - `stopping`: as `starting`, after the stop is confirmed.
4. For a `hosted` route there is no status: the daemon sends no model call to probe it (a probe would spend). The
   run starts; a failing upstream shows as a failed call (below).

### 4. While the run runs: one route per call, no silent fallback

- Every model step of the run (model nodes, the decider, and, once `story:model-entry-for-bound-programs` lands,
  a bound program's call through the documented entry) reaches the gateway of its route with the route's alias as
  its model, the way `[gateway]` works today: one file in the run's private directory, the address and the token set
  for the model process only, nothing through Loom's environment.
- A call that fails because the route cannot answer (connection refused, `503`, `target-unavailable`,
  `model-cold-start` past its retry) fails that step with the cause `route-unavailable` and the route's name. The
  protocol decides what a failed step does, as for any failed program. Runbook never re-sends the call to another
  route, and never changes the route of a running run.

### 5. Booking and caps

- Every call through a route is one ledger line: run, step, route, alias, input and output tokens, and its cost.
- Cost: with prices declared on the route, tokens times price; without, `cost_known: false` and the call books its
  reservation (`max_call_micro_usd`), as the ledger does today for a call of unknown cost. A managed pod's hourly
  cost is the gateway's (its daily and total spend ceilings); Runbook does not split pod hours over runs (**D-3**).
- The run's `max_llm_calls` and `cap_micro_usd`, and the daemon's daily spend ledger, count these lines. A cap that
  is reached stops the run as today.
- A tool that a program starts and that calls a model on its own is not booked by this design: that needs a proxy
  in front of the gateway per run (**D-2**, the consumer's R-4).

### 6. What the daemon shows

- The run's header on the daemon page: `route qwen · managed · ready` (or `starting`, `stopped`, `unreachable`),
  and the run state `waiting-for-route` while it waits.
- A **Routes** view (`GET /daemon/views/routes`, and an MCP tool `list_routes`): each declared route with its alias,
  kind, the gateway's readiness and the model's phase (read at most once every 15 s and cached), and today's calls
  and booked spend from the ledger.
- The run record names the route, the alias and the route's source (run, protocol or default).

### 7. Confinement

A run at the confinement level `substrate` cannot reach a gateway on loopback today
(`story:gateway-under-substrate-confinement`, draft). Until that story is decided, a route serves runs at the level
`local` only, and a submit at `substrate` that needs a route is refused by name (**D-4**).

## The noun

`ModelRoute` is a new entity of the daemon's ESS domain (`ess/domains/daemon.yaml`): name, gateway, alias, kind,
readiness, phase. `Run` gains `model_route` and its source. The specification is written before the code
(the repository's rule for a new noun).

## The stories this design becomes

1. `daemon-declares-model-routes`: `[routes.*]`, `[gateways.*]`, `route_policy`, the Routes view and MCP tool; the
   `[gateway]` table read as `[gateways.default]`.
2. `run-names-a-model-route`: the submit's and the protocol's word, precedence, refusals; `runbook run --model-route`.
3. `route-readiness-before-a-run`: `/ready`, `/v1/models`, the managed model's status, wake, `waiting-for-route`,
   `stopped: route`.
4. `route-calls-are-booked-and-capped`: one ledger line per call with route and tokens, declared prices, the
   reservation when unknown; the daemon's Loom runtime takes the caps (`story:daemon-run-names-its-caps`).
5. `daemon-page-shows-the-route`: run header and Routes view.

Built on: `story:daemon-runs-take-a-gateway` (active) and `story:model-entry-for-bound-programs` (active), which this
design extends from one gateway to named routes.

## Tests (no model, no network)

A stand-in gateway on loopback serves `/ready`, `/v1/models`, the status and wake routes and a chat route that
answers a fixed completion with usage; a test clock drives the wait. Cases: a run on a ready route reaches the
stand-in with the alias as its model and books one line; a stopped managed route is woken once and the run starts
when the status turns `ready`; a route that stays `starting` past the wait ends `stopped: route`; a `spend-ceiling`
wake ends it naming the ceiling; a gateway answering `503` refuses the submit; an undeclared route, a route outside
`routes_allowed` and a submit naming an address are refused by name; a failing call fails its step with
`route-unavailable` and no second route is called.

## Decisions for the operator

- **D-1 Where routes are declared.** A: in the daemon's config only; protocols and runs choose among them by name
  (recommended: an address and a cost stay the operator's, as the gateway rule says today). B: a protocol may also
  declare a route of its own.
- **D-2 A tool's own model calls (R-4).** A: later, its own story with a per-run booking proxy (recommended: it is
  a new path through the run's boundary). B: in this design's first wave.
- **D-3 Cost of a self-hosted model.** A: declared price per million tokens on the route, else the reservation
  (recommended: per call, comparable with hosted models). B: split the pod's hourly cost over the runs that used it.
- **D-4 Confined runs.** A: routes serve `local` runs only until `story:gateway-under-substrate-confinement` is
  decided (recommended). B: decide that story now and open one egress to the gateway.
- **D-5 Who starts the pod.** A: the daemon wakes a managed route on submit (`wake_on_submit`, recommended for a
  daemon run nobody watches). B: only a person or the gateway's first call starts it.
- **D-6 Today's route `qwen` is a hosted endpoint on a pod proxy address**, so nothing can read whether its pod runs.
  A: move it to a managed RunPod model in the gateway's document, so readiness and wake work (recommended). B: keep
  it hosted; runs on it start blind.

## Part two: a session plugin whose delegate action is served by Runbook

The operator's second requirement (2026-11-25): a Runbook plugin for Claude Code, enabled for one session only;
with it on, the session's delegate action (a sub-agent) is served by Runbook: the daemon takes the request, runs it
on a named model route, records it, and hands the result back as the sub-agent's result. This section is being
written from the Claude Code plugin documentation and follows in the next commit of this pull request.
