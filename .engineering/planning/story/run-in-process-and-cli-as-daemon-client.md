---
format: aep.planning-md/3
id: story:run-in-process-and-cli-as-daemon-client
kind: story
status: active
title: runbook run in process and the CLI as a daemon client
summary: 'One command layer, two modes: runbook run in process, and runs, decide and events against a daemon.'
tags:
- one-binary
relations:
- decomposes: epic:runbook-one-binary-and-daemon
- depends_on: story:daemon-skeleton-on-a-runtime-trait
- depends_on: story:engine-on-loom-library-entry
- informed_by: story:runbook-cli-skeleton
scope:
- confidence: inferred
  path: runbook/src/client.rs
- confidence: cited
  path: runbook/src/main.rs
- confidence: inferred
  path: runbook/src/run.rs
- confidence: cited
  path: runbook/tests/cli.rs
revision: 6
transitions:
- {from: "draft", to: "proposed", at: "2026-11-22T01:47:33Z", actor: "human:operator", revision: 2, executor: "agent:plan-onebin", correlation: "operator-requirement-2026-11-22-one-binary"}
- {from: "proposed", to: "active", at: "2026-11-22T09:23:15Z", actor: "human:operator", revision: 5, executor: "agent:onebin-k-cli-client", correlation: "operator-standing-acceptance-2026-11-22"}
---
# Story: run-in-process-and-cli-as-daemon-client

## Outcome

`runbook run` runs one case in process through the engine, and the same commands talk to a daemon
when one is named: one binary, two modes, one command layer.

## Source

The operator, 2026-11-22: "What we want is one pre-built runbook binary, which can just run a
certain case, given that context, the intake, and given the protocol which is being specified, and
given certain command line arguments." and "using the CLI would allow us to start a case, and this
would either happen basically in process or via a daemon".

## Context

Design decision 3 gives the commands. `runbook run` keeps the instruction rule: instructions come
from the runbook's files (`instructions/`), never from the binary. `story:runbook-cli-skeleton`
built `doctor`, `list` and `--version`; this story adds the rest of the commands to the same crate.

## Acceptance

- `runbook run sdc --intake <file> --context <dir> --set key=value` runs in process, prints the
  run id and one line per case outcome, and exits 0 when the root case ends `delivered`, 3 when it
  stops for a person's decision (the exit codes of `scripts/run-trial.sh` today), 1 on failure.
  `--intake -` reads standard input.
- `runbook run ... --detach` with `--daemon <url>` (or `RUNBOOK_DAEMON`) submits the run to the
  daemon, prints its run id and exits 0 at once; without a daemon, `--detach` exits 64 and says a
  daemon is needed.
- `runbook runs list`, `runbook runs show <run>`, `runbook runs events <run>` (follows the SSE
  stream until the run ends, `--no-follow` prints what exists), `runbook runs cancel <run>` and
  `runbook decide <run> <request> <answer>` print what the matching API routes return, against a
  daemon started with `runbook serve`; each has `--json`.
- Against a stub daemon `runbook runs events <run>` prints the same lines as `curl -N` of the run's
  SSE route, and after a dropped connection resumes with `Last-Event-ID` and prints no event twice.
- An unknown subcommand or a missing `--intake` exits 64 with the usage.
- `runbook run` on a fixture case ends with the same outcome as `story:engine-on-loom-library-entry`'s
  engine test and starts none of `canon`, `b10x-loom`, `loom`, `sdc-drive`.
- No test over 60 s, the step within 120 s; new tests go into `runbook/tests/cli.rs`.

## Scope

`runbook/src/main.rs`, `runbook/src/run.rs`, `runbook/src/client.rs` (new), `runbook/tests/cli.rs`,
`docs/` (one page for the commands), `CHANGELOG.md` (one line). Overlaps D, F, G, H on
`runbook/src/main.rs`: E starts after D.

## Client part

Built on `impl/daemon-cli-client` (base `7965b98b`): the commands against a daemon, `docs/cli.md`,
`runbook/src/client/`, `runbook/tests/daemon/cli_client.rs` (20 tests, each under 1 s). The
in-process engine is not built here.

Acceptance lines, in the order above:

- Line 1 (`runbook run ... ` in process, exit 0 / 3 / 1, `--intake -`): waits for
  `story:engine-on-loom-library-entry`. Without `--detach` the command exits 64 with the line
  "runbook run: running in process waits for story:engine-on-loom-library-entry; submit to a daemon
  with --detach and --daemon <url>", pinned by `run_without_detach_names_the_story_it_waits_for`.
  `--intake -` is built and tested for the detached form
  (`detach_reads_the_daemon_from_the_environment_and_the_intake_from_standard_input`).
- Line 2, met: `detach_submits_the_run_and_prints_its_id` (the run id, exit 0, the daemon holds
  the run with its intake, settings and absolute context path),
  `detach_reads_the_daemon_from_the_environment_and_the_intake_from_standard_input`
  (`RUNBOOK_DAEMON`), `detach_without_a_daemon_exits_64_and_says_a_daemon_is_needed`.
- Line 3, met against the stub daemon served in process (a daemon started with `runbook serve` is
  the same code): `runs_list_and_show_print_the_routes_as_text_and_as_json`,
  `runs_events_prints_the_lines_curl_gives`, `runs_events_follows_a_live_run_until_it_ends`,
  `cancel_ends_a_live_run_and_maps_the_refusals_to_exit_codes`,
  `decide_answers_a_request_and_a_second_answer_is_a_conflict`; each has `--json`, which prints the
  route's body. `runs decisions` is an addition: it lists the open request ids `decide` needs.
- Line 4, met: `runs_events_prints_the_lines_curl_gives` (byte for byte with a raw `GET` asking
  `text/event-stream`) and
  `runs_events_resumes_with_last_event_id_after_a_dropped_connection_and_prints_no_event_twice` (a
  proxy cuts the first stream after its first event; the second request carries `Last-Event-ID: 1`;
  four events, once each).
- Line 5, usage part met: `an_unknown_subcommand_and_a_missing_intake_exit_64_with_the_usage`. The
  exit codes of `docs/cli.md` are held equal to the client's table by
  `the_exit_code_table_of_the_page_is_the_one_the_client_has`.
- Line 6 (the fixture case, no `canon`, `b10x-loom`, `loom`, `sdc-drive`): waits for the engine.
- Line 7 (budget): the 20 tests are in the crate's one test binary; none ran over 1 s.

Decisions: the client opens no session (`SubmitRun` takes none); `https://` is refused (no TLS client
in the crate; open); the token comes from a mode-600 file only and is removed from every answer the
client prints (`a_401_exits_77_and_no_token_is_printed`,
`a_server_that_echoes_the_token_in_its_401_gets_no_echo_from_the_cli`); no dependency was added; the
input types of the commands are the server's (`Serialize` added to four structs of
`runbook/src/commands.rs`).
