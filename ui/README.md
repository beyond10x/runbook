# Runbook run inspector

The run inspector of Runbook: a read-only web view of a run directory (a `runbook run` directory, or
a case store an older runtime left). It shows where each case stands in the phases of the software
delivery cycle (SDC), the first runbook, and in its Canon protocol, why (claims down to evidence
records, excluded records and the revision that excluded them), what is admissible next, who acted
(program, LLM, person, decider), what each agent step was allowed to do and did, and run metrics. It
updates live while a run writes its directory. It acts on nothing: it starts no program, takes no
decision and sends no command to the daemon. A decision is answered with `runbook decide <run>
<request> <answer>` or on the page of `runbook serve --ui`; `runbook runs decisions <run>` lists the
open ones. Every place that would offer the act says so in one line.

## Start

```console
node server.mjs --port 7490 ../runs/trial-todo-1/store ../runs/full-cycle-1/store
```

Open http://127.0.0.1:7490/. Pick the run in the header (the run picker). Or, from this directory, `task serve`.

- The server listens on 127.0.0.1 and answers a request whose `Host` is `127.0.0.1:<port>`,
  `localhost:<port>` or `[::1]:<port>` (the daemon's own set), with no `Origin` or the same one;
  any other `Host` or `Origin` is refused with 421 before a route. A request it cannot parse is
  answered 400 and one that fails inside a route 500; the process goes on.
- A file of a run is read only when its real path lies under the real path of the run directory
  (`runfiles.mjs`): a link that leaves the run is not followed, not listed and not served
  (`/api/blob` answers 404); a link that stays inside is. `web/` is everything the server serves;
  the code that answers a decision lives in `daemon/decide.js`, which only the daemon serves.
- Each argument is a store directory (its subdirectories are root cases) or one case directory. A
  path that does not exist yet is waited for; cases and events show up as the run writes them.
- `--runs DIR` (repeatable) serves every `DIR/<run>/store`, and a run that appears later joins the
  run picker without a restart (`node server.mjs --runs ../runs`). Explicit store arguments come
  first and are not listed twice.
- A run directory with `run-stopped.json` (`sdc.run-stopped/1`: at, by, reason, verdict_at_stop,
  case) is a run stopped on purpose: the Now band says "Stopped by … at …: reason" and what it was
  waiting on, in the past tense; every clock (elapsed, waiting, the graph node, the case tree) stands
  at the stop; the run picker and the case tree mark it; its open decisions show greyed in the
  drawer; the inspector takes none of them.
- Whether a run is live: `runbook run` writes `settings.json` when it starts and `run-stopped.json`
  or a `status.json` with state `closed` when it ends, and no pid or lock record, so nothing in a
  run directory tells a live run from one that was killed. The server reads those files only and
  never looks a process up. An ended run is stopped or closed (above). A run directory with
  `settings.json` and no end record is "live state unknown": the Now band and the run picker say
  so, and every clock stands at the last event, including the waiting time. A case store an older
  runtime left (no `settings.json`) is history: reported "not live", and a step it started and
  never finished is shown as "Not live", not running. `/api/live` carries this as `liveness`
  (`ended`, `unknown` or `history`).
- `run-settings.jsonl` (`sdc.run-settings/1`, last line) says who holds the decisions. With a decider
  holding them, any time a step waited on a person is a failed KPI: the Elapsed tile turns red, says
  "should be 0" and links each stop (case, action or findings, duration) to its event. Autonomy
  `A5` is autopilot: the page never names a person or "you" for a decision, and says "the decider is
  deciding" or "no decision is pending". `stand_in: true` marks the run "end-to-end test with a
  stand-in" in the header and its KPI strip "not eval numbers".
- Who decided comes from the record's principal (`web/principal.js`): `person:<id>` (and a bare id,
  as older drivers wrote it) is a person; `decider:<kind>` is "decider <model>, on the operator's
  behalf"; `rule:<name>` and `decider:approve|reject` are "rule <name>"; `human` without an id is
  "recorded as human (principal not recorded)", never a person.
- The server needs Node 22 and nothing else, and listens on 127.0.0.1 only.
- The server never writes anything and starts no program. Every request that is not a GET or HEAD
  answers 405, and an option it does not know (`--write`, `--principal` and `--write-only` of earlier
  versions) is refused with exit code 2. There is no route, token field or proxy to the daemon.
- LLM calls, tokens, cost and tool calls come from `llm-calls.jsonl` next to the store; decider
  decisions from `decisions.jsonl` there. Panels without data say "not recorded yet".

## Use

- **Run picker** (header, decision D-009): the button shows the run on screen with its state as a
  shape and a word. It opens a panel with a search field (focus on open) and the runs, most recent
  first: "Running now", then "Recent", and stub and fixture runs in their own group, collapsed
  unless the run on screen is one of them. When any run belongs to an experiment, "Recent" holds one
  collapsible group per experiment (most recent first; open: the most recent one and the one of the
  run on screen) and "Unassigned" for the runs of none. An experiment's head gives its id, its
  purpose (the record's `purpose` or `title`), its variants with their number of runs ("low ×3 ·
  high ×2", or "3 variants × 3") and how many of its ended runs passed clean (closed and delivered,
  nothing forced or unstuck); its rows go by variant, then ordinal, each tagged "variant #n". A run
  belongs to an experiment by its `experiment.json` (`sdc.experiment-run/1`: `experiment`,
  `variant`, `ordinal`, `tree_commit`, …) or by its directory name in an experiment record's
  `variants[].runs` (`experiments/<id>.yaml` of the checkout served; `--experiments DIR`,
  repeatable, reads others); a run in two records is a row in each. Each row has the run's name, its state (running; closed
  with its outcome; closed, not delivered; stopped with its cause; not running), when it started
  (absolute on hover), how long it ran (ticking while it runs), cost and LLM calls, the models in
  short ("decider Opus · work Sonnet") and reopen, forced and unstuck counts from `status.json`.
  The search filters on name, state, outcome, stop cause, model, experiment id and variant, every
  word must match, and opens the experiments it matches in; arrow keys and Enter pick (Enter on an
  experiment's head opens or closes it), Escape closes. The pick goes into the URL (`?source=N&run=<name>`; `run`
  wins when both are given). The rows come from `/api/sources` (`run` per source), one request.
  On a phone the panel is a full-width sheet.
- **Now** (top band): every action in flight in the case tree, each with its case, who runs it (LLM
  agent and model, program and its command, copy, driver), a clock that ticks in the browser since
  the step started, and for an agent call its turns, tool calls, denials and last tool call, read
  live from the call's event stream. Under it, the files the step changed so far. A case awaiting a
  person says who holds the decision ("Held by": You (principal), Decider `<model>` with its
  progress while it decides, or Rule approve/reject), why (the decider refused it and its first
  reason, or "the decider setting is not recorded"), how long it has waited, for what (capability,
  subject at its revision), how many fields one approval covers, and what taking it does. The band's
  head says what is true in words: "Waiting on you: nothing runs until this is decided", "Decider
  deciding", "Stopped: stuck since …, no step is admissible (open obligations)", "Closed: ended
  <outcome>"; "Driver gone" for a step that was started, never finished and has no live driver;
  with no driver attached the head keeps its words and the line under it says "no driver attached,
  last activity <time>". The holder comes from a decider call in progress, `decisions.jsonl`, and the run's
  decider setting in `run-settings.jsonl`; a run without that file says the setting is not
  recorded. On the right, the last
  four finished steps with duration and result. Click a card for the case and its action. The same
  action glows in the graph (with its clock) and in the phase strip.
- **KPI strip** (under the header): five run KPIs of `docs/kpis.md`, live. Elapsed (R7: first event
  to the close, or to now) with the time spent waiting on a person or authority (L2's non-autonomous
  part: verdict to the case's next event); cost (R6 `llm-calls.jsonl` work + H4 `decisions.jsonl`
  decider); LLM calls (R4, work and decider); person decisions (H1/L1) with decider escalations
  (H2); progress (F10 cases closed of opened, and the first SDC phase not done). A value the run
  did not record says "not recorded". Click a KPI for its row and definition in Metrics.
- **What changed** (left, under the case tree): every change of the run by what it is, newest
  first, with a filter chip per class. Planning (AEP artifacts: created, status moves, edges, scope,
  acceptance, sections, read from the front matter before and after), Specification (ESS sources:
  entities, fields, states, transitions, commands, outcomes, views, components and system domains
  added, removed or changed, from the parsed YAML), Direction (requirements, questions and answers,
  context fields, vision, roadmap epics and their order), Decision (decider and person decisions,
  answered questions, ADR-like documents under `work/`), Code (one row per step, its files). Each row
  names the case, the action and who caused it; a change opens its text diff with the meaning above
  it, a decision its grounds (reasons, cites, model, cost). A file the reader cannot parse keeps its
  class and says so; its diff is the record. The Now band and the Changes tab show the same meaning
  for each change. A run without `fs/changes.jsonl` shows what the store still says: each artifact
  revision with its action (the content of a revision is not recorded), the AEP transitions from
  the final front matter, and the decisions.
- **Changes** (right panel tab): every file change of the run from `fs/changes.jsonl`
  (docs/fs-changes.md), on a time strip and grouped by the action that made it, with the tool call
  and the attribution basis (`exact`: inside one driver action; `inferred`: matched to one agent
  tool call by time; `call`: in an agent or decider call's directory outside any action). A late
  `sdc.fs-attribution/1` line supplies the tool of an earlier change. Click a file for its
  syntax-highlighted diff (Rust, TS/JS, YAML, JSON, Markdown, shell, TOML); ← and → step through.
- **Phase strip** (top): the SDC phases of `docs/phases.md`, each derived from Canon claims of the
  case tree, never from a stored phase. Click a phase to open the claim that holds it back.
- **Case tree** (left): every case and its state (open, running, awaiting a person or authority,
  stuck, ended with an outcome).
- **Graph** (centre): the selected case's protocol. Artifacts → actions → evidence kinds → claims
  → outcomes and obligations, coloured from the stored `canon-decision/1`. Actions carry who runs
  them (LLM, person, driver, program, review). Admissible actions glow. "Reads" draws each
  action's precondition edges. Drag to pan.
- **Timeline** (bottom): drag the playhead, or use ← and →, to see the case at any event; the
  colours come from the evaluation stored at or before that event.
- **Right panel**: a rail of tab names until a tab is opened; clicking the open tab again or the
  chevron closes it, and the graph takes the width. Open or closed and the tab are remembered
  (localStorage). Selecting a node, a phase or a Now card opens it on Explain. Explain (click any node), Autonomy (next person action with the exact command,
  decisions taken by a person or a `decider:*` principal, every step and its LLM call), Security
  (approvals and what they are bound to, independent producers, allowed vs did per agent step),
  Metrics, Events, Run log.
- **Decisions** (header button, count of what waits for you): every pending approval of the tree
  (the `next` entries of the last verdict) with the revision and artifact to review and what
  approving makes true, every open scope-changing question with its options and default, and the
  history of decisions taken. The drawer is read-only: it has no Approve, Answer, Change or
  Continue control, and one line at the top says where a decision is answered (`runbook serve
  --ui`, `runbook decide <run> <request> <answer>`, `runbook runs decisions <run>`). The Now
  band's waiting card has a Decide button that only opens this drawer.

  An approval over a structured artifact (`context.yaml`: fields with value, status and reason)
  reads as one decision: "One decision over N fields", who holds it and what taking it does come
  first, then each field with its value, reason and state (default kept, accepted or changed by
  whom) and, when a decider decided it, what its reply says per field (else "approved as a whole").

  A server of an earlier version wrote `runs/<run>/ui-actions.jsonl` for the acts it took. The page
  still reads that file in an old run, as history, and no longer writes it.

URL parameters for links: `source`, `case` (path below the store, for example
`IDEA-1/children/IDEA-1.intake`), `node` (for example `claim:dissection.valid`), `at` (event seq),
`tab`, `reads=1`, `drawer=1`.

## The view file: one inspector, any runbook

What the page knows about one runbook, and cannot read from the case store, is data the runbook
ships: `views/sdc.view.json` for SDC (format `runbook.view/1`). The code knows the runtime's records
(events, evaluations, evidence, decisions, bindings, the AEP, ESS and ADR file formats) and nothing
about one runbook. `node server.mjs --view FILE` (repeatable) loads views; without `--view` the
server loads every `views/*.view.json` of the checkout; `--no-view` loads none. A run is shown with
the first view that covers it, so one server can hold runs of different runbooks. The page reads the
view of the run on screen from `/api/view?source=N` (`null` without one) and what is missing from
`/api/view-info?source=N` (`protocols`, `file`, `runbook`, `missing`).

A view is checked when the server starts: a member the inspector does not know, or one of the wrong
type, refuses the start with a message that names it (`decisions.scope_answer.escalate: view member
"escalate" is not known to the inspector (runbook.view/1)`), exit code 2. A new member needs a change
of `web/view.js` (`VIEW_SCHEMA`), not only of the file.

Members (all optional but `format`):

- `runbook`: its name. `protocols`: regular expressions of its protocol ids; the view covers a run
  when one case of the run has such a protocol. Without `protocols` the protocols of `phases.items`
  count, and a view without either covers every run.
- `phases`: `stages` (names, in order), `items` (`id`, `stage`, `label`, `short`, `protocol`, the
  `claims` that make the phase, `doc` for the tooltip, `has_source`, `root_outcome`) and `absent`
  (a stage shown with a note while no case of `protocol_pattern` exists, the "Ticket" row of SDC).
- `file_kinds`: `rules` (a `match` regular expression, optional `flags`, a `class` of planning, spec,
  direction, decision or code, a `kind`, or `$basename`; the first rule that matches wins),
  `artifacts` (an artifact name of the protocols to its class) and `parsers` (a `kind` of a rule to
  what a change of such a file means, below).
- `decisions`: which decisions the page shows as a person's; the page answers none of them.
  - `scope_answer`: a question a person answers and the approval that goes with it. `action` (its
    capability is named like it), `artifact` (a name of the case's artifacts), `list` (the key of
    the records in that file), `flag` (the record's field that marks a question for a person),
    `fields` (the file's names for `id`, `item`, `question`, `options`, `default`, `status`,
    `answer`, `answered_by`, `answered_at`, `reason`; the file's own names when a field is not
    listed) and `statuses` (the file's words for `open`, `accepted`, `changed`).
  - `stall`: a wait on `action` with no record in `artifact`'s `list` whose `status` is
    `needs_person` is not a decision (the Now band says why nothing runs); `noun` is the word the
    sentence uses.

A parser (`file_kinds.parsers.<kind>`) says what a change of a file of that kind means, in sentences.
Two types read it. `keyed-list`: records with an id, in a list under `list` (or a map, `as: "map"`);
`added` is the first rule whose `when` holds (`text`), `removed` a sentence, `changed` rules in order
(a rule with `stop` ends them, `each` and `except` make one sentence per changed field, `variants`
refine a sentence), `otherwise` the sentence when no rule fired. `when` holds when every part holds:
`set` (fields not empty now), `was_unset`, `changed`, `equals`. In a sentence `{name}` is a field,
`{old.name}` the field before, `{id}` the record's id, `{$field}` a changed field's name,
`{a|b|'x'}` the first that is not empty, `{name!N}` the value in quotes cut to N characters, and
`[ ... ]` is left out when a field in it is empty. `document`: a front matter document with
sections; `entries` (`word`, `sections`, `summary`: where its entries are named) and `created` (the
sentence for a new file). A kind with no parser is read as its top-level keys.

Without a view the page still works: one stage "Protocols" with one phase per protocol found in the
store, every file labelled by its path (class code), and no decision of the runbook; a note above the phases says which view is missing (the protocols found,
`--view FILE`). The same holds for a run no loaded view covers: it is never shown with another
runbook's view. `test/view.spec.mjs` runs a second runbook (`test/second-runbook.mjs`, protocol
`triage.ticket`, a record `asks.yaml` and a decision of its own) on servers of free ports with no view,
a two-phase view and a view that does not cover it; it also checks that no route takes an answer, and
refuses a view with an unknown member.

## Where "now" comes from

- In flight: a case's `action_started` without a later `action_finished`.
- Whether the run is live: see "Whether a run is live" above. The runner writes no pid and no lock,
  so the Now band never says a run is live. A run directory of the runner with no end record says
  "live state unknown"; a case store of an older runtime says "not live", and a step it started and
  never finished shows as "Not live", not running.
- Since when: `observed.jsonl` beside each `events.jsonl` (`sdc.observed/1`: index, event, action,
  observed_at, pid), matched by line index to the events (and by `event` when the line has it);
  the card says "driver observed_at". Without it, the server's clock of each event line (the mtime
  of `events.jsonl` when the line appeared). The events' `at` is the instant passed to the whole
  run, shared by every step of it, so it dates a run, not a step; a step whose start the server did
  not see shows "run since".
- Agent and decider calls: `llm/NNN-<action>/` and `decider/NNN-<id>/` with `request.json` and a
  `status.json` (`sdc.call-status/1`: state `running`, `finished` or `failed`, pid, started_at,
  finished_at, exit). The state decides "in progress"; a running state whose pid is gone says so.
  Without `status.json`: an empty or missing `response.json` (the adapter's stdout is redirected
  into it, so the shell creates it empty when the call starts). Progress from the call's
  `events.jsonl` (metaharness.event/1, one line per event as it happens). A call joins its case
  through `request.json.case`; failing that a decider call through `decide-request.json`, an agent
  call by its action name.
- Durations of finished steps: the two `observed_at` of the driver, else the server's clocks of
  start and finish, else `duration_ms` of the call in `llm-calls.jsonl`; earlier steps of a run
  the server did not watch have none.

To watch it without a real run, play a recorded run as if it happened now:

```console
node test/fixture-run.mjs agent ~/scratch/runs/agent    # also: two, decider, person, changes
node server.mjs --port 7495 ~/scratch/runs/agent/store
```

## Screenshots and tests

```console
npm ci
task test               # the test step: Playwright, four workers, 60 s a test, 120 s the run
task slow               # the slow lane: specs that need runs/
```


The test step needs no `runs/`, no built binary and no server of your own: every server a spec
starts is `server.mjs --port 0`, which takes a free port from the kernel and prints it
(`test/serve.mjs`). It writes `screenshots/{wide,laptop}-*.png` from the committed fixture
(`test/recorded/trial-todo-1-slim`, a slim copy of `runs/trial-todo-1`: cases intake and direction;
no prompt, transcript or answer of an LLM, no absolute home path, a reason names the role and never
a person; remade with `node test/slim-source.mjs`). `test/lanes.mjs` names each spec of the slow lane
and what it needs: `runs/trial-todo-1` and `runs/full-cycle-1` (`now`, `changes`, `screens-runs`,
`meaning-runs`), or a store the caller names (`live`: `SDC_UI_LIVE_DIR`, `SDC_UI_LIVE_SOURCE`).

`test/now-model.spec.mjs`, `meaning-model.spec.mjs`, `holder-model.spec.mjs` and
`stall-model.spec.mjs` test the derivations without a browser. `test/read-only.spec.mjs` proves the
inspector acts on nothing: every route that took an act answers 404 or 405, a server started with
an option of the removed acts exits 2, and a Decisions drawer shows the line on where a decision is
answered and no control that acts. `test/view-race.spec.mjs` holds two sources with two views on one
server and checks that concurrent requests each keep their own view. `test/paths.spec.mjs` fails
when a file git would commit under `ui/` or any page or API answer names an absolute home path;
`test/paths-served.spec.mjs` checks the same on a served copy of the committed fixture. The server
shows paths relative to the repository, its workspace (`../`) or `~`.

## UI iteration loop

`ux/` holds the measured loop of the `ui-iteration-loop` skill. `ux/adapter.mjs` names the gating
viewports (1600x1000, 1366x768, the operator tile 805x1270 and the phone 390x844; 1280x720,
1920x1080 and the phones 360x800, 414x896 and 768x1024 are measured only), the one theme, what is
always visible (Now band with its run state, KPI strip, phase bar, the run picker, Decisions; decision D-009), the
desktop order from the top and the KPI tiles that must be whole (decision D-008), the scroll exceptions (decision
D-001), the guidance cue (the Now band's waiting card and its Decide control) and six scenarios
with their truth. `ux/core/` is generic: the layout contract with axe-core, the screen-only user's
eyes, the metrics, the ratchet and the round command. `ux/specs/` joins them: the driver bot
(`test/fixture-run.mjs`) plays each run, the contract samples it, the screen-only user does the
scenario's tasks with only what it can see.

```console
task ux                 # fast loop at the gating viewports, then the ratchet against ux/baseline.json
task ux:update          # take improvements into the baseline (DECISION=D-nnn for a worse value)
task ux:round ROUND=R-02   # measure a round: ux/rounds/R-02/{report.md,round.json,shots/} and trend.md
```

Each server of the loop and of the phone contract (and the contract's recorded runs) takes a free
port and names it in its first line, so two loops on one machine do not collide. `SDC_UI_UX_PORT`
pins them instead: its value on for the loop, 60 above for the phone contract, 80 above for its
recorded runs. Findings are `ux/findings/UXF-nnn.md`; a closed S1 or S2 needs
a regression asset that exists (`ux/specs/findings.spec.mjs`). Round R-01: `ux/rounds/R-01/`
(report, the two reviews, triage, decision).
A baseline key may carry its own `tolerance` (decision D-003: the wall-clock sampled metrics of the
played scenarios); the ratchet counts a value as worse or better only outside that band. Decision
D-004 took worse values the operator's wording asks for: the line "no driver attached, last
activity …" (reading load +7 to +9 words in person, decider and changes; a layout shift in agent
and decider and two more cue changes in two at 1366x768 when the played driver detaches at the
end), and 3 more truncated elements at 1366x768 (and in
closed at 1600x1000) from the longer principal labels and stop marks of the same change.
Decision D-005 made the measurement deterministic. The ratchet failed on a different sampled value
from run to run because the fixture player ran on the wall clock, the server rescanned every 700 ms,
and the loop sampled every 1.4 s of wall time and divided layout shift by wall minutes: which state
changes fell into the window depended on the machine's load. The measured run now shares one clock
(`vclock.mjs`) between the server, the player and the page, moved only by the loop: per step the
clock moves 1.4 s, the player plays 1.4 s of its sleeps and waits, the server rescans when the
player reports, and the page fetches and paints (`window.__sdcSync`) before anything is sampled.
Animations and transitions are off in the measured page, layout shift is counted per minute of the
steps' clock, and a scenario waiting on someone reaches its truth when the card names who it waits
on. The values this changed (the measurement, not the UI) were taken under D-005, and the D-003
tolerances were dropped. Round files are written with repository-relative paths.

### Phone sizes (decision D-006)

The operator, 2026-11-21 ("mobile view looks pretty unusable atm"), set the adapter's values for a
phone (`ux/adapter.mjs` `phone` and `policy`), applied at the gating phone size 390x844 and reported
at 360x800, 414x896 and 768x1024:

- Vertical page scroll is allowed on a phone; horizontal page scroll is not, and nothing runs past
  the screen's edge.
- Always visible at every state change, scrolled or not: the run's state (running, waiting, stopped
  with its reason), the Now band's current step and the open-decision indicator (Decisions).
- The top KPIs stay reachable within one scroll screen.
- The right rail is a full-width sheet opened from a visible control (Details), never a column.
- The case graph is a list or scrolls sideways inside its own box with a visible hint, never a
  clipped canvas.
- Dialogs (the change dialog, the Decisions drawer with its decision cards) are full-screen sheets
  with their close control in view.
- Every tap target is at least 44x44 CSS px; no body text under 14 px (and no label under 12 px).
- Desktop behaviour does not change: every phone rule is under `html.phone`, which `index.html`
  sets from one query, `SDC_PHONE` (decision D-008).

`ux/core/phone.mjs` measures these facts (generic); `ux/specs/phone.spec.mjs` is the phone layout
contract at every key moment of the six scenarios and of the recorded runs `trial-2-train`,
`trial-3-haiku` and `trial-todo-1` (skipped without `runs/`), with metrics `phone.*` in the
ratchet; `test/mobile.spec.mjs` holds one test per phone finding
(UXF-016 to UXF-026) over played runs that need no `runs/`. In `ux/specs/loop.spec.mjs` the
screen-only user may scroll the page at a phone size; what it scrolls to costs a glance 3. Round
R-03: `ux/rounds/R-03/`.

Decision D-007 labels stub runs. The played scenarios are fixture runs, so the measured page
shows the label "stub run: canned content, not a model's work" in the header and in a row of its
own above the Now band. reading_load rises by the label's 8 words in every played scenario, and two's
layout shift falls a little because the band starts one row lower. These values were taken under
D-007: the UI changed on purpose. On a phone (390x844) the header mark and the label row are whole
and in the first screen (`test/stub-runs.spec.mjs`); there reading_load rises by 8 as well, and in two
the screen-only user scrolls once more (screen_only.looks 4 to 6, glance_cost.p95 1 to 3) because
the second card moves below the first screen.

### Two layouts, chosen by device (decision D-008)

The operator, 2026-11-21, on the phone layout in his desktop browser, a tile about 805x1270: "the UI
changes you did for the mobile version, they are horrible for the non-mobile version … I like the
overall, like the green bar stuff. Like when it was on top, it was way better for the normal
version. I mean, just make two versions. Don't break it for the non-mobile one". The phone rules
had switched on at `max-width: 900px`, so any narrow desktop window got the phone layout.

- The phone layout is for a phone: `SDC_PHONE` in `index.html`, `(max-width: 600px), (max-width:
  900px) and (pointer: coarse)`, sets `html.phone`, and `app.js` reads the same query. A desktop
  window wider than 600 px with a mouse gets the desktop layout of any width.
- 805x1270, the operator tile, is a gating viewport with the desktop rules: always visible the Now
  band with its run state, the KPI strip, the phase bar, the store selector and Decisions; from the
  top the header, the KPI strip, the Now band and the phase bar, each below the one before; no KPI
  tile cut off by the window or its strip (`contract.clipped`); no page scroll.
- In a desktop window below 1000 px the layout is the one of 1366 px with narrower Now band columns
  (the Just finished list 200 px, 184 px when the run waits or stopped), so a card shows its action,
  executor, model and holder whole; the run's state text breaks a long name inside its column
  instead of running under the cards and, in the desktop layout at any width, ends after 12 lines
  with an ellipsis, the whole text in its title (a stop's reason kept the band 800 px tall); the live pill's time and a stub run's header mark give way
  with an ellipsis and a title, and the run picker's button narrows to its choice (at most 230 px).
- The phone sizes are measured as phones (touch and a coarse pointer: `isMobile`, `hasTouch`), in
  the loop, `test/mobile.spec.mjs` and `test/stub-runs.spec.mjs`, which also checks the tile.

### Run picker (decision D-009)

The operator, 2026-11-21: "the select box for the individual runs… that's for sure not scaling, so
we should just show the most recent ones and make it a custom UI element where we have autocomplete,
a nice search, and maybe also some visual indication on when this has been run, what the outcome
was, the duration… So we can actually reason about what to select there." And, on the experiment
records: "organize these trials in a scientific, manageable and scaling manner".

- The store select is the run picker (see Use). Its button (`#runpick`) is always visible at every
  gating viewport; its list scrolls inside its own box (`#pk-list`, a D-001 exception); on a phone
  its panel is a sheet (`phone.sheet_picker`).
- `ux/specs/loop.spec.mjs` opens it with the keyboard alone in the closed scenario, finds another
  run by a part of its name and opens it: `picker.tabs_to_button`, `picker.hidden`,
  `picker.axe_serious`, `picker.rows_out_of_view` (of the first 8 rows) and `picker.keys_to_open`
  at each gating viewport, new keys taken under D-009.
- The state word in the button has the width of the longest one ("not delivered"), so a run whose
  driver detaches does not move the header (it had raised decider `layout_shift.per_minute` at
  1366x768 from 0.03 to 0.056).
- The played scenarios belong to no experiment, so the measured page lists them without experiment
  groups.
