# Jumps: which changes moved the frontier (SDC, 2026-11-21)

A jump is ranked only by a number: the stage reached (`timeline.md`, stage numbers 1 to 7), a
count of ends turned into continuations, or the first pass. One sample per setting per attempt is
the norm in this record, so each delta is a single observation, not an established effect.

## Ranked

| # | Change | Measure | Before | After | Source |
|---|---|---|---|---|---|
| J1 | Strongest setting (Opus decider, Sonnet for every work node) in a parallel batch of 10 runs | first pass | 0 passes in 9 run ends over 401 min | 2 of 2 strong runs closed `operating` 7 min 34 s and 7 min 53 s after the start | `runs/trial-3-strong-{1,2}/status.json`; transcript 10:35:33Z |
| J2 | Spec loop fix: repeated findings dropped, no re-derivation for the writer's own defaults (`7f5ef7c`, `37fba0b`) | frontier, Haiku setting | 3 (A2, `writer_spent` after 6 `spec.author` calls) | 6 (A3, 1 `spec.author` call) | `trial3/attempt2-notes.txt`, `attempt3-notes.txt` |
| J3 | Decider replies schema-constrained, citations resolvable, invalid replies retried twice with the check's error, then a fallback (`bbd927e`, `f74c68b`) | frontier | 2, round 3 (A1, both settings) | 3 (Haiku) and 5 (mixed) in A2; 0 `invalid_reply` lines in the 17 later runs' `decisions.jsonl` | commit bodies; `grep -c invalid_reply runs/trial-3-*/decisions.jsonl` |
| J4 | A rejected approval sends the writer back with the reasons, at most 2 times (`d4139af`, `4b62a59`, `a23755a`) | merge rejects that ended the run | 1 of 1 (A2, `trial-3-mixed-2`) | 0 of 4 in A4 and A5: each was reworked and then approved (`trial-3-haiku-4` 08:34:38Z, `trial-3-mixed-3` 08:31:04Z and 08:40:12Z, `trial-3-haiku-5` 09:55:16Z) | `trial3/attempt4-notes.txt`, `attempt5-notes.txt` |
| J5 | Ends by rule: a refused release ends the tree `not_delivered` (`25e0709`, `b6ce269`); a terminable root closes by `rule:autopilot` (`83d3cbf`) | runs that closed by themselves | 0 of 7 trial-3 runs before A4 | `trial-3-mixed-3` closed `not_delivered` at 08:52:25Z, the first; `trial-3-haiku-5` reopened on `83d3cbf` closed `operating` with 0 new calls | `runs/trial-3-mixed-3/status.json`; `attempt5-notes.txt` |
| J6 | Next attempt run from a trial branch with the fixes, ahead of the merge queue (D135) | stop to next start | 115 min (A3 to A4, through the queue) | 33 min (A4 to A5) | transcript 09:25:23Z; `timeline.md` |

## What J1 does and does not show

- Two variables changed at once: the decider (Haiku to Opus) and the work nodes (mixed to Sonnet).
  `trial-3-sonnet-1` (Sonnet work, Haiku decider, same tree) was still in the spec stage at
  10:45Z after 13 `spec.author` calls; `trial-3-opus-1` (Opus work, Haiku decider) was in the story
  stage. One sample each: the decider is the likelier factor, *inferred*.
- The Opus decider approved all 22 of its decisions. No reject route ran in either pass
  (`host-operator`, 10:35:46Z). The Haiku decider rejected 1 to 3 times in each of six runs
  (`decisions.jsonl` of `trial-3-haiku-{3,4,5}`, `trial-3-mixed-{2,3,5}`); one of those rejects
  (`trial-3-haiku-3`, release) was a correct product judgement, one (`trial-3-mixed-3`, release)
  a false refusal from its input.
- The review step was the stub in both passes; nobody started the published app (10:35:33Z).
- The strong runs spent 278 and 306 s in the driver; the all-Haiku runs on the same tree had spent
  788 and 833 s and were unfinished at 10:45Z.
- Cost was not the constraint: the five attempts cost 4.60 USD in model calls together, the two
  passes 5.92 USD.

## Built after the pass, not yet measured on the frontier

| Change | What the record shows so far | Source |
|---|---|---|
| Route audit over every decision point and cap | one pass found 12 routes (R1 to R12) and 13 dead ends; each dynamic attempt had found one | `docs/reject-routes.md` on `audit/reject-routes` |
| `sdc-check-routes` and `rule-ends.json` | the audit's dead ends as program output, compared line by line in `check-bindings.sh` | same, "The check (D148)" |
| Canon `simulate` on all eight protocols | 71 findings, 8 stuck states, 63 cycles; triage running | transcript 10:38:54Z |
| `--keep-going` (`2a95956`) and `--reopen` with refused decisions (`6c4886a`) | red-green cycles only | commit bodies |
| Scope cuts: `sdc.bugfix`, stages that run alone, cycle ends at `delivered` | filed or designed; `sdc.delivery` 128 states | `c1b95e9`; transcript 10:31:36Z, 10:44:04Z |
| Superseded answers (`460b486`), decider close input (`75e65e1`) | red-green cycle `superseded-answer` replays the `trial-3-haiku-5` sequence | commit bodies |

## Earlier static analysis, measured once

- At 04:17Z an agent used Canon's unreleased `simulate` on the plan, spec and epic protocols and
  fixed one rule in all eight protocols (`7f453fb`: a check that records a result runs once per
  artifact revision): findings 1,113 to 53. The intake rose from 3 to 177, which the agent
  attributed to cycles that had hidden them (transcript 04:17:35Z).
- At 09:21Z, 6 of 9 simulate findings on `sdc` and `sdc.epic` were already removed by the changes
  that A3's dynamic stop forced (`25e0709`, `ebe8694`): the static walk had shown the same dead ends.
