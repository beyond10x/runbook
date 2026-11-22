# Changelog

All notable changes to this repository are recorded here. One version covers the repository
(`sdc-drive`, `sdc-watch`, `sdc-goals`, `runbook`, `ui/`, the plugin manifests), released under bare-version
tags such as `0.1.0`. The crates are `publish = false`; releases are source releases at
bare-version tags (`docs/releasing.md`).

## [Unreleased]

### Added

- `runbook mcp` and `runbook serve --mcp` expose the run commands as MCP tools (`submit_run`, `list_runs`, `read_run`, `read_events`, `answer_decision`, `cancel_run`) on the command layer of the daemon: over standard input and output as a client of a running daemon (`--daemon`, `RUNBOOK_DAEMON`), and over streamable HTTP at `POST /mcp` behind the daemon's token check; MCP revision 2026-07-28, with `initialize` for the two revisions before it (`docs/mcp.md`, `story:mcp-exposure`).
- `runbook serve` runs the daemon (`docs/daemon-api.md`, "Running the daemon") on a runtime trait with a stub runtime: the five client commands and the twelve views of `runbookd.yaml` over HTTP and JSON, a journal that survives a restart, a bearer token that every route checks, and `406` for an event view asked as a stream; the runtime is a stub and streams, the UI route and `runbook run` come in later parts (`story:daemon-skeleton-on-a-runtime-trait`, part C1).
- The daemon ends a run that was `Running` or `WaitingOnDecision` when it stopped and whose runtime holds no live run (the stub always) as `Ended` with the outcome `orphaned` and withdraws its Open decision requests, instead of leaving it live (`Runtime::recover`, command `EndOrphanedRun`, event `RunOrphaned`; the run's stream event is `runbook.run-orphaned`, no longer `runbook.orphaned`); a second answer to a decision request is `DecisionAlreadyAnswered` whatever its run is doing; a repeated submit with a known idempotency key names the first run in its body (`published[0].payload.run_id`) as well as in `Location`; the intake limit is stated as 1,048,576 bytes (`docs/daemon-api.md`, `story:daemon-api-specified-in-ess`, decisions 16 to 19).
- The three event views of the daemon (`events`, `session-events`, `daemon-events`) answer as a server-sent event stream when asked with `Accept: text/event-stream`, in place of the `406` of part C1: the SSE `id:` is the event's position, `Last-Event-ID` or `after_position` resumes without loss or repeat, a run stream ends with its run, a session stream with its session, every stream at shutdown, and a client that falls more than 1,024 events behind is closed and resumes by position (`docs/daemon-api.md`, "Event streams"; `story:daemon-skeleton-on-a-runtime-trait`, part C2).
- `scripts/run-finding-cycles.sh <scratch dir> --only <name>[,<name>...]` (and `task cycles:only SCRATCH=<dir> NAMES=<names>`) runs the named finding cycles alone: a cycle that reads an earlier one's output runs that one too and says so, an unknown name ends it with exit 2 before anything runs, and the verdict line says how many were selected (`docs/cycles-suite.md`, "Running named cycles").
- `runbook check` reads the text of a checkout and prints one finding per line, with the file and the fix: the text layout, the two forms of bindings (they parse, name the same existing instruction files per action and run executable tools), the placeholders of the instruction files, the view files, and `cycles/suite.yaml` against the arrays of `scripts/run-finding-cycles.sh`; it exits 1 on any finding and has `--json`. `task check:cycles-suite` runs it instead of its shell, and `CYCLES_SUITE` is replaced by `RUNBOOK_HOME` (`story:runbook-check-of-the-text-layout`).
- The run inspector reads what a change of a runbook's records means (`file_kinds.parsers`) and the decisions a person can take (`decisions`) from the view file too, a run is shown with the view that covers its protocols and never with another runbook's, a run no view covers says which view is missing, and a view with a member the inspector does not know is refused with a message that names it (`ui/README.md`, `story:ui-view-data-parsers-and-actions`).

- The first comparison of `stub-cycle` under the old driver and under Loom `2c0a6f48` is equal on what both sides record (8 cases, 69 evidence records, 54 program runs, 5 person steps, 62 product files; six members Loom's status holds nothing for are named, not compared); `docs/parity.md` holds the facts, what "equal" does not cover and the items of the first-run list that stay open (`story:parity-first-run`, `story:driver-tools-find-binaries-without-build-dir`).
- `runbook parity compare` reads a protocol id as its registration stem (`sdc.intake` equals `sdc-intake`), neither compares nor counts the revisions of a child's `case_outcome` record, and `parity summarize` leaves `.sdc/loom-feedback/` out of the product tree; all three are named rules in `compare --rules` and `docs/parity.md` (`story:parity-first-run`).
- `runbook parity summarize|compare|approvals` reads a finished run of the old driver into a runtime-neutral summary (`runbook.parity-summary/1`), compares two summaries by a table of rules (`compare --rules`) and lists the person steps a second runtime must repeat; it is the driver's half of the comparison with a run under Loom (`story:parity-run-on-loom`, `docs/parity.md`).
- `runbook parity summarize --loom-status <file> [--product-tree <dir>]` reads the JSON of Loom's `tree status` (`loom.tree-status/1`) into the same summary: a status of another format is refused by name, a member it needs and does not find is an error that names the member and the case, and what the status holds nothing for is listed under `not_recorded`, which `compare` reports as `not recorded on one side` and does not compare (`story:parity-run-on-loom`, the table in `docs/parity.md`).
- `runbook serve --ui` serves the web UI from files compiled into the binary (`ui/web/`), beside the API, behind the daemon's token rule: `GET /` is the page, `404` without `--ui`; the page reads the runs and their events from the daemon when it is served by it (`docs/daemon-api.md`, "The web UI")
- `runbook run --detach`, `runbook runs list|show|events|cancel|decisions` and `runbook decide` call a daemon (`--daemon <url>` or `RUNBOOK_DAEMON`, plain HTTP; a token from `--token-file` or `RUNBOOK_TOKEN_FILE`, mode 600): text by default, the route's JSON with `--json`; `runs events` prints the lines of `curl -N` and resumes with `Last-Event-ID` after a dropped connection; exit codes 64 usage, 65 refused, 66 not found, 67 conflict, 69 unreachable, 77 denied, 78 config; `runbook run` without `--detach` exits 64 until `story:engine-on-loom-library-entry` (`docs/cli.md`, `story:run-in-process-and-cli-as-daemon-client`, client part).
- The daemon's one guard now also refuses, on every route with or without `--ui`: a `Host` that is not the listen address on a loopback listener, and an `Origin` that is not the request's own host, with `403`; a command or an MCP call that is not `Content-Type: application/json` with `415`. The cookie of the UI login no longer holds the token (the SHA-256 of a fixed prefix and the token), is refused as a bearer token, and every method on `/ui/login` goes through the guard (`docs/daemon-api.md`, "Requests from a browser"). The build refuses a symbolic link in `ui/web/`.

### Changed

- `runbook serve` refuses to start with a token shorter than 32 bytes (exit 1; the message names the minimum and `--token-file`, never the token); a client takes the token it was given (`docs/daemon-api.md`, "Running the daemon").
- The specification of the daemon (`ess/domains/daemon.yaml`) now says what the daemon does: `SubmitRun` writes the run's first event (`runbook.submitted`, sequence 1, so the runtime's first record is sequence 2), and `CancelRun` and `EndOrphanedRun` withdraw the run's Open decision requests in the same step (an answer to one is `DecisionWithdrawn`); all 12 scenarios of the daemon now pass against the running daemon and on the interpreted target, where 6 failed against the daemon before (`docs/daemon-api.md`; `story:daemon-synthesized-suite-has-no-refusal`, part C4).
- `runbook doctor` checks the text layout by name (`bindings/`, `bindings/*.yaml`, `bindings/tools/`, `instructions/`, `views/`, `cycles/` beside the earlier directories) and the four tools at `bindings/tools/`, and cites no line numbers; `runbook --version` prints the repository version (0.3.0, not 0.1.4) and `task release:check` reads `runbook/Cargo.toml` (`story:runbook-command-knows-the-text-layout`).
- The model instructions of `sdc-reply implement-run` (request mode), `sdc-conform` and `sdc-unstick` are files under `instructions/` (`change.implement.request.*`, `conformance.table*`, `run.unstick*`), read at run time through one renderer with the rules of `llm-node.sh`; the prompts are byte-identical, held by goldens under `instructions/golden/`. `sdc-conform prompt` prints the table node's prompt without a model call (`story:rust-prompts-as-instruction-files`).
- `scripts/check-protocols.sh` ends in under a minute instead of about 10: it checks the digest of each protocol's sources against the committed `protocol/check-ledger.json` and runs `canon check`, the gate probe and `canon simulate` only for a protocol whose digest changed (`--record` writes the ledger, `task check:protocols:slow` regenerates it in full and compares bytes), prints the time per protocol and fails above 120 s (`story:check-protocols-budget`).
- The fast lane of `scripts/check-protocols.sh` is part of `task check`; each ledger entry carries a seal over its digest and results, so a hand edit of `protocol/check-ledger.json` fails it; `--record` and `--repin` refuse `SDC_SIMULATE=off`; CI runs the slow lane when `protocol/**`, `driver/canon-pin.json` or the script change (`.github/workflows/protocols.yml`).

- `task check` runs the UI test step (`task test:ui`, Playwright, 112 tests) beside the driver's (`task test:driver`); on the shared pool the two together took 105 s of the 120 s each step may take (the UI step alone 77 s, the driver's alone 59 s).
- The UI server reads Canon from `SDC_UI_CANON` and the driver from `SDC_UI_DRIVE` when set (one place, `ui/binaries.mjs`, shared with the test fixtures); a program that is not there is a refusal that names its path.

### Fixed

- A test step that never got a slot in the build pool is no longer recorded as red tests and no longer sends the writer to repair them: `sdc-tool.sh` bounds the wait itself (`--pool-wait`, one bound per step, the pool's own 10 minutes still bound the run) and exits 69 when the job was still queued, `sdc-drive` runs such a step up to 3 times, records nothing and ends with the verdict `pool_wait_spent` (exit 5), `sdc-run-metrics` counts `pool_waits` apart from `could_not_run`, and the limits of story `tests.run` (3000 s) and release `acceptance.run` (2400 s) cover wait plus run (`docs/driver.md`, "The build pool wait", `story:pool-wait-is-not-a-failure`). A job the pool ends at its run limit (slot 0.22: exit 124) is its own result too, exit 70 "ran over its time": not red tests, not a pool wait, the step is not run again and the tree stops with `run_over_time` (`ran_over_time` in `sdc-run-metrics`). Under Loom both exits stay `failed` until its loader has `exits` (`bindings/PENDING.md`).
- The bug-fix steps `repro.run`, `tests.run` and `gate.run` no longer count the wait for the build pool as a failed run: `sdc-bugfix` bounds the wait of its jobs itself (`--pool-wait 1200` in the bindings, one bound per step, the environment variable `SDC_POOL_WAIT` wins), cancels a job still queued at the bound and exits 69, and exits 70 for a job the pool ended at its run limit, with the same driver behaviour as for story and release (3 runs then `pool_wait_spent`; `run_over_time` at once; no evidence, no `change.fix`). The limits are 3000, 3000 and 4200 s (`docs/driver.md`, "The build pool wait", `story:pool-wait-is-not-a-failure`).
- The UI server keeps one view per request and per tick (an `AsyncLocalStorage` scope read by `ui/web/view.js`), so an action that waits in its tool is no longer given the view of another source's request that arrived meanwhile; a stage name or phase id from a view is escaped in `ui/web/lenses.js`; four reasons in a committed fixture name the role instead of a first name.
- `runbook parity summarize` exits 2 and names the file for an input it cannot read or parse (a truncated event file, a malformed evidence record, a missing member) and no longer drops it; its digests use the `sha2` crate.
- The shell tools no longer fall back to a `beyond10x/sdc` path for `sdc-findings` and `sdc-finding-kpis` (a missing one stops with its name and where it looked), `llm-adapter-claude-cli.sh` runs with `HOME` unset (`--claude-credentials <file>` on `llm-node.sh`, else a message naming the variables), and the text of `in/feedback.md` and the requester request is in `instructions/` (`story:tool-texts-and-paths`).

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
