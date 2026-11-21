# SDC prototype

## What it is

- A family of eight Canon `protocol/1` documents under `protocol/`, one case per stage, composed
  with child cases: idea, intake, direction, epic, spec, plan, story, release (`docs/protocol-notes.md:3-16`).
- The stage is derived from facts (files in the repository, recorded run results), never from a
  stored phase; a passing ESS gate marks the spec complete (`docs/phases.md:3-6`, `:34-35`).
- `sdc-drive` (`driver/`) runs one case tree on Canon: it reads `canon-decision/1`, runs the tool
  bound to one admissible action, records `canon-evidence/1`, evaluates again (`docs/driver.md:3-6`).
- An LLM is only ever one action's executor; its output is an artifact revision, never evidence
  and never the next state (`docs/driver.md:5-6`).
- The SDC is an application on Loom; the driver's runtime parts are a temporary copy of what Loom
  must provide (`docs/layers.md:51`; `docs/driver.md:43-45`).

## Directory map

- `protocol/` the eight stage protocols and `sdc-bugfix` (one bug fix, red first: `docs/bugfix.md`), `compiled/` IR, `probes/`, `draft/` (`orchestrate.protocol.yaml`).
- `cases/` fixture cases evaluated by `scripts/evaluate-cases.sh`.
- `driver/` Rust crate `sdc-drive` (plus `sdc-findings`, `sdc-finding-kpis`, `sdc-run-compare`: `docs/run-compare.md`; `sdc-bugfix`: `docs/bugfix.md`); bindings in `driver/examples/sdc/`.
- `watch/` Rust crate `sdc-watch`: filesystem changes of a run (`docs/fs-changes.md`).
- `ui/` Node live view of a run (`ui/README.md`).
- `decision/` decision-contract examples (`docs/decisions.md`).
- `ess/` ESS specification of the decision contract (domain `sdc.decision`).
- `eval/` eval runner, adversaries, levels, reports (`docs/eval-design.md`).
- `runs/` recorded runs: case stores, event logs, LLM and decider records.
- `scripts/` check, evaluate, trial and full-cycle scripts.
- `tools/` decision-contract shell tools (`decision-grounds.sh`, `decision-from-record.sh`).
- `docs/` design notes and research results; start with `phases.md`, `driver.md`, `orchestration.md`.
- `.engineering/planning/` the AEP planning store.

## Rules

- Anything new that runs is Rust (the organization's language rule). Never Python. The existing bash and
  jq tools (`driver/examples/sdc/tools/`, `scripts/`, `tools/`) are prototype glue: they get fixes,
  no new bash tool is added, and each is retired by a Rust binary or by Loom.
- Every build and test runs through `slot`. The `cargo` shim does it; prefix anything else
  (`slot npx playwright test --workers=2`). Never set `-j` or `CARGO_BUILD_JOBS`.
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

## Commits

- Every commit and push is `b10x-bot[bot]`'s (author and committer
  `b10x-bot[bot] <316511680+b10x-bot[bot]@users.noreply.github.com>`), made with
  `b10x-gates bot -- commit -F <message file>` and `b10x-gates bot -- push`. It mints a token from
  the organization App's credentials. Every other GitHub write goes through
  `b10x-gates api` or `b10x-gates gh`.
- No attribution lines. Every commit has a subject and a body.
- Common Gates: hooks with `b10x-gates --repository beyond10x/sdc --policy ../gates-policy/policy.json
  install`; before a push, `b10x-gates --repository beyond10x/sdc check --receipt <file>`. CI runs
  `.github/workflows/shared-gates.yml`, the reusable Gates workflow pinned by commit. Install and
  check need the repository enrolled in `gates-policy` (GitHub id and adoption baseline, the first
  commit).
- Recorded runs (`runs/`, `eval/runs/`, `eval/reports/*-runs/`) stay out of git: they hold absolute
  paths, LLM transcripts and prompts (`.gitignore`).

## Sessions (the operator, 2026-11-21)

- The main session only orchestrates (plans, dispatches, verifies, reports): it writes no code, edits no file, runs no long build or test in the foreground and is never blocked on one.
- Implementation, research, drafting, scoping and reviews run in subagents, model by difficulty: Opus for design, hard implementation and diagnosis; Sonnet for ordinary implementation and research; Haiku for sweeps and mechanical edits. No subagent runs on the main session's own model.
- Epics and stories created in this store from the operator's requirements are accepted on creation (standing rule of 2026-11-21, `approval-record:operator-standing-acceptance-2026-11-21`): create them and move them to `proposed` in the same step. Upstream blockers stay blockers.
- The project's name is postponed (`story:project-rename`).
- Before working in another repository from this session, list the running sessions; when a session for that repository exists or appears, stop the own agent there at once with the stop tool, not with a message (a message waits behind the agent's running command), and hand the work to that session with the list of everything touched.

## Work order (the operator, 2026-11-21)

"So we're probably focusing first on the non-orchestration bits." In order:

1. Non-orchestration work: late-ambiguity loop merge (`next/` into the tree, `docs/trial-2.md`),
   trial 2, decision contract (`docs/decisions.md`), file-change view (`docs/fs-changes.md`,
   `watch/`), eval (`docs/eval-design.md`, `eval/`).
2. Then the orchestration epics (`docs/orchestration.md` § 7; `aep plan artifact list --kind epic`).
   They are drafts; the operator accepted the design on 2026-11-21, to be implemented "when it's time for it".
