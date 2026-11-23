# Quality of the 0.1.0 to-do products: SDC runs beside a free agent

The operator, 2026-11-21: "evaluate the quality". Nine delivered products of one idea were judged by
one procedure: six from unattended SDC runs (`experiments/baseline-0.1.0/report.md:43-48`, settings
strong and mixed) and three from free-running agent sessions (one Sonnet session each, run
directories `free-agent-0.1.0-sonnet-{1,2,3}`, not in git). Scores, evidence and screenshots:
`experiments/quality-0.1.0/`. Harness: `eval/quality/`.

## Inputs

- Idea text, the same words on both sides (each run's `idea.yaml`, key `text`): "A to-do list web
  app for me and my partner that also works on the train without a connection".
- The free prompt also named the deliverables (requirements, an ESS spec, a plan of 1 to 3
  stories, the implementation, a self-review) and the stack: Rust 2021, standard library only, a
  daemon that serves a web UI and a JSON API with `std::net::TcpListener`, port and data file as
  arguments, tests per command, `Taskfile.yml`, README. The SDC's implement node gets the same
  stack wording from its binding (lines 382-384 of the model-node script of `driver/examples/sdc/tools/` at the
  baseline's tree 4468bfd).

## Procedure

1. Rubric (`experiments/quality-0.1.0/rubric.md`, 23 items, sha256 a2358f50…) written from the
   idea text before any product was opened. One point per pass; "not testable" only for a harness
   limit, never for a missing feature (none was needed).
2. Blinding: `eval/quality/blind.mjs` copied each implementation (sources, Cargo files, Taskfile,
   README; no PROVENANCE, spec, plan or run data) to `p1`…`p9` in a random order; the key was
   read after all scores and exploratory findings were on disk.
3. Execution: every product was built, tested and started inside the bubblewrap view of
   `sdc-free-agent`'s job confinement (branch `feat/run-timeline`,
   `driver/src/bin/sdc-free-agent.rs`, `base_view` and `job_bwrap`): `/usr` and `/etc` read only,
   toolchain at `/opt/rust`, fresh `/proc`, `/dev`, `/tmp`, home on tmpfs, writable only the
   product copy and its cargo home, empty environment plus PATH and the cargo variables, new PID,
   IPC and UTS namespaces; builds and tests with `--unshare-net`, each a slot job
   (`eval/quality/confine.ts`, `build.ts`). The server and Chromium (Playwright 1.65) ran in a
   network namespace with loopback and a dummy default route that drops every packet
   (`netns.ts`). Offline: `context.setOffline(true)`, and separately the server process killed.
4. Exploratory probes, the same for every product (`explore.ts`, `slow.ts`), at most 5 findings
   each, every finding repeated or deterministic.
5. Static facts by program (`static.ts`): lines of product code without tests, `#[test]` count,
   dependencies, service worker and storage use; routes from the READMEs, checked against route
   literals.

## Results

| run | blind id | score | failed items | own tests | product lines | cost USD | wall s |
|---|---|---|---|---|---|---|---|
| trial-0.1.0-strong-1 | p9 | 20/23 | F3 edit, S3 live, O7 offline edit conflict | 25 | 1356 | 5.432 | 1075 |
| trial-0.1.0-strong-2 | p8 | 23/23 | - | 50 | 1666 | 6.194 | 1247 |
| trial-0.1.0-strong-3 | p4 | 23/23 | - | 42 | 1450 | 5.503 | 1430 |
| trial-0.1.0-mixed-1 | p6 | 20/23 | F3 edit, S3 live, O7 | 38 | 1086 | 2.497 | 1480 |
| trial-0.1.0-mixed-2 | p2 | 22/23 | F5 remove | 37 | 1651 | 3.883 | 2091 |
| trial-0.1.0-mixed-3 | p1 | 23/23 | - | 34 | 1335 | 2.549 | 1489 |
| free-agent-0.1.0-sonnet-1 | p3 | 20/23 | F3 edit, F5 remove, O7 | 17 | 663 | 0.470 | 163 |
| free-agent-0.1.0-sonnet-2 | p7 | 20/23 | F3 edit, F5 remove, O7 | 16 | 623 | 0.414 | 143 |
| free-agent-0.1.0-sonnet-3 | p5 | 19/23 | F3 edit, F5 remove, S3 live, O7 | 19 | 763 | 0.478 | 173 |

Not testable: 0 everywhere. Costs: SDC from the baseline report; free from each run's
`events.jsonl` `result` event (`total_cost_usd`, `duration_ms`).

Per side, median [min, max]:

| side | score | product lines | own tests | cost USD | wall s |
|---|---|---|---|---|---|
| SDC strong | 23 [20, 23] | 1450 [1356, 1666] | 42 [25, 50] | 5.503 [5.432, 6.194] | 1247 [1075, 1430] |
| SDC mixed | 22 [20, 23] | 1335 [1086, 1651] | 37 [34, 38] | 2.549 [2.497, 3.883] | 1489 [1480, 2091] |
| free agent | 20 [19, 20] | 663 [623, 763] | 17 [16, 19] | 0.470 [0.414, 0.478] | 163 [143, 173] |

Where the sides differ (passes out of 3, strong / mixed / free):

- F3 edit a task: 2 / 2 / 0. F5 remove a task: 3 / 2 / 0. O7 conflicting offline edits: 2 / 2 / 0
  (needs edit). No free product has an edit or remove control.
- S3 live update within 10 s: 2 / 2 / 2 (strong-1 and mixed-1 never refresh without reload;
  free-3 polls every 15 s).
- Every other item passed in all nine: build, own tests, start, browser page, list, add, tick,
  restart, sharing after reload, page kept with the browser offline and with the server stopped,
  reopen offline (all nine register a service worker and keep the list in `localStorage`; none
  uses IndexedDB; none has a dependency), offline add and tick reaching the partner, empty, long
  and markup titles.

Exploratory findings (all on the SDC side; none found in the three free products, which also
offer fewer controls to go wrong):

- mixed-1: a change made while the server is unreachable but the browser is online is not sent
  after the server is back (the page retries only on the browser's `online` event or a reload);
  the partner never got it within 16 s. Repeated.
- mixed-3: on a connection delayed 2 s the add box keeps the previous title until the server
  answers; text typed meanwhile is joined to it ("FirstSecond" was added). Repeated.
- strong-1: adding a title that already exists adds nothing (tasks are keyed by their text).
- strong-2: the README's first run command starts `sync-service`, whose page has Add and Edit
  only; the full list is the second binary further down the README.
- No uncaught page error in any product; console errors were only failed requests while offline
  and 409 answers on conflicting changes.

Reading: on this rubric the SDC products are better on edit, remove and offline edit conflicts,
and carry two to three times the code and tests; on the other 20 items the sides are not
distinguishable. The free products cost about a fifth of the mixed and a twelfth of the strong runs and took
an eighth to a ninth of the wall time. No significance claim: three products per side. The SDC costs and wall times
are 2.50 to 6.19 USD and 1075 to 2091 s, not "2.55 to 5.50 USD and 20 to 25 minutes".

## What gave the origin away

Blinding held for file names and run data, not for style: two free READMEs cite `../spec` and
`../plan.md`; SDC READMEs name stories, findings and decisions ("decision q7", `findings.yaml`) or
a proprietary licence line. The scorer also saw one free run's file list before blinding, to find
the delivered trees. The rubric steps were fixed beforehand; each product needed its own locators
(`eval/quality/adapters.ts`), found from its page and README.

## Harness correction

A first pass ran Chromium in a loopback-only namespace (`bwrap --unshare-net`). There Chromium
reports `navigator.onLine` false from the start and fires no `offline`/`online` events
(`eval/quality/probe-online.ts`), so a page that waits for `online` never resyncs. Found on
mixed-1 (O5, O6 failed); the pass was discarded and all nine were run again with `netns.ts`.
Only mixed-1 changed (18 to 20).

## Limits

- One idea, three products per side; the rubric is one reading of the idea text.
- Two browser contexts on one machine stand in for two people; offline is emulated
  (`setOffline`, killed server), not a phone losing its network; service workers only on
  `127.0.0.1`/`localhost`, no HTTPS, no mobile browser.
- The 10 s live limit and the 15 s resync window are the rubric's choice.
- Started from the binary of the README's build (B1) instead of `cargo run`.
- A browser test sees behaviour, not code quality, security beyond markup injection, or data
  integrity over weeks.
- The harness is TypeScript (Playwright), as ordered for this evaluation; the organization's rule
  for new running code is Rust.

## Where the harness lives

`eval/quality/`: `AGENTS.md:26` maps `eval/` to the eval runner described in
`docs/eval-design.md`, and decision D13 (`docs/eval-design.md:633`) keeps the KPI suite in
`sdc/eval/` for now. `eval-design.md` names no product-quality harness; this is the nearest home.
Run: `npm install` in `eval/quality/`, then with `QUALITY_DIR=<scratch>` (default: the parent of the
harness directory, where these runs had it): `node blind.mjs <runs> $QUALITY_DIR`;
per product `node build.ts $QUALITY_DIR pN <build args>`, `slot node netns.ts rubric.ts pN`,
`slot node netns.ts explore.ts pN`, `slot node netns.ts slow.ts pN`, `node static.ts <copy>`;
then `node summary.ts $QUALITY_DIR --unblind` and `node export.ts $QUALITY_DIR <out>`.
