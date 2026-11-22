# R-01 report review

Reviewer: report reviewer (review-agents.md § 2), fresh context. Inputs read: `report.md`, `round.json`,
`playwright.json`, `shots/`, `ux/out/metrics-*.jsonl`, the round README, the tool sources. No previous
round, no `trend.md` history, no questionnaire. Nothing re-run.

**Verdict: not fit to send as is.** Default action: send R-01 only as the list of failed checks in § 1
with the caveats in § 4; do not use its metric table or the `baseline.json` taken from it (written
02:22:02, 37 s after the round, from this failing run) as a ratchet; fix T1–T4 and T14–T16 before
R-02.

## 1. Are the numbers right and meaningful?

Recomputed from `ux/out/metrics-*.jsonl` (464 records, no duplicates, every record in its viewport's
file) and `playwright.json`:

- Every one of the 476 metric cells in `report.md` equals the raw fold; `round.json` equals the raw
  fold; `baseline.json` (234 entries, gating viewports) equals this run's values exactly.
- Tests: 36 results, 13 passed, 17 failed, 6 skipped (engine and findings specs run once by design).
  The report gives none of these counts.
- "29 gate failures" (report, `trend.md`) is 29 failed expectations, not 29 gate failures: 19 are hard
  gates (gate 1 document scroll: 1366x768 ×6 and 1600x1000 `two`; gate 5 axe: 1366x768 ×6,
  1600x1000 ×6), 10 are screen-only-user task failures, and 5 of those 10 are at the measured-only
  viewports 1280x720 and 1920x1080. Differs by 10 from the hard-gate count.
- `trend.md` Σ screen_only.actions 16, Σ looks 106, Σ truncated_hidden 353 sum all four viewports;
  over the gating viewports they are 8, 53, 177 (each differs by exactly half).
- The gate messages in the report ("Received + 23") hide the content. From `playwright.json`: gate 5
  is two axe rules everywhere, `color-contrast` (58–89 nodes, KPI labels `.k-time > .k-l`,
  `.k-cost > .k-l`) and `scrollable-region-focusable` (`#tree`, `.fd-list`, 1–2 nodes); gate 1 is
  `document` every time.

Does each metric mean what its name says this round?

| metric | meaningful? | why |
|---|---|---|
| screen_only.actions, .looks, actions_per_fix, glance_cost | no | only successes are counted (T1). `person` scores 1 action / fix 1 at all 4 viewports while the approve step failed at all 4; `decider` looks 2 at 1280/1366 vs 3 at 1600/1920 because the `claude-haiku-5-5` read failed. Lower here means worse. |
| hard gate 4 (fix ≤ 2 actions) | vacuous | passes on `person` and `changes`, whose fixes never completed (T1). |
| completion.miss / .false | vacuous on 4 of 6 | `agent`, `two`, `decider`, `changes` have completion `null` in the adapter (T4). |
| cue_churn.* | partly | blind to Deciding → Awaiting a person in `decider` (0 changes, visible change in the shots) and to a new action in the same case (T3). |
| screen_only.time_to_act_steps.max | no | measures how long the fixture takes to reach its truth (0 everywhere), not the user (T5). |
| contract.axe_serious | count of nodes, not violations | 2 rules each time; the node count grows with viewport size (61 → 93), sampled once (T7). |
| layout_shift.per_minute | noisy | a CLS score per minute over ~8 s; `agent` 0.5 at 1280x720 vs 0.04–0.06 elsewhere (T8, T14). |
| reading_load.* | noisy | `agent` median 63 at 1280x720 vs 78 at 1366x768 with equal max 78/78: sampled at different moments (T14). |
| cue.* (engine) | tautological | the truth re-implements the code under test (T17). |
| contract.scroll, .hidden, .truncated_hidden, keyboard.tabs_to_cue | yes, as named | — |

Scenario truth: `decider`'s truth holds only for the first seconds (T15), `two` moves through
several actions during sampling (two-end shows `plan.decompose` and `spec.synthesize`), so their
L2 metrics are over states that are not the card's truth.

## 2. Findings the report misses

1. The approve control sits below the fold of the Decisions drawer at every viewport
   (`shots/1366x768-person-decided.png`: only the field table and "Continue run" are in view). The
   report shows `person` as a clean 1-action task.
2. What changed rows are `<div class="fd-row">` (`web/feed.js:89`), not buttons: not reachable by
   role or keyboard. At 1366x768 the rows are below the viewport entirely
   (`shots/1366x768-changes-end.png`). The `agent` "open the diff" pass came from a Now-band file
   button via the regex fallback (`shots/1366x768-agent-diff.png` opens `llm/015…/main.rs`), not
   from the newest row (T2).
3. The Now band's height varies with content at 1366x768: ends at about y 257 (`agent-0`), 310
   (`decider-0`, "driver attached pid … since …" wrapped over 5 lines), 355 (`changes-end`), leaving
   the graph about 100 px. Inferred cause of the document scroll and of `two`'s layout shift.
4. The decider scenario flips from "DECIDING … claude-haik…" to "AWAITING A PERSON … no driver
   attached" with "by decider 1" (`shots/1366x768-decider-0.png` vs `-end.png`): a decision recorded
   and the person still asked, with no explanation on screen (partly the fixture, T15).
5. Clocks disagree on one screen: `person-0` at 1366x768 shows "waiting on a person 7:52", "4:13
   waiting since 02:15:24 AM" and "2026-11-20 23:19:43"; `1600x1000-changes-0` shows the same instant
   as "2026-11-20 23:57:02" (timeline) and "12:57:02 AM" (Just finished), two time zones unmarked.
   Partly the fixture (T16).
6. Two revisions in one decision: drawer chip "revision td1e3bfc8da7ec670", text "approve context
   revision tdfd49c629fecba19" (`1366x768-person-decided.png`). Fixture artefact (T16), but it puts
   the person on the "changed since the run recorded it" path, not the card's.
7. Requirement ids leak into KPI labels at 1600 and 1920 ("R7 · L2", "R6 · H4", "R4", "H1 · H2",
   "F10", `1600x1000-two-0.png`).
8. "Cost $0.00" next to "decider $0.004" (`1366x768-changes-end.png`); "12/8 turns" in the decider
   card (`1366x768-decider-0.png`); "Live" on the closed recorded run (`1280x720-closed-0.png`).
9. The D-001 scroll exceptions `#tree` and `.fd-list` are the ones failing axe
   `scrollable-region-focusable`: the baseline's exception contradicts gate 5.
10. The `changes` end shot still shows the Specification filter marked on with the other rows faded
    after the bot toggled it off (`1600x1000-changes-end.png`); inferred, the state at the shot is
    unclear.

## 3. Tool defects

- **T1** `ux/specs/loop.spec.mjs:60-72, 156-162`: `read`/`use` record looks and actions only on
  success and `u.fixes.push` runs whether or not the fix completed, so a failed task lowers
  actions, looks, glance and actions_per_fix. Input: `person` at all viewports, `decider` at
  1366x768/1280x720. The ratchet will call fixing the approve control "WORSE".
- **T2** `ux/specs/loop.spec.mjs:130-137`: the last-change task matches the row's text as an exact
  button name (rows are divs) and falls back to any visible button matching
  `/\.(rs|md|yaml|toml|lock)\b/`; `adapter.mjs:34` `lastChange.cls: 'code'` is never checked. Input:
  `agent` passes by opening a Now-band file.
- **T3** `ux/specs/loop.spec.mjs:97`: stripping `:\d+$` from the card key leaves `wait:<case>:<action>`
  and `run:<case>`, so a decider → person handoff and successive actions in one case are not churn.
  Input: `decider` (0 changes).
- **T4** `ux/adapter.mjs:34-49`: completion `null` for 4 scenarios; the README cards say `decider`
  "waiting" and the others "running". Input: every non-person, non-closed scenario.
- **T5** `ux/specs/loop.spec.mjs:45-55`: `time_to_act_steps` counts 500 ms polls until the fixture's
  truth appears, not the user's steps. Input: all scenarios, 0.
- **T6** `ux/specs/loop.spec.mjs:60-72`: glance 3 (not in view, SKILL § 7) is never recorded; a miss
  records nothing. Input: `person` approve step.
- **T7** `ux/specs/loop.spec.mjs:93, 107`: axe runs on sample 0 only and the metric sums nodes, so it
  scales with viewport size. Input: `person` 64 at 1366x768 vs 93 at 1920x1080, same two rules.
- **T8** `ux/core/contract.mjs:73-83`: `layout_shift.per_minute` is a CLS sum extrapolated from an
  ~8 s window. Input: `agent` 0.5 at 1280x720 vs 0.04 at 1366x768.
- **T9** `ux/core/metrics.mjs:10-13`: only per-test aggregates are written; the samples (words,
  truncation, card keys) are lost, so medians and maxima cannot be recomputed.
- **T10** `ux/core/round.mjs:40`: error messages cut to 6 lines and 400 characters drop the axe rule
  ids, node counts and the scroll offender. Input: every gate line ("Received + 23").
- **T11** `ux/core/round.mjs:53, 77-79`: "Gates" lists task failures and measured-only viewports as
  gates, the header does not mark gating viewports, and `trend.md` sums across all viewports. Input:
  29 vs 19 hard-gate failures; Σ 16/106/353 vs 8/53/177.
- **T12** `ux/core/round.mjs:65-69`: worst moments take the first maximum on ties and always link the
  `-0` shot. Input: `contract.scroll` names `person` at 1280x720 (measured-only) while the gate
  fails at 1366x768; layout shift is measured after the `-0` shot is taken.
- **T13** `ux/specs/loop.spec.mjs:22-33, 163`: the keyboard check stops at opening the drawer (gate 5
  "completed by keyboard alone" is not checked); the `-decided` shot is written when nothing was
  decided. Input: `person-decided.png` at all viewports.
- **T14** `ux/specs/loop.spec.mjs:18, 87-95` with `ux/specs/env.mjs:34-39`: sampling on the wall clock
  against a fixture at speed 3, two projects in parallel, makes values differ by moment, not by
  layout; `round.mjs:5` claims determinism and the ratchet's count tolerance is 0. Input: `agent`
  reading_load.median 63 vs 78 at equal max.
- **T15** `test/fixture-run.mjs:272-278`: `decider` stops after the decider call: decision recorded,
  case left awaiting a person, driver gone. Input: `decider-0` vs `decider-end`.
- **T16** `test/fixture-run.mjs:77-92`: events are cut back but `work/` files stay at the final
  revision and event `at` stays at the recorded time while mtimes and observations are now. Input:
  `person` drawer revisions td1e3… vs tdfd49…; "Elapsed 2:03:34" and three waiting clocks.
- **T17** `ux/specs/engine.spec.mjs:29-36`: the cue truth repeats `web/now.js:48-55` (`waitOf`), so
  `cue.inaccuracy_pct` 0 holds by construction.
- **T18** `ux/core/ratchet.mjs:31`: the committed-baseline check reads `git show HEAD:`; the repo has
  no commits and `ui/ux/` is untracked, so the "baseline made worse" check is silently off. Input:
  `git log` "does not have any commits yet".
- **T19** `ux/specs/env.mjs:15-40` (inferred risk, not observed to misfire): each failing test starts
  a new worker (19 workers for 4 projects); `start` deletes the viewport's fixtures and binds the same
  port while the old server is killed without waiting, and the readiness loop accepts any answer.

## 4. Caveats to send with the round

- No person, no recordings, no questionnaire, no previous round: nothing here is a trend.
- The metric table is not a measure of the UI this round: screen-only metrics reward failure (T1),
  completion is unchecked on 4 of 6 scenarios (T4), and timing-dependent metrics differ between
  viewports for the same layout (T14).
- `baseline.json` now holds this failing run's values; ratcheting on it locks in T1's numbers.
- The `decider`, `person` and clock observations are partly driver-bot artefacts (T15, T16).
- The UI offered a "Continue run" control in the drawer the bot drives (removed since: the inspector
  acts on nothing); the bot's role-name matching kept it off, a broader pattern would have started a run.
