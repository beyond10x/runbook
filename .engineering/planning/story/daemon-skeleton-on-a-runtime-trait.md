---
format: aep.planning-md/3
id: story:daemon-skeleton-on-a-runtime-trait
kind: story
status: active
title: Daemon skeleton on a runtime trait with a stub runtime
summary: runbook serve keeps many runs behind the specified API and SSE with resume, on a stub runtime, with the loopback and token floor.
tags:
- one-binary
relations:
- decomposes: epic:runbook-one-binary-and-daemon
- depends_on: story:daemon-api-specified-in-ess
scope:
- confidence: cited
  path: runbook/Cargo.toml
- confidence: inferred
  path: runbook/src/api
- confidence: inferred
  path: runbook/src/commands.rs
- confidence: cited
  path: runbook/src/main.rs
- confidence: inferred
  path: runbook/src/runtime.rs
- confidence: inferred
  path: runbook/src/state.rs
- confidence: inferred
  path: runbook/src/stub.rs
- confidence: cited
  path: runbook/tests
revision: 10
transitions:
- {from: "draft", to: "proposed", at: "2026-11-22T01:47:32Z", actor: "human:operator", revision: 2, executor: "agent:plan-onebin", correlation: "operator-requirement-2026-11-22-one-binary"}
- {from: "proposed", to: "active", at: "2026-11-22T05:12:16Z", actor: "human:operator", revision: 7}
---
# Story: daemon-skeleton-on-a-runtime-trait

## Outcome

`runbook serve` is a daemon that keeps many runs, answers the API specified in ESS and streams
events over SSE, on a runtime trait with a stub runtime. No real runtime is needed for the
conformance suite of `story:daemon-api-specified-in-ess` to be green.

## Source

The operator, 2026-11-22: "using the CLI would allow us to start a case, and this would either
happen basically in process or via a daemon, and the daemon would maintain all the different
runs. And different sessions."

## Context

One command layer (`runbook/src/commands.rs`) holds submit, list, read, cancel, decisions and
sessions; HTTP is one face of it, MCP (`story:mcp-exposure`) and the CLI the others. The runtime
trait has the shape of `runbook::engine` (design decision 2: start a run, answer a decision,
cancel, event sink); the stub runtime emits scripted records and decision requests; Loom
replaces it in `story:engine-on-loom-library-entry`. The security floor of design decision 10
belongs here: loopback by default, a bearer token required for any other address, one check for
every route.

## Acceptance

Every route below is one of the generated OpenAPI document `ess/generated/openapi/openapi/runbookd.yaml`
(commands are `POST /daemon/commands/<name>`, views are `GET /daemon/views/<name>`); the daemon
serves no other path for the API.

- `runbook serve --listen 127.0.0.1:0` prints the bound address on one line and keeps running; with
  `--listen 0.0.0.0:7000` and no token (`--token-file` or `RUNBOOK_TOKEN`) it exits 1 and prints
  that a token is required; with a token, a request without `Authorization: Bearer` gets 401 on
  the `/daemon` routes, the event streams and the UI route, and a request with it gets 200.
- `POST /daemon/commands/submit-run` answers 202 with outcome `submitted` and a run id;
  `GET /daemon/views/runs` lists it; `GET /daemon/views/run?run_id=<id>` shows its state;
  `POST /daemon/commands/cancel-run` moves it to `Cancelled`; a second cancel is refused 409 with
  `RunAlreadyEnded` and the state stays `Cancelled`.
- `GET /daemon/views/events?run_id=<id>&after_position=0` with `Accept: text/event-stream` sends
  events whose SSE `id` is the event's `position`, strictly increasing, with the runtime's record
  unchanged plus run id and sequence number; reconnecting with `Last-Event-ID: <n>` sends exactly
  the events with a position after `n`, with none repeated and none lost; `session-events`
  (`session_id`, `after_position`) and `daemon-events` (`after_position`) do the same.
- A stub run that asks a decision shows up in `GET /daemon/views/open-decisions?run_id=<id>`;
  `POST /daemon/commands/answer-decision` accepts the answer once, and a second answer to the same
  request is refused 409 with `DecisionAlreadyAnswered`.
- A session groups runs: after `POST /daemon/commands/open-session` for `s1` and `s2`, two runs
  submitted into `s1` and one into `s2` are listed per session by
  `GET /daemon/views/runs-of-session?session_id=<id>`, and `session-events` of `s1` carries only
  the events of its two runs.
- A run's record (its store and logs) is under `$XDG_STATE_HOME/runbook/runs/<id>`, and the
  directory for its workspace is `$XDG_STATE_HOME/runbook/workspaces/<id>`, a child of the one
  shared directory `$XDG_STATE_HOME/runbook/workspaces` (the caller's duty of Loom's
  `concurrent-confined-runs`, `docs/library-entries.md`, "What the daemon must do for confined
  runs"); after the daemon is stopped and started again, `GET /daemon/views/runs` lists the same
  runs with the same states.
- The 1 MiB intake limit is counted in bytes by the handler (1,048,576 bytes of UTF-8), tested with a
  multi-byte text.
- The checks of `ContextOutsideRoots` and `NoSuchRunbook` read the host and are tested here.
- The conformance suite of `story:daemon-api-specified-in-ess` runs against the stub daemon and all
  its scenarios pass.
- Budget: no test over 60 s and the test step within 120 s from job start; one daemon is started
  once per test run (a setup step or a cache on disk, not once per test, and not an in-process
  `OnceLock` under cargo-nextest); the step prints its ten slowest tests; each test over 5 s is named
  with its time in the report. New tests go into the existing `runbook/tests/` binary.

## Scope

`runbook/src/` (new modules `api/`, `commands.rs`, `runtime.rs`, `stub.rs`, `state.rs`; one added
subcommand in `main.rs`), `runbook/Cargo.toml` (an HTTP server crate), `runbook/tests/`,
`Taskfile.yml`. Shared files with the later stories are `runbook/src/main.rs` and
`runbook/Cargo.toml`: C lands before D to H, which add their own modules and one registration line
each.

## Part C1

Branch `impl/daemon-skeleton`: runtime trait and stub (`runtime.rs`, `stub.rs`), state and journal
(`state.rs`), the command layer (`commands.rs`), HTTP and `runbook serve` (`api/`, `serve.rs`).
All tests are in `runbook/tests/` (modules `daemon::stub`, `daemon::state`, `daemon::commands`,
`daemon::http`, `daemon::openapi`).

Met by part C1, with the test that shows it:

- Serve line and token floor: `serve_prints_the_bound_address_keeps_running_and_checks_the_token_of_the_file_or_the_environment`
  (the printed address, 401 without the bearer and 200 with it, token file over environment),
  `a_listen_address_that_is_not_loopback_needs_a_token_and_the_process_exits_1_at_once` (exit 1 within 2 s),
  `a_token_file_that_others_can_read_is_refused_and_the_message_names_the_mode`,
  `a_token_is_checked_by_one_middleware_on_every_route` (command, view and unknown routes).
- Submit, list, show, cancel, second cancel `RunAlreadyEnded`: `one_round_trip_per_client_command`,
  `one_round_trip_per_view`, `a_cancel_moves_the_run_to_cancelled_and_a_second_cancel_is_refused`.
- Open decision and one answer: `a_decision_is_answered_once_and_the_run_goes_on`,
  `a_second_answer_is_refused_and_names_the_reason_the_specification_orders_first`.
- Sessions: `sessions_group_runs_and_their_events`.
- Record, workspace and restart: `the_record_and_the_workspace_are_children_of_the_state_directory`,
  `after_a_restart_the_views_list_the_same_runs_with_the_same_states`,
  `without_a_state_dir_the_record_is_under_xdg_state_home_runbook`.
- Intake in bytes: `the_intake_limit_is_counted_in_bytes`,
  `the_intake_limit_holds_over_http_and_the_body_limit_is_a_little_over_what_a_valid_submit_needs`.
- `ContextOutsideRoots` and `NoSuchRunbook`: `a_context_outside_the_roots_is_refused_also_through_a_link_and_through_dots`,
  `a_runbook_is_known_by_the_runtime_table_or_the_runbook_directory`.
- Every route of the document and no other: `the_server_has_the_routes_of_the_openapi_document_and_no_others`;
  every refusal of every client command: `every_refusal_of_every_client_command_has_a_trigger_and_the_specification_s_name`.
- Event views as a stream are 406 and name part C2: `an_event_view_asked_as_a_stream_is_406_and_names_the_next_part`.

Waits for later parts:

- SSE with `Last-Event-ID` and the stream routes (the three event views are JSON pages here).
- 401 on the UI route (there is no UI route yet).
- The conformance suite against the stub daemon.
- The budget runner: the 60 s kill of a test and the ten slowest tests printed by the step. No test
  here is over 5 s alone; one daemon is started per test in process at a measured cost
  (`starting_a_server_costs_milliseconds`), not once per run.

Gaps in `ess/domains/daemon.yaml`, listed and not changed:

- Line 801-824: the guards of `answer-decision` check the run's state before the request's, so the
  second answer to the only open request of a run (which has resumed or ended) is `RunNotWaiting`;
  `DecisionAlreadyAnswered` is reached only while another request keeps the run waiting.
- Line 884 (`EndRun`) and the runtime-side commands: no command ends or fails a run from outside the
  runtime, so after a restart a run that was `Running` or `WaitingOnDecision` keeps its state and gets
  the event `runbook.orphaned`.
- Line 492: `intake.count` counts characters, the handler counts bytes (line 139 says 1 MiB).
- Lines 520-531: `already-submitted` says the answer is the first run, but its event `SubmitReplayed` carries only the idempotency key; the id of the first run is given in the `Location` header only.

## Part C2

Branch `impl/daemon-sse`, from the head of part C1: the three event views asked with
`Accept: text/event-stream` are streams (`runbook/src/api/sse.rs`; `docs/daemon-api.md`, "Event
streams"). It replaces the `406` of part C1. All tests are in `runbook/tests/daemon/sse.rs`.

How it works: the state keeps a version that grows with every commit (`State::subscribe`, a
`tokio::sync::watch`); a stream holds a position, reads the events after it from the state
(`State::events_page`), sends them and waits on the version when there is none. It reads events from the
state and never from the notification, so a notification that is missed or doubled loses and repeats
nothing. No new crate and no axum feature: `axum::response::sse` is always built; tokio gains the
features `sync` and `time` and `futures-util` (already in the tree, behind axum) is named in
`Cargo.toml` for `stream::unfold`.

Decisions: an `after_position` is optional on a stream (none means 0, the whole record); the header
wins and then the parameter is not read; an unknown run or session id answers as the page form does
(`200`, no row), so the stream is `200` and ends at once; the bound of a slow client is 1,024 events
(`Streams::max_lag`), counted from events recorded after the connection (the replay at connect is
exempt), and the stream is closed with a comment line that names the position to resume from; the
keep-alive is 15 s (`Streams::keep_alive`) and is no `serve` option, because no existing option fits.

Met by part C2, with the test that shows it (each one is under 0.3 s unless a time is given):

- What a stream sends: `a_stream_sends_every_recorded_event_in_position_order_then_each_new_event`
  (all three views; `id` is the position, `event` the kind, `data` equals the row of the page).
- Resume and the rule that the header wins:
  `a_stream_resumes_after_the_position_of_the_header_or_of_the_parameter_and_the_header_wins`; no event
  twice or skipped while a stream connects: `no_event_is_lost_or_sent_twice_while_a_stream_connects`
  (12 rounds of 50 events recorded from another thread, started at a barrier at the moment of the
  connect; 2.3 s).
- A `Last-Event-ID` that is no position is `400`, a position beyond the last waits:
  `a_last_event_id_that_is_not_a_position_is_refused_400_and_a_position_beyond_the_last_waits`.
- The end of a run, session and daemon stream:
  `a_run_stream_ends_after_its_last_event_when_the_run_is_terminal`,
  `a_session_stream_ends_when_the_session_is_closed_and_all_its_runs_are_terminal`,
  `the_daemon_stream_ends_only_at_shutdown`; SIGTERM and exit 0 within 2 s (measured 12 to 240 ms):
  `sigterm_closes_every_open_stream_and_serve_exits_0_within_2_s`.
- Keep-alive at an injected interval: `a_comment_line_is_sent_as_keep_alive_at_the_injected_interval`.
- A slow client: `recording_never_waits_for_a_stream_and_a_client_that_falls_behind_is_disconnected_and_resumes_by_position`
  (5.7 s, the only test over 5 s; most of it is 40 events of 256 KiB that fill the connection's buffers,
  recorded at debug speed). The bound is injected as 100 in this test; the default 1,024 is asserted.
- The token: `a_token_is_checked_on_a_stream_as_on_every_route`.
- Unknown ids: `an_unknown_run_or_session_answers_as_the_page_form_does_before_any_stream_starts`.

Outside the files C2 was given: `runbook/src/commands.rs` has two additions, `Daemon::read` (the
state under its lock, for `api::sse`) and `event_rows` made `pub(crate)`, because the state is private
to that module; `runbook/tests/daemon/http.rs` lost the test of the `406`.

Known limit: a client that holds a connection and never reads keeps its connection task until the
process ends; `serve` returns one second after the shutdown began whether or not it finished.

## Part C5

Branch `impl/daemon-budget-clock`: the test budget of the crate is enforced by its runner, the daemon
reads one clock, and the stories name the routes that exist.

- Budget: `runbook/.config/nextest.toml` (30 s period, terminate after 2: a test over 60 s is killed and
  fails) and the task `test:runbook` (build with `--no-run` first, then the timed run, the ten slowest
  tests, exit non-zero over 120 s), a dependency of `task check` in place of the crate's `cargo test`.
  The forge runs the same task for a change classified as code (`.github/workflows/pages.yml`, with
  go-task and cargo-nextest installed from checksummed release archives as `surf` does). Before this the
  forge ran no test of the crate. Step times: `cargo test --locked` 41.5 s test run, 47 s job; nextest
  40.8 s run, 57 s job (both with the pool queue wait not counted, 171 tests, before the clock). The
  in-process `OnceLock` of `runbook/tests/daemon/openapi.rs` reads and parses the generated OpenAPI
  document (well under 1 s per test, no test of it in the ten slowest), so it stays. Test binaries carry
  no debug info already (`.cargo/config.toml` of the repository sets `debug = false` and wins over the
  manifest here: `readelf -S` on `target/debug/deps/cli-*` shows no `.debug_*` section, 13 MB).
  Proof of the kill: a scratch test sleeping 70 s was `TIMEOUT [60.154s]`, the summary said `1 timed
  out` and the task exited non-zero; the test is not committed.
- Clock: `runbook/src/clock.rs` has the trait `Clock`, `SystemClock` (the only production
  implementation) and `SetClock` (tests). The state holds the clock and `State::now` is the one place
  the daemon reads the time for a record (12 call sites of the former free function `state::now`, all in
  `commands.rs`; `Instant::now` in `doctor.rs` and `stub.rs` measures durations and records nothing).
  `Daemon::start` takes `SystemClock`; `Daemon::start_with_clock` is called by test code only: `serve`
  does not call it, and no option, environment variable or request reaches it. The conformance runner
  already built the daemon in process on `127.0.0.1:0`; it now builds it with a `SetClock` and moves the
  clock to the `at` of each command step before the step runs, and compares instants as instants.
- Conformance: before the clock the two scenarios, read strictly, failed (12 executed, 10 passed, 2
  failed: `answered_at` and `closed_at` held the wall clock); after: 12 executed, 12 passed, 0 skipped,
  and `GATE` says so. The `ended_at` claims removed by the earlier unit are back in
  `daemon-cancel-a-run` and `daemon-a-run-whose-runtime-is-gone-ends-as-orphaned`. The interpreted target
  still passes 15 of 15. Red proof (not committed): `cancel_run` stamping `2020-01-01T00:00:00.000Z`
  turned `daemon-cancel-a-run` red on `ended_at`.
- Routes: the story's own routes (`/daemon/commands/<name>`, `/daemon/views/<name>`) are the 5 commands
  and 12 views of the generated OpenAPI document and of the route table; two other stories of the epic
  named `GET /v1/runs`, which exists nowhere, and now name `GET /daemon/views/runs`
  (`ui-embedded-in-the-binary`, `confinement-level-per-run`). Not changed, outside the routes:
  `confinement-level-per-run` says a submit below the floor answers `403`; the specification and the
  daemon answer `409` (`BelowConfinementFloor`).
