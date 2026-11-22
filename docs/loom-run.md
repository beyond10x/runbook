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

Every option is required and none has a default. `--confinement` is passed unchanged to every
`tree run` (`local` is unconfined and declared insecure; `substrate` is Substrate's linux adapter).

## What it does

1. Refuses (exit 2, nothing written) when `--loom` is not the file `--loom-sha256` names, `--run` is
   not a plain name, the settings are not a JSON object, the decider settings are not usable, a
   path is missing, or `<runs-dir>/<name>` exists.
2. Creates the run directory, writes `settings.json` (the login file's path replaced by `named`),
   copies `--bindings` into `bindings/` (the copy is what Loom reads; the checkout is never
   edited), puts `--idea` at `work/idea.yaml`, registers every `*.protocol.yaml` of `--protocols`
   (`b10x-loom protocols add`) and opens the tree (`tree open --store store/ --workspace work/`).
3. Loops `b10x-loom tree run --max-steps 1 --confinement <level>`. After each step it reads what the
   step cost, then reacts to the exit code of `tree run`:

   | Exit | Meaning | The runner |
   |---|---|---|
   | 0 | terminal | ends: outcome `ended` |
   | 3 | waits for a decision | asks the decider for each entry of `decisions[]`, runs the printed command, loops |
   | 4 | stuck | ends: outcome `stuck` (runner exit 3) |
   | 5 | the step budget of one step is spent | loops (that is every normal step) |
   | anything else | not known | ends: outcome `error` with the exit code and the tail of standard error |

4. At the end it stores `tree status` and `tree verify` (`tree-status.json`, `tree-verify.json`). A
   non-zero `tree verify` makes the outcome `error`.

## The decisions

A decision is a JSON object of which only `case`, `kind`, `decides` and `command` are stable. The
runner passes the whole object to the decider whatever its `kind` (an unknown kind is the decider's
to answer or refuse, and a refusal names the kind), never reads `decides`, and runs `command` as
data: `command[0]` must be `b10x-loom`, the rest is the argv. It changes two things only: the value
after `--principal` when the decider named a principal, and a final `--deny` when the answer is a
denial. A printed command that exits non-zero ends the run as `error`, with its exit code and the
last 1500 characters of its standard error in `outcome.json`; the runner does not repair or repeat it.

A denied decision that comes back (same case, action, revision and text) ends the run as `denied`
(exit 9) without asking the decider again. An approved decision that comes back is an `error`.

The decider is chosen from `settings.json` key `decider` (see `runbook/src/loom_run/decider.rs`).
With no `decider` key every decision ends the run as `stopped: decider`. `{"kind": "scripted",
"answers": [...]}` is the test double the tests use.

## The environment of the child

The child gets a built environment (`env_clear`, then these three), never the runner's:

| Variable | Value |
|---|---|
| `PATH` | the runner's `PATH` |
| `HOME` | the directory `home/` of the run directory |
| `XDG_DATA_HOME` | `data/` inside that directory (Loom's protocol catalog) |

Bound programs get only `PATH` from Loom itself. No login file, no path of one and no credential
variable is in the run directory or in the child's environment; the login file's path is a setting
the bindings copy uses (a link inside the copy) and is replaced by `named` in every file the runner
writes.

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
to `.sdc/llm/calls.jsonl` at its end. A directory without a matching line, a null cost or an
unreadable line fails the run closed and books the whole reservation.

### Fail closed

The run ends as `error` (exit 1) when: the ledger is unreadable or invalid, a decider answers
without a cost, `tree run` exits with a code the runner does not know, `tree run` prints no JSON,
a model step has no cost record, or an event sink fails. Known limits: a SIGKILL of the runner
leaves a reservation in the ledger (it counts against the cap until the ledger is reset), and one
call can overshoot its budget by what the model tool itself allows.

## The run directory

```
<runs-dir>/<name>/settings.json    redacted settings
                  events.jsonl     one object per event: {"at": seconds, "event": "...", ...}
                  outcome.json     runbook.loom-run-outcome/1: outcome, reason, loom_exit_code,
                                   stderr_tail, exit_code, steps, spent_micro_usd, llm_calls
                  tree-status.json, tree-verify.json
                  bindings/ work/ store/ home/
```

The files that `sdc-experiment report` and `runbook parity summarize` read are written by
`run_dir.rs` from the same events.

## Tests

`runbook/tests/loom_run/` runs the command against a fake `b10x-loom` that the tests generate (a
`/bin/sh` script that records its argv and environment and answers from scripted files); no test
starts the real binary or makes a model call. The wall-time cap is also tested with an injected
clock in `run_loop.rs`.
