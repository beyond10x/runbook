# R-01 triage

Applied by rule (SKILL § 3.3): priority = severity weight × frequency × confidence ÷ cost, with S1 9,
S2 3, S3 1; frequency = scenarios affected ÷ 6; confidence 2 when two sources agree; cost S 1, M 2.

## Caveats

- The report reviewer judged R-01 "not fit to send as is" (review-report.md). Fixed before the build
  and the gate: T1 (a failed control or read costs a glance 3 and a failed fix counts 3), T2 (the last
  change is the newest row of What changed, of the card's class, no fallback), T3 (the cue key keeps
  the event seq), T4 (every card has a completion state: running, waiting or closed, each checked), T6,
  T7 (contract.axe_serious counts rules, not elements), T10 and T11 (round.mjs separates hard gates,
  tasks and measured-only viewports, and the trend sums the gating viewports), T13 (the keyboard check
  goes on to the Approve control). Open, carried as caveats: T5 (time_to_act counts the driver bot's
  steps), T8 (layout shift is a CLS score per minute over about 8 s), T9 (per-sample values are not
  kept), T12 (worst-moment ties), T14 (wall-clock sampling against a 3x replay: values depend on
  timing), T15 and T16 (the decider fixture stops after the decider's call; the person fixture keeps
  final work/ revisions while events are cut back, which shows two revisions: part of UXF-002), T17
  (the L0 cue truth repeats the UI's logic, so cue accuracy 0 errors is guaranteed), T18 (the repo has
  no commits, so the ratchet's "baseline worse than HEAD" check is off), T19.
- Because of T1, T3, T6 and T7 the R-01 baseline values of looks, cue churn and axe are re-based with
  decision D-002 when the change set's values are taken.
- No previous round: the table "did the last change sets move their metrics?" starts with R-02.

## Severity by rule

A finding that fails a hard gate at a gating viewport is S1 (§ 4: no merge passes with it). The
heuristic reviewer filed UXF-004 (gate 1), UXF-005 (gate 4 and gate 5) and UXF-008 (gate 5) as S2;
they are S1 here. UXF-007 stays S2; its one gate-failing instance (gate 4: the decider's model name is
cut at 1366x768) is treated as S1 and goes with C1. The graph-height half of UXF-004 is split off as
UXF-013 (S2, open). UXF-010, UXF-011 and
UXF-012 are S1 as filed (the operator could not tell what the screen meant).

## Priorities

| finding | sev | scenarios | freq | conf | cost | priority | in set |
|---|---|---|---|---|---|---|---|
| UXF-004 page scrolls at 1366x768 | S1 | all 6 | 1.00 | 2 | M | 9.0 | C4 |
| UXF-007 labels cut (decider model) | S1 | all 6 | 1.00 | 2 | M | 9.0 | C1 (the decider card); the rest stays open |
| UXF-008 contrast, scroll regions | S1 | all 6 | 1.00 | 2 | M | 9.0 | C3 |
| UXF-010 "no driver attached" | S1 | person, decider, closed | 0.50 | 2 | S | 9.0 | C1 |
| UXF-001 Approve below the fold | S1 | person | 0.17 | 2 | S | 3.0 | C2 |
| UXF-005 feed rows not controls | S1 | changes | 0.17 | 2 | S | 3.0 | C5 |
| UXF-011 whose decision | S1 | person, decider | 0.33 | 2 | M | 3.0 | C1 |
| UXF-012 several fields, one decision | S1 | person | 0.17 | 1 | M | 0.75 | C2 |
| UXF-003 layout shift | S2 | two, agent, changes, decider | 0.67 | 2 | M | 2.0 | next round |
| UXF-006 state vocabulary | S2 | 5 | 0.83 | 1 | M | 1.25 | next round |
| UXF-002 two revisions | S2 | person, decider | 0.33 | 2 | M | 1.0 | next round (T16 first) |
| UXF-009 alert colours, fades | S3 | 5 | 0.83 | 1 | S | 0.83 | next round (its red "no driver" part goes with C1) |

Eight S1 findings and at most five changes: the S1 findings on one surface share a change.

## Change set R-01

| change | findings | target metric | regression asset |
|---|---|---|---|
| C1 the Now band says what is true in words, and who holds each open decision and what taking it does | UXF-010, UXF-011, UXF-007 (decider card) | screen_only.looks, the person and decider reads | test/holder-model.spec.mjs (UXF-010, UXF-011); ux/specs/loop.spec.mjs "sees who holds the decision", "reads in words", "sees claude-haiku-5-5" |
| C2 the decision card: one decision over its fields, the one Approve control first | UXF-012, UXF-001 | actions_per_fix.max (person), screen_only.actions | test/holder-model.spec.mjs (UXF-012); ux/specs/loop.spec.mjs "the approve control is in view", keyboard "Tab reaches the Approve control" |
| C3 text contrast 4.5:1 on every surface; the scroll regions take focus | UXF-008 | contract.axe_serious | ux/specs/loop.spec.mjs "hard gate 5" |
| C4 the collapsed rail runs the full height beside the bands | UXF-004 (the scroll) | contract.scroll | ux/specs/loop.spec.mjs "hard gate 1" |
| C5 the sentence of a change is a button | UXF-005 | actions_per_fix.max (changes) | ux/specs/loop.spec.mjs "finds the last change as a control it can use" |

All loop assertions above failed in the R-01 measurement (report.md) before the change. The
holder-model tests were written before the change to web/now.js and web/inbox.js; their red run was
not kept.

Out of the change set but in the same build, from the coordinator: the server refuses every act that
would start the driver or a tool when a program it needs is not on its PATH (test/preflight.spec.mjs).
Two S1 findings arrived during the build and went into it ahead of the open S2 findings: UXF-014
(a wait on amend.upstream with no finding for a person is a writer out of runs, not a decision;
test/stall-model.spec.mjs) and UXF-015 (absolute home paths in committed files and served answers;
test/paths.spec.mjs). Tests that read a recorded run under runs/ (git-ignored) now fail naming the
missing run directory (test/need-run.mjs).

## Gate

Fast loop at the gating viewports: 16 passed, exit 0; ratchet green against the baseline with
decisions D-002 (values made worse on purpose by C1, C2 and the corrected tools) and D-003 (per-key
tolerance for sampled metrics in agent and two: three runs on unchanged code spread 70 to 110).
Whole UI suite: 98 passed, 60 skipped, exit 0. R-02 (ux/rounds/R-02/report.md): 0 hard-gate and 0
task failures at the gating viewports. Moved: contract.scroll@1366x768 1 → 0 in every scenario (C4);
contract.axe_serious 58 to 93 elements → 0 rules (C3); person task fails → completes in 2 actions,
keyboard reaches Approve (C2); changes "finds the last change" fails → passes (C5); decider
layout_shift.per_minute 0.33 → 0.012.
