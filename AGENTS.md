# Runbook prototype

## What it is

- Runbook: agentic runbooks. A runbook is a fixed, checked way to handle a case. A program decides
  the next step from recorded facts; a model carries out single steps. The product name is
  Runbook (the operator, 2026-11-21: "Yes, runbook. Agentic runbook."); the repository is
  `beyond10x/runbook` (stage 3); the code, binaries, environment names and format ids still carry
  `sdc` until stage 2 of `story:project-rename`.
- The software delivery cycle (SDC), the first runbook, takes an idea to a built, tested product.
  Other runbooks are planned, not built: incident handling, investigations, ticket triage.
- The SDC is a family of eight Canon `protocol/1` documents under `protocol/`, one case per stage, composed
  with child cases: idea, intake, direction, epic, spec, plan, story, release (`docs/protocol-notes.md:3-16`).
- The stage is derived from facts (files in the repository, recorded run results), never from a
  stored phase; a passing ESS gate marks the spec complete (`docs/phases.md:3-6`, `:34-35`).
- `runbook run` (`runbook/src/loom_run/`) runs one case tree through Loom on Canon: it reads
  `canon-decision/1`, runs the tool bound to one admissible action, records `canon-evidence/1`,
  evaluates again (`docs/loom-run.md`; `docs/driver.md:3-11`).
- An LLM is only ever one action's executor; its output is an artifact revision, never evidence
  and never the next state (`docs/driver.md:3-11`).
- Runbook is an application on Loom, and Loom is the runtime: the driver's temporary copy of it is
  deleted, and `driver/` is the crate `sdc-tools` of the bound programs and tools (`docs/layers.md`,
  "State now"; `docs/driver.md`, "Now").

## Where Runbook is going

- The target (the operator, 2026-11-22): one prebuilt `runbook` binary that links Canon and Loom as
  crates and needs no `canon` or `b10x-loom` at run time. `runbook run` runs a case in
  process; `runbook serve` (also `runbookd`) is a daemon for many runs with an API specified in ESS,
  SSE events, MCP and the embedded UI.
- Each run has a confinement level (default confined) and the daemon has a floor below which it
  refuses. The binary carries the runbooks as presets that the evaluations measure and improve.
- The driver runtime (its main file, the trial and cycles scripts, the model-node and decider
  shell tools) is deleted: a run on Loom matched the driver's (`story:parity-run-on-loom`), then
  `story:driver-runtime-removed` removed it. `runbook run` is the way to run a case. The pinned Loom
  commit is `ed644a4d3487d4f05120aa705085b9e143905982` (`ed644a4d`; Loom's tag `0.61.0`; it replaced `d8944d49`, and `c9b4c177` and `db2726ff` before it), for the library the
  crate links; the child binary of `runbook run --loom` is still the one of `70b0e6e0` (`docs/loom-run.md`). A run of `stub-cycle` through `runbook loom-run` (the former name of `runbook run`) on `5af3f9c2`
  (`--confinement local`, scripted approvals, no model) compared `parity: identical` with the
  driver's (`docs/parity.md`, "Identical comparison on Loom `5af3f9c2`"); it does not cover a run
  with a model. The first equal comparison was made against `2c0a6f48` (`docs/parity.md`, "First
  equal comparison").
- The plan is `epic:runbook-one-binary-and-daemon`; read its stories in order before work on the
  runtime, the UI server or the release.

## Directory map

- `protocol/` the eight stage protocols and `sdc-bugfix` (one bug fix, red first: `docs/bugfix.md`), `compiled/` IR, `probes/`, `draft/` (`orchestrate.protocol.yaml`).
- `bindings/` the nine bindings in Loom's `loom.bindings/1`, run by `runbook run`; `PENDING.md` lists what the format lacks (`docs/loom-bindings.md`). `driver/examples/sdc/*.bindings.json` are the source of the documentation site's data. `bindings/tools/` the programs the bindings run (`sdc-tool.sh`, the adapters, `node-classes.json`): Loom mode when `LOOM_CASE` is set; the model step is the program `sdc-node`.
- `instructions/` the model's instruction texts, one file per prompt part (`<action>.md`, `shared/`, `decider/`), read and filled by `sdc-node` and by the runner's own decider (`runbook/src/loom_run/decider/`, reads `decider/` too), and, for `change.implement.request.*` and `conformance.table*`, by the Rust binaries `sdc-reply implement-run` and `sdc-conform` through `driver/src/instructions.rs`; `golden/` holds the prompt each makes on stub inputs, checked byte for byte by `driver/tests/bugfix_prompt.rs` (`docs/driver.md`, "Instructions").
- `cycles/` the finding cycles as data: `cycles/tree-suite.yaml`, run by `runbook test` (`task test:cycles-loom`; `docs/cycles-suite.md`); `task check:cycles-suite` runs `runbook check`, which checks the text layout (bindings, instruction files, views). A cycle builds nothing: `cycles/recorded/` holds the build and test results that the suite's stand-in `slot` (`sdc-recorded-pool`) answers from, keyed by the digest of command and tree; after a change to a tree a cycle builds, `task test:cycles-record` (background, no `slot` prefix) records the missing ones.
- `cases/` fixture cases evaluated by `scripts/evaluate-cases.sh`.
- `driver/` Rust crate `sdc-tools`, with the library `sdc_tools` (`driver/src/lib.rs`; `driver/src/reply/` holds what `sdc-reply` does, `runbook` links it) (plus `sdc-findings`, `sdc-finding-kpis`, `sdc-run-compare`: `docs/run-compare.md`; `sdc-bugfix`: `docs/bugfix.md`; `sdc-run-metrics`, `sdc-experiment`: `docs/workflow-metrics.md`; `sdc-run-timeline`: `docs/llm-is-not-the-runtime.md`; `sdc-conform`, `sdc-conform-adapter`: `docs/conformance.md`); bindings in `driver/examples/sdc/`; `driver/ess-pin.json` pins the ess; the Canon that Runbook calls is the library `runbook` links (`runbook canon`, `runbook/src/canon_cmd.rs`; the pin is `runbook/Cargo.lock`; no script builds or enters a Canon checkout).
- `experiments/` experiment records (`sdc.experiment/1`), their reports and run summaries, and `INDEX.md`; launched, checked and reported by `sdc-experiment`. `experiments/releases.yaml` is the release table of the progress ledger (`docs/progress.md`, `docs/img/progress.svg`, written by `sdc-experiment progress`; the loop: `docs/improving.md`).
- `watch/` Rust crate `sdc-watch`: filesystem changes of a run (`docs/fs-changes.md`).
- `runbook/` Rust crate `runbook`, the command people type: `runbook doctor`, `runbook list`, `runbook check` (the text layout of a checkout; `task check:cycles-suite` runs it), `runbook --version`, `runbook parity` (`src/parity.rs`: summarize, compare, approvals; `docs/parity.md`) and `runbook run` (`src/loom_run/`; in this process, `here.rs` is its option list, and `--detach` is the daemon client; the same code was first the subcommand `loom-run`: the loop, the caps and the spend ledger, the decider, the answers file `bindings/tools/answers.yaml` (`answers.rs`, `decider/ask.rs`, `run_loop/declared.rs`, the program `driver/src/bin/sdc-answers.rs`; given through the host, so the same on the child and in the library), the copy of the bindings, the private directory of the login link, the run directory writer; `docs/loom-run.md`, `docs/loom-run-settings.md`, `docs/loom-run-directory.md`, `docs/loom-run-decisions.md`; tests in `runbook/tests/loom_run/` against a fake `b10x-loom`; with no `--loom` it runs through Loom as a library, with the gate tests in `runbook/tests/loom_run/inproc.rs`; the slow lanes are `task test:loom-real` against the real binary, `task test:loom-in-process` comparing the two ways and `task test:replay`, the nine tests of `runbook/tests/loom_run/replay.rs` that read the six re-drives of `runbook run --recorded` made once by the nextest setup step `replay-runs`, and `task test:bugfix-slow`, the two `#[ignore]`d bug-fix tests of `runbook/tests/loom_run/bugfix.rs`). `ui` and the other commands are not built (`story:runbook-cli-skeleton`).
- `ui/` Node live view of a run (`ui/README.md`).
- `views/` the view file each runbook ships for the inspector: phases, file kinds, decisions (`views/sdc.view.json`; `ui/README.md`, "The view file").
- `decision/` decision-contract examples (`docs/decisions.md`).
- `ess/` ESS specification of the decision contract (domain `sdc.decision`).
- `eval/` levels, personas, quality checks and reports; the scripts that drove the old driver are deleted, and `reports/` and `baseline.json` are the record of its evaluation, not ported (`eval/README.md`, `docs/eval-design.md`).
- `runs/` recorded runs: case stores, event logs, LLM and decider records.
- `scripts/` check and evaluate scripts, and `scripts/install.sh --prefix <dir>` (POSIX sh: builds `runbook`, the helper programs a run starts and `sdc-watch` with `--locked --release` and writes them with the asset directories under one prefix, helpers in `libexec/runbook/`; its tests are `runbook/tests/install/`, the release build is `task test:install`).
- `tools/` decision-contract shell tools (`decision-grounds.sh`, `decision-from-record.sh`).
- `docs/` design notes and research results; start with `phases.md`, `loom-run.md`, `orchestration.md`.
- `plugin/` the `runbook` plugin (skills `workflow-builder`, `harden`, `record`, `delegation`; marketplace `.claude-plugin/marketplace.json`; `plugin/README.md`; for Claude Code only, the MCP server `runbook mcp` in `plugin/.mcp.json` and the hook `plugin/hooks/delegation.sh` that denies every Agent call, `plugin/hooks/hooks.json`, tested on recorded hook input by `driver/tests/plugin.rs`). A command or path in a `SKILL.md` must be checked against the script or binary it names before it is written; `driver/tests/plugin.rs` fails on a missing path or a version that differs from the release version.
- `.engineering/planning/` the AEP planning store.

## Rules

- Anything new that runs is Rust (the organization's language rule). Never Python. The existing bash and
  jq tools (`bindings/tools/`, `scripts/`, `tools/`) are prototype glue: they get fixes,
  no new bash tool is added, and each is retired by a Rust binary or by Loom.
- Every build and test runs through `slot`. The `cargo` shim does it; prefix anything else
  (`slot npx playwright test --workers=2`). Never set `-j` or `CARGO_BUILD_JOBS`.
- The driver tests run as `slot task test:driver` (cargo-nextest, `driver/.config/nextest.toml`; the forge runs the same task in `.github/workflows/pages.yml` for a code change, after steps that install the pinned ess with `sdc-conform ess-install` and the pinned aep (`driver/aep-pin.json`) with `sdc-conform aep-install`, with the nextest profile `forge` that `CI` picks; `DRIVER_TEST_FORGE=1 slot task test:driver` runs it on 2 CPUs as `RUNBOOK_TEST_FORGE` does; like `test:runbook` it puts the traps `ess` and `aep` first on the `PATH` of its tests, and the directory of the pinned aep (`sdc-conform aep-path`) ahead of them; run `cargo run --locked --manifest-path driver/Cargo.toml --bin sdc-conform -- aep-install` once per host): a test over 60 s is killed and fails, the ten slowest are printed, the run fails over 120 s. `driver/tests/main.rs` is the one integration-test binary: a new test file needs a `mod` line there (`autotests = false`); each test is its own process, so a product made once per run is a cache on disk under `driver/target/` (see `schemas()` in `driver/tests/decision_request.rs`).
- The `runbook` crate tests run as `slot task test:runbook` (cargo-nextest, `runbook/.config/nextest.toml`; the forge runs the same task in `.github/workflows/pages.yml` for a code change): a test over 60 s is killed and fails, the ten slowest are printed, the run fails over 120 s. `runbook/tests/cli.rs` is the one integration-test binary (the daemon's tests are modules under `runbook/tests/daemon/`). The daemon reads one clock (`runbook/src/clock.rs`): a test that needs an exact instant builds the daemon in process with `Daemon::start_with_clock` and a `SetClock`; no option, environment variable or request sets a clock. A test that needs the wall cap of a run spent at one moment (while a given program runs) builds the engine with `Engine::with_wall_clock(WallClock::standing())` and moves that clock with `advance`, never a sleep against the host clock (`runbook/tests/daemon/caps.rs`). Both test tasks pass `--color never` to every nextest call whose log they parse (the forge sets `CARGO_TERM_COLOR=always` through `dtolnay/rust-toolchain`); `task check:ci-paths` fails when a nextest call of `Taskfile.yml` lacks it. `test:runbook` puts a trap `ess` (`scripts/host-tool-traps/ess`, exit 127) first on the `PATH` of its tests, so a test that reaches the host's `ess` fails on this host as it does on the forge: a test brings its own stand-in `ess` and its own `PATH` (`exit_classes` does).
- The forge's runner has 4 CPUs and runs `test:runbook` with the nextest profile `forge` (4 test threads; `CI` set picks it; the task drops a `NEXTEST_TEST_THREADS` of the environment for it, such as the build pool's 16). `RUNBOOK_TEST_FORGE=1 slot task test:runbook` runs the same profile on 2 CPUs of the host it runs on (`taskset`), which comes closest to the time the forge measures; other work on those 2 CPUs slows it, so the task prints the load average before and after. Before the fix the forge's step took 123 s, 2 CPUs of the build node 127 s, all 64 of them 91 s. The step is bound by CPU time, so a test that waits costs it little and a test that computes costs it much: the tests start the `runbook` binary and the shell stand-ins about 1600 times. The dependencies of `runbook/` build at `opt-level = 1` in the dev and test profiles (`runbook/Cargo.toml`), the fake `b10x-loom` (`tests/loom_run/fake.rs`) and the model-call hooks (`model_call_hook`, `RECORD_DIR` in `tests/loom_run/core.rs`) start as few processes as they can, and a case proves a bound with a small input where the bound is a parameter.
- The forge runs go-task of the `TASK_VERSION` in `.github/workflows/pages.yml` (3.52.0); this host may have another. A step of `Taskfile.yml` that prints a runner's output opens the output again for one command or one group (`{ ...; } >> /dev/stdout 2>> /dev/stderr`), never by `exec` with redirections only: the interpreter of go-task 3.52 closes the file that form opens at the end of its line and the block writes on into the closed file, so not one line of the runbook step after the build reached the forge's log (pull request 78). Opening the output again keeps the lines when node has made the shared output of `task check` non-blocking. `task check:task-output` (part of `task check`, no network) fails on that form and runs the test steps with stand-ins the way of `task check` under the local go-task. When a shell block of `Taskfile.yml` or `ui/Taskfile.yml` changes, also run `slot task check:task-output:forge` (not part of `task check`): it runs those steps and the build, clippy and test loops the forge's way with the forge's go-task, fetched once into `target/forge-task/<version>` and checked against `TASK_SHA256`, and fails when a line does not arrive.
- The engine (`runbook/src/engine/`, `docs/engine.md`) is the Loom runtime of the daemon: library calls into the pinned Loom (`runbook/Cargo.toml`, `rev`), the per-run store under the daemon's state directory, no Loom or Canon process. The wrapper before a bound program starts is `runbook/src/engine/announce.rs`, the one hook of both runtimes: `runbook run` writes the `action_started` line, the daemon records the event `runbook.action-started` and shows the step in flight in the view `run` (`docs/daemon-api.md`). The gate-lane tests are `runbook/tests/daemon/engine.rs` over `runbook/tests/fixtures/engine-runbook/`; the slow lane is `task test:engine-real`. Cargo reaches Loom through its forge URL; a local build that needs a clone uses `GIT_CONFIG_*` `url.<clone>.insteadOf` in the shell only, never a `[patch]` or a path in a committed file.
- MCP (`runbook mcp`, `runbook serve --mcp`; `runbook/src/mcp/`, `docs/mcp.md`) is a face of the command layer: a tool calls `api::routes::command_answer` or `view_answer` and returns the route's body, and its input schema is derived from the OpenAPI document, never typed by hand; a new tool is a row in `mcp/tools.rs`.
- The local gate equals the forge for each test step of `task check` that fits the 120 s at the forge's CPU count: the forge runs `task test:runbook` and `task test:driver`, not `task test:ui` (`docs/forge-gate.md`). Measured on 2 CPUs of the build node at 4 workers, the UI run had 550 to 587 of its 611 tests done when its 120 s bound ended it (412 to 413 of 609 before the cuts of story `the-ui-step-fits-the-forge-budget`; all 611 in 70 s to 93 s on all CPUs). The UI tests run with `reducedMotion: 'reduce'` (`ui/playwright.config.mjs`): under it `ui/web/style.css` stops every animation and transition and the blur behind a dialog or the drawer, each of which made the browser draw the page frame after frame; the ux loop keeps `no-preference`: a test-budget defect of `test:ui`, which goes into the forge when it fits; until then a UI change is checked by the local gate only. `UI_TEST_FORGE=1 slot task test:ui` measures it the forge's way. `task check:ci-paths` fails when a `test:` dependency of `task check` has neither a forge step `run: task <name>` nor an `OUTSIDE` line with its reason, when a forge test step does not run for a code change or runs for a planning or docs-site change, names a `timeout-minutes` above the runbook step's, or its task lacks the `CI` way and a `*_TEST_FORGE` way.
- The UI test step is `task test:ui` at the root (`task test` in `ui/`; part of `task check`, beside `test:driver`; Playwright, four workers, 60 s a test, 120 s the run, the ten
  slowest printed). It needs no `runs/`, no built binary and no server of your own; its servers take free
  ports. Specs that need those are the slow lane (`task slow`, listed in `ui/test/lanes.mjs`).
  `ui/test/lanes.mjs` is the one list of which spec runs at which size: a file in `WIDE_ONLY` runs at 1600x1000 only (the laptop project ignores it, so no skipped entry waits for the end of the run), a file in `BOTH` runs at both sizes; `ui/test/budget.spec.mjs` fails when a spec is in neither list or in both, or when a wide-only spec that names a size, a position or a visibility matcher of the list in `ui/test/lanes.mjs` lacks an entry in `SIZE_EXCEPTIONS`. A spec waits for an event (`settled` in `ui/test/settled.mjs`, the hold file that `fixture-run.mjs` gets as its fourth argument); a sleep (a `waitForTimeout`, a `setTimeout` that a `new Promise` with an arrow function waits for or a window `Date.now() + <number>`, counted per call site in the gate's specs and every file they import by a relative path; the spellings the check does not find are listed in `ui/README.md`) is made only where `FIXED_WAITS` in `ui/test/lanes.mjs` names the file, the count and the sum of the literal milliseconds, and `budget.spec.mjs` fails any other count or sum, and a sleep that takes its time from a helper's argument unless the entry says `variable` with a reason, the helper and the largest number its call sites pass; a beforeAll plays its fixtures and starts its servers at once (`playFixtures` in `ui/test/fixtures.mjs`, `Promise.all` over `startServer`).
  A test child (server, fixture player, daemon) is started with `spawnChild` or `startServer` of `ui/test/child.mjs` and `ui/test/serve.mjs`, never with `spawn`: it ends with its parent, also after SIGKILL (`ui/test/guard.mjs`, `ui/test/child.spec.mjs`).
- Anything that runs longer than about 30 s runs in the background.
- Layers: Canon is the kernel (protocol semantics, evaluation), Loom the runtime, Substrate
  confinement and process start (`docs/layers.md:55-60`). A capability missing in one of them is
  requested from that repository's session, not patched here. Record the request in
  `docs/canon-requests.md` or the planning store (stories marked blocked upstream).
- Planning lives in `.engineering/planning/` and changes only through the `aep` CLI
  (`aep plan artifact new|move|relate|body|validate|waves`). Never hand-edit an artifact file.
  Body scratch files go in `.engineering/drafts/` (git-ignored).
  Every write runs with `AEP_ACTOR=human:operator` and `--executor agent:<session>` where the verb
  takes it: without it aep records `human:$USER`, the login name of a person, into a file of this
  public repository (2026-11-25, a story move caught before its push).
- Protocol changes are checked with `scripts/check-protocols.sh` and `scripts/check-bindings.sh`.
  A protocol change runs the simulate step; a changed finding set is re-pinned (`--repin`) in the
  same commit with the reason in the commit body (`docs/protocol-notes.md`, "Simulate").
  A new `revision` of a protocol file moves the registration (`<id with . and _ as ->@<revision>`)
  in `bindings/<name>.yaml` and in every `copy` `from:` in the same commit;
  `scripts/check-bindings.sh` refuses a mismatch.
  `scripts/check-protocols.sh` is a digest check against the committed `protocol/check-ledger.json`
  (the 14 `canon check` calls cost about 590 s and are not repeated for an unchanged protocol): after
  a change to a protocol, the Canon pin in `runbook/Cargo.lock` or the script, run `scripts/check-protocols.sh --record`
  and commit the ledger; `task check:protocols:slow` regenerates it in full and compares bytes.
  The fast lane (under 1 s) is part of `task check`; each ledger entry carries a seal over its digest
  and result fields, so a hand edit of a result fails it. CI runs the slow lane
  (`.github/workflows/protocols.yml`) when `protocol/**`, `runbook/Cargo.lock`, `runbook/src/canon_cmd.rs` or the script changed.
- After a merge, `scripts/check-bindings.sh` runs before the push (build the driver first): a merge
  can leave `website/data/protocols/index.json` stale without a conflict.
- A run that `sdc-experiment run` starts is a systemd unit, which has `LANG=de_DE.UTF-8` on this
  host. A tool that reads a number printed by bash or a program does not depend on the decimal
  separator (`EPOCHREALTIME` holds a comma there). A run that passes in a terminal and fails in the
  unit is checked for the locale first.
- An LLM call whose output is a data document is made with a response schema and its reply is
  validated by a program against the same schema; an invalid reply is retried with the validator's
  error at most twice and then fails with a named cause. Free-form outputs (code, prose) are the
  only exception and are listed in `docs/driver.md` (the operator, 2026-11-21).
- Cite `path:line` for facts in docs; mark what was not observed as *inferred*.
- No `/home/<name>/` path in any committed file: write repository-relative paths, sibling-relative
  paths (`../canon/…`) in docs, and `$HOME`-based or argument paths in scripts.
- A run that cannot continue (a defect that needs a code change, a missing capability, or a
  decision nobody will take) is stopped at once by whoever finds it. A run of an experiment record
  is stopped with
  `sdc-experiment stop <record | id> --run <run name> --reason "<reason and the fix it waits on>"`
  (it writes that run's stop record and stops its unit; without `--run` it does so for every open
  run of the record and sets the record `abandoned`; `docs/workflow-metrics.md`). Any other
  `runbook run` is stopped with SIGTERM or SIGINT (it ends as `stopped: signal`, exit 7;
  `docs/loom-run.md`).
  No run is left waiting when no person or agent is working on it. Demo and fixture runs are never
  listed beside real runs without a label (the operator, 2026-11-21).
- Keep-going (the driver's option to collect every defect of a run in `defects.jsonl`) is not
  ported: `runbook run` has none. A forced run of the
  former driver (`forced N` in `status.json` and the closed line) is evidence for defects and never
  counts as an end-to-end pass; the pass is a later run without the flag (the operator,
  2026-11-21).
- A real run belongs to an experiment record `experiments/<id>.yaml` (`docs/workflow-metrics.md`
  § 7), whose report and `experiments/INDEX.md` account for it. The operator, 2026-11-21: "make
  sure to organize these trials in a somewhat scientific and organized, manageable and scaling
  manner. It shouldn't end up messy."
- No run is started by hand: `sdc-experiment run <id>` starts them, on the record's commit, with the
  record's settings (the operator, 2026-11-21, same words). Stop one with `sdc-experiment stop`.
- CI classifies the changed files in one place, the step "Classify the changed files" of `.github/workflows/pages.yml` (planning, docs-site or code; a planning-only change builds and tests nothing); a change to that path list changes `task check:ci-paths` in the same commit.
- `runbook run` has no money limit built in (the operator: "runbook should not have any limitations in terms of money cap - there is basically no technical limitation at all"). `--spend-ledger`, `--cap-micro-usd` and `--max-llm-calls` are each optional for every run and every confinement; none named means no limit, no refusal and no ledger. A cap a user names stops the run (exit 4, exit 6). Add no option, default, level rule or refusal that requires or implies a cap (`docs/loom-run.md`, "The money options").
- The rustc version is pinned in `rust-toolchain.toml`, and the pages workflow's `toolchain:` repeats it; change both in one commit. The step "Check the rustc pin" of `.github/workflows/pages.yml` fails when the workflow's `toolchain:` value differs from the channel in `rust-toolchain.toml` (it reads both as text; inside the repository rustup runs the pinned channel whatever the workflow installs).

## Commits

- Every commit and push is `b10x-bot[bot]`'s (author and committer
  `b10x-bot[bot] <316511680+b10x-bot[bot]@users.noreply.github.com>`), made with
  `b10x-gates bot -- commit -F <message file>` and `b10x-gates bot -- push`. It mints a token from
  the organization App's credentials. Every other GitHub write goes through
  `b10x-gates api` or `b10x-gates gh`.
- A pull request is merged only by a bot merge commit made locally (`b10x-gates bot -- merge --no-ff -F
  <message file> <head>` on a tree at `origin/main`) and pushed with `b10x-gates bot -- push origin
  <merge>:refs/heads/main`, never by the GitHub merge API or the web button. 2026-11-25: PR 88 merged
  through the API left a commit with GitHub as committer on `main`, and the bot's push of every range
  holding it was refused ("branch authority missing or ambiguous": the repository has no ruleset
  `b10x-bot-branch-authority`).
- No attribution lines. Every commit has a subject and a body.
- Common Gates: hooks with `b10x-gates --repository beyond10x/runbook --policy ../gates-policy/policy.json
  install`; before a push, `b10x-gates --repository beyond10x/runbook check --receipt <file>`. CI runs
  `.github/workflows/shared-gates.yml`, the reusable Gates workflow pinned by commit. Install and
  check need the repository enrolled in `gates-policy` (GitHub id and adoption baseline, the first
  commit).
- Recorded runs (`runs/`, `eval/runs/`, `eval/reports/*-runs/`) stay out of git: they hold absolute
  paths, LLM transcripts and prompts (`.gitignore`).

## Sessions (the operator, 2026-11-21)

- The main session only orchestrates (plans, dispatches, verifies, reports): it writes no code, edits no file, runs no long build or test in the foreground and is never blocked on one.
- Implementation, research, drafting, scoping and reviews run in subagents, model by difficulty: Opus for design, hard implementation and diagnosis; Sonnet for ordinary implementation and research; Haiku for sweeps and mechanical edits. No subagent runs on the main session's own model.
- Epics and stories created in this store from the operator's requirements are accepted on creation (standing rule of 2026-11-21, `approval-record:operator-standing-acceptance-2026-11-21`): create them and move them to `proposed` in the same step. Upstream blockers stay blockers.
- The project's name is Runbook, decided on 2026-11-21; the rename runs in three stages
  (`story:project-rename`).
- Before working in another repository from this session, list the running sessions; when a session for that repository exists or appears, stop the own agent there at once with the stop tool, not with a message (a message waits behind the agent's running command), and hand the work to that session with the list of everything touched.

## Work order (the operator, 2026-11-21)

"So we're probably focusing first on the non-orchestration bits." In order:

1. Non-orchestration work: late-ambiguity loop merge (`next/` into the tree, `docs/trial-2.md`),
   trial 2, decision contract (`docs/decisions.md`), file-change view (`docs/fs-changes.md`,
   `watch/`), eval (`docs/eval-design.md`, `eval/`).
2. Then the orchestration epics (`docs/orchestration.md` § 7; `aep plan artifact list --kind epic`).
   They are drafts; the operator accepted the design on 2026-11-21, to be implemented "when it's time for it".
