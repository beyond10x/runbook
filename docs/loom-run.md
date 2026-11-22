# `runbook run`: one case tree through Loom, in this process

`story:runbook-loom-run-command` (built as the subcommand `loom-run`, now `runbook run`), `story:driver-runtime-removed`, `story:engine-on-loom-library-entry` (slice 2). The command runs
one case tree to its end, to a cap or to a stop, through Loom: the `b10x-loom` binary as a child
process, or Loom as a library in this process. It has a spend ledger that caps the money over all
runs and a run directory that records what happened. It makes model calls only when the bound
programs or the decider do; the command itself calls no model and no API.

```
runbook run <runbook> --intake <file|-> --run <name> --runs-dir <dir> --confinement <local|substrate>
  --settings <file.json> --spend-ledger <file> --cap-micro-usd <n> --max-llm-calls <n> --max-wall-seconds <n>
  [--loom <b10x-loom> --loom-sha256 <hex>] [--bindings <bindings/>] [--protocols <dir>]
  [--root-protocol <name>@<revision>] [--case <id>]
```

`<runbook>` is the id of a protocol file under the protocols directory (`sdc`, `sdc-bugfix`; the
code of `runbook list`). Required, with no default: `<runbook>`, `--intake`, `--run`, `--runs-dir`,
`--confinement`, `--settings`, `--spend-ledger`, `--cap-micro-usd`, `--max-llm-calls` and
`--max-wall-seconds`; a missing one is a usage error (exit 64) that names it. Optional:
`--bindings` and `--protocols` default to `bindings/` and `protocol/` of the asset root
(`RUNBOOK_HOME`, the installed share directory or the checkout the executable lies in; the lookup
of `runbook doctor`), `--root-protocol` to `<protocol id with . as ->@<revision>` of the runbook's
file (`sdc@8`), `--case` to `IDEA-1`, and `--loom` with `--loom-sha256` to none (Loom is a library of
the process). `--intake` is a text file, or `-` for standard input, holding the idea: a text that is
a mapping with a `text` member is the idea file as it is, any other text becomes `text: <the text>`
(`runbook::engine::layout::idea_document`, as the daemon writes it), and the runner puts it at
`work/idea.yaml`; an empty intake is refused. With `--detach` the command is the daemon client
(`runbook run --detach --daemon <url>`), takes none of the options above and none of these takes
`--detach`. The settings file has one format, `runbook.loom-run-settings/1`
(`docs/loom-run-settings.md`). `--confinement` is passed unchanged to every `tree run` (`local` is
unconfined and declared insecure; `substrate` is Substrate's linux adapter). Exit codes: 0 ended,
1 error, 2 refused before anything was written, 3 stuck, 4 to 11 stopped (spend cap, wall time, model
calls, signal, decider, denied, rejected, recording diverged), 64 usage. The subcommand was called `loom-run` before; this is the
same code, moved (`runbook/src/loom_run/`), and the run directory is the one it wrote.

A bug-fix case: `--bugfix <seed dir>` or `--bugfix-ticket <ticket file>` (one of them; both is exit 64,
and so is a directory or file that is not there). The root protocol is `sdc-bugfix` and the root case
`BUG-1` (a `--case` that is not `IDEA-1` is kept); the runbook argument may be `sdc` or `sdc-bugfix`,
another is exit 64, and a `--root-protocol` that is not `sdc-bugfix@<revision>` is refused with exit
2. Before the tree opens, the runner starts the helper `sdc-bugfix` (found as `runbook doctor` finds
the helpers: beside the executable, `driver/target/` of a checkout, then `PATH`; not found is exit 2
before anything is written) with the environment of the bound programs, in `work/`, and the seed
writes `bug.yaml`, `mainline/`, `impl/`, `checks.yaml` and the story there (`runbook/src/loom_run/bugfix.rs`,
`docs/bugfix.md`). With a seed directory it runs `sdc-bugfix seed --from <dir> --text <the intake's
text> --story story:bug-<run>`; with a ticket, `sdc-bugfix seed --ticket <file>`, and the intake is
not read for it (it is still required, and the runner still writes `work/idea.yaml` from it). The
runner prints nothing of a seed that worked. A seed that failed ends the run `error`, exit 1, with
the last three lines the seed printed (`sdc-bugfix` quotes the ticket's id and no other value of it).
The text of a ticket is in `work/` and in no other file the runner writes and on neither stream
(`runbook/tests/loom_run/bugfix.rs`). The bug-fix runbook works on Rust repositories only
(`docs/bugfix.md`).

## The two ways to Loom

One of two (`--loom` without `--loom-sha256`, or the reverse, is refused with exit 2, nothing written):

| Way | Options | What runs |
|---|---|---|
| child | `--loom <bin> --loom-sha256 <hex>` | the binary, one `b10x-loom` call at a time, as below |
| library | neither `--loom` nor `--loom-sha256` | the Loom this binary links (a git dependency pinned to one commit), called in this process; no `b10x-loom`, `loom`, `canon` or `sdc-drive` process is started, only the bound programs |

The loop, the decider, the budget, the spend ledger, the settings, the copy of the bindings, the
private directory and the run directory files exist once (`src/loom_run/run_loop.rs`); they reach
Loom through one trait (`src/loom_run/host.rs`) with a child implementation (`child_host.rs`) and a
library implementation (`library.rs`). The library calls are the ones the daemon's engine makes
(`src/engine/calls.rs`, `docs/engine.md`): there is one copy of the code that calls Loom.

The library refuses, before anything is written (exit 2), a `--confinement` level it cannot give on
this host: `local` it always can; `substrate` (Substrate's linux adapter) only when the process may
delegate a cgroup, as the engine's own levels (`runbook::engine::calls::host_levels`).

Under the library a decision is answered from its structured members (`case`, `kind`, `action`,
`capability`, `outcome`) by library calls (`approve`, `close`), never by reading the printed
`command`. The principal of an approval is the one the decider names, else the one the run's copy
of the bindings binds the action to (`executor.principal`; `operator` when it names none), which is
the one Loom prints in its command. A `Handoff` runs nothing of its own: the next step is the
`tree run` it would have printed.

### What is recorded

`settings.json` and `run-settings.jsonl` hold the Loom used, in `loom`: `sha256` of the binary
(child), or `in_process: true` and `commit`, the 40 hex digits of the pinned Loom commit (library;
a constant of the build, read from `Cargo.lock` by `build.rs`). A settings file may carry
`loom.in_process: true` only when Loom is a library (no `--loom`) and `loom.sha256` only with `--loom-sha256`; a
member that disagrees with the option, or `loom.sha256` under the library, is refused.

The run directory holds the same files in the same shapes under both ways. Two members only a
child has are `null` under the library, in `outcome.json` and in the `ended` event:

| Member | Child | Library |
|---|---|---|
| `loom_exit_code` | the exit code of the failed `b10x-loom` call | `null` |
| `stderr_tail` | the last 1500 characters of its standard error | `null` |

A refusal of the library is an `error` outcome with the reason Loom gave; there is no exit code or
standard error to record. `tree-status.json` and `tree-verify.json` are the documents `tree status`
and `tree verify` print (`status_json`, `verify_json`); `tree-verify.json` holds `exit` 0 when the
log verifies and 3 when it does not, as the command's.

### The environment of a bound program

Under the library a bound program gets `PATH` and `LOOM_*` variables and nothing else: Loom builds
the environment of a program explicitly and inherits none from the runbook process (the library's
runner takes no environment from its caller). `tests/loom_run/inproc.rs` puts a marker variable and
a fake token variable into the environment of `runbook` and has a bound program dump its
environment: the dump holds neither. The child's own environment is in "The environment of the
child".

## What it does

1. Refuses (exit 2, nothing written) when `--loom` and `--loom-sha256` do not go together, `--loom`
   is not the file `--loom-sha256` names, Loom is a library and the level is one it cannot
   give, `--run` is not a plain name, the settings are not a JSON object, the decider settings are not usable, a
   path is missing, or `<runs-dir>/<name>` exists.
2. Creates the run directory, writes `settings.json` (the login file's path replaced by `named`),
   copies `--bindings` into `bindings/` (the copy is what Loom reads; the checkout is never
   edited), writes the idea file `work/idea.yaml` from `--intake`, copies every `*.protocol.yaml` of `--protocols` into
   `protocols/`, registers them (`b10x-loom protocols add`) and opens the tree (`tree open --store
   store/ --workspace work/`).
3. Loops `b10x-loom tree run --max-steps 1 --confinement <level>`. After each step it reads what the
   step cost, refreshes `tree-status.json` (one `tree status` call, written through a temporary file
   and a rename), then reacts to the exit code of `tree run`:

   | Exit | Meaning | The runner |
   |---|---|---|
   | 0 | terminal | ends: outcome `ended` |
   | 3 | waits for a decision | asks the decider for each entry of `decisions[]`, runs the printed command, loops |
   | 4 | stuck | ends: outcome `stuck` (runner exit 3) |
   | 5 | the step budget of one step is spent | loops (that is every normal step) |
   | anything else | not known | ends: outcome `error` with the exit code and the tail of standard error |

4. At the end it stores `tree status` and `tree verify` (`tree-status.json`, `tree-verify.json`). A
   non-zero `tree verify` makes the outcome `error`; a `tree status` the binary refuses (at any step
   or at the end) ends the run as `error` with the exit code and the tail of standard error.

## The decisions

A decision is a JSON object of which only `case`, `kind`, `decides` and `command` are stable. The
runner passes the whole object to the decider whatever its `kind` (an unknown kind is the decider's
to answer or refuse, and a refusal names the kind), never reads `decides`, and runs `command` as
data: `command[0]` must be `b10x-loom`, the rest is the argv. It replaces the value after
`--principal` when the decider named a principal, and it adds to an approval of a person action
(printed with `--action`) the result the action declares: `--result performed` for an approval and,
for a rejection, `--result rejected --reasons-file <file>`. The file lies in `work/.sdc/reasons/`
of the run directory and holds the decider's text (a model decider's `reasons`), with the login
file's path, the run directory and the home directory taken out, at most 60 lines and 65536 bytes
(Loom's bounds); a rejection without a text gets one line that says so. A result the bindings copy
does not declare for the action (the decider rejects an action whose `results` lists no `rejected`)
ends the run as `error` naming the action and the declared results, before Loom is called. A
capability-only approval (`kind` `Approve`, no `--action`) has no result to name: its rejection is
Loom's `--deny`. A `Close` is run as printed when the decider approves it (Loom prints one per
legitimate outcome; the other outcomes of that case are not asked once one ran) and not run when it
declines. A `Handoff`'s printed command is `tree run`: it runs as printed through the ledger and
its output is read like any `tree run`. A printed command that exits non-zero ends the run as
`error`, with its exit code and the last 1500 characters of its standard error in `outcome.json`;
the runner does not repair or repeat it.

A decision whose action `tools/answers.yaml` of the bindings declares is first answered question by
question or finding by finding (`docs/loom-run-decisions.md`, "The answers file"): the runner asks the
decider once per request, applies each answer, runs the file's `then`, and then runs the printed
command as above with the decider's principal. Each call counts against `decider.max_calls`, the
call cap and the spend ledger, and each answer is a line of `decisions.jsonl`. Without a
declaration, or with no request open, nothing changes.

A denied decision that comes back (same case, action, revision and text) ends the run as `denied`
(exit 9) without asking the decider again; a rejection that sent a writer back returns as a
decision at a new revision and is asked again. An approved decision that comes back is an `error`
(a handoff whose work nobody did is one).

The decider is chosen from the settings key `decider` (`docs/loom-run-settings.md`; what each kind
answers is in `docs/loom-run-decisions.md`). With no `decider` key every decision ends the run as
`stopped: decider`. `{"kind": "scripted", "answers": [...]}` is the test double the tests use. For
the kinds `model` and `rule` the decider runs inside the runner: a rule answers by itself, a model
decision starts only the adapter program of `decider.adapter` from the run's own copy (the adapters
are bash with `jq`, so `jq` has to be on the runner's `PATH`; a missing adapter or `jq` ends the run
as `stopped: decider`), and `sdc-reply` found on that `PATH` is handed to it as `SDC_REPLY`. The bindings copy is made by `bindings_copy::materialise` with
the settings as read, before `tree open`, and Loom is given the root file it returns.

## The environment of the child

The child gets a built environment (`env_clear`, then these three), never the runner's:

| Variable | Value |
|---|---|
| `PATH` | the runner's `PATH` |
| `HOME` | the directory `home/` of the run directory |
| `XDG_DATA_HOME` | `data/` inside that directory (Loom's protocol catalog) |

`tree status`, `tree verify` and `tree run` all read the protocol catalog, which lives under
`HOME` and `XDG_DATA_HOME`. A store is only readable with the catalog that holds its protocols:
measured with the real binary on one store, `env -i` with `PATH` alone answers `UnknownProtocol:
the host's protocol catalog cannot be read: HOME is unavailable`, the same store with the
environment of another catalog answers `StoreUnreadable: ... GovernorUnavailable`, and `HOME` plus
`XDG_DATA_HOME` of the run's catalog answers the status. The working directory does not matter. So
every call, `tree status` too, gets the three variables of the table; a copy of a store has to be
read with the catalog of its run.

Bound programs get only `PATH` from Loom itself. No login file, no path of one and no credential
variable is in the run directory or in the child's environment; the login file's path is a setting
that is replaced by `named` in every file the runner writes. The programs reach the login through
the link of the private directory (next section).

## The login file

When the settings name a login file (`login_file` or `loom.login_file`), the run directory holds no
path of it and no link to it. The bindings copy and the model decider read a link in a private
directory, `$XDG_RUNTIME_DIR/runbook/<run name>/` (mode 0700, the link is `login`). A link and not a
copy: no second copy of the credential is written to disk, and a token the model CLI refreshes lands
in the real file. The run directory records only that a file was named (`settings.json` holds
`login_file: named`).

- The run is refused (exit 2, nothing written) when a login file is named and `XDG_RUNTIME_DIR` is
  unset, not absolute, or not a directory owned by the user running the command, when the login file
  is not an absolute path of a file that exists, and when the private directory of that run name
  exists already. No message holds the login file's path.
- The directory is removed when the run ends: after a normal end, after an error, and after SIGTERM or
  SIGINT (the runner ends the running call and returns, and the guard removes the link and the
  directory). The test proves each (`runbook/tests/loom_run/login.rs`), and that no file of the run
  directory holds the path or a link to the file.
- After a SIGKILL (or a crash of the host) the private directory stays: one link in a directory of
  mode 0700 that holds the path of the login file. The next run of the same name is refused until the
  directory is removed by hand (`rm -r $XDG_RUNTIME_DIR/runbook/<run name>`); `$XDG_RUNTIME_DIR` itself
  is removed by the system at the user's last logout. The run directory holds nothing of it either way.

### How each adapter gets the login under Loom

A bound program has `PATH` and `LOOM_*` and no `HOME`. The bindings copy writes the link as
`--claude-credentials <link>` into the argv of every model node, `sdc-node` turns it into
`SDC_CLAUDE_CREDENTIALS`, and each adapter makes its own private copy for its one call. The copy lies
in the private directory of the run, the directory of the link (the adapter takes it when the
directory of `SDC_CLAUDE_CREDENTIALS` belongs to the user and has mode 700), and never in the call's
record directory, which is inside the run directory:

| Path | How it finds the login | Where the copy lies | Who removes it |
|---|---|---|---|
| `llm-adapter-claude-cli.sh` (`--adapter claude-cli`; the review step) | copies it into a config home (mode 700, file 600) and sets `CLAUDE_CONFIG_DIR` for `claude`; the whole config home moves, as it is the directory `claude` is given | `<private dir>/config-home.XXXXXX` | the adapter on exit; after a SIGKILL the runner |
| `llm-adapter-metaharness.sh` (`--adapter metaharness`) | metaharness reads `$HOME/.claude/.credentials.json` and no `CLAUDE_CONFIG_DIR`, and finds `claude` only on `$HOME/.local/bin:/usr/local/bin:/usr/bin:/bin`; the adapter makes a home (mode 700) with `.claude/.credentials.json` (a copy, mode 600) and `.local/bin/claude` (a link to the `claude` of its `PATH`), and sets `HOME` and `TMPDIR` to it for the metaharness process alone, so metaharness's scratch root (its own copy of the login in its config home) lies there too | `<private dir>/h.XXXXXX`, the scratch root `<private dir>/h.XXXXXX/.tmpXXXXXX` | metaharness removes its scratch root when it ends, the adapter the home on exit, also after a failed call or SIGTERM; after a SIGKILL the runner |
| `decider.sh` (the model decider) | `SDC_CLAUDE_CREDENTIALS` from the runner (the link), handed to `sdc-node`, so the adapter of the first or second row | as its adapter | as its adapter |

`PrivateDir` removes its directory with everything in it when the run ends (a normal end, an error,
SIGTERM or SIGINT), so a home that a SIGKILL of an adapter or of metaharness left is gone with it. It
removes the link itself and never follows it: the login file stays. A SIGKILL of the runner leaves
the directory, as the section above says.

The metaharness adapter makes its home only when metaharness would find no login otherwise:
`SDC_CLAUDE_CREDENTIALS` names a file in such a directory and `$HOME/.claude/.credentials.json` is
not a file. With a usable `HOME` nothing changes, also when `SDC_CLAUDE_CREDENTIALS` is set. Where
`SDC_CLAUDE_CREDENTIALS` is not in such a directory (the driver's runs) the metaharness adapter does
nothing new and the claude-cli adapter makes its config home in the record directory, as it always
did. With `ANTHROPIC_API_KEY` the claude-cli adapter copies no login. Without
`SDC_CLAUDE_CREDENTIALS` the claude-cli adapter reads `CLAUDE_CONFIG_DIR`, then `HOME`. With
`SDC_LLM_ENDPOINT` the metaharness adapter keeps `--credentials none` and copies no login. The path
and the content of the login are in no file of the run directory, also after a SIGKILL of an adapter
(tests in `driver/tests/questions_check.rs`, `loom_mode`, and `runbook/tests/loom_run/login.rs`; the
checks with the real `metaharness` and a stand-in `claude` are skipped where `metaharness` is not on
`PATH`).

## The caps

| Cap | Outcome | Exit | Checked |
|---|---|---|---|
| spend | `stopped: spend cap` | 4 | before each step and each decider call |
| wall seconds | `stopped: wall time` | 5 | before each call, and a running call is ended at the deadline |
| model calls | `stopped: llm calls` | 6 | before each step and each decider call |
| SIGTERM or SIGINT | `stopped: signal` | 7 | the running call is ended (SIGTERM, then SIGKILL); in the library a `handoff_when` check that runs is ended too (Loom `5b3142c7`: refusal `Cancelled: action <action>: the check <program> was cancelled`, `tree run` status 5, `next: [<action>]`), and the run ends `stopped: signal` |

An approval of a person action whose binding names an `argv` runs that program through the runner of the run's `--confinement` (Loom `70b0e6e0`, `tree approve`; in the library `ApproveRequest.runner`). The refusals of it end the run `error` (exit 1) with Loom's text, as every refusal of an answer does: `ConfinementRefused` when no runner is admitted for bound programs, `PersonProgramFailed` when the program exits other than 0, is killed at its timeout, or leaves a placed artifact unreadable; the approval and the run stay in the store (`tree approve` prints the command that takes the action again, with `--confinement`).

Other exits: 0 ended, 1 error, 2 refused, 3 stuck, 8 `stopped: decider`, 9 `denied`, 10 `stopped: rejected` (a rejection of an action with no `rejected` result; cause `rejected_without_rework_path`), 11 `stopped: recording diverged` (`--recorded`), 64 usage.

### The spend ledger

`--spend-ledger` is one JSON file (`runbook.spend-ledger/1`) under an exclusive file lock, shared by
every run that counts against the same cap. A call first reserves the most it may cost (the
largest `--max-budget-usd` of the bindings, or the decider's own maximum, whichever applies, cut to
what the cap has left), then books the real cost and releases the rest. Two runs at once therefore
never grant more than the cap: two processes that each want 0.60 USD against a cap of 2.00 USD
spend at most 2.00 USD between them.

The per-call budget is the `--max-budget-usd` argv value of the model tools in the copy, lowered
before each step to the reservation. No `max_budget_micro_usd` is written: in a tree only `max_runs`
acts, and Loom refuses the other budget members on a program binding.

Where the cost of a step is read: Loom's `tree status` carries none. A model tool writes
`<workspace>/.sdc/llm/NNN-<action>/` at the start of a call and a line with `cost_usd` and `record`
to `.sdc/llm/calls.jsonl` at its end (also `action`, `adapter` and the adapter's `exit`). A directory
without a matching line, a null cost or an unreadable line fails the run closed and books the whole
reservation.

### A failed model call

The old driver does not end a run for a model call that exits with a number other than 0 and has no
cost: it records the action as finished without evidence, the step's retry rule decides (`max_runs`,
then the `writer_spent` verdict), and only the exits 126, 127, 75, 69 and 124 of a program end the run
(`driver/src/main.rs`, `program_error`). `runbook run` does the same:

- A call with an exit other than 0, 126, 127 and 75 and no usable cost is booked at its reservation
  (the spend rule stays fail-closed), written to `llm-calls.jsonl` with `cost_unknown: true`, and the
  run goes on; repeated failures run into the spend cap and end as `stopped: spend cap`.
- A call with exit 126, 127 or 75, a call with exit 0 and no cost, and a call directory with no line
  end the run as `error`, with the reservation booked.
- A call directory with no line and no `request.json`, whose `status.json` names an exit, is no call:
  `sdc-node` writes `request.json` before every start of an adapter, so the step ended before
  any adapter started. It costs nothing, its reservation is released, the event `no_model_call` (note
  `step, no model call made`, the call's name and the exit) is written to `events.jsonl`, and the run
  goes on with what Loom says of the step. A directory with `request.json`, or without a `status.json`
  that names an exit, stays as the previous item says. The decider's calls do not pass the cost
  reader and are unchanged.
- The reason of such an end is `model step <action>: adapter <adapter> exited <code>: <first line of
  the adapter's stderr>`: the first non-empty line of `adapter-stderr.txt`, else `stderr.txt`, of the
  call directory, the home prefix replaced by `~`, cut at 300 characters, `(no stderr)` when empty.

### A dry run

`--dry` is the old launcher's `--dry`: the stub cycle with every model command printed and none run.
It sets three things before the run opens and keeps every required option required (`--confinement`,
`--cap-micro-usd`, `--max-llm-calls`, `--max-wall-seconds`, the paths).

- Every model node of the bindings copy runs the adapter `dry` (`bindings/tools/llm-adapter-dry.sh`):
  it builds the command the real adapter would run and calls nothing. A settings file that names an
  `adapter` is refused with `--dry` (take it out), and `adapter: dry` without `--dry` is refused
  (give `--dry`).
- A model decider runs its own `dry` adapter (it answers by the approve rule and calls no model);
  with no decider in the settings the rule decider `approve` at autonomy `A5` plays the person, so
  the person steps of the copy are bound to `decider:approve` and every one is approved by script. A
  decider or an autonomy that the settings name is kept, except an adapter, which is refused.
- A dry call costs 0. The spend ledger stays at 0 and no login file is read (`login_file` is not
  needed). A dry call still counts against `--max-llm-calls`.
- The effort of a node is on its dry command as `'--effort' '<level>'`: the setting
  `efforts.<role>.effort` (`docs/loom-run-settings.md`), written by the bindings copy as `--effort`
  of `sdc-node`, else the class default of `node-classes.json`. The two adapters that take an effort
  (`metaharness`, `claude-cli`) pass it on as `--effort`; the adapter `dry` builds the metaharness
  command, so it shows it. A level outside `low`, `medium`, `high`, `xhigh`, `max` is refused before
  the run directory is made.
- A dry call writes the prompt it would have sent to `prompt.md` of its call directory
  (`work/.sdc/llm/<NNN>-<action>/`, `docs/loom-run-directory.md`): the program `sdc-node` composes
  it before it starts the adapter.

Every command a dry call would have run is printed on standard output as
`  would run: metaharness <arguments>` (the prompt as `"$(cat <prompt file>)"`) and appended to
`dry-commands.txt` of the run directory. `settings.json` and the first line of `run-settings.jsonl`
carry `dry: true` and `stand_in: "runbook run --dry"`: the run is no evaluation's. The tests are in
`runbook/tests/loom_run/dry.rs`.

### A recorded run

`--recorded <source>` re-drives the case tree with every model call and every decider call answered
from a recording (a directory of `sdc-replay pack`, `driver/examples/sdc/fixtures/recorded/stub` is
one) or from a run directory of this runner or of the old driver, by the adapter half of
`sdc-replay`. No model is called and no login file is read. Options: `--on-divergence stop|nearest|live`
(default `stop`), `--live-adapter`, `--live-cap`, `--inject <fault>` (repeatable: `reject@<decision>#<n>[=<reason>]`,
`timeout@<case>/<action>#<n>`, `check-fail@<action>#<n>`), `--set k=v` (conditions: `max_runs.<name>`,
`timeout_s.<name>`), `--take-recorded <actions>` and `--source-tree <commit>`;
each of them without `--recorded` is exit 64 (`<option> needs --recorded <source>`), as is a source that
is no directory.

- The plan of `sdc-replay prepare` becomes settings of the run: the source's autonomy and decider
  (on the adapter `sdc-replay`), and under `replay` the adapter of each model node (`sdc-replay`
  for a node the source sent to a model, `stub` for the others), the principal that the story
  digests were recorded under (`--principal` of `sdc-node` and `sdc-tool.sh`) and the conditions
  of each action. The runner passes a bound program no environment, so it hands the adapter the
  path of the run's `replay/plan.json`: `sdc-node` takes it as `--replay-plan` (in the argv of each
  replayed node of the bindings copy) and sets `SDC_REPLAY` for its adapter, and the runner's
  decider takes it from `decider.replay_plan` and sets `SDC_REPLAY` in the adapter's environment.
  The adapter searches no directory: a `replay/plan.json` inside `work/` is never read. The settings
  file may not name `adapter` or `replay` (the run is refused and the key named); `replay` without
  `--recorded` is refused too, exit 2 before the run directory is made, naming the key and the
  option, and the one adapter a re-drive takes is `sdc-replay`.
- Conditions are written into the bindings copy (`bindings_copy.rs`): `max_runs` is the binding's
  `budget.max_runs`, `timeout_s` its `timeout_seconds`, and a wrap of the argv (a program the plan names, as for `check-fail`) goes before it.
- After every step the runner reads `replay/diverged.json`. A call that has no recorded reply
  under `stop` (or no reply to reuse, or a live call over `--live-cap`, which fails closed) ends the
  run as `stopped: recording diverged`, exit 11, cause `recording_diverged` in `run-stopped.json`
  and `outcome.json`, with the first divergence as the reason (`diverged.json` keeps the first).
  Exit 9 is `denied`, so the old driver's 9 for a divergence is 11 here. `nearest` reuses the
  nearest recorded reply of the same action; `live` makes the call on `--live-adapter`. The
  request a call is compared by holds the model and the reasoning effort, so a caller's `efforts`
  setting that differs from the recorded effort diverges at the first call, as a changed model
  does (intended). The inputs are compared by digest after what differs between two runs is masked
  (run directories, home paths, instants, revisions, seconds), and so is the version label of the
  pinned ESS in the header line of the conformance log (`in/tests.txt`, written as `<system> v<n>
  against <implementation> <version> — passed`; it is rewritten to the label the committed
  recording was packed with, `0.97.3`, so the recording's digests hold): a new ESS pin does not
  diverge a recording, a changed test name does.
- At the end `sdc-replay seal` writes the counts (`replay.calls`: recorded, reused, live, diverged,
  paid, prompt_changed, faults) into `status.json` or `run-stopped.json`, the virtual seconds into
  `rounds.jsonl` and each call's mode into `llm-calls.jsonl`.
- `sdc-replay` reads a Loom run directory: the run log `store/runlog-case-<hex>.jsonl` and the
  per-case `events.jsonl` that the run directory projects. Arrival and try of a call: arrival 1 at
  the first `Evaluated` line of a case; a later arrival is a change of `case_revision` between two
  `Evaluated` lines with no `ActionFinished` or `ChildClosed` between them (a rejection that sends
  a writer back is not an arrival of its own); try is the number of finished runs since the last
  arrival (not exits 126, 127, 75, 69, 70) plus 1. A decider call is keyed by case, name, arrival
  and try, and its own `in/decision.json` and `in/checks.json` are Loom's and are not compared.
  Recordings of the old driver replay as before.
- Differences from the old launcher, each by what the engine does: the old exit 4 `writer_spent` is
  `stuck` (exit 3); a timed-out call (exit 124) is not retried, because the binding of
  `change.implement` has no result for it and Loom refuses it (`ResultNotInTable`), so the tree stops
  `stuck` (`--inject timeout@...`); `--set max_rejections`, `review_retries` and `unstick`, and
  `max_llm_calls` and `max_wall_seconds` (use the runner's options), are refused with a line;
  no `review_mode` setting is recorded; `--take-recorded` needs a run directory, not a packed
  recording; `--source-tree` is passed to `sdc-replay prepare` as before.
- The committed recording `driver/examples/sdc/fixtures/recorded/stub` was packed again from a run
  of this runner (source tree `a9ca39348b84`, `calls/` unchanged): the old one no longer replayed
  strictly, because the ess pin moved (0.97.0 to 0.97.3) and `diff` orders in the C locale. The old
  launcher diverges identically on `in/tests.txt`, so this was a defect already present.

The tests are in `runbook/tests/loom_run/replay.rs`. A re-drive takes about 70 Loom steps, so no
test makes one, and the nine tests that read a run are the slow lane `task test:replay`, not the
default test step (they are `#[ignore]`d; the default `cargo nextest run` starts no setup step). The
lane runs them with `--run-ignored only` on the nextest profile `replay`, whose setup step
`replay-runs` (`runbook/.config/nextest.toml`, `runbook/tests/replay-runs.sh`) makes the six runs
once, all at once, in `${XDG_STATE_HOME:-$HOME/.local/state}/runbook/replay-tests` (cargo refuses
a crate below the `[workspace]` of `runbook/`, so not under `target/`; runs older than 120 minutes
are removed), and the tests read them (`RUNBOOK_REPLAY_RUNS`); a test takes 0.01 s to 0.05 s. The
setup step may take 300 s, a test 60 s. The lane needs `driver/target/debug` built (the task builds
it). A run is about 64 Loom rounds and 25 CPU seconds (measured 2026-11-22: the six together 150
CPU seconds; setup 88 s wall at host load 70 to 90, 116 s at load 40 to 50 on a later run, and
over 180 s at load 100 to 127). The tests that need no run stay in the default step: the option
refusals (`runbook/tests/loom_run/flags.rs`), the settings and the reading of `diverged.json`
(unit tests in `runbook/src/loom_run/replay.rs`), the clean recording, the indexer on the
committed recording and the plan-only settings (`replay.rs` of the tests), each under 0.1 s.

### Fail closed

The run ends as `error` (exit 1) when: the ledger is unreadable or invalid, a decider answers
without a cost, `tree run` exits with a code the runner does not know, `tree run` prints no JSON,
a model step has no cost record, `tree status` is refused, or an event sink fails.

### What the spend cap does not promise

- A call can end above its budget, and so the ledger above the cap. The budget of a call is the
  `--max-budget-usd` value, lowered to what the cap has left. Both adapters
  (`llm-adapter-claude-cli.sh`, `llm-adapter-metaharness.sh`) pass it on as `--max-budget-usd` to the
  model CLI and do nothing else with it: they do not stop the call, do not cut the cost and write the
  CLI's `total_cost_usd` and `subtype` into the call record. The CLI ends the session at a turn
  boundary, so a call is over its budget by at most the cost of the turn that crossed it. No setting
  of this crate bounds that turn (inferred from the adapters, not measured against a real model).
  The runner books the whole `cost_usd` of the record, never less, and releases the rest of the
  reservation, so the next call finds the real figure, and the run ends as `stopped: spend cap` before
  the next call. Test: `a_call_that_crosses_its_budget_is_booked_in_full_and_the_ledger_ends_above_the_cap`
  (a call given 0.40 USD that costs 0.90 ends the ledger at 1.50 USD against a cap of 1.00).
- A SIGKILL of the runner leaves its reservation in the ledger: `reserved` holds an entry no process
  will release. That entry reduces what the next run finds left (`cap - spent - reserved`), and no
  entry can raise it: the ledger only adds to `spent` and removes the reservation of the process
  that made it. Cost: the money of a killed call is not spent and stays held, so a ledger of a
  campaign loses that much capacity until someone edits `reserved` by hand. Tests:
  `a_reservation_left_by_a_killed_run_reduces_what_is_left_and_never_raises_it` (a stale 0.70 USD
  leaves 0.30 of 1.00; a stale 1.00 leaves none and the run stops before its first call) and
  `after_a_sigkill_the_next_run_on_the_same_ledger_finds_less_left_not_more` (a real SIGKILL on a
  running call, then a second run on the same ledger).

## The run directory

```
<runs-dir>/<name>/settings.json    redacted settings, and what the command was given: loom
                                   {sha256 | in_process and commit, confinement, cap_micro_usd},
                                   max_llm_calls, max_wall_seconds, case, root_protocol (not the
                                   binary's path)
                  events.jsonl     one object per event: {"at": seconds, "event": "...", ...}
                  outcome.json     runbook.loom-run-outcome/1: outcome, reason, loom_exit_code,
                                   stderr_tail, exit_code, steps, spent_micro_usd, llm_calls
                  tree-status.json, tree-verify.json
                  protocols/ bindings/ work/ store/ home/
```

The files this command writes name no path of the runs directory: in `events.jsonl`,
`outcome.json`, `tree-status.json` and `tree-verify.json` the run directory is made relative
(`<run dir>/store` is `store`, the run directory itself `.`). Loom's own `store/tree.json` holds
absolute paths and is not edited, so the run directory as a whole can still hold one.

The files that `sdc-experiment report` and `runbook parity summarize` read are written by
`run_dir.rs` from the same events (`docs/loom-run-directory.md`).

## Tests

`runbook/tests/loom_run/` runs the command against a fake `b10x-loom` that the tests generate (a
`/bin/sh` script that records its argv and environment and answers from scripted files); no test
of the gate lane starts the real binary or makes a model call. The wall-time cap is also tested with
an injected clock in `run_loop.rs`.

The slow lane is `task test:loom-real` (needs `RUNBOOK_LOOM_BIN`, the real binary, and
`RUNBOOK_LOOM_WORK`, a directory outside the repository: below the repository `cargo` finds its
workspace above the product crate and refuses it): the stub cycle through
`runbook run` on the real binary with `--confinement local`, scripted approvals and no model,
summarized with `runbook parity summarize --loom-run` and compared with the committed driver summary
`runbook/tests/fixtures/parity/stub-cycle-driver-summary.json` (`docs/parity.md`). The test is
`#[ignore]`d and runs with `--ignored`; it takes minutes, so run the task in the background.

`runbook/tests/loom_run/inproc.rs` runs `runbook run` with Loom as a library on the small `sh` fixture runbook of the
engine tests (`runbook/tests/fixtures/engine-runbook/`), in the gate lane and with no `b10x-loom`:
a delivery with scripted approvals, a rejection with reasons that sends the writer back, the spend
cap, a sample of `/proc` that finds no `b10x-loom`, `canon` or `sdc-drive` below the process, the
environment of a bound program, and the usage errors. The slow lane `task test:loom-in-process`
(needs `RUNBOOK_LOOM_WORK`; starts the child binary of Loom `70b0e6e0` from
`~/.local/state/runbook/loom/70b0e6e0/b10x-loom`, or `RUNBOOK_LOOM_BIN`, and skips with a message
when it is absent) runs the `sdc` stub cycle once through the child and once with Loom as a library (no `--loom`),
summarizes both with `runbook parity summarize --loom-run` and compares them with `runbook parity
compare --product-digests`, which has to end `parity: identical`.

With `--loom`, the runner starts, in that run, only `runbook`, `b10x-loom` and the programs the bindings name
(`sdc-tool.sh`, `sdc-node`, and below them `sdc-findings`, `ess`, `cargo` and what `cargo` starts);
no `sdc-drive`, `run-trial.sh` or `run-finding-cycles.sh`.
