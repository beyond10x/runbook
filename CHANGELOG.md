# Changelog

All notable changes to this repository are recorded here. One version covers the repository
(`sdc-tools`, `sdc-watch`, `sdc-goals`, `runbook`, `ui/`, the plugin manifests), released under bare-version
tags such as `0.1.0`. The crates are `publish = false`; releases are source releases at
bare-version tags (`docs/releasing.md`).

## [Unreleased]

### Added

- `runbook run` decides once, after the bindings copy is made and before the first `tree run`, whether the run has a model node (`story:run-without-model-node-needs-no-caps`): a binding that names `sdc-node` (any form, a wrapper included), a decider that calls a model, `--recorded` or `--conformance observe`; a form it cannot read counts as one. The decision is the event `model_node_decided` and `has_model_node` in `outcome.json`, and it decides one thing: the link to the login file and its private directory are made only for a run that has a model node. It limits nothing and refuses nothing (`docs/loom-run.md`, "A run without a model node"). The page states the limit that remains: a bound program the author wrote that starts a model program by a name the copy does not hold is not seen; it gets no login and no token, and a call it records is booked and counted like any recorded call.

### Changed

- Breaking: `runbook run` has no money limit built in (the operator: "runbook should not have any limitations in terms of money cap - there is basically no technical limitation at all"). `--spend-ledger`, `--cap-micro-usd` and `--max-llm-calls` are each optional, alone and together, for every run, under `--confinement local` and `substrate`, with or without a model node, with `--dry` and in the library; a run that names none opens no ledger, refuses nothing and applies no limit. A named cap stops the run when reached, as before (exit 4 for the spend cap, exit 6 for the call cap), and a named 0 means spend nothing: the run starts and a model program that may spend is stopped before its first call. `--cap-micro-usd` without `--spend-ledger` is counted in the run itself, no file is written; `--spend-ledger` without a cap records and stops nothing. Gone, and a script that read them reads nothing now: the usage error (exit 64) for a missing `--cap-micro-usd`, `--max-llm-calls` or `--spend-ledger` of a run with a model node, the refusal by name of a run without a model node under `--confinement local` without caps (exit 2, `NO_NETWORK_LEVELS`), and `stopped: model call` (exit 13, cause `model_call_without_model_node`, the event `cap_reached` with cap `model_node`) with the checks that ended a run by it (the bindings copy read again, the tree manifest, a recorded call, the model decider, `--conformance observe`). In a run opened without a model node, with a spend cap or a call cap named, the printed command of a handoff still takes one step and the caps are read between two steps; with none named it runs as printed, and in a run opened with a model node it runs as printed under a named cap too (the bound for it is the story `handoff-under-a-named-cap-runs-one-step-at-a-time`). A call of unknown cost in a run that names no spend cap books nothing and is recorded with `cost_unknown` true and `cost_usd` null. A settings member `cap_micro_usd` or `max_llm_calls` without its option names the cap, and beside its option must still equal it. The decider has no built-in limits: a decider that names no `decider.max_calls` or `decider.max_budget_usd` is not limited (the old defaults were 20 calls and 0.25 USD per call); a named one holds as before. `--max-wall-seconds` stays required and unchanged. `docs/loom-run.md`, "The money options".
- Breaking: the settings file of `runbook run` is TOML and each setting has one place (story `run-settings-are-toml`). To convert a file: write it as TOML, take the members out of the `loom` object to the top level (`loom.sha256` is `loom_sha256`, the others keep their name: `in_process`, `confinement`, `cap_micro_usd`, `login_file`) and leave `decider`, `models.<role>`, `efforts.<role>` and `review` as tables. `--settings` reads TOML only: a file that is JSON is refused by its content before anything starts (exit 2), with a message that names the TOML form and `docs/loom-run-settings.md`, and a member in its old place is refused with the new place named. A whole number is an integer: a float, a text, a date and an integer over 64 bits are refused for it. `sdc-experiment run` writes `settings.toml` (its `--print` shows the file after `settings:`; the experiment records keep their keys). A syntax error of the file is refused with its line, its column and the parser's own words, and never with a line or a value of the file (it can hold the login's path), and a JSON file is refused as JSON also after a byte order mark or comment lines. A whole number may be written as TOML writes an integer (`0x10` is 16, `1_000` is 1000, `+5` is 5), and a member that repeats an option must equal it. The records of a run, `settings.json` and `run-settings.jsonl`, keep their form, their members and their values; a reader of the records reads by member name, and the order of members follows the settings file (for a file `sdc-experiment` wrote, the order differs from 0.7.0) (`docs/loom-run-settings.md` has the table of places and one complete example).
- The test step of `driver/` pays each expensive step once: the `decision_request` shape tests read the committed `ess/generated/schema` instead of running `ess generate`, the conformance tests resolve the pinned `ess` once per test and compile a fixture once per directory; `sdc-conform` and `sdc-conform-adapter` poll a child within at most 5 ms (the bounded commands) and 50 ms (the capture and the readiness request) in place of 100 ms, and `sdc-conform-adapter` decides a capture whose stream ended without the value by what the product does within the capture's `timeout_s`: it exits (`product_not_started`), it answers `start.ready` (`capture_missing`), neither (the capture did not appear within `timeout_s`). The adapter reads a product's output lines as bytes, so a line that is not UTF-8 no longer ends the reading. The pin of the `ess` binary is unchanged: one digest over every byte of the binary, streamed in chunks, at each start of `sdc-conform` that observes (`observe`, `recompute`, `ess-path`; `check-table` takes none), and no cache, file or variable stands in for that digest.

### Fixed

- A spend cap that is named and has nothing left starts nothing that may spend (story `spend-cap-with-nothing-left-starts-nothing`): no `tree run`, no handoff and no `tree approve` follows, whatever the bindings name (a model tool without a budget option, a cap of 0, a cap that earlier steps booked whole, a model node that the decider or a re-drive makes), and the run ends `stopped: spend cap` (exit 4) with the open decision left open in the records. A run in which nothing can spend (no model node, no model tool in its copy of the bindings, nothing booked) is not stopped by a cap of 0, and a run that names no spend cap is never stopped by this. The same holds before every program of an answers file (`requests`, `apply`, `then`; a decider whose own cost takes the cap stops the approve, the run directory exists and the run is not resumed) and before the conformance observation of a run that closed (`--conformance observe`): it reserves what the cap has left, books what its record names (its reservation when the record names none), and with nothing left it is not started, the end of the run says so in one line of amounts, and the run keeps the exit it had. A reached call cap (`--max-llm-calls`) starts no `tree approve` and no program of an answers file either. The limit stays: a model a bound program reaches by a name no text of its copy holds is outside the cap. `docs/loom-run.md`, "What the spend cap promises".

## [0.8.0] - 2026-11-23

The inspector has a Products tab, Loom `c9b4c177` is linked and the bindings state their exits. The tab lists
the files a step left under `work/` and shows one as text; a model node that was not called no longer counts
against `budget.max_runs`; each of the 50 program bindings has `exits.error` and each of the 14 model nodes
`exits.not_called: [79]`, which `scripts/check-bindings.sh` checks; `runbook test` takes `--cycle-timeout` and
`--list`. Breaking: `sdc-node` exits 79, not 0, where it prints "not called"; a bindings file of a user's own
without `exits` has no exit class under Loom; a cycle of a suite works in `<scratch>/work/<n>` and keeps its tree
in `<scratch>/store/<n>`; `runbook test` prints a `slowest:` line. No evaluation run was made on this code. The
longer note is `docs/releases/0.8.0.md`.

### Added

- The run inspector (`ui/`) and the daemon's page show what a step produced (story `inspector-shows-step-products`). A Products tab lists the evidence records of the case (every field, nested values readable, a field the run did not record says "not recorded by this run"), the actions (exit code, the tail of the output, where the run records no start, end or duration it says so and shows the step's span from `observed.jsonl`) and the files under `work/` (a `.txt` step file with its `result:` line, JSON indented, YAML as written, Markdown rendered by `ui/web/markdown.js`, a file that is not text by size and type, a file over 1 MiB cut with its full byte count). The product of the run is linked under the header: the view file's new member `product` (a path relative to the run directory, refused when it is not a string, is empty, absolute, leaves the run or has a dot component; `runbook check` refuses the same), else `work/<runbook>/<runbook>.md`, else the newest Markdown files, said as which rule named it. The routes `/api/products`, `/api/file` and `/api/output` only read, through the run-path check on the open file, regular files only, and serve only what lies under a `work/` directory of the run (a `work/` that is a link is no root). They never serve a path with a component that starts with a dot (a tool's cache or login: 404, also through a link; the listing says how many it left out), a file with more than one link, a link that leaves the run, or anything outside `work/` (`settings.json`, `store/`, `llm-calls.jsonl`). The listing is bounded (500 files, 2000 directories, 20000 entries, 6 levels, and it says what it cut and how many files it left out), the run log is read from its end in at most 8 MiB, a text read of a run file (`readText`) refuses a file with more than one link and a file over its size limit, every read of a run's file goes through one checked open (the decisions read under the run, and `yq` is given the text on standard input, never a file name), and the Markdown renderer's cost is bounded (`ui/README.md`). The page builds nodes from run content and never assigns markup. The daemon's page has the same tab and says the daemon serves no files of a run yet. Gaps named, none changed: the runner's projected events drop Loom's `blob_digest` (`runbook/src/loom_run/run_dir.rs` `read_runlog`), so the page reads Loom's run log of the case in the run directory; the runner records no start, end or duration per action (`run_dir.rs` where it writes `action_started` and `action_finished`); the daemon's API has no file, evidence or action-output view (`ui/README.md` "What a step produced", `docs/daemon-api.md`).
- `runbook test --cycle-timeout <seconds>` (Loom's `SuiteSettings::cycle_timeout`, 1 to 60, default 60): a cycle past it has its running program killed and ends `CycleTimedOut`, naming its step; a whole number outside 1 to 60 is refused by `runbook` itself (exit 2) before the suite is read, a word or an empty value by the option parser (exit 64). `runbook test --list` prints one line per cycle (`<name>`, `<name>: pending`, `<name>: pending, waits for story:<id> (<kind>)`; Loom's `list_suite`), runs nothing and needs no `--confinement`. `--help` says both. There is no `--cgroup-root` option on `runbook test` (it passes none; `substrate` is still refused, `docs/cli.md`). Tests: `runbook/tests/test_suite/options.rs`.
- A no-model test holds the state of run 1 of record `parity-loom-strong-4` (`runbook/tests/test_suite/notcalled.rs`, suite `notcalled.suite.yaml`, hand-written sh programs): a check refuses the specification, the writer is due again through its `retry_when` and exits 79 (`exits.not_called`) while a finding waits for a person; it is started once, no run is counted against `budget.max_runs`, and the run stops `AwaitingPerson` (cycle exit 3). Red at Loom `db2726ff` (the writer was started 59 times and the cycle ended at the 60 s bound, exit 5), green at `c9b4c177`.

### Changed

- The crate `runbook` links Loom `c9b4c177` (`c9b4c1777c646afdfae843dae691da36859f56ee`, version 0.59.0, branch `wave/2026-11-21-w7` of the engine on the forge, in no tag; replaces `db2726ff`) for all three Loom crates, one git rev, and carries the unit `bindings-state-their-exits`. A model node that ends "not called" (`exits.not_called`) is not started again in a loop: Loom does not take that writer again until the case is evaluated again for a reason other than its own moves (`story:writer-not-called-leaves-the-due-set`), so it starts once per evaluation and the run goes on to the next useful action or to the handoff check. A not-called writer that moves an artifact and a writer of evidence on that artifact still alternate until the cycle's time bound. In `runbook/Cargo.lock` only the 14 Loom packages move (their source commit; no version and no other package changes). The embedder changes are handled by name: `SuiteSettings::cycle_timeout` is set from `--cycle-timeout`, `DifferenceReason::CycleTimedOut` has no match in the crate (a cycle that ends so is a failed cycle with Loom's line, `options.rs` holds it), `suite::list_suite`, `suite::slowest_line` and `suite::CYCLE_WALL_SECONDS` are used by `runbook test`; the `b10x_loom_cli` members (`TreeTestArgs`, `TREE_TEST_*`) are not linked here. No new run-log kind and no new `TreeRefusalReason`, so `runbook/src/engine/refusal.rs` is unchanged. The pin text is in `AGENTS.md`, `docs/loom-bindings.md`, `docs/cli.md` and row M of `docs/library-entries.md`; the slow lane `task test:loom-in-process` still names the child binary of `70b0e6e0` (`docs/loom-run.md`).
- A cycle of a suite works in `<scratch>/work/<n>` and keeps its tree in `<scratch>/store/<n>` (Loom `c9b4c177`; was `<scratch>/<n>/work` and `<scratch>/<n>/store`): the tests of `runbook test` and `exit_classes`, the fixture program `meet-write.sh` and `docs/cli.md` follow. `runbook test` prints Loom's `slowest: <name> (<s> s), ...` line in place of the `ten slowest:` block; `--workers` above 16 under `--confinement substrate` is refused with Loom's `SubstrateWorkersUnserved` (was above 1; `substrate` itself is still refused, `runbook test` holds no cgroup root).
- The driver finds the workspace where Loom started the program (found by running the review under the new suite layout): `sdc_tools::reply::records::case_work` is the one place that decides it. Under `LOOM_CASE` it is the current directory (Loom `c9b4c177` starts a bound program in the workspace root with `PATH`, `LOOM_CASE`, `LOOM_TIMEOUT_SECONDS`, `LOOM_CASE_NAME` and `LOOM_PARENT_CASE_NAME`, and names no case directory and no store), without it `<case dir>/work`; the case directory a caller names is not read under Loom. `Facts::open`, `sdc-reply`, `sdc-node` (`change.review`, `change.implement`) and the fixture adapter use it. Before, `<workspace>/..` was the case directory and its `work` the workspace; at `c9b4c177` that is `<scratch>/work`, which has no `work`, so the rejections of `change.review` were counted as none; and a rule that read a directory named `work` in the workspace (a repository may have one) would have misread it. `driver/tests/workspace_layout.rs` and three tests of `sdc_node.rs` hold the layout with no `work/`, an empty `work/` and a look-alike case in `work/`.
- The bindings state their exits (`story:bindings-state-their-exits`, Loom `70b0e6e0`): each of the 50 program bindings of `bindings/*.yaml` has `exits.error` (`[75, 126, 127]`; `[69, 70, 75, 126, 127]` on `tests.run` of `sdc-story` and `sdc-bugfix`, `acceptance.run`, `repro.run`, `gate.run`), and each of the 14 model nodes `exits.not_called: [79]`. `sdc-node` exits 79 where it prints "not called" (`questions.write` with an interview, findings that wait for a person) and writes no evidence (a derived tree that is only renewed keeps exit 0: the renewed marker is what moves the case); a tool that cannot run ends the run `ProgramError` and no run of it is counted against `max_runs`. The D36, D37 and pool-wait entries leave `bindings/PENDING.md`; `scripts/check-bindings.sh` checks the lists and that no code is in both classes or in a result table. `sdc-tool.sh` keeps no log record of a tree that does not exist yet, so the first `spec.gate` of an epic (it runs before the spec does) still fails as before and no longer ends the run `ProgramError` with exit 126. `cycles/tree-suite.yaml` has the entry `spec-person-wait`, the state of record `parity-loom-strong-4` run 1.
- `scripts/check-bindings.sh` refuses by name an `exits` class that Loom does not read (it reads `error` and `not_called`), a class that is not a list of exit codes and an `exits` that is not a mapping; before, an unknown class passed and a class that was a number stopped the script with a raw `jq` error. `check-bindings.sh --exits <root>` runs the exits check alone over `<root>/bindings/*.yaml`; `driver/tests/check_bindings_exits.rs` holds one case per refusal.
- Tests of the neighbours of the not-called fix (`runbook/tests/test_suite/notcalled_neighbours.rs`: an exit 79 the binding does not list, a program not called on every evaluation, two writers not called at once, a writer that writes and then exits 79) and of an empty `LOOM_CASE` (`driver/tests/workspace_empty_loom_case.rs`: it is no Loom mode).

### Fixed

- `task test:runbook` was red on the forge, not here: `exit_classes::the_gate_on_a_missing_spec_fails_and_is_no_program_error` ran the host's `ess`, and the forge's runner has none (127 instead of 1). The test now writes its own stand-in `ess` (and `yq`, `jq`) and gives `sdc-tool.sh` an explicit `PATH`; a second test pins the no-`ess` case (127, "ess is not on PATH", and 127 in `exits.error` of the binding). The step puts a trap `ess` (`scripts/host-tool-traps/ess`, exit 127) first on `PATH`, so a test that starts the host's `ess` fails here too.
- The run inspector (`ui/`) no longer says "Nothing" about what changed in a run of `runbook run` that has no `fs/changes.jsonl`. "What changed" and the Changes tab say "Changes were not recorded for this run", that it was started without `--watch`, and that `runbook run … --watch` records every file change of the run in `<run>/fs/changes.jsonl` (the words of `docs/cli.md`); "Nothing" is said only when that file exists and is empty. The SDC runbook's texts do not change (`ui/test/loom-run-record.spec.mjs`).
- The run inspector (`ui/`) takes the protocol revision of a case from the `opened` event of its `events.jsonl`, as it takes the protocol id: the case header, the explanation header, the event list and the canon-check line of Metrics show `r<revision>`, and a case whose event has no revision shows no "r" and no "case" without a number, where they read "r" and "case " (`ui/web/model.js` `protocolRevisionOf`, `ui/test/loom-run-record.spec.mjs`).
- The run inspector (`ui/`) no longer shows a claim count of 0/0 or 0/6 for a run of `runbook run`. The runner writes no `evaluations/*.json` and no claim count, so the phase strip, its stage head, the case tree, the Progress phase and the Claims TRUE tile of Metrics say "claims not recorded for this run"; a count shows only for a case with an evaluation (`ui/web/model.js` `claimsRecorded`, `ui/test/loom-run-record.spec.mjs`).

## [0.7.0] - 2026-11-23

The old driver runtime is deleted and a refusal of Loom ends a run by name. The binary `sdc-drive`,
`scripts/run-trial.sh`, `scripts/run-finding-cycles.sh`, `scripts/run-full-cycle.sh`,
`scripts/watch-run.sh`, `bindings/tools/llm-node.sh` and `bindings/tools/decider.sh` are gone: a run is
`runbook run`, the runtime is Loom's, and the finding cycles run as `runbook test cycles/tree-suite.yaml`.
Loom `db2726ff` is linked: an action is dated by the revisions its own step left, so the amend step is
taken again after a check that failed following a later write. A refusal of Loom ends a run with the
outcome `refused`, exit 12 and a named `cause`; `runbook test` takes `--workers` and `--only` through
Loom's selection. Breaking: the package `sdc-drive` is `sdc-tools`; the producer ids of the bindings are
`runbook`, `runbook.build` and so on; `ConfinementRefused`, `PersonProgramFailed` and `NotAPersonAction`
were exit 1 and are exit 12; a `submit-run` below the daemon's floor that names a missing session is refused `no-session`, no longer `below-floor`; 37 of the 130 entries of the old cycle suite are dropped. No evaluation run
was made on this code. The longer note is `docs/releases/0.7.0.md`.

### Added

- `runbook test` takes `--workers <N>` (1 to 64, default 1: one cycle after another) and passes it to Loom's suite runner (`SuiteSettings.workers`); more than 1 under `--confinement substrate` is refused before anything is written with Loom's own `SubstrateWorkersUnserved` (exit 2); a value outside 1 to 64 is exit 64. `--help` lists both options. `--only` is Loom's own selection now (`SuiteSettings.only`): the command no longer writes a filtered copy of the suite beside it, a name that is no cycle is Loom's `CycleUnknown`, and the closing lines say `selected S of C cycles`. A cycle keeps the directory of its position in the file (`<dir>/<n>/`). Tests: `runbook/tests/test_suite/workers.rs` (help, two workers meeting in one run, the refusal, the range) and `mod.rs` (`--only`, `--help`, the default).
- A run ends on a refusal of Loom by name: the new outcome `refused` (exit 12) with the member `cause` in `outcome.json`, in the `ended` event and in `run-stopped.json`, for every reason of Loom's `TreeRefusalReason` (29, one `cause` each in `runbook/src/engine/refusal.rs`, an exhaustive match with no catch-all arm, so a reason Loom adds stops the build here) and for a program the engine's runner ended by a signal (`ProgramSignalled`, cause `program_signalled`, read from the action's `ActionFinished.refusal` when the tree ends `Stuck`). The reasons of Loom `db2726ff` that are new to a run are `TreeStopped`, `TreeAlreadyStopped`, `TreeNotStopped` and `StopUnusable`. The whole change in exit codes: in a library run every refusal of `tree open` that leads with a reason name (`BindingsRefused`, `ProtocolRefused`, `StoreNotEmpty`, `UnknownProtocol`, ...), and every refusal of `tree run` and of an answer that does (`ConfinementRefused`, `PersonProgramFailed` and `NotAPersonAction` among them), is exit 12 `refused` where it was exit 1 `error`; with `--loom` the runner reads the child's `b10x-loom: tree refused: <Reason>` line after `tree open`, `tree run`, an answer and a handoff alike, so the same holds there (a library run runs no handoff); refusals of `tree status` and `tree verify` after the run stay exit 1 `error`, with no cause. Exit 2 stays a refusal before a run directory exists (`--loom` without `--loom-sha256`, a settings file that disagrees with the options, a confinement level the library cannot give: `--confinement substrate is not a level the library can give here`, no `cause`; a missing or invalid option is exit 64); exit 12 is Loom refusing a call of a run that has started. `docs/loom-run.md` has the table and the list of the 29 causes, `docs/loom-run-directory.md` and `docs/loom-run.md` the members of `run-stopped.json` and `outcome.json` (`cause`); `runbook/tests/loom_run/pages.rs` holds the pages to `refusal.rs` both ways (a cause the page lacks, a cause or a count it has wrong). Tests: `runbook/src/loom_run/mod.rs` (`outcome_tests`), `runbook/src/engine/refusal.rs`, `runbook/src/loom_run/library.rs` (the text of `check_level`), `runbook/tests/loom_run/child_refusal.rs` (a child's refused `tree run`, `tree approve`, `tree open` and handoff, a reason that is no reason, a refusal line on a known exit code, a refusal tail with no login path or token), `runbook/tests/loom_run/inproc.rs` (a bindings file Loom refuses at `tree open`), `runbook/tests/loom_run/run_dir.rs`.
- A tree that is held (`tree hold`) is refused by name: `runbook/tests/loom_run/held.rs` runs a held tree through the library (`TreeStopped`, outcome `refused`, exit 12, cause `tree_stopped`), reopens it and runs it again, and refuses a second hold and a reopen of an open tree (`TreeAlreadyStopped`, `TreeNotStopped`).
- The run-log lines `StopRecorded` and `StopLifted` (a hold and its reopen) are read without error by every reader of a run log or of its projection, one test each: the runner's run directory (`runbook/tests/loom_run/run_dir.rs`), the tail of the engine (`runbook/src/engine/tail.rs`, which reports both as Loom encodes them), and the re-drive tool `sdc-replay` (`events_of_log`, no event for either line; `driver/src/bin/sdc-replay.rs`). The run inspector reads the projected `events.jsonl`, never the run log, and the projection skips both kinds, so it never meets them (no test of its own). `sdc-watch` reads no line of a run log (it ignores the file) and the daemon (`ui/daemon/decide.js`) reads no event.
- A no-model test holds the state of run 2 of record `parity-loom-strong-4` (`runbook/tests/test_suite/amend.rs`, suite `amend.suite.yaml`, hand-written `sh` programs): an action, a later write, a check that exits non-zero and a handoff check that says there is nothing to wait for. Loom `70b0e6e0` ended it `Stuck` (exit 4); `db2726ff` takes the amend step again and the run ends `passed`.

### Changed

- `cycles/tree-suite.yaml` at Loom `db2726ff`: every `pending` entry was run once. `run-stop` (its steps expect the refusal `TreeStopped` and its check reads the `StopRecorded` log line), `replay-sibling` and `decider-rework` run as expected and lost `pending`; `decider-rework-spent` and `rework-fresh-allowance` keep it (the third and fourth run of `change.implement` meet `RunsSpent`, `budget.max_runs` 3), as do the other 12 with their difference in `docs/cycles-suite.md` (40 converted, 14 pending; was 37 and 17). No entry changed from expected to another state. A cycle ends on Loom's constant 60 s wall bound when the host load is high (11 entries at a one-minute load of 100 to 177); the counts are of single runs at a quiet host.
- The crate `runbook` links Loom `db2726ff` (version 0.59.0, the merge into the branch `wave/2026-11-21-w7` of the engine on the forge, in no tag; replaces `70b0e6e0`) for all three Loom crates, one git rev. In `runbook/Cargo.lock` only the 14 Loom packages move (11 of them from 0.58.0 to 0.59.0); Canon, Substrate and the model crate are unchanged. The pin compiled without a change of the sources; what is new is handled by name (above). The pin text is in `AGENTS.md`, `docs/loom-bindings.md` and row K of `docs/library-entries.md`. The slow lane `task test:loom-in-process` still names the child binary of `70b0e6e0` (`docs/loom-run.md`).
- The package `sdc-drive` is renamed `sdc-tools` (`driver/Cargo.toml`; the library is still `sdc_tools`, `runbook` depends on it as `sdc-tools`); its lock files change by the package name only. The helper programs keep their names (`sdc-node`, `sdc-reply`, `sdc-findings`, `sdc-bugfix` and the rest).
- `runbook check` compares `cycles/tree-suite.yaml` (`loom.tree-suite/1`: format, cycle names, seeds under `cycles/`) with `cycles/old-suite-names.txt`, the committed list of the 130 entries of the deleted suite, and with the rows of `docs/cycles-suite.md`, in place of the `expected` and `pending` arrays of the deleted script; it also reads `runbook/src/` for the instruction files a program names. `runbook doctor` no longer lists `canon`, `sdc-drive`, `sdc-unstick`, `rsync` or `bindings/tools/decider.sh`: nothing starts them.
- The prompts and requests of the decider that `bindings/tools/decider.sh` printed are held byte for byte by the runner's own tests (`runbook/src/loom_run/decider/golden.rs` and `requester_tests.rs`, goldens in `instructions/golden/decider/` and `instructions/golden/requester-request/`): the 12 model prompts and the 3 requests equal the goldens of the shell decider byte for byte, the 2 requester prompts are re-made by the Rust decider (they now list `in/goals.yaml` and carry the grounding check's own refusal text); the feedback file of a writer sent back by checks is held by a test of `sdc-node` (`driver/tests/sdc_node.rs`, golden `instructions/golden/feedback/checks.md`). `scripts/evaluate-cases.sh` no longer makes a `canon` shim.
- The documents, the website, the plugin skills and `AGENTS.md` say that the runtime is Loom's and that a run is `runbook run`: they no longer tell a reader to run the driver binary, the trial script, the cycles script or the shell model and decider tools (`story:driver-runtime-removed`). `docs/driver.md` says what `driver/` is now (the crate `sdc-tools`, the bound programs and tools) and keeps the former runtime as a dated record; the pages that are records of a past state say so at their top.
- The producer ids of the bindings name the runner: `sdc-drive` is `runbook` and `sdc-drive.build`, `.deploy`, `.monitor`, `.merge`, `.inspect`, `.falsifier` are `runbook.build` and so on, in `bindings/*.yaml`, `driver/examples/**` and the generated `website/data/protocols/*.json`; no Canon or Loom list names a producer, and no digest in a committed fixture covers one (`driver/tests/producer_ids.rs` holds the tree to it). Records of past runs (`cases/**/evidence`, `experiments/`, the recorded run of `ui/test/recorded/`) keep the old id.
- The six `driver/examples/sdc/*.bindings.json` files start `sdc-node <action>` for their agent actions, as `bindings/*.yaml` does, and `scripts/check-bindings.sh` fails when the two leading `argv` members of the two copies differ, or what the route check reads of a binding: its kinds, results, subjects, `retry_when`, `max_runs`, `manual` and `handoff` (the JSON copies of `change.implement`, `questions.answer` and `amend.upstream` now equal the YAML's; the copy of `questions.answer` gains its evidence); `model_step_argv` recognises `sdc-node` only. The block `driver` of `driver/examples/sdc/rule-ends.json` is `runtime` and carries `provided_by`: who holds each capability in a run of `runbook run` on Loom (`loom:`, `runner:` or `absent:`, each with the file and line that says it); `sdc-check-routes` refuses a file whose `runtime` block it cannot read whole (exit 2, nothing on standard output, every reason on standard error, one line each): a missing or unknown member, a member of the wrong type, a `provided_by` value with no known prefix, nothing after it or (for `loom:`) no `file:line`, and a member that contradicts its entry; `scripts/check-bindings.sh` prints `REFUSED routes` and stops with exit 3 on it, and goes on with exit 1 on findings. It prints one `capability` line per member after its routes, and now finds 36 routes (were 15): the block states what Loom does (`max_runs` caps every action, per action and not per kind, with no reset on an arrival, and feeds back only the records `retry_when` lists), so `own-failure` and `shared-cap` findings appear and `inert-cap` on `change.review` goes. The `case_outcome` records of an opened child carry the producer `loom-child-case` (`docs/driver.md`).
- In `ess/domains/daemon.yaml` the outcome `no-session` of the command `SubmitRun` is declared above `below-floor`, the order in which the precedence answers, so that the next ESS release accepts the specification (`story:submitrun-outcome-order`); the committed suites and their `DIGEST` are regenerated. One answer changes with it (next bullet), and the daemon's conformance run prints 13 of 13 authored scenarios and synthesized 34 total, 32 executed, 27 passed, 5 failed, 2 skipped (before: 12 of 12, and 33 total, 31 executed, 26 passed, 5 failed, 2 skipped). The table `OUTCOMES` of `runbook/src/commands.rs` lists the outcomes of every command in the order of the domain file (`answer-decision` moves `already-answered` above `no-run`; the handler already answered in that order), and a test holds the order.
- The daemon answers `no-session` before `below-floor` when both hold, as `ess/domains/daemon.yaml` declares (`runbook/src/commands.rs`, `submit_run_for`): a `SubmitRun` below the confinement floor that names a session that does not exist is refused as `NoSuchSession`, where it was refused as `BelowConfinementFloor`; a caller that read `below-floor` for that input sees another refusal. A request below the floor into an existing session, open or closed, is still refused as `BelowConfinementFloor`. A new authored scenario (`ess/scenarios/daemon-a-missing-session-is-refused-before-the-floor.yaml`) and a test of the daemon's commands hold it; the suites and `DIGEST` are regenerated.

### Removed

- The old driver runtime: the binary `sdc-drive` (`driver/src/main.rs`), `scripts/run-trial.sh`, `scripts/run-finding-cycles.sh`, `scripts/run-full-cycle.sh`, `scripts/watch-run.sh`, `bindings/tools/llm-node.sh` and `bindings/tools/decider.sh`. A run starts with `runbook run` (the model step is `sdc-node`, the decider is the Rust decider of `runbook run`); the finding cycles run as `runbook test cycles/tree-suite.yaml` (`task test:cycles-loom`, `ONLY=<name>[,<name>...]` for named cycles) in place of `task cycles:only`; `task test:decider-parity` is gone with the script it compared.
- What only those files used: `sdc-unstick` (the program, `instructions/run.unstick*.md`, `instructions/golden/unstick/`, the class `unstick` of `bindings/tools/node-classes.json`), `cycles/suite.yaml`, `instructions/shared/feedback-run.*` and the goldens of the driver's own feedback, `decision/examples/goals/trial-2.yaml`, `watch/tests/e2e.sh`, the tamper script of the rule-approval fixtures, the parity tests of the decider (`parity.rs`, `parity_cases.rs`) and the scripts of the adversary evaluation (`eval/run.sh`, `eval/run-adversaries.sh`, `eval/adversary.sh`; `eval/reports/` and `eval/baseline.json` stay as the record).
- 37 of the 130 entries of the old cycle suite are dropped with the driver, because they tested the driver's own autopilot loop, `--keep-going`, the unstick step, the rule approver, stand-in binaries through the environment or a second tool reading a finished run; `docs/cycles-suite.md` names each one. The other 93 are held: 37 converted and 17 pending in `cycles/tree-suite.yaml`, 39 by Rust tests of the runner.

## [0.6.0] - 2026-11-23

The runner routes decisions by autonomy level and can re-drive a recording. `runbook run` reads
`tools/autonomy-levels.json` once before the first action and stops every decision for a person when
it cannot, at A5 too; `runbook run --recorded <source>` re-drives a case tree from a recording with
`--on-divergence` and `--live-cap`; `runbook canon` runs the linked Canon library, so `runbook doctor`
needs no `canon` binary; the effort per node class reaches a model node (`sdc-node --effort`); and the
run inspector is read-only. Breaking: the inspector's acts, `POST /api/act` and its options `--write`,
`--write-only` and `--principal` are gone; a run with no readable routing table stops every decision for
a person, at A5 too; the decision contract refuses a letter nobody offered, an escalation to a principal
who is not eligible, a repeated option letter and an empty record id. The three evaluation runs of
`experiments/parity-loom-strong-4.yaml` on this code delivered 0 of 3. The old driver (`sdc-drive`,
`scripts/run-trial.sh`, `bindings/tools/llm-node.sh`, `bindings/tools/decider.sh`) is still in the tree.
Loom is pinned at `70b0e6e0` and Canon at 0.11.0. The longer note is `docs/releases/0.6.0.md`.

### Added

- `runbook run` routes decisions by autonomy level: `tools/autonomy-levels.json` (`sdc.autonomy-levels/1`, the table of `docs/autonomy-levels.md` § 2.2, a test compares the two) names the holder of each class at each level, and one loader (`runbook/src/loom_run/decider/routing.rs`, shared with the requester decider's class reader) reads it. At A1 to A4 the decider answers only a class the table gives to `decider` and the copy of the bindings gives the decider's principal only to those person steps; every other holder, an unknown class and a missing or inconsistent table stop the run for a person with no model call, and `escalate: final` applies at A5 only. The table is read first at every level, A5 included: with no readable table no rule and no model decides, and the run ends as a decision that waits for a person (exit 8). At A5 the close through `operating` is taken by `rule:autopilot` with no decider call. The record of a close names its decider (`rule:autopilot`, `decider:model`), never a person. A rejection of an action that declares no `rejected` result stops the run `stopped: rejected` (exit 10, cause `rejected_without_rework_path`, reason `<step> in <case>: …`) instead of `error`; the reason of a stop at a handoff names no person. The `#[ignore]`d tests of the six runner differences are rewritten and run: `approved_on_checks` is not in the runner (no claim values; an approval nobody validly gave stays `CannotDecide`), and the release and the close at A3 are a person's under the table (`docs/loom-run-decisions.md`, `docs/cycles-suite.md`).
- Six tests of the runner that were `#[ignore]`d for a difference from the cycle are rewritten for what the runner does and run, none ignored: `approved_on_checks` after three invalid replies stays `CannotDecide` (`runbook/src/loom_run/decider/tests.rs`); a rejection with no result to record it stops the run with exit 10 and cause `rejected_without_rework_path`, the close at A5 is taken by `rule:autopilot`, the close and the release at A3 are a person's, and the reason of a stop at a decision the decider cannot take names no person (`runbook/tests/loom_run/decisions.rs`).
- `runbook run --recorded <source>` re-drives the case tree through Loom with every model and decider call answered from a recording or a run directory by `sdc-replay` (`runbook/src/loom_run/replay.rs`): `--on-divergence stop|nearest|live`, `--live-adapter`, `--live-cap`, `--inject`, `--set` (`max_runs.<name>`, `timeout_s.<name>`), `--take-recorded` and `--source-tree` are built and need `--recorded` (exit 64 without it). The plan of `sdc-replay prepare --plan-only` becomes settings of the run (a new `replay` member, last in `SETTINGS_KEYS`) and `bindings_copy.rs` writes the adapter of each node, `--principal`, `budget.max_runs`, `timeout_seconds` and an argv wrap into the bindings copy. The runner reads `replay/diverged.json` after every step and ends `stopped: recording diverged` with exit 11 (cause `recording_diverged`; 9 stays `denied`, 10 is `stopped: rejected`); the live cap fails closed. `sdc-replay` indexes, seals, compares and packs a Loom run directory (arrival 1 at the first `Evaluated` of a case, a later arrival is a `case_revision` change between two `Evaluated` lines with no finish or close between, try is the finished runs since the last arrival plus 1; recordings of the old driver replay as before). The old exit 4 `writer_spent` is `stuck` (exit 3) and a timed-out call is not retried (Loom refuses exit 124 of `change.implement`). `sdc-experiment` passes `--recorded` and its options to `runbook run` in place of `port_recorded`. The recording `driver/examples/sdc/fixtures/recorded/stub` is packed again from a run of the runner (the ess pin and the C-locale `diff` order had moved its recorded `in/tests.txt` and `in/change.diff`; the old launcher diverged on it too). The ten replay entries of `cycles/suite.yaml` are Rust tests in `runbook/tests/loom_run/replay.rs`: nine read six runs that the cargo-nextest setup step `replay-runs` (profile `replay` of `runbook/.config/nextest.toml`, `runbook/tests/replay-runs.sh`, in `${XDG_STATE_HOME:-$HOME/.local/state}/runbook/replay-tests`) makes once, and are `#[ignore]`d in the default test step because six re-drives take minutes at host load: they are the slow lane `task test:replay`; the tenth (the committed recording is clean) and two small ones (the indexer on the recording, the plan-only settings) run in the default step; `docs/cycles-suite.md` names the test of each (`docs/loom-run.md` "A recorded run", `docs/cli.md`, `story:driver-runtime-removed`).
- The effort per node class reaches a model node: `sdc-node` takes `--effort <level>` (the level of `efforts` in `bindings/tools/node-classes.json`, else exit 64 naming the levels; without it the class default as before), the settings key `efforts.<role>.effort` (roles `writer`, `implement`, `review`, `other`) is written by the bindings copy as `--effort` into the model nodes of the role, and a level outside the set, a role that is none or another member is refused by name (`settings.efforts.writer.effort`) before the run directory is made. The adapters `metaharness` and `claude-cli` pass the level on as their `--effort`, as the old launcher did, and the dry adapter shows it on its command; the `fixture` and `stub` adapters take none. The cycle entry `efforts` is held by Rust tests (`runbook/tests/loom_run/dry.rs`, `driver/tests/sdc_node.rs`; `docs/cycles-suite.md`). `sdc-experiment` still refuses `efforts.<class>.effort` in a `loom` variant (`loom_settings_json` does not write it yet).

### Changed

- The ESS pins move from 0.97.3 to 0.97.4 (faster synthesis, suites byte-identical to 0.97.3, no breaking change): `driver/ess-pin.json` (tag `0.97.4`, commit `b076ef00be`; archive sha256 equal to the release's `SHA256SUMS`), `requires: ess 0.97.4` in `ess/ess-inputs.yaml` and the `ESS_TOOLCHAIN` default of `bindings/tools/sdc-tool.sh`. `ess/generated/conformance/DIGEST` moved with its source `ess/ess-inputs.yaml`; `suite.json`, the schema and the OpenAPI output are byte-identical.
- The run inspector (`ui/`) acts on nothing. The Approve, Answer, Change and Continue acts are gone, with `POST /api/act`, the option `--write`, `--write-only` and `--principal` (an unknown option exits 2, any request that is not a GET or a HEAD answers 405), the lookup of the old driver's and Canon's binaries (`ui/binaries.mjs`), the toast and every code path that started `sdc-drive` or `scripts/run-trial.sh`. The Decisions drawer says in one line where a decision is answered (`runbook serve --ui`, `runbook decide <run> <request> <answer>`, `runbook runs decisions <run>`). The inspector has no route, token or proxy to the daemon. Whether a run is live is read from what `runbook run` writes: `settings.json` without `run-stopped.json` or a closed `status.json` is "live state unknown" (the runner writes no pid or lock), and a case store of the old driver is history, shown as not live; no process is looked up by name. In the view file, `tool_owners`, `decisions.scope_answer.check`, `refused_answerers`, `decisions.context` and `decisions.continue` are no longer members (a view that has one is refused). `ui/README.md` says what is true now.
- `runbook canon validate|compile|check|simulate|evaluate` runs the Canon library the crate links (`b10x-canon`, direct dependency at tag `0.11.0`, `runbook/src/canon_cmd.rs`) with the arguments, output and exit statuses of the `canon` binary of that release (byte-equal on the 9 protocols and the 52 `cases/` evaluations, apart from the usage text of a command line it refuses). `scripts/evaluate-cases.sh`, `scripts/check-protocols.sh`, `scripts/limitations.sh`, `plugin/skills/{harden,record}` and `.github/workflows/protocols.yml` call it and no longer need a `canon` binary or its cached install; `driver/canon-pin.json` is gone and the pin is the `b10x-canon` entry of `runbook/Cargo.lock` (`runbook doctor` reports that linked Canon, a row that is `ok` on every host: it looks up no `canon` binary and reads no `CANON`). The protocols workflow builds `runbook` once per job and runs on `runbook/Cargo.lock` and `runbook/src/canon_cmd.rs` in place of `driver/canon-pin.json`. The ledger digests of `protocol/check-ledger.json` move with the script (its sha256 is part of the digest); no result field changed.
- `scope.answer` of `bindings/sdc-intake.yaml` is a person step with a program: `argv: [tools/sdc-tool.sh, scope-answer, "{principal}", --action, scope.answer]` (Loom `70b0e6e0`, `story:person-action-runs-a-program`), so the engine runs the program at the approval, through the runner of the run's `--confinement`, with the approving principal (`person:owner` in the suite, `decider:<kind>` where the answers file declares the step and the copy of the bindings binds it to the decider). `bindings/tools/answers.yaml` no longer has the `then` program for `scope.answer`, which the runner ran outside the engine. `bindings/PENDING.md` loses D11, D15 and D38. The entry `scope-answer` of `cycles/tree-suite.yaml` is converted (it ends `operating`); `scope-changed` and `intake-decision` run further and stay pending, `decide:` now names the case `root.intake`. Tests: `runbook/tests/loom_run/inproc.rs` (`the_approval_of_a_person_step_runs_its_program_with_the_approving_principal`, `a_person_program_that_fails_ends_the_run_with_the_engines_refusal`, `a_declared_step_with_a_program_gets_the_decider_as_principal_from_the_engine_and_no_then_runs`).
- The crate `runbook` links Loom `70b0e6e0` (branch `wave/2026-11-21-w7` of the engine, on the forge) in place of `5b3142c7`; Canon stays at 0.11.0 and Substrate moves from 0.21.0 to 0.24.0. A person action whose binding names an `argv` runs that program when its approval is taken (`story:person-action-runs-a-program`), so `calls::apply` hands the run's own runner (the one `--confinement` opened) to `ApproveRequest.runner`; a binding with no `argv` starts nothing, so no run of today changes. The engine's new refusals `ConfinementRefused` and `PersonProgramFailed` of an approval end a run `error` (exit 1) with Loom's text (`docs/loom-run.md`). The child binary of the slow lane `task test:loom-in-process` is the one of `70b0e6e0` (`~/.local/state/runbook/loom/70b0e6e0/b10x-loom`).

### Fixed

- The run inspector (`ui/`) applies a view file to a run of `runbook run`: the protocol of a case is read from the `opened` event of its `events.jsonl` when it has no `case.json` (`/api/view-info` answered `protocols: []` and no view for such a run with any `--view`); the page shows that protocol in the case header too. A case with both files keeps the protocol of `case.json`.
- The run inspector (`ui/`) no longer shows a run of `runbook run` that ended well as "× not delivered". A good end is declared: the view file has an optional member `good_outcomes` (the outcomes of the runbook that are a good end; `views/sdc.view.json` lists `operating` and `delivered`, so the SDC pages do not change), or the runner's own `status.json` says `delivered: true`. A closed run is good (check), bad (cross: the view lists good ends and not this one) or neutral (diamond, "ended: <outcome>", no red: no view and no record says whether it is a good end); a Canon `protocol/1` outcome carries no class, so the format gives none. `/api/sources` carries `run.end` and `run.endWhy`; a closed run that has no status.json is no longer called delivered by its outcome's name (`ui/README.md` "The view file", `ui/test/good-end.spec.mjs`).
- The run inspector (`ui/`) draws the graph of a run of `runbook run` from the protocol's artifacts and actions and the run's bindings copy (`protocols/*.protocol.yaml`, `bindings/*.yaml`, read with `yq` as `work/questions.yaml` is), not from the store's `ir.json` and `bindings.json` alone: the artifacts are drawn as artifacts with the path the run placed them at, each action with who runs it and its bound program. A part it cannot read says "not available for this run" instead of "unbound" (`ui/runfacts.mjs`, `ui/test/graph-any-runbook.spec.mjs`).
- `runbook doctor` needs no `canon` binary: its `canon` row is the Canon this binary is linked with (version and the first 8 digits of the revision, read from `runbook/Cargo.lock`), required and `ok` on every host. It no longer looks at `CANON`, a cache or a checkout, and its hint names no deleted file. `runbook canon` with no subcommand prints the subcommands it has on standard error (exit 64), and `runbook canon -V` prints the version line as `--version` does (exit 0).
- The inspector (`ui/server.mjs`) skips a line of an `events.jsonl` that is no event (valid JSON that is not an object with a numeric `seq`), and a run that cannot be read does not stop the server: the scan at start-up and the periodic rescan catch a throw per run, log it once by the name of the run directory, list the run with an idle state and go on with the other runs. A `case.json` that holds `null` reads as absent.
- The inspector (`ui/server.mjs`) reads a file of a run only when its real path lies under the real path of the run directory (`ui/runfiles.mjs`, one check for every read, listing and stat of a run file, and for the artifact path of the decisions inbox): a link that leaves the run is not followed, not listed and not served (`/api/blob` answers 404 for it), a link that stays inside is. A request whose `Host` is not `127.0.0.1:<port>`, `localhost:<port>` or `[::1]:<port>`, or whose `Origin` is not its own, is refused with 421 before a route. A request line that does not parse is answered 400 and a request that fails in a route 500; the process goes on.
- The inspector serves no act code: the file with the token field and the POST of an answer is `ui/daemon/decide.js`, which `runbook/build.rs` embeds beside `ui/web/` and only the daemon serves; the page loads it only when the daemon names itself. `ui/test/read-only.spec.mjs` scans every `.js` and `.html` file of `ui/web/` for act code.
- `bugfix_from_ticket_with_dry_ends_fixed_and_the_fix_prompt_carries_the_constraints` no longer waits for `--dry`: a dry call already wrote its prompt, to `work/.sdc/llm/<NNN>-change.fix/prompt.md` in the root case's work directory, and the test looked for it outside `work/`; it now reads that path and passes in the slow lane (it needs `driver/target/debug` built, like its sibling).
- The decision contract (`ess/domains/decision.yaml`) refuses a letter the request does not offer: `Decide` and a verified `Recommend` answer `OptionNotOffered`, so a letter nobody offered is never stored and silence never takes it (`docs/decisions.md`).
- The decision contract refuses an `Escalate` to a principal who is not in the request's `eligible` list (`NotEligible`, outcome `target-not-eligible`); escalation adds nobody to that list.
- The decision contract refuses a `Request` whose options repeat a letter (`DuplicateOptionLetter`); `sdc-decision-request` also refuses a question of more than 26 options, for which it has no letters left.
- The decision contract refuses `RecordProduced` with an empty record id (`EmptyRecordId`), before it looks for the request: an empty id on a request that does not exist answers `EmptyRecordId`, not `NoSuchRequest`, and a non-empty id answers `NoSuchRequest`. What a caller can observe is that an empty record id is refused by `empty-record-id` before `already-recorded` and before `no-request` (the scenario `decision-empty-record-id-on-an-unknown-request.yaml` holds that order). The predicate of `already-recorded` now requires a defined record (`defined(produced_record)` first); no input can reach that after the empty-id guard, so no scenario can tell it from the old predicate and it is not a change of behaviour. The refusals of `OptionNotOffered` are held by scenarios and by `driver/tests/decision_contract_mutants.rs`, which runs the pinned ess over the specification with a guard weakened.
- `sdc-replay` masks the version label of the pinned ESS in the header line of the conformance log, the first line of that shape in `in/tests.txt` and in no other file, prompt or request (`<system> v<n> against <implementation> <version> — passed`; to the label the committed recording was packed with, so no digest of the recording changes): a new ESS pin no longer makes a re-drive diverge at `change.review`, while a changed test name still does. The replay lane `task test:replay` was red on 5 of 9 after the pin moved to 0.97.4. A recording packed by an older `sdc-replay` at another label diverges at `in/tests.txt`; the mask is not widened to meet it. The message names the label as the cause and says to pack the recording again only when that is proven (the unmasked digest of the current text is the recorded one); otherwise it says that `in/tests.txt` changed and names the old-label case as one possible reason, with no advice, because a changed test at the committed label gives the same mismatch.
- The replay adapter takes the plan of the run from the path the runner hands it, not from the nearest `replay/plan.json` above its call: `sdc-node` has the option `--replay-plan` (it sets `SDC_REPLAY` for its adapter), the bindings copy sets it on each replayed model node (`settings.replay.plan`), and the decider takes it from `decider.replay_plan`. A `replay/plan.json` inside `work/` is never read.
- The settings key `replay` without `--recorded` is refused, exit 2 before the run directory is made, naming the key and the option; with `--recorded` the adapter that key names must be `sdc-replay`. Before, any program path became the `--adapter` of every model node.
- The dry bug-fix test and the stub bug-fix test run in the slow lane `task test:bugfix-slow` (60 s a test); `docs/cycles-suite.md` names what the rows `bugfix-from-ticket` and `efforts` hold today, `docs/loom-run-settings.md` and `docs/workflow-metrics.md` say that `flags.recorded` is passed on as `--recorded`, and `sdc-experiment` gives the true reason for refusing `efforts.*` in a `loom` variant (the record format does not carry it to the runner yet). A caller's `efforts` that differs from the recorded effort diverges at the first call of a re-drive, as a changed model does (`docs/loom-run.md`).
- The live call cap of a re-drive counts a call from its start. Before, the replay adapter appended the line of a live call to `replay/calls.jsonl` only after the live program ended, so a live call that was ended before it finished (the action's time limit, a signal, a retry) was not counted and a second call also ran live at a cap of 1. The adapter now appends a line with `"phase": "started"` before it starts the live program, under a lock of the log, and the call's own line (`"phase": "finished"`) afterwards: two adapter processes at a cap of 1 start one live program between them, and a started line without a finish counts against the cap. A line without a phase (an older log) is a finished call. `replay.calls.live` counts started calls and `replay.calls.live_unfinished` those without a finish; a cap that cannot be read still starts no live call (`docs/loom-run.md`). The log is read as bytes, line by line: a line that is not valid UTF-8 or not a JSON object, a cut-off last line included, counts as one started live call (before, one byte that was not text read the whole log as empty and lifted the cap), the adapter writes a newline first when the last line of the log has none, and `replay.calls.unreadable_lines` counts the lines that could not be read.
- A bound program can no longer change who decides. The runner reads the table of the autonomy levels and the classes of decisions once, right after the bindings copy is made and before the first action of the run (`Decider::pin_routing`), and routes every decision of the run, the requester's classes included, from that read; a rewrite of the run's copy (`tools/autonomy-levels.json`, `driver/examples/sdc/decision-classes.json`, writable under `--confinement local`) changes no routing. The copy stays in the run directory as the record. A table that cannot be read at that first read still stops every decision for a person, at every level.

## [0.5.0] - 2026-11-22

The runner on the Loom engine. `runbook run <runbook>` runs a case in this process with Loom linked as
a library and the command `loom-run` is gone; `runbook test <suite>` runs a tree suite in process; the
model, rule and requester decider run in Rust; all fourteen model nodes of the bindings are the
program `sdc-node`; and `--dry`, `--bugfix`, `--bugfix-ticket`, `--goals`, `--watch` and
`--conformance observe` are built. `sdc-experiment run` starts runs only with `runbook run`. Breaking:
the command `loom-run` is gone; `sdc-experiment run` refuses a record with `runner: sdc` or no
runner; `sdc-experiment stop` takes no `--builds-dir`; the setting `decider.program` is refused; a
reject of a question or a finding by the decider is an invalid reply. The replay options of `runbook
run` are still refused. Loom is pinned at `5b3142c7` and Canon at 0.11.0. The longer note is
`docs/releases/0.5.0.md`.

### Added

- `runbook run --watch` starts `sdc-watch run <run directory> --kind loom --parent <runner pid>` as its child once the run directory exists and ends it by pid on every end of the run (closed, stopped, cancelled, a signal, an error); the pid is in `fs/watcher.json`, the watcher's output in `fs/watch.log`; the watcher ends itself within 5 s when the runner is gone (parent-death signal plus a poll every 200 ms); without the helper the run is refused with exit 2 before anything is written, and without `--watch` no watcher starts. `sdc-watch` reads a Loom run directory (`work/` is the root; `settings.json` marks it, or `--kind loom`): a change is attributed to the `tree run` step in flight (basis `step-window`, member `step`, read from `rounds.jsonl`), with the case and action of an open call record (`work/.sdc/llm/<NNN-action>/status.json`) and `null` where a Loom run cannot give one; the driver's attribution is unchanged. `ui/web/now.js` shows the step in the group header and the diff header and tells `step-window` from `none`. `task test:runbook` builds `watch/` first (`docs/fs-changes.md`, `docs/cli.md`, `story:driver-runtime-removed`).
- Rust tests of the runner for the cycle-suite entries that tested the old driver's own code: the decider's model name, `escalate: final`, citations (an entry, a line range, an id, an artifact address), the retry after a cite that does not resolve, the end after three invalid replies, `escalate` without `best`, a rejected question, the files of a decider's input, the obligations only a later step meets (`runbook/src/loom_run/decider/tests.rs`); the refusal of `decider.kind: person`, a rejection with no result to record it, a stop at a decision the decider cannot take, the stand-in principal (`runbook/tests/loom_run/`); and `sdc-run-metrics` on a run directory of the Loom runner (`driver/tests/run_metrics.rs`). `docs/cycles-suite.md` names the test for each entry. Where the runner differs from the cycle the test is written for the cycle and is `#[ignore]`d with the difference: `approved_on_checks` after three invalid replies, a rejection that names its case with exit 6, the decider's answer to a release at A3, a close at A5 taken by rule, a close at A3 asked of the decider, a stop reason with no person.
- `change.review` runs under Loom: `sdc-reply` reads its inputs through one source type (`Facts`), the driver's case directory or, with `LOOM_CASE` set, record files of the workspace under `.sdc/records/` (the log of the conformance run and of the gate about the tree they ran on, written by `sdc-tool.sh story-tests` and `ess-gate` through the new `sdc-reply record`; the review's earlier rejections, written by `check-review`), and the fixture adapter counts the rejections from the workspace; the old driver is unchanged. Seven cycles of `cycles/tree-suite.yaml` move from pending to converted (`review-reject-once`, `review-minimal-input`, `review-invalid`, `review-spent`, `review-invalid-named`, `implement-request-review-rework`, `coverage-complete`): 27 converted, 23 pending, 80 not converted (`docs/cycles-suite.md`, `docs/loom-bindings.md`).
- `sdc.story` is revision 8 and `sdc.intake` revision 10 (bindings `sdc-story@8`, `sdc-intake@10`): under Loom the requirement-coverage refusal of `change.implement` and the writer's own refused apply of `questions.answer` send the writer back with the refused run's output. `sdc.story` gains the claim `change.coverage_refused` (result `coverage_refused` of `implementation_change`), which keeps `tests.run` off that revision; `sdc.intake` gains the evidence kind `answer_applied` produced by `questions.answer`. `bindings/tools/llm-node.sh` runs `sdc-reply check-coverage` after the writer only under Loom (exit 8 on a refusal); the driver keeps its postcondition and its records. Five cycles of `cycles/tree-suite.yaml` leave `pending` (`coverage-not-built`, `coverage-line-outside`, `coverage-test-missing`, `coverage-spent`, `writer-own-error`): 25 converted, 25 pending.
- (`runbook loom-run` is gone at this release and `--in-process` with it: `runbook run` makes the run in the process, see Changed.) `runbook loom-run --in-process` runs the case tree through Loom as a library in the process (the commit `Cargo.lock` pins, no `b10x-loom`, `canon` or `sdc-drive` process) with one copy of the run loop, the decider, the ledger and the run directory writer behind a `LoomHost` trait and the one `engine::calls` module; `--loom --loom-sha256` stays the child, exactly one of the two is required, `outcome.json` holds `null` for `loom_exit_code` and `stderr_tail`, a run records the pinned `commit` in place of a binary `sha256`, and a bound program gets `PATH` and `LOOM_*` only; `sdc-experiment` accepts `loom.in_process: 'true'` in a `runner: loom` record and refuses one that names both ways; `task test:loom-in-process` compares the stub cycle through both ways (`docs/loom-run.md`, `story:engine-on-loom-library-entry`, slice 2).
- `runbook::engine` is a second runtime of the daemon: it runs a case tree through Loom's library calls (`open_tree`, `run_tree`, `approve`, `close`) in the daemon's process, linked as pinned git dependencies, with no `b10x-loom`, `loom`, `canon` or `sdc-drive` process; `runbook serve --runtime loom|stub` (default `stub`) and `--runbook-home` select it (`docs/engine.md`, `docs/library-entries.md` "States at `5af3f9c2`"); `task test:engine-real` runs the `sdc` stub cycle through it.
- `runbook test <suite file> [--scratch <dir>] [--only <name>,...] --confinement <local|substrate>` runs the cycles of a `loom.tree-suite/1` file through Loom's suite runner (`run_suite`) in its own process, prints one line per cycle, the ten slowest and `N of M as expected, P pending`, and exits 0 only when every cycle that ran is as expected (1 not, 2 refused with Loom's reason and place, 64 usage). `--confinement` has no default and `substrate` is refused (`docs/cli.md`).
- The crate `runbook` (the engine, `runbook::engine`) links Loom `2e74232a`, the commit with the tree suite runner (`b10x_loom_tree::suite`), in place of `5af3f9c2`; no signature the engine uses changed, the gate lane passes (396 tests) and the slow lane `task test:engine-real` passes on both pins.
- `cycles/tree-suite.yaml` (`loom.tree-suite/1`, seeds in `cycles/seeds/`) states the 130 entries of `cycles/suite.yaml` for `runbook test`: 10 converted and as expected, 25 pending (7 wait for a Loom story and are not run, 18 run and still fail), 95 not converted with their class, all in a table in `docs/cycles-suite.md`; `task test:cycles-loom` runs it (279 s wall for the whole file, a slow lane); `bindings/tools/llm-node.sh` reads a seeded workspace directory `.stub` as the stub's fixtures and findings; `cycles/suite.yaml` and `scripts/run-finding-cycles.sh` stay unchanged.
- The crate `runbook` links Loom `5bcc26d9` (`handoff_when` of a program binding) in place of `2e74232a`. One member of a struct the engine builds changed: `NextDecision` has `principal`, which `engine::calls::decision_from_json` now reads from the printed decision; no signature of the crate changed. `bindings/sdc-intake.yaml`, `sdc-direction.yaml`, `sdc-spec.yaml`, `sdc-plan.yaml` and `sdc-story.yaml` say `handoff_when` for `amend.upstream`; `sdc-direction.yaml` gives `amend.upstream` a `retry_when` on `context_defaults: [finding]`. In `cycles/tree-suite.yaml` `finding-program`, `offline-decision`, `bugfix-full` and `bugfix-never-red` are converted (14 of 130); `bugfix-not-red` and `bugfix-edited-test` run and are pending (`docs/cycles-suite.md`).
- The scripted replies of a model node are seeds in `cycles/seeds/stub/` (`settings/`, `replies/`) that `bindings/tools/llm-node.sh` reads from `.stub/` under Loom (a variable already set wins, a run without `.stub/` is unchanged); with them `cycles/tree-suite.yaml` has 24 of the 130 entries converted and as expected, 30 pending (5 wait for a Loom story and are not run, 25 run and still fail) and 76 not converted; the whole file ran 940 s with `24 of 24 as expected, 30 pending` (`docs/cycles-suite.md`, "The layout of `.stub/`").
- `sdc.bugfix` records `dissection_written` (by `bug.dissect`) and `repro_written` (by `test.reproduce`), and `dissection.check` and `repro.run` wait for them, so a check does not run on a document that does not exist yet (Loom ended such a case `not_reproduced`). `bindings/sdc-bugfix.yaml` and the JSON bindings carry the records; `gate.run` is 3600 s in the YAML file (Loom's limit).
- `runbook run`: the model and rule decider run inside the runner (`runbook/src/loom_run/decider/`: rules, prompt, inputs, call, validate, approval, ask) and no longer start `bindings/tools/decider.sh`, `jq`, `yq`, `rsync` or `bash`; the only program a decision starts is the adapter of `decider.adapter`, which gets `PATH`, `HOME`, `SDC_REPLY` and `SDC_CLAUDE_CREDENTIALS` and no `SDC_DECIDER_*` or `SDC_LLM_*_DECIDER` variable (no adapter reads them). `decider.program` is refused. The slow lane `task test:decider-parity` runs `decider.sh` and the Rust decider on 90 invented cases (every kind and request type) and compares the reply, the record directory files and the cost and decision lines; it and its fixtures are deleted with `decider.sh`. A reject of a question or finding is an invalid reply, and the rule `approve` answers a question with the request's own default.
- `runbook run` takes the options of the old driver's launcher for the six ports not built yet, with their old names (`--dry`; `--recorded`, `--on-divergence`, `--live-adapter`, `--live-cap`, `--inject`, `--set`, `--take-recorded`, `--source-tree`; `--bugfix`, `--bugfix-ticket`; `--goals`; `--watch` (built since, see above) and `--conformance off|observe` in place of the variables `SDC_WATCH` and `SDC_CONFORMANCE`), parsed into `Args.ports` and each refused with exit 64 and `<option> is not built yet`; `--set` in process is a replay condition (it was refused as detach-only) and needs `--recorded`. One function per port holds the refusal (`runbook/src/loom_run/{dry,replay,bugfix,goals,watch,conformance}.rs`) At this release the replay options (`--recorded`, `--on-divergence`, `--live-adapter`, `--live-cap`, `--inject`, `--set`, `--take-recorded`, `--source-tree`) are the ones still refused; `--dry`, `--bugfix`, `--bugfix-ticket`, `--goals`, `--watch` and `--conformance observe` are built, see the items below and above (`story:driver-runtime-removed`).
- `sdc-node` runs `change.implement` (session, coverage and request mode); with it all fourteen model nodes of `bindings/*.yaml` are the Rust program, and the three step log lines it adds are files (`instructions/shared/node.coverage.invalid.md`, `node.implement.api-ended.md`, `node.implement.nothing-recorded.md`).
- `runbook run --goals <file>` copies the goals document unchanged to `work/goals.yaml` of the run before the tree opens (a path that is no file: exit 64), and the setting `decider.requester: true` (kind `model`, needs `--goals`) makes the decider answer the requester-side classes of `decision-classes.json` as `decider:requester`, in the runner: the call directory, the in/ set, the prompt text of `instructions/decider/requester*.md` and the grounding check by the program `tools/decision-grounds.sh` are what `decider.sh requester` made (a scope question's request comes from `sdc-decision-request` beside `sdc-reply`). The run's bindings copy carries the checkout's `tools/` beside it for that program. `task test:decider-parity` compares the requester's cases (an approval, a close and a question: an option, a free answer, no ground under stop and final, refused grounds, a missing reply, the dry adapter) too. The cycles `requester-decider` and `requester-no-ground` are held as fake-Loom tests in `runbook/tests/loom_run/goals.rs`; under the real engine they wait for a program on a person's action (`docs/cycles-suite.md`).
- `runbook run --dry` is built (it was refused as not built yet): the stub cycle with every model node of the bindings copy on the adapter `dry`, a model decider on its own `dry` adapter, and with no decider in the settings the rule decider `approve` at autonomy `A5` playing the person (the person steps are bound to `decider:approve`). Every command a dry call would have run is printed (`  would run: metaharness ...`) and appended to `dry-commands.txt` of the run directory; a dry call costs 0, so the spend ledger stays 0 and no login file is read; `settings.json` and `run-settings.jsonl` carry `dry: true` and `stand_in`. `--confinement`, the cap and the bounds stay required, a settings file that names an adapter is refused with `--dry`, and `adapter: dry` without it is refused (`runbook/src/loom_run/dry.rs`). The cycle entries `models` (per class, one model for all, per binding) and the decider's effort of `efforts` are held as Rust tests in `runbook/tests/loom_run/dry.rs` (`docs/cycles-suite.md`); the effort per node class stays open: it needs an `--effort` argument on the model node.
- `sdc-experiment` accepts `flags.dry: 'true'` in a `runner: loom` record and passes `--dry` to `runbook run`; `adapter` and `decider.adapter` are refused beside it.
- `runbook run --conformance observe` is built: after a run that ended closed the product in the run's `work/` is held to the specification's conformance suite and `oracle.json` is written into the run directory; it decides nothing, and a failure is the file's `unknown` with a cause or one line, never a change of the exit. The code moved from `driver/src/bin/sdc-conform.rs` into the library (`driver/src/conform.rs`, `sdc_tools::conform`), which the program now only calls (its output is unchanged); ess is run through the new `sdc-tool.sh ess` subcommand. `sdc-experiment` passes a `conformance` setting of a `loom` variant as `--conformance`. The cycle `conformance-default-off` is held by the Rust test `conformance_default_off` (`story:driver-runtime-removed`).
- `runbook run --bugfix <seed dir>` and `--bugfix-ticket <file>` open a bug-fix case: root `BUG-1` on `sdc-bugfix` (the runbook argument `sdc` or `sdc-bugfix`; another runbook or root protocol is refused), seeded before the tree opens by the helper `sdc-bugfix seed` run in `work/` (found as `runbook doctor` finds the helpers); a ticket's text reaches `work/` only, never a stream or another file of the run; a failed seed ends the run `error` with the last three lines the seed printed. `sdc-experiment` starts a `seed` input of a `loom` variant through `--bugfix` with `sdc-bugfix` as root; the `bugfix-from-ticket` cycle is held by Rust tests in `runbook/tests/loom_run/bugfix.rs` (`story:driver-runtime-removed`).

### Changed

- The crate `runbook` links Loom `23f1f4b3` (branch `wave/2026-11-21-w7` of the engine, which the forge serves) in place of `5bcc26d9`. Two members the crate builds or matches changed: `RunSettings` has `cgroup_root` (set to none) and `TreeStop` has `Cancelled`, which a run ends with when its cancel handle is set (`stop_status` 5, as for a spent budget; `Finish::Cancelled`). The child binary of the slow lane `task test:loom-in-process` is the one of `23f1f4b3` (`~/.local/state/runbook/loom/23f1f4b3/b10x-loom`).
- `runbook run <runbook>` without `--detach` makes the run in this process with the code that was `runbook loom-run`, and the subcommand `loom-run` is gone. The runbook id is the positional argument; `--bindings`, `--protocols` and `--root-protocol` default to what the asset root holds for that id and each can still be given; `--case` defaults to `IDEA-1`; Loom is a library of the process unless `--loom` is given with `--loom-sha256` (the child way), and `--in-process` is gone. `--intake <file|->` takes the idea's text and the runner writes the idea file the tree reads (`--idea` is gone: a text that is already a mapping with `text` is kept as it is). `--run`, `--runs-dir`, `--confinement`, `--settings`, `--spend-ledger`, `--cap-micro-usd`, `--max-llm-calls` and `--max-wall-seconds` stay required, a missing one is a usage error (64) that names it; exit codes, run directory files and events are what `loom-run` wrote. `--detach` is unchanged. `sdc-experiment run` starts a `runner: loom` variant with `runbook run sdc` of the record's build and refuses a build without the in-process `run` in one line (`docs/loom-run.md`, `docs/workflow-metrics.md` § 7.1; `story:driver-runtime-removed`).
- `sdc-experiment run` starts a run only with `runbook run` (`runner: loom`; `runner: free-agent` still starts `sdc-free-agent`). A variant with `runner: sdc` or with no `runner` is a record of the old driver: `check` and `report` still read it (the records under `experiments/` are history and `check --all` is unchanged), `run` refuses it with one line that says the old driver is gone and names `runner: loom`. `check` cannot tell a new registration from a planned old record, so it does not refuse a missing `runner`. `stop` has one path for every run: the stop record, then `systemctl --user stop <unit>` (SIGTERM to the run's process, never a process name), and no longer calls `scripts/run-trial.sh --stop`; `stop` takes no `--builds-dir`. A loom variant's refusals of `flags.dry`, `flags.recorded`, `conformance` and of a `seed` input are one function each in `sdc-experiment.rs`, marked `// port: <name>`, for the port of that option into `runbook run` to remove.
- Every model node of `bindings/*.yaml` starts `sdc-node` (14 argv entries moved off `tools/llm-node.sh`). `runbook` puts `--tools-dir <run copy>/tools` into each of them when it copies the bindings, ships no `llm-node.sh`, and finds the model nodes by the name `sdc-node`.
- `bindings/tools/llm-node.sh` serves the old driver only: Loom mode (`LOOM_CASE` set) is refused with exit 126, and its dispatch to the program and `SDC_NODE_SHELL` are gone.
- The tests of `sdc-node` (`driver/tests/sdc_node.rs`, formerly `node_parity.rs`) start the program alone and compare exit code, both streams and the files it wrote with the committed `driver/tests/expected/<case>.txt`; the expectations were written from the script's runs, which the program equalled. The golden prompt test of the agent nodes starts the program once per node instead of the script in old-driver mode (about 31 to 38 s before). The goldens of the coverage and review-retry prompts carry the real schema and refusal reason; `change.fix.feedback-run.md` is deleted with its case.
- The crate `runbook` links Loom `5b3142c7` (branch `wave/2026-11-21-w7` of the engine, on the forge) in place of `23f1f4b3`, and the Canon pin moves from 0.10.0 to 0.11.0 (`driver/canon-pin.json`, tag `0.11.0`, commit `e81f970d`; `protocol/simulate/CANON_REV`) in the same change, because Loom `5b3142c7` pins Canon 0.11.0. No source of `runbook/src/` changed: `run_handoff_check` gains `cancel: Option<&Cancel>` and the crate has no call of it (Loom's `run_tree` makes the `handoff_when` checks and holds the cancel the crate passes it), so a cancel now ends a running `handoff_when` check: refusal `Cancelled: action <action>: the check <program> was cancelled`, status 5, `next: [<action>]`, `Finish::Cancelled` (`a_cancel_while_a_handoff_check_runs_ends_the_check_and_the_run_and_starts_no_later_step`, `docs/engine.md`, `docs/loom-run.md`). The 9 protocols and the 52 `decision.json` of `cases/` pass Canon 0.11.0 unchanged but for `computed_from.semantics.canon`; the check ledger and the site data carry the new Canon commit. The child binary of the slow lane `task test:loom-in-process` is the one of `5b3142c7` (`~/.local/state/runbook/loom/5b3142c7/b10x-loom`).

### Fixed

- The runner's decider resolves a reply's artifact address (`<artifact>@<revision>#<entry>`): the case directory it makes for a decision with a path now holds the `bindings.json` those addresses read. A stop at the decider now names the step and the case (`<step> in <case>: <reason>`).

## [0.4.1] - 2026-11-22

A patch release of 0.4.0, the same day: a fix, checks and tools, and records; no protocol or behaviour
change of the product. A model step of `runbook loom-run` that started no adapter costs nothing, the
checks of a run and the ESS specification run on ESS 0.97.3 with the daemon's synthesized conformance
suite committed, `sdc-check-unreferenced` is a new check, the two evaluation tools read the run
directory of `runbook loom-run`, and the UI test children end with their parent. The longer note is
`docs/releases/0.4.1.md`.

### Added

- `sdc-check-unreferenced` and `task check:unreferenced` (part of `task check`) list the tracked files of `scripts/`, `bindings/`, `instructions/`, `cycles/`, `cases/`, `tools/`, `watch/`, `views/`, `decision/`, `goals/`, `eval/` and `driver/tests/fixtures/` that no other tracked text file refers to, and exit 1 when there are any; the history files (`.engineering/`, `experiments/`, `CHANGELOG.md`, `docs/releases/`) do not count as referrers; `--without <path>...` lists the files that only the given paths refer to. `driver/unreferenced-keep.txt` lists 26 files the first run found, as unexamined, and `driver/unreferenced-whole.txt` 10 directories a program reads whole; nothing is deleted (`docs/releasing.md`, "The unreferenced-files check").
- The record `experiments/parity-loom-strong-3.yaml` (the strong setting through Loom on tree `e7dfa4b58`: all three runs passed `scope.answer`, none closed; run costs 1.761, 0.565 and 1.766 USD) and the result of `experiments/release-0.3.1.yaml` (nine runs, eight closed and one stopped at `writer_spent`; clean passes strong 3 of 3, mixed 3 of 3, haiku 1 of 3; 26.478 USD in all). Both records are descriptive and decide nothing; the run summaries are under `experiments/<id>/runs/`, the reports beside them. No run metric of the product is affected.

### Changed

- The ESS specification of the daemon is pinned to ess 0.97.3 (`requires: ess 0.97.3` in `ess/ess-inputs.yaml`); its tasks run inside `ess/`, where the dispatcher switches to the pin. The synthesized conformance suite is committed (`ess/generated/conformance/suite.json`, `DIGEST`) and its fast digest check is part of `task check:ess-generated`; `task check:ess-suite:slow` synthesizes it again and compares bytes.
- `driver/ess-pin.json` pins ess 0.97.3 (tag `0.97.3`, commit `cd9698fd65`; archive sha256 equal to the release's `SHA256SUMS`) instead of 0.97.0; `sdc-conform ess-install` and `ess-path` resolve it and the conformance tests of `driver/` pass with it. Neither breaking change of 0.97.3 (`--component` scopes synthesis and the report to one component; read-grant scenarios no longer copy the reads of other views) moved a committed file here: nothing outside `ess/` is written by an `ess`, the fixture specifications under `driver/tests/fixtures/conformance/` are read only, and no tool here passes `--component`. The product checks of a run (`ess-gate`, `story-tests`, and the other `ess` calls of `bindings/tools/sdc-tool.sh`) now run ESS 0.97.3 (`ESS_TOOLCHAIN` default) instead of 0.96.0: validate, compile, synthesize and generate give the same output and exit codes on every fixture specification, and the interpreted and `billing` conformance lines and `run` reports name `0.97.3` where they named `0.96.0`. A record that compares runs across this change (`runbook parity`, `sdc-run-compare`, experiment reports) says so; runs and recordings made before it carry 0.96.0.
- `sdc-finding-kpis` and `sdc-run-timeline` read the run directory of `runbook loom-run` as well as the driver's, detected from its files (`settings.json`, else `store/<case>/events.jsonl`) and printing the same shape; a figure a Loom directory has no source for is `null` with a reason, and a directory of neither kind is refused with exit 2 (`docs/loom-run-directory.md`, "The two evaluation tools").

### Fixed

- The UI test step no longer leaves its servers running when it is ended: every server, fixture player and daemon a UI spec starts (and the two servers of the docs-site tests) goes through `ui/test/child.mjs` (`spawnChild`, `startServer`), whose guard (`ui/test/guard.mjs`) ends the child's process group when the process that started it is gone, also after SIGKILL, within 0.1 s of polling; two runs ended at their 120 s bound had left `node server.mjs --port 0` processes alive for 36 and 77 minutes, holding the checkout, ports and memory (`story:ui-test-servers-end`). A server started by hand is unchanged.
- The digest of the sources of the committed ESS suite (`task ess-suite:digest`) no longer hashes the text the host's `ess --version` prints, so `task check:ess-generated` no longer fails on a host whose `ess` on `PATH` prints more or less than the pin (the recorded digest had been made with a dispatcher that printed two lines); the pin is the `requires: ess` line of `ess/ess-inputs.yaml`, which the digest hashes as a source. `task ess-suite:record` and `task check:ess-suite:slow` run `task ess-suite:pin` first and refuse when the `ess` that runs in `ess/` is not that release.
- `runbook loom-run`: a model step that ended before it started any adapter (its call directory has no `request.json`, which `llm-node.sh` writes before every adapter start, and a `status.json` with the step's exit) costs nothing: its reservation is released instead of booked, an event `no_model_call` names the call and the exit, and the run goes on with what Loom says of the step instead of ending `error` at the cost reader; a directory with a `request.json` and no cost record, and one without a readable `status.json` exit, stay fail-closed as before (`docs/loom-run.md`).

## [0.4.0] - 2026-11-22

The Runbook daemon and the runner on Loom. `runbook serve` runs a daemon with an HTTP API specified in
ESS, event streams, MCP and an embedded page with a decisions drawer, on a stub runtime; `runbook run
--detach`, `runbook runs` and `runbook decide` are its client. `runbook loom-run` runs one case on a
pinned `b10x-loom` binary, `sdc-experiment` knows `runner: loom`, and `runbook parity` reads a
loom-run directory. The longer note is `docs/releases/0.4.0.md`.

### Added

- `runbook loom-run` gives the decider's answer where the driver does: `bindings/tools/answers.yaml` (format `runbook.loom-run-answers/1`) declares per action which program prints the requests, which writes one answer and which takes the step, so a model run passes `scope.answer` (a scope-changing question) and the `amend.upstream` findings that need a person instead of ending `decider_cannot_decide`; each answer is one decider call against `decider.max_calls` and the spend ledger and one line of `decisions.jsonl`, escalate acts as in the driver, an undeclared action keeps `CannotDecide` and `person:owner`; the new `sdc-answers` helper ports the driver's `yq` logic and `sdc-experiment run` refuses a `loom` launch when a program the answers file names is not on the unit's `PATH`; Loom's approval of `scope.answer` lists no result, so it cannot carry the answer text (`docs/loom-run-decisions.md`).
- A model step under Loom on the metaharness adapter finds its login (`--claude-credentials`): the adapter gives metaharness a private `HOME` with a copy of the login, for that process alone, only when its own `HOME` has no login, and the copy of both adapters lies in the runner's private directory (never in the run directory), removed after the call and by the runner after a SIGKILL; and `runbook loom-run` treats a model call that exits with an error and has no cost as the driver does (reservation booked, the run goes on), with a reason that names the action, the adapter, the exit and the first line of its stderr where the run ends (`docs/loom-run.md`); `experiments/parity-loom-strong-2.yaml` replaces `parity-loom-strong`.
- `runbook loom-run` is pinned to Loom `5af3f9c2` and runs every decision command Loom prints as printed: a rejection is `tree approve --action <a> --result rejected --reasons-file <file>` with the decider's text in a file of the run directory (no login, token or home path), an approval names `--result performed` where the action declares results, a `Close` runs `tree close --outcome <o>` and a `Handoff` runs its printed `tree run`; a result the bindings do not declare ends the run with a named error. The person steps `context.approve`, `vision.approve`, `roadmap.approve` and `merge.approve` declare `rejected`, so their writers are sent back with the reasons in `LOOM_RETRY_FEEDBACK`. `stub-cycle` through `runbook loom-run` on it compares `parity: identical` with the driver (`docs/parity.md`, "Identical comparison on Loom `5af3f9c2`").
- `sdc-experiment run <record> --login-file <path>` names the model login for one launch (every `loom` variant, as `loom.login_file`) without a record holding it; `run --print` shows it as `<loom.login_file>`.
- `experiments/parity-loom-strong.yaml` registers the three evaluation runs through Loom (the strong setting of `release-0.1.2`, spend cap 20 USD, stop line 19 USD); the tolerance is in `story:parity-run-on-loom`.
- `runbook loom-run` runs one case tree through the `b10x-loom` binary as a child process: the binary's sha256 is checked, the child gets a built environment, `--confinement` has no default, every decision goes to a decider and its printed command runs as data, and a spend ledger under a file lock, a model-call cap, a wall-time cap and SIGTERM end a run with their own outcome and exit code; it fails closed on an unreadable ledger, a missing cost or an unknown exit code of Loom (`docs/loom-run.md`, `story:runbook-loom-run-command`).
- Decider and bindings copy: `runbook loom-run` answers the decisions of a tree with a model or rule decider (`decider.sh` of the run's own copy of the bindings) and rewrites the copy only (principal, models, turns, budgets, adapter, login link); the decisions table and the settings are in `docs/loom-run-decisions.md`, the one settings format `runbook.loom-run-settings/1` in `docs/loom-run-settings.md`.
- Run directory: `RunDirWriter` writes the files `sdc-experiment report` and `runbook parity summarize` read; layout, member sources and what a Loom run does not record are in `docs/loom-run-directory.md`.
- `sdc-experiment` knows `runner: loom`: a prospective record starts a model run through Loom (`runbook loom-run` of the record's tree) as a unit with the base environment and no `SDC_` variable; `run --print` shows the unit, the exact command and the settings, a missing `loom.*` setting or a setting of the old driver is refused by name, the login file is named by path in the settings file and never printed, and `stop` writes the stop record before it stops the unit (`docs/workflow-metrics.md` § 7.1, `story:experiment-runner-loom`).
- A model run goes through one chain: `sdc-experiment run` writes the one settings file `runbook.loom-run-settings/1` (decider kind, autonomy, `without_claims`, escalate, the decider call limit, the adapters), `runbook loom-run` reads it whole, and `sdc-experiment status` and `report` read the run directory it leaves. One committed fixture, `runbook/tests/fixtures/loom-run-settings.json`, is the file the writer produces and the file the runner runs; no test builds the other crate (`docs/loom-run-settings.md`, `story:experiment-runner-loom`).
- `runbook loom-run` keeps the login file's path and any link to it out of the run directory: the link lives in `$XDG_RUNTIME_DIR/runbook/<run>/` (mode 0700) for the length of the run, is refused when `XDG_RUNTIME_DIR` is unset or not the user's, and is removed on a normal end, on an error and on SIGTERM or SIGINT; a SIGKILL leaves the directory and `docs/loom-run.md` says what is then to do. `docs/loom-run.md` also states how far one call can end above its budget and what a reservation left by a killed run does to the next run on the same ledger (it only lowers what is left), each with a test.
- `runbook parity summarize --loom-run <run dir>` reads the run directory of `runbook loom-run` whole: the settings, the decisions and the digests of the protocols the status does not hold, and the copies' action, subject and revision from the target case's evidence; `parity compare` ends with `parity: identical`, `parity: equal on what both sides record` or `parity: different`. The stub cycle through `runbook loom-run` on the real `b10x-loom` (`--confinement local`, scripted approvals, no model) compares `parity: identical` with the driver's, exit 0; `task test:loom-real` is the slow lane that makes that run (`docs/parity.md`, `story:parity-run-on-loom`).
- The page served by `runbook serve --ui` lists the open decisions of the selected run in its Decisions drawer and answers one with the daemon's own `answer-decision` (bearer token and JSON, same origin, never the cookie; the token is kept in the memory of the page only), and the stub runtime has the preset `stub-decision`, a run that waits on two decisions (`docs/daemon-api.md`, "The web UI", `story:ui-embedded-in-the-binary`, slice 2).
- `runbook mcp` and `runbook serve --mcp` expose the run commands as MCP tools (`submit_run`, `list_runs`, `read_run`, `read_events`, `answer_decision`, `cancel_run`) on the command layer of the daemon: over standard input and output as a client of a running daemon (`--daemon`, `RUNBOOK_DAEMON`), and over streamable HTTP at `POST /mcp` behind the daemon's token check; MCP revision 2026-07-28, with `initialize` for the two revisions before it (`docs/mcp.md`, `story:mcp-exposure`).
- `runbook serve` runs the daemon (`docs/daemon-api.md`, "Running the daemon") on a runtime trait with a stub runtime: the five client commands and the twelve views of `runbookd.yaml` over HTTP and JSON, a journal that survives a restart, a bearer token that every route checks, and `406` for an event view asked as a stream; the runtime is a stub and streams, the UI route and `runbook run` come in later parts (`story:daemon-skeleton-on-a-runtime-trait`, part C1).
- The daemon ends a run that was `Running` or `WaitingOnDecision` when it stopped and whose runtime holds no live run (the stub always) as `Ended` with the outcome `orphaned` and withdraws its Open decision requests, instead of leaving it live (`Runtime::recover`, command `EndOrphanedRun`, event `RunOrphaned`; the run's stream event is `runbook.run-orphaned`, no longer `runbook.orphaned`); a second answer to a decision request is `DecisionAlreadyAnswered` whatever its run is doing; a repeated submit with a known idempotency key names the first run in its body (`published[0].payload.run_id`) as well as in `Location`; the intake limit is stated as 1,048,576 bytes (`docs/daemon-api.md`, `story:daemon-api-specified-in-ess`, decisions 16 to 19).
- The three event views of the daemon (`events`, `session-events`, `daemon-events`) answer as a server-sent event stream when asked with `Accept: text/event-stream`, in place of the `406` of part C1: the SSE `id:` is the event's position, `Last-Event-ID` or `after_position` resumes without loss or repeat, a run stream ends with its run, a session stream with its session, every stream at shutdown, and a client that falls more than 1,024 events behind is closed and resumes by position (`docs/daemon-api.md`, "Event streams"; `story:daemon-skeleton-on-a-runtime-trait`, part C2).
- `runbook serve --ui` serves the web UI from files compiled into the binary (`ui/web/`), beside the API, behind the daemon's token rule: `GET /` is the page, `404` without `--ui`; the page reads the runs and their events from the daemon when it is served by it (`docs/daemon-api.md`, "The web UI")
- `runbook run --detach`, `runbook runs list|show|events|cancel|decisions` and `runbook decide` call a daemon (`--daemon <url>` or `RUNBOOK_DAEMON`, plain HTTP; a token from `--token-file` or `RUNBOOK_TOKEN_FILE`, mode 600): text by default, the route's JSON with `--json`; `runs events` prints the lines of `curl -N` and resumes with `Last-Event-ID` after a dropped connection; exit codes 64 usage, 65 refused, 66 not found, 67 conflict, 69 unreachable, 77 denied, 78 config; `runbook run` without `--detach` exits 64 until `story:engine-on-loom-library-entry` (`docs/cli.md`, `story:run-in-process-and-cli-as-daemon-client`, client part).
- The daemon's one guard now also refuses, on every route with or without `--ui`: a `Host` that is not the listen address on a loopback listener, and an `Origin` that is not the request's own host, with `403`; a command or an MCP call that is not `Content-Type: application/json` with `415`. The cookie of the UI login no longer holds the token (the SHA-256 of a fixed prefix and the token), is refused as a bearer token, and every method on `/ui/login` goes through the guard (`docs/daemon-api.md`, "Requests from a browser"). The build refuses a symbolic link in `ui/web/`.
- The record of release 0.3.1: `approval-record:release-0-3-1` and `experiments/release-0.3.1.yaml` (the to-do idea in three settings, three runs each, on tag 0.3.1's tree, cap 30.00 USD), registered before any run; the record holds no run summary in this tree.

### Changed

- The view `open-decisions` carries `case_id`, `action` and `capability` of each request, so the Decisions drawer of the page shows which step asks (as text), and the new MCP tool `read_open_decisions` (the seventh; there was no MCP read of open decisions) returns the same rows (`docs/daemon-api.md`, `docs/mcp.md`).
- `runbook serve --principal person:<id>|agent:<id>` is required (no default, no environment variable; a missing or malformed one is a start refusal, exit 1) and fixes the daemon's principal for the life of the process. `answer-decision` hands the runtime that principal and never the typed `answered_by`, which stays a label of at most 200 characters; the `DecisionAnswered` event, the `decisions` view and the MCP answer carry both (`answered_by`, `principal`), and `GET /daemon/views/configuration` names the principal. A state file written before this change still loads (`principal` is then absent in the view; `docs/daemon-api.md`).
- `answer-decision` refuses an `answered_by` longer than 200 characters with `400`, like `client` and `idempotency_key` (the specification says it as the type `sdc.daemon.AnswererLabel`, `docs/daemon-api.md`), and the stub runtime puts at most the first 200 characters of the intake into each text it makes, so that one intake of 1 MiB no longer fills the `open-decisions` answer of `stub-decision`.
- `runbook serve` refuses to start with a token shorter than 32 bytes (exit 1; the message names the minimum and `--token-file`, never the token); a client takes the token it was given (`docs/daemon-api.md`, "Running the daemon").
- The specification of the daemon (`ess/domains/daemon.yaml`) now says what the daemon does: `SubmitRun` writes the run's first event (`runbook.submitted`, sequence 1, so the runtime's first record is sequence 2), and `CancelRun` and `EndOrphanedRun` withdraw the run's Open decision requests in the same step (an answer to one is `DecisionWithdrawn`); all 12 scenarios of the daemon now pass against the running daemon and on the interpreted target, where 6 failed against the daemon before (`docs/daemon-api.md`; `story:daemon-synthesized-suite-has-no-refusal`, part C4).

### Fixed

- `sdc-experiment status` says `unreadable: <reason>` for a run directory that holds `outcome.json` and cannot be read, and no longer `not started` (found by running the chain of a launched run by hand).

## [0.3.1] - 2026-11-22

A step that waited for the build pool is no longer recorded as a failed test run (the story tool and
the bug-fix tool), which is why the registration runs of 0.3.0 were stopped and are repeated on this
release. It also holds `runbook check`, the cycle script's `--only` option, the driver's test budget,
instruction files for more prompts and tool texts, `runbook parity` and the first equal comparison of
`stub-cycle` on Loom, and the ESS specification and plan of the daemon, without the daemon itself.
The longer note is `docs/releases/0.3.1.md`.

### Added

- The ESS domain `sdc.daemon` specifies the API of the Runbook daemon before any handler exists: `ess/domains/daemon.yaml`, 10 scenarios under `ess/scenarios/daemon-*.yaml`, the generated JSON Schemas and the OpenAPI document `runbookd.yaml` under `ess/generated/`, `task check:ess-generated` (part of `task check`) holding the committed output current, and `docs/daemon-api.md`. The daemon, `runbook serve` and `runbookd` are not built (`story:daemon-api-specified-in-ess`).
- `scripts/run-finding-cycles.sh <scratch dir> --only <name>[,<name>...]` (and `task cycles:only SCRATCH=<dir> NAMES=<names>`) runs the named finding cycles alone: a cycle that reads an earlier one's output runs that one too and says so, an unknown name ends it with exit 2 before anything runs, and the verdict line says how many were selected (`docs/cycles-suite.md`, "Running named cycles").
- `runbook check` reads the text of a checkout and prints one finding per line, with the file and the fix: the text layout, the two forms of bindings (they parse, name the same existing instruction files per action and run executable tools), the placeholders of the instruction files, the view files, and `cycles/suite.yaml` against the arrays of `scripts/run-finding-cycles.sh`; it exits 1 on any finding and has `--json`. `task check:cycles-suite` runs it instead of its shell, and `CYCLES_SUITE` is replaced by `RUNBOOK_HOME` (`story:runbook-check-of-the-text-layout`).
- The run inspector reads what a change of a runbook's records means (`file_kinds.parsers`) and the decisions a person can take (`decisions`) from the view file too, a run is shown with the view that covers its protocols and never with another runbook's, a run no view covers says which view is missing, and a view with a member the inspector does not know is refused with a message that names it (`ui/README.md`, `story:ui-view-data-parsers-and-actions`).
- The first comparison of `stub-cycle` under the old driver and under Loom `2c0a6f48` is equal on what both sides record (8 cases, 69 evidence records, 54 program runs, 5 person steps, 62 product files; six members Loom's status holds nothing for are named, not compared); `docs/parity.md` holds the facts, what "equal" does not cover and the items of the first-run list that stay open (`story:parity-first-run`, `story:driver-tools-find-binaries-without-build-dir`).
- `runbook parity compare` reads a protocol id as its registration stem (`sdc.intake` equals `sdc-intake`), neither compares nor counts the revisions of a child's `case_outcome` record, and `parity summarize` leaves `.sdc/loom-feedback/` out of the product tree; all three are named rules in `compare --rules` and `docs/parity.md` (`story:parity-first-run`).
- `runbook parity summarize|compare|approvals` reads a finished run of the old driver into a runtime-neutral summary (`runbook.parity-summary/1`), compares two summaries by a table of rules (`compare --rules`) and lists the person steps a second runtime must repeat; it is the driver's half of the comparison with a run under Loom (`story:parity-run-on-loom`, `docs/parity.md`).
- `runbook parity summarize --loom-status <file> [--product-tree <dir>]` reads the JSON of Loom's `tree status` (`loom.tree-status/1`) into the same summary: a status of another format is refused by name, a member it needs and does not find is an error that names the member and the case, and what the status holds nothing for is listed under `not_recorded`, which `compare` reports as `not recorded on one side` and does not compare (`story:parity-run-on-loom`, the table in `docs/parity.md`).

### Changed

- The build pool wait no longer treats a job as started because slot's `queued` line has not appeared within 3 s: on a loaded machine that line came late, the wait bound was never applied and a step waited for the whole job; the tool and `sdc-bugfix` read the line for as long as the job's process lives, and a job that prints none and runs to its end was admitted at once and waited nothing (`docs/driver.md`, "The build pool wait", `story:pool-wait-is-not-a-failure`).
- `runbook doctor` checks the text layout by name (`bindings/`, `bindings/*.yaml`, `bindings/tools/`, `instructions/`, `views/`, `cycles/` beside the earlier directories) and the four tools at `bindings/tools/`, and cites no line numbers; `runbook --version` prints the repository version (0.3.0, not 0.1.4) and `task release:check` reads `runbook/Cargo.toml` (`story:runbook-command-knows-the-text-layout`).
- The model instructions of `sdc-reply implement-run` (request mode), `sdc-conform` and `sdc-unstick` are files under `instructions/` (`change.implement.request.*`, `conformance.table*`, `run.unstick*`), read at run time through one renderer with the rules of `llm-node.sh`; the prompts are byte-identical, held by goldens under `instructions/golden/`. `sdc-conform prompt` prints the table node's prompt without a model call (`story:rust-prompts-as-instruction-files`).
- `scripts/check-protocols.sh` ends in under a minute instead of about 10: it checks the digest of each protocol's sources against the committed `protocol/check-ledger.json` and runs `canon check`, the gate probe and `canon simulate` only for a protocol whose digest changed (`--record` writes the ledger, `task check:protocols:slow` regenerates it in full and compares bytes), prints the time per protocol and fails above 120 s (`story:check-protocols-budget`).
- The fast lane of `scripts/check-protocols.sh` is part of `task check`; each ledger entry carries a seal over its digest and results, so a hand edit of `protocol/check-ledger.json` fails it; `--record` and `--repin` refuse `SDC_SIMULATE=off`; CI runs the slow lane when `protocol/**`, `driver/canon-pin.json` or the script change (`.github/workflows/protocols.yml`).
- `task check` runs the UI test step (`task test:ui`, Playwright, 112 tests) beside the driver's (`task test:driver`); on the shared pool the two together took 105 s of the 120 s each step may take (the UI step alone 77 s, the driver's alone 59 s).
- The driver's test step is `task test:driver` (cargo-nextest, `driver/.config/nextest.toml`): a test over 60 s is killed and fails, the ten slowest are printed and the step fails when the test run takes over 120 s. The 11 integration-test binaries are one, `driver/tests/main.rs` (`autotests = false`; 122 integration and 157 unit tests before and after), so `cargo test --test <file>` no longer exists and the form is `cargo nextest run --test main -E 'test(<module>::)'`. The conformance tests were 280 s with three tests over 60 s before and no test over 41 s after, the test run 63 to 88 s on a loaded machine (`story:driver-test-budget`, merge `357f218c`).
- Every `bindings/*.yaml` names the registration of its protocol file (`sdc@8`, `sdc-intake@9`, ..., `sdc-bugfix@1`) and states `program_root: bindings`, so Loom accepts the files without a rewrite; `scripts/check-bindings.sh` refuses a file whose registration differs from its protocol file, a copy source no protocol file registers and a program binding without `program_root` (`story:bindings-name-real-registrations`, merge `f9b38d67`).
- The bug-fix cycles assert the red-then-green evidence in record order (the red `repro_run`, the fix, the passed `test_run`, the tree digests set and different) and, for `bugfix-never-red`, the protocol's own refusal in `decision.json`; no protocol rule changed (`story:red-then-green-as-evidence`, merge `fbd84489`).
- The UI server reads Canon from `SDC_UI_CANON` and the driver from `SDC_UI_DRIVE` when set (one place, `ui/binaries.mjs`, shared with the test fixtures); a program that is not there is a refusal that names its path.
- The slow, record and repin lanes of `scripts/check-protocols.sh` start each unit with `slot run --timeout 30m` (the fast lane keeps the pool's 10 minutes), and a unit the pool ends at its limit is named, `unit <protocol>.<part> ended at the pool's run limit`, where it was a silent `0s`; the report prints the canon seconds of each unit. Measured: the slow lane, 9 of 9 protocols, 7.4 minutes wall at load average 95 to 131, the longest units `sdc-plan.check` 430 s and `sdc-plan.probe` 440 s (`protocol/check-ledger.json` rewritten, digest and seal only).
- The experiment record `release-0.3.0` has status `abandoned`: its registration runs were stopped after 8 of 9 had started (7.90 USD), because the test step of a run waited in the shared build pool, was killed at its limit and counted as a failed run. The records of the 8 runs stay under `experiments/release-0.3.0/runs/` and say nothing about the release; `experiments/releases.yaml` gives 0.3.0 no registration numbers and the runs are repeated on 0.3.1 under a record of its own.
- The four crates, their lock files, `ui/package.json` and the plugin manifests are at 0.3.1.

### Fixed

- A test step that never got a slot in the build pool is no longer recorded as red tests and no longer sends the writer to repair them: `sdc-tool.sh` bounds the wait itself (`--pool-wait`, one bound per step, the pool's own 10 minutes still bound the run) and exits 69 when the job was still queued, `sdc-drive` runs such a step up to 3 times, records nothing and ends with the verdict `pool_wait_spent` (exit 5), `sdc-run-metrics` counts `pool_waits` apart from `could_not_run`, and the limits of story `tests.run` (3000 s) and release `acceptance.run` (2400 s) cover wait plus run (`docs/driver.md`, "The build pool wait", `story:pool-wait-is-not-a-failure`). A job the pool ends at its run limit (slot 0.22: exit 124) is its own result too, exit 70 "ran over its time": not red tests, not a pool wait, the step is not run again and the tree stops with `run_over_time` (`ran_over_time` in `sdc-run-metrics`). Under Loom both exits stay `failed` until its loader has `exits` (`bindings/PENDING.md`).
- The bug-fix steps `repro.run`, `tests.run` and `gate.run` no longer count the wait for the build pool as a failed run: `sdc-bugfix` bounds the wait of its jobs itself (`--pool-wait 1200` in the bindings, one bound per step, the environment variable `SDC_POOL_WAIT` wins), cancels a job still queued at the bound and exits 69, and exits 70 for a job the pool ended at its run limit, with the same driver behaviour as for story and release (3 runs then `pool_wait_spent`; `run_over_time` at once; no evidence, no `change.fix`). The limits are 3000, 3000 and 4200 s (`docs/driver.md`, "The build pool wait", `story:pool-wait-is-not-a-failure`).
- The UI server keeps one view per request and per tick (an `AsyncLocalStorage` scope read by `ui/web/view.js`), so an action that waits in its tool is no longer given the view of another source's request that arrived meanwhile; a stage name or phase id from a view is escaped in `ui/web/lenses.js`; four reasons in a committed fixture name the role instead of a first name.
- `runbook parity summarize` exits 2 and names the file for an input it cannot read or parse (a truncated event file, a malformed evidence record, a missing member) and no longer drops it; its digests use the `sha2` crate.
- The shell tools no longer fall back to a `beyond10x/sdc` path for `sdc-findings` and `sdc-finding-kpis` (a missing one stops with its name and where it looked), `llm-adapter-claude-cli.sh` runs with `HOME` unset (`--claude-credentials <file>` on `llm-node.sh`, else a message naming the variables), and the text of `in/feedback.md` and the requester request is in `instructions/` (`story:tool-texts-and-paths`).

- The step's wait bound for the build pool holds under a locale with a decimal comma: `pool_run` in `sdc-tool.sh` read bash's `EPOCHREALTIME` as "1763810464,227523" under `LC_ALL=de_DE.UTF-8` (a systemd unit on such a host), took noise for the elapsed time and never cancelled a queued job, so the finding cycle `pool-wait` ran 10 minutes and failed. The stand-in pool of that cycle now keeps a queued job 30 s at most, and `scripts/run-full-cycle.sh` names the program and its first line when `sdc-drive run` prints no JSON, in place of jq's parse error (`story:pool-wait-is-not-a-failure`).

## [0.3.0] - 2026-11-22

The bindings move onto Loom's program contract and the instructions of the model's calls become
files, Canon 0.10.0 is pinned, and four changes to what a run does arrive behind switches that are
off by default. The `runbook` command, the `runbook` plugin, the bug-fix runbook on a ticket and on
any language, a view file for the run inspector and a list of the finding cycles as data are new.
The longer note is `docs/releases/0.3.0.md`.

### Added

- `driver/canon-pin.json` pins Canon 0.10.0: every script resolves one binary (`CANON`, else the cached pinned install) and refuses another `canon --version`, none builds or enters a Canon checkout (`CANON_DIR` and the `$HOME` default are gone), and `runbook doctor` compares the version with the pin (`story:canon-0-10-0-pin`).
- Loom mode of `llm-node.sh` takes the `llm` object's `instructions` as `--instructions <json>` (`bindings/*.yaml` names it for the 14 agent nodes), so the instruction files load under Loom as under the driver. `bugfix_prompt` and `questions_check` name the tools at `bindings/tools/`.
- `cycles/suite.yaml` (`runbook.cycles/1`, `story:cycles-suite-as-data`): the finding cycles of
  `scripts/run-finding-cycles.sh` as data, what cannot be said declaratively listed per cycle as
  `unexpressed`, and `task check:cycles-suite` holding it equal to the script's cycle ids and states
  (`docs/cycles-suite.md`).
- The local gate and CI read one pinned rustc (`rust-toolchain.toml`, 1.99.1), `.cargo/config.toml` drops debug info from the dev and test profiles, and `task ci:watch SHA=<commit>` exits non-zero unless every forge run of a commit succeeded (`story:local-gate-equals-forge`).
- The run inspector reads its runbook's phases, file kinds and tool owners from a view file
  (`views/sdc.view.json`, format `runbook.view/1`; `--view FILE`, `--no-view`; `ui/README.md`), so it
  renders a runbook that is not SDC (`story:ui-view-data-per-runbook`).
- The instructions of the 14 agent actions and of the decider are files under `instructions/`
  (`instructions/<action>.md`, `instructions/shared/`, `instructions/decider/`, `{{name}}`
  placeholders), named by `llm.instructions` in each JSON binding; `llm-node.sh` and `decider.sh`
  only read and fill them, and `driver/tests/bugfix_prompt.rs` holds all 40 prompts byte for byte
  against `instructions/golden/` (`story:instructions-as-files`, `docs/driver.md`, "Instructions").
- `change.implement` as one request (`story:implement-one-request`), behind the run setting
  `SDC_IMPLEMENT_MODE=session|request` (default `session`, recorded as `implement_mode` in
  `run-settings.jsonl`, a launch setting of `sdc-experiment`). `sdc-reply implement-run` assembles
  the node's whole input (the story, its requirements and acceptance lines, only its commands and
  views from the compiled IR with what they name, the build constraints, the product tree by rule
  up to 64,000 bytes with the rest listed by path and size, findings, and on a rework the failed
  checks and the rejection), asks once with no tool, checks the reply's envelope against
  `sdc-reply schema --type implement` and every path against the product tree, retries an invalid
  reply at most twice (`implement_reply_invalid`), continues by rule for paths the reply left
  `remaining` (at most two, `implement_reply_incomplete`), and writes the files into the revision.
  The call's `llm-calls.jsonl` line carries `requests`, `continuations` and `reply_retries`. Request
  mode runs on a schema adapter (`SDC_IMPLEMENT_ADAPTER`, else claude-cli under metaharness;
  metaharness is refused at start). `docs/driver.md`, "Implement as one request".
- `docs/driver.md`: the list of free-form exceptions to the response-schema rule; request mode's
  envelope is schema-checked and its file contents are free text.
- Cycles `implement-request-build`, `-tests-rework`, `-review-rework`, `-invalid-retry`,
  `-invalid-named`, `-path-escape` (fixture adapter, `SDC_IMPLEMENT_REPLIES`).
- Experiment records `implement-one-request` (mixed) and `implement-one-request-strong`, session
  against request, three runs each, registered before any run.
- Approvals by program (`story:decider-program-approvals`), behind the run switch
  `SDC_RULE_APPROVALS=on|off` (default `off`, so a default run asks the decider as before): under
  A5, `context.approve`, `roadmap.approve` and `release.publish` are taken by `rule:autopilot`,
  with no decider call, when the program check guarding each (`context.check`,
  `roadmap.validate`; `spec.gate`, `release.build`, `acceptance.run`) recorded a passing result
  for the exact revision approved; otherwise the decider is asked. The mapping is
  `driver/examples/sdc/rule-ends.json` (`approvals.*.by_rule`), checked by `sdc-check-routes`
  (finding `rule-guard`); `vision.approve` has no program check and is left out. The switch is
  recorded in `run-settings.jsonl` (`rule_approvals`); the records
  `decider-program-approvals{,-mixed}` set it per variant. Not measured: no experiment has tested
  this change; cycle `rule-approvals-default` re-drives main's stub recording with the switch
  unset (same calls, prompts and actions). The decision record names the rule, the check and the
  evidence ids. `sdc-run-metrics` counts these as `decisions_by_rule`,
  not decider calls, and reports `decider_s`. New `sdc-reply rule-approval`; cycles
  `rule-approvals*`; experiment record `experiments/decider-program-approvals.yaml`.
- `docs/improving.md`, "Run switches": the four switches (`SDC_IMPLEMENT_MODE`,
  `SDC_RULE_APPROVALS`, `SDC_REVIEW_MODE`, `SDC_IMPLEMENT_COVERAGE`), their defaults, settings,
  records and cycles. A change in what a run does merges off by default until its record holds.
  Cycles `rule-approvals-default`, `review-mode-default` and `implement-coverage-default` re-drive
  main's stub recording once with every switch at its default (run `switches-default`).
- `runbook`, the command people type (`runbook/`, a new crate; `story:runbook-cli-skeleton`):
  `runbook doctor` checks the assets, the Canon binary and the programs a run needs, one line per
  check with what to do on a fail (`--for`, `--json`); `runbook list` lists the runbooks and child
  protocols of `protocol/`; `runbook --version`. `run`, `ui` and the other commands are not built.
  `task check` builds, tests and lints it (`b6f7225`). No run metric is affected.
- The `runbook` plugin (`plugin/`, `.claude-plugin/marketplace.json`): skills `workflow-builder`
  (moved from `skills/`), `harden` and `record`, for Claude Code and Codex. Its manifests carry the
  repository version, which `task release:check` compares; `driver/tests/plugin.rs` checks the
  versions, the marketplace entry and that every repository path a `SKILL.md` names exists
  (`story:runbook-plugin`). No run metric is affected.
- The bug-fix runbook starts from a ticket document (`sdc.ticket/1`) on a clean git worktree:
  `sdc-bugfix seed --ticket <file> [--repo <worktree>]`, `scripts/run-trial.sh --bugfix-ticket`
  (`story:ticket-input-and-repo-seed`). No run metric is affected: the benchmark input does not run
  `sdc-bugfix`.
- The bug-fix runbook works on a repository of any language. Its suite, single-test and gate
  commands are the case's `checks.yaml` (`sdc.checks/1`), from `seed --checks`, the repository's
  `.runbook/checks.yaml`, or the marker-file table `driver/examples/sdc/checks-detect.yaml`; with
  none, the seed fails naming `--checks`. Tested: Rust (unchanged), JavaScript with `node --test`,
  and a supplied `checks.yaml`; the Go, Python, Java, npm and make rows are not tested. The writers'
  prompts quote `checks.yaml` and name no language (`story:bugfix-any-language`). No run metric is
  affected: the benchmark input does not run `sdc-bugfix`.
- The six runs of `build-constraints` on tree 3ca26fe (`fix/superseded-answers-0.1.2`, not merged):
  report and run summaries, Haiku 0 of 3 clean passes, mixed 1 of 3; descriptive, the record has
  no hypothesis (`89b8de0`). Two runs ended after the ess on PATH changed at about 20:54Z (see
  Fixed); whether they called it after that is not checked.
- The six runs of `effort-decider-mixed` on tree 72a3eef: report and run summaries, xhigh 3 of 3
  clean passes, medium 2 of 3; `run.wall_s` xhigh 3032 [1913, 3677] (3), medium 1831 [1424, 2191]
  (3); inconclusive, the ranges overlap (`experiments/effort-decider-mixed/report.md`).
- `bindings/`: the nine bindings files in Loom's `loom.bindings/1`, beside the JSON files the
  driver still runs from, each accepted by Loom's loader at Loom `8a8fad3b`; `bindings/PENDING.md`
  lists what the format cannot express yet, and `scripts/check-bindings.sh` checks the YAML files
  against the JSON files and the protocols (`docs/loom-bindings.md`).
- In Loom mode `llm-node.sh` and `sdc-tool.sh` take the epic and the story of a case from `LOOM_PARENT_CASE_NAME` and `LOOM_CASE_NAME` (the names Loom gave the cases) and fall back to the cut of the case id when neither is set, so a story under a repeated epic (`IDEA-1.billing.2.story-a`) is `story-a` (`docs/loom-bindings.md`, `bindings/PENDING.md`).

### Changed

- The tools the bindings run moved from `driver/examples/sdc/tools/` to `bindings/tools/`
  (`story:loom-tools-program-contract`): `bindings/*.yaml` names each as `tools/<file>`, with no
  `..`, and `scripts/check-bindings.sh` refuses any other program path. `sdc-tool.sh` and
  `llm-node.sh` run under Loom's program contract when `LOOM_CASE` is set (arguments on standard
  input, `LOOM_RETRY_FEEDBACK`, the binding's `--action`, `--model`, `--stub` and other values as
  argv, the driver's binaries from `PATH`), and as before when it is not
  (`docs/loom-bindings.md`, "Tool changes").
- Runbook evaluates with Canon 0.10.0 (`story:canon-0-10-0-pin`): the 52 `cases/*/decision.json` are
  regenerated (new `semantics.canon` and the new `missing_evidence` and `pending_decisions`
  sections; no claim, action or outcome changed) and the docs that state the pin say 0.10.0.
- `change.review` on the smallest fact set (`story:review-minimal-facts`), behind the run switch
  `SDC_REVIEW_MODE=session|facts` (default `session`, main's review; recorded as `review_mode` in
  `run-settings.jsonl`, a launch setting of `sdc-experiment`). With `facts`:
  `sdc-reply review-inputs` assembles one document, `in/review.md`, with the story's requirements
  and commands, the test summary (counts and failing names, not the log), the build constraints
  when the tree has them, the diff, the full text of only the files the diff touches and a listing
  of every other product file (path, size, top-level item names; capped at 4 KB, then paths only); the
  review is one request with no tools (`operations` empty, `max_turns` 4, `--tools ""`), and the
  document's size is recorded per call as `input_bytes` in `llm-calls.jsonl`. Verdict rules and the
  response schema are unchanged except that a finding's `at` may cite a listed file by path
  without a line (a line of an untouched file stays refused). The record
  `review-minimal-facts` (variant `minimal-facts` sets `review_mode: facts`) decides by recorded
  replay whether the decisions hold (`docs/driver.md`, "Facts review"). Not measured: no experiment
  has tested this change; cycle `review-mode-default` re-drives main's stub recording with the
  switch unset.
- `change.implement` cites its requirements (`story:implement-cites-requirements`), behind the
  run switch `SDC_IMPLEMENT_COVERAGE=on|off` (default `off`, main's implement step; recorded as
  `implement_coverage` in `run-settings.jsonl`, a launch setting of `sdc-experiment`; a binding's
  `postcondition` may name a `switch` it runs under). With `on`, besides its change it replies with a coverage document checked against a response schema (per requirement
  of the story, `built` with file, line and a test, or `not_built` with a reason), and the
  binding's new `postcondition`, `sdc-reply check-coverage`, run by `sdc-drive` right after the
  implement step, refuses the revision before `tests.run` when a requirement is missing, not
  built, cited at a file or line the tree lacks, or named with a test that does not exist. The
  refusal names the requirement ids, reaches the next implement run as `in/feedback.md`, counts
  under its own retry kind `requirement_coverage` against `max_runs`, and shows in
  `sdc-run-metrics` as `requirement_coverage:refused`. The review gets the document as
  `in/coverage.json`. Off, no coverage document is asked for or checked. No protocol rule changed (`docs/driver.md`, "Requirement coverage").
  Not measured: no experiment has tested this change; `experiments/implement-cites-requirements.yaml`
  (both variants set `implement_coverage: on`) is registered and not launched; cycle
  `implement-coverage-default` re-drives main's stub recording with the switch unset.
- The approval record of 0.2.1 and the ledger tree of 0.2.1 (99f5617) (`db85df0`).
- CI: a change of only planning or prose files passes `Documentation validation` without building or testing, and deploys nothing; a docs-site change skips the `sdc-protocol-site` unit tests (`story:ci-planning-only-skip`).
- Docs tests: the Playwright step ends within 120 s (49.7 s for 54 tests with four workers on the release tree, the slowest 10.0 s; it was over 120 s before the budget story), 60 s per test, ten slowest tests printed (`story:docs-test-budget`).
- The three crates, their lock files, `ui/package.json` and the plugin manifests are at 0.3.0; `runbook/Cargo.toml` stays at 0.1.4.

### Fixed

- The tools of the bindings (`driver/examples/sdc/tools/sdc-tool.sh`) run ess 0.96.0 whatever
  `ess` is on PATH (`ESS_TOOLCHAIN=0.96.0`, `ESS_TOOLCHAIN_QUIET=1` unless the caller sets them).
  Since about 20:54Z on 2026-11-21 the `ess` on PATH can be a version dispatcher, 0.97.1, which
  ran itself in a case tree: the conformance line of a story's `tests.run` log then names 0.97.1,
  so `in/tests.txt` and the replay cycles depended on the installed ess (`f82aa7c`). It restores
  the release of every committed recording and record; not measured otherwise.

### Security

- `llm-node.sh` and `decider.sh` refuse an instruction path that is not `instructions/<...>.md` or
  has a `..` segment (exit 126), and `llm-node.sh` in Loom mode reads a retry feedback log only for a
  blob digest of 64 hex digits, so a binding value or a feedback record cannot read a file outside
  `instructions/` or the feedback directory into a prompt; `scripts/check-bindings.sh` parses the
  `--instructions` value.

## [0.2.1] - 2026-11-21

A patch release of 0.2.0: two fixes of the documentation checks. The driver, protocols, bindings,
tools and run inspector are unchanged apart from the version. The longer note is
`docs/releases/0.2.1.md`.

### Fixed

- `website/tests/smoke.spec.ts` expected the evaluation page's list of releases without runs to be
  exactly 0.1.1, 0.1.3 and 0.1.4, so the 0.2.0 entry made four smoke tests and `Documentation
  validation` fail on the 0.2.0 tag commit. The assertion requires those three first and admits
  later ones (`e46a237`). No run metric is affected.
- `website/data/protocols/index.json` was stale on the 0.2.0 tree; it is written again with
  `sdc-protocol-site write` (`e46a237`). No run metric is affected.

### Changed

- The approval record of 0.2.0 and the ledger tree of 0.2.0 (6cde564).
- The three crates, their lock files and `ui/package.json` are at 0.2.1.

## [0.2.0] - 2026-11-21

The product is named Runbook and the repository moves to `beyond10x/runbook`; the software delivery
cycle (SDC) is the first runbook and keeps its ids. With it come a fix of the spec gate, the results of
the release-0.1.2 runs and a reworked documentation site. No protocol or binding changed; one tool's
check got stricter. The longer note is `docs/releases/0.2.0.md`.

### Changed

- The product name people read is now Runbook (stage 1 of `story:project-rename`; the operator,
  2026-11-21: "Yes, runbook. Agentic runbook."). Renamed in `README.md`, `AGENTS.md`, the
  documentation site (title, tagline, navbar brand and mark, footer, page titles and descriptions,
  the landing hero, "What it is", "What it is built on", "Run it", "The flow") and the run
  inspector (page title, header, `ui/README.md`). The software delivery cycle keeps the name SDC
  and is the first runbook; its protocols, trials and measurements still say SDC. The landing lede
  is three short lines (`d03494d`, `a70640f`).
- Not renamed yet: crate, binary and file names (`sdc-drive` and the others), `SDC_*` environment
  names, `sdc.*` format and protocol ids (stage 2, with one `runbook` command and release binaries).
  Release notes, trial notes and experiment records keep the names they were written with.
- The repository is `beyond10x/runbook` (was `beyond10x/sdc`) and the documentation site is at
  https://beyond10x.github.io/runbook/ (was `/sdc/`; stage 3 of `story:project-rename`). The old git
  and web addresses of the repository redirect; the old site address does not, so a link to
  `https://beyond10x.github.io/sdc/` no longer opens the site. Written addresses in the site, the
  documentation, `AGENTS.md`, the Pages workflow and the protocol-site generator follow the new name
  (`0418062`).
- The evaluation page opens with headline tiles (clean passes, median cost and wall time per
  setting, the cost target with the factor still to go, the blinded quality medians, the
  conformance lower bound) and large charts (clean passes, cost with the target and the free agent's
  median, wall time, where the cost of a run goes). Then come stops by cause, the improvement
  experiments and every run. A chart column exists only for a release with committed runs.
  `sdc-data` reads `experiments/targets.yaml` as targets (`c6880da`, `b42f34d`).
- The navbar has four entries (How it works, Protocols, Evaluation, Run it) and GitHub as an icon
  link; the docs sidebar carries the How it works group; `tests/navigation.spec.ts` (`12cba63`).
- The progress ledger's 0.1.2 entry names `release-0.1.2` as its experiment: 7 of 9 clean passes
  on the 0.1.2 tree, and the site's charts get the 0.1.2 column (`cebbf42`).
- `experiments/baseline-0.1.0/inputs.md`, `costs.md` and the record `review-minimal-facts`: the
  review is a reading session with `Glob`, `Read` and `Grep`, not one request (`39d2db3`).
- The three crates, their lock files and `ui/package.json` are at 0.2.0.

### Fixed

- The spec gate requires the synthesized conformance suite to pass against ESS's interpreted target
  (`sdc-tool.sh` `spec_conform`, used by `spec.synthesize` and `spec.gate`): an unsupported, failed
  or errored scenario refuses the spec, and `spec.author` gets one line per such scenario through
  the existing `spec_synthesis` feedback route. Before, a spec whose own suite did not pass reached
  the story, where the implementing writer cannot change it (`release-0.1.2.mixed.1`, `writer_spent`
  at `change.implement`). Cycle `spec-interp-unsupported`; the committed replay recording re-packed
  because the review's `in/gate.txt` now ends with the interpreted run's line (`238c70d`,
  `501d207`).

### Added

- A slider of three claims below the landing hero (what it does, why it is simple, why it is solid),
  with numbers from committed data, and `tests/pitch.spec.ts` (`93a96a6`, `d51609c`).
- A Context page (`/docs/context`) on what a model call is given, with two graphics and line
  citations, and `tests/context.spec.ts` (`d7514a3`, `957bcae`).
- The nine runs of `release-0.1.2` on tree 72a3eef: report, run summaries, timelines and
  `notes.md` (strong 3 of 3 clean, mixed 2 of 3, Haiku 2 of 3) (`12e3069`, `74b6f1e`).
- A logo (`docs/img/runbook-logo.svg`, `runbook-logo-dark.svg`) and a rewritten `README.md`: logo,
  one-line summary, links, progress chart, how it works, the dry run, the layout and the status.

## [0.1.4] - 2026-11-21

An observation, data and docs: the free-running agent comparison (its registered hypothesis
contradicted), the blinded quality evaluation of both sides' products, the conformance observation
of a delivered product, and the cost target set from them. No rule of a protocol changed. The longer note is `docs/releases/0.1.4.md`.

### Added

- `sdc-free-agent` and the record `free-agent-0.1.0`: one confined free-running agent session
  (Sonnet) given the same idea and deliverables as the SDC, three runs, with run summaries,
  timelines, product sizes (`sizes.json`) and conformance; `docs/llm-is-not-the-runtime.md`
  (`3ad3391`, `fd3a673`, `4baca16`, `8c75540`, `87ccaf4`). The registered hypothesis, that the free
  agent costs more than the SDC on 0.1.0, is contradicted on all four pairs.
- The analysis of the 0.1.0 baseline (`experiments/baseline-0.1.0/losses.md`, `costs.md`,
  `inputs.md`, `models.md`), the cost target in `experiments/targets.yaml` (checked by
  `sdc-experiment check --all`) and `docs/improving.md` "What the 0.1.0 baseline says" and
  "Target"; the records `build-constraints`, `review-haiku-replay` and `review-minimal-facts`,
  registered before any run (`d315e05`, `26c5423`, `110ab2e`).
- Conformance observation after close (`docs/conformance.md`, `story:conformance-observe-after-close`):
  `sdc-conform observe runs/<run>` holds the delivered product to the conformance suite of the
  run's own spec through `ess verify conform run --target process` and the generic HTTP adapter
  `sdc-conform-adapter`, with route tables written after the run by the node `conformance.table`
  (read-only, response schema, program check, two retries) and writes `runs/<run>/oracle.json`:
  `conformance: spec scenarios proven passed: k of n (lower bound); f failed; u not measurable;
  tables agree on a of n`.
  The suite runs once per table (`--tables`, default 3); a scenario passes only when every counted
  table that reached it passed it, fails only when every one failed it, and is disputed when they
  disagree (cause `disputed` when nothing failed). Several statuses may mean one outcome
  (`status: [409, 404]`). The check refuses a literal in an event payload or error field, and a
  check of a field filled from the input is recorded unverified (`unverified_checks`). The start
  command cites the README or manifest line that states it (`start.from`; `cargo run` and `go run`
  refused), and an argument or header a README describes as a test, demo or mock mode is refused.
  A refusal of a command on an id the scenario never created is read by the adapter as the
  command's unknown-instance answer (ESS: not-found, else `wrong_state`), and 404 for an id it
  created never; a start value the README leaves to the operator can come from the scenario
  (`start.args`, `from_scenario`); a slot refused after its retry gets one more session (five per
  slot). The figure goes into a release as an observation: `passed` is a lower bound, judged against
  the run's own specification; nothing else is claimed. A table whose run shows its own fault (a capture
  that never appears, a request naming a value the scenario leaves out, 401 or 403) is refused and
  asked again once with that evidence. `held` only when k = n > 0 (u > 0 is never held); a run
  without a counted table is `unknown` with a named cause and prints no counts
  (`not measured (<cause>), n scenarios`). A scenario is not
  measurable when it needs what the table declares `none` of kind
  `not_reachable_over_the_interface`; `not_implemented` fails. The table can capture values the
  product prints at start (`captures`), send them in paths, headers and bodies, and bind them to a
  spec input (`bindings`). `scripts/run-trial.sh` runs it after a close when `SDC_CONFORMANCE=observe`.
  Observation only: no gate. A scenario one counted table saw fail and the others declared
  unreachable counts failed. When every slot's table is refused only for the product's 401 or 403 in
  the same scenarios, none of which had a success before its refusal (a sign-in the specification
  does not model), those scenarios count failed with cause `precondition_not_in_spec`
  ("f failed, of which p refused for a precondition the specification does not model").
  `sdc-conform recompute` takes the figure again from stored table runs, `sdc-conform summary`
  prints the sanitized summary the progress ledger reads from
  `experiments/<id>/conformance/<run>.json`; the ledger shows conformance always (`not observed`
  where a run has none), with the baseline-0.1.0 figures. One text form in oracle.json,
  run-metrics, run-compare and the ledger. About 1 to 3 USD of node cost per observed product.
  A recorded re-drive (`run-trial.sh --recorded`) skips the observation: its recording holds no
  table call, and each one counted as a divergence.
- `driver/ess-pin.json`: ess 0.97.0 pinned by archive and binary sha256 per platform, resolved
  through `ESS_BIN` or a cache path (`sdc-conform ess-install`); the `ess` on PATH is not used.
- The row `conformance` in `sdc-run-compare`, `metrics.conformance*` and the text line in
  `sdc-run-metrics`; a conformance column, line and per-run figure in the progress ledger.
- Site, Evaluation: the chart "Spec scenarios proven passed (lower bound)" per release and setting
  (median, min to max, n; a run without an observation is "not observed", never 0), the
  observer's line and the failed count with failures for a precondition the specification does not
  model split out per run, and three cited sentences on what is checked, why it is a lower bound and
  what it costs.
- Site, "The LLM is not the runtime": the comparison of record `free-agent-0.1.0` drawn from the
  committed record, run summaries, timelines, `sizes.json` and conformance summaries: the result in
  the first paragraph, the timelines on one time axis and cost scale, medians, the hypothesis and
  its verdict per pair, product sizes, conformance of both sides, what the SDC run records that the
  free run does not, what was held equal and the confounds; the quality result follows below.
- Blinded quality evaluation of the nine 0.1.0 to-do products (`docs/quality-0.1.0.md`,
  `experiments/quality-0.1.0/`, harness `eval/quality/`): a 23-item rubric written from the idea
  text before any product was opened, scored in a real browser on blinded copies.
- Site, "The LLM is not the runtime": the quality result in place of "pending", from
  `scores.json` and the rubric: points per side beside cost and wall time, a chart of points
  against cost per product, the items where the sides differ, the defects found and the limits;
  one sentence of it in the first paragraph. Evaluation describes it as a case with a link.

### Changed

- The conformance observation after close is off by default: `scripts/run-trial.sh` runs it only
  when `SDC_CONFORMANCE=observe`, because it costs about 1 to 3 USD of model calls per run; an
  experiment record turns it on with the launch setting `conformance: observe` (`e1dd70a`).
- The three crates, their lock files and `ui/package.json` are at 0.1.4.

### Known limits

- The conformance figure is a lower bound judged against each run's own specification; the
  started product is not confined, events are built from the response body, each table is a
  model's reading, and only products with an HTTP JSON surface are observed (`docs/conformance.md`).
- The conformance tests need the pinned ess installed (`sdc-conform ess-install`); CI does not run
  them.
- The blinded quality evaluation covers one idea with three products per side; offline is emulated
  and a browser test sees behaviour, not code quality (`docs/quality-0.1.0.md` § Limits).
- No run has used the 0.1.2, 0.1.3 or 0.1.4 tree.

## [0.1.3] - 2026-11-21

The documentation site and its generator. No protocol, binding or case changed. The longer note is
`docs/releases/0.1.3.md`.

### Added

- The documentation site, `website/` (Docusaurus with the beyond10x docs-system theme), published
  at https://beyond10x.github.io/sdc/ by `.github/workflows/pages.yml` and
  `.github/workflows/b10x-docs-site.yml`: what the SDC is, the main flow and a browser with one
  page per protocol, what it is built on, the evaluation per release and model setting, and one
  run step by step ("The LLM is not the runtime") (`3e93276`, `3c2930b`, `b7497fa`).
- `sdc-protocol-site`: writes the site's protocol data, `website/data/protocols/`, from the
  protocol YAML, the bindings and the committed check outputs; `check` fails on stale data and runs
  in `scripts/check-bindings.sh`; `site-manifest` writes the site's `.well-known/b10x-site.json`
  (`19209b8`, `3ba6bd6`).
- Each run in `docs/progress.json` carries its metrics (cost, wall time, model and decider calls,
  rework takes, review and merge rejections, looping factor and first-pass yield per node, stop
  cause; null where the run summary has none) (`c45bc91`).
- `task docs`, `docs:generate`, `docs:build`, `docs:test`: the site's data check, build and smoke
  test (every page at 1440 and 390 px in both themes, axe, internal links).
- Two experiment records registered before their runs, on the 0.1.2 tree: `release-0.1.2` (the
  baseline of 0.1.2 against `baseline-0.1.0`) and `effort-decider-mixed` (the mixed setting's Haiku
  decider at effort medium against xhigh) (`f619fd4`). Both are planned; no run has been made.

### Changed

- The three crates, their lock files and `ui/package.json` are at 0.1.3.

### Known limits

- No run has used the 0.1.2 or the 0.1.3 tree; the two registered records are planned, not run.
- A check of the delivered product against its specification is in work and not in this release.

## [0.1.2] - 2026-11-21

Corrections and tools after 0.1.1: recorded replay, the timeline of a run, one rework cap, the
LLM decider's principal renamed, and the root protocol's description. No rule of a protocol
changed. The longer note is `docs/releases/0.1.2.md`.

### Added

- Recorded replay: `scripts/run-trial.sh <run> --autopilot --recorded <source>` re-drives a
  recorded run with the real driver and programs and the recorded model replies (`sdc-replay`,
  `332d3aa`, `b5ead4c`, merged again in `8a96b50`), with a committed stub recording and seven
  re-drive cycles. `--set` changes a condition (`max_rejections`, timeouts, retries, run caps),
  `--inject` puts a decision in place of a recorded one, and `--on-divergence stop|nearest|live`
  says what a call the source never made gets. `sdc-replay` answers
  `--capabilities` (`26b8b85`). A re-driven run has `run.unstick` off unless the source called it,
  because a node the source never called has no recorded reply; `--set unstick=on` decides
  otherwise (`f717443`).
- `sdc-run-timeline`: the timeline of one run (`sdc.run-timeline/1`, `f461e61`), and the timelines
  of `trial-0.1.0-strong-1` and `trial-0.1.0-mixed-1` under `experiments/baseline-0.1.0/timelines/`
  (`89581f0`).

### Changed

- The LLM decider's principal is `decider:model`, whatever the model; its model id is recorded
  beside it in `run-settings.jsonl` and on each `decisions.jsonl` line (D149, `41d0023`). Before,
  the strong runs of trial 3 recorded an Opus decider as `decider:haiku`. `SDC_DECIDER=haiku` stays
  an alias of `model`. For a reader of old runs: `decider:haiku` in a run before 0.1.2 is the same
  principal as `decider:model`; the tools and the live view read both, and `sdc-experiment` counts
  `haiku` and `model` as one setting.
- One effective rework cap (D184, `9940eed`): `rework_limit` of `rule-ends.json` is the default,
  `SDC_MAX_REJECTIONS` overrides it when set, and a value that is not a whole number exits 64.
  `run-settings.jsonl` records it as `rework_limit: {value, source}`; the driver and the writer's
  rework prompt both read that value. Before, the prompt said "of at most 2" whatever the setting.
- "SDC" stands for "software delivery cycle" (the operator, 2026-11-21). The root protocol
  `protocol/sdc.protocol.yaml` said "software development cycle" in its description; it now says
  "delivery". That changes its compiled IR (`protocol/compiled/sdc.ir.json`, sha256
  `41c6acc7…` to `8a05e12f…`) and so the IR hash a new case records, but no rule: no action,
  condition, outcome or binding changed, and `canon simulate` gives the pinned report byte for byte.
  Runs recorded before keep their own IR in their logs; the committed replay recording still
  re-drives with every call matched (`1109656`).
- The three crates, their lock files and `ui/package.json` are at 0.1.2.

### Corrections to 0.1.0

`docs/releases/0.1.0.md` and the GitHub Release of 0.1.0 are not edited; this section holds the
corrected figures.

- Decider cost: `sdc-run-compare` summed the cost of the `decisions.jsonl` lines that are not
  `escalation_suppressed`, so every decider call whose escalation was suppressed (a real model
  call, its cost sits on the suppressed line) was missing from the decider cost and the total.
  The 0.1.0 Trial 3 figures were taken from the tool. Recomputed for the seven passes as the sum
  of `cost_usd` over `llm-calls.jsonl` and over every line of `decisions.jsonl`; the decider
  figure also equals the sum of `decider/*/response.json`.

  | Run | Published (USD) | Recomputed (USD) | Difference |
  |---|---|---|---|
  | `trial-3-strong-2` | 3.00 | 3.00 (2.996) | 0 |
  | `trial-3-strong-1` | 2.93 | 2.93 (2.926) | 0 |
  | `trial-3-opus-1` | 5.09 | 5.09 (5.088) | 0 |
  | `trial-3-mixed-6` | 1.86 | 1.86 (1.859) | 0 |
  | `trial-3-haiku-7` | 1.00 | 1.03 (1.034) | +0.03: its decider cost is 0.371, not 0.335 (1 suppressed line) |
  | `trial-3-strong-6` | 4.05 | 4.05 (4.055) | 0 |
  | `trial-3-strong-5` | 5.34 | 5.34 (5.338) | 0 |

  Only `haiku-7` has a suppressed-escalation line among the seven; the others have none, so their
  published figures stand. Other runs of `docs/trial-3.md` changed the same way (`haiku-6`
  0.540 to 0.560, `mixed-5` 1.476 to 1.588, and the attempts), and `docs/trial-3.md` is corrected.
- The fix of the tool is in 0.1.1 (`df58754`, `9ff2996`; `docs/releases/0.1.1.md`, "A defect in a
  figure tool"); this section is the correction of the 0.1.0 figures that note promises
  (`8e61332`, `956f1df`).

### Known limits

- No run has used the 0.1.2 tree. The baseline is still `experiments/baseline-0.1.0`: 6 of 9
  clean passes on the 0.1.0 tree.
- A check of the delivered product against its specification is in work and not in this release.
- Recorded replay re-drives the committed stub recording; a re-driven run reproduces the source's
  calls and has no reply for a node the source never called.

## [0.1.1] - 2026-11-21

Fixes and tools after the first release; no protocol of the nine changed. The longer note is
`docs/releases/0.1.1.md`.

### Added

- Keep-going (`scripts/run-trial.sh <run> --autopilot --keep-going`, D142): trial and eval runs
  force past autopilot's stops and record each defect in `defects.jsonl`; a forced run never
  counts as an end-to-end pass (`2a95956`).
- `run.unstick` (D147, `sdc-unstick`): under autopilot, before a stop that is no cap, a model picks
  a legal move (run a writer again, ask again, end), checked by program (`b8a0116`); its class
  `unstick` has the default effort `xhigh` (`0f7eb8f`).
- `sdc-drive reopen` and `run-trial.sh --reopen`: a step that ran at a case's current state but
  recorded nothing runs once more on the unchanged state, and a refused decision is put back
  (`3cbda08`, `6c4886a`).
- `sdc-check-routes`, run from `scripts/check-bindings.sh` (D148): the static check of the reject
  and cap routes, whose findings must equal the list in `docs/reject-routes.md` (`a9a1185`).
- The simulate step of `scripts/check-protocols.sh`: `canon simulate` on every protocol, compared
  byte for byte with `protocol/simulate/<name>.txt` (`c8747ef`), pinned to Canon `0.10.0`
  (commit `2c7a1ea6`), the first Canon release with `simulate`.
- Workflow metrics and experiment records: `sdc-run-metrics`, `sdc-experiment` (records under
  `experiments/`, their reports and `INDEX.md`, checked by `check-bindings.sh`) and the progress
  ledger `docs/progress.md` with its image `docs/img/progress.svg`, written from
  `experiments/releases.yaml` (`a0f8e9e`, `6296229`; `docs/workflow-metrics.md`,
  `docs/improving.md`).
- The live view's run picker: a search over recent runs with their facts, grouped by experiment
  (`009b8fd`, `4e09b4c`, `ab8472f`).
- Three more `sdc-bugfix` seeds and four scripted bug-fix cycles (`99238c0`, `e108d5e`).
- The first baseline on a released tree: `experiments/baseline-0.1.0`, nine runs on the 0.1.0
  tree, recorded in the progress ledger.

### Changed

- `docs/releasing.md` § Standing rule: under the operator's standing order of 2026-11-21 a patch
  or minor release of a measured improvement needs no further word; the orchestrating session
  orders the cut once `main` is green on the release commit, and the merging session tags and
  publishes on that order.
- The three crates, their lock files and `ui/package.json` are at 0.1.1.

### Fixed

- Reject and cap routes of the audit: a writer's failed run is fed back to its next run (R9,
  `5297ef0`); a writer's runs are capped per retry kind (R7, `a2f363e`); a failed check's own
  output reaches the writer (R13, `97024ce`); an unattributed scope-changing answer goes to the
  decider as a question (R2, `a919898`); a refused finding decision is put to the decider again
  with the refusal (R3, `306fde9`).
- Verdicts and counts are read from stdout alone; an unreadable result is a tool error
  (`7af5a97`).
- `sdc-bugfix` attributes the reproducing test by its own run: the red-check defect named in the
  0.1.0 limits (`99238c0`).
- The driver counts no not-called run as an LLM call (`9ce86bf`).
- `sdc-run-compare` and the live view's KPI strip count every decider call made, once per record
  (`df58754`, `9ff2996`). Before, `sdc-run-compare` left out the cost of every decider call whose
  escalation was suppressed (`trial-3-haiku-9`: 0.159 USD reported, 0.325 USD spent).

### Known limits

- No run has used the 0.1.1 tree. The baseline of 0.1.0 is `experiments/baseline-0.1.0`: 6 of 9
  clean passes (strong 3 of 3, median 5.50 USD and 1,247 s; mixed 3 of 3, median 2.55 USD and
  1,489 s; Haiku 0 of 3, stopped `writer_spent` twice and `decision_not_takeable` once).
- Recorded replay is not in this release: merged and reverted before it (`7c5dd5e`), it follows in
  a later one.

## [0.1.0] - 2026-11-21

The first release: a prototype. No interface is stable. The longer note, with the Trial 3 runs and
the limits, is `docs/releases/0.1.0.md`.

### Added

- Nine Canon `protocol/1` protocols: the eight of the cycle from an idea to a built, tested
  product (idea, intake, direction, epic, spec, plan, story, release) and `sdc-bugfix`, one bug fix
  red first (`protocol/*.protocol.yaml`, `docs/bugfix.md`).
- `sdc-drive` (`driver/`): evaluates a case tree on Canon, runs the tool bound to one admissible
  action, records the result as `canon-evidence/1`. Autopilot (a decider model in place of a
  person, final), a review node as a model call (`change.review`, `50d90b9`), rework after a
  rejected approval (at most two reworked, the third ends the run), a run that ends by rule
  (`rule:autopilot`), a finding re-raised after a decision stops the run `finding_reraised`
  (`6c7ba78`), a model per node class (`SDC_LLM_MODEL_*`), a hermetic `claude-cli` adapter with
  an explicit default effort per node class (`f747085`), and the requester-side decider
  (`decider:requester`, `goals/`).
- `watch/` (`sdc-watch`: file changes of a run) and `ui/` (the live view of a run).
- `scripts/run-finding-cycles.sh` (68 expected cycles and 7 pending on this tree) and
  `scripts/evaluate-cases.sh` (52 situations in 12 fixture cases).
- `docs/reject-routes.md` (the audit of the reject and cap routes), `docs/simulate-findings.md`
  (the triage of Canon's 71 `simulate` findings, 5 real dead ends) and `docs/trial-3.md`.
- Trial 3, 2026-11-21, recorded in runs that are not in git: five of eight batch runs of the
  to-do idea closed delivered, unattended, with an Opus decider and Sonnet work, a Haiku decider
  and Opus work, a mixed Sonnet and Haiku, or an all-Haiku setting; two more runs, with the review
  model call, closed delivered with an Opus decider and Sonnet for all work and the review
  (`docs/trial-3.md`, `docs/releases/0.1.0.md`).
- `CHANGELOG.md`, `docs/releasing.md`, `docs/releases/0.1.0.md` and a root `Taskfile.yml`
  (`task check`, `task release:check VERSION=<version>`).

### Changed

- `ui/package.json` and `ui/package-lock.json` carry the repository version, 0.1.0. The three
  crates and their lock files were already at 0.1.0.
- `README.md` § State names the release; `docs/driver.md` no longer lists `change.review` among
  the program stubs.

### Known limits

- Earlier runs' decisions were taken with the host's settings (the decider's call was not
  hermetic), so the passes are not reproducible on another machine as recorded; no pass ran on
  the hermetic adapter.
- Only two runs through the fixed review node have passed (`trial-3-strong-6`, `trial-3-strong-5`, on `main` plus the
  review fix, with a non-hermetic decider); five passes ran with the review step a stub.
- Nobody started the delivered product.
- The decider's principal is `decider:haiku` whatever model decides, and the driver's "LLM calls so
  far" counts runs that called no model.
- The bug-fix runs ran on a branch of which only `d2e4896` is in this release; the red-check defect
  fixed in `99238c0` is not, and no scripted cycle covers `sdc-bugfix`.
- The driver's runtime parts are a temporary copy of what the Loom runtime must provide.
- Release and operate are being split out of the cycle; that is not in this release.
