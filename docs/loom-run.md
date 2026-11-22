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
