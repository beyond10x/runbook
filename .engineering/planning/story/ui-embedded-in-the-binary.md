---
format: aep.planning-md/3
id: story:ui-embedded-in-the-binary
kind: story
status: active
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
revision: 9
transitions:
- {from: "draft", to: "proposed", at: "2026-11-22T01:47:34Z", actor: "human:operator", revision: 2, executor: "agent:plan-onebin", correlation: "operator-requirement-2026-11-22-one-binary"}
- {from: "proposed", to: "active", at: "2026-11-22T10:13:13Z", actor: "human:operator", revision: 7, executor: "agent:onebin-u-ui-embed", correlation: "operator-standing-acceptance-2026-11-22"}
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

### What the security review changed

An independent review of slice 1 wrote 12 tests (`runbook/tests/daemon/ui_embed_review.rs`, committed red:
6 of 12 failed) and six findings. Each is fixed on `impl/daemon-ui-embed` and the 12 tests are green. Where
the text above says otherwise, this section is the current state.

- F1, one guard for every route (`runbook/src/api/auth.rs`; commands, views, event streams, `/mcp`, UI files,
  `/ui/login`; with and without `--ui`, with and without a token). The order is Host, Origin, token,
  content type. On a loopback listener `Host` must be `127.0.0.1:<port>`, `localhost:<port>`, `[::1]:<port>` or
  the listen address, else 403 before any other work; a non-loopback listener (token required) has no Host
  check. An `Origin` that is not `http://` and the request's own host and port is 403 (`null` too); no
  `Origin` passes. `POST /daemon/commands/<name>` and `POST /mcp` take `application/json` only, else 415. The
  bodies are constant and name no header value. The MCP endpoint's own Origin check is gone: the guard does
  it. `api::guarded` is gone: the extra routes are merged before the one guard layer (`api::serve_all`).
  Proof: `ui_guard.rs` (4 route sets of 6 routes each, the Host forms, a non-loopback listener, 415, the
  cross-site `text/plain` post) and the review file.
- 401, 403 and 415 are in the document (`docs/daemon-api.md`) only, not in the specification
  (`ess/domains/daemon.yaml`, the generated OpenAPI): 401 never was, so 403 and 415 are not added. The
  OpenAPI's own 403 ("not granted", with an `actor`) is another answer that this daemon never sends. No
  regeneration was needed.
- F2, the cookie holds `hex(SHA-256("runbook-ui-read/1" 0x00 token))`, compared in constant time for GET
  and HEAD only; it is refused as a bearer token.
- F3, `/ui/login` is a route behind the same guard: any method but POST without the token is 401. The POST
  carries its token in its body, so the guard lets it through (after Host and Origin) and the login checks it.
- F4, `docs/daemon-api.md` says that the cookie has no `Secure` (plain http only) and that a login over a
  non-loopback address sends the token in clear: the page is for loopback or a tunnel.
- F5, `runbook/build.rs` fails the build on a symbolic link (file or directory) and names it; the embedded
  list test compares with `git ls-files ui/web` (tracked files only).
- F6, `ui/web/source.js` passes `position` and `sequence` through `Number()` before they enter markup.
- The daemon enforces no minimum token length: `load_token` in `runbook/src/api/auth.rs` refuses an empty
  token only. Not changed.

## Slice 2

Branch `impl/daemon-ui-decide` (base `160c4df85`). Goal story moved: this one, acceptance line 3 (a decision
request of a stub run is answered from the page's drawer through the ESS-specified command, and the run
continues). Not in this slice: removal of `ui/server.mjs` and the other Node files (line 5), the Loom runtime
as the daemon's default, and any command or view added to the specification.

What was built.

- Page: `ui/web/decide.js` (new, embedded by the build like every file of `ui/web/`; no inline script, the policy
  is unchanged). When the daemon serves the page, the Decisions drawer lists the open decision requests of the
  selected run (question, each option with its letter, what it does, cost, what it admits and blocks) and
  answers one: an option, or free text. Hooks: `source.js` answers `/api/inbox` from the view
  `GET /daemon/views/open-decisions`, `app.js` calls `renderDaemonDrawer` in place of `renderDrawer` (and
  reads the inbox when a run is selected, only for a daemon's page), `style.css` has the few rules of the
  form. Every text of the daemon is set with `textContent`; the module has no `innerHTML`.
- The command is the daemon's own `answer-decision`, the one `runbook decide` sends. One answer is one
  request: `POST /daemon/commands/answer-decision`; headers the page sets: `Content-Type: application/json`
  and, only for a daemon with a token, `Authorization: Bearer <token>`; `credentials: 'omit'`, so no cookie
  goes with it; the browser adds `Origin` (the page's own), `Host` and the usual. Body: `run_id`,
  `request_id`, `answer`, `answered_by`. No new route, no change of the guard or of the specification.
- Token. The read cookie is unchanged (`GET` and `HEAD` only). The page asks for the token once, in a field
  of the drawer, and keeps it in a variable of the module: no cookie, no `localStorage`, `sessionStorage` or
  `IndexedDB`, not in the URL, the history state or the DOM (the field is emptied when read). A reload
  forgets it and the next command asks again, before anything is sent. A wrong token is `401` and the daemon's
  message shows on the page; the page does not send again. Whether a token is needed is found with one read of
  `GET /daemon/views/configuration` without credentials (`401`: needed); a daemon without a token on
  loopback takes the command without a bearer and the page shows no token field.
- Principal. The command needs `answered_by`; the daemon authenticates none, so the form asks for a name
  and says it is a label the daemon records (as `--by`).
- Stub runtime: `StubRuntime::decision_script`, the preset `stub-decision` of `runbook serve`. Two requests at
  once, the question made from the intake (`{intake}` and `{run}` in an `Ask` are filled per run, so the
  request ids are unique per daemon), `stub.answered` after both are answered, outcome `approved`. Without it
  no run of the binary waited.
- After an answer the page reads the daemon again: the views `runs` and `open-decisions`, and the event
  stream goes on (`stub.answered` shows in the feed). The daemon records no event when a run starts to wait
  or goes on, so the page also asks again every two seconds while visible.

What did not hold.

- "By which step": the views `open-decisions` and `decisions` carry no case, action or capability
  (`ess/domains/daemon.yaml`, view `OpenDecisionsOfRun`; `runbook/src/commands.rs`, `open_decisions_of_run`),
  although the request has them (`DecisionRequest`). The page shows the question and the options and does
  not say which step asked. Adding the three fields to both views is a change of the specification and of
  the generated contract: open for the orchestrator.
- The gate config has projects `wide` and `laptop`, no mobile project; the phone is a `test.use` of a
  phone viewport (as `mobile.spec.mjs` does), inside project `wide`.
- `aep plan artifact body` takes no actor or correlation.

Tests, with their time.

- `ui/test/daemon-decide.spec.mjs` (gate lane, project `wide`; six tests, three runs of the file with
  `daemon-ui.spec.mjs` 24 of 24 passed, 51 s with load, the slowest 7.3 s): the open decisions show and an
  answer ends the wait, 3 to 6 s; token asked once, wrong token refused, one request only, 6 to 13 s (a 2.5 s
  wait that proves no second send); a reload asks again, 3 to 5 s; token in no cookie, storage, URL, history
  state or DOM, 2 to 5 s; markup in a question is text, 1.6 to 2 s; phone, 2 to 4 s. Mutants killed: the
  token field not emptied (two tests fail), credentials sent with the command (the token test fails).
- The whole UI step: 119 passed, 1 failed (the phone test measured the drawer while it slid in; it waits for
  it now), 51 s of 120 s.
- Crate (`slot task test:runbook`, 263 tests, 4.1 s): `ui_decide.rs`, in the one test binary:
  `a_cookie_alone_does_not_answer_a_decision` 0.011 s (the generic cookie test
  `a_cookie_authorizes_get_and_head_and_nothing_else_on_any_route` covered the route; this one shows the
  decision stays open), `a_decision_from_another_origin_is_refused_with_a_token_and_without_one` 0.018 s,
  `a_decision_is_answered_by_json_only` 0.015 s, `the_page_keeps_the_token_in_memory_and_writes_no_markup`
  0.007 s, `the_binary_has_a_stub_run_that_waits_on_two_decisions_and_goes_on_when_they_are_answered`
  0.018 s; `stub.rs`: `an_ask_is_filled_with_the_run_id_and_the_intake_so_that_two_runs_of_one_script_do_not_collide`
  0.012 s. `the_embedded_list_is_the_directory_and_the_committed_files` holds with `decide.js` added.
  None over 5 s.
