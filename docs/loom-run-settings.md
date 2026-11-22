# The settings file of `runbook run`: `runbook.loom-run-settings/1`

One JSON object, named by `--settings`. One format serves the loop (`docs/loom-run.md`), the decider
and the bindings copy (`docs/loom-run-decisions.md`) and the run directory writer
(`docs/loom-run-directory.md`). `sdc-experiment run` writes this file for a launched run
(`docs/workflow-metrics.md` section 7.1); the last section lists what it writes. The reading is `runbook/src/loom_run/settings.rs` (`normalise`, `fill_decider`),
`decider.rs` (`from_settings`) and `bindings_copy.rs` (`materialise`).

An unknown key is refused by name, at any level, before the run does anything that costs money. A
key that repeats an option must agree with it; the option decides and a different value is a refusal
(exit 2, nothing written), never a silent override.

## Top level

| Key | Type | Default | Read by | Meaning |
|---|---|---|---|---|
| `format` | text | none | `normalise` | when present, `runbook.loom-run-settings/1` |
| `run` | text | none | `normalise` | when present, equal to `--run` |
| `loom` | object | none | `normalise` | `sha256` (equal to `--loom-sha256`; refused when Loom is a library (no `--loom`)), `in_process` (`true`, only when Loom is a library), `confinement`, `cap_micro_usd` (each equal to its option `--confinement`, `--cap-micro-usd`), `login_file` (see below). `loom.bin` and any other member are refused: the binary is the option `--loom` |
| `login_file` | path | none | `materialise`, decider | absolute path of the login file, a file that exists. `loom.login_file` is the same setting; give one of them. It is replaced by `named` in every file the runner writes |
| `max_llm_calls` | whole number | none | `normalise` | when present, equal to `--max-llm-calls`; null or text is refused |
| `max_wall_seconds` | whole number | none | `normalise` | when present, equal to `--max-wall-seconds` |
| `autonomy` | `A0` to `A5` | none | decider, `materialise` | the decider answers only at `A5`; with no level every decision stops the run for a person |
| `decider` | object | none | `from_settings` | see below; no `decider` key: every decision ends the run as `stopped: decider` |
| `models` | object | none | `materialise` | `models.<role>` for `writer`, `implement`, `review`, `other`; members `model` (text), `max_turns` (whole number, 1 or more), `max_budget_usd` (number above 0); written into the model nodes of the copy |
| `efforts` | object | none | `efforts` (`bindings_copy.rs`), `materialise` | `efforts.<role>` for `writer`, `implement`, `review`, `other`; the one member is `effort`: `low`, `medium`, `high`, `xhigh` or `max`. Written as `--effort <level>` into the model nodes of the role in the copy; a role with none keeps the class default of `node-classes.json`. Another role, another member or another level is refused by name (`settings.efforts.writer.effort`) before the run directory is made. The decider's effort is `decider.effort` |
| `adapter` | `metaharness` or `claude-cli` | none | `materialise` | `--adapter` of every model node of the copy |
| `review` | object | none | `materialise` | only `review.adapter: claude-cli` |

The recorded copy of the settings (`settings.json`, the `RunStarted` event) is this file with
`login_file` replaced by `named`, `format` and `run` taken out, and `loom`, `max_llm_calls`,
`max_wall_seconds`, `case` and `root_protocol` set from the options. It never holds the path of the
binary.

## `decider`

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
| `answers`, `max_call_micro_usd` | list, whole number | none | scripted | the double's answers and its reservation |

## What `sdc-experiment run` writes, and what a run does with it

Both sides use this format. The writer is `loom_settings_json` in `driver/src/bin/sdc-experiment.rs`
(`runner: loom` records). It writes `format`, `run`, `autonomy`, `decider.kind`, `decider.model` (or
`decider.verdict` for `rule`), `models.<role>.model` for the four roles, `max_llm_calls`,
`max_wall_seconds`, `loom.sha256` (`loom.in_process: true` in its place for a record that sets it), `loom.confinement`, `loom.cap_micro_usd` and `loom.login_file`
(only when the record names one), and, when the record sets them:

| Record key | File key |
|---|---|
| `decider.kind` is not `rule` | `decider.without_claims: true` (a model decider acknowledges that Loom's output holds no claims) |
| `decider.escalate` | `decider.escalate` |
| `decider.max_decider_calls` | `decider.max_decider_calls` (the decider call limit) |
| `decider.adapter` | `decider.adapter` |
| `adapter` | `adapter` |
| `review.adapter` | `review.adapter` |

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
The decider runs inside the runner and starts no `decider.sh`, `jq`, `yq` or `rsync`: `jq` is needed
only by the bash adapters it starts; the shell programs of the bindings (`sdc-node`,
`sdc-tool.sh`, the adapters) still need `jq` and `yq`.

Efforts (`efforts.<role>.effort`) and `implement_mode` stay refused in a record of a `loom`
variant, with the key named: the runner takes the efforts (`efforts` above, `sdc-node --effort`),
but the record format does not carry them to it yet. `flags.recorded` is passed on as
`--recorded`: the runner has a recorded mode (`runbook run --recorded`, `docs/loom-run.md`), and
`sdc-experiment` passes the replay flags of the record (`REPLAY_FLAGS`) as the options that go with
it. `flags.dry: 'true'` is accepted: `sdc-experiment run` passes `--dry` to `runbook run`, and then `adapter` and `decider.adapter` are not set (the runner puts every model node and a model decider on the adapter `dry`).

The proof is a file, not a copy of the other crate's code. `driver/tests/experiment_loom.rs` writes the
file for the strong record and compares it with `runbook/tests/fixtures/loom-run-settings.json`
(one equality check; the fixture holds the print form with the login path redacted). The runner's
`runbook/tests/loom_run/settings.rs` reads that same fixture, fills in the login path and the
sha256 of the fake binary, and runs it to the end; neither test builds the other crate.
