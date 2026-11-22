# The bindings in Loom's format (`bindings/`)

Step 1 of the move of Runbook onto Loom (`story:loom-bindings-files`): the nine bindings files of
`driver/examples/sdc/*.bindings.json` written again in Loom's `loom.bindings/1`, one file per
protocol under `bindings/`. The JSON files stay: `sdc-drive` still runs from them, and the
documentation site's data is still made from them. The YAML files are not run by anything yet.
Step 2 (`story:loom-tools-program-contract`) moved the tools both formats call to
`bindings/tools/` and gave them Loom's program contract beside the driver's ("Tool changes").

The format is the one Loom's loader reads at Loom commit `8a8fad3b` (`../loom`,
`crates/loom-bindings/src/decode.rs`, `crates/loom-bindings/src/load.rs`,
`ess/domains/bindings.yaml`). The differences between the two formats are numbered `D1` to `D39`
in the Loom session's comparison of the two formats (2026-11-21); this page cites those numbers.
What the format cannot express yet is listed in `bindings/PENDING.md`, never written in invented
syntax. The bindings ran under Loom commit `5af3f9c2` in the identical comparison with the driver
(`stub-cycle`, 2026-11-22; `docs/parity.md`, "Identical comparison on Loom `5af3f9c2`"), after the
first equal one at `2c0a6f48` ("First equal comparison").

## Files

| Protocol | YAML | Registration | Bindings | Unbound (as in the JSON) |
|---|---|---|---|---|
| `sdc` | `bindings/sdc.yaml` | `sdc@8` | 4 | none |
| `sdc.intake` | `bindings/sdc-intake.yaml` | `sdc-intake@9` | 10 | none |
| `sdc.direction` | `bindings/sdc-direction.yaml` | `sdc-direction@7` | 9 | `context.change` |
| `sdc.epic` | `bindings/sdc-epic.yaml` | `sdc-epic@5` | 6 | none |
| `sdc.spec` | `bindings/sdc-spec.yaml` | `sdc-spec@5` | 9 | `spec.reuse`, `spec.reconstruct` |
| `sdc.plan` | `bindings/sdc-plan.yaml` | `sdc-plan@7` | 8 | none |
| `sdc.story` | `bindings/sdc-story.yaml` | `sdc-story@7` | 7 | none |
| `sdc.release` | `bindings/sdc-release.yaml` | `sdc-release@4` | 5 | `release.rollback` |
| `sdc.bugfix` | `bindings/sdc-bugfix.yaml` | `sdc-bugfix@1` | 8 | none |

Every action bound in a JSON file is bound in its YAML file: 66 bindings.

## Field mapping

| JSON (`driver/examples/sdc/*.bindings.json`) | `loom.bindings/1` | D |
|---|---|---|
| one object keyed by action id | `actions:`, a list; each binding names its `action` | 1 |
| (none) | `format: loom.bindings/1` | 2 |
| (the protocol file given to `init --protocol`) | `protocol: <name>@<revision>`: the protocol id with `.` and `_` as `-`, then the revision of `protocol/<name>.protocol.yaml` (`sdc@8`), as Loom's catalog registers it | 3 |
| (none) | `program_root: bindings` on every file that binds a program: a program path with a `/` is relative to the directory of the bindings file | 13 |
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
| `{dir}/tools/<tool>` (since step 2 `{dir}/../../../bindings/tools/<tool>`) | `tools/<tool>`, relative to `bindings/` (`bindings/tools/`); no `..`, and a program without a `/` comes from `PATH`. The file names `program_root: bindings`, so Loom (at `af8de2c9`) reads the path beside the bindings file; `scripts/check-bindings.sh` refuses a file that binds a program without it | 13 |
| `{work}` in `handoff` | "of the workspace"; the placeholder is pending | 14 |
| `handoff` | `handoff` | 16 |
| `handoff_when` | pending | 17 |
| `llm: {model, max_turns, max_budget_usd, stub, operations}` | argv of `tools/llm-node.sh` since step 2: `--model`, `--max-turns`, `--max-budget-usd`, `--operations`, `--instructions <json>`, `--stub <argv>`; the `agent` executor is pending: a `program` executor takes no agent member, the agent text is in `bindings/PENDING.md` | 18-22 |
| `retry_when` | `retry_when` | 23 |
| `max_runs` | `budget: {max_runs}` | 24 |
| `timeout_s` | `timeout_seconds` (see "Timeouts") | 25 |
| `copy: {kind, from: sdc.direction}` | `executor: {kind: copy, evidence: <kind>, from: sdc-direction@7}`, the registration of `protocol/sdc-direction.protocol.yaml` | 26 |
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
| 1800 | `spec.gate` (epic, plan, story, release) | `ess-gate`: compile, synthesis, interpreted suite (`bindings/tools/sdc-tool.sh:239-244`) |
| 3000 | story `tests.run` | `story-tests` with `--pool-wait 1200`: wait 1200 + 2 pool jobs x 600 + 600 of other work; the limit covers the queue wait and the run (`docs/driver.md`, "The build pool wait") |
| 2400 | release `acceptance.run` | `conform-run release` with `--pool-wait 1200`: 1200 + 1 job x 600 + 600 |
| 1800 | spec `spec.compile`, `spec.synthesize`, `spec.trace`; plan `plan.cover` | `ess specify compile` or the interpreted suite (`sdc-tool.sh:246`, `:248`, `:418-427`; `sdc-findings` `gate spec` compiles, `driver/src/bin/sdc-findings.rs:590`) |
| 3000, 3000, 4200 | bugfix `repro.run`, `tests.run`, `gate.run` | `--pool-wait 1200` with 2, 2 and 4 pool jobs of 600 and 600 of other work; the limit covers the queue wait and the run (`docs/driver.md`, "The build pool wait") |
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
The bindings that wait for the build pool (story `tests.run`, release `acceptance.run`, bugfix `repro.run`, `tests.run`, `gate.run`) add exit 69 (no
slot within `--pool-wait`, nothing ran) to their list: `[69, 70, 75, 126, 127]` (70: the pool ended a started job at its run limit, "ran over its time"). Until then Loom maps 69 and 70
by the binding's `*` result to `failed`; the tool exits 69 and 70 in both modes.

## Checks

- `scripts/check-bindings.sh` (CI, `task check`): for each JSON file, its `bindings/<name>.yaml`
  exists, binds every action the JSON binds or `bindings/PENDING.md` has a line
  `` - `<name>.bindings.json:<line>` `<action>` ``, and binds no action the compiled protocol
  (`protocol/compiled/<name>.ir.json`) does not declare. Since step 2 also: no `executor.argv`
  or `arguments_from` entry holds a `..` segment, and every program named with a `/` is
  `tools/<file>`, an executable file under `bindings/tools/`; a program without a `/` comes from
  `PATH`. Also: each file's `protocol:` is the registration of `protocol/<name>.protocol.yaml`
  (the id with `.` and `_` as `-`, then `@<revision>`), the `from:` of every `copy` executor is the
  registration of a protocol file in `protocol/`, and a file that binds a program has
  `program_root: bindings`; a failure names the file and the value. A new revision of a protocol
  file moves its registration here and in every `copy` `from:`. Plain text with `jq` and `yq`.
- The tools' Loom mode: `driver/tests/questions_check.rs`, module `loom_mode`, runs a check
  (`check-questions`, `check-requirements`), `list-stories`, an agent node on the stub and on dry
  (`idea.dissect`, `spec.author`) and a retried writer (`questions.write` with
  `LOOM_RETRY_FEEDBACK`) twice: in driver mode and under Loom's contract (an empty environment but
  `PATH` and `LOOM_CASE`, the arguments on standard input, a fresh workspace), and compares exit
  code, output and the files written.
- The load check, with Loom's own loader, outside this repository (no Rust of Runbook's): a scratch
  binary crate that depends on `b10x-loom-bindings` by path in an export of Loom at `8a8fad3b`
  (`git -C ../loom archive 8a8fad3b Cargo.toml Cargo.lock crates generated protocols`, Loom's
  `Cargo.lock` copied beside the crate) and on `b10x-canon` tag `0.10.0` (the run of 2026-11-21
  below used `0.9.0`). Per file it parses
  `protocol/<name>.protocol.yaml` with `b10x_canon::model::parse`, then calls
  `b10x_loom_bindings::decode` on `bindings/<name>.yaml` and
  `b10x_loom_bindings::check(&document, "<name>@<revision>", &protocol)`, and prints `ok` or the refusal.
  Build it through `slot` (`cargo build --release`; a cold build took 184 s, 44 s of it queued).
  On 2026-11-21 all nine files printed `ok`, at `8a8fad3b` and, after step 2, at Loom's
  integration head `882c1866` (which has no `resolve_programs`, so only the load is checked). Loom's `b10x-loom bindings check <file> --protocol
  <name@major>` (N2, wave 7) replaces it; `b10x-loom` 0.4.0 has no such command.

## Tool changes for Loom's program contract

Step 2 (`story:loom-tools-program-contract`). The tools moved from `driver/examples/sdc/tools/` to
`bindings/tools/`, beside the files that name them: `bindings/*.yaml` names each as
`tools/<file>`, and `driver/examples/sdc/*.bindings.json` as `{dir}/../../../bindings/tools/<file>`.
Each tool has two modes. With `LOOM_CASE` unset it runs as before, for `sdc-drive`. With
`LOOM_CASE` set it reads Loom's program contract: the arguments as one JSON object on standard
input, the case id from `LOOM_CASE`, the epic and the story from `LOOM_PARENT_CASE_NAME` and `LOOM_CASE_NAME` (the names Loom gave the cases; without them the cut of the case id), retry feedback from the `loom.retry-feedback/1` document that
`LOOM_RETRY_FEEDBACK` names, the start directory at the workspace root, and everything the driver
gave in a variable as an argv entry of the binding (`sdc-tool.sh <sub> ... --action <id>`;
`llm-node.sh <action> --model ... --max-turns ... --max-budget-usd ... [--operations ...] [--instructions <json>] --stub
<argv>`). The tools map these inputs onto the driver's variables and export them, so the programs
they start (`sdc-findings`, `sdc-bugfix`, `sdc-reply`, the adapters) run unchanged. The driver
crate's programs come from `PATH`, else from `driver/target/debug/` of this checkout, found from
the tool's own directory; never from `HOME`. A bad argv entry exits 64.

Status per read: done, or pending with the Loom story in `bindings/PENDING.md` ("Tool environment
under Loom"); a pending read takes the driver's default under Loom. The first column is a string to `grep -n` for in the tool: line numbers of a tool that changes go stale.

`bindings/tools/sdc-tool.sh`

| Marker | Reads | Under Loom | Status |
|---|---|---|---|
| `cat > /dev/null` | standard input | read and dropped: every subcommand takes its arguments from argv | done |
| `SDC_FINDINGS:-` | `SDC_FINDINGS`, else `driver/target/debug/` of this checkout | `sdc-findings` on `PATH`, else relative to the tool; no `HOME`; not found: exit 127, the message names the binary and where it looked | done |
| `SDC_BUGFIX:-` | `SDC_BUGFIX` | `sdc-bugfix` the same way | done |
| `executor=agent:` | `SDC_ACTION` (the `aep` executor `agent:<action>`) | `--action <id>`, in every `sdc-tool.sh` argv of `bindings/*.yaml` | done |
| `aep_actor=` | `SDC_PRINCIPAL` (`AEP_ACTOR`) | `--principal <principal>` is taken; no binding states one | pending: `story:bound-program-declares-its-environment` (wave 9) or G2 |
| `stub-file)` | `SDC_STUB_FIXTURES` (`stub-file`) | `--stub-fixtures <dir>` is taken | pending: N3 (the stub overlay's binding states it) |
| `SDC_STUB_COUNTER:-` | `SDC_STUB_COUNTER` | `--stub-counter <dir>` is taken (default `.sdc`) | pending: N3 |
| `stub-tree)` | `SDC_STUB_FIXTURES` (`stub-tree`) | as for `stub-file` | pending: N3 |
| `SDC_POOL_WAIT` | `SDC_POOL_WAIT` (the step's wait bound for the build pool) | `--pool-wait <seconds>`, in the argv of story `tests.run`, release `acceptance.run` and bugfix `repro.run`, `tests.run`, `gate.run` of `bindings/*.yaml`; the environment variable wins | done |
| `report-$SDC_ACTION` | `SDC_ACTION` (report file name) | `--action` | done |
| `impl-gate)`, `story=$(cut` | `SDC_CASE` (`impl-gate`: the story id) | `LOOM_CASE`; the story is `LOOM_CASE_NAME` when Loom sets it (`LOOM_PARENT_CASE_NAME` set: its own name; only `LOOM_CASE_NAME`: an epic case, no story) | done |
| `list-stories)`, `epic=$(cut` | `SDC_CASE` (`list-stories`: the epic) | `LOOM_PARENT_CASE_NAME`, else `LOOM_CASE_NAME` (an epic case), else the cut of `LOOM_CASE`; given to `arguments_from` too | done |
| `scope-answer)` | `$2`, the `{principal}` argument (`scope-answer`) | argv from G2 | pending: G2 (wave 3) |

`bindings/tools/llm-node.sh`

| Marker | Reads | Under Loom | Status |
|---|---|---|---|
| `SDC_ARGS` | `SDC_ARGS` (`spec.author`'s `domain`) | standard input, one JSON object (empty input is `{}`) | done |
| `SDC_LLM_NODE` | `SDC_LLM_NODE` (the binding's `llm` object) | `--model`, `--max-turns`, `--max-budget-usd`, `--operations`, `--instructions <json>`, `--stub <argv>` | done, until the `agent` executor (`story:bound-agent-node`) |
| `SDC_REPLY` | `SDC_REPLY` | `sdc-reply` on `PATH`, else relative to the tool | done |
| `SDC_FEEDBACK`, `LOOM_RETRY_FEEDBACK` | `SDC_FEEDBACK` (failing records and their logs) | `LOOM_RETRY_FEEDBACK`, written in the driver's form to `work/.sdc/loom-feedback/<digest>/`: each record's kind, result, subject and log tail | done; the records name no action and no exit, so `in/feedback.md` reads `(from ?, exit ?)` |
| `not_called()` | `SDC_NOT_CALLED` (the not-called file) | the reason on standard output, exit 0, as in the driver | pending: G1 (`exits.not_called`) |
| `SDC_PRINCIPAL` | `SDC_PRINCIPAL` | `--principal` is taken; no binding states one | pending: `story:bound-program-declares-its-environment` or G2 |
| `SDC_LLM_ADAPTER` | `SDC_LLM_ADAPTER` | `--adapter` is taken; the YAML names none, so the stub runs | pending: N3, N9 |
| `SDC_CLAUDE_CREDENTIALS` | `SDC_CLAUDE_CREDENTIALS` (the login file of `llm-adapter-claude-cli.sh` and of `llm-adapter-metaharness.sh`; else `CLAUDE_CONFIG_DIR`, else `HOME`) | `--claude-credentials <file>`; with none of the four (`ANTHROPIC_API_KEY` too) the claude-cli adapter exits 127 and names them | done; the runner writes it into every model node (`docs/loom-run.md`, "How each adapter gets the login under Loom") |
| `SDC_IMPLEMENT_MODE`, `SDC_REVIEW_MODE`, `SDC_IMPLEMENT_COVERAGE` | `SDC_IMPLEMENT_MODE`, `SDC_REVIEW_MODE`, `SDC_IMPLEMENT_COVERAGE` | the defaults | pending: N9 |
| `SDC_LLM_NODES` | `SDC_LLM_NODES` | the default: every node on the adapter | pending: N9 |
| `SDC_STUB_FINDINGS` | `SDC_STUB_FINDINGS` | the default: none | pending: N3 |
| `SDC_FINDINGS:-` | `SDC_FINDINGS`, else `driver/target/debug/` of this checkout | as in `sdc-tool.sh` | done |
| `cd_=${SDC_CASE_DIR:-}`, `events.jsonl` | `SDC_CASE_DIR` (`events.jsonl`: a rejection no longer current) | does not run: `LOOM_RETRY_FEEDBACK` holds only records about the current revision | pending: `story:bound-agent-node` |
| `A=$(jq -r '.approval` | `SDC_CASE_DIR` (`bindings.json`: the rejected subject's path) | `artifacts` of the `bindings/*.yaml` file that binds the rejecting approval | done |
| `SDC_LLM_RECORD`, `SDC_LLM_LOG` | `SDC_LLM_RECORD`, `SDC_LLM_LOG` | the default `work/.sdc/llm/` | pending: `story:agent-node-run-records` |
| `epic=$(cut`, `story=$(cut` | `SDC_CASE` (epic and story ids) | `LOOM_PARENT_CASE_NAME` (epic) and `LOOM_CASE_NAME` (story) when Loom sets them, else the cut of `LOOM_CASE` | done |
| `SDC_CASE_DIR`, `case_dir=` | `SDC_CASE_DIR` (`change.review` inputs, the request record, implement request mode) | the directory above the workspace | pending: `story:bound-agent-node` |
| `SDC_RUN_SETTINGS` | `SDC_RUN_SETTINGS` | the default: no rework limit | pending: N9 |
| `SDC_LLM_MODEL`, `SDC_LLM_EFFORT` | `SDC_LLM_MODEL[_<CLASS>]`, `SDC_LLM_EFFORT[_<CLASS>]` | the binding's `--model`; the class's default effort | pending: N9 |
| `SDC_REVIEW_RETRIES` | `SDC_REVIEW_RETRIES` | the default 2 | pending: N9 |

`driver/src/bin/*.rs` called through the tools

| Marker | Reads | Under Loom | Status |
|---|---|---|---|
| `sdc-bugfix.rs:746`, `:786` | `SDC_ACTION` (`story.update`'s `aep` actor) | exported by `sdc-tool.sh` from `--action` | done |
| `sdc-bugfix.rs:1382` | `PATH` | unchanged: Loom gives `PATH` | done |
| `sdc-findings.rs:1435` | `SDC_STUB_REFUSE_DECISION` (stub cycles) | the default: none | pending: N3 |
| `sdc-findings.rs:1552` | `SDC_CONSISTENCY_RULES` | unchanged: both tools pass `--rules` | done |
| `sdc-reply.rs:310-314` | `SDC_<CLASS>_ADAPTER`, `SDC_LLM_ADAPTER` | `llm-node.sh` exports `--adapter`; the class adapters take their defaults | pending: N9 |

Variables beyond `PATH` read under `set -u` (checked 2026-11-22, `story:tool-texts-and-paths`):

| Where | Reads | What was done |
|---|---|---|
| `llm-adapter-claude-cli.sh` | `HOME` (the login under `~/.claude`) | read as `${HOME:-}`; the login is `SDC_CLAUDE_CREDENTIALS`, else `CLAUDE_CONFIG_DIR`, else `HOME`, and with none of them (and no `ANTHROPIC_API_KEY`) the adapter exits 127 naming all four; llm-node's `--claude-credentials <file>` gives the first one |
| `llm-adapter-metaharness.sh`, `llm-adapter-dry.sh`, `llm-adapter-fixture.sh` | `METAHARNESS`, `SDC_LLM_ENDPOINT`, `SDC_LLM_DRY_LOG`, `SDC_COVERAGE_REPLIES`, `SDC_REPLY`, and for the metaharness adapter `SDC_CLAUDE_CREDENTIALS` | all read with a default (`${X:-}`); the metaharness adapter, when `HOME` has no login and the login's directory is the runner's private one (mode 700), sets `HOME` and `TMPDIR` to a home in that directory with a copy of the login for the metaharness process alone, removed on exit (the runner removes what a SIGKILL left) |
| `sdc-tool.sh` `conform-run` | `SDC_ACTION` (the report file name) | read as `${SDC_ACTION:-conform-run}`; the binding states `--action` anyway |
| `sdc-tool.sh` `ess` calls | `ESS_TOOLCHAIN`, `ESS_TOOLCHAIN_QUIET` | exported with defaults at the top of the tool: nothing to change |
| `sdc-tool.sh` `ess` calls | `ESS_TOOLCHAIN_DIR`, else `XDG_CACHE_HOME`, else `HOME` (the 0.97.1 and later dispatcher finds and installs the toolchain below it, 0.97.3 since 2026-11-22, 0.96.0 before) | with none of the three set, `spec-validate` exited 1 (`neither XDG_CACHE_HOME nor HOME is set; set ESS_TOOLCHAIN_DIR`); in Loom mode the tool now exports `ESS_TOOLCHAIN_DIR=<home>/.cache/ess/toolchains`, `<home>` from `getent passwd "$(id -u)"`, when none of the three is set; `scripts/check-bindings.sh` runs `spec-validate` under `env -i PATH LOOM_CASE` |
| `sdc-tool.sh` `slot task` (`conform-run`), `slot cargo test` (`impl-gate`) | none: `PWD` only | nothing to change |
| `ess`, `slot`, `cargo`, `aep`, `claude`, `jq`, `yq` | `HOME` | started with `env -i PATH=...` they answer `--version` (`slot status` too): they take the home directory from the passwd entry; a build through `cargo` without `HOME` was not run (*inferred*). `ess` is the exception once it hands off to a toolchain (row above). `aep` was run under `env -i PATH=...` for `plan artifact new`, `list`, `move`, `validate --strict`: it opens no file under the home directory and needs nothing |

A host whose user has no passwd entry still needs `story:bound-program-declares-its-environment`
(wave 9) or `--confinement local`.

### Where Loom's head differs from this contract

Read at Loom's integration head `882c1866` (`wave/2026-11-21-w7`) and the branches named.

- `LOOM_CASE` (`story:bound-program-case-id`) and `LOOM_RETRY_FEEDBACK`
  (`story:bound-program-retry-records`) are on Loom's branches `impl/bound-program-case-id` and
  `impl/bound-program-retry-records`, not in the head. At the head a program gets `PATH` alone.
- The head had no `program_root` and no `resolve_programs` (*inferred* for the decoder: it refuses a
  member it does not declare): a relative program path was a workspace path, so
  `tools/sdc-tool.sh` was looked for in the workspace. Loom at `af8de2c9` decodes
  `program_root` (`workspace` or `bindings`, `crates/loom-bindings/src/decode.rs:122-134`), and
  every bindings file here that binds a program names `program_root: bindings`. No Loom ref has a
  reserved-action-id check.
- A retry record names its evidence id, kind, subject, subject revision, result, producer, time
  and blob; not the action that produced it nor its exit. A log is the last 60 lines, at most
  64 KiB (`llm-node.sh` quotes the last 80 lines of a driver log). Only records about the subject's
  current revision are given; the variable is unset when there are none and never set for
  `arguments_from`.
- A child case id is `<parent id>.<name>`, as in the driver. Loom names a repeated child
  `<id>.2`; the tools cut the epic and the story from the second and third dot-separated fields,
  so a story under a repeated epic would read `2` as its story.
- At the head a step that placed an artifact at a path takes the content digest of that path as
  the artifact's revision (`impl/bound-artifact-revisions`, merged); the tools need no change for it.
