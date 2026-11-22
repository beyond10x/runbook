# `runbook loom-run`: one case tree through `b10x-loom`

`story:runbook-loom-run-command`. The command runs one case tree to its end, to a cap or to a stop,
by starting the `b10x-loom` binary as a child process, with a spend ledger that caps the money over
all runs and a run directory that records what happened. It makes model calls only when the bound
programs or the decider do; the command itself calls no model and no API.

```
runbook loom-run --run <name> --runs-dir <dir> --loom <b10x-loom> --loom-sha256 <hex>
  --bindings <bindings/> --protocols <dir> --root-protocol <name>@<revision> --case <id>
  --idea <file> --confinement <local|substrate> --settings <file.json>
  --spend-ledger <file> --cap-micro-usd <n> --max-llm-calls <n> --max-wall-seconds <n>
```

Every option is required and none has a default. The settings file has one format, `runbook.loom-run-settings/1` (`docs/loom-run-settings.md`). `--confinement` is passed unchanged to every
`tree run` (`local` is unconfined and declared insecure; `substrate` is Substrate's linux adapter).

## What it does

1. Refuses (exit 2, nothing written) when `--loom` is not the file `--loom-sha256` names, `--run` is
   not a plain name, the settings are not a JSON object, the decider settings are not usable, a
   path is missing, or `<runs-dir>/<name>` exists.
2. Creates the run directory, writes `settings.json` (the login file's path replaced by `named`),
   copies `--bindings` into `bindings/` (the copy is what Loom reads; the checkout is never
   edited), puts `--idea` at `work/idea.yaml`, copies every `*.protocol.yaml` of `--protocols` into
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
the kinds `model` and `rule` the runner sets `decider.program` to `bindings/tools/decider.sh` of the
run's own copy and, for `model`, finds `sdc-reply` on its `PATH` when `decider.reply_bin` is not
given; `jq`, `yq` and `rsync` have to be on that `PATH` too (`decider.sh` exits 127 without them and
the run ends as `stopped: decider`). The bindings copy is made by `bindings_copy::materialise` with
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
`--claude-credentials <link>` into the argv of every model node, `llm-node.sh` turns it into
`SDC_CLAUDE_CREDENTIALS`, and each adapter makes its own private copy for its one call. The copy lies
in the private directory of the run, the directory of the link (the adapter takes it when the
directory of `SDC_CLAUDE_CREDENTIALS` belongs to the user and has mode 700), and never in the call's
record directory, which is inside the run directory:

| Path | How it finds the login | Where the copy lies | Who removes it |
|---|---|---|---|
| `llm-adapter-claude-cli.sh` (`--adapter claude-cli`; the review step) | copies it into a config home (mode 700, file 600) and sets `CLAUDE_CONFIG_DIR` for `claude`; the whole config home moves, as it is the directory `claude` is given | `<private dir>/config-home.XXXXXX` | the adapter on exit; after a SIGKILL the runner |
| `llm-adapter-metaharness.sh` (`--adapter metaharness`) | metaharness reads `$HOME/.claude/.credentials.json` and no `CLAUDE_CONFIG_DIR`, and finds `claude` only on `$HOME/.local/bin:/usr/local/bin:/usr/bin:/bin`; the adapter makes a home (mode 700) with `.claude/.credentials.json` (a copy, mode 600) and `.local/bin/claude` (a link to the `claude` of its `PATH`), and sets `HOME` and `TMPDIR` to it for the metaharness process alone, so metaharness's scratch root (its own copy of the login in its config home) lies there too | `<private dir>/h.XXXXXX`, the scratch root `<private dir>/h.XXXXXX/.tmpXXXXXX` | metaharness removes its scratch root when it ends, the adapter the home on exit, also after a failed call or SIGTERM; after a SIGKILL the runner |
| `decider.sh` (the model decider) | `SDC_CLAUDE_CREDENTIALS` from the runner (the link), handed to `llm-node.sh`, so the adapter of the first or second row | as its adapter | as its adapter |

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
| SIGTERM or SIGINT | `stopped: signal` | 7 | the running call is ended (SIGTERM, then SIGKILL) |

Other exits: 0 ended, 1 error, 2 refused, 3 stuck, 8 `stopped: decider`, 9 `denied`, 64 usage.

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
(`driver/src/main.rs`, `program_error`). `runbook loom-run` does the same:

- A call with an exit other than 0, 126, 127 and 75 and no usable cost is booked at its reservation
  (the spend rule stays fail-closed), written to `llm-calls.jsonl` with `cost_unknown: true`, and the
  run goes on; repeated failures run into the spend cap and end as `stopped: spend cap`.
- A call with exit 126, 127 or 75, a call with exit 0 and no cost, and a call directory with no line
  end the run as `error`, with the reservation booked.
- A call directory with no line and no `request.json`, whose `status.json` names an exit, is no call:
  `llm-node.sh` writes `request.json` before every start of an adapter, so the step ended before
  any adapter started. It costs nothing, its reservation is released, the event `no_model_call` (note
  `step, no model call made`, the call's name and the exit) is written to `events.jsonl`, and the run
  goes on with what Loom says of the step. A directory with `request.json`, or without a `status.json`
  that names an exit, stays as the previous item says. The decider's calls do not pass the cost
  reader and are unchanged.
- The reason of such an end is `model step <action>: adapter <adapter> exited <code>: <first line of
  the adapter's stderr>`: the first non-empty line of `adapter-stderr.txt`, else `stderr.txt`, of the
  call directory, the home prefix replaced by `~`, cut at 300 characters, `(no stderr)` when empty.

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
                                   {sha256, confinement, cap_micro_usd}, max_llm_calls,
                                   max_wall_seconds, case, root_protocol (not the binary's path)
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
`runbook loom-run` on the real binary with `--confinement local`, scripted approvals and no model,
summarized with `runbook parity summarize --loom-run` and compared with the committed driver summary
`runbook/tests/fixtures/parity/stub-cycle-driver-summary.json` (`docs/parity.md`). The test is
`#[ignore]`d and runs with `--ignored`; it takes minutes, so run the task in the background.

The runner starts, in that run, only `runbook`, `b10x-loom` and the programs the bindings name
(`sdc-tool.sh`, `llm-node.sh`, and below them `sdc-findings`, `ess`, `cargo` and what `cargo` starts);
no `sdc-drive`, `run-trial.sh` or `run-finding-cycles.sh`.
