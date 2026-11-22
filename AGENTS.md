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
- `sdc-drive` (`driver/`) runs one case tree on Canon: it reads `canon-decision/1`, runs the tool
  bound to one admissible action, records `canon-evidence/1`, evaluates again (`docs/driver.md:3-6`).
- An LLM is only ever one action's executor; its output is an artifact revision, never evidence
  and never the next state (`docs/driver.md:5-6`).
- Runbook is an application on Loom; the driver's runtime parts are a temporary copy of what Loom
  must provide (`docs/layers.md:51`; `docs/driver.md:43-45`).

## Where Runbook is going

- The target (the operator, 2026-11-22): one prebuilt `runbook` binary that links Canon and Loom as
  crates and needs no `canon`, `b10x-loom` or `sdc-drive` at run time. `runbook run` runs a case in
  process; `runbook serve` (also `runbookd`) is a daemon for many runs with an API specified in ESS,
  SSE events, MCP and the embedded UI.
- Each run has a confinement level (default confined) and the daemon has a floor below which it
  refuses. The binary carries the runbooks as presets that the evaluations measure and improve.
- The driver runtime (`driver/src/main.rs`, `scripts/run-trial.sh`, `scripts/run-finding-cycles.sh`,
  `bindings/tools/llm-node.sh`, `bindings/tools/decider.sh`) is deleted once a run on Loom matches
  the driver's (`story:parity-run-on-loom`, then `story:driver-runtime-removed`). The pinned Loom
  commit is `5bcc26d9`, for the child binary of `runbook run --loom` and for the library the
  crate links. A run of `stub-cycle` through `runbook loom-run` (the former name of `runbook run`) on `5af3f9c2`
  (`--confinement local`, scripted approvals, no model) compared `parity: identical` with the
  driver's (`docs/parity.md`, "Identical comparison on Loom `5af3f9c2`"); it does not cover a run
  with a model. The first equal comparison was made against `2c0a6f48` (`docs/parity.md`, "First
  equal comparison").
- The plan is `epic:runbook-one-binary-and-daemon`; read its stories in order before work on the
  runtime, the UI server or the release.

## Directory map

- `protocol/` the eight stage protocols and `sdc-bugfix` (one bug fix, red first: `docs/bugfix.md`), `compiled/` IR, `probes/`, `draft/` (`orchestrate.protocol.yaml`).
- `bindings/` the nine bindings in Loom's `loom.bindings/1`, not run yet (the driver runs `driver/examples/sdc/*.bindings.json`); `PENDING.md` lists what the format lacks (`docs/loom-bindings.md`). `bindings/tools/` the programs both formats run (`sdc-tool.sh`, `llm-node.sh`, the adapters, `decider.sh`, `node-classes.json`): driver mode, or Loom mode when `LOOM_CASE` is set.
- `instructions/` the model's instruction texts, one file per prompt part (`<action>.md`, `shared/`, `decider/`), read and filled by `bindings/tools/llm-node.sh` and `decider.sh`, and, for `change.implement.request.*`, `conformance.table*` and `run.unstick*`, by the Rust binaries `sdc-reply implement-run`, `sdc-conform` and `sdc-unstick` through `driver/src/instructions.rs`; `golden/` holds the prompt each makes on stub inputs, checked byte for byte by `driver/tests/bugfix_prompt.rs` (`docs/driver.md`, "Instructions").
- `cycles/` the finding cycles as data: `cycles/suite.yaml` lists every cycle of `scripts/run-finding-cycles.sh` with its state; `task check:cycles-suite` runs `runbook check`, which holds the two equal and checks the rest of the text layout (bindings, instruction files, views).
- `cases/` fixture cases evaluated by `scripts/evaluate-cases.sh`.
- `driver/` Rust crate `sdc-drive` (plus `sdc-findings`, `sdc-finding-kpis`, `sdc-run-compare`: `docs/run-compare.md`; `sdc-bugfix`: `docs/bugfix.md`; `sdc-run-metrics`, `sdc-experiment`: `docs/workflow-metrics.md`; `sdc-run-timeline`: `docs/llm-is-not-the-runtime.md`; `sdc-conform`, `sdc-conform-adapter`: `docs/conformance.md`); bindings in `driver/examples/sdc/`; `driver/ess-pin.json` pins the ess and `driver/canon-pin.json` the Canon that Runbook calls (`canon_resolve` in `scripts/evaluate-cases.sh` checks the version; no script builds or enters a Canon checkout).
- `experiments/` experiment records (`sdc.experiment/1`), their reports and run summaries, and `INDEX.md`; launched, checked and reported by `sdc-experiment`. `experiments/releases.yaml` is the release table of the progress ledger (`docs/progress.md`, `docs/img/progress.svg`, written by `sdc-experiment progress`; the loop: `docs/improving.md`).
- `watch/` Rust crate `sdc-watch`: filesystem changes of a run (`docs/fs-changes.md`).
- `runbook/` Rust crate `runbook`, the command people type: `runbook doctor`, `runbook list`, `runbook check` (the text layout of a checkout; `task check:cycles-suite` runs it), `runbook --version`, `runbook parity` (`src/parity.rs`: summarize, compare, approvals; `docs/parity.md`) and `runbook run` (`src/loom_run/`; in this process, `here.rs` is its option list, and `--detach` is the daemon client; the same code was first the subcommand `loom-run`: the loop, the caps and the spend ledger, the decider, the answers file `bindings/tools/answers.yaml` (`answers.rs`, `decider/ask.rs`, `run_loop/declared.rs`, the program `driver/src/bin/sdc-answers.rs`; given through the host, so the same on the child and in the library), the copy of the bindings, the private directory of the login link, the run directory writer; `docs/loom-run.md`, `docs/loom-run-settings.md`, `docs/loom-run-directory.md`, `docs/loom-run-decisions.md`; tests in `runbook/tests/loom_run/` against a fake `b10x-loom`; with no `--loom` it runs through Loom as a library, with the gate tests in `runbook/tests/loom_run/inproc.rs`; the slow lanes are `task test:loom-real` against the real binary and `task test:loom-in-process` comparing the two ways). `ui` and the other commands are not built (`story:runbook-cli-skeleton`).
- `ui/` Node live view of a run (`ui/README.md`).
- `views/` the view file each runbook ships for the inspector: phases, file kinds, tool owners (`views/sdc.view.json`; `ui/README.md`, "The view file").
- `decision/` decision-contract examples (`docs/decisions.md`).
- `ess/` ESS specification of the decision contract (domain `sdc.decision`).
- `eval/` eval runner, adversaries, levels, reports (`docs/eval-design.md`).
- `runs/` recorded runs: case stores, event logs, LLM and decider records.
- `scripts/` check, evaluate, trial and full-cycle scripts.
- `tools/` decision-contract shell tools (`decision-grounds.sh`, `decision-from-record.sh`).
- `docs/` design notes and research results; start with `phases.md`, `driver.md`, `orchestration.md`.
- `plugin/` the `runbook` plugin (skills `workflow-builder`, `harden`, `record`; marketplace `.claude-plugin/marketplace.json`; `plugin/README.md`). A command or path in a `SKILL.md` must be checked against the script or binary it names before it is written; `driver/tests/plugin.rs` fails on a missing path or a version that differs from the release version.
- `.engineering/planning/` the AEP planning store.

## Rules

- Anything new that runs is Rust (the organization's language rule). Never Python. The existing bash and
  jq tools (`bindings/tools/`, `scripts/`, `tools/`) are prototype glue: they get fixes,
  no new bash tool is added, and each is retired by a Rust binary or by Loom.
- Every build and test runs through `slot`. The `cargo` shim does it; prefix anything else
  (`slot npx playwright test --workers=2`). Never set `-j` or `CARGO_BUILD_JOBS`.
- The driver tests run as `slot task test:driver` (cargo-nextest, `driver/.config/nextest.toml`): a test over 60 s is killed and fails, the ten slowest are printed, the run fails over 120 s. `driver/tests/main.rs` is the one integration-test binary: a new test file needs a `mod` line there (`autotests = false`); each test is its own process, so a product made once per run is a cache on disk under `driver/target/` (see `schemas()` in `driver/tests/decision_request.rs`).
- The `runbook` crate tests run as `slot task test:runbook` (cargo-nextest, `runbook/.config/nextest.toml`; the forge runs the same task in `.github/workflows/pages.yml` for a code change): a test over 60 s is killed and fails, the ten slowest are printed, the run fails over 120 s. `runbook/tests/cli.rs` is the one integration-test binary (the daemon's tests are modules under `runbook/tests/daemon/`). The daemon reads one clock (`runbook/src/clock.rs`): a test that needs an exact instant builds the daemon in process with `Daemon::start_with_clock` and a `SetClock`; no option, environment variable or request sets a clock. Both test tasks pass `--color never` to every nextest call whose log they parse (the forge sets `CARGO_TERM_COLOR=always` through `dtolnay/rust-toolchain`); `task check:ci-paths` fails when a nextest call of `Taskfile.yml` lacks it.
- The engine (`runbook/src/engine/`, `docs/engine.md`) is the Loom runtime of the daemon: library calls into the pinned Loom (`runbook/Cargo.toml`, `rev`), the per-run store under the daemon's state directory, no Loom or Canon process. The gate-lane tests are `runbook/tests/daemon/engine.rs` over `runbook/tests/fixtures/engine-runbook/`; the slow lane is `task test:engine-real`. Cargo reaches Loom through its forge URL; a local build that needs a clone uses `GIT_CONFIG_*` `url.<clone>.insteadOf` in the shell only, never a `[patch]` or a path in a committed file.
- MCP (`runbook mcp`, `runbook serve --mcp`; `runbook/src/mcp/`, `docs/mcp.md`) is a face of the command layer: a tool calls `api::routes::command_answer` or `view_answer` and returns the route's body, and its input schema is derived from the OpenAPI document, never typed by hand; a new tool is a row in `mcp/tools.rs`.
- The UI test step is `task test:ui` at the root (`task test` in `ui/`; part of `task check`, beside `test:driver`; Playwright, four workers, 60 s a test, 120 s the run, the ten
  slowest printed). It needs no `runs/`, no built binary and no server of your own; its servers take free
  ports. Specs that need those are the slow lane (`task slow`, listed in `ui/test/lanes.mjs`).
- Anything that runs longer than about 30 s runs in the background.
- Layers: Canon is the kernel (protocol semantics, evaluation), Loom the runtime, Substrate
  confinement and process start (`docs/layers.md:47-52`). A capability missing in one of them is
  requested from that repository's session, not patched here. Record the request in
  `docs/canon-requests.md` or the planning store (stories marked blocked upstream).
- Planning lives in `.engineering/planning/` and changes only through the `aep` CLI
  (`aep plan artifact new|move|relate|body|validate|waves`). Never hand-edit an artifact file.
  Body scratch files go in `.engineering/drafts/` (git-ignored).
- Protocol changes are checked with `scripts/check-protocols.sh` and `scripts/check-bindings.sh`.
  A protocol change runs the simulate step; a changed finding set is re-pinned (`--repin`) in the
  same commit with the reason in the commit body (`docs/protocol-notes.md`, "Simulate").
  A new `revision` of a protocol file moves the registration (`<id with . and _ as ->@<revision>`)
  in `bindings/<name>.yaml` and in every `copy` `from:` in the same commit;
  `scripts/check-bindings.sh` refuses a mismatch.
  `scripts/check-protocols.sh` is a digest check against the committed `protocol/check-ledger.json`
  (the 14 `canon check` calls cost about 590 s and are not repeated for an unchanged protocol): after
  a change to a protocol, `canon-pin.json` or the script, run `scripts/check-protocols.sh --record`
  and commit the ledger; `task check:protocols:slow` regenerates it in full and compares bytes.
  The fast lane (under 1 s) is part of `task check`; each ledger entry carries a seal over its digest
  and result fields, so a hand edit of a result fails it. CI runs the slow lane
  (`.github/workflows/protocols.yml`) when `protocol/**`, `driver/canon-pin.json` or the script changed.
- After a merge, `scripts/check-bindings.sh` runs before the push (build the driver first): a merge
  can leave `website/data/protocols/index.json` stale without a conflict.
- The full cycle run starts as a systemd unit, which has `LANG=de_DE.UTF-8` on this host. A tool
  that reads a number printed by bash or a program does not depend on the decimal separator
  (`EPOCHREALTIME` holds a comma there). A cycle that passes in a terminal and fails in the unit is
  checked for the locale first.
- An LLM call whose output is a data document is made with a response schema and its reply is
  validated by a program against the same schema; an invalid reply is retried with the validator's
  error at most twice and then fails with a named cause. Free-form outputs (code, prose) are the
  only exception and are listed in `docs/driver.md` (the operator, 2026-11-21).
- Cite `path:line` for facts in docs; mark what was not observed as *inferred*.
- No `/home/<name>/` path in any committed file: write repository-relative paths, sibling-relative
  paths (`../canon/…`) in docs, and `$HOME`-based or argument paths in scripts.
- A run that cannot continue (a defect that needs a code change, a missing capability, or a
  decision nobody will take) is stopped at once by whoever finds it, with
  `SDC_PRINCIPAL=<principal> scripts/run-trial.sh <run> --stop "<reason and the fix it waits on>"`.
  No run is left waiting when no person or agent is working on it. Demo and fixture runs are never
  listed beside real runs without a label (the operator, 2026-11-21).
- Trial runs use `--autopilot --keep-going` to collect every defect of a run (`defects.jsonl`)
  before the fixes are made in one batch. A forced run (`forced N` in `status.json` and the
  closed line) is evidence for defects and never counts as an end-to-end pass; the pass is a
  later run without the flag (the operator, 2026-11-21).
- A real run belongs to an experiment record `experiments/<id>.yaml` (`docs/workflow-metrics.md`
  § 7), whose report and `experiments/INDEX.md` account for it. The operator, 2026-11-21: "make
  sure to organize these trials in a somewhat scientific and organized, manageable and scaling
  manner. It shouldn't end up messy."
- No run is started by hand: `sdc-experiment run <id>` starts them, on the record's commit, with the
  record's settings (the operator, 2026-11-21, same words). Stop one with `sdc-experiment stop`.
- CI classifies the changed files in one place, the step "Classify the changed files" of `.github/workflows/pages.yml` (planning, docs-site or code; a planning-only change builds and tests nothing); a change to that path list changes `task check:ci-paths` in the same commit.
- The rustc version is pinned in `rust-toolchain.toml`, and the pages workflow's `toolchain:` repeats it; change both in one commit. The step "Check the rustc pin" of `.github/workflows/pages.yml` fails when the workflow's `toolchain:` value differs from the channel in `rust-toolchain.toml` (it reads both as text; inside the repository rustup runs the pinned channel whatever the workflow installs).

## Commits

- Every commit and push is `b10x-bot[bot]`'s (author and committer
  `b10x-bot[bot] <316511680+b10x-bot[bot]@users.noreply.github.com>`), made with
  `b10x-gates bot -- commit -F <message file>` and `b10x-gates bot -- push`. It mints a token from
  the organization App's credentials. Every other GitHub write goes through
  `b10x-gates api` or `b10x-gates gh`.
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
