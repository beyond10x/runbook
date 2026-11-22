---
format: aep.planning-md/3
id: review-result:adversary-unit-c-pass-1
kind: review-result
status: active
title: Adversary pass 1 on unit C (documents) of wave 2026-11-22-w1, head 9fa5d1e
summary: '25 findings: 21 introduced, 4 pre-existing. The reviewer''s second sending: the store refused the first one''s findings block (unquoted YAML).'
tags:
- wave-2026-11-22-w1
relations:
- reviews: story:driver-runtime-removed
revision: 1
---
unit: C (documents), tree `<trees>/b10x/runbook/line3-c`, branch `impl/line3-docs`, head `9fa5d1e`, base `4677c2b`
verdict: red
cases: executed 0→5 (sh checks run by path; no suite exists for text and none was run), red 5
origin: introduced 21 / pre-existing 4 / undecided 0
wrote-outside-worktree: 10 paths
needs-coordinator: (1) merging C with unit A's head so far (`2229e62`) conflicts in 7 hunks: `AGENTS.md` 1, `docs/eval-design.md` 2, `docs/orchestration.md` 2, `docs/kpis.md` 1, `docs/parity.md` 1. (2) C changed 25 lines of `docs/cycles-suite.md`, which its brief gives to unit A. (3) Rows 4 and 13 turn on whether unit A deletes or rewrites `driver/examples/sdc/*.bindings.json` and `sdc-check-routes`. (4) The 4 pre-existing rows sit in C's own file list, so routing them out of the wave leaves the site and README contradicting the pages C rewrote.

The strongest verdict in the findings is NEEDS-CHANGE. This resend changes nothing in the 25 findings; only the block's form, the two path roots and the header's second line differ. No new work was done in the tree.

## 1. `git --no-pager diff --stat`

Empty. `git status --porcelain` is also empty (0 lines), before and after. I edited nothing in the tree and wrote nothing under `.engineering/`. The unit's own diff, base..head, is 48 files, +320 −244.

## 2. Cases added

All five are POSIX sh under `<scratch>/removal/line3-c/adversary/`, and all are red now.

**`case-01-history-list.sh <tree> <history.md>`** asserts that every file in the unit's scope with a remaining grep line has a row in the unit's history list. Exit 1:
```
FAIL not named as history: docs/progress.json (25 lines)
FAIL not named as history: docs/progress.md (7 lines)
FAIL not named as history: docs/trial-1.md (14 lines)
FAIL not named as history: docs/trial-2.md (35 lines)
FAIL not named as history: docs/trial-3.md (3 lines)
```

**`case-02-driver-sections.sh <tree>`** asserts that the three sections `docs/driver.md` declares live name nothing deleted, say true things, and are the only ones cited by pages that are no record. It prints `fails: 18`; line 1274 is a false positive of my pattern (past tense), so 17 count. Lines cut at 210 columns:
```
FAIL live section "Instructions" line 1090:   (`instructions/conformance.table.md`, `.refused.md`, `.run-fault.md`) and `sdc-unstick`
FAIL live section "Instructions" line 1091:   (`instructions/run.unstick.md`, `.retry.md`, `.closing.md`). They share one function,
FAIL live section "Instructions" line 1095:   `sdc-unstick`). A binary finds `instructions/` as the tools do: the nearest directory above the
FAIL live section "Instructions" line 1112: against `instructions/golden/` (40 prompts on stub inputs, among them the retry texts, the writer's own failur
FAIL live section "Action-to-tool bindings" line 1274: pool_wait_spent` or `run_over_time`. The former trial script ended with that cause and did not call `run.unsti
     record sections: Child cases 2, Observation records 3, Case store 2, The loop 7, Status 4, Full-cycle run 3, LLM executor 36, Gaps 1, Recorded replay 8, Decision 1
FAIL driver/examples/sdc/*.bindings.json: sdc-node on 0 lines, llm-node.sh on 14 lines
FAIL bindings/*.yaml: --instructions on 14 lines
     ok   AGENTS.md:23 "Now"; AGENTS.md:49 "Instructions"; bugfix.md:46, workflow-metrics.md:43, loom-bindings.md:84,87 "The build pool wait"
FAIL docs/conductor-comparison.md:67:driver.md`, "Fixed policy": lies in "The loop", which the page calls a record of the deleted runtime
FAIL docs/conductor-comparison.md:67:driver.md`, "Tools for a chooser": lies in "The loop", ...
FAIL docs/conductor-comparison.md:145:driver.md`, "Verdicts and exit codes": lies in "The loop", ...
FAIL docs/conductor-comparison.md:207:driver.md` "Decision": lies in "Decision", ...
FAIL docs/bugfix.md:410:driver.md`, "Child cases": lies in "Child cases (TEMPORARY: ...)", ...
FAIL docs/fs-changes.md:83:driver.md` "Observation records": lies in "Observation records (2026-11-21)", ...
FAIL docs/decisions.md:210:driver.md`, "Decider replies": lies in "LLM executor (2026-11-21)", ...
FAIL docs/run-compare.md:51:driver.md` "Unstick": lies in "LLM executor (2026-11-21)", ...
FAIL docs/autonomy-levels.md:218:driver.md` "Fixed policy": lies in "The loop", ...
FAIL docs/autonomy-levels.md:260:driver.md` "Approvals by program": lies in "LLM executor (2026-11-21)", ...
FAIL docs/autonomy-levels.md:466:driver.md`, "Decider replies": lies in "LLM executor (2026-11-21)", ...
fails: 18
```
(The "record sections" and "ok" lines are condensed here; the script prints one per line.)

**`case-03-shifted-citations.sh <tree> <base>`** asserts that a `path:line` citation that opened on its text at the base still does at the head. Exit 1:
```
docs/layers.md: base line 3 is now line 11 (base line 1 now 1)
docs/eval-design.md: base line 3 is now line 9 (base line 1 now 1)
docs/kpis.md: base line 3 is now line 8 (base line 1 now 1)
docs/reject-routes.md: base line 3 is now line 8 (base line 1 now 1)
FAIL AGENTS.md cites docs/layers.md:47 for "Is meant to be": on it at the base, not at the head
FAIL docs/decisions.md cites docs/kpis.md:123 for "H2": on it at the base, not at the head
FAIL docs/decisions.md cites docs/kpis.md:87 for "R8": on it at the base, not at the head
     docs/conductor-comparison.md -> docs/kpis.md:255 "L1": base no, head no
     docs/conductor-comparison.md -> docs/layers.md:155 "confine": base no, head no
FAIL docs/takt-comparison.md cites docs/layers.md:164 for "takt": on it at the base, not at the head
```

**`case-04-live-pages.sh <tree>`** asserts that the pages a reader lands on name nothing the deletion removes as the thing to use; the acceptance grep sees none of these lines. Exit 1:
```
FAIL built-on: website/docs/built-on.mdx:13 holds "the runtime is meant to come from Loom"
FAIL built-on: website/docs/built-on.mdx:15 holds "Today the driver still holds a temporary copy of the runtime parts"
FAIL built-on: website/src/data/layers.ts component loom has use:'not-yet'
FAIL built-on: website/src/data/layers.ts:99 holds "Not used yet. Runbook is meant to run as an application on Loom."
FAIL built-on: website/src/data/layers.ts:87 holds "The driver calls canon evaluate as a subprocess"
FAIL context: website/docs/context.mdx:117 holds "Every lever that exists on `main`"
FAIL context: website/docs/context.mdx:120 holds "`scripts/run-trial.sh <run> "<idea>"` writes"
FAIL context: website/docs/context.mdx:140 holds "`sdc-drive approve … --result rejected"
FAIL context: website/docs/context.mdx:144 holds "Where: `SDC_MAX_REJECTIONS`"
FAIL context: website/docs/context.mdx:152 holds "the node's entry in `bindings/tools/llm-node.sh`"
FAIL context: website/docs/context.mdx:164 holds "or `--autopilot` (A5)"
FAIL context: website/docs/context.mdx:32 holds "For every model node that tool is `llm-node.sh`"
FAIL context: website/docs/context.mdx:36 holds "They are written in `llm-node.sh`"
FAIL context: website/src/data/context.ts:138 says instructions/, cites llm-node.sh:540-559 at 7bbdee60c4595bf2beee8406a3f45b984def5dba, which has 0 directory instructions/
FAIL readme: README.md:96 holds "`doctor`, `list` and `--version` so far"
FAIL readme: README.md:105 holds "Check, evaluate and trial scripts"
FAIL agents: AGENTS.md:64 holds "start with `phases.md`, `driver.md`, `orchestration.md`"
FAIL fs-changes: docs/fs-changes.md:167 holds "`scripts/watch-run.sh start|stop|status <run-dir>`"
FAIL cycles: docs/cycles-suite.md:58 holds "task cycles:only SCRATCH=<scratch dir> NAMES="
FAIL cycles: docs/cycles-suite.md:6 holds "Until then the script runs the cycles and the suite is checked against it."
     (Taskfile.yml: cycles:only runs scripts/run-finding-cycles.sh)
FAIL bugfix: docs/bugfix.md:172 names bugfix-from-ticket as a cycle of cycles/tree-suite.yaml; the file has 0
FAIL conformance: docs/conformance.md:17 says the observation is skipped on a recorded re-drive; runbook/src/loom_run/conformance.rs reads "recorded" on 0 lines
FAIL reject-routes: marked a record at docs/reject-routes.md:3; scripts/check-bindings.sh:153 fails the gate when its "Check output" block differs from the live check
FAIL stop-rule: AGENTS.md:120 gives the stop of one run without --run <name>; driver/src/bin/sdc-experiment.rs:2709 stops every open run of the record then, and :2735 sets the record abandoned
FAIL stop-rule: AGENTS.md says "the way docs/loom-run.md says a run is stopped"; docs/loom-run.md holds sdc-experiment stop on 0 lines
fails: 25
```

**`case-05-paths-in-added-lines.sh <tree> <base>`** asserts that every repository path in a line the unit added exists and is not deleted. Exit 1 with 6 hits. I judge none a defect of its own: two are proposals carried over, one is an ignored directory, one is historical, one is past tense, and the `cycles/suite.yaml` hits belong to row 24.
```
1 FAIL docs/autonomy-levels.md names eval/admitted.yaml: not on the tree
1 FAIL docs/autonomy-levels.md names protocol/autonomy-levels.yaml: not on the tree
2 FAIL docs/cycles-suite.md names cycles/suite.yaml: the deletion removes it
1 FAIL docs/kpis.md names eval/runs: not on the tree
1 FAIL docs/quality-0.1.0.md names driver/examples/sdc/tools/: not on the tree
1 FAIL website/src/data/layers.ts names driver/src/main.rs: the deletion removes it
```

## 3. Suite run

None. The brief forbids cargo, a build or a test run in this tree, and no docs build was run. Every check is the grep or `git show` above.

## 4. Findings (head `9fa5d1e` against base `4677c2b`)

Rows marked (w) are true at the base and turn false when unit A's deletion lands. They do not reproduce at the base, so they are `introduced` by the wave and fall to this unit as its documents half.

| # | file:line | verdict | origin | what reaches it |
|---|---|---|---|---|
| 1 | `AGENTS.md:120` | NEEDS-CHANGE | introduced | An agent that finds one stuck run types the rule's command. Without `--run <name>` it stops every open run of the record (`sdc-experiment.rs:2709`) and sets the record `abandoned` (`:2735`). The base rule stopped one named run. The rule also credits `docs/loom-run.md`, which never names `sdc-experiment stop`. |
| 2 | `website/docs/context.mdx:117` (w) | NEEDS-CHANGE | introduced | Sidebar page "Context". Under the unit's record note (18-22) it still says "Every lever that exists on `main`" and gives `scripts/run-trial.sh`, `sdc-drive approve`, `SDC_MAX_REJECTIONS`, `bindings/tools/llm-node.sh`, `--autopilot` (120, 140, 144, 152, 164). The history reason "permalinks" covers 27 of its 34 grep lines; 32, 36, 38, 50, 120, 140, 152 are prose. |
| 3 | `<scratch>/removal/line3-c/history.md` | NEEDS-CHANGE | introduced | Acceptance wants each history file named with its reason. `docs/trial-*.md` (52 lines) and `docs/progress.*` (32 lines) have no row, though brief step 5 lists both groups. Case 01. |
| 4 | `docs/driver.md:1127` | NEEDS-CHANGE | introduced | A section declared live says the agent nodes of `driver/examples/sdc/*.bindings.json` run `sdc-node`. Those files hold `sdc-node` on 0 lines and `llm-node.sh` on 14. |
| 5 | `docs/driver.md:1090` (w) | NEEDS-CHANGE | introduced | The live section "Instructions" lists `sdc-unstick` and `instructions/run.unstick.md` (1090, 1091, 1095, 1112) as existing; unit A deletes both. |
| 6 | `docs/driver.md:1108` | CONFIRMED | pre-existing | "`bindings/*.yaml` names no instruction file", while `bindings/*.yaml` has `--instructions` on 14 lines at the head and at the base. |
| 7 | `docs/driver.md:36` | NEEDS-CHANGE | introduced | The declaration turns "The loop", "LLM executor", "Child cases", "Observation records" and "Decision" into records. 11 present-tense citations from 6 pages that are no record point into them (list in case 02). |
| 8 | `AGENTS.md:85` | NEEDS-CHANGE | introduced | It cites `docs/layers.md:47-52` for the layers table; the unit's top paragraph moved the table to 55-60. |
| 9 | `docs/decisions.md:233` | NEEDS-CHANGE | introduced | It cites `kpis.md:123` (H2) and `kpis.md:87` (R8); both moved 5 lines down. |
| 10 | `docs/takt-comparison.md:85` | CONFIRMED | introduced | It cites `docs/layers.md:164`; the line moved 8 down. Not checked one by one: 62 unpinned line citations into `docs/driver.md`, which shifted 36 lines. |
| 11 | `docs/bugfix.md:172` | NEEDS-CHANGE | introduced | It names four cycles of `cycles/tree-suite.yaml`. `bugfix-from-ticket` is not in the file (it is a Rust test, `docs/cycles-suite.md:376`); `bugfix-not-red` and `bugfix-edited-test` are `pending: true`. |
| 12 | `docs/conformance.md:17` | NEEDS-CHANGE | introduced | The rewrite attaches "skipped on a recorded re-drive" to the observation as such. At the base it described the trial script. `run_loop.rs:163` calls `after_close` on every run, and `conformance.rs:38-47` checks only the option and the outcome. |
| 13 | `docs/reject-routes.md:3` | NEEDS-CHANGE | introduced | Marked "Record … kept as the record of the audit". `scripts/check-bindings.sh:153` fails the gate when its "Check output" block differs from a live `sdc-check-routes` run, and `plugin/skills/harden/SKILL.md:51` tells the reader to run it. |
| 14 | `docs/parity.md:4` | NEEDS-CHANGE | introduced | "It did" (the same facts under both). The same page marks items **Open** at 436, 444, 462, 504, 507, and `story:parity-run-on-loom` is `status: active`. |
| 15 | `website/src/data/context.ts:138` | NEEDS-CHANGE | introduced | The rewritten lever says "instruction files under instructions/" and cites `llm-node.sh:540-559` at commit `7bbdee60`, which has no `instructions/`; those lines are the inline prompt. |
| 16 | `website/src/routes/Evaluation.tsx:586` | NEEDS-CHANGE | introduced | One sentence says runs start "with the driver's trial script in autopilot" and that it "starts `runbook run`". Line 588 keeps the stop record "by `rule:autopilot`"; the runner writes `rule:loom-run` (`docs/loom-run-directory.md:76`). |
| 17 | `website/docs/run-it.md:35` | NEEDS-CHANGE | introduced | The new `runbook run sdc --dry …` command stands under a citation of `README.md:61-68` at tag 0.1.1, which shows `scripts/run-trial.sh --dry trial-dry`. "A stub idea" also stands beside a required `--intake <idea file>`. |
| 18 | `README.md:83` | NEEDS-CHANGE | introduced | "Try it", and `run-it.md:38`, give `runbook run …` with no step that builds `runbook` or puts it on `PATH`. The base command was a script that built it. Line 76 still says "the scripts build `runbook` once". |
| 19 | `README.md:96` | NEEDS-CHANGE | pre-existing | The layout table says `runbook` has "`doctor`, `list` and `--version` so far", 13 lines below the unit's `runbook run` command. Line 105 says "trial scripts". |
| 20 | `website/docs/built-on.mdx:13` | NEEDS-CHANGE | pre-existing | Sidebar page "Built on": "the runtime is meant to come from Loom … Today the driver still holds a temporary copy" (13-15). |
| 21 | `website/src/data/layers.ts:97` | NEEDS-CHANGE | pre-existing | Loom is `use: 'not-yet'`, "Not used yet" (99); Canon is "The driver calls canon evaluate as a subprocess" (87). The unit edited lines 70 and 102 of this file. |
| 22 | `docs/fs-changes.md:167` (w) | NEEDS-CHANGE | introduced | It gives `scripts/watch-run.sh start\|stop\|status <run-dir>` as a command; unit A deletes the script. The unit edited this page elsewhere. |
| 23 | `docs/loom-run.md:295` (w) | NEEDS-CHANGE | introduced | Live text cites `driver/src/main.rs`, which unit A deletes. Also `docs/loom-bindings.md:73`, `docs/workflow-metrics.md:25,47`, `bindings/PENDING.md:26`, `docs/bugfix.md:353,359`. The acceptance grep has no pattern for that path. |
| 24 | `docs/cycles-suite.md:58` | CONFIRMED | introduced | The unit changed 25 lines here, table rows included, against brief step 6. It left `task cycles:only` (58; the task runs the deleted script) and line 6 in present tense. |
| 25 | `AGENTS.md:64` (w) | CONFIRMED | introduced | "start with `phases.md`, `driver.md`, `orchestration.md`". `driver.md` is now mostly a record, and the unit changed the same sentence in `README.md:59` to `loom-run.md`. |

## 5. Attacked, not broken

- Every option of the README and run-it commands exists in clap (`runbook/src/loom_run/here.rs`, `runbook/src/client/mod.rs`).
- Exit numbers and labels match `docs/loom-run.md:258-269` (7 `stopped: signal`, 8 `stopped: decider`).
- `sdc-experiment stop --reason` exists, writes `run-stopped.json` and stops the systemd unit.
- All 18 programs in the "Now" table of `docs/driver.md` are `[[bin]]` entries of `driver/Cargo.toml`.
- Task names cited exist: `test:cycles-loom`, `check:cycles-suite`, `test:loom-real`, `test:loom-in-process`, `test:replay`.
- `sdc-node` takes `--instructions`, `--adapter`, `--model` (`driver/src/bin/sdc-node/setup.rs:54`).
- Plugin `SKILL.md` files name existing paths and commands.
- With the acceptance's five names alone, the live sections of `docs/driver.md` hold 0 lines; all 56 are in record sections.
- The record marks of `docs/kpis.md` and `docs/eval-design.md` are at the top and match where the old names sit.
- No person name, organisation name, home path, `file://`, or text from a run directory in the 320 added lines or the 4 commit messages.
- The AGENTS.md rules on money, logins, confinement and what never enters git are word for word as at the base.
- All 4 commits are author and committer `b10x-bot[bot]`, each with a body.
- Website tests assert none of the changed texts.

## 6. Paths written outside the worktree

All under `<scratch>/removal/line3-c/adversary/`:
- `case-01-history-list.sh`
- `case-02-driver-sections.sh`
- `case-03-shifted-citations.sh`
- `case-04-live-pages.sh`
- `case-05-paths-in-added-lines.sh`
- `case-05.out`
- `docs-rest.diff`
- `grep.out`
- `added.txt`
- `merge-probe.txt`

The `worktree` lease of session `adversary-line3-c-p1` on the tree is released (exit 0).

## 7. Findings block

```findings
[
  {"file": "AGENTS.md", "line": 120, "category": "contract-drift", "severity": "blocker", "verdict": "NEEDS-CHANGE", "origin": "introduced", "message": "the stop rule gives sdc-experiment stop without --run, which stops every open run of the record and sets the record abandoned, and credits docs/loom-run.md, which does not name the command"},
  {"file": "website/docs/context.mdx", "line": 117, "category": "acceptance", "severity": "blocker", "verdict": "NEEDS-CHANGE", "origin": "introduced", "message": "a sidebar page marked as a record still says \"Every lever that exists on main\" and gives deleted commands, and 7 of its 34 grep lines are prose the history reason does not cover"},
  {"file": "<scratch>/removal/line3-c/history.md", "category": "acceptance", "severity": "warning", "verdict": "NEEDS-CHANGE", "origin": "introduced", "message": "the history list has no row and no reason for docs/trial-*.md (52 grep lines) and docs/progress.* (32)"},
  {"file": "docs/driver.md", "line": 1127, "category": "contract-drift", "severity": "warning", "verdict": "NEEDS-CHANGE", "origin": "introduced", "message": "a section declared live says the agent nodes of driver/examples/sdc/*.bindings.json run sdc-node, and those files name llm-node.sh on 14 lines and sdc-node on none"},
  {"file": "docs/driver.md", "line": 1090, "category": "contract-drift", "severity": "warning", "verdict": "NEEDS-CHANGE", "origin": "introduced", "message": "the section \"Instructions\", declared live, lists sdc-unstick and instructions/run.unstick.md, which the deletion removes"},
  {"file": "docs/driver.md", "line": 1108, "category": "contract-drift", "severity": "note", "verdict": "CONFIRMED", "origin": "pre-existing", "message": "the sentence that bindings/*.yaml names no instruction file is false, the files carry --instructions on 14 lines"},
  {"file": "docs/driver.md", "line": 36, "category": "judgement", "severity": "warning", "verdict": "NEEDS-CHANGE", "origin": "introduced", "message": "declaring every other section a record leaves 11 present-tense citations from 6 pages that are no record pointing into record sections"},
  {"file": "AGENTS.md", "line": 85, "category": "contract-drift", "severity": "warning", "verdict": "NEEDS-CHANGE", "origin": "introduced", "message": "the citation docs/layers.md:47-52 no longer opens on the layers table, which the unit's top paragraph moved to 55-60"},
  {"file": "docs/decisions.md", "line": 233, "category": "contract-drift", "severity": "warning", "verdict": "NEEDS-CHANGE", "origin": "introduced", "message": "the citations kpis.md:123 and kpis.md:87 no longer open on H2 and R8 after the unit's top paragraph"},
  {"file": "docs/takt-comparison.md", "line": 85, "category": "contract-drift", "severity": "note", "verdict": "CONFIRMED", "origin": "introduced", "message": "the citation docs/layers.md:164 no longer opens on the takt line, and 62 unpinned line citations into docs/driver.md moved by 36 lines unchecked"},
  {"file": "docs/bugfix.md", "line": 172, "category": "contract-drift", "severity": "warning", "verdict": "NEEDS-CHANGE", "origin": "introduced", "message": "bugfix-from-ticket is named a cycle of cycles/tree-suite.yaml and is not in it, and two of the other three are pending"},
  {"file": "docs/conformance.md", "line": 17, "category": "contract-drift", "severity": "warning", "verdict": "NEEDS-CHANGE", "origin": "introduced", "message": "the rewrite says the observation is skipped on a recorded re-drive, which was the trial script's behaviour and is not in the runner"},
  {"file": "docs/reject-routes.md", "line": 3, "category": "judgement", "severity": "warning", "verdict": "NEEDS-CHANGE", "origin": "introduced", "message": "the page is marked a kept record while scripts/check-bindings.sh:153 gates its \"Check output\" block against a live check and the harden skill tells the reader to run it"},
  {"file": "docs/parity.md", "line": 4, "category": "judgement", "severity": "warning", "verdict": "NEEDS-CHANGE", "origin": "introduced", "message": "\"It did\" states the parity result as reached while the page keeps five items marked Open and the parity story is active"},
  {"file": "website/src/data/context.ts", "line": 138, "category": "contract-drift", "severity": "warning", "verdict": "NEEDS-CHANGE", "origin": "introduced", "message": "the rewritten lever names instructions/ and cites llm-node.sh:540-559 at a pinned commit that has no instructions/ directory"},
  {"file": "website/src/routes/Evaluation.tsx", "line": 586, "category": "contract-drift", "severity": "warning", "verdict": "NEEDS-CHANGE", "origin": "introduced", "message": "one sentence says runs start with the driver's trial script and with runbook run, and the next keeps the stop record by rule:autopilot where the runner writes rule:loom-run"},
  {"file": "website/docs/run-it.md", "line": 35, "category": "contract-drift", "severity": "warning", "verdict": "NEEDS-CHANGE", "origin": "introduced", "message": "the new runbook run command stands under a pinned citation of README.md:61-68 at 0.1.1, which shows the trial script, and \"A stub idea\" stands beside a required --intake"},
  {"file": "README.md", "line": 83, "category": "judgement", "severity": "warning", "verdict": "NEEDS-CHANGE", "origin": "introduced", "message": "Try it and run-it give runbook run with no step that builds runbook or puts it on PATH, which the replaced script did itself"},
  {"file": "README.md", "line": 96, "category": "contract-drift", "severity": "warning", "verdict": "NEEDS-CHANGE", "origin": "pre-existing", "message": "the layout table says runbook has doctor, list and --version so far, below the unit's runbook run command, and line 105 still says trial scripts"},
  {"file": "website/docs/built-on.mdx", "line": 13, "category": "judgement", "severity": "warning", "verdict": "NEEDS-CHANGE", "origin": "pre-existing", "message": "the site page Built on says the runtime is meant to come from Loom and the driver still holds a temporary copy"},
  {"file": "website/src/data/layers.ts", "line": 97, "category": "judgement", "severity": "warning", "verdict": "NEEDS-CHANGE", "origin": "pre-existing", "message": "the data of the Built on page has Loom as not-yet and Canon as a subprocess of the driver, in a file the unit edited at lines 70 and 102"},
  {"file": "docs/fs-changes.md", "line": 167, "category": "contract-drift", "severity": "warning", "verdict": "NEEDS-CHANGE", "origin": "introduced", "message": "the page gives scripts/watch-run.sh as a command to run and the deletion removes the script"},
  {"file": "docs/loom-run.md", "line": 295, "category": "contract-drift", "severity": "warning", "verdict": "NEEDS-CHANGE", "origin": "introduced", "message": "live text in five pages cites driver/src/main.rs, which the deletion removes and the acceptance grep has no pattern for"},
  {"file": "docs/cycles-suite.md", "line": 58, "category": "boundary", "severity": "warning", "verdict": "CONFIRMED", "origin": "introduced", "message": "the unit changed 25 lines of a page its brief gives to unit A and left task cycles:only and line 6 naming the deleted script as current"},
  {"file": "AGENTS.md", "line": 64, "category": "judgement", "severity": "note", "verdict": "CONFIRMED", "origin": "introduced", "message": "the reading order still starts with driver.md, now mostly a record, while the same sentence in README.md was changed to loom-run.md"}
]
```
