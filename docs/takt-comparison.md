# takt compared with SDC

**Verdict.** takt and SDC overlap in the loop and differ in coverage. takt is a story-delivery executor: it takes one story or intent, works it on a real branch, opens and merges a real pull request, and counts nothing as done without validator evidence. It is already an application on Loom's SDK. SDC is a protocol family for the whole path from idea to release, with a 4,090-line prototype driver, file copies for git and merge, and one toy real run. takt is not superseded as a whole. The loop, case store, event log and harness plumbing are Loom's job and takt already uses Loom for them. Its forge, git, validator gates, held-out scenario runs and improvement loop have no counterpart in SDC, Loom or Canon. Above the story (planning as a unit, trees of cases, standing operation) takt's design is superseded by SDC's accepted orchestration design; takt built none of it. takt is stalled: last commit 2026-11-13, nothing runs it. Recommendation: path A, freeze takt and harvest, after one bounded probe of path B (section 6).

Date: 2026-11-21. Read-only; nothing built or run, except `git` reads and `systemctl`/`ps` listings. `T` = `../takt`. The `../takt` checkout is on branch `wave/35` (release 0.9.0), so every takt fact below is read from takt's `origin/main` (commit 25c62df, 2026-11-13, tag 0.36.0) with `git show` / `git grep`, cited as `T:path:line`. `SDC` paths are repository-relative. *Inferred* marks what was not observed. A line number in `T:` is a line of `origin/main`, not of the checkout.

## 1. What each one is

| | takt | SDC |
|---|---|---|
| purpose | "Agentic coding that counts nothing as done until a validator has recorded the evidence" (`T:README.md:3`) | A family of eight Canon `protocol/1` documents, idea to release, run by one driver (`AGENTS.md:5-12`) |
| unit of work | one story from the AEP planning store, or an `--intent` text, or a work item from a `command` source (`T:docs/design/takt.md:44`; README status 0.34.0) | one idea text becomes a case tree: idea, intake, direction, epic, spec, plan, story, release (`docs/protocol-notes.md:3-16`) |
| size | 87,436 `.rs` lines in `crates/*/src` (80,877 library incl. one YAML, 6,573 CLI), 134,784 lines in `crates/takt/tests`, 286,446 `.rs` lines in all (`git grep -c ''`); 1,851 commits, 2026-10-18 to 2026-11-13; tag 0.36.0 | 4,090 `.rs` lines (`find driver watch -name '*.rs' \| xargs wc -l`), 1,622 YAML lines of protocols, 4,740 UI lines (`docs/conductor-comparison.md:24-25`); no commit (`git log` fails: no commits yet) |
| foundations | Loom SDK 0.52.0, Canon 0.8.0, engineering-protocols 0.15.2, Substrate host 0.14.0 (`T:Cargo.toml:12-17`) | Canon 0.9.0 binary as a subprocess, `ess`, `aep`, metaharness for LLM nodes (`docs/driver.md:107-108`, `:13-16`) |
| planning store | 493 stories: 278 implemented, 196 draft, 13 archived, 5 rejected, 1 active; 13 epics (`T:.engineering/planning/`, `status:` frontmatter counted) | 52 stories, 8 epics, 8 upstream blockers (`.engineering/planning/`, `ls`) |

The README badge "52 of 160 stories implemented" is stale: its generator (`T:docs/metrics/badges.py:57-60`) counts implemented over non-archived stories, which gives 278 of 480 on `origin/main`. The badge block was last rewritten 2026-10-18 (`git log -- docs/metrics/badges.py`). *Inferred*: nobody reran it.

## 2. Axis by axis

| axis | takt | SDC |
|---|---|---|
| next step decided by | Canon through Loom's governor (`T:docs/design/takt.md:59`; `T:crates/takt/tests/no_own_evaluation.rs`). In a judgement step the harness proposes one admissible action; in a deterministic step a validator's action is due (`:48-49`, `:62`). takt still keeps its own pre-checks over an evaluation until `story:takt-harness-trait-retired` (draft) | Canon `evaluate` per step, then the fixed policy `useful()`; the LLM never picks the action, it writes one artifact (`docs/driver.md:43-45`, `:75ff`) |
| protocols run | `story.delivery/3` (own, `T:protocols/story-delivery/3.yaml`), `story.delivery/1,2`, `self.improvement/1`, `loop.improvement/1`, `bench.task/1`, any other engineering protocol by `--protocol` (`T:AGENTS.md` layout row). engineering-protocols holds 10 protocols (`docs/engineering-protocols-boundary.md:24`) | its own eight: `sdc`, `sdc.intake`, `.direction`, `.epic`, `.spec`, `.plan`, `.story`, `.release` (`protocol/`). They are not in the engineering-protocols catalog yet (`docs/boundary.md:36`) |
| phases | the story/change loop only: tests, implementation, gate, PR open, CI read, merge, handover (`T:protocols/story-delivery/3.yaml` claims `tests.pass`, `gate.green`, `pr.opened`, `ci.green`, `pr.merged`). Intake, direction, ESS-gated specification, planning as a unit: not built; `plan.refill/1` and `wave.delivery/1` are design (`T:.engineering/planning/epic/continuous-orchestration.md`, draft, 0 stories done). Release: the forge seam lists "all but release" (`T:docs/design/takt.md:88`) | all of them as protocols; intake through release ran once, with stubs (full cycle) and once with a real Haiku (`docs/driver.md:59-63`). Spec complete = ESS gate passes (`docs/phases.md:34-35`). Operate: designed only (`docs/orchestration.md:271ff`) |
| verification | validators turn observations into `canon-evidence/1` bound to a revision: `TestsRun` (`T:crates/takt/src/validator.rs:1901`), `GateRun` over the engineering assertions (`:2482`, `:5264` `prepare_with`), `PrOpen` (`:5783`), `CiRead` (`:6434`), `PrMerged` (`:6655`). A step guard keeps the harness off CI files, the gate document and ESS inputs (`:3999-4040`, README 0.33.0). Held-out check outside the case: `takt scenario run` writes `held-out.txt` (`T:crates/takt-cli/src/main.rs:4691`, `:61-66`). ESS conformance of an implementation: not found in takt | ESS gate for the spec; for the story an interim `impl-gate` (source exists, tests name the commands, `cargo test` passes) that does not check behaviour (`docs/driver.md:451-481`); no conformance against `impl/` (gap 16), reviews are `test -f` stubs (gap 17); eval harness with scripted adversaries (`docs/eval-design.md`, `eval/reports/`, 4 report sets of 2026-11-20) |
| git and forge | real: case branch, commits with `Refs:`, PR, CI wait, merge on a grant, five forge adapters `none`, `local` (bare repository), `gitlab` (glab), `command`, `connectors` (GitHub through a Connectors instance); lost answers are read back, never sent twice (`T:crates/takt/src/forge.rs:1-26`, `:526-586`; README 0.29.0, 0.31.0) | none: merge is `cp -r impl/. mainline/`, publish is a copy (`bindings/tools/sdc-tool.sh:229,235`, via `docs/conductor-comparison.md:77`) |
| harness adapters | `--harness claude\|codex\|harness` through Loom's session executor over metaharness (`T:crates/takt/src/harness.rs:263-266`) | metaharness adapter, `claude -p`, dry, or any program with the contract (`docs/driver.md:48-51`) |
| decisions, autonomy | a merge needs a grant for the exact revision (`takt approve`); a delegation file may give `repository.merge` to an agent, then a run merges with no person (`T:README.md` status 0.23.0). Named stops with exit codes | Canon authority with principal; deciders `person`, `approve`, `reject`, `haiku`; levels A0 to A5 designed, routing for A1-A4 not built (`docs/autonomy-levels.md:34-42`, `docs/conductor-comparison.md:70`). Grants are not bound to a revision (gap 1) |
| child cases, parallelism | none. One per-case lock; parallel workers and `case.dispatch` are design (`T:.engineering/planning/design/continuous-orchestration.md`) | child cases built as a temporary driver copy (`docs/driver.md:251-286`); depth first, one lock, one shared `work/`; concurrency designed, epic `concurrent-child-cases` draft (`docs/orchestration.md:25-48`) |
| standing operation | `takt run --every` stays resident, one pass per slot inside time windows, until an `--until` condition (`T:crates/takt/src/schedule.rs:1-12`, 0.35.0) | designed (`docs/orchestration.md` § 3), draft protocol only |
| self-improvement | `self.improvement/1` and `loop.improvement/1`: a finding is reproduced on the promoted binary, fixed, verified, scored by `takt scorecard --against`, promoted by `takt promote` into a hash-chained store (`T:crates/takt/src/improvement.rs:1-25`, `promotion.rs:1-20`, 3,421 lines). SDC has none | none; `sdc-findings` is a late-finding loop inside one run, not a change to SDC (`docs/trial-2.md`) |
| UI | `takt view` writes one offline HTML page over `takt-trace/1` files (`T:crates/takt/src/view.rs:1-6`); epic `terminal-frontend` 0 of 5 | `ui/` run inspector, live claims, evidence, LLM calls and cost (`ui/README.md`) |
| metrics | `docs/metrics/*.jsonl`: `scenario-runs.jsonl` 68 rows, `improvement-turns.jsonl` 3, `waves.jsonl` 6, `events.jsonl` 2, `journeys.jsonl` 1,042 (`wc -l`); raw usage only, "null, never 0" (`T:docs/metrics/README.md`) | `docs/kpis.md` catalogue, `eval/baseline.json`; `docs/kpis.md:21-45` already names takt's scorecard as the baseline instrument |

## 3. What has run for real

- takt, scenario runs: 47 `takt` rows plus 21 free-arm rows (Claude Code, Codex), all dated 2026-10-18 (`T:docs/metrics/scenario-runs.jsonl`, `jq`). 33 takt rows state a cost: sum $10.60, median $0.253, max $0.849. Held-out check: 54 of 56 recorded rows pass (36 of 38 takt, 18 of 18 free). 33 takt rows name a PR URL; how many merged: not found per row.
- takt, later: five runs of `story.delivery/3` on takt's own story with `--forge none` on 2026-10-29; a trial on 0.23.0 on 2026-10-30 that stopped on the token budget (`T:docs/runs/`, 85 files). Three improvement turns accepted and promoted on 2026-10-18 and 19 (README badge, `improvement-turns.jsonl`), the later ones driven by validators in scratch trials (`T:docs/metrics/events.jsonl`). The design file states a delivery on a real repository with PR, CI and merge "is not proven" (`T:docs/design/takt.md:128`).
- takt, M2 proof: one scenario PR merged on 2026-10-18, with 4 invocations where 2 were required (`T:docs/design/takt.md:116`). The earlier comparison's "one merged PR" holds for scenario runs. takt's own repository also has 27 merge commits (`git log --merges`), made by the AEP wave process; whether `takt run` made any: not found.
- takt, M5 (a week of daily runs on three repositories): not started; `story:week-of-daily-runs` is draft.
- SDC: one real LLM run of the whole cycle on a toy to-do idea, 15 work calls and 5 decider calls, $0.1004 (`runs/trial-todo-1/llm-calls.jsonl`, `decisions.jsonl`); one trial 2 run, 21 work and 4 decider calls, $0.1977 (`runs/trial-2-train/`); stub full cycles with replay 8 of 8.
- Cost per delivered item: takt not found (its rows are single-story scenarios, not a delivered backlog); SDC not comparable (no merge, one stub review).

## 4. Does anything run takt today

Not found. No takt user unit (`systemctl --user list-units --all | grep -i takt` empty), no takt process (`ps`). The installed binary is 0.17.0 (`takt --version`), 19 releases behind the tag. Its promotion store was last written 2026-10-22 and its local-forge directory 2026-11-06 (`$XDG_STATE_HOME/takt`). Every one of the 1,851 commits has author `b10x-bot[bot]` (`git log --format=%an`), so the author field does not tell an agent session from a `takt run`. Last commit 2026-11-13, eight days before this note.

## 5. The four answers

**1. Overlap (same mechanism built twice).**

| mechanism | takt | SDC | further along |
|---|---|---|---|
| evaluate, admit, perform, evidence, evaluate loop | on Loom's runtime and governor | own driver, declared a temporary copy (`docs/boundary.md:30-33`) | takt: it is on Loom. Both are to end on Loom |
| case as files, event log, replay | `.takt/cases`, `events.jsonl`, `takt replay` (`T:crates/takt/src/replay.rs`) | `case.json`, hash-chained events, the driver's `replay` | equal; takt has resume, SDC has no resume of a cut step (gap 15) |
| harness through metaharness | 3 harnesses | metaharness default | takt (3 adapters, pinned versions checked) |
| decision and grant records | grant per revision | grant not bound to a revision | takt; the fix is a Canon request (`docs/boundary.md:147-153`) |
| scorecard, KPIs, baseline | `takt-scorecard/1`, rule `{trials, margin}` | `docs/kpis.md` reuses the takt rule | takt (built and promoted against); SDC has catalogue and baseline |
| run view | `takt view` | `ui/` | SDC for live view; takt for offline page |
| bounded LLM decider for routine decisions | not found | the shell decider (Haiku) | SDC |
| child cases | none | temporary copy | SDC (Loom has none either, `../loom` origin/main grep of `case_outcome`: empty) |

**2. What takt has that SDC needs.** Gaps are rows of `docs/conductor-comparison.md` § 7. Cost is lines at stake and the move; an effort estimate: not found.

| candidate | files (`T:`) | closes | move |
|---|---|---|---|
| PR open, CI read, merge validators and forge adapters | `crates/takt/src/forge.rs` (2,306), `forge/{local,gitlab,connectors,command,relay}.rs`, `validator.rs:5783,6434,6655` | gap 4 forge evidence | port into Loom as effects: the validators are already Commission effects over `canon-evidence/1`, so the evidence shape matches SDC; `forge.rs` leans on takt's `confinement::Command` and `scan` (*inferred* coupling) |
| git on a case branch, worktree, base pin | `crates/takt/src/git.rs` (4,484), `workspace.rs`, `commit_style.rs` | gap 3 real git | port; tied to takt's case layout (*inferred*) |
| gate validator | `validator.rs:2482,4983,5264`, `scripts/gate-exec` | gap 2 verification (for the story: tests and policy gate; not ESS conformance) | port, and blocked by the data-only catalog decision: `prepare_with` is engineering-protocols' assertions crate, whose observation collection is to move into Loom executors (`docs/engineering-protocols-boundary.md:1-10`) |
| held-out scenario runner and scorecard | `crates/takt-cli/src/main.rs:4691`, `crates/takt/src/scorecard.rs` (2,243), `loop_scorecard.rs` (1,790) | gaps 2 and 12 (operating record) | reuse as a library or call the binary on SDC cases; the format `takt-scorecard/1` is reused already |
| harness adapters | `crates/takt/src/harness.rs` (8,493) | none open | already Loom's session executor and metaharness; nothing to harvest except the version-pin checks |
| improvement-turn loop | `improvement.rs` (2,411), `promotion.rs` (3,421), `protocols/loop-improvement/metrics.yaml` | none of the five; it is new capability | rewrite on SDC's protocols later; reuse the promoted-baseline idea. Not needed before gap 12 |
| resume | `replay.rs`, `case.rs` resume | gap 6 resume of a cut step | port the idea (*inferred*: not read in detail) |
| scheduling | `schedule.rs` (1,089) | gap 10 standing operation (not in the five asked) | reuse as a library |

**3. Superseded or not, by part.**

| part | superseded by | note |
|---|---|---|
| loop, case store, governor, session executor | Loom (takt already runs on it) | takt keeps its own pre-checks and the `Harness` trait until two draft stories land |
| `workflow/1` loader, declared loops (M3) | nothing built; SDC's protocol bindings play that role | draft stories only |
| continuous orchestration, plan refill, wave as a case tree | SDC's accepted orchestration design (`docs/orchestration.md` § 7) | takt built none of it |
| forge, git, PR validators, step guard, held-out runner, scorecard, promotion, improvement turns, schedule | nothing | only takt has them |
| confinement of the test command | Substrate | takt already uses Substrate for the test command only (`T:docs/design/takt.md:88-89`); `docs/layers.md:172` lists a takt `seatbelt.rs`, which is not in `origin/main` (stale, from the 0.9.0 checkout) |
| protocol catalog dependency | engineering-protocols as a data-only catalog | takt pins tag 0.15.2 and calls the registry (`T:crates/takt/src/protocol.rs:60`) |

If takt were frozen today we would lose the only working forge and PR evidence, the step guard (judge surface), the scorecard and promotion chain with its recorded baseline, 47 recorded takt scenario runs as a comparison base, and `run --every`. We would not lose the loop, the case store or harness access.

**4. Three paths.**

- **A. Freeze takt, harvest into Loom and SDC.** Cost: the five rows above, in the order forge validators, git, gate, scorecard use. Risk: harvest from a frozen 87k-line code base with 8 days without a commit; coupling to takt's case types makes ports larger than the file lengths suggest (*inferred*); the gate port waits on the catalog change. Gain: one runtime (Loom), no fifth copy.
- **B. takt as the story-stage executor; SDC keeps intake to plan and release; takt runs `sdc.story`.** Cost: bind `sdc.story` actions to commands through takt's `command_actions` (0.35.0), map `takt.run-outcome/1` to a `case_outcome` record, keep two case stores. Risk: takt pins Canon 0.8.0, SDC compiles with Canon 0.9.0, so whether takt can evaluate SDC's protocols is not found; takt validators are keyed to `story.delivery` action names; SDC's driver stays a copy until Loom has child cases; takt gets no development (stalled). It adds a second executor to a layer that is to end up as one (Loom).
- **C. Both continue.** Cost: two loops, two case stores, two scorecards, and takt's Loom pin (0.52.0) drifts from Loom's main (tag 0.57.2, 2026-11-05). Risk: the duplication of section 5 row 1 grows; SDC never gets real git or forge.

**Recommendation: A.** It matches the standing decision (SDC succeeds Conductor, Loom is the runtime) and the earlier comparison's order. Do first one cheap probe of B: run `sdc.story` once through takt's `command_actions` on a scratch repository. If takt evaluates SDC's protocol at Canon 0.8.0 and the PR evidence matches, harvest by running takt as the story executor until Loom has child cases and a forge effect (B as a bridge). Evidence that would change the recommendation:

- Loom's plan has no forge effect or child-case runtime within the time SDC needs a real PR: then B for longer.
- takt's M5 week of daily runs gets run and shows real delivery before SDC has gap 12: then B or C.
- The probe shows takt cannot evaluate SDC protocols without a Canon bump: A is confirmed.

## 6. Not found or least sure

- Whether takt (Canon 0.8.0) evaluates SDC's Canon 0.9.0 protocols: not tested.
- How coupled `forge.rs`, `git.rs` and the validators are to takt's case types: judged from file headers and imports, not from a trial move.
- Whether `takt run` made any of takt's own 27 merged PRs or its post-2026-10-30 commits: not found; commit authorship is one bot for all 1,851.
- Merged count of the 33 takt rows with a PR URL; takt's cost per delivered story after 2026-10-18: not found.
- Line numbers cited as `T:` are from `origin/main` at 25c62df; the `validator.rs:1987-2039` span in the earlier note belongs to the 0.9.0 checkout.
