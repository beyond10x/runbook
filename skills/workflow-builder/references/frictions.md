# Frictions: what cost time or made failures repeat (SDC, 2026-11-21)

Ranked by the minutes or repeats behind each. Each item: what happened, the number, the source, and
the rule in `SKILL.md` it produced. Sources as in `timeline.md`.

## F1. Stopping at the first error with a full-length run

- Every trial-3 attempt ended at the first path that ran with a real model for the first time; the
  defects behind that point waited for the next attempt (transcript 10:15:45Z).
- Full runs took 1,609 to 2,554 s wall (A3 to A5). Stop-to-next-start loops took 50, 57, 115 and
  33 min. First start to first pass: 401 min for five attempts (`timeline.md`, "Totals").
- Each attempt yielded one or two defects: 9 run ends, 9 distinct causes.
- The operator, 10:14:29Z: "stopping the session for every single baby error isn't useful, because
  you're just having too big of cycles." The orchestrator's answer: the saving is *inferred*, not
  measured; one defect (the intake stop of `trial-3-mixed-4`) appeared in only one of two samples
  (10:15:45Z). `--keep-going` (`2a95956`) came after the pass.
- Rule: SKILL.md phase 5, keep going past stops and collect defects; fix in one batch.

## F2. The same class of stop came back at a later step

- `rejected_without_rework_path`: merge approval (`trial-3-mixed-2`, 05:00:59Z, recorded with
  cause null), release (`trial-3-haiku-3`, 06:22:22Z), root close (`trial-3-haiku-5`, 10:09:32Z).
  Each fix covered the step that stopped (`a23755a`, `25e0709`/`b6ce269`, `83d3cbf`).
- `writer_spent`: `spec.author` (`trial-3-haiku-2`), then `change.implement` (`trial-3-haiku-4`).
- Decider shown an obligation only a later step can meet: `release.publish` post-publish claims
  (trial 1), `questions.settled` at `scope.answer` (trial 2b), `health.required` at
  `release.publish` (`trial-3-mixed-3`). Three occurrences, three local fixes, the last
  `073c173` generic (derived from the compiled protocol).
- `host-operator` asked at 10:16:06Z for an audit of every step for both classes. That one audit
  (`docs/reject-routes.md` on `audit/reject-routes`) listed 16 decision points and 15 writer or
  cap rows, found 9 missing or wrong routes (R1 to R9) plus 3 dead ends nobody had asked about
  (R10 to R12), and printed 13 dead ends as program output. One full run found one stop.
- Rule: SKILL.md phase 6, fix the class across all steps; phase 2, route table before the first run.

## F3. The decider's input produced wrong refusals

Of the 9 run ends before the batch, these came from what the decider was shown or how its reply was
checked, not from the product:

- A1, both runs: ids it had to cite did not exist in its input (`bbd927e` body).
- A1: the run mode did not tell it that it was final (`f74c68b` body).
- A4 `trial-3-mixed-3`: an obligation of a later step shown as open (`073c173`).
- A5 `trial-3-haiku-5`: the planning tool's `status: draft` front matter read as "not approved";
  `Cargo.lock` dropped from its input copy, so four decisions doubted `tests.pass` or
  `release.built`; the prompt offered "escalate to a person" in a run with no person (`75e65e1`).
  The fourth reason, r1 citing two contradicting answers, was a real fault (F7).
- A5 `trial-3-mixed-4`: a valid decision refused by a program check over a fault the decision did
  not write (`102ce99`).
- Count: 5 of 9 ends trace wholly or partly to the decider's input or reply check.
- Rule: SKILL.md phase 3, audit every decision's input against what that decision needs.

## F4. Stub cycles cannot find prompt-dependent defects

- The fixture adapter ignores the prompt (transcript 10:15:45Z; D138 at 09:49:15Z: a review node
  received its own rejection as "Rework" text, found by the merger, not by any cycle).
- Stub cycles grew from 16 to 53 during the day; no attempt ended on a path a cycle covered.
- Rule: SKILL.md phase 4, stub cycles prove routes and machinery only; real runs are the test of
  prompts and inputs.

## F5. Checks were green on a hollow product

- `trial-3-haiku-3`: `gate.passed`, `release.built`, `release.accepted` all true while sign-in (r3)
  and the offline copy (r4) were never built (`runs/trial-3-haiku-3/console.txt:27`). The spec
  trace accepted a requirement citing a domain with no command (`eb8e384` body).
- The code review step was a program stub through all five attempts and the batch (transcript
  10:35:33Z). The decider was the only node that judged the product.
- The spec writer copied a topology example that names Postgres and was never told the builder's
  "standard library only" rule (close diagnosis, transcript 10:16:51Z, *inferred* by that agent).
- Rule: SKILL.md phase 1, every requirement traces to checkable behaviour; phase 3, examples in
  prompts are neutral to the product.

## F6. Rules lived in the driver and scripts, where no static check saw them

- The rework limit was a literal `2` in `run-trial.sh`, the refusal ends a bash array, the cap
  counted per arrival and shared across retry kinds (`docs/reject-routes.md` on
  `audit/reject-routes`, "The check (D148)": `rule-ends.json` "holds the autopilot rules that lived
  only in code").
- Canon's `protocol/1` has no count form (transcript 10:41:51Z), so a loop bound cannot be a
  protocol fact without a recorded verdict (D152).
- Rule: SKILL.md phase 2, every bound and rule end is data a checker reads.

## F7. Amendments appended decisions without reconciling requirements

- `trial-3-haiku-5`: r1 kept q10 (Postgres) after q12 (data file) replaced it; the product used the
  data file, the published spec still said Postgres; three decider approvals saw the contradiction
  and approved (`460b486` body; transcript 10:20:54Z).
- `trial-3-mixed-3`: r3's text and its appended decision q8 disagreed (D144, 10:20:54Z).
- Rule: SKILL.md phase 3, a replacing decision supersedes the old one in every artifact that cites it.

## F8. Static analysis came late

- Requested by the operator at the start (2026-11-20 21:56Z: "a deterministic simulation of all …
  cases … exhaust the state space").
- `canon simulate` existed on Canon's unreleased branch by 04:17Z and was used once (plan, spec,
  epic: 1,113 findings to 53 after `7f453fb`). Its remaining stuck states were not triaged.
- At 09:21Z, 6 of 9 simulate findings on two protocols were already gone, removed by the refused-
  release changes (`25e0709`, `ebe8694`) that the A3 run had forced at a cost of a 1,609 s run and a
  115 min loop.
- All eight protocols were simulated first at 10:38Z, after the pass: 71 findings, 8 stuck states.
- Rule: SKILL.md phase 2, static analysis is a gate before the first real run.

## F9. The model setting was fixed by the goal, not chosen for a first pass

- The goal (02:36Z) named the settings: all Haiku, and Sonnet for some nodes; the decider was
  Haiku in every attempt.
- The operator at 06:01Z: "maybe haiku is not strong enough for that task". The first run with a
  stronger decider started at 10:26Z, 265 min later, after the operator's 10:25Z message.
- That batch passed 7 min 34 s after it started (`timeline.md`).
- Rule: SKILL.md phase 5, first real runs use the strongest setting.

## F10. One merge queue, many branches, the same files

- Files changed most by the day's fix commits (63 non-merge commits from 04:00Z on `main`,
  `fix/attempt5-close`, `fix/superseded-answers`): `scripts/run-finding-cycles.sh` 20,
  `docs/driver.md` 14, `driver/examples/sdc/tools/llm-node.sh` 12, `scripts/run-trial.sh` 10,
  `sdc-tool.sh` 9 (`git log --name-only`).
- Recorded collisions: the `scope-two` fixture, `amend_one` in `sdc-findings.rs`, the
  `intake-decision` cycle, a clippy lint, and D138 (transcript summary 4, "Errors and fixes";
  10:33:23Z).
- The queue's gates after A3 took 44 min (07:31Z merge to 08:15Z push).
- A5 ran from a trial branch ahead of the queue (D135, 09:25:23Z), which cut that loop to 33 min.
- Rule: SKILL.md phase 6, run the next attempt from a trial branch with the fixes; merge behind it.

## F11. A late-stage defect needed a full run to reach

- A release defect was reachable only after 25 to 40 min of earlier stages (A3, A4, A5).
- Stages that run alone from recorded inputs were filed only at 10:33Z (`c1b95e9`,
  `story:stage-runs-alone`); release and operate were cut from the cycle at 10:36Z (D150) on the
  operator's words ("release and operate makes no sense in an eval").
- Rule: SKILL.md phase 1, every stage runnable alone from recorded inputs; cut stages a pass does
  not need.

## F12. Runs unlabelled, shown running when stopped, or labelled wrong

- A canned demo loop was shown beside real runs for 3 h 50 min of elapsed clock and taken for a
  real run by the operator (transcript 03:05:46Z).
- Stopped runs showed running clocks (operator 02:41Z); stub runs with a billing fixture were
  taken for real output (operator 06:00:48Z, reply 06:01:34Z).
- The strong passes record the Opus decider as `decider:haiku`; a console line says 13 calls where
  `llm-calls.jsonl` has 12 (`host-operator`, 10:35:46Z).
- A reopened run counted 553 s of waiting for a fix as waiting on a person
  (`trial3/attempt5-notes.txt`).
- Rule: SKILL.md "What to record".

## F13. The orchestrator's own wrong statements

- Corrected within the trial window: "the rework was skipped" (09:00Z, corrected 09:25:23Z: it ran
  and its code did not compile); "`sdc-finding-kpis` exits 1" (the `$?` of another command,
  09:03:39Z); "the write-time check missed it" (corrected 09:48:27Z); a line number `:259` for
  `:279` (06:33:27Z); a UI brief that never tested the operator's window size (08:13:42Z).
- Corrected by other sessions earlier: a Substrate deny rule, Loom's crash case, D107, D131, a
  pointer to Canon's `protocol-floor` (transcript summaries 3 and 4).
- Rule: SKILL.md phase 6, every claim about a run cites the record line; read each exit code alone.

## F14. Usage limits and disk

- The model API usage limit stopped subagents at least seven times; each time every agent was
  resumed by message (transcript summaries 3 and 4).
- At 07:40Z the disk was at 38 GB free, below the build pool's 50 GB limit; every build on the
  machine queued (transcript 07:40:46Z).
- Rule: none specific; keep parallel runs within what the pool and the disk hold.
