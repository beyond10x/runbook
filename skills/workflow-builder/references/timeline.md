# Timeline: from the first real run to the first pass (SDC, 2026-11-20 and 2026-11-21)

Worked example for the skill. All times are UTC. "Stage" numbers the SDC stages in order: 1 intake,
2 direction, 3 spec, 4 plan, 5 story, 6 release, 7 root close. "Frontier" is the furthest stage a run
reached. Sources: run directories under `runs/` (git-ignored, read on 2026-11-21 at about 10:45Z),
the trial notes in the orchestrator's session scratch (`trial3/attempt{2,3,4,5}-notes.txt`,
`trial3/kpis-*.txt`), commit hashes of `beyond10x/sdc`, and the orchestrator transcript
(session `018bcc85…`, cited by timestamp).

## Before trial 3 (person in the loop or stand-in)

| Run | Start | End | Frontier | How it ended | Figures |
|---|---|---|---|---|---|
| trial 1, `runs/trial-todo-1` | 2026-11-20 23:16:05 | about 00:02 (console mtime 00:02:43) | 7, closed `operating` | the decider escalated `release.publish` because post-publish claims were unknown; the operator approved it (`docs/trial-1.md`, transcript summary 1 "Errors and fixes") | 15 work calls 0.077 USD, 5 decider calls 0.023 USD |
| trial 2, `runs/trial-2-train` | 00:53:37 | stopped on purpose 02:21:31 at finding f16 | 5 (second story) | 8 stuck states R1 to R8 on the way, all defects in the driver or tool scripts (`docs/trial-2.md:57-64`) | 18 driver legs, 1,009 s in the driver, 5,274 s wall, 81 % of wall outside the driver (`docs/trial-2.md:21`); 27 work calls 0.503 USD |
| trial 2b, `runs/trial-2b-fresh` | 02:43:22 | 31 s run; stop record (dated 02:43:00) written at 03:08 | 1 | the decider escalated at `scope.answer` because it was shown `questions.settled: false`, the claim that action itself settles (transcript summary 3, "Errors and fixes") | 3 work calls 0.007 USD |

Gap to trial 3: 69 min (02:43 to 03:52), waiting for the autopilot run mode (`5aebd1b`, `620a00f`).

## Trial 3: five attempts, then a batch

Goal set by the operator at 02:36Z: the same idea run unattended end to end, once all Haiku and once
with Sonnet for spec, plan, implementation and review, zero time waiting on a person.
Idea: "A to-do list web app for me and my partner that also works on the train without a connection"
(`trial3/start-strong.sh`). Pass rule D143 as revised at 10:26Z: a fresh run that closes in its
success outcome by itself, with no stop, no reopen and no forced step; product faults are listed and
do not void the pass (transcript 10:26:31Z).

| Attempt | Run | Setting (work / decider) | Start | End | Frontier | Cause of the end | Fix (commits) | Stop to next start |
|---|---|---|---|---|---|---|---|---|
| A1 | `trial-3-haiku` | Haiku / Haiku | 03:52:34 | stopped 03:54:15 | 2, round 3 of 3 | decider citations refused: `context.<field>` ids were never citable because a `yq` expression failed with its error sent to `/dev/null`; the refused reply became a reject; `--autopilot` did not export `SDC_DECIDER_ESCALATE=final` | `bbd927e` (resolvable citations, 2 retries with the check's error, fallbacks), `f74c68b` | 50 min |
| A1 | `trial-3-mixed` | Sonnet writer and implement, Haiku other / Haiku | 03:52:34 | stopped 03:54:18 | 2, round 3 | same | same | 50 min |
| A2 | `trial-3-haiku-2` | Haiku / Haiku | 04:44:46 | stopped 04:57:52 | 3 | `writer_spent` `spec.author`, 4 of 4 runs: each rewrite raised one new finding (17 findings) | `7f5ef7c`, `37fba0b` | 57 min |
| A2 | `trial-3-mixed-2` | mixed / Haiku | 04:44:46 | stopped 05:00:59 | 5, second story | a valid decider reject of the merge; no action sent a rejected change back | rework path `d4139af`, `4b62a59`, `a23755a` (first used in A4) | not rerun until A4 |
| A3 | `trial-3-haiku-3` | Haiku / Haiku | 05:54:41 | stopped 06:22:22 | 6, round 9 | `rejected_without_rework_path` at `release.publish`. The reject was correct: no sign-in, no offline copy, while `gate.passed`, `release.built` and `release.accepted` were all true (`runs/trial-3-haiku-3/console.txt:27`) | `25e0709`, `b6ce269` (a refused release ends the tree `not_delivered`), `eb8e384` (every requirement traces to behaviour) | 115 min |
| A4 | `trial-3-haiku-4` | Haiku / Haiku | 08:17:29 | stopped 08:52:36 | 5, second story | `writer_spent` `change.implement`: a run that called no model counted as run 3 of 3, so a compile failure could not go back to the writer | `b296a18` | 33 min (68 min start to start) |
| A4 | `trial-3-mixed-3` | mixed / Haiku | 08:17:29 | closed `not_delivered` 08:52:25 | 6, round 13 | the decider refused the release because `health.required` was shown open, an obligation only the later `release.observe` can meet. First run of the day to close by itself | `073c173` (`met_after_this_action`), `ccb3571` (replay after a child closes) | 33 min |
| A5 | `trial-3-mixed-4` | mixed / Haiku | 09:25:47 | stopped 09:28:33 (55 s in the driver) | 1 | `decision_not_takeable`: a valid decider decision refused by a program check over a citation fault the normalise step wrote earlier | `102ce99` | 48 min (to `trial-3-mixed-5`) |
| A5 | `trial-3-haiku-5` | Haiku / Haiku | 09:25:47 | stopped 10:09:32 | 7, round 14 of 14 | the decider rejected the root close: r1 cited q10 (Postgres) and q12 (data file); it misread the planning tool's `status: draft`; its input lacked `Cargo.lock` | `83d3cbf` (close by `rule:autopilot`), later `460b486`, `75e65e1` | 17 min (to the batch) |
| A5 reopen | `trial-3-haiku-5` | same | 10:17:34 | closed `operating` 10:17:34 | 7 | reopened on `83d3cbf`; 0 new calls; counts as defect evidence, not a pass (`trial3/attempt5-notes.txt`) | — | — |

Commit times are local (+0100) in `git log`; the table converts them to UTC.

### The batch (10:26:33Z, tree `trial/3b` at `acd6f12`, call cap raised from 90 to 150)

Started after the operator's message at 10:25:32Z ("just pass the whole thing … even if it's very
costly because you use more expensive models like Opus"). Eight to-do runs plus two runs of a smaller
idea (a line, word and character counter) at 10:27:41Z (`trial3/start-batch.sh`, `start-small.sh`).

| Run | Setting (work / decider) | State at about 10:45Z | Work calls | Decider records | Cost USD (work + decider) | Driver s |
|---|---|---|---|---|---|---|
| `trial-3-strong-2` | Sonnet / Opus | closed `operating` 10:34:07, 454 s wall, replay 10 of 10 | 12 | 11, 0 rejects | 1.034 + 1.962 = 2.996 | 278 |
| `trial-3-strong-1` | Sonnet / Opus | closed `operating` 10:34:26, 473 s wall, replay 10 of 10 | 12 | 11, 0 rejects | 0.970 + 1.955 = 2.925 | 306 |
| `trial-3-mixed-5` (started 10:16:23) | mixed / Haiku | closed `not_delivered` 10:44:36, release withheld, 3 rejects | 15 | 14 | 1.308 + 0.280 | 478 |
| `trial-3-haiku-6` | Haiku / Haiku | running, story stage (`todo.access`) | 15 | 10 | 0.235 + 0.131 | 788 |
| `trial-3-haiku-7` | Haiku / Haiku | running, story stage (`todo.access`) | 12 | 10 | 0.209 + 0.138 | 833 |
| `trial-3-mixed-6` | mixed / Haiku | running, story stage (`list.items`) | 13 | 10 | 0.982 + 0.163 | 508 |
| `trial-3-sonnet-1` | Sonnet / Haiku | running, spec stage, 13 `spec.author` calls | 18 | 17 | 1.329 + 0.282 | 331 |
| `trial-3-opus-1` | Opus / Haiku | running, story stage | 11 | 9 | 3.069 + 0.114 | 636 |
| `trial-3-small-haiku-1` | Haiku / Haiku | running, story stage, 1 reject | 16 | 11 | 0.158 + 0.165 | 614 |
| `trial-3-small-sonnet-1` | Sonnet / Haiku | running, spec stage, 8 `spec.author` calls | 15 | 19 | 0.843 + 0.270 | 221 |

The strong runs' records name the decider `decider:haiku` although the model was `claude-opus-5-5`
(`run-settings.jsonl` `decider.kind`, `console.txt`); `host-operator` reported it at 10:35:46Z.

Read at 10:53Z, after this record ends: `trial-3-small-sonnet-1` stopped `decider_budget_spent`
at 10:46:51Z in its spec, `trial-3-small-haiku-1` stopped `writer_spent` on `change.implement` at
10:50:11Z, `trial-3-sonnet-1` stopped `decider_budget_spent` at 10:50:37Z in the story stage;
`trial-3-haiku-6`, `-7`, `trial-3-mixed-6` and `trial-3-opus-1` were still open. The small idea did
not pass in either setting it ran in, both with the Haiku decider.

## Totals

- First trial-3 start to first pass: 03:52:34 to 10:34:07, 401 min, five attempts, 9 runs ended
  before the batch.
- Stop-to-next-start loops: 50, 57, 115, 33 min (A1 to A4), then 17 min to the batch.
- The 115 min loop after A3: fixes committed 06:51 to 07:30; merged on the queue 07:31
  (`753210d`); the queue's gates (stub cycles, fixture cases, UI suite, Gates, CI) ran until the push
  at 08:15; at 07:40 the disk was at 38 GB free, below the build pool's 50 GB limit, and all builds
  queued (transcript 07:40:46Z, 08:08:36Z, 08:15:51Z).
- Model spend of the five attempts: 0.053 + 1.097 + 0.469 + 2.156 + 0.821 = 4.60 USD (sum of
  `llm-calls.jsonl` and `decider/*/response.json`). The two passing runs cost 5.92 USD together.
- Stub cycles on `main` grew from 16 (trial 2) to 32 (06:02Z), 36 (08:15Z), 41, 46, 48 and 53
  (10:33Z) while every attempt still ended on a path no cycle had covered.

## Frontier by attempt

| | A1 | A2 | A3 | A4 | A5 | batch |
|---|---|---|---|---|---|---|
| Haiku setting | 2 | 3 | 6 | 5 | 7 (stop at close) | still running at 10:45Z |
| mixed setting | 2 | 5 | not run | 6 (closed `not_delivered`) | 1 | 6 (closed `not_delivered`) |
| strong setting | — | — | — | — | — | 7, closed `operating`, 2 of 2 |

## After the pass (in flight when this record ends, 10:43Z)

- Route audit `docs/reject-routes.md` on `audit/reject-routes` (worktree `reject-routes-audit`):
  16 decision points, 15 writer or cap rows, routes R1 to R9 ruled (D145), R10 to R12 proposed,
  13 dead ends printed by the new `sdc-check-routes`.
- Canon `simulate` on all eight protocols at `dfe4a3f`: 71 findings, 8 stuck states, 63 cycles
  (transcript 10:38:54Z). Triage not finished.
- Unmerged: `fix/attempt5-close` (`83d3cbf`, `2a95956` `--keep-going`, `6c4886a` `--reopen`),
  `fix/superseded-answers` (`460b486`, `75e65e1`), `plan/bugfix-cycle` (`c1b95e9`: `sdc.bugfix`,
  stages that run alone), `proto/cycle-ends-delivered` (D150), `feat/simulate-gate` (D151).
