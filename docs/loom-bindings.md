# The bindings in Loom's format (`bindings/`)

Step 1 of the move of Runbook onto Loom (`story:loom-bindings-files`): the nine bindings files of
`driver/examples/sdc/*.bindings.json` written again in Loom's `loom.bindings/1`, one file per
protocol under `bindings/`. The JSON files stay as they are: `sdc-drive` still runs from them, and
the documentation site's data is still made from them. The YAML files are not run by anything yet.

The format is the one Loom's loader reads at Loom commit `8a8fad3b` (`../loom`,
`crates/loom-bindings/src/decode.rs`, `crates/loom-bindings/src/load.rs`,
`ess/domains/bindings.yaml`). The differences between the two formats are numbered `D1` to `D39`
in the Loom session's comparison of the two formats (2026-11-21); this page cites those numbers.
What the format cannot express yet is listed in `bindings/PENDING.md`, never written in invented
syntax.

## Files

| Protocol | YAML | Registration | Bindings | Unbound (as in the JSON) |
|---|---|---|---|---|
| `sdc` | `bindings/sdc.yaml` | `sdc@1` | 4 | none |
| `sdc.intake` | `bindings/sdc-intake.yaml` | `sdc-intake@1` | 10 | none |
| `sdc.direction` | `bindings/sdc-direction.yaml` | `sdc-direction@1` | 9 | `context.change` |
| `sdc.epic` | `bindings/sdc-epic.yaml` | `sdc-epic@1` | 6 | none |
| `sdc.spec` | `bindings/sdc-spec.yaml` | `sdc-spec@1` | 9 | `spec.reuse`, `spec.reconstruct` |
| `sdc.plan` | `bindings/sdc-plan.yaml` | `sdc-plan@1` | 8 | none |
| `sdc.story` | `bindings/sdc-story.yaml` | `sdc-story@1` | 7 | none |
| `sdc.release` | `bindings/sdc-release.yaml` | `sdc-release@1` | 5 | `release.rollback` |
| `sdc.bugfix` | `bindings/sdc-bugfix.yaml` | `sdc-bugfix@1` | 8 | none |

Every action bound in a JSON file is bound in its YAML file: 66 bindings.

## Field mapping

| JSON (`driver/examples/sdc/*.bindings.json`) | `loom.bindings/1` | D |
|---|---|---|
| one object keyed by action id | `actions:`, a list; each binding names its `action` | 1 |
| (none) | `format: loom.bindings/1` | 2 |
| (the protocol file given to `init --protocol`) | `protocol: <name>@1`, the protocol id with `.` as `-` | 3 |
| `$artifacts` | `artifacts` | 4 |
| `$independence` | `independence` | 5 |
| `argv` | `executor: {kind: program, argv}` | 6 |
| `produces`, `subject`, `producer`, `upstream`, `results` on the binding | `evidence:`, one entry per kind with `kind`, `subject`, `producer`, `from`, `results`, `upstream` | 6 |
| `produces: [a, b]` with `results` exit → `{a: r, b: s}` (`questions.check`) | two entries over the same exit, each with its own table | 7 |
| a table keyed by exit code | `from: exit` | 8 |
| `producer: "{principal}"` | `producer: person:owner` (the approving principal replaces it at run time, `story:tree-run-command`) | 9 |
| `manual: true` | `executor: {kind: person, principal: person:owner}` | 10 |
| `manual: true` with `argv: ["true"]` | dropped: `true` does nothing; a `person` executor runs no program | 11 |
| `"6": "rejected"` on a manual binding | pending (`bindings/PENDING.md`) | 12 |
| `{dir}/tools/<tool>` | `../driver/examples/sdc/tools/<tool>`, relative to `bindings/`; the base is pending (G1) | 13 |
| `{work}` in `handoff` | "of the workspace"; the placeholder is pending | 14 |
| `handoff` | `handoff` | 16 |
| `handoff_when` | pending | 17 |
| `llm: {model, max_turns, max_budget_usd, stub, operations}` | pending: a `program` executor takes no agent member; the agent text is in `bindings/PENDING.md` | 18-22 |
| `retry_when` | `retry_when` | 23 |
| `max_runs` | `budget: {max_runs}` | 24 |
| `timeout_s` | `timeout_seconds` (see "Timeouts") | 25 |
| `copy: {kind, from: sdc.direction}` | `executor: {kind: copy, evidence: <kind>, from: sdc-direction@1}` | 26 |
| `child: {name \| each, bindings}` | `executor: {kind: open, name \| each, bindings: <file>.yaml}` | 27, 29 |
| `child.protocol` (a file path) | dropped: the child's protocol is the name the action's `case_open.protocol` declares (`protocol/sdc.protocol.yaml:96-98`), resolved in the catalog `tree open --protocols <dir>` builds from the `*.yaml` files of that directory (Loom wave 2); the loader at `8a8fad3b` asks for nothing in its place | 28 |
| `child.revisions` | pending | 30 |
| opening `results: {"0": null}` | `from: effect`, `results: {performed: null}` | 31 |
| `args_from` | `arguments_from` | 32 |
| a manual binding's `subject` with no `produces` (`scope.answer`) | pending | 38 |

The JSON files hold no `arguments` and nothing else that is the same in both formats.

## Timeouts

Loom's default is 60 s (`../loom/crates/loom-intake-slice/src/bindings.rs:69`); the driver's is
1800 s (`driver/src/main.rs:1508`). Each binding with a `timeout_s` keeps its value. A binding
without one gets `timeout_seconds: 1800`, today's effective value, when its program calls `ess`
beyond `ess specify validate`, `cargo` or `slot` (a compile, a synthesis, a test suite or a
build). The others are checks over files with `yq`, `jq`, `aep`, `grep`, `cp`, `cmp` or an
`sdc-findings` or `sdc-bugfix` check, which end in seconds, and keep Loom's 60 s. A `person`,
`open` or `copy` binding runs no program and names none; an `arguments_from` argv (`list-epics`,
`list-stories`, `spec-domain`: `aep` or `yq` over files) runs under its binding's timeout.

| Value | Bindings | Why |
|---|---|---|
| 1800 | `spec.gate` (epic, plan, story, release) | `ess-gate`: compile, synthesis, interpreted suite (`driver/examples/sdc/tools/sdc-tool.sh:191-196`) |
| 1800 | story `tests.run` | `story-tests`: conformance run and `slot task check` (`sdc-tool.sh:207-246`, `:380-382`) |
| 1800 | release `acceptance.run` | `conform-run release` (`sdc-tool.sh:207-233`) |
| 1800 | spec `spec.compile`, `spec.synthesize`, `spec.trace`; plan `plan.cover` | `ess specify compile` or the interpreted suite (`sdc-tool.sh:198`, `:200`, `:340-346`; `sdc-findings` `gate spec` compiles, `driver/src/bin/sdc-findings.rs:590`) |
| 1800 | bugfix `repro.run`, `tests.run`, `gate.run` | `cargo` builds and tests (`driver/src/bin/sdc-bugfix.rs:609-613`) |
| 1500 | spec `spec.author`, story `change.implement`, bugfix `change.fix` | `timeout_s` of the JSON; agent nodes |
| 900 | the other agent nodes: intake 4, direction 3, plan `plan.decompose`, story `change.review`, bugfix `test.reproduce` | `timeout_s` of the JSON |
| 600 | bugfix `bug.dissect` | `timeout_s` of the JSON |
| 300 | every `amend.upstream` (intake, direction, spec, plan, story) | `timeout_s` of the JSON |
| 60 (default) | intake `idea.check`, `dissection.check`, `questions.check`, `requirements.check`; direction `context.check`, `roadmap.validate`; spec `spec.validate`, `spec.scan`, `spec.review`; plan `plan.validate`, `plan.scope`, `plan.status`; story `change.merge`; release `release.build`, `release.publish`, `release.observe`; bugfix `dissection.check`, `story.update` | checks over files and copies |

## Written in the YAML, carried out by Loom later

These constructs load today; Loom runs them once the story named lands (waves as in Runbook's
proposed order of Loom's open stories, 2026-11-21).

- `copy` (`roadmap.read` in `sdc`, `sdc-epic`, `sdc-spec`, `sdc-plan`): `story:record-copy-between-cases`, wave 1. `from` is the registration, as in Loom's guide (`../loom/website/docs/guides/bindings.md:138-142`).
- `open` with its `from: effect` entries, the child's `independence`: `story:child-case-runtime`, wave 1.
- `arguments_from` on an `open` binding (`epics.open`, `stories.open`): `story:tree-run-command`, wave 2.
- `retry_when` and the feedback to the writer (`LOOM_RETRY_FEEDBACK`, `loom.retry-feedback/1`): the retry-records story, wave 1; a document with a `retry_when` is refused by the cut `tree-run-command` until wave 2 lifts it.
- `handoff`: `story:tree-run-command`, wave 2.
- A `person`'s record produced by the approving principal, not the literal `person:owner`: `story:tree-run-command`, wave 2.
- `budget.max_runs`, counted since the case's last arrival as the driver does (D24): `story:run-caps-per-arrival`, wave 6.

## Pending (`bindings/PENDING.md`)

| Loom work | Wave | What it brings |
|---|---|---|
| G1 bound program contract | 2 | argv relative to the bindings file's directory (D13), program-error exits 75, 126, 127 per binding (D36), not-called runs (D37) |
| G2 person actions | 3 | a program on a person binding with the principal as argument (D11, D15), `rejected` with reasons (D12), a subject with no evidence (D38) |
| `story:bound-agent-node` | 4 | `agent` executors for the 14 agent nodes: adapter, model, `max_turns`, `max_budget_micro_usd`, `tools.allow` (D18-D20, D22) |
| N8 | 4 | `{work}` in `handoff` (D14) |
| N3 fixture adapter | 5 | the stub of each agent node (D21) |
| G3 | 8 | `handoff_when` (D17) |
| G4 | 10 | initial child revisions (D30), only if step 3 shows `sdc.epic` needs them |

The exit lists: every program binding states program-error exits `[75, 126, 127]` (today's
`program_error`, `driver/src/main.rs:579-581`), and each of the 14 agent nodes a not-called exit,
which `llm-node.sh` takes in step 2 in place of the `SDC_NOT_CALLED` file. Loom ships no default
list (Loom story G1a); until the loader has the member, the lists stay in `bindings/PENDING.md`.

## Checks

- `scripts/check-bindings.sh` (CI, `task check`): for each JSON file, its `bindings/<name>.yaml`
  exists, binds every action the JSON binds or `bindings/PENDING.md` has a line
  `` - `<name>.bindings.json:<line>` `<action>` ``, and binds no action the compiled protocol
  (`protocol/compiled/<name>.ir.json`) does not declare. Plain text with `jq` and `yq`.
- The load check, with Loom's own loader, outside this repository (no Rust of Runbook's): a scratch
  binary crate that depends on `b10x-loom-bindings` by path in an export of Loom at `8a8fad3b`
  (`git -C ../loom archive 8a8fad3b Cargo.toml Cargo.lock crates generated protocols`, Loom's
  `Cargo.lock` copied beside the crate) and on `b10x-canon` tag `0.9.0`. Per file it parses
  `protocol/<name>.protocol.yaml` with `b10x_canon::model::parse`, then calls
  `b10x_loom_bindings::decode` on `bindings/<name>.yaml` and
  `b10x_loom_bindings::check(&document, "<name>@1", &protocol)`, and prints `ok` or the refusal.
  Build it through `slot` (`cargo build --release`; a cold build took 184 s, 44 s of it queued).
  On 2026-11-21 all nine files printed `ok`. Loom's `b10x-loom bindings check <file> --protocol
  <name@major>` (N2, wave 7) replaces it; `b10x-loom` 0.4.0 has no such command.

## Tool changes for Loom's program contract

Step 2, not done here. Under Loom a bound program gets its arguments as one JSON object on
standard input, `PATH` alone in its environment, the case id as `LOOM_CASE`, retry feedback as the
blob `LOOM_RETRY_FEEDBACK` names (`loom.retry-feedback/1`), and its start directory at the
workspace root; its result is read through the binding's result table. No variable names the case
store. Each read of a variable the driver sets (`driver/src/main.rs:1479-1514`) or the run sets
(`scripts/run-trial.sh`) changes as below.

`driver/examples/sdc/tools/sdc-tool.sh`

| Line | Reads | Under Loom |
|---|---|---|
| 27-28 | `SDC_FINDINGS`, else `$HOME/beyond10x/sdc/...` | `sdc-findings` on `PATH`, or its path relative to the tool (`$here`); no `HOME` |
| 33 | `SDC_BUGFIX` | `sdc-bugfix` on `PATH`, or relative to `$here` |
| 77 | `SDC_ACTION` (the `aep` executor `agent:<action>`) | the action as an argv entry of the binding (`sdc-tool.sh <sub> --action <id>`) |
| 78 | `SDC_PRINCIPAL` (`AEP_ACTOR`) | a literal of the binding (`story:bound-program-declares-its-environment`, wave 9) or argv |
| 88-94 | `SDC_STUB_FIXTURES` (`stub-file`) | the fixture directory as argv of the stub overlay's binding; later N3 |
| 91, 103 | `SDC_STUB_COUNTER` | argv of the stub overlay's binding (default `.sdc`) |
| 102-106 | `SDC_STUB_FIXTURES` (`stub-tree`) | as for `stub-file` |
| 213, 224 | `SDC_ACTION` (report file name) | the action as argv |
| 258, 266 | `SDC_CASE` (`impl-gate`: the story id) | `LOOM_CASE` |
| 330 | `SDC_CASE` (`list-stories`: the epic, read under `set -u`) | `LOOM_CASE`, given to `arguments_from` too |
| 402 | `$2`, the `{principal}` argument (`scope-answer`) | argv from G2 |

`driver/examples/sdc/tools/llm-node.sh`

| Line | Reads | Under Loom |
|---|---|---|
| 36 | `SDC_NOT_CALLED` (the not-called file) | exit with the binding's not-called code (G1) |
| 48 | `SDC_PRINCIPAL` | literal or argv, as in `sdc-tool.sh` |
| 56, 140-141, 190, 494-500 | `SDC_FEEDBACK` (failing records and their logs) | `LOOM_RETRY_FEEDBACK` |
| 59, 75-77, 566 | `SDC_LLM_NODE` (the binding's `llm` object: model, stub, operations) | argv of the binding (`llm-node.sh <action> --stub ... --model ...`) until the `agent` executor |
| 60 | `SDC_LLM_ADAPTER` | argv of the binding (the stub overlay names `stub`); later the engine file (N9) |
| 65, 402 | `SDC_REPLY` | `sdc-reply` on `PATH`, or relative to `$here` |
| 70 | `SDC_LLM_NODES` | argv or the engine file (N9) |
| 104-105 | `SDC_STUB_FINDINGS` | argv of the stub overlay's binding; later N3 |
| 124-125 | `SDC_FINDINGS`, else `$HOME/...` | as in `sdc-tool.sh` |
| 152-154 | `SDC_CASE_DIR` (`events.jsonl`: the last rejection) | `LOOM_RETRY_FEEDBACK`, which holds the rejected record |
| 170, 404 | `SDC_CASE_DIR` (`bindings.json`: the subject's path) | the path as argv, or read from `bindings/<protocol>.yaml` `artifacts` |
| 211, 591 | `SDC_LLM_RECORD`, `SDC_LLM_LOG` | argv; later Loom's run log (`agent-node-run-records`) |
| 229-230, 571, 582 | `SDC_CASE` (epic and story ids) | `LOOM_CASE` |
| 350 | `SDC_ARGS` (`spec.author`'s `domain`) | standard input |
| 528 | `SDC_RUN_SETTINGS` | argv; later the engine file (N9) |
| 565-568 | `SDC_LLM_MODEL[_<CLASS>]`, `SDC_LLM_EFFORT[_<CLASS>]` | argv; later the engine file (N9) |
| 571 | `SDC_CASE_DIR` (in the request record) | dropped: no store-directory variable |
| 628 | `SDC_REVIEW_RETRIES` | argv |

`driver/src/bin/*.rs` called by a binding (through `sdc-tool.sh`)

| Line | Reads | Under Loom |
|---|---|---|
| `sdc-bugfix.rs:416`, `:456` | `SDC_ACTION` (`story.update`'s `aep` actor) | the action as argv |
| `sdc-bugfix.rs:599` | `PATH` | unchanged: Loom gives `PATH` |
| `sdc-findings.rs:1435` | `SDC_STUB_REFUSE_DECISION` (stub cycles) | argv of the stub overlay's binding |
| `sdc-findings.rs:1552` | `SDC_CONSISTENCY_RULES` | unchanged: `sdc-tool.sh` passes `--rules` (`sdc-tool.sh:29`) |
| `sdc-reply.rs:709` | `SDC_<CLASS>_ADAPTER` (`adapters`, from `llm-node.sh:67` for `change.review`) | argv; later the engine file (N9) |

`HOME` is also read by `cargo`, `slot` and `aep` that the tools start; under `PATH` alone they
need `story:bound-program-declares-its-environment` (wave 9) or `--confinement local`.
