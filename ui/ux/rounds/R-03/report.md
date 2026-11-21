# R-03 report

Written by `ux/core/round.mjs` at 2026-11-21T06:07:50.900Z. Suite exit 1. Viewports: 1280x720, 1366x768, 1600x1000, 1920x1080, 360x800, 390x844, 414x896, 768x1024.

## Phone (decision D-006), added by hand

- Phone layout contract (`ux/specs/phone.spec.mjs`, 9 tests: six scenarios and the recorded runs
  trial-2-train, trial-3-haiku, trial-todo-1): 0 on every `phone.*` metric at every key moment, at
  390x844 (gating) and at 360x800, 414x896 and 768x1024 (measured). Before the change set: failed 9
  of 9 at 390x844 (`shots/before-phone-facts.jsonl`, git-ignored).
- `test/mobile.spec.mjs`, one test per finding at 390x844: 10 of 10 red before C1, 10 of 10 green after C5.
- Measured on the shared clock (D-005), merged from `impl/ui-stopped-runs` before this run.
- The one suite failure, changes at 1280x720 (measured only), is R-02's, unchanged.
- Before and after at 390x844, side by side (git-ignored): `shots/compare-390x844-person-truth.png`,
  `shots/compare-390x844-person-drawer.png`, `shots/compare-390x844-agent-diff.png`,
  `shots/compare-390x844-trial-3-haiku-truth.png`, `shots/compare-390x844-trial-todo-1-rail.png`.
- Still bad on a phone: UXF-025 (phase names in the strip are initials); the Changes strip's ticks
  are 3 px buttons the target measurement does not select; the panel's tables stay dense; the store
  select cuts a long store name; the graph needs sideways panning.

## Hard gates at the gating viewports (1600x1000, 1366x768, 390x844)

Every hard gate passed.

## Screen-only user tasks at the gating viewports

Every task completed.

## Measured-only viewports (not gating)

- **1280x720** · changes: layout contract and the screen-only user: Error: finds the last change as a control it can use expect(received).toBe(expected) // Object.is equality Expected: true Received: false 158 | if (s.truth.lastChange?.cls) expect.soft(await first.getAttribute('class'), `the newest change is ${s.truth.lastChange.cls}`).toContain(`c-${s.truth.lastChange.cls}`); 159 | const ok = !!name && await use('button', name); > 160 | expect.soft(ok, 'finds the last change as a control it can use').toBe(true); | ^ 161 | if (!ok) u.fixes.push(3); 162 | if (ok) { 163 | await expect.soft(page.getByRole('dialog')).toBeVisible(); at ui/ux/specs/loop.spec.mjs:160:72

## Metrics (lower is better)

Each cell: this round / baseline / R-02.

### person

| metric | 1280x720 | 1366x768 | 1600x1000 | 1920x1080 | 360x800 | 390x844 | 414x896 | 768x1024 |
|---|---|---|---|---|---|---|---|---|
| actions_per_fix.max | 2 / — / 2 | 2 / 2 / 2 | 2 / 2 / 2 | 2 / — / 2 | 2 / — / — | 2 / — / — | 2 / — / — | 2 / — / — |
| actions_per_fix.median | 2 / — / 2 | 2 / 2 / 2 | 2 / 2 / 2 | 2 / — / 2 | 2 / — / — | 2 / — / — | 2 / — / — | 2 / — / — |
| completion.false | 0 / — / 0 | 0 / 0 / 0 | 0 / 0 / 0 | 0 / — / 0 | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| completion.miss | 0 / — / 0 | 0 / 0 / 0 | 0 / 0 / 0 | 0 / — / 0 | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| contract.axe_serious | 0 / — / 0 | 0 / 0 / 0 | 0 / 0 / 0 | 0 / — / 0 | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| contract.hidden | 0 / — / 0 | 0 / 0 / 0 | 0 / 0 / 0 | 0 / — / 0 | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| contract.scroll | 0 / — / 0 | 0 / 0 / 0 | 0 / 0 / 0 | 0 / — / 0 | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| contract.truncated_hidden | 12 / — / 9 | 11 / 11 / 8 | 4 / 4 / 4 | 4 / — / 4 | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| cue_churn.changes | 0 / — / 0 | 0 / 0 / 0 | 0 / 0 / 0 | 0 / — / 0 | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| cue_churn.flip_flops | 0 / — / 0 | 0 / 0 / 0 | 0 / 0 / 0 | 0 / — / 0 | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| glance_cost.median | 1 / — / 1 | 1 / 1 / 1 | 1 / 1 / 1 | 1 / — / 1 | 1 / — / — | 1 / — / — | 1 / — / — | 1 / — / — |
| glance_cost.p95 | 2 / — / 2 | 2 / 2 / 2 | 2 / 2 / 2 | 2 / — / 2 | 3 / — / — | 3 / — / — | 2 / — / — | 2 / — / — |
| keyboard.tabs_to_cue | 9 / — / 9 | 9 / 9 / 9 | 9 / 9 / 9 | 9 / — / 9 | 10 / — / — | 10 / — / — | 10 / — / — | 10 / — / — |
| keyboard.tabs_to_decide | 0 / — / 0 | 0 / 0 / 0 | 0 / 0 / 0 | 0 / — / 0 | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| layout_shift.per_minute | 0 / — / 0.01 | 0 / 0 / 0.01 | 0 / 0.01 / 0.01 | 0 / — / 0 | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| phone.axe_serious | — | — | — | — | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| phone.covered | — | — | — | — | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| phone.hidden | — | — | — | — | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| phone.hscroll_px | — | — | — | — | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| phone.kpi_reach | — | — | — | — | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| phone.scroll | — | — | — | — | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| phone.sheet_drawer | — | — | — | — | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| phone.small_body_text | — | — | — | — | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| phone.small_targets | — | — | — | — | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| phone.small_text | — | — | — | — | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| phone.truncated_hidden | — | — | — | — | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| phone.wide | — | — | — | — | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| reading_load.max | 107 / — / 98 | 107 / 107 / 98 | 116 / 116 / 107 | 116 / — / 107 | 107 / — / — | 107 / — / — | 107 / — / — | 107 / — / — |
| reading_load.median | 107 / — / 98 | 107 / 107 / 98 | 116 / 116 / 107 | 116 / — / 107 | 107 / — / — | 107 / — / — | 107 / — / — | 107 / — / — |
| screen_only.actions | 2 / — / 2 | 2 / 2 / 2 | 2 / 2 / 2 | 2 / — / 2 | 2 / — / — | 2 / — / — | 2 / — / — | 2 / — / — |
| screen_only.decision_refused | 1 / — / 1 | 1 / 1 / 1 | 1 / 1 / 1 | 1 / — / 1 | 1 / — / — | 1 / — / — | 1 / — / — | 1 / — / — |
| screen_only.looks | 8 / — / 8 | 8 / 8 / 8 | 8 / 8 / 8 | 8 / — / 8 | 10 / — / — | 10 / — / — | 8 / — / — | 8 / — / — |
| screen_only.steers | 0 / — / 0 | 0 / 0 / 0 | 0 / 0 / 0 | 0 / — / 0 | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| screen_only.time_to_act_steps.max | 0 / — / 0 | 0 / 0 / 0 | 0 / 0 / 0 | 0 / — / 0 | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |

### agent

| metric | 1280x720 | 1366x768 | 1600x1000 | 1920x1080 | 360x800 | 390x844 | 414x896 | 768x1024 |
|---|---|---|---|---|---|---|---|---|
| actions_per_fix.max | 1 / — / 1 | 1 / 1 / 1 | 1 / 1 / 1 | 1 / — / 1 | 1 / — / — | 1 / — / — | 1 / — / — | 1 / — / — |
| actions_per_fix.median | 1 / — / 1 | 1 / 1 / 1 | 1 / 1 / 1 | 1 / — / 1 | 1 / — / — | 1 / — / — | 1 / — / — | 1 / — / — |
| completion.false | 0 / — / 0 | 0 / 0 / 0 | 0 / 0 / 0 | 0 / — / 0 | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| completion.miss | 0 / — / 0 | 0 / 0 / 0 | 0 / 0 / 0 | 0 / — / 0 | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| contract.axe_serious | 0 / — / 0 | 0 / 0 / 0 | 0 / 0 / 0 | 0 / — / 0 | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| contract.hidden | 0 / — / 0 | 0 / 0 / 0 | 0 / 0 / 0 | 0 / — / 0 | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| contract.scroll | 0 / — / 0 | 0 / 0 / 0 | 0 / 0 / 0 | 0 / — / 0 | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| contract.truncated_hidden | 31 / — / 28 | 31 / 31 / 28 | 12 / 12 / 12 | 12 / — / 12 | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| cue_churn.changes | 0 / — / 0 | 0 / 0 / 0 | 0 / 0 / 0 | 0 / — / 0 | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| cue_churn.flip_flops | 0 / — / 0 | 0 / 0 / 0 | 0 / 0 / 0 | 0 / — / 0 | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| glance_cost.median | 1 / — / 1 | 1 / 1 / 1 | 1 / 1 / 1 | 1 / — / 1 | 1 / — / — | 1 / — / — | 1 / — / — | 1 / — / — |
| glance_cost.p95 | 2 / — / 2 | 2 / 2 / 2 | 2 / 2 / 2 | 2 / — / 2 | 3 / — / — | 3 / — / — | 3 / — / — | 3 / — / — |
| layout_shift.per_minute | 0.51 / — / 0.45 | 0.02 / 0.02 / 0.07 | 0.01 / 0.01 / 0.06 | 0 / — / 0.05 | 0.18 / — / — | 0.38 / — / — | 0.57 / — / — | 0.82 / — / — |
| phone.axe_serious | — | — | — | — | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| phone.covered | — | — | — | — | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| phone.graph | — | — | — | — | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| phone.hidden | — | — | — | — | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| phone.hscroll_px | — | — | — | — | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| phone.kpi_reach | — | — | — | — | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| phone.scroll | — | — | — | — | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| phone.sheet_diff | — | — | — | — | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| phone.sheet_rail | — | — | — | — | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| phone.small_body_text | — | — | — | — | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| phone.small_targets | — | — | — | — | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| phone.small_text | — | — | — | — | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| phone.truncated_hidden | — | — | — | — | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| phone.wide | — | — | — | — | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| reading_load.max | 78 / — / 78 | 78 / 78 / 78 | 89 / 89 / 89 | 89 / — / 89 | 78 / — / — | 78 / — / — | 78 / — / — | 78 / — / — |
| reading_load.median | 52 / — / 52 | 52 / 52 / 76 | 63 / 63 / 63 | 63 / — / 63 | 52 / — / — | 52 / — / — | 52 / — / — | 52 / — / — |
| screen_only.actions | 1 / — / 1 | 1 / 1 / 1 | 1 / 1 / 1 | 1 / — / 1 | 1 / — / — | 1 / — / — | 1 / — / — | 1 / — / — |
| screen_only.looks | 6 / — / 6 | 6 / 6 / 6 | 6 / 6 / 6 | 6 / — / 6 | 7 / — / — | 7 / — / — | 7 / — / — | 7 / — / — |
| screen_only.steers | 0 / — / 0 | 0 / 0 / 0 | 0 / 0 / 0 | 0 / — / 0 | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| screen_only.time_to_act_steps.max | 0 / — / 0 | 0 / 0 / 0 | 0 / 0 / 0 | 0 / — / 0 | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |

### two

| metric | 1280x720 | 1366x768 | 1600x1000 | 1920x1080 | 360x800 | 390x844 | 414x896 | 768x1024 |
|---|---|---|---|---|---|---|---|---|
| actions_per_fix.max | 0 / — / 0 | 0 / 0 / 0 | 0 / 0 / 0 | 0 / — / 0 | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| actions_per_fix.median | 0 / — / 0 | 0 / 0 / 0 | 0 / 0 / 0 | 0 / — / 0 | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| completion.false | 0 / — / 0 | 0 / 0 / 0 | 0 / 0 / 0 | 0 / — / 0 | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| completion.miss | 0 / — / 0 | 0 / 0 / 0 | 0 / 0 / 0 | 0 / — / 0 | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| contract.axe_serious | 0 / — / 0 | 0 / 0 / 0 | 0 / 0 / 0 | 0 / — / 0 | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| contract.hidden | 0 / — / 0 | 0 / 0 / 0 | 0 / 0 / 0 | 0 / — / 0 | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| contract.scroll | 0 / — / 1 | 0 / 0 / 0 | 0 / 0 / 0 | 0 / — / 0 | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| contract.truncated_hidden | 30 / — / 26 | 30 / 30 / 26 | 12 / 12 / 12 | 12 / — / 12 | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| cue_churn.changes | 0 / — / 1 | 0 / 0 / 1 | 0 / 0 / 1 | 0 / — / 1 | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| cue_churn.flip_flops | 0 / — / 0 | 0 / 0 / 0 | 0 / 0 / 0 | 0 / — / 0 | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| glance_cost.median | 1 / — / 1 | 1 / 1 / 1 | 1 / 1 / 1 | 1 / — / 1 | 1 / — / — | 1 / — / — | 1 / — / — | 1 / — / — |
| glance_cost.p95 | 1 / — / 1 | 1 / 1 / 1 | 1 / 1 / 1 | 1 / — / 1 | 3 / — / — | 1 / — / — | 1 / — / — | 1 / — / — |
| layout_shift.per_minute | 2.3 / — / 3.06 | 2.56 / 2.56 / 3.41 | 3.82 / 3.82 / 3.03 | 2.76 / — / 3.38 | 4.72 / — / — | 4.83 / — / — | 4.94 / — / — | 4.44 / — / — |
| phone.axe_serious | — | — | — | — | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| phone.covered | — | — | — | — | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| phone.hidden | — | — | — | — | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| phone.hscroll_px | — | — | — | — | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| phone.kpi_reach | — | — | — | — | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| phone.scroll | — | — | — | — | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| phone.small_body_text | — | — | — | — | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| phone.small_targets | — | — | — | — | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| phone.small_text | — | — | — | — | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| phone.truncated_hidden | — | — | — | — | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| phone.wide | — | — | — | — | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| reading_load.max | 122 / — / 122 | 122 / 122 / 122 | 133 / 133 / 133 | 137 / — / 137 | 126 / — / — | 126 / — / — | 126 / — / — | 126 / — / — |
| reading_load.median | 85 / — / 99 | 85 / 85 / 99 | 96 / 96 / 110 | 100 / — / 114 | 89 / — / — | 89 / — / — | 89 / — / — | 89 / — / — |
| screen_only.actions | 0 / — / 0 | 0 / 0 / 0 | 0 / 0 / 0 | 0 / — / 0 | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| screen_only.looks | 4 / — / 4 | 4 / 4 / 4 | 4 / 4 / 4 | 4 / — / 4 | 6 / — / — | 4 / — / — | 4 / — / — | 4 / — / — |
| screen_only.steers | 0 / — / 0 | 0 / 0 / 0 | 0 / 0 / 0 | 0 / — / 0 | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| screen_only.time_to_act_steps.max | 0 / — / 0 | 0 / 0 / 0 | 0 / 0 / 0 | 0 / — / 0 | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |

### decider

| metric | 1280x720 | 1366x768 | 1600x1000 | 1920x1080 | 360x800 | 390x844 | 414x896 | 768x1024 |
|---|---|---|---|---|---|---|---|---|
| actions_per_fix.max | 0 / — / 0 | 0 / 0 / 0 | 0 / 0 / 0 | 0 / — / 0 | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| actions_per_fix.median | 0 / — / 0 | 0 / 0 / 0 | 0 / 0 / 0 | 0 / — / 0 | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| completion.false | 0 / — / 0 | 0 / 0 / 0 | 0 / 0 / 0 | 0 / — / 0 | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| completion.miss | 0 / — / 0 | 0 / 0 / 0 | 0 / 0 / 0 | 0 / — / 0 | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| contract.axe_serious | 0 / — / 0 | 0 / 0 / 0 | 0 / 0 / 0 | 0 / — / 0 | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| contract.hidden | 0 / — / 0 | 0 / 0 / 0 | 0 / 0 / 0 | 0 / — / 0 | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| contract.scroll | 0 / — / 0 | 0 / 0 / 0 | 0 / 0 / 0 | 0 / — / 0 | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| contract.truncated_hidden | 12 / — / 9 | 12 / 12 / 9 | 5 / 5 / 5 | 4 / — / 4 | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| cue_churn.changes | 0 / — / 0 | 0 / 0 / 0 | 0 / 0 / 0 | 0 / — / 0 | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| cue_churn.flip_flops | 0 / — / 0 | 0 / 0 / 0 | 0 / 0 / 0 | 0 / — / 0 | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| glance_cost.median | 1 / — / 1 | 1 / 1 / 1 | 1 / 1 / 1 | 1 / — / 1 | 1 / — / — | 1 / — / — | 1 / — / — | 1 / — / — |
| glance_cost.p95 | 1 / — / 1 | 1 / 1 / 1 | 1 / 1 / 1 | 1 / — / 1 | 1 / — / — | 1 / — / — | 1 / — / — | 1 / — / — |
| layout_shift.per_minute | 0.04 / — / 0.02 | 0.04 / 0.03 / 0.02 | 0.02 / 0.01 / 0.01 | 0.02 / — / 0.01 | 0.78 / — / — | 1.7 / — / — | 0.66 / — / — | 2.05 / — / — |
| phone.axe_serious | — | — | — | — | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| phone.covered | — | — | — | — | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| phone.hidden | — | — | — | — | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| phone.hscroll_px | — | — | — | — | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| phone.kpi_reach | — | — | — | — | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| phone.scroll | — | — | — | — | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| phone.small_body_text | — | — | — | — | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| phone.small_targets | — | — | — | — | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| phone.small_text | — | — | — | — | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| phone.truncated_hidden | — | — | — | — | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| phone.wide | — | — | — | — | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| reading_load.max | 105 / — / 96 | 105 / 105 / 96 | 113 / 108 / 108 | 113 / — / 108 | 105 / — / — | 105 / — / — | 105 / — / — | 105 / — / — |
| reading_load.median | 105 / — / 96 | 105 / 105 / 96 | 113 / 108 / 108 | 113 / — / 108 | 105 / — / — | 105 / — / — | 105 / — / — | 105 / — / — |
| screen_only.actions | 0 / — / 0 | 0 / 0 / 0 | 0 / 0 / 0 | 0 / — / 0 | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| screen_only.looks | 4 / — / 4 | 4 / 4 / 4 | 4 / 4 / 4 | 4 / — / 4 | 4 / — / — | 4 / — / — | 4 / — / — | 4 / — / — |
| screen_only.steers | 0 / — / 0 | 0 / 0 / 0 | 0 / 0 / 0 | 0 / — / 0 | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| screen_only.time_to_act_steps.max | 1 / — / 0 | 1 / 1 / 0 | 1 / 1 / 0 | 1 / — / 0 | 1 / — / — | 1 / — / — | 1 / — / — | 1 / — / — |

### closed

| metric | 1280x720 | 1366x768 | 1600x1000 | 1920x1080 | 360x800 | 390x844 | 414x896 | 768x1024 |
|---|---|---|---|---|---|---|---|---|
| actions_per_fix.max | 0 / — / 0 | 0 / 0 / 0 | 0 / 0 / 0 | 0 / — / 0 | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| actions_per_fix.median | 0 / — / 0 | 0 / 0 / 0 | 0 / 0 / 0 | 0 / — / 0 | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| completion.false | 0 / — / 0 | 0 / 0 / 0 | 0 / 0 / 0 | 0 / — / 0 | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| completion.miss | 0 / — / 0 | 0 / 0 / 0 | 0 / 0 / 0 | 0 / — / 0 | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| contract.axe_serious | 0 / — / 0 | 0 / 0 / 0 | 0 / 0 / 0 | 0 / — / 0 | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| contract.hidden | 0 / — / 0 | 0 / 0 / 0 | 0 / 0 / 0 | 0 / — / 0 | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| contract.scroll | 0 / — / 0 | 0 / 0 / 0 | 0 / 0 / 0 | 0 / — / 0 | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| contract.truncated_hidden | 41 / — / 36 | 41 / 41 / 36 | 18 / 18 / 15 | 18 / — / 15 | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| cue_churn.changes | 0 / — / 0 | 0 / 0 / 0 | 0 / 0 / 0 | 0 / — / 0 | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| cue_churn.flip_flops | 0 / — / 0 | 0 / 0 / 0 | 0 / 0 / 0 | 0 / — / 0 | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| glance_cost.median | 1.5 / — / 1.5 | 1.5 / 1.5 / 1.5 | 1.5 / 1.5 / 1.5 | 1.5 / — / 1.5 | 1.5 / — / — | 1.5 / — / — | 1.5 / — / — | 1.5 / — / — |
| glance_cost.p95 | 2 / — / 2 | 2 / 2 / 2 | 2 / 2 / 2 | 2 / — / 2 | 2 / — / — | 2 / — / — | 2 / — / — | 2 / — / — |
| layout_shift.per_minute | 0 / — / 0.01 | 0 / 0.01 / 0.01 | 0 / 0.01 / 0.01 | 0 / — / 0 | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| phone.axe_serious | — | — | — | — | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| phone.covered | — | — | — | — | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| phone.hidden | — | — | — | — | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| phone.hscroll_px | — | — | — | — | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| phone.kpi_reach | — | — | — | — | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| phone.scroll | — | — | — | — | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| phone.small_body_text | — | — | — | — | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| phone.small_targets | — | — | — | — | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| phone.small_text | — | — | — | — | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| phone.truncated_hidden | — | — | — | — | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| phone.wide | — | — | — | — | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| reading_load.max | 39 / — / 39 | 39 / 39 / 39 | 47 / 47 / 47 | 47 / — / 47 | 39 / — / — | 39 / — / — | 39 / — / — | 39 / — / — |
| reading_load.median | 39 / — / 39 | 39 / 39 / 39 | 47 / 47 / 47 | 47 / — / 47 | 39 / — / — | 39 / — / — | 39 / — / — | 39 / — / — |
| screen_only.actions | 0 / — / 0 | 0 / 0 / 0 | 0 / 0 / 0 | 0 / — / 0 | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| screen_only.looks | 3 / — / 3 | 3 / 3 / 3 | 3 / 3 / 3 | 3 / — / 3 | 3 / — / — | 3 / — / — | 3 / — / — | 3 / — / — |
| screen_only.steers | 0 / — / 0 | 0 / 0 / 0 | 0 / 0 / 0 | 0 / — / 0 | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| screen_only.time_to_act_steps.max | 0 / — / 0 | 0 / 0 / 0 | 0 / 0 / 0 | 0 / — / 0 | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |

### changes

| metric | 1280x720 | 1366x768 | 1600x1000 | 1920x1080 | 360x800 | 390x844 | 414x896 | 768x1024 |
|---|---|---|---|---|---|---|---|---|
| actions_per_fix.max | 3 / — / 3 | 1 / 1 / 1 | 1 / 1 / 1 | 1 / — / 1 | 1 / — / — | 1 / — / — | 1 / — / — | 1 / — / — |
| actions_per_fix.median | 2 / — / 2 | 1 / 1 / 1 | 1 / 1 / 1 | 1 / — / 1 | 1 / — / — | 1 / — / — | 1 / — / — | 1 / — / — |
| completion.false | 0 / — / 0 | 0 / 0 / 0 | 0 / 0 / 0 | 0 / — / 0 | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| completion.miss | 0 / — / 0 | 0 / 0 / 0 | 0 / 0 / 0 | 0 / — / 0 | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| contract.axe_serious | 0 / — / 0 | 0 / 0 / 0 | 0 / 0 / 0 | 0 / — / 0 | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| contract.hidden | 0 / — / 0 | 0 / 0 / 0 | 0 / 0 / 0 | 0 / — / 0 | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| contract.scroll | 0 / — / 0 | 0 / 0 / 0 | 0 / 0 / 0 | 0 / — / 0 | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| contract.truncated_hidden | 16 / — / 18 | 16 / 16 / 18 | 6 / 6 / 6 | 5 / — / 5 | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| cue_churn.changes | 0 / — / 0 | 0 / 0 / 0 | 0 / 0 / 0 | 0 / — / 0 | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| cue_churn.flip_flops | 0 / — / 0 | 0 / 0 / 0 | 0 / 0 / 0 | 0 / — / 0 | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| glance_cost.median | 1 / — / 1 | 1 / 1 / 1 | 1 / 1 / 1 | 1 / — / 1 | 1 / — / — | 1 / — / — | 1 / — / — | 1 / — / — |
| glance_cost.p95 | 3 / — / 3 | 2 / 2 / 2 | 2 / 2 / 2 | 2 / — / 2 | 3 / — / — | 3 / — / — | 3 / — / — | 3 / — / — |
| layout_shift.per_minute | 0 / — / 0.01 | 0 / 0 / 0.01 | 0 / 0.01 / 0.01 | 0 / — / 0 | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| phone.axe_serious | — | — | — | — | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| phone.covered | — | — | — | — | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| phone.hidden | — | — | — | — | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| phone.hscroll_px | — | — | — | — | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| phone.kpi_reach | — | — | — | — | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| phone.scroll | — | — | — | — | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| phone.sheet_diff | — | — | — | — | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| phone.small_body_text | — | — | — | — | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| phone.small_targets | — | — | — | — | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| phone.small_text | — | — | — | — | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| phone.truncated_hidden | — | — | — | — | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| phone.wide | — | — | — | — | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| reading_load.max | 91 / — / 84 | 91 / 91 / 84 | 101 / 101 / 94 | 105 / — / 98 | 95 / — / — | 95 / — / — | 95 / — / — | 95 / — / — |
| reading_load.median | 91 / — / 84 | 91 / 91 / 84 | 101 / 101 / 94 | 105 / — / 98 | 95 / — / — | 95 / — / — | 95 / — / — | 95 / — / — |
| screen_only.actions | 2 / — / 2 | 3 / 3 / 3 | 3 / 3 / 3 | 3 / — / 3 | 3 / — / — | 3 / — / — | 3 / — / — | 3 / — / — |
| screen_only.looks | 11 / — / 11 | 11 / 11 / 11 | 11 / 11 / 11 | 11 / — / 11 | 12 / — / — | 12 / — / — | 12 / — / — | 12 / — / — |
| screen_only.steers | 0 / — / 0 | 0 / 0 / 0 | 0 / 0 / 0 | 0 / — / 0 | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| screen_only.time_to_act_steps.max | 0 / — / 0 | 0 / 0 / 0 | 0 / 0 / 0 | 0 / — / 0 | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |

### engine

| metric | 1280x720 | 1366x768 | 1600x1000 | 1920x1080 | 360x800 | 390x844 | 414x896 | 768x1024 |
|---|---|---|---|---|---|---|---|---|
| cue.ahead | — | — | 0 / 0 / 0 | — | — | — | — | — |
| cue.flip_flops | — | — | 0 / 0 / 0 | — | — | — | — | — |
| cue.inaccuracy_pct | — | — | 0 / 0 / 0 | — | — | — | — | — |
| cue.missing | — | — | 0 / 0 / 0 | — | — | — | — | — |

### trial-2-train

| metric | 1280x720 | 1366x768 | 1600x1000 | 1920x1080 | 360x800 | 390x844 | 414x896 | 768x1024 |
|---|---|---|---|---|---|---|---|---|
| phone.axe_serious | — | — | — | — | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| phone.covered | — | — | — | — | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| phone.graph | — | — | — | — | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| phone.hidden | — | — | — | — | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| phone.hscroll_px | — | — | — | — | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| phone.kpi_reach | — | — | — | — | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| phone.scroll | — | — | — | — | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| phone.sheet_drawer | — | — | — | — | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| phone.small_body_text | — | — | — | — | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| phone.small_targets | — | — | — | — | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| phone.small_text | — | — | — | — | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| phone.truncated_hidden | — | — | — | — | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| phone.wide | — | — | — | — | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |

### trial-3-haiku

| metric | 1280x720 | 1366x768 | 1600x1000 | 1920x1080 | 360x800 | 390x844 | 414x896 | 768x1024 |
|---|---|---|---|---|---|---|---|---|
| phone.axe_serious | — | — | — | — | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| phone.covered | — | — | — | — | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| phone.hidden | — | — | — | — | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| phone.hscroll_px | — | — | — | — | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| phone.kpi_reach | — | — | — | — | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| phone.scroll | — | — | — | — | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| phone.sheet_drawer | — | — | — | — | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| phone.small_body_text | — | — | — | — | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| phone.small_targets | — | — | — | — | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| phone.small_text | — | — | — | — | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| phone.truncated_hidden | — | — | — | — | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| phone.wide | — | — | — | — | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |

### trial-todo-1

| metric | 1280x720 | 1366x768 | 1600x1000 | 1920x1080 | 360x800 | 390x844 | 414x896 | 768x1024 |
|---|---|---|---|---|---|---|---|---|
| phone.axe_serious | — | — | — | — | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| phone.covered | — | — | — | — | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| phone.graph | — | — | — | — | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| phone.hidden | — | — | — | — | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| phone.hscroll_px | — | — | — | — | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| phone.kpi_reach | — | — | — | — | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| phone.scroll | — | — | — | — | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| phone.sheet_rail | — | — | — | — | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| phone.small_body_text | — | — | — | — | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| phone.small_targets | — | — | — | — | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| phone.small_text | — | — | — | — | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| phone.truncated_hidden | — | — | — | — | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| phone.wide | — | — | — | — | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |

## Worst moments

- contract.truncated_hidden: 41 in **closed** at 1280x720 · ![](shots/1280x720-closed-0.png)
- layout_shift.per_minute: 4.94 in **two** at 414x896 · ![](shots/414x896-two-0.png)
- reading_load.max: 137 in **two** at 1920x1080 · ![](shots/1920x1080-two-0.png)
- glance_cost.p95: 3 in **person** at 360x800 · ![](shots/360x800-person-0.png)
- screen_only.actions: 3 in **changes** at 1366x768 · ![](shots/1366x768-changes-0.png)

## Missing inputs

- No recordings (rrweb is not set up; the driver bot plays each run, and the screenshots stand in for the contact sheet: `contact.html`).
- No questionnaire, no L4 keyed replay, no L5 probe, no L6 live round, no L8 sessions: skipped for this loop (paid or people).

## Screenshots

- shots/1280x720-agent-0.png
- shots/1280x720-agent-diff.png
- shots/1280x720-agent-end.png
- shots/1280x720-changes-0.png
- shots/1280x720-changes-end.png
- shots/1280x720-closed-0.png
- shots/1280x720-closed-end.png
- shots/1280x720-decider-0.png
- shots/1280x720-decider-end.png
- shots/1280x720-person-0.png
- shots/1280x720-person-decided.png
- shots/1280x720-person-end.png
- shots/1280x720-two-0.png
- shots/1280x720-two-end.png
- shots/1366x768-agent-0.png
- shots/1366x768-agent-diff.png
- shots/1366x768-agent-end.png
- shots/1366x768-changes-0.png
- shots/1366x768-changes-diff.png
- shots/1366x768-changes-end.png
- shots/1366x768-closed-0.png
- shots/1366x768-closed-end.png
- shots/1366x768-decider-0.png
- shots/1366x768-decider-end.png
- shots/1366x768-person-0.png
- shots/1366x768-person-decided.png
- shots/1366x768-person-end.png
- shots/1366x768-two-0.png
- shots/1366x768-two-end.png
- shots/1600x1000-agent-0.png
- shots/1600x1000-agent-diff.png
- shots/1600x1000-agent-end.png
- shots/1600x1000-changes-0.png
- shots/1600x1000-changes-diff.png
- shots/1600x1000-changes-end.png
- shots/1600x1000-closed-0.png
- shots/1600x1000-closed-end.png
- shots/1600x1000-decider-0.png
- shots/1600x1000-decider-end.png
- shots/1600x1000-person-0.png
- shots/1600x1000-person-decided.png
- shots/1600x1000-person-end.png
- shots/1600x1000-two-0.png
- shots/1600x1000-two-end.png
- shots/1920x1080-agent-0.png
- shots/1920x1080-agent-diff.png
- shots/1920x1080-agent-end.png
- shots/1920x1080-changes-0.png
- shots/1920x1080-changes-diff.png
- shots/1920x1080-changes-end.png
- shots/1920x1080-closed-0.png
- shots/1920x1080-closed-end.png
- shots/1920x1080-decider-0.png
- shots/1920x1080-decider-end.png
- shots/1920x1080-person-0.png
- shots/1920x1080-person-decided.png
- shots/1920x1080-person-end.png
- shots/1920x1080-two-0.png
- shots/1920x1080-two-end.png
- shots/360x800-agent-0.png
- shots/360x800-agent-diff.png
- shots/360x800-agent-end.png
- shots/360x800-agent-graph.png
- shots/360x800-agent-rail.png
- shots/360x800-agent-truth.png
- shots/360x800-changes-0.png
- shots/360x800-changes-diff.png
- shots/360x800-changes-end.png
- shots/360x800-changes-truth.png
- shots/360x800-closed-0.png
- shots/360x800-closed-end.png
- shots/360x800-closed-truth.png
- shots/360x800-decider-0.png
- shots/360x800-decider-end.png
- shots/360x800-decider-truth.png
- shots/360x800-person-0.png
- shots/360x800-person-decided.png
- shots/360x800-person-drawer.png
- shots/360x800-person-end.png
- shots/360x800-person-truth.png
- shots/360x800-trial-2-train-drawer.png
- shots/360x800-trial-2-train-graph.png
- shots/360x800-trial-2-train-truth.png
- shots/360x800-trial-3-haiku-drawer.png
- shots/360x800-trial-3-haiku-truth.png
- shots/360x800-trial-todo-1-graph.png
- shots/360x800-trial-todo-1-rail.png
- shots/360x800-trial-todo-1-truth.png
- shots/360x800-two-0.png
- shots/360x800-two-end.png
- shots/360x800-two-truth.png
- shots/390x844-agent-0.png
- shots/390x844-agent-diff.png
- shots/390x844-agent-end.png
- shots/390x844-agent-graph.png
- shots/390x844-agent-rail.png
- shots/390x844-agent-truth.png
- shots/390x844-changes-0.png
- shots/390x844-changes-diff.png
- shots/390x844-changes-end.png
- shots/390x844-changes-truth.png
- shots/390x844-closed-0.png
- shots/390x844-closed-end.png
- shots/390x844-closed-truth.png
- shots/390x844-decider-0.png
- shots/390x844-decider-end.png
- shots/390x844-decider-truth.png
- shots/390x844-person-0.png
- shots/390x844-person-decided.png
- shots/390x844-person-drawer.png
- shots/390x844-person-end.png
- shots/390x844-person-truth.png
- shots/390x844-trial-2-train-drawer.png
- shots/390x844-trial-2-train-graph.png
- shots/390x844-trial-2-train-truth.png
- shots/390x844-trial-3-haiku-drawer.png
- shots/390x844-trial-3-haiku-truth.png
- shots/390x844-trial-todo-1-graph.png
- shots/390x844-trial-todo-1-rail.png
- shots/390x844-trial-todo-1-truth.png
- shots/390x844-two-0.png
- shots/390x844-two-end.png
- shots/390x844-two-truth.png
- shots/414x896-agent-0.png
- shots/414x896-agent-diff.png
- shots/414x896-agent-end.png
- shots/414x896-agent-graph.png
- shots/414x896-agent-rail.png
- shots/414x896-agent-truth.png
- shots/414x896-changes-0.png
- shots/414x896-changes-diff.png
- shots/414x896-changes-end.png
- shots/414x896-changes-truth.png
- shots/414x896-closed-0.png
- shots/414x896-closed-end.png
- shots/414x896-closed-truth.png
- shots/414x896-decider-0.png
- shots/414x896-decider-end.png
- shots/414x896-decider-truth.png
- shots/414x896-person-0.png
- shots/414x896-person-decided.png
- shots/414x896-person-drawer.png
- shots/414x896-person-end.png
- shots/414x896-person-truth.png
- shots/414x896-trial-2-train-drawer.png
- shots/414x896-trial-2-train-graph.png
- shots/414x896-trial-2-train-truth.png
- shots/414x896-trial-3-haiku-drawer.png
- shots/414x896-trial-3-haiku-truth.png
- shots/414x896-trial-todo-1-graph.png
- shots/414x896-trial-todo-1-rail.png
- shots/414x896-trial-todo-1-truth.png
- shots/414x896-two-0.png
- shots/414x896-two-end.png
- shots/414x896-two-truth.png
- shots/768x1024-agent-0.png
- shots/768x1024-agent-diff.png
- shots/768x1024-agent-end.png
- shots/768x1024-agent-graph.png
- shots/768x1024-agent-rail.png
- shots/768x1024-agent-truth.png
- shots/768x1024-changes-0.png
- shots/768x1024-changes-diff.png
- shots/768x1024-changes-end.png
- shots/768x1024-changes-truth.png
- shots/768x1024-closed-0.png
- shots/768x1024-closed-end.png
- shots/768x1024-closed-truth.png
- shots/768x1024-decider-0.png
- shots/768x1024-decider-end.png
- shots/768x1024-decider-truth.png
- shots/768x1024-person-0.png
- shots/768x1024-person-decided.png
- shots/768x1024-person-drawer.png
- shots/768x1024-person-end.png
- shots/768x1024-person-truth.png
- shots/768x1024-trial-2-train-drawer.png
- shots/768x1024-trial-2-train-graph.png
- shots/768x1024-trial-2-train-truth.png
- shots/768x1024-trial-3-haiku-drawer.png
- shots/768x1024-trial-3-haiku-truth.png
- shots/768x1024-trial-todo-1-graph.png
- shots/768x1024-trial-todo-1-rail.png
- shots/768x1024-trial-todo-1-truth.png
- shots/768x1024-two-0.png
- shots/768x1024-two-end.png
- shots/768x1024-two-truth.png
- shots/before-360x800-agent-diff.png
- shots/before-360x800-agent-graph.png
- shots/before-360x800-agent-rail.png
- shots/before-360x800-agent-truth.png
- shots/before-360x800-changes-diff.png
- shots/before-360x800-changes-truth.png
- shots/before-360x800-closed-truth.png
- shots/before-360x800-decider-truth.png
- shots/before-360x800-person-drawer.png
- shots/before-360x800-person-truth.png
- shots/before-360x800-trial-2-train-drawer.png
- shots/before-360x800-trial-2-train-graph.png
- shots/before-360x800-trial-2-train-truth.png
- shots/before-360x800-trial-3-haiku-drawer.png
- shots/before-360x800-trial-3-haiku-truth.png
- shots/before-360x800-trial-todo-1-graph.png
- shots/before-360x800-trial-todo-1-rail.png
- shots/before-360x800-trial-todo-1-truth.png
- shots/before-360x800-two-truth.png
- shots/before-390x844-agent-diff.png
- shots/before-390x844-agent-graph.png
- shots/before-390x844-agent-rail.png
- shots/before-390x844-agent-truth.png
- shots/before-390x844-changes-diff.png
- shots/before-390x844-changes-truth.png
- shots/before-390x844-closed-truth.png
- shots/before-390x844-decider-truth.png
- shots/before-390x844-person-drawer.png
- shots/before-390x844-person-truth.png
- shots/before-390x844-trial-2-train-drawer.png
- shots/before-390x844-trial-2-train-graph.png
- shots/before-390x844-trial-2-train-truth.png
- shots/before-390x844-trial-3-haiku-drawer.png
- shots/before-390x844-trial-3-haiku-truth.png
- shots/before-390x844-trial-todo-1-graph.png
- shots/before-390x844-trial-todo-1-rail.png
- shots/before-390x844-trial-todo-1-truth.png
- shots/before-390x844-two-truth.png
- shots/before-414x896-agent-diff.png
- shots/before-414x896-agent-graph.png
- shots/before-414x896-agent-rail.png
- shots/before-414x896-agent-truth.png
- shots/before-414x896-changes-diff.png
- shots/before-414x896-changes-truth.png
- shots/before-414x896-closed-truth.png
- shots/before-414x896-decider-truth.png
- shots/before-414x896-person-drawer.png
- shots/before-414x896-person-truth.png
- shots/before-414x896-trial-2-train-drawer.png
- shots/before-414x896-trial-2-train-graph.png
- shots/before-414x896-trial-2-train-truth.png
- shots/before-414x896-trial-3-haiku-drawer.png
- shots/before-414x896-trial-3-haiku-truth.png
- shots/before-414x896-trial-todo-1-graph.png
- shots/before-414x896-trial-todo-1-rail.png
- shots/before-414x896-trial-todo-1-truth.png
- shots/before-414x896-two-truth.png
- shots/before-768x1024-agent-diff.png
- shots/before-768x1024-agent-graph.png
- shots/before-768x1024-agent-rail.png
- shots/before-768x1024-agent-truth.png
- shots/before-768x1024-changes-diff.png
- shots/before-768x1024-changes-truth.png
- shots/before-768x1024-closed-truth.png
- shots/before-768x1024-decider-truth.png
- shots/before-768x1024-person-drawer.png
- shots/before-768x1024-person-truth.png
- shots/before-768x1024-trial-2-train-drawer.png
- shots/before-768x1024-trial-2-train-graph.png
- shots/before-768x1024-trial-2-train-truth.png
- shots/before-768x1024-trial-3-haiku-drawer.png
- shots/before-768x1024-trial-3-haiku-truth.png
- shots/before-768x1024-trial-todo-1-graph.png
- shots/before-768x1024-trial-todo-1-rail.png
- shots/before-768x1024-trial-todo-1-truth.png
- shots/before-768x1024-two-truth.png
