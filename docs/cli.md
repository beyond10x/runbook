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
| `runbook run <runbook> --intake <file\|-> --run <name> --runs-dir <dir> --confinement <level> --settings <file> --spend-ledger <file> --cap-micro-usd <n> --max-llm-calls <n> --max-wall-seconds <n>` | none: the run is made in this process | one line on standard error; the exit code is the outcome's (`docs/loom-run.md`) |
| `runbook run <runbook> … [--dry] [--recorded <dir> [--on-divergence stop\|nearest\|live] [--live-adapter <a>] [--live-cap <n>] [--inject <fault>]... [--set k=v]... [--take-recorded <actions>] [--source-tree <commit>]] [--bugfix <dir>] [--bugfix-ticket <file>] [--goals <file>] [--watch] [--conformance off\|observe]` | none: the options of the old driver's launcher | `--dry` runs the stub cycle on the adapter `dry`, prints every model command and runs none (no login, spend 0; `docs/loom-run.md` "A dry run"); every other one is accepted and refused with exit 64 and `<option> is not built yet` until its port is built; `--conformance off` is the default and changes nothing; `--conformance observe` is built: after a run that ended closed it writes `oracle.json` into the run directory (`docs/conformance.md`) |
| `runbook run <sdc-bugfix\|sdc> … (--bugfix <dir> \| --bugfix-ticket <file>)` | none: a bug-fix case in this process | opens root `BUG-1` on `sdc-bugfix` after `sdc-bugfix seed` ran in `work/`; a missing directory or file, both options, another runbook or root protocol, or no `sdc-bugfix` helper is refused before anything is written (`docs/loom-run.md`, `docs/bugfix.md`) |
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
  replay and the other port options are listed in the second row and are not built yet. `--detach` without a daemon exits 64 and says a daemon is needed. `--json` prints the
  body of the submit's answer.
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
runbook test <suite file> [--scratch <dir>] [--only <name>[,<name>...]] --confinement <local|substrate>
```

Runs the cycles of a `loom.tree-suite/1` file one after another through Loom's suite runner
(`b10x_loom_tree::suite::run_suite`), in this process (`runbook/src/test_suite.rs`). It is not a
client of a daemon. No `b10x-loom`, `canon` or `sdc-drive` process is started; the process tree of a
run holds `runbook` and the bound programs, and a bound program gets `PATH` and `LOOM_*` from Loom
and nothing of this process's environment. The protocols are the files of the asset root's
`protocol/` directory (`RUNBOOK_HOME`, else the checkout). The suite's seeds, bindings and suite
sources are paths below the suite file's own directory (Loom's rule).

- `--confinement` is required, has no default and is passed on unchanged. Loom's suite runner takes
  no cgroup root, so `substrate` is refused with exit 2 (`ConfinementRefused: CgroupUndelegated`);
  `local` runs the programs unconfined, declared insecure.
- Output, one line per cycle when it ends, in Loom's words: `<name>: as expected (<s> s)`,
  `<name>: FAILED: <check> (<kind>, <reason>): expected <value>, found <number, name, or length and
  digest> (<s> s)`, `<name>: pending, still failing: ...`, `<name>: pending, passing (<s> s)`, `<name>:
  pending, waits for story:<id> (<kind>)`. Then `ten slowest:` (seconds and name) and `<N> of <M> as
  expected, <P> pending`, then `failed: <names>` when a cycle failed. A program's output never
  reaches these lines (Loom prints a found text as its length and digest).
- Exit 0: every cycle that ran and is not pending is as expected. Exit 1: one is not, or SIGINT or
  SIGTERM ended the run (they set Loom's cancel flag: the cycle that runs ends, its running program
  is ended and no later step starts, and no further cycle runs; the line `cancelled` says so). Exit 2: refused before any cycle, with
  Loom's reason and the member it names on standard error (`SuiteUnreadable`, `StepKindUnknown:
  cycles[0].steps[1].do`, `ScratchNotEmpty`, ...), or `--only` names no cycle of the file, or the
  confinement level is refused. Exit 64: a missing or malformed argument.
- `--scratch <dir>` is absent or empty (Loom refuses otherwise) and is never removed. Without it the
  directory is new under `$XDG_RUNTIME_DIR/runbook/`, else under `$XDG_STATE_HOME/runbook/` (or
  `~/.local/state/runbook/`), never under `/tmp`. It is removed when the exit code is 0; otherwise
  it is kept and the last line, `scratch kept: <dir>`, names it. Cycle `n` works in `<dir>/<n>/`
  (`store/`, `work/`).
- `--only` runs the named cycles in suite order, nothing else. Loom's runner takes a file, not a
  list, so the command writes the selected cycles to `.runbook-test-only-<pid>.suite.yaml` beside
  the suite (its seeds are paths relative to that directory) and removes it when the run ends.

Tests: `runbook/tests/test_suite/mod.rs` over `runbook/tests/fixtures/test-suite/` (POSIX sh
programs).

## Open

- HTTPS to a remote daemon: no TLS client in the crate.
