---
format: aep.planning-md/3
id: story:ui-embedded-in-the-binary
kind: story
status: proposed
title: UI embedded in the binary and served beside the API
summary: runbook serve --ui serves the embedded ui/web files from the daemon API and SSE; the Node server is removed.
tags:
- one-binary
relations:
- decomposes: epic:runbook-one-binary-and-daemon
- depends_on: story:daemon-skeleton-on-a-runtime-trait
- depends_on: story:engine-on-loom-library-entry
- informed_by: epic:live-view
scope:
- confidence: inferred
  path: runbook/build.rs
- confidence: inferred
  path: runbook/src/ui.rs
- confidence: cited
  path: ui/README.md
- confidence: cited
  path: ui/decisions.mjs
- confidence: cited
  path: ui/package.json
- confidence: cited
  path: ui/server.mjs
- confidence: cited
  path: ui/test
- confidence: cited
  path: ui/udiff.mjs
- confidence: cited
  path: ui/vclock.mjs
- confidence: cited
  path: ui/web
revision: 6
transitions:
- {from: "draft", to: "proposed", at: "2026-11-22T01:47:34Z", actor: "human:operator", revision: 2, executor: "agent:plan-onebin", correlation: "operator-requirement-2026-11-22-one-binary"}
---
# Story: ui-embedded-in-the-binary

## Outcome

`runbook serve --ui` serves the web UI from files embedded in the binary, beside the API, reading
the daemon's API and SSE stream; several runs show at once. The Node server is removed at the end.

## Source

The operator, 2026-11-22: "we want the web UI to be embedded into the binary or into the server. So
when we do runbook serve, or runbookd the daemon, then we should just also specify --ui ... and
then it would ... serve it besides the API itself. And multiple such runs should be possible to run
there."

## Context

The UI is `ui/web/` (static files) with `ui/server.mjs`, `ui/decisions.mjs`, `ui/vclock.mjs` and
`ui/udiff.mjs` as its Node server and Playwright specs under `ui/test/`. Each of the Node server's
routes the UI calls is replaced by an API route or an SSE event of the daemon; where one has no
equal, the missing command goes into the ESS specification first
(`story:daemon-api-specified-in-ess`). The existing UI stories name `ui/server.mjs`; their scope
moves with this one (the orchestrator decides; this story edits none of them).

## Acceptance

- The files of `ui/web/` are compiled into the binary at build time: `runbook serve --ui` started from
  an empty directory with no `ui/` answers `GET /` with 200 and the content of `ui/web/index.html`
  byte for byte, and `GET /app.js` likewise; without `--ui`, `GET /` answers 404.
- The UI reads runs from `GET /daemon/views/runs` and events from the SSE routes of the daemon; with two stub
  runs in two sessions the page lists both and the feed of each shows its own events (a Playwright
  spec, in the existing `ui/test/` set, two workers).
- A decision request of a stub run is answered from the UI's drawer through
  `POST` of the ESS-specified command, and the run continues (Playwright spec).
- The UI route is guarded by the daemon's token rule: on a non-loopback listener a request without
  the token gets 401.
- At the end of the story `git ls-files ui/server.mjs ui/decisions.mjs` prints nothing,
  `grep -rn 'server.mjs\|decisions.mjs' --include='*' . --exclude-dir=node_modules --exclude-dir=.git
  --exclude-dir=.engineering --exclude-dir=docs` prints nothing, and `ui/README.md` and `ui/package.json`
  say how the UI is started now.
- The Playwright step ends within 120 s with no test over 60 s.

## Scope

`ui/web/` (reads the API; static files stay one copy), `ui/server.mjs`, `ui/decisions.mjs`,
`ui/vclock.mjs`, `ui/udiff.mjs` (removed or ported to the daemon where still used), `ui/test/`,
`ui/package.json`, `ui/README.md`, `runbook/src/ui.rs` (new), `runbook/Cargo.toml` (an embedding
crate or `include_dir`), `runbook/build.rs` if needed, `AGENTS.md` (directory map line for `ui/`).

## Slice 1

Branch `impl/daemon-ui-embed` (base `7965b98b`). Goal story moved: this one, acceptance lines 1 (files
compiled in, `GET /` byte for byte, 404 without `--ui`), 4 (token rule) and the first half of 2 (the page
lists two runs and shows each run's events). Not in this slice: answering a decision from the drawer
(acceptance line 3), removal of `ui/server.mjs` and the other Node files (line 5), moving the scope of the
other UI stories, and any command added to the specification.

What was built.

- `runbook/build.rs` lists `ui/web/` at every build (dot files skipped) and `runbook/src/ui_embed.rs`
  includes each file with `include_bytes!`: a file added to `ui/web/` is embedded by the next build, with no
  list to keep. No new dependency (`sha2` was there; the base64 of a CSP hash is ten lines). Test
  `the_embedded_list_is_the_directory_and_the_committed_files` holds the list equal to the directory and to
  `git ls-files ui/web` (tracked, or not yet added and not ignored). Embedded: 25 files, 589,541 bytes (the debug binary: 13,006,600 bytes before, 14,025,856 after).
- `runbook serve --ui`: the files are the router's fallback, so they sit behind the daemon's one token
  middleware. A path is looked up as sent: `/fonts/`, `/..`, `/%2e%2e/x`, `/..%2fx`, `//app.js`, `/app.js/`
  and 14 more shapes (20 in all) answer 404 (`no_directory_listing_and_no_path_outside_the_embedded_list`).
- Headers of every embedded file: `X-Content-Type-Options: nosniff`; `Cache-Control: private, no-cache` with
  `ETag` = SHA-256 of the bytes and `304` on `If-None-Match` (the paths carry no hash, so a file is
  revalidated by its digest, never cached blind); `Referrer-Policy: no-referrer`; `Server-Timing: runbookd`
  (how the page knows who served it, without a request that a Node server would answer 404).
- Content security policy, chosen after reading the page: `index.html` has ONE inline script (the phone
  breakpoint, `window.SDC_PHONE`); `app.js`, `lenses.js` and `now.js` write `style="..."` attributes into their
  markup (7 lines: indent, bar width, playhead, stage grow, turn bar, change strip); no `<style>` element, no
  `eval`, no `setAttribute('style')`, one `data:` image (the select arrow in `style.css`). The narrowest policy
  that works: `default-src 'none'; script-src 'self' 'sha256-<the inline script>'; style-src 'self';
  style-src-attr 'unsafe-inline'; img-src 'self' data:; font-src 'self'; connect-src 'self'; base-uri 'none';
  form-action 'self'; frame-ancestors 'none'`. The hash is computed from the embedded `index.html` when the
  daemon first serves it, so an edit of the script cannot leave a stale hash. Proof: the Playwright spec
  records every `securitypolicyviolation` and every console error while it drives the page, and both lists
  are empty.
- Token. A non-loopback listener has a token by the daemon's floor, so `GET /` there is 401 without it
  (`a_non_loopback_listener_gives_the_ui_to_the_token_only`: the binary on `0.0.0.0:0`). The check is the one
  of the API: the same middleware calls `Auth::admits`. A browser cannot send a header with a page load, so
  it hands the token over once in a form body: the 401 of `GET /` with `Accept: text/html` is a form that
  posts to `POST /ui/login` (the one route outside the middleware; checked by `Auth::admits`), which answers
  303 to `/` with the cookie `runbook_ui` (`HttpOnly`, `SameSite=Strict`, the token in hex). The token is in
  a body and a cookie, never in a URL, so no log has it. The middleware turns the cookie into the same
  bearer check for `GET` and `HEAD` only; a cookie never authorizes a `POST` (the answer-a-decision slice
  will need its own header rule). A form with another `Origin` is 403. Tests:
  `a_browser_hands_over_the_token_in_a_form_body_and_keeps_it_in_a_cookie`,
  `the_token_rule_guards_the_ui_through_the_same_check_as_the_api`, `without_ui_a_cookie_is_not_a_token`.
- The page, served by the daemon, reads one new module, `ui/web/source.js`. When the Node server serves the
  page it changes nothing (`fetch` of the old routes, `EventSource('/events')`); the existing Playwright set
  passes unchanged. When the daemon serves it: `/api/sources` is `GET /daemon/views/runs`; the events of every
  run come from one stream, `GET /daemon/views/daemon-events` with `Accept: text/event-stream` (read with
  `fetch`, because an `EventSource` hears named kinds only and a kind is open text; resumed by position after
  a drop); the routes that describe a case store answer an empty store. The hooks in the existing files are
  six lines in `app.js` (import, `openEvents()`, the empty-store branch shows the run's events and a note) and
  one in `picker.js` (a run row says the daemon's own state word). The page lists each run as
  `<runbook_id> <last 8 of run_id>` and names the daemon's state (`Ended · done`, ...) and nothing else.

### Inventory: what the page asks of the Node server

The page calls 13 routes: 12 `/api/*` and `/events`. The files `GET /` and `GET /<file>` are the embedded
files now. 5 routes have an equal in the daemon's API, in whole or in part; 8 have none.

| # | Route | Returns | Equal in the daemon |
|---|---|---|---|
| 1 | `GET /api/sources` | the runs: name, path, state, times, cost and calls, models, counts, experiments, program kind, stop, last write, view info | in part: view `runs` (+ `sessions`). Fields it lacks are listed below |
| 2 | `GET /events` (SSE) | `hello`, `change` (cases, run, live), `sources`, `proc` and `procend` (stdout of a continued run), `act` | in part: `daemon-events` / `events` stream for `change` and `sources`. `proc`, `procend`: no equal, needs a stream of a run's console output. `act`: not needed (an answer shows as an event) |
| 3 | `GET /api/meta` | compiled protocol IRs, check output, eval levels, tool paths | no equal: needs a view `protocols` (the compiled IR of each protocol the runtime knows) |
| 4 | `GET /api/view` | the runbook's view of the run (`runbook.view/1`) | no equal: needs a view `runbook-view` of a run |
| 5 | `GET /api/view-info` | whether a view covers the run, its protocols and problems | no equal: the same view |
| 6 | `GET /api/tree[?cases=]` | the case tree: each case's `case.json`, IR, bindings, events, evidence, evaluations, authority | no equal: needs a view `cases-of-run` (waits on `engine-on-loom-library-entry`) |
| 7 | `GET /api/run` | `run.log` commands and verdicts, model call records, console, decisions made, stop record, settings | in part: view `run` holds `settings`, `state`, `outcome`, times. Commands, call records, console and stop record: no equal, needs a view of a run's calls and console |
| 8 | `GET /api/live` | records still open, drivers, the file changes of the run | no equal: needs views `run-activity` and `run-file-changes` |
| 9 | `GET /api/workfacts` | the planning files of the store | no equal: needs a view of the work files of a run's workspace |
| 10 | `GET /api/fschange?seq=` | one file change with its diff (`udiff.mjs` makes the diff when the recorder left none) | no equal: needs a view `run-file-change` with the diff |
| 11 | `GET /api/inbox` | open decisions, decision history with grounds, `write`, principal, missing programs, stop | in part: views `open-decisions` and `decisions` (request id, case, action, capability, question, options, answer, answered by and at). History grounds (reasons, cites, model, cost), `write`, principal, missing programs and stop: no equal |
| 12 | `POST /api/act` | `approve`, `answer`, `context`, `continue` (`decisions.mjs` runs them) | in part: `approve` and `answer` are `answer-decision`. `context` (change a context field) and `continue` (resume a stopped run): no equal, need commands `set-context` and `resume-run` |
| 13 | `GET /api/blob?case&hash` | the text of a blob of a case | no equal: needs a view `blob` by hash |

So: 5 routes with an equal in part (1, 2, 7, 11, 12), 8 without (3, 4, 5, 6, 8, 9, 10, 13). No route has a
complete equal yet. `vclock.mjs` (a shared test clock for the ux loop) and `binaries.mjs` need no equal: they
stay with the Node server until it goes. Nothing was added to the specification in this slice.

Fields the page uses and the daemon's views do not carry (the page shows none of them, and none is made up):

- run row (`runs`): cost, calls (work, decider), models, reopens, forced, unstuck, experiments (id, variant,
  ordinal), program kind (stub or real; the daemon says only in its start line that its runtime is a stub),
  who stopped a run and why (`Cancelled` has no actor), whether an `Ended` run delivered (`outcome` is open
  text), a name for the run, the run directory, the number of cases.
- decisions: the drawer is empty and its count 0 in daemon mode, even when a run waits: `open-decisions` is not
  read in this slice, so that count is NOT the truth for a run in `WaitingOnDecision`.
- the whole case model: tree, graph, phases, KPIs, claims. A run of the daemon shows a note in their place.
- run feed: each event shows `kind`, its sequence in the run, its time and its record; the page's
  "what changed" classes (planning, specification, decision, code) need the file-change view of row 8.
- "last write" of a run is the time of its last event (else its `submitted_at`); the page orders runs by it.

### Tests, with their time

Crate (`runbook/tests/daemon/ui_embed.rs`, in the one test binary; `slot task test:runbook`: 188 passed, test
run 20.5 s of 120 s, none over 5 s that is mine): `every_file_of_ui_web_is_served_at_its_path_byte_for_byte`
0.12 s, `the_embedded_list_is_the_directory_and_the_committed_files` 0.04 s,
`content_types_follow_the_extension` 0.08 s, `without_ui_the_root_and_every_file_answer_404` 0.01 s,
`embedded_files_carry_the_security_headers_and_a_digest_cache_rule` 0.17 s,
`the_csp_hash_is_base64_of_the_sha256` 0.01 s, `no_directory_listing_and_no_path_outside_the_embedded_list`
0.23 s, `a_query_does_not_change_the_file_served` 0.07 s,
`the_token_rule_guards_the_ui_through_the_same_check_as_the_api` 0.09 s,
`a_browser_hands_over_the_token_in_a_form_body_and_keeps_it_in_a_cookie` 0.11 s,
`without_ui_a_cookie_is_not_a_token` 0.03 s,
`serve_ui_from_an_empty_directory_answers_the_index_and_without_ui_404` 0.08 s (the binary, started from an
empty directory), `a_non_loopback_listener_gives_the_ui_to_the_token_only` 0.10 s (the binary on `0.0.0.0:0`).

UI (`ui/test/daemon-ui.spec.mjs`, gate lane, project `wide` only; the binary is built by `task test:ui`
first): the daemon page lists both runs and each feed shows its own events, 2.3 s; a run submitted while the
page is open appears and its events arrive, 1.3 s. Both check that no `securitypolicyviolation` and no console
error happened. The Node-served specs pass unchanged: `view.spec.mjs` and this spec together 11 passed in 24.6 s;
the subsets of the other specs that failed in full runs on a host at load average 90 to 150 pass when run alone.
The full step took 114 s once at lower load (3 failures, all in `view.spec.mjs`, caused by a probe request of
`source.js` that logged a 404 in the console; replaced by the `Server-Timing` header) and 120 to 540 s at load
above 130: the step's 120 s budget is not met by this change or any other while the pool is loaded like that;
a measurement of the step needs a quiet host.
