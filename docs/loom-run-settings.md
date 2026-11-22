# The settings file of `runbook loom-run`: `runbook.loom-run-settings/1`

One JSON object, named by `--settings`. One format serves the loop (`docs/loom-run.md`), the decider
and the bindings copy (`docs/loom-run-decisions.md`) and the run directory writer
(`docs/loom-run-directory.md`). `sdc-experiment run` writes this file for a launched run
(`docs/workflow-metrics.md` section 7.1); the last section lists what it writes today and what must
change. The reading is `runbook/src/loom_run/settings.rs` (`normalise`, `fill_decider`),
`decider.rs` (`from_settings`) and `bindings_copy.rs` (`materialise`).

An unknown key is refused by name, at any level, before the run does anything that costs money. A
key that repeats an option must agree with it; the option decides and a different value is a refusal
(exit 2, nothing written), never a silent override.

## Top level

| Key | Type | Default | Read by | Meaning |
|---|---|---|---|---|
| `format` | text | none | `normalise` | when present, `runbook.loom-run-settings/1` |
| `run` | text | none | `normalise` | when present, equal to `--run` |
| `loom` | object | none | `normalise` | `sha256`, `confinement`, `cap_micro_usd` (each equal to its option `--loom-sha256`, `--confinement`, `--cap-micro-usd`), `login_file` (see below). `loom.bin` and any other member are refused: the binary is the option `--loom` |
| `login_file` | path | none | `materialise`, decider | absolute path of the login file, a file that exists. `loom.login_file` is the same setting; give one of them. It is replaced by `named` in every file the runner writes |
| `max_llm_calls` | whole number | none | `normalise` | when present, equal to `--max-llm-calls`; null or text is refused |
| `max_wall_seconds` | whole number | none | `normalise` | when present, equal to `--max-wall-seconds` |
| `autonomy` | `A0` to `A5` | none | decider, `materialise` | the decider answers only at `A5`; with no level every decision stops the run for a person |
| `decider` | object | none | `from_settings` | see below; no `decider` key: every decision ends the run as `stopped: decider` |
| `models` | object | none | `materialise` | `models.<role>` for `writer`, `implement`, `review`, `other`; members `model` (text), `max_turns` (whole number, 1 or more), `max_budget_usd` (number above 0); written into the model nodes of the copy |
| `adapter` | `metaharness` or `claude-cli` | none | `materialise` | `--adapter` of every model node of the copy |
| `review` | object | none | `materialise` | only `review.adapter: claude-cli` |

The recorded copy of the settings (`settings.json`, the `RunStarted` event) is this file with
`login_file` replaced by `named`, `format` and `run` taken out, and `loom`, `max_llm_calls`,
`max_wall_seconds`, `case` and `root_protocol` set from the options. It never holds the path of the
binary.

## `decider`

| Key | Type | Default | Kinds | Meaning |
|---|---|---|---|---|
| `kind` | `model`, `rule`, `scripted` | none: refused | all | `model`: one `decider.sh model` call per decision. `rule`: `decider.sh approve` or `reject`. `scripted`: the test double |
| `model` | text | none: refused | model | `SDC_LLM_MODEL_DECIDER` |
| `verdict` | `approve`, `reject` | none: refused | rule | the rule's answer |
| `without_claims` | true, false | false | model | the run's acknowledgement that Loom's output holds no claims; without it every decision is `CannotDecide` |
| `reply_bin` | path | the `sdc-reply` found on the runner's `PATH` | model | the `sdc-reply` binary; none found: refused ("decider.reply_bin: missing") |
| `program` | | the copy's `bindings/tools/decider.sh` | model, rule | set by the runner; a value in the file is refused |
| `adapter` | text | none | model | `SDC_DECIDER_ADAPTER` |
| `effort` | `low`, `medium`, `high`, `xhigh`, `max` | none | model | `SDC_LLM_EFFORT_DECIDER` |
| `max_turns` | whole number, 1 or more | none | model | `SDC_DECIDER_MAX_TURNS` |
| `max_budget_usd` | number above 0 | 0.25 | model | most one decider call may cost |
| `max_calls`, `max_decider_calls` | whole number, 1 or more | 20 | model | most decider model calls; the two names are one limit, give one |
| `escalate` | `final`, `stop` | `stop` | model | what an `escalate` reply does |
| `retries` | whole number | 2 | model | asks again after an invalid reply |
| `timeout_seconds` | whole number, 1 or more | 900 | model, rule | the program's whole process group is ended at the limit |
| `principal` | text | none | model, rule | accepted when it is what the kind gives (`decider:model`, `decider:approve`, `decider:reject`) and then taken out; another value is refused |
| `answers`, `max_call_micro_usd` | list, whole number | none | scripted | the double's answers and its reservation |

## What `sdc-experiment run` writes today, and what a run does with it

Unit R4 writes the file (`loom_settings_json`, commit `970db1f7`): `format`, `run`, `decider.principal`
(always `decider:model`), `decider.model`, `loom.sha256`, `loom.confinement`, `loom.cap_micro_usd`,
`loom.login_file` (only when the record names one), `models.<role>.model` for the four roles,
`max_llm_calls`, `max_wall_seconds`. It refuses these keys in a record of a loom variant:
`decider.kind`, `decider.escalate`, `decider.max_decider_calls`, `adapter`, `decider.adapter`,
`autonomy`, `implement_mode`, `efforts.*`, `flags.*`.

What `runbook loom-run` does with that file as it is (tests in `runbook/tests/loom_run/settings.rs`):

- It is read in full, and refused (exit 2, nothing written) naming `decider.kind`.
- With `decider.kind` added and nothing else, it starts, the model decider is built, and the first
  decision ends the run as `stopped: decider` ("no autonomy level named"): the decider answers only
  at `A5`.
- With `decider.kind`, `autonomy: "A5"`, `decider.without_claims: true` and a reachable `sdc-reply` it
  runs to the end (test `two_model_steps_and_an_approval_by_the_model_decider_end_delivered_with_the_run_directory`).

R4 must change:

1. Write `decider.kind` (`model` for the strong variant) and stop refusing it in a loom variant.
2. Write `autonomy` (`A5` for the strong variant) and stop refusing it.
3. Write `decider.without_claims: true` for a model decider (a setting the record must carry: the
   run makes no claim about claims it does not have).
4. Make `sdc-reply` reachable: write `decider.reply_bin`, or give the unit a `PATH` that holds it. The
   unit's `PATH` is the login's; `jq`, `yq` and `rsync` have to be on it too, for `decider.sh`.
5. Write, and stop refusing, the optional members the strong variant sets: `decider.escalate`
   (`final`), `decider.max_calls` or `decider.max_decider_calls` (40), `decider.adapter`
   (`claude-cli`), `adapter` (`metaharness`), `review.adapter` (`claude-cli`). This crate reads all of
   them.
6. `decider.principal` may stay (it is checked) or go (it is derived from the kind).
7. Efforts per role (`efforts.<role>.effort`) and `flags.*` have no setting here: `llm-node.sh` has no
   option for an effort under Loom, and the runner has no dry or recorded mode. R4's refusal of both is
   right.
