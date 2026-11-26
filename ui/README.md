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

The page has two modes and its header names the mode. **watch**: this server serves it, and it only
reads (no start form, no token field, `405` to every `POST`). **operate**: the daemon serves it
(`runbook serve --ui`): the same page, plus "Start a run" (a runbook from the daemon's view
`runbooks`, the intake, the confinement at or above the daemon's floor; `daemon/start.js`) and the
answer to a decision (`daemon/decide.js`), and "Routes", the daemon's model routes with alias, kind,
gateway, status and phase (`daemon/routes.js`; the header of a run on a route names the route, its kind
and its status). These files are in `daemon/`, which this server does not serve.

On a run, the daemon's page has none of the inspector's bands it has no data for (KPIs, Now, phases, the graph and its
legend, the timeline). Its centre lists the run's steps read from the run's events (`web/runsteps.js`): step, role,
state, start, duration and try, the step in flight marked with its duration counting, a line that says what runs now
or how the run ended, and the counts (steps done, model calls, tokens, cost). A step ends at the run's
`runbook.step-ended`, not the run; a `Stopped` with `BudgetSpent`/`MaxSteps` none names is a wait or the end of the
tree, and in a run with no `runbook.step-ended` (an older runtime) that `Stopped` line ends the step (`stepEnds`). The
run events are said in words, the engine's own lines folded.
The side panel has only the tabs the daemon serves data for, Products and Events (`tabsOf`, `web/source.js`), and
Metrics when the daemon's view `run-metrics` answers a row for the run (`daemon/metrics.js` draws it; a step's model
calls whose events name none are taken from that row, `withViewCalls` in `web/runsteps.js`, and an unknown count is
said as not known, never 0; `test/daemon-metrics-tab.spec.mjs`). The Metrics pane says times, token counts and costs
in the step list's words (`19:53:00 CET`, `1,200 in, 800 out`, `$0.0042`), names a step's case only when it is not the
run's own, by its short id, and draws a step as six columns (step, role, state, took, calls, cost) that fit the panel at
1280 px with a line under them for the rest; no pane says what is missing in place of showing
something. Times on the page are 24 h local time of the browser with the
zone's name (`web/clock.js`). Specs: `test/daemon-live.spec.mjs` over the synthetic daemon `test/daemon-live-fixture.mjs`
(`node test/daemon-live-fixture.mjs --live` serves it to look at), `test/daemon-steps-model.spec.mjs`.

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
  the code that answers a decision lives in `daemon/decide.js` and the code that starts a run in
  `daemon/start.js`, which only the daemon serves.
- Each argument is a store directory (its subdirectories are root cases) or one case directory. A
  path that does not exist yet is waited for; cases and events show up as the run writes them.
- `--runs DIR` (repeatable) serves every `DIR/<run>/store`, and a run that appears later joins the
  run picker without a restart (`node server.mjs --runs ../runs`). Explicit store arguments come
  first and are not listed twice.
- When the server reads: only for a page that looks, or for a request. With no page connected it does
  nothing (no timer runs, no run file is read; measured: 50 runs, under 1 % of a core). A page opens
  `/events?source=<index>` for the run it shows, and while that stream is open a round every 700 ms
  reads that run only (the stream is opened again when the page shows another run), and a file whose
  size, modification time, inode and status-change time are those of its last read is answered from memory
  (the memory held is at most 16 MiB, the entries and their overhead counted, a text with a character above
  U+00FF at two bytes per character; the least recently used entry goes first; an entry whose file is gone,
  refused or changed leaves when it is seen; every call still passes the run-path check first). Another run
  is read when a request names it (`/api/sources` reads each run once, then only what changed), so the run
  picker is as old as its last opening, except that a page that opens its stream within a minute of a list
  that showed another state of its run is sent `sources` at once and reads the list again. A line that is
  written to a run nobody looked at for 5 s is dated by the file's time, as on first sight. A run that
  appears under `--runs` is announced to every page. A page that is more than 1 MiB of output behind (it
  does not read) is dropped: its stream is ended and the page opens it again. With the environment variable
  `SDC_CLOCK_DIR` set (the clock of the ux loop; the server has no option for it) every run is read on each
  step of the clock, as before. `--count-reads` (a test option) counts the rounds and the reads of run
  files by run at `GET /api/reads`, and says at `GET /api/rounds` whether the timer of the rounds runs and
  how many pages hold a stream; without it neither route exists. `--round-ms N` (a test option, 10 to 700)
  sets the time from one round to the next, 700 ms without it.
- A run directory with `run-stopped.json` (`sdc.run-stopped/1`: at, by, reason, verdict_at_stop,
  case) is a run stopped on purpose: the Now band says "Stopped by … at …: reason" and what it was
  waiting on, in the past tense; every clock (elapsed, waiting, the graph node, the case tree) stands
  at the stop; the run picker and the case tree mark it; its open decisions show greyed in the
  drawer; the inspector takes none of them.
- Whether a run is live: `runbook run` writes `settings.json` when it starts, `run-stopped.json` or a
  `status.json` with state `closed` when it ends, and `runner.json` (its pid, the start time the kernel gave it,
  and `closed_at`) with an `action_started` line in `events.jsonl` for each action that starts
  (`docs/loom-run-directory.md`, "The runner and the step in flight"). An ended run is stopped or closed (above).
  A run with `runner.json` is one of three, by `ui/runnerlive.mjs`: **running** (the runner lives and an action
  started after its last step ended: the band says "<action> running since <time>, <n> min", the minutes
  ticking), **between steps** (the runner lives and no action is in flight: "Between steps"; a case whose last
  Loom event is `step_budget_spent` is not "Stopped" then, that event ends every `tree run --max-steps 1`) or
  **not live** (the record is closed, or no process of that pid has that start time, or the process has ended and its
  parent has not yet collected it, state `Z` or `X`: "Not live: last started <action>", and a step it started is not
  shown running; the clocks stand at the last event). The server asks the kernel one thing, whether
  `/proc/<pid>/stat` field 22 equals `start_ticks` (and field 3 is no `Z` or `X`): it reads that file, sends no
  signal and needs no permission beyond the read of `/proc`; a read that fails for another reason than "no such
  process" makes the run unknown, with that reason in `liveness.why` of `/api/live` (the page says "live state unknown" and shows no reason). The run files are read through the checked open as
  every other. `runner.json` and the start lines are what files of the run directory claim: under `--confinement
  local` a program of the run can write them, so they are read as text and a claim, nothing acts on them, a
  `runner.json` over 4 KiB, or one that is not a regular file, has more than one name or is not a record, is not read (unknown; `liveness.why` says which), and the `action`, `case` and `source` of a start line are cut to 256
  characters. A run directory with `settings.json`, no end record and no `runner.json` (an older runner)
  is "live state unknown", as before: the Now band and the run picker say so and every clock stands at the last
  event, including the waiting time. A case store an older runtime left (no `settings.json`) is history:
  reported "not live", and a step it started and never finished is shown as "Not live", not running.
  `/api/live` carries this as `liveness` (`ended`, `running`, `between`, `not_live`, `unknown` or `history`;
  with `runner` {pid, started_at, closed_at} and `step` {action, case, try, at, source, role}
  for `running`, `between` and `not_live`).
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
  versions) is refused with exit code 2. There is no route, token field or proxy to the daemon, and
  the header of its page says "watch".
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
  high ×2", or "3 variants × 3") and how many of its ended runs passed clean (closed with a good end,
  nothing forced or unstuck); its rows go by variant, then ordinal, each tagged "variant #n". A run
  belongs to an experiment by its `experiment.json` (`sdc.experiment-run/1`: `experiment`,
  `variant`, `ordinal`, `tree_commit`, …) or by its directory name in an experiment record's
  `variants[].runs` (`experiments/<id>.yaml` of the checkout served; `--experiments DIR`,
  repeatable, reads others); a run in two records is a row in each. Each row has the run's name, its state (running; closed
  with its outcome and a check when it is a good end; closed, not delivered, with a cross, when the
  view lists good ends and not this one; "ended: <outcome>" with a diamond and no colour when
  nothing says whether it is a good end, see `good_outcomes`; stopped with its cause; not running), when it started
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
  the final front matter, and the decisions. A run of `runbook run` without `fs/changes.jsonl` (started
  without `--watch`, `docs/cli.md`) says "Changes were not recorded for this run" and names `--watch`,
  which records every file change of the run in `<run>/fs/changes.jsonl`; "Nothing" is said only when
  that file exists and is empty.
- **Changes** (right panel tab): every file change of the run from `fs/changes.jsonl`
  (docs/fs-changes.md), on a time strip and grouped by the action that made it, with the tool call
  and the attribution basis (`exact`: inside one driver action; `inferred`: matched to one agent
  tool call by time; `call`: in an agent or decider call's directory outside any action). A late
  `sdc.fs-attribution/1` line supplies the tool of an earlier change. Click a file for its
  syntax-highlighted diff (Rust, TS/JS, YAML, JSON, Markdown, shell, TOML); ← and → step through.
- **Phase strip** (top): the phases of the runbook's view (the SDC phases of `docs/phases.md` for the SDC
  runbook), each derived from Canon claims of the
  case tree, never from a stored phase. Click a phase to open the claim that holds it back. A run of
  `runbook run` writes no `evaluations/*.json` and no claim count, so for it the strip, the case tree and
  the Metrics tile say "claims not recorded for this run" in place of a count; a count shows only when
  evaluations exist. The case header and the event list show the protocol revision of the `opened`
  event, and no "r" when the event has none.
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
The protocol of a case is read from its `case.json` (`protocol`), else from the `opened` event that starts its
`events.jsonl`, so a run of `runbook run`, which writes no `case.json`, gets its view too
(`test/view-loom-store.spec.mjs`).

The graph of a case of `runbook run` is built from what the runner copied into the run directory, read
with `yq` as `work/questions.yaml` is (`runfacts.mjs`): the protocol's artifacts and actions
(`protocols/*.protocol.yaml`) and the bindings copy (`bindings/*.yaml`, `loom.bindings/1`: each
action's executor, the placement of each artifact). The store's own `ir.json` and `bindings.json` hold
only the effect of each action and the person and copy actions. An action is drawn as an action, an
artifact as an artifact with the path the run placed it at. A part that cannot be read (no
`protocols/` copy, no `bindings/` copy, no `yq`) says "not available for this run" (a chip on the
action, a line in place of the graph), never "unbound"; "unbound" is said only of an action that a
readable bindings copy does not bind (`test/graph-any-runbook.spec.mjs`).

A view is checked when the server starts: a member the inspector does not know, or one of the wrong
type, refuses the start with a message that names it (`decisions.scope_answer.escalate: view member
"escalate" is not known to the inspector (runbook.view/1)`), exit code 2. A new member needs a change
of `web/view.js` (`VIEW_SCHEMA`), not only of the file.

Members (all optional but `format`):

- `runbook`: its name. `protocols`: regular expressions of its protocol ids; the view covers a run
  when one case of the run has such a protocol. Without `protocols` the protocols of `phases.items`
  count, and a view without either covers every run.
- `good_outcomes`: the outcomes of the runbook's protocols that are a good end of a run, for example
  `"good_outcomes": ["operating", "delivered"]` (the SDC view). A good end is declared, never guessed
  from the SDC outcomes. A Canon `protocol/1` outcome has a `description` and `requires` and no
  class (`deny_unknown_fields`), and the runner's `status.json` says `delivered: true` only for
  `operating`, `fixed` and `delivered`, so the view is where a runbook says it. A closed run is
  shown in one of three states, in the run picker and wherever the run's end is named: good (a check)
  when `status.json` says `delivered: true` or the view lists its outcome; bad (a cross, "closed ·
  not delivered") when the view lists good ends and not this one; neutral (a diamond, "ended:
  <outcome>", no red) when there is no view, or the view has no `good_outcomes`. A run that was
  stopped (`run-stopped.json`) is "stopped", whatever the view says.
- `product`: the main file a run of the runbook produces, one string: a path relative to the run directory
  (`"product": "work/example/example.md"`). The page links it under the header, one click from the first
  screen. It is refused where the view is checked (the server start and `runbook check`) when it is not a
  string, is empty, is absolute, leaves the run (`..`) or has a component that starts with a dot. Without
  the member the page takes `work/<runbook>/<runbook>.md` when the run has it, else the Markdown files
  directly under `work/` and one level below, the newest first, and says which rule named the file. A
  view that names a file the run has not written says "the view names <path>; this run has not written
  it".
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

## What a step produced: the Products tab

The Products tab of the panel (and the product line under the header) shows what the steps of a run
produced: the evidence records of the case with every field and nested value, the actions with their exit
code, and the files of the run with their content. It reads and acts on nothing.

**A product is a file an action of the run wrote or changed.** A file the run was seeded with (the copy
of a repository, the intake, a goals file) is an input, not a product. The tab lists the products grouped
by the step that wrote them, newest step first, and shows the inputs as one closed line, "Inputs", with
the seeded trees and their file counts. Two records of the run directory say which is which, and neither
is compared by time:

- `fs/changes.jsonl` (`docs/fs-changes.md`): every file the watcher saw change under `work/`, with the
  step that was in flight. It needs a run started with `--watch`.
- `seeded.json` and `seeded-files.jsonl` (`docs/loom-run-directory.md`): what the runner listed in `work/`
  when the seed ended, each file with the SHA-256 of its content. A seeded file is a `create` of step 1 in
  the first record, the same as a file the first step writes, so the hash tells them apart: a path that is
  listed and whose last change ends at the listed hash is an input and is not listed; listed and ending
  at another hash it is a product marked `changed`; not listed it is a product (marked `added` when it lies
  in a seeded tree); listed and deleted by an action it is marked `removed`. A path in either record goes
  through the run-path check like every other path: one that leaves `work/` or the run is not listed and not
  opened, and is only counted. A name is looked at one component at a time from the real path of the run, and a
  link is resolved by the run-path check before the next component is looked at, so a name behind a link that
  leaves the run is counted as left out (`leftOut`), the same whether or not anything lies behind it; `gone` counts
  only names that lie in the run and are not there.
- The answer of `/api/products` has `basis`: `records` (as above: `groups` by step, each with its `actions`
  and `files`; `files` flat in the same order; `inputs` `{trees, files}`; `total`, `shown`, `notShown`,
  `truncated`, `cut`, `leftOut`, `gone`, `linkCut`, `maxLinks`, `linksResolved`, `refusedRecords`, `cutStrings`, `maxAction`, `recordsStopped`) or `listing` (an older run, one without
  `--watch`, or a seed record that is incomplete: every file under `work/` as before, and `basisNote` says
  why). The products are not cut by depth; the page lists at most 500 and says how many are not shown. Every string of a record
  that enters the answer has a bound, and the answer says when it cut one: an action name is cut at `maxAction` (256)
  characters (a character of more than 16 bits counts one, and the cut never falls inside one) and counted in
  `cutStrings` (`cut: "strings"`), a path longer than 1024 characters is not listed and is counted in `refusedRecords`.
  An answer resolves at most `maxLinks` (8) link components of one name; a name past it is left out like a name behind a
  link that leaves the run, and is counted in `linkCut` besides `leftOut` (`cut: "links"`); `linksResolved` is the number
  of link components the answer resolved over the names it lists (at most 500), at most `maxLinks` per name. The server keeps at most
  100 000 records of `fs/changes.jsonl` for one run and at most 64 MiB of their lines, without the diff; a late
  attribution line that replaces one already kept counts what it adds to the longest line counted for that change (the
  variables `SDC_UI_MAX_RECORDS` and `SDC_UI_MAX_RECORD_BYTES` set the bounds, for a test; a value that is not a whole
  number of at least 1 that a number holds exactly, at most 2^53 - 1, is the default), and it keeps the state of the records with them, so an answer for a run
  that did not change reads no record again; each answer still looks at the files it lists. When the reader stops before
  the end of that file (a line longer than 32 MiB or bytes with no line end in a whole chunk, the 100 000 records or the
  64 MiB) the answer has `recordsStopped` `{cause: "line" | "records" | "bytes", read, offset, size, why}`,
  `truncated: true` and `cut: "records"`; a file the reader cannot take one record from is answered as `listing` with the
  reason. The live feed is cut at the same bound: `/api/live` carries the same `recordsStopped` in `fs` (the feed and the
  File changes tab show it as a note, the tab also when the list holds no record), and `/api/fschange` answers a change after the last record the list holds as
  `404` "past the bound" with `recordsStopped`, never as "no such change".
- The file the product line names is the last file the last finished step wrote (`via: last-written`),
  never a file of an input tree; a view's `product` or the runbook's own file is refused when it is an
  unchanged seeded file. For an older run the newest Markdown file is taken, but not one inside a tree with
  a build manifest at its top.

- Routes, all GET and behind the guard of every route (a Host that is not this server's is 421):
  `/api/products?source=N` (the files under the run's `work/` and the `work/` of each case, with size,
  format and, for a `.txt` step file, its `result:` line; the product and the rule that named it; how many
  entries were left out), `/api/file?source=N&path=P` (one file: its text in a JSON member, or its size and
  type), `/api/output?source=N&case=C&seq=S` (the tail of an action's output). Every answer is JSON with
  `X-Content-Type-Options: nosniff`; no file is served as a type a browser renders.
- Every read goes through the run-path check of `runfiles.mjs`: the real path lies under the run. The type
  is read before the file is opened and only a regular file is read: a directory, a named pipe, a socket
  and a device are refused (415, with the type) and never opened; a link that leaves the run is refused
  (403). A path that is absolute or climbs out (`..`) is 400. After the open, the descriptor must still be
  the file that was checked (its real path from the kernel is the checked path): a directory on the way
  that is swapped for a link between the check and the open does not make another file read. A regular file
  with more than one link (a hard link may be a file outside the run, and only its link count tells) is
  refused (403) and not listed. The check is made on the open descriptor's own real path (the kernel's
  name for it): the file route, the listing, the output route and the blob route each ask it whether that
  path may be served before a byte is read. Where the system cannot give that name (no `/proc`) the file is
  not read (403, `unverified`): the read fails closed.
- The file route and the listing serve only what lies under a `work/` directory of the run, by name and by
  real path: the run's `work/` and the `work/` of each of its cases. A path outside them (`settings.json`,
  `store/...`, `llm-calls.jsonl`) is 404, as a link in `work/` that points at one is. A `work/` is a root
  only when its real path is `<real parent>/work`, a real directory directly in a real directory of the
  run: a `work/` that is a link (to the run directory, to `store/`, anywhere) is no root, and serves and
  lists nothing. Evidence records and the output of an action have their own routes.
- The listing is bounded in every direction: it lists a file by `lstat`, does not descend a link to a
  directory, visits each real directory once, reads a directory an entry at a time (a directory of a
  million entries costs no more than the bound), and stops at 500 files, 2000 directories or 20000 entries
  of any kind; a directory deeper than 6 levels is not entered. The answer says it was cut (`truncated`,
  `cut`: `files`, `directories`, `entries` or `depth`) and the page says by what. A file that is not
  listed because it has more than one link, or is a link that is not served, is counted (`leftOut`) and the
  page says how many. The page names as the product only a file the file route serves.
- The run log of a case is read through the same type check, in pieces from its end, at most 8 MiB of it.
  An action finish that is not in that part answers "not found within the last 8 MiB of the run log". The
  blob (`/api/blob`) must be a file of the run's `store/blobs/` by its real path, with one link, under no
  dot-directory (a `case` with a dot component is 404, as for the file route); it is read through the
  same bounded regular-files reader, at most 200000 bytes of it, and a file over that is 404, not read
  whole.
- A path the request names without a dot, whose real path has one (a link into a dot-directory), is
  answered as any path that is not served (404, "only a file under a `work/` directory of the run is
  served"); the answer does not say what the target is.
- The other reads of the server's own are bounded too: a file read as text (`readText`) over its limit is
  not read at all, and a regular file with more than one link is not read (`null`, as an absent file), for
  every caller (`console.txt`, `expected-llm-calls.txt`, the events, the case files; the default limit is 64 MiB; the diff of blobs reads at most 8 MiB per file, the work facts
  64 KiB). The reads of the run's own small control files (`settings.json`, `events.jsonl`, case
  state) keep the default bound; no smaller size is defined for them. The three reads that need a part of
  a file go through the same checked open as the blob and the file route (`readRegular` of `runfiles.mjs`:
  one descriptor, its real path under the run, a regular file, one link), on the one descriptor: the first
  64 KiB of a case's `events.jsonl` (its protocol), `fs/changes.jsonl` from the offset the page has seen (in
  chunks of 32 MiB), and one change of it by its position. A file that fails the check is read as absent
  (`fs.present` is false; the change is 404). No read of a file of a run opens it any other way; the files
  the server reads without that check are not files of a run (the checkout's protocols, docs and
  experiment records, the files of `--view` and `--clock`, the page's own files). A position (`at`) is a
  whole number of bytes from the start, else nothing is read; a position at or past the end is an empty
  window. The reads of the decisions (`decisions.mjs`) have the run as root, the step log of a failed
  action the case's `blobs/`, and a name with a dot part is refused; the record a writer names (`llm-calls.jsonl`)
  is read only under the run. No program opens a file of a run by its name: the text of a YAML file is read
  once through the checked open and given to `yq` on standard input, and `yq`'s messages are not shown.
  `test/runfiles-inventory.spec.mjs` holds the list of the calls that open, read or list a file.
- An evidence record nested deeper than 64 levels is cut by the server at load (`cut at depth 64`), and the
  page draws at most 24 levels of it (`cut at depth 24`).
- A path with a component that starts with a dot (a tool's cache or login) is neither listed nor served: 404
  for the file route, also through a link and an encoded dot; the listing says how many entries it left out.
  There is no switch to show them.
- A file over 1 MiB is shown cut at 1 MiB with its full byte count. A file that is not UTF-8 text (a NUL
  byte or bytes that are not UTF-8) is shown by its size and type only. JSON is shown indented, YAML and text
  as written, Markdown rendered.
- The Markdown renderer (`web/markdown.js`) is code of this repository with no dependency. It builds
  elements from text and never assigns markup; raw HTML in a file stays text; an image is a link with its alt
  text and loads nothing; a link opens only when it is an http or https address (a new tab, no opener) or a
  path inside the run (opened in the panel); any other address is shown as text with its address. Its cost is
  bounded: a line of white space costs one pass, a document makes at most 30000 elements, list items
  included (the rest is shown as plain text with a line that says so), and a table has at most 50 columns, 500 rows and 5000 cells.
- An action's exit code is shown as recorded. Its output is the one blob the run keeps (standard output
  followed by standard error), tail of 4096 bytes. `runbook run` records no start, end or duration of an
  action, only a whole-second span for its step (`observed.jsonl`): the page says "not recorded by this
  run" and shows the span of the step. It shows standard output and standard error as one stream: the run
  does not keep them apart. The blob of a run of `runbook run` is found through Loom's own run log
  (`store/runlog-case-<hex of the case id>.jsonl`, `blob_digest`, `store/blobs/<sha256>`); the runner's
  projected events do not carry the digest.
- Served by the daemon (`runbook serve --ui`), the page has the same tab and line. The tab lists the files of the
  run's `work/` directory (the daemon's views `products` and `product`); the line counts the files and stands with the
  run's steps (`docs/daemon-api.md`, "The web UI").

Specs: `test/step-products-api.spec.mjs`, `test/step-products-page.spec.mjs`, `test/markdown-hostile.spec.mjs`,
`test/view-product.spec.mjs`, `test/daemon-ui.spec.mjs`, `test/daemon-run-tree.spec.mjs`, `test/daemon-run-tree-security.spec.mjs`, `test/step-products-bounds.spec.mjs`,
`test/written-model.spec.mjs`, `test/written-api.spec.mjs`, `test/written-page.spec.mjs`,
`test/markdown-bounds.spec.mjs` and the reviewer's `test/security-products-*.spec.mjs`.

## Where "now" comes from

- In flight: a case's `action_started` without a later `action_finished`.
- Whether the run is live: see "Whether a run is live" above. A run with the runner's record says
  running, between steps or not live; a run directory of an older runner with no end record says "live
  state unknown"; a case store of an older runtime says "not live", and a step it started and never
  finished shows as "Not live", not running.
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

Each spec of the test step is in one list of `test/lanes.mjs`: `WIDE_ONLY` (runs at 1600x1000 only, with the reason in five words) or `BOTH` (runs at 1600x1000 and 1280x800; today `screens`). The Playwright config reads the list: the laptop project ignores the wide-only files, so a spec that does not need a second size costs no skipped entry. `test/budget.spec.mjs` fails for a spec in no list or in two, for a reason of no word or over five words, for a `BOTH` file that skips on the project name, and for a wide-only spec whose code matches the `SIZE` pattern (`viewport:`, `setViewportSize`, `viewportSize`, `boundingBox`, `getBoundingClientRect`, `scrollWidth`, `innerWidth`, `toHaveScreenshot`, `screenshot(`, `isVisible`, `toBeVisible`, `toBeHidden`, `isHidden`, `toBeInViewport`, `scrollIntoViewIfNeeded`, `matchMedia`) without an entry (with its reason) in `SIZE_EXCEPTIONS`. A spec waits for an event: `settled(page)` (`test/settled.mjs`) returns when fonts are loaded and the finite animations have ended; `fixture-run.mjs` stops at the file it gets as its fourth argument until a test writes it (a shell variable holds nothing). A sleep is a `waitForTimeout` (a call, with a space before the bracket, as a bracket member or as a reference), a `setTimeout` that a `new Promise` with an arrow function waits for, or a window `Date.now() + <number>`; it is counted per call site in the gate's specs and in every file they import by `from '<path>'` or `import('<path>')` with a relative path (`./`, `../`, `ux/` too; an `import '<path>'` without bindings is not followed). Any other spelling is not found, among them a `function` callback of a promise, `timers/promises`, `Atomics.wait`, an alias of `setTimeout`, `setInterval` or a window whose time is no number, and it does not count a `waitForFunction` or `expect.poll` timeout or interval, a timer that ends a child or a request or the number of times a loop runs a call. A sleep is made only where `FIXED_WAITS` in `test/lanes.mjs` names the file, the count and the sum of the literal milliseconds, and `budget.spec.mjs` fails any other count or sum. A sleep whose time is the argument of a helper (`later(250)`) needs the entry to say `variable` with a reason, the `helper` and the `largest` number its call sites in the file pass: a call site over it or with no number fails. A window needs `window` with a reason; an entry may carry a `why`; each reason is five words at most. The helper check does not find a second sleep helper in a file, a helper that another file calls, a call written `later.call(null, 60000)` or `later?.(60000)` and an alias (`const l = later`): no file of the gate has one of these forms, and a new one needs a reviewer's eye. The fixture player `test/fixture-run.mjs`, which the specs spawn and do not import, holds 12 `sleep(<ms> / speed)` call sites and one lag timer, `after(1500 / speed, lag)` at line 281, that no entry lists and the check does not read (a case counts the `sleep(` sites). The Playwright config takes a file as a test only as `test/<name>.spec.mjs`, directly under `test/` (`SPEC_FORM` in `test/lanes.mjs` starts at that directory and is matched on the absolute path, so a spec below a subdirectory, also one below a directory that is itself named `test`, is no test; the cases "the config does not take a spec below a nested directory named test" and "the stray check tells a spec below a nested directory named test" hold it), and `budget.spec.mjs` fails a file anywhere below `test/` that Playwright's own default match (`*.spec.*`, `*.test.*`, also in a subdirectory) would take and that form does not; it reads the files of `test/` and its subdirectories except `fixtures/` (git-ignored scratch; the other specs write their run directories under `target/fixture-runs`, `test/fixture-dir.mjs`) and `recorded/` (committed data). A beforeAll that needs several fixture runs or servers starts them at once (`playFixtures` in `test/fixtures.mjs`, `Promise.all` over `startServer`).

`test/now-model.spec.mjs`, `meaning-model.spec.mjs`, `holder-model.spec.mjs` and
`stall-model.spec.mjs` test the derivations without a browser; `runner-live-model.spec.mjs` does it for the three
states of a run with the runner's record, and `runner-live.spec.mjs` serves hand-made live runs (`runner-fixture.mjs`) and holds the
server's answer and the page's three texts at the wide size. `test/read-only.spec.mjs` proves the
inspector acts on nothing: every route that took an act answers 404 or 405, a server started with
an option of the removed acts exits 2, and a Decisions drawer shows the line on where a decision is
answered and no control that acts; its last test holds the mode: the page says "watch", has no
start form and no token field, sends no command, and a `POST` of the start command is `405`.
`test/daemon-start.spec.mjs` is the other mode: four daemons (stub runtime with a runbook directory,
the same with a token and a floor of `local`, one whose runbook goes away, one on the Loom runtime)
and the page that starts a run. `test/daemon-routes.spec.mjs` runs a daemon with two routes on a
stand-in gateway on loopback (one managed and starting, one hosted and ready): the header names each run's
route, the Routes view lists both, and with it open a daemon that refuses every request is asked at most 4
times a second. `test/view-race.spec.mjs` holds two sources with two views on one
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
with their truth, and the daemon's run page at a live run (scenario `daemon-live`, `ux/specs/daemon.spec.mjs` over
`test/daemon-live-fixture.mjs`). `ux/core/` is generic: the layout contract with axe-core, the screen-only user's
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
