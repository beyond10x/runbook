# The CLI as a client of the daemon

`runbook run --detach`, `runbook runs ...` and `runbook decide` call the routes of a daemon
(`runbook serve`, [the daemon API](daemon-api.md)). Each command is one call of one route, made with
the input types of the server's own routes (`runbook/src/commands.rs`, paths from
`runbook/src/api/routes.rs`), and prints what the route returns: text lines by default, the route's
JSON body with `--json`. The code is `runbook/src/client/`.

## Where the daemon is

| Option | Environment | Meaning |
|---|---|---|
| `--daemon <url>` | `RUNBOOK_DAEMON` | `http://host:port`. Plain HTTP only; no path, no user info. No default: a command that needs a daemon and has none exits 64. |
| `--token-file <path>` | `RUNBOOK_TOKEN_FILE` | A file with the bearer token, one line. Mode 600: a file readable by group or others is refused. |

The token is read from that file and from nothing else: not from an argument, not from a plain
environment value (the daemon's own `RUNBOOK_TOKEN` is not read by the client). It is never printed,
never in an error line and never in `--json` output; the client replaces it by `[token]` in every
answer it prints, even from a server that echoes it. It is sent to the daemon named by `--daemon`
and to no other host. A token to a daemon that is not on loopback travels in clear over plain HTTP:
the client says so on standard error; use a tunnel (`ssh -L`).

`https://` is refused with exit 64: the crate has no TLS client. Reaching a remote daemon over TLS is
open (below).

Sessions. `SubmitRun` takes a session only if the caller names one. `runbook run --detach` names none
and opens none, so its run belongs to no session; `--json` shows the daemon's answer.

## Commands

| Command | Route | Prints (text) |
|---|---|---|
| `runbook run <runbook> --intake <file\|-> [--context <dir>] [--set k=v]... --detach` | `POST /daemon/commands/submit-run` | the run id |
| `runbook run <runbook> --intake <file\|-> [--run <name>] [--runs-dir <dir>] [--confinement <level>] [--settings <file.toml>] [--spend-ledger <file>] [--cap-micro-usd <n>] [--max-llm-calls <n>] [--max-wall-seconds <n>]` | none: the run is made in this process | one line on standard error; the exit code is the outcome's (`docs/loom-run.md`). Each option is the default of a `runbook.toml` key; the confinement, the wall time and the two caps are refused (exit 64) when neither states them |
| `runbook config [--config <file>] [--no-config] [the value options of run]` | none | every effective value of a run with its origin (`docs/loom-run.md` "Where a value comes from"); opens no login file; exit 2 for a refused file |
| `runbook init` | none | writes a starter `runbook.toml` (mode 600) into the working directory; exit 2 when one exists |
| `runbook run <runbook> … [--dry] [--recorded <dir> [--on-divergence stop\|nearest\|live] [--live-adapter <a>] [--live-cap <n>] [--inject <fault>]... [--set k=v]... [--take-recorded <actions>] [--source-tree <commit>]] [--bugfix <dir>] [--bugfix-ticket <file>] [--conformance off\|observe]` | none: the options of the old driver's launcher | `--dry` runs the stub cycle on the adapter `dry`, prints every model command and runs none (no login, spend 0; `docs/loom-run.md` "A dry run"); `--recorded <dir>` re-drives the case tree with every model call answered from a recording or a run directory by `sdc-replay` (`docs/loom-run.md` "A recorded run"): `--on-divergence` (default `stop`), `--live-adapter`, `--live-cap`, `--inject`, `--set` and `--take-recorded` need it (exit 64 without), a source that is no directory is exit 64, and a divergence ends the run with exit 11; the options of the other ports not built yet are accepted and refused with exit 64 and `<option> is not built yet`; `--conformance off` is the default and changes nothing; `--conformance observe` is built: after a run that ended closed it writes `oracle.json` into the run directory (`docs/conformance.md`) |
| `runbook run <sdc-bugfix\|sdc> … (--bugfix <dir> \| --bugfix-ticket <file>)` | none: a bug-fix case in this process | opens root `BUG-1` on `sdc-bugfix` after `sdc-bugfix seed` ran in `work/`; a missing directory or file, both options, another runbook or root protocol, or no `sdc-bugfix` helper is refused before anything is written (`docs/loom-run.md`, `docs/bugfix.md`) |
| `runbook run <runbook> … --goals <file>` | none: the option of the old driver's launcher | copies the goals document unchanged to `work/goals.yaml` of the run before the tree opens; a path that is no file is refused with exit 64; with `decider.requester` in the settings the decider answers the requester-side decisions from it |
| `runbook run <runbook> … --watch` | none: `sdc-watch` runs beside the run | records every file change of the run in `<run>/fs/changes.jsonl` (`docs/fs-changes.md`); see the `run` item below |
| `runbook runs list` | `GET /daemon/views/runs` | one line per run, newest first: id, state, runbook, submitted-at, `outcome=` or `failure=` |
| `runbook runs show <run>` | `GET /daemon/views/run`, `.../open-decisions` | the run's fields as `key: value`, then each open decision request |
| `runbook runs events <run> [--no-follow]` | `GET /daemon/views/events` as a stream (`--no-follow`: as a page) | the lines of the stream |
| `runbook runs cancel <run>` | `POST /daemon/commands/cancel-run` | `cancelled <run>` |
| `runbook runs decisions <run>` | `GET /daemon/views/open-decisions` | the open requests: `<request id>: <question>` and one line per option |
| `runbook decide <run> <request> <answer> [--by <name>]` | `POST /daemon/commands/answer-decision` | `answered <request>` |

Every command but the in-process `run` takes `--json`, `--daemon` and `--token-file`.

- `run`. The intake is a file, or `-` for standard input (at most 1,048,576 bytes). A relative
  `--context` is sent as the absolute path of the directory; the daemon needs it under one of its
  context roots. `--set` is repeatable and each key may be given once. Without `--detach` the run
  is made in this process (`docs/loom-run.md`); the options of the table are those of `--detach`,
  and in process `--context`, `--json`, `--daemon` and `--token-file` are refused with exit 64.
  In process `--set` is a condition of the replay and needs `--recorded` (exit 64 without it); the
  replay is built (`docs/loom-run.md` "A recorded run"), the other port options listed in the
  second row are not built yet. `--detach` without a daemon exits 64 and says a daemon is needed. `--json` prints the
  body of the submit's answer.
- `run --watch` (in process only). The run starts the helper `sdc-watch run <run directory>` as its child once the
  run directory exists, found as the other helpers are (`runbook doctor`: `libexec`, beside the executable, the
  checkout's `watch/target`, `PATH`), and ends it by its process id when the run ends, whatever the end (closed,
  stopped, cancelled, a signal, an error). The id is in `<run>/fs/watcher.json` (`runbook.watcher/1`), the
  watcher's own output is `fs/watch.log`. The watcher ends itself when the runner is killed (`sdc-watch run
  --parent <pid>`: a parent-death signal and a poll every 200 ms). Without the helper the run is refused before
  anything is written (exit 2); a helper that does not start ends the run as `error`. Under this runner a change
  is attributed to the `tree run` step in flight, not to an action (`docs/fs-changes.md`, "What is attributed under each runner"). Without
  `--watch` no watcher starts and no `fs/` is made.
- `runs events`. The text is the lines `curl -N -H 'Accept: text/event-stream'` of the run's route
  gives: `id:` (the event's position), `event:` (its kind), `data:` (its row as one line) and a blank
  line per event; the `: keep-alive` comment lines are left out. It follows the stream until the
  daemon closes it with the run over. After a dropped connection (a reset, a cut mid-chunk, 45 s of
  silence) it reconnects with `Last-Event-ID` set to the last position it printed, and an event whose
  position it printed is not printed again. A stream the daemon closes while the run is still live
  (a client that fell behind, a restart) is resumed the same way. Five attempts in a row without an
  event end it with exit 69. `--no-follow` prints what the run has and returns, in the same lines.
  `--json` prints one row of the events route per line.
- `decide` records `--by` (default `$USER`, else `cli`) as the answer's label. The daemon authenticates
  no principal, and the label never acts as one: the principal of the answer is the daemon's own (`runbook serve --principal`), shown beside the label.

## Exit codes

0 is success. Every other code is one of these; a line `runbook <command>: <status> <code>: <message>`
goes to standard error, with the error's code and message as the daemon's body gives them
(`sdc.daemon.NoSuchRun` for a refusal of the specification, `unauthorized` for an answer outside it).

| Code | Name | Cause |
|---|---|---|
| 1 | failure | any other failure: a 5xx answer, an answer that cannot be read, a failed write |
| 64 | usage | unknown subcommand, a missing or malformed argument, no daemon named where one is needed, an https or user-info URL |
| 65 | refused | the daemon refused the request as invalid: HTTP 400, 413 or 422 (a refusal of the specification such as NoSuchRunbook) |
| 66 | not found | HTTP 404, an unknown run, or a local file or directory the command names that does not exist |
| 67 | conflict | HTTP 409: the state of the run or request does not take the command |
| 69 | unreachable | no connection to the daemon: connection refused, timed out, or lost more often than a stream retries |
| 77 | denied | HTTP 401 or 403: the token is missing or wrong |
| 78 | config | the token file is missing, empty or readable by group or others |

The views answer `200` with no row for an unknown run; `runs show` and `runs events` turn that into
exit 66 (`no such run`).

## `runbook test`

```
runbook test <suite file> [--scratch <dir>] [--only <name>[,<name>...]] [--workers <N>] [--cycle-timeout <seconds>] --confinement <local|substrate>
runbook test <suite file> --list [--only <name>[,<name>...]]
```

Runs the cycles of a `loom.tree-suite/1` file (one after another, or `--workers` beside each other) through Loom's suite runner
(`b10x_loom_tree::suite::run_suite`), in this process (`runbook/src/test_suite.rs`). It is not a
client of a daemon. No `b10x-loom` or `canon` process is started; the process tree of a
run holds `runbook` and the bound programs, and a bound program gets `PATH` and `LOOM_*` from Loom
and nothing of this process's environment. The protocols are the files of the asset root's
`protocol/` directory (`RUNBOOK_HOME`, else the checkout). The suite's seeds, bindings and suite
sources are paths below the suite file's own directory (Loom's rule).

- `--confinement` is required (except with `--list`), has no default and is passed on unchanged. Loom's
  `tree test` runs the `substrate` level under a delegated cgroup root (`--cgroup-root`, or a scope it
  starts itself again in); `runbook test` has no `--cgroup-root` option and holds no root (it passes
  none, `cgroup_root: None`), so `substrate` is refused with exit 2 (`ConfinementRefused:
  CgroupUndelegated`); `local` runs the programs unconfined, declared insecure. No level is lowered
  and no variable or mount is added for a bound program.
- Output, one line per cycle when it ends, in Loom's words: `<name>: as expected (<s> s)`,
  `<name>: FAILED: <check> (<kind>, <reason>): expected <value>, found <number, name, or length and
  digest> (<s> s)`, `<name>: pending, still failing: ...`, `<name>: pending, passing (<s> s)`, `<name>:
  pending, waits for story:<id> (<kind>)`. Then Loom's `slowest: <name> (<s> s), ...` line (up to ten
  cycles that ran, slowest first; `slowest: none` when none ran) and `<N> of <M> as expected, <P>
  pending`, then `failed: <names>` when a cycle failed. A cycle that ran past `--cycle-timeout` is
  `<name>: FAILED: steps[<i>] (run, CycleTimedOut): expected an end within <T> s, found <s> (<s> s)`. A program's
  output never reaches these lines (Loom prints a found text as its length and digest).
- Exit 0: every cycle that ran and is not pending is as expected. Exit 1: one is not, or SIGINT or
  SIGTERM ended the run (they set Loom's cancel flag: the cycle that runs ends, its running program
  is ended and no later step starts, and no further cycle runs; the line `cancelled` says so). Exit 2: refused before any cycle, with
  Loom's reason and the member it names on standard error (`SuiteUnreadable`, `StepKindUnknown:
  cycles[0].steps[1].do`, `ScratchNotEmpty`, ...), or `--only` names no cycle of the file, or the
  confinement level is refused. Exit 64: a missing or malformed argument.
- `--scratch <dir>` is absent or empty (Loom refuses otherwise) and is never removed. Without it the
  directory is new under `$XDG_RUNTIME_DIR/runbook/`, else under `$XDG_STATE_HOME/runbook/` (or
  `~/.local/state/runbook/`), never under `/tmp`. It is removed when the exit code is 0; otherwise
  it is kept and the last line, `scratch kept: <dir>`, names it. Cycle `n` works in `<dir>/work/<n>/`
  and keeps its tree in `<dir>/store/<n>/` (Loom `c9b4c177`; before, `<dir>/<n>/work` and
  `<dir>/<n>/store`).
- `--only` runs the named cycles in suite order, nothing else: it is Loom's own selection
  (`SuiteSettings.only`), so no file is written beside the suite. A name that is no cycle of the
  file is refused before any cycle (`CycleUnknown`, exit 2). A cycle keeps the directory of its
  position in the suite (`<dir>/work/<n>/`, `n` the position in the whole file), so a selected run and a
  whole run name a cycle's directory alike. With `--only` the closing lines start with `selected S
  of C cycles`.
- `--workers <N>` runs up to N cycles beside each other (Loom's `SuiteSettings.workers`), 1 to 64,
  default 1: one after another. Each cycle has its own directory and its own time bound, and the
  lines are printed in suite order. Under `local` nothing is confined, so a program can read the
  other cycles' workspaces; the cycles of a suite must not depend on that. More than 16 under
  `--confinement substrate` is refused with Loom's `SubstrateWorkersUnserved` (exit 2, before
  anything is written; a Substrate driver runs at most 16 executions at once); since `runbook test`
  holds no cgroup root, 1 to 16 under `substrate` reach the level's own refusal. A value outside 1 to
  64 is a usage error (exit 64).
- `--cycle-timeout <seconds>` (Loom's `SuiteSettings.cycle_timeout`; 1 to 60, default 60) bounds each
  cycle from its start: past it the cycle's running program is killed and the cycle ends
  `CycleTimedOut`, naming its step; the other cycles go on. A whole number outside 1 to 60 is refused by
  `runbook` itself, in its own words, with exit 2 before the suite is read (`--cycle-timeout 61 is
  outside 1 to 60 seconds: a cycle may not outlive the 60 s bound of one test`); a word or an empty
  value is not a number and is refused by the option parser with exit 64. Raising the bound is not possible: 60 s is
  the most.
- `--list` prints one line per cycle (`<name>`, `<name>: pending`, `<name>: pending, waits for
  story:<id> (<kind>)`) in suite order and runs nothing (Loom's `list_suite`): it reads and decodes
  the suite alone (a suite Loom refuses is exit 2), needs no `--confinement` and writes nothing;
  `--only` applies. Exit 0.

Tests: `runbook/tests/test_suite/` over `runbook/tests/fixtures/test-suite/` (POSIX sh programs):
`mod.rs` (the options), `workers.rs` (`--workers` and its refusal), `options.rs` (`--cycle-timeout` and
`--list`), `amend.rs` (the state of run 2) and `notcalled.rs` (the state of run 1: a writer that is not
called while a finding waits is started once and the run waits for the person).

## Open

- HTTPS to a remote daemon: no TLS client in the crate.
