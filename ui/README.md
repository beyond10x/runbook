# SDC run inspector

A web view of an `sdc-drive` case store: where each case stands in the SDC phases and in its Canon
protocol, why (claims down to evidence records, excluded records and the revision that excluded
them), what is admissible next, who acted (program, LLM, person, decider), what each agent step was
allowed to do and did, and run metrics. It updates live while a run writes the store. With
`--write` it is also where a person takes the run's decisions.

## Start

```console
node server.mjs --port 7490 ../runs/trial-todo-1/store ../runs/full-cycle-1/store
```

Open http://127.0.0.1:7490/. Pick the store in the header. Or, from this directory, `task serve`.

- Each argument is a store directory (its subdirectories are root cases) or one case directory. A
  path that does not exist yet is waited for; cases and events show up as the run writes them.
- `--runs DIR` (repeatable) serves every `DIR/<run>/store`, and a run that appears later joins the
  store selector without a restart (`node server.mjs --runs ../runs`). Explicit store arguments come
  first and are not listed twice.
- A run directory with `run-stopped.json` (`sdc.run-stopped/1`: at, by, reason, verdict_at_stop,
  case) is a run stopped on purpose: the Now band says "Stopped by … at …: reason" and what it was
  waiting on, in the past tense; every clock (elapsed, waiting, the graph node, the case tree) stands
  at the stop; the store selector and the case tree mark it; its open decisions show greyed in the
  drawer, and the server refuses to take them (continuing the run stays possible).
- A run with no stop record, not closed, and no driver attached (no live pid in `driver.json` or
  `.sdc-drive.lock`) moves no more: the Now band adds "no driver attached, last activity <time>" and
  every clock stands at the last event, including the waiting time.
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
- Without `--write` it never writes anything. With `--write` the Decisions drawer can act on every
  store, with `--write-only STORE` (repeatable) on those stores only; the principal is `operator`
  unless `--principal NAME` says otherwise.
- LLM calls, tokens, cost and tool calls come from `llm-calls.jsonl` next to the store; decider
  decisions from `decisions.jsonl` there. Panels without data say "not recorded yet".

## Use

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
  (the driver's `next` entries) with the revision and artifact to review and what approving makes
  true, every open scope-changing question, and the history of decisions taken. Acts:
  - Approve runs the driver's own `sdc-drive approve --capability … --principal … --action …`.
  - Answer is one act with the person's principal. While the run waits on a `scope.answer`
    approval (`questions.shaped`, capability `scope.answer`; the tool is
    `sdc-tool.sh scope-answer <principal>`), each pending question is a card showing that approval.
    The answer is written with its status (`accepted` or `changed`) and without `answered_by`;
    with the last open question the server runs `sdc-drive step --action questions.check`, then
    `sdc-drive approve --capability scope.answer --principal <principal> --action scope.answer`,
    and the driver's tool sets `answered_by: <principal>`. Writing the name first would settle the
    check and Canon would drop `scope.answer`. A person who edits the file instead gets the approve
    card, which lists the questions and does the same check and approve. Without a pending
    `scope.answer` the answer is written with `answered_by: <principal>` as before. The principal
    cannot be `non-interactive` (the server exits 2).
  - Change writes a `context.yaml` field: value, status `changed`, reason `<principal>, <date>`.
  - Continue run starts `scripts/run-trial.sh <run>` in the background and streams its output.

  An approval over a structured artifact (`context.yaml`: fields with value, status and reason)
  reads as one decision: "One decision over N fields", who holds it, what taking it does and the one
  Approve control come first, then each field with its value, reason and state (default kept,
  accepted or changed by whom) and, when a decider decided it, what its reply says per field (else
  "approved as a whole"). Opening the drawer puts the focus on that control.

  Every act goes through `POST /api/act` (whitelisted kinds, an argv, never a shell string), is
  refused when the revision on screen is no longer the current one, and is appended to
  `runs/<run>/ui-actions.jsonl`. An act that would start the driver or a tool is refused, and starts
  nothing, when a program it needs is not on the server's PATH or not built (yq, jq, `sdc-drive`,
  canon, and per tool subcommand ess, aep, slot); the drawer names what is missing.

URL parameters for links: `source`, `case` (path below the store, for example
`IDEA-1/children/IDEA-1.intake`), `node` (for example `claim:dissection.valid`), `at` (event seq),
`tab`, `reads=1`, `drawer=1`.

## Where "now" comes from

- In flight: a case's `action_started` without a later `action_finished` (driver `main.rs`).
- The driver: `driver.json` (`sdc.driver/1`: pid, started_at, argv) beside `.sdc-drive.lock` in the
  tree root. The Now band says "driver attached" with the pid when `/proc/<pid>` exists (a live
  lock pid also counts as attached). A step started and not finished with no live driver is shown
  as "driver gone", not running: `/proc/<pid>` absent, or no `driver.json` while an
  `observed.jsonl` exists in the tree. With neither file (an old run) nothing is claimed and the
  step shows as running, as before.
- Since when: `observed.jsonl` beside each `events.jsonl` (`sdc.observed/1`: index, event, action,
  observed_at, pid), matched by line index to the events (and by `event` when the line has it);
  the card says "driver observed_at". Without it, the server's clock of each event line (the mtime
  of `events.jsonl` when the line appeared). The events' `at` is the `--at` of the whole
  `sdc-drive run`, shared by every step of it, so it dates a run, not a step; a step whose start
  the server did not see shows "run since".
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
npm install
slot npx playwright test --workers=2
```

Writes `screenshots/{wide,laptop}-*.png` from a server on port 7490 (`SDC_UI_URL` to change).
`test/now.spec.mjs` starts its own server (ports 7496, 7497, 7500, 7501; `SDC_UI_NOW_PORT` moves them) over runs it plays
with `test/fixture-run.mjs` into `test/fixtures/runs/` (`SDC_UI_FIXTURE_DIR`) and writes
`screenshots/*-now-*.png`; with `SDC_UI_WATCH_RUN` (a run recorded by sdc-watch, such as the copy
`../watch/tests/e2e.sh <scratch>` leaves in `<scratch>/e2e/trial-todo-1`) it also checks the
real recorder's changes. `test/now-model.spec.mjs` tests the derivation without a browser.
`test/changes.spec.mjs` (ports 7503, 7504; `SDC_UI_CHANGES_PORT`) plays the `changes` fixture (every
class of change) and checks the feed, the KPI strip and the panel rail, writing
`screenshots/*-changes-*.png`; `test/meaning-model.spec.mjs` tests the YAML reader, the change
classes and sentences, the feed and the KPIs without a browser.
`test/decisions.spec.mjs` acts, so it runs only against copies of a store: set `SDC_UI_WRITE_URL`
to a `--write` server over the copies and `SDC_UI_WRITE_DIRS` to their two run directories.
`test/scope.spec.mjs` is self-contained: `test/scope-fixture.mjs <dir> [one|two]` makes a store
with the real driver and the stub executors, stopped at `awaiting_authority` for `scope.answer`
(needs `driver/target/debug/sdc-drive` and canon), and the spec answers through a `--write --principal anna`
server of its own on port 7522 (`SDC_UI_SCOPE_PORT`). `fixture-run.mjs` writes `observed.jsonl`,
`driver.json`, `request.json.case` and `status.json` for `agent`, `two` and `decider`, and the old
files only for `person`; `gone`, `gone-nofile` and `legacy` are static runs for the absence cases.
`test/holder-model.spec.mjs` tests the Now band's wording, the holder of a decision and the fields
of one approval without a browser.
`test/preflight.spec.mjs` (port 7524, `SDC_UI_PREFLIGHT_PORT`) starts a server whose PATH has jq and
no yq: every act that would start the driver or a tool is refused with the missing programs named
(nothing runs, no check is recorded), and the Decisions drawer shows the same message up front.
`test/stopped.spec.mjs` (port 7528, `SDC_UI_STOPPED_PORT`) serves `--runs` over played runs (`person`,
`held` with a decider in its settings, `stopped`, and `late`, which appears while the page is open)
and checks the stopped state, the failed waiting KPI and the live store list; `test/autopilot.spec.mjs`
(port 7530, `SDC_UI_AUTOPILOT_PORT`) checks that an autopilot run (fixture `autopilot`) never names a
person and that a legacy run (fixture `human`) says "recorded as human". Screenshots at both gating
viewports: `screenshots/{1600x1000,1366x768}-{stopped,stopped-drawer,held,autopilot,autopilot-drawer,human}.png`.
`test/stall-model.spec.mjs` tests the stalled case (a wait on `amend.upstream` with no finding for a
person names the writer that used its runs and its last error).
`test/paths.spec.mjs` (port 7526, `SDC_UI_PATHS_PORT`) fails when a file git would commit under
`ui/` or any page or API answer names an absolute home path; the server shows paths relative to
the repository, its workspace (`../`) or `~`.
`runs/` is git-ignored, and the suite exits 0 on a checkout without it. `stopped.spec.mjs` and
`autopilot.spec.mjs` play their runs from `test/recorded/trial-todo-1-slim`, a committed slim copy of
`runs/trial-todo-1` (cases intake and direction; no prompt, transcript or answer of an LLM, no
absolute home path; remade with `node test/slim-source.mjs`). `now.spec.mjs`, `changes.spec.mjs`,
`meaning-model.spec.mjs` and `live.spec.mjs` need the recorded runs `runs/trial-todo-1` and
`runs/full-cycle-1`, `screens.spec.mjs` a server over `runs/full-cycle-1`, and `scope.spec.mjs`,
`preflight.spec.mjs` and the second test of `paths.spec.mjs` the built `sdc-drive` and `canon`:
without them they are skipped through `test/need-run.mjs`, the reason naming what is missing. The
ux loop and the server without arguments still need the recorded runs and say which is missing.

## UI iteration loop

`ux/` holds the measured loop of the `ui-iteration-loop` skill. `ux/adapter.mjs` names the gating
viewports (1600x1000, 1366x768, the operator tile 805x1270 and the phone 390x844; 1280x720,
1920x1080 and the phones 360x800, 414x896 and 768x1024 are measured only), the one theme, what is
always visible (Now band with its run state, KPI strip, phase bar, store selector, Decisions), the
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
ratchet; `test/mobile.spec.mjs` (port 7532, `SDC_UI_MOBILE_PORT`) holds one test per phone finding
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
  with an ellipsis and a title, and the store select is as wide as its choice.
- The phone sizes are measured as phones (touch and a coarse pointer: `isMobile`, `hasTouch`), in
  the loop, `test/mobile.spec.mjs` and `test/stub-runs.spec.mjs`, which also checks the tile.
