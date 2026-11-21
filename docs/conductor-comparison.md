# SDC and Conductor: comparison, the Canon-based executor, and a successor path

**Verdict.** Different jobs. Conductor is a fleet manager: its Rust loop starts one Claude Code
controller session per repository, and that session holds control flow inside the repository. SDC
is a single-case executor: a program holds control flow, an LLM fills one action at a time. SDC can
replace the controller session with a smaller mechanism, not yet the fleet layer: no parallelism,
no git or forge, no confinement, no run on a real repository. Conductor is over-engineered in one
way: most of its machinery keeps long-running LLM sessions alive; its deterministic loop is not.
Recommendation: path A (SDC succeeds, Conductor stays frozen), gated on the 12 items of § 7.
Conductor has been stopped since 2026-11-08, so A adds no new freeze cost.

Date: 2026-11-21. Read only; nothing was built or run. Path prefixes used below:
`C` = `../conductor`, `S` = `.` (this repository), `R` = unpublished host records
(Conductor configuration, records and state; in no repository), `T` = `../takt`,
`LP` = `../loom/.engineering/planning` on Loom's SDC planning branch (Loom's plan
worktree for SDC). *Inferred* marks a statement that was not observed. Two helper agents (sonnet)
swept the Conductor planning store and the operational records. Their numbers are cited with the
command or file they came from, and I re-checked the key ones (stop date, retro lines, cost lines).

## 1. Size and shape in numbers

| measure | Conductor | SDC |
|---|---|---|
| Rust, `src` only | 203,992 lines in 5 crates: `conductor` 117,055, `conductor-model` 81,251 (generated from `spec/`, `C/AGENTS.md:48`), `conductor-docs` 2,952, `conductor-broker` 2,311, `tool` 423 (`find crates/<c> -name '*.rs' -path '*src*' \| xargs cat \| wc -l`). All `.rs` including tests: 350,658 | 3,772: `driver/src` 1,839 (`main.rs` 943, `sdc-findings` 715, `sdc-finding-kpis` 181), `watch` 1,933 (`wc -l`) |
| other code | dashboard 12,026 lines (ts/tsx/vue/css under `crates/conductor/dashboard/src`) | protocols 8 files, 1,618 YAML lines; shell 2,123 (`tools`+`driver/examples` 1,089, `scripts`+`eval` 1,034); UI 4,740 (`ui/`, without `node_modules`) |
| specification | ESS: 16 domains, 97 scenarios, 23,030 YAML lines (`ls C/spec/domains C/spec/scenarios`) | Canon `check`: 8 protocols, 4,608 to 65,536 states each, 0 findings (`S/docs/check-output.txt:2-16`); 65,536 is Canon's bound (`S/docs/orchestration.md:212-213`) |
| history | 1,139 commits on `origin/main`, 2026-10-08 to 2026-11-14; 60 tags, newest `v1.9.0` (`git rev-list --count origin/main`, `git tag`). The primary checkout is at 1.6.0 | **no commits**: `git log` in `S` prints "does not have any commits yet"; the whole tree is untracked |
| planning store | 722 artifacts: story 528 (draft 208, active 21, implemented 247, archived 52), epic 35, review-result 126 (helper 1, `awk` over frontmatter per kind directory) | `.engineering/` holds only `project.yaml`; the draft epics `S/docs/orchestration.md:423` names are not in it |
| waves | 44 main-series pages (`wave-01` to `wave-51`, 7 numbers missing) plus 7 `v1-wave-*`, 2026-10-08 to 2026-11-11 (`ls C/docs/waves`) | none; one story per trial run |
| reviews | 45 adversary records over 30 units (15 with 1 pass, 15 with 2), 66 plan-critic, 9 security, 490 finding entries (helper 1, file names and `grep -c '^- file:'`) | `spec.review` and `change.review` are `test -f spec/system.yaml` and `test -s impl/README.md` (`S/bindings/tools/sdc-tool.sh:237-238`) |
| prose for agents | role profiles: controller 120, maintainer 60, supervisor 460, teacher 177 lines (`wc -l C/crates/conductor/roles/*.md`); instance `rules.md` 98,546 bytes, 151 items (`C/docs/analysis/takeover-baseline-2026-10-17.md:26`) | `S/AGENTS.md` 57 lines; prompts inside `tools/llm-node.sh` |
| real runs | the beyond10x instance ran 2026-10-06 to 2026-11-08 (unpublished host records) | 1 real LLM run, `S/runs/trial-todo-1`, 2026-11-20T23:16:05Z to 2026-11-21T00:02:42Z; 4 stub or dry runs (`ls S/runs`) |

## 2. What runs each system

**Conductor** (verified unless marked):

- `conductord` (`conductor daemon serve`), a user unit that starts each instance's runtime and
  serves the dashboard on port 7300 (`C/README.md`, table "What it is"; `C/AGENTS.md:254`).
- The loop `conductor run`, one per instance, "the instance's loop as a program, with no model call"
  (`C/crates/conductor/src/run.rs:1-2`). Every `cadence.watch` it runs the watch, every
  `cadence.cycle` a snapshot and dispatch, and once a day the report and records commit (`C/README.md`).
- One controller per active repository: `claude --bg … --permission-mode bypassPermissions`
  (`C/crates/conductor/src/spawn.rs:20-21`), capped by `controllers.max_working`
  (`C/docs/design/conductor-run.md:115-122`). The controller plans in AEP, delivers in waves in
  managed trees and releases (`conductor-run.md:135-145`).
- conductor-dev (the maintainer) develops Conductor itself and restarts the loop (`C/AGENTS.md:27-30`).
- Host roles conductor-supervisor (model opus) and conductor-teacher (model fable)
  (host agent definitions). Each wakes on an event, or at the latest
  20 minutes after its last pass (unpublished host records).
- Outside Conductor: two host services, for account rotation and disk space (unpublished host records).
- Now: 0 conductor units (`systemctl --user list-units --all | grep -ci conduct` → 0). The beyond10x
  instance "stopped on the operator's order" on 2026-11-08 16:42 CET
  (unpublished host records).

**SDC**: one process, `sdc-drive run`. Each step evaluates with Canon, selects with the fixed policy
`useful()`, runs the bound argv, records evidence and evaluates again (`S/docs/driver.md`, "The
loop"). The trial script calls it in a loop of rounds (`S/scripts/run-trial.sh:238-240`). A person
or the Haiku decider (`tools/decider.sh`) takes decisions (`S/docs/trial-1.md:30-50`). There are no
long-lived sessions.

## 3. Comparison by axis

| axis | Conductor | SDC |
|---|---|---|
| unit of work, entry | a repository with a charter and an open dispatch serving a goal (`conductor-run.md:115-122`); entry is a goal, a charter, or a message to the mailbox (`C/AGENTS.md:274`) | one idea text → a case tree (idea, intake, direction, epic, spec, plan, story, release) (`S/AGENTS.md:5-6`); entry is `run-trial.sh <name> "<idea>"` (`trial-1.md:10`). Ticket, store id and "continue" are designed, not built (`orchestration.md:282-290`) |
| who decides the next step | the program decides **which repository** gets a session; inside the repository an LLM session decides every step (`spawn.rs:20-21`; `controller.md`). Requests are classified in code; class C gets one bounded `claude -p --tools=` call (`conductor-run.md:124-133`; `decide/model.rs:6, 103-108`) | the program decides every step: Canon admissibility plus the fixed policy (`driver.md`, "Fixed policy"). "The driver itself never asks an LLM which action comes next" (`driver.md`, "Tools for a chooser"). The LLM writes one artifact per call |
| state model, replay | an event log in SQLite or Postgres (`store.rs:1-25`, helper 2); records as git-committed JSONL and markdown (`C/AGENTS.md:66`); a state tree in unpublished host records. Replay: not found | plain files per case: `case.json`, content-addressed evidence and blobs, hash-chained events; the stage is derived, never stored (`S/docs/phases.md:3-6`). `sdc-drive replay`: 8 of 8 cases match their state and decision digests (`S/runs/trial-todo-1/replay.json`) |
| verification | ESS spec with conformance in `task check`, CI gate on each PR (`C/AGENTS.md:76, 87`); per-story adversary 1 to 2 passes, plan critics, security rounds (§ 1) | Canon gates: no claim without program evidence; ESS gate for the spec (`phases.md:34-35`). Holes: conformance does not run against `impl/` (`driver.md` gap 16); reviews are stubs (gap 17); a scripted implementer that writes only `README.md` reaches `passed` (`S/docs/eval-design.md:30-35`) |
| human decisions, autonomy | classes C/O/H; O and H go to the operator with options, a default and a deadline (`conductor-run.md:124-133`). Supervisor acts "with the operator's authority" on a listed set (`C/crates/conductor/roles/supervisor.md:236`). K1 operator touches per merged PR: 0.245 in week 1 (`C/docs/design/self-improvement.md:39`), 0.53 on 2026-10-14..17 (`takeover-baseline-2026-10-17.md:17`) | Canon authority records with principal; `manual` actions and handoffs stop with the exact command (`driver.md`); deciders `person`, `approve`, `reject`, `haiku` (`trial-1.md:30-55`). Levels A0 to A5 designed; A1 to A4 routing not built (`S/docs/autonomy-levels.md:427`). trial-todo-1: 2 person approvals (`owner.approve` 23:32:11Z, `release.approve` 00:01:16Z) and 5 decider calls (`events/*.jsonl` `authority_recorded`; `decisions.jsonl`) |
| parallelism, WIP | up to `max_working` controllers per instance (set per instance in unpublished host records); waves of up to N implementors per story (AEP wave skill). Build queue median wait 15 min on 2026-10-14 (`C/docs/analysis/efficiency-baseline.md:17`) | none executed: children run "one at a time, depth first, under one lock and one shared `work/`" (`orchestration.md:31-35`, `driver/src/main.rs:794`). WIP limits decided, not built (`orchestration.md:175-191`) |
| confinement, security | "The guard is the only check"; sessions run `bypassPermissions`; `git push` and forge `curl` are not checked; "Conductor confines no session itself", Substrate only with a `placement` (`C/README.md:37-51`). 236 guard denials in week 1 (`self-improvement.md:48`) | none at process level: `Command::new(argv[0])`, environment inherited (`main.rs:567`; `S/docs/layers.md:155`). Per LLM node, an operation allow-list (`file.read/write/edit`, `dir.list`, `search`), max 15 turns, $0.50 cap (`trial-todo-1/llm-calls.jsonl` line 1); no shell (`driver.md` gap 18) |
| cost, tokens | list-price equivalent, 2026-10-07..13: $16.97 median / $39.59 p90 per task; $21.22 per merged PR fully loaded; $7,977 for the window (`self-improvement.md:76`); per dispatch $11 median, $54 p90 (`:79`); "not what the organisation is charged" (`:173`). 3,355M input tokens for 118 merged PRs on 2026-10-14..17, 29 % of them conductor-dev's (`takeover-baseline:11-14`) | trial-todo-1: 15 work calls $0.0770 and 5 decider calls $0.0234, together $0.1004, all `claude-haiku-5-5` (`jq` sums over `llm-calls.jsonl`, `decisions.jsonl`). The output is one local to-do app with a stub review and no real merge, so the two figures **do not compare** |
| observability, UI | dashboard served by conductord, with a UX gate in `task check` (`C/AGENTS.md:84, 260`); KPIs K1 to K17 and T1 to T4 (`self-improvement.md:36-80`); `conductor status` took 43 to 120 s on 2026-11-08 (unpublished host records) | run inspector `S/ui` (live claims, evidence, who acted, LLM calls and cost; `S/ui/README.md:1-7`); file changes per run (`S/docs/fs-changes.md`); KPI catalogue and `eval/run.sh` baseline (`S/docs/kpis.md`) |
| recovery | handlers for context hand-over, usage-limit resume, stalled or gone sessions (`conductor-run.md` § 3); lease, reclaim and reconcile: `reclaim` 603 hits in 27 files, `lease` 1,349 in 53 (helper 2, `grep` over `crates/conductor/src`). 2026-11-08: 246 failed loop resumes, 7 hand restarts raced the loop, a recovery fix took 10 h 34 min from commit to production (unpublished host records) | a stale-pid lock is taken over (`main.rs:108-138`); a new `run` re-evaluates from files. **No resume**: a step cut between the tool's exit and `action_finished` is re-run, and the log is a record, not the source (`driver.md` gaps 13 and 15) |
| self-maintenance | conductor-dev turns defects into stories and delivers them in waves (`C/AGENTS.md:9-12`); the teacher found 5 defects and added 11 supervisor rules in one night (unpublished host records) | none, by design. Changes are made by hand in sessions |
| brownfield | built for existing repositories: collectors over git, `gh`, `glab`, AEP, ESS (`C/README.md`, Requirements); onboarding runs on GitHub and GitLab scratch repositories (`C/AGENTS.md:205-211`) | rated 2 of 5 by its own doc: no roadmap kind, an all-or-nothing ESS gate, no import of existing approvals (`orchestration.md:340-355`). Merge is `cp -r impl/. mainline/`, publish is a copy (`sdc-tool.sh:230, 235`) |
| maturity | about 33 days in operation on up to 3 instances; 376 merged PRs in week 1 (`self-improvement.md:49`), 118 merged PRs and 76 releases in 2026-10-14..17 (`takeover-baseline:14`); stopped 2026-11-08; no commit since 2026-11-14 | 1 real cycle (46.6 min wall time including person waits, one story), protocol checks, fault-injection evals (11 reports in `S/eval/reports`); not under version control |

## 4. Similarity (question 1)

**Duplicated in SDC** (the same idea, built twice):

- Decisions as records with holder, reasons and evidence: Conductor `decisions/*.jsonl` and
  design § 5; SDC `decisions.jsonl` and Canon authority (`S/docs/decisions.md`).
- One bounded LLM call for a routine decision: Conductor class C via `claude -p --tools=`
  (`decide/model.rs:103-108`); SDC `tools/decider.sh` (Haiku, 8 turns, $0.25 cap, `trial-1.md:38-40`).
- KPI catalogues that measure operator touches per item: K1 (`self-improvement.md:39`) and L1
  (`S/docs/kpis.md:255`).
- A live view of a run, AEP stores and waves as the plan, slot for builds, WIP caps
  (`max_working` against `limits.wip.*`, `orchestration.md:180-182`).

**Replaced by a smaller mechanism in SDC:**

| Conductor mechanism | SDC replacement |
|---|---|
| a controller session that plans, codes, merges and hands over at a context threshold (`controller.md`; 59 of 118 sessions over 200k, `takeover-baseline:19`) | a protocol plus a 943-line driver; each LLM call is one-shot and bounded (15 turns, $0.50), so there is no context to hand over |
| dispatch state from parsed reports (started, blocked, done) | the stage derived from files and evidence (`phases.md:3-6`) |
| a PreToolUse guard of heuristic Bash rules | an operation allow-list per node, with no shell |
| restart and resume handlers, leases, races | re-evaluate from files; one lock per tree (resume of a cut step is missing, § 3) |
| a records commit plus a teacher reading transcripts | hash-chained events, content-addressed evidence, `replay` |

**Not in SDC:** multi-repository dispatch and forge collectors (PRs, CI, releases); real git, PR,
merge and release; executed parallelism; Substrate placement (optional in Conductor); host
resources (disk reclaim, account rotation, usage-limit resume); messages to running sessions; an
API and dashboard across instances; standing operation that never ends (`orchestration.md:253-279`,
design only); self-maintenance.

## 5. The Canon-based executor (question 2)

**Not found under that name.** No hits for `blend`, `canon-based`, `canon executor` or
`executor.*canon` in `C/.engineering`, `C/docs`, `C/spec`, `C/crates`, `conductor-archive`, `canon`
or `loom` (`grep -rniE`, helper 1 and my own pass). The only Canon mention in Conductor's store is
unrelated (`C/.engineering/planning/story/build-run-session-bound.md:24`). No Conductor crate depends
on canon, loom or takt (`grep` over `Cargo.toml`), and `takt` occurs 0 times in
`C/crates/conductor/src`.

**What it most likely refers to** (*inferred*): takt, together with Conductor's agent-kind slot.

- Origin: on 2026-10-18 the operator directed "composable agentic work on canon +
  engineering-protocols; Loom, conductor, control-plane not required". conductor-dev then created
  the takt repository and wrote its M1 kernel (unpublished host records). Option
  D3 B, "loop stays Rust + canon dispatch closure", was left open with default A
  (`PLAN.md:115`).
- The slot in Conductor: "a controller could also be a takt protocol … any combination possible"
  (`C/docs/design/architecture-next.md:374-376`). The `takt` kind is `run-to-exit` and "a draft
  story after v1" (`:391`), but no such story exists in `C/.engineering/planning/story` (`ls | grep
  -i takt` → none). `story:agent-kinds` (implemented 2026-11-08) built the trait, and `claude-code`
  is its only kind (`architecture-next.md:388-390`). `epic:deterministic-steps` is a draft
  (`C/.engineering/planning/epic/deterministic-steps.md:5`).
- State of takt itself: 87,436 Rust lines on `origin/main` (`git grep -c ''` over `crates/**/src`),
  1,851 commits from 2026-10-18 to 2026-11-13, tag `0.36.0`. It depends on `b10x-canon` and
  `b10x-loom-sdk` (`crates/takt/Cargo.toml` on `origin/main`). Milestones: M1 done; M2 built, with one
  PR merged on 2026-10-18 using "4 invocations where 2 were required"; M3 not built; M5 ("a daily
  `run --hours 2` for a week") not started (`T/docs/design/takt.md:95-101`). No commit since
  2026-11-13.

**Usefulness in Conductor today: none delivered.** It exists as one row of a table and a trait with
one implementation. It would have helped where Conductor hurt most, the interactive controller
session (*inferred*).

**SDC and this executor:** SDC is a second Canon-protocol executor beside takt. `S/docs/boundary.md:30-33`
calls the SDC driver "the fourth copy beside Loom, takt and `metaharness aep drive`". For Conductor
the bridge is the `run-to-exit` kind slot, not takt specifically: `sdc-drive run` already exits 0, 3,
4 or 5 with a reason and writes events (`driver.md`, "Verdicts and exit codes"). That is the shape
the slot asks for (`architecture-next.md:386`). takt's own design leaves orchestration across
repositories to Conductor (`architecture-next.md:590`). If SDC moves onto Loom (Loom's
`story:bindings-document` and `story:deterministic-default-selector` are active in
`LP/story/`), takt-in-Conductor is moot as a separate effort. takt still has pieces SDC lacks: PR
merge through the forge, three harness adapters and worktrees (`takt.md:98, 100`). Choosing one
executor, and harvesting the other, is a decision this document does not take.

## 6. Is Conductor over-engineered? (question 3)

**Evidence for:**

- 122,741 hand-written `src` lines plus 81,251 generated and about 146,666 lines of tests (§ 1, by
  subtraction). 528 stories (247 implemented) and 60 tags in about 37 days.
- Three LLM roles exist to keep the system running, not to deliver product work: maintainer,
  supervisor and teacher. The supervisor profile is 460 lines and at its cap (unpublished host records). On
  2026-10-14..17, conductor-dev used 971M of 3,355M input tokens, or 29 % (`takeover-baseline:12`).
- Machinery for the system's own sessions and host: context hand-over, usage-limit resume, account
  rotation, leases (1,349 hits), reclaim (603 hits), a disk ledger, guard latency budgets and
  guard friction (236 denials in a week, `self-improvement.md:48`).
- On 2026-11-08: 7 maintainer sessions in 15.5 h, 246 failed resumes, 7 restart races, and
  supervisor and teacher both out of usage at once (unpublished host records).
- Scope beyond the problem: a v1 with a daemon, pods, Postgres, TLS and Identity bearers, container
  images and four release targets (`C/AGENTS.md:179-235`; `architecture-next.md:1-12`).
- Delivery per token was low: 3 PRs merged for 164M input tokens in 3 h, with 8 of 10 controllers
  producing nothing (`efficiency-baseline.md:10-15`).

**Evidence against:**

- It delivered real work across many repositories (376 merged PRs in week 1, § 3), which SDC has
  not done once.
- The loop moved from an LLM session into Rust because of measured failures (`conductor-run.md`;
  `takeover-baseline:22`: 236 of 268 messages were relays). That is the same principle SDC starts from.
- The instrumentation is honest and sourced (KPIs, baselines, retros). Disk and build machinery
  came from real incidents: 200G free fell to 15-19G three times in one day
  (`takeover-baseline:30`).

**Parts that carry their weight:** the deterministic loop and dispatch (`run.rs`); request
classification in code plus one bounded class-C call; decisions and dispatches as records; the
forge, git, AEP and ESS collectors; `max_working`; the KPI definitions; onboarding checks.

**Parts that exist to manage the system's own complexity** (*inferred* classification): the context
hand-over and its races; usage-limit resume and account rotation; the supervisor and teacher roles;
the maintainer's self-development loop; the guard as the only safety check of `bypassPermissions`
sessions; mailbox delivery into terminal panes (`conductor-run.md:100-112`); much of reclaim and
lease code; the dashboard UX gate; the v1 daemon, pod and API work for a system that ran on one host.

**Verdict:** over-engineered relative to what it delivered per token and per operator hour. The
cause is architectural: LLM sessions hold control flow at three levels (controller, maintainer,
supervisor and teacher), and most of the code that is not the loop compensates for that.

## 7. What SDC must gain before it could replace Conductor (question 4)

| # | gap | evidence it is missing | where it is planned |
|---|---|---|---|
| 1 | version control of SDC itself, and its planning store populated | no commits; store holds `project.yaml` only (§ 1) | nothing planned |
| 2 | real verification: conformance against the implementation, independent reviews, the verifier hole closed | `driver.md` gaps 16 and 17; `eval-design.md:30-35` | SDC docs: `driver.md` gaps 16-17 ("Next: ESS's generated … runner"), `eval-design.md` § 4.5 |
| 3 | real git: a worktree per story, branch, merge, release tag | merge and publish are `cp` (`sdc-tool.sh:230, 235`) | `orchestration.md` § 2.4a (one worktree per story, removed at merge); no story in any store |
| 4 | forge evidence: PR, CI and release state as `canon-evidence/1` | none in SDC protocols or bindings (`grep -iE 'forge\|pr\.\|ci\.'` → 0 hits); "evidence producers for GitHub and CI do not exist anywhere" (unpublished host records) | nothing planned (takt has a forge merge, `takt.md:98`) |
| 5 | executed parallelism with WIP limits | `orchestration.md:31-35, 175-191` | SDC: `orchestration.md` § 2.4, epics named at `:423` (not in the store). Loom: `story:child-case-runtime` proposed, `story:tree-run-command` proposed, `story:tree-daemon` draft (`LP/story/`) |
| 6 | resume of a cut step | `driver.md` gap 15 | `S/docs/canon-research/01-event-stream-and-resume.md` (research only); Loom `run-event-stream` implemented (`LP/story/run-event-stream.md`), SDC not on it |
| 7 | confinement of every bound command and LLM node | `layers.md:155` | Loom: `story:bound-commands-run-confined` proposed, `story:confinement-opens-from-engine-config` proposed, `story:child-launches-through-confinement-adapters` draft (`LP/story/`); `layers.md` § 5 items 8-10 |
| 8 | run on Loom instead of its own runtime copy | `driver.md` "Decision"; `boundary.md:30-33` | Loom: `story:bindings-document` active, `story:deterministic-default-selector` active (`LP/story/`) |
| 9 | brownfield entry: adopt a repository, ratchet ESS gate, import existing approvals | rated 2 of 5 (`orchestration.md:340-355`) | SDC: `orchestration.md` § 4 and § 7 decision 3; epic `brownfield-entry` (not in the store) |
| 10 | standing operation per area (never "done", re-activates on new evidence) | `orchestration.md:253-279` | SDC: `orchestration.md` § 3, draft `protocol/draft/orchestrate.protocol.yaml` |
| 11 | per-class autonomy routing and authority bound to a revision | `autonomy-levels.md:427`; `driver.md` gap 1 | SDC: `autonomy-levels.md` § 3; Canon request in `S/docs/canon-requests.md` |
| 12 | an operating record: N real stories on real repositories, with cost and operator touches per merged item | 1 run, one toy story (§ 1) | SDC: `eval-design.md` § 5 (pilot ≤ $15, not run), `kpis.md` § 3 |

Not listed on purpose: self-maintenance, multi-instance daemon and API, account rotation. These
belong to Conductor's own complexity (§ 6) and need no SDC equivalent (*inferred*). Usage-limit
handling stays a host concern: nothing planned.

## 8. Three paths (question 5)

**(A) SDC succeeds Conductor; Conductor stays frozen once § 7 is met.**

- Cost: § 7 items 1-12. Items 5, 7 and 8 sit in Loom's plan (2 active, 3 proposed, 2 draft stories
  in `LP/story/`); items 3, 4 and 12 have no story anywhere. Conductor itself costs nothing more:
  it has been stopped since 2026-11-08, with no commit since 2026-11-14.
- Risk: no fleet tool until items 3-5 exist. SDC is proven on one toy run, so its per-item cost and
  operator touches on real work are unknown. The forge evidence (item 4) is unowned.
- Keep from Conductor: decision classes, the KPI definitions (K1, K14, T3), the onboarding checks,
  and the agent-kind trait as a design reference.

**(B) Conductor adopts SDC's protocols as its executor and stays the fleet and host layer.**

- Cost: one `run-to-exit` kind that runs `sdc-drive` per dispatch (`architecture-next.md:386-391`;
  the trait exists). Plus § 7 items 2-4, which B needs as well. Plus restarting Conductor's
  operation: three instances, the maintainer, supervisor and teacher sessions.
- Risk: it brings back the burden of § 6 (2026-11-08 retro) for the fleet layer. Two state models
  (Conductor's store and Canon case files) need reconciling. Conductor's fleet layer was built for
  interactive sessions (mailbox, hand-over, guard), and most of it would become dead weight
  (*inferred*).

**(C) Both continue separately.**

- Cost: two systems maintained, plus takt as a third executor (§ 5) and Loom's runtime under all of
  them. `boundary.md:30-33` already counts four driver copies.
- Risk: the cost per item is never compared on the same work. The duplication of § 4 grows.

**Recommendation: A.** Conductor's stop is already a de facto freeze, and the measured problems
(cost per PR, operator touches, self-management load) come from the part SDC replaces: LLM
sessions holding control flow. Order: § 7 items 1, 2, 3 and 12 first (cheap, and they decide
whether SDC delivers at all), then 4, then 5, 7 and 8 through Loom.

**What would change it:**

- To B: real multi-repository work is needed before SDC's item 5 exists. Then run `sdc-drive` as a
  `run-to-exit` kind under a minimal Conductor (loop and dispatch only, no supervisor or teacher),
  for a bounded period.
- Away from SDC as the executor: the first 10 real SDC stories with items 2 and 3 in place cost
  more per merged item than Conductor's $11 median per dispatch (`self-improvement.md:79`), or need
  more than 0.245 operator touches per merged item (`:39`).
- Toward takt instead of SDC as the Canon executor: takt's M5 week of daily runs
  (`takt.md:101`) shows real delivery before SDC's item 12 does.

## 9. Not found or not verified

- A work item named "Canon-based executor" or "executor blend": not found (§ 5). Its identification
  with takt is *inferred*.
- Cost per delivered item for Conductor after 2026-10-13: not found. The dollar figures cover one
  week, and they are list-price equivalents from tokens on a subscription (`self-improvement.md:173`).
- The models used by controller sessions in the cost window: not found in the cited tables.
- Worktree count: the Conductor repository has 17 (`git worktree list \| wc -l`). Managed trees
  across the host were counted only in unpublished host records (helper 2), inflated by group
  directories: approximate.
- `delivered` in the dispatch logs of the other instances (unpublished host records) counts message
  deliveries into sessions (`conductor-run.md:100`), not work items. Merged-item counts for those
  two instances: not found.
- Canon `check` reaching exactly 65,536 states on three protocols: whether that exploration
  finished below the bound or stopped at it is not verified.
