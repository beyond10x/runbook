# R-03 triage

Round aimed at one operator complaint of 2026-11-21: "mobile view looks pretty unusable atm". Applied
by rule (SKILL § 3.3): priority = severity weight × frequency × confidence ÷ cost, with S1 9, S2 3,
S3 1; frequency = scenarios affected ÷ 9 (the six hermetic scenarios and the recorded runs
trial-2-train, trial-3-haiku, trial-todo-1); confidence 2 when the phone measurement and the
screenshot review agree, else 1; cost S 1, M 2, L 4.

## Caveats

- No person, no recording, no questionnaire, no paid level (L4, L5), no review agents: the round was
  run by one session, as asked. The two sources are the program's facts
  (`ux/specs/phone.spec.mjs` over `ux/core/phone.mjs`, file `shots/before-phone-facts.jsonl`) and the
  session's own look at every before shot.
- Severity by the operator's question: can the operator see what is running and take or see a
  decision on a phone; cannot is S1. A finding that fails a hard gate at the gating size is S1 too
  (R-01's rule): UXF-022, through axe-core target-size (serious, hard gate 5).
- The rules of the phone size are the operator's (decision D-006 in `ui/README.md`); the 12 px floor
  for labels (up to 3 words) is this round's reading of "no body text under 14 px" for short labels.
- The before measurement ran twice: the first pass counted the body's overflow as a clipping box
  (it belongs to the viewport) and so found no element past the edge; `ux/core/phone.mjs` was fixed
  and the measurement repeated on the same page. Only the second pass is kept.
- 768x1024 (measured only) already fit before the change (no content past the edge); its tap
  targets and type were as small as on the phones.

## Did the last change sets move their metrics?

| change set | target metric | moved |
|---|---|---|
| R-01 C1 to C5 | screen_only, axe, scroll, the person and decider reads | yes, R-02 report (0 hard-gate and 0 task failures) |
| R-02 (stopped runs, autopilot, real principals; no triage) | none of the loop's | not a loop change set |

The phone keys are new in this round; the next round reports their movement.

## Severity by rule and priorities

| finding | sev | scenarios | freq | conf | cost | priority | in set |
|---|---|---|---|---|---|---|---|
| UXF-017 Decisions off the screen | S1 | all 9 | 1.00 | 2 | S | 18.0 | C1 |
| UXF-016 desktop grid past the right edge | S1 | all 9 | 1.00 | 2 | M | 9.0 | C1 |
| UXF-018 current step cut and covered | S1 | all 9 | 1.00 | 2 | M | 9.0 | C1 |
| UXF-022 tap targets under 44 px, axe target-size | S1 | all 9 | 1.00 | 2 | M | 9.0 | C5 |
| UXF-024 KPI tiles cut and overlapping | S2 | all 9 | 1.00 | 2 | S | 6.0 | C1 (same surface) |
| UXF-020 drawer and dialog not sheets | S2 | 5 | 0.56 | 2 | S | 3.33 | C4 |
| UXF-019 rail a column off the screen | S2 | all 9 | 1.00 | 2 | M | 3.0 | C2 |
| UXF-023 type under 12 and 14 px | S2 | all 9 | 1.00 | 2 | M | 3.0 | C5 (same rule block) |
| UXF-021 graph clipped, no hint | S2 | 3 measured | 0.33 | 2 | S | 2.0 | C3 |
| UXF-025 phase names not readable | S3 | all 9 | 1.00 | 1 | S | 1.0 | open |
| UXF-026 What changed cuts actor and action | S3 | 8 | 0.89 | 1 | S | 0.89 | C1 (the one-column wrapping) |

Four S1 findings, on two surfaces: the page frame (UXF-016, UXF-017, UXF-018) and the controls'
size (UXF-022). Both are in the set, so one change set of five was enough (no second set). The three
remaining slots went to the S2 findings in priority order; UXF-024 and UXF-026 ride on C1 and
UXF-023 on C5 because their fix is the same rule set as the change they join. UXF-025 stays open.

## Change set R-03

| change | findings | target metric | regression asset (red on 4d5d75d, green after) |
|---|---|---|---|
| C1 one column; sticky header with store, Follow and Decisions; a status bar with the run's state and current step; KPI tiles two a row; cards and rows wrap | UXF-016, UXF-017, UXF-018, UXF-024, UXF-026 | phone.hscroll_px, phone.wide, phone.hidden, phone.truncated_hidden, phone.covered, phone.kpi_reach | test/mobile.spec.mjs UXF-016, UXF-017, UXF-018, UXF-024, UXF-026 |
| C2 the right panel a full-screen sheet opened from Details | UXF-019 | phone.sheet_rail | test/mobile.spec.mjs UXF-019 |
| C3 the graph scrolls in its own box at scale 1.3, with a hint | UXF-021 | phone.graph | test/mobile.spec.mjs UXF-021 |
| C4 the drawer and the dialogs full-screen sheets, Close 44 px | UXF-020 | phone.sheet_drawer, phone.sheet_diff | test/mobile.spec.mjs UXF-020 |
| C5 tap targets 44x44, labels 12 px, body text 14 px | UXF-022, UXF-023 | phone.small_targets, phone.small_text, phone.small_body_text, phone.axe_serious | test/mobile.spec.mjs UXF-022, UXF-023 |

The phone layout contract (`ux/specs/phone.spec.mjs`, every key moment, gating at 390x844) asserts
all of them at once; it failed on every one of its 9 tests at 390x844 before C1 and passes after C5.
Red run of test/mobile.spec.mjs on 4d5d75d: 10 failed, 0 passed. Green after each change: C1 5 of
its 5, C2 4, C3 5, C4 2, C5 10 of 10.

Desktop: every phone rule sits under `@media (max-width: 900px)`; the status bar, Details and the
graph hint are `display: none` above it. Screenshots of trial-2-train, trial-3-haiku and
trial-todo-1 (page, Explain open, drawer open) at 1600x1000 and 1366x768 from the base and from the
change set differ only in the header's live clock ("updated hh:mm:ss").

## Gate

After merging `impl/ui-stopped-runs` (the shared clock, D-005), on the merge and the commit after it:

- Whole UI suite with `runs/`: 117 passed, 83 skipped, exit 0. In a fresh worktree without `runs/`:
  76 passed, 124 skipped, exit 0. Both with `SDC_UI_NOW_PORT` and `SDC_UI_AUTOPILOT_PORT` moved,
  because another session's suite held 7496/7497 and 7530 at the same time.
- Round: 0 hard-gate and 0 task failures at 1600x1000, 1366x768 and 390x844. The one failure is
  changes at 1280x720 (measured only), as in R-02.
- Ratchet: 238 same at the desktop sizes, nothing worse; the 226 new keys at 390x844 (`phone.*` and
  the loop's) taken with `--update`, all of them 0 for `phone.*`.
- Fast loop then ratchet, three times after the update: the first stopped on ECONNRESET from the
  agent scenario's server (no assertion failed); the next two passed 32 and read "464 same, 0 other"
  twice in a row.
- The shared clock showed one bug in this change set: at 360x800 the screen-only user's scroll
  skipped a line cut a fraction of a pixel below the screen. Fixed in loop.spec (the scroll centres
  the first visible match).
