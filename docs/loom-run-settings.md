# The settings file of `runbook run`: `runbook.loom-run-settings/1`

One TOML file, named by `--settings`. A file that is JSON is refused before anything starts, by its
content and not by its name, also after a byte order mark, blank lines and comment lines, with a
message that says it is JSON and names the TOML form and this page (exit 2, nothing written). A syntax
error is refused with its line, its column and the parser's own words for the kind of error (cut to
160 characters), and never with a line, a value or a part of a value of the file: a line of the file can
hold the path of the login file. One format serves the loop (`docs/loom-run.md`), the decider and the bindings copy
(`docs/loom-run-decisions.md`) and the run directory writer (`docs/loom-run-directory.md`).
`sdc-experiment run` writes this file for a launched run (`docs/workflow-metrics.md` section 7.1);
the last section lists what it writes. The reading is `runbook/src/loom_run/settings.rs` (`read`,
`normalise`, `fill_decider`), `decider.rs` (`from_settings`) and `bindings_copy.rs` (`materialise`).

A `runbook.toml` (`docs/loom-run.md` "Where a value comes from") may carry the members `login_file`,
`autonomy`, `decider`, `adapter`, `models`, `efforts` and `review` of this format as defaults, read by the same reader; a
member of the `--settings` file replaces the file's member as a whole. `format`, `run`, `loom_sha256`,
`in_process` and `replay` belong to one run and are refused in that file.

Each setting has one place. A table exists only where it groups: `[decider]`, `[models.<role>]`,
`[efforts.<role>]` and `[review]`. There is no `[loom]` table: what lived under `loom` in the JSON form
is at the top level (the table at the end of this section lists the old and the new place).

An unknown key is refused by name, at any level, before the run does anything that costs money. A
key that repeats an option must agree with it; the option decides and a different value is a refusal
(exit 2, nothing written), never a silent override. A key in an old place is refused with its new
place named.

Types are TOML's, and a member takes the one type it names. A whole number is written as an integer
(`100`): `100.0`, `1e2`, `"100"`, a negative number and an integer that does not fit 64 bits are
refused, as are a date, a date and time and a time, which no setting takes. A path is text.

A whole number may be written in any spelling TOML gives an integer: `0x10` is 16, `0o20` is 16,
`0b10` is 2, `1_000` is 1000 and `+5` is 5. A float (`1.0`, `1e3`, `inf`, `nan`) is refused where a
whole number is asked for. The three members that repeat an option, `cap_micro_usd`, `max_llm_calls`
and `max_wall_seconds`, must equal the option: a spelling that is the option's number (`0x4C4B40` for
5000000) is the option, any other number is refused with the member named, and no value is read as
larger than the option, as absent or as unlimited.

## Example

One complete file for a run of a model decider on a child Loom binary (`run` is what `--run`
names, `loom_sha256` what `--loom-sha256` names). `runbook/src/loom_run/settings.rs` reads this block
in a test, so it is the form the runner reads.

```toml
format = "runbook.loom-run-settings/1"
run = "lx.low.1"
autonomy = "A5"
adapter = "metaharness"
max_llm_calls = 100
max_wall_seconds = 3600
loom_sha256 = "d80830ea5b1b2c3d4e5f60718293a4b5c6d7e8f90123456789abcdef01234567"
confinement = "local"
cap_micro_usd = 5000000
login_file = "/path/to/the/login-file"

[decider]
kind = "model"
model = "claude-opus-5-5"
without_claims = true
escalate = "final"
adapter = "claude-cli"
effort = "high"
max_turns = 8
max_budget_usd = 0.25
max_decider_calls = 40
retries = 2
timeout_seconds = 900
requester = false

[review]
adapter = "claude-cli"

[models.writer]
model = "claude-sonnet-5-5"
max_turns = 30
max_budget_usd = 2.5

[models.implement]
model = "claude-sonnet-5-5"

[models.review]
model = "claude-sonnet-5-5"

[models.other]
model = "claude-sonnet-5-5"

[efforts.review]
effort = "max"

[efforts.other]
effort = "low"
```

## Top level

| Key | Type | Default | Read by | Meaning |
|---|---|---|---|---|
| `format` | text | none | `normalise` | when present, `runbook.loom-run-settings/1` |
| `run` | text | none | `normalise` | when present, equal to `--run` |
| `loom_sha256` | text | none | `normalise` | when present, equal to `--loom-sha256`; refused when Loom is a library (no `--loom`) |
| `in_process` | `true` | none | `normalise` | `true` only when Loom is a library (no `--loom`); `false` and any other value are refused |
| `confinement` | text | none | `normalise` | when present, equal to `--confinement`. The option has no default and a file never sets or lowers the level. `runbook serve` reads the same key as its default level for a submit that names none (`--default-confinement` wins; `docs/daemon-api.md`) |
| `cap_micro_usd` | whole number | none | `normalise` | when present, equal to `--cap-micro-usd` |
| `login_file` | path | none | `materialise`, decider | absolute path of the login file, a file that exists; its content is never a setting. It is replaced by `named` in every file the runner writes |
| `max_llm_calls` | whole number | none | `normalise` | when present, equal to `--max-llm-calls`; text, a float and a date are refused |
| `max_wall_seconds` | whole number | none | `normalise` | when present, equal to `--max-wall-seconds` |
| `autonomy` | `A0` to `A5` | none | decider, `materialise` | the decider answers only at `A5`; with no level every decision stops the run for a person |
| `decider` | table | none | `from_settings` | see below; no `decider` table: every decision ends the run as `stopped: decider` |
| `models` | tables | none | `materialise` | `[models.<role>]` for `writer`, `implement`, `review`, `other`; members `model` (text), `max_turns` (whole number, 1 or more), `max_budget_usd` (number above 0, an integer or a float); written into the model nodes of the copy |
| `efforts` | tables | none | `efforts` (`bindings_copy.rs`), `materialise` | `[efforts.<role>]` for `writer`, `implement`, `review`, `other`; the one member is `effort`: `low`, `medium`, `high`, `xhigh` or `max`. Written as `--effort <level>` into the model nodes of the role in the copy; a role with none keeps the class default of `node-classes.json`. Another role, another member or another level is refused by name (`settings.efforts.writer.effort`) before the run directory is made. The decider's effort is `decider.effort` |
| `adapter` | `metaharness` or `claude-cli` | none | `materialise` | `--adapter` of every model node of the copy |
| `review` | table | none | `materialise` | only `adapter = "claude-cli"` |
| `replay` | | | | not a setting: `--recorded` makes it (the plan of a re-drive); a file that holds it is refused |

The members of the old JSON form that lived under `loom`, and where they are now:

| Old place | New place |
|---|---|
| `loom.sha256` | `loom_sha256` |
| `loom.in_process` | `in_process` |
| `loom.confinement` | `confinement` |
| `loom.cap_micro_usd` | `cap_micro_usd` |
| `loom.login_file` | `login_file` |
| `loom.bin` | none: the binary is the option `--loom`, not a setting |

The recorded copy of the settings (`settings.json`, the `RunStarted` event) stays JSON and keeps its
form: it is the settings with `login_file` replaced by `named`, `format` and `run` taken out, and
`loom` (an object, as before: `sha256` or `in_process` and `commit`, `confinement`, `cap_micro_usd`),
`max_llm_calls`, `max_wall_seconds`, `case` and `root_protocol` set from the options. It never holds
the path of the binary. The same holds for `run-settings.jsonl` (`sdc.run-settings/1`). The input is
TOML, the records are not. The members of the records stand in the order of the settings file, with
`loom`, `case` and `root_protocol` after them; a reader of the records reads by member name, not by
position (the file `sdc-experiment` writes has its own order, so a run of 0.7.0 from such a file has
its members in another order).

## `[decider]`

| Key | Type | Default | Kinds | Meaning |
|---|---|---|---|---|
| `kind` | `model`, `rule`, `scripted` | none: refused | all | `model`: one call of the adapter per decision, made by the decider inside the runner. `rule`: the runner answers by the rule `approve` or `reject`, no program. `scripted`: the test double |
| `model` | text | none: refused | model | `model` of the adapter's request |
| `verdict` | `approve`, `reject` | none: refused | rule | the rule's answer |
| `without_claims` | true, false | false | model | the run's acknowledgement that Loom's output holds no claims; without it every decision is `CannotDecide` |
| `reply_bin` | path | the `sdc-reply` found on the runner's `PATH`, else none | model | handed to the adapter as `SDC_REPLY` (the decider itself reads no `sdc-reply`; a scripted adapter of a test may) |
| `program` | | | | not a setting: the decider runs inside the runner; a value in the file is refused |
| `adapter` | text | `metaharness` | model | the adapter program: `metaharness`, `claude-cli`, `dry` (of the bindings copy), `stub` (no call: every decision is an invalid reply) or a path |
| `effort` | `low`, `medium`, `high`, `xhigh`, `max` | `node-classes.json` of the copy (xhigh) | model | `effort` of the adapter's request |
| `max_turns` | whole number, 1 or more | 8 | model | `max_turns` of the adapter's request |
| `max_budget_usd` | number above 0 | 0.25 | model | most one decider call may cost |
| `max_calls`, `max_decider_calls` | whole number, 1 or more | 20 | model | most decider model calls; the two names are one limit, give one |
| `requester` | true, false | false | model | with `--goals`: the decisions of the requester-side classes are answered from the goals as `decider:requester` (`docs/loom-run-decisions.md` "The requester decider"); true without `--goals` is refused |
| `escalate` | `final`, `stop` | `stop` | model | what an `escalate` reply does |
| `retries` | whole number | 2 | model | asks again after an invalid reply |
| `timeout_seconds` | whole number, 1 or more | 900 | model, rule | the program's whole process group is ended at the limit |
| `principal` | text | none | all | model, rule: accepted when it is what the kind gives (`decider:model`, `decider:approve`, `decider:reject`) and then taken out; another value is refused. scripted: the principal of every answer that names none (an answer's own `principal` wins); none given: the principal Loom printed is kept |
| `answers`, `max_call_micro_usd` | list of tables, whole number | none | scripted | the double's answers and its reservation |

## What `sdc-experiment run` writes, and what a run does with it

Both sides use this format. The writer is `loom_settings_toml` in `driver/src/bin/sdc-experiment.rs`
(`runner: loom` records); it writes `settings.toml` in the run's launch directory, outside the run
directory. It writes `format`, `run`, `autonomy`, `decider.kind`, `decider.model` (or
`decider.verdict` for `rule`), `models.<role>.model` for the four roles, `max_llm_calls`,
`max_wall_seconds`, `loom_sha256` (`in_process = true` in its place for a record that sets it),
`confinement`, `cap_micro_usd` and `login_file` (only when the record names one), and, when the
record sets them:

| Record key | File key |
|---|---|
| `decider.kind` is not `rule` | `decider.without_claims = true` (a model decider acknowledges that Loom's output holds no claims) |
| `decider.escalate` | `decider.escalate` |
| `decider.max_decider_calls` | `decider.max_decider_calls` (the decider call limit) |
| `decider.adapter` | `decider.adapter` |
| `adapter` | `adapter` |
| `review.adapter` | `review.adapter` |

The record keeps its own names (`loom.confinement`, `loom.sha256`, `loom.cap_micro_usd`,
`loom.login_file`, `loom.in_process`: `experiments/*.yaml` are records of past runs and are not
rewritten); the writer maps them to the places above. A value is written so that the reader reads
back the same value: a text is quoted and escaped by the TOML library, and a number that is not a
whole number of 64 bits is refused by the writer, never written as something else.

It does not write `decider.principal`: the runner gives a model or rule decider the principal of its
kind (`decider:model`, `decider:<verdict>`). A `scripted` decider (the stub-cycle comparison) names
its principal in the file, and `decider.principal` is the default of its answers (an answer's own
`principal` wins). Nothing about the principal is fixed in the runner for one cycle.

`sdc-reply`, `jq` and `yq` are reached through `PATH`, not through `decider.reply_bin`. The
launcher puts the build's `driver/target/debug` first on the unit's `PATH` (`sdc-reply`,
`sdc-findings` and `sdc-bugfix` are there) and refuses to start when `jq` or `yq` is on no
directory of that `PATH` (the shell programs of the bindings need them; `rsync` is no longer
needed by a run). The reason: `b10x-loom` gives every bound program the `PATH` the runner was
started with, and the model nodes of the bindings (`sdc-node`) call `sdc-reply` too, so a directory
on `PATH` reaches the decider and the nodes, and a `decider.reply_bin` would reach the decider only.
`decider.reply_bin` stays accepted in the file for a caller that does not control `PATH`.
The decider runs inside the runner and starts no `jq`, `yq` or `rsync`: `jq` is needed
only by the bash adapters it starts; the shell programs of the bindings (`sdc-node`,
`sdc-tool.sh`, the adapters) still need `jq` and `yq`.

Efforts (`efforts.<role>.effort`) and `implement_mode` stay refused in a record of a `loom`
variant, with the key named: the runner takes the efforts (`efforts` above, `sdc-node --effort`),
but the record format does not carry them to it yet. `flags.recorded` is passed on as
`--recorded`: the runner has a recorded mode (`runbook run --recorded`, `docs/loom-run.md`), and
`sdc-experiment` passes the replay flags of the record (`REPLAY_FLAGS`) as the options that go with
it. `flags.dry: 'true'` is accepted: `sdc-experiment run` passes `--dry` to `runbook run`, and then `adapter` and `decider.adapter` are not set (the runner puts every model node and a model decider on the adapter `dry`).

The proof is a file, not a copy of the other crate's code. `driver/tests/experiment_loom.rs` writes the
file for the strong record and compares it byte for byte with `runbook/tests/fixtures/loom-run-settings.toml`
(the fixture holds the print form with the login path redacted as `<loom.login_file>`). The
runner's `runbook/tests/loom_run/settings.rs` reads that same fixture, fills in the login path and the
sha256 of the fake binary, and runs it to the end; neither test builds the other crate. The path with a
quote, a backslash and a letter outside ASCII has its own fixture,
`runbook/tests/fixtures/loom-run-settings-odd-path.toml`, which the writer's test compares byte for byte
and the runner's reader reads back to the same path.

## `[deciders.<kind>]` of the daemon

`runbook serve` reads one more table of `runbook.toml`, `[deciders.<kind>]`, for the runs of the daemon that take
an autonomy level and a decider kind (`docs/engine.md`, "Autonomy and a decider"). `runbook run` does not read it: it has
the one `decider` of its settings. A kind is `rule` or `model`; the table holds the members of `[decider]` without
`kind`. A table the decider's reader refuses, a kind the daemon does not take and a `kind` member that is not the
table's own name end `runbook serve` with exit 2, naming the table and no value.

| Where | Meaning |
|---|---|
| `runbook serve --autonomy A0..A5` | the level of a submit that names none; none given: such a run has no level and every decision of it waits for a client; needs `--decider`, or the daemon does not start |
| `runbook serve --decider <kind>` | the kind of a submit that names none; a kind that is not configured and the daemon does not start |
| `submit_run` `autonomy`, `decider` | the run's own level and kind; they win over the daemon's; an unknown level, a kind that is not configured and a level with no kind are refused (400) before anything is written |
| `login_file`, `cap_micro_usd`, `max_llm_calls` | read for the model decider of a run only; none is required, and a rule needs none |
