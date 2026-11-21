# R-02 report

Written by `ux/core/round.mjs` at 2026-11-21T01:49:07.319Z. Suite exit 1. Viewports: 1280x720, 1366x768, 1600x1000, 1920x1080.

## Hard gates at the gating viewports (1600x1000, 1366x768)

Every hard gate passed.

## Screen-only user tasks at the gating viewports

Every task completed.

## Measured-only viewports (not gating)

- **1280x720** · changes: layout contract and the screen-only user: Error: finds the last change as a control it can use expect(received).toBe(expected) // Object.is equality Expected: true Received: false 145 | if (s.truth.lastChange?.cls) expect.soft(await first.getAttribute('class'), `the newest change is ${s.truth.lastChange.cls}`).toContain(`c-${s.truth.lastChange.cls}`); 146 | const ok = !!name && await use('button', name); > 147 | expect.soft(ok, 'finds the last change as a control it can use').toBe(true); | ^ 148 | if (!ok) u.fixes.push(3); 149 | if (ok) { 150 | await expect.soft(page.getByRole('dialog')).toBeVisible(); at ui/ux/specs/loop.spec.mjs:147:72

## Metrics (lower is better)

Each cell: this round / baseline / R-01.

### person

| metric | 1280x720 | 1366x768 | 1600x1000 | 1920x1080 |
|---|---|---|---|---|
| actions_per_fix.max | 2 / — / 1 | 2 / 2 / 1 | 2 / 2 / 1 | 2 / — / 1 |
| actions_per_fix.median | 2 / — / 1 | 2 / 2 / 1 | 2 / 2 / 1 | 2 / — / 1 |
| completion.false | 0 / — / 0 | 0 / 0 / 0 | 0 / 0 / 0 | 0 / — / 0 |
| completion.miss | 0 / — / 0 | 0 / 0 / 0 | 0 / 0 / 0 | 0 / — / 0 |
| contract.axe_serious | 0 / — / 64 | 0 / 0 / 64 | 0 / 0 / 89 | 0 / — / 93 |
| contract.hidden | 0 / — / 0 | 0 / 0 / 0 | 0 / 0 / 0 | 0 / — / 0 |
| contract.scroll | 0 / — / 1 | 0 / 0 / 1 | 0 / 0 / 0 | 0 / — / 0 |
| contract.truncated_hidden | 9 / — / 8 | 8 / 8 / 8 | 4 / 4 / 4 | 4 / — / 4 |
| cue_churn.changes | 0 / — / 0 | 0 / 0 / 0 | 0 / 0 / 0 | 0 / — / 0 |
| cue_churn.flip_flops | 0 / — / 0 | 0 / 0 / 0 | 0 / 0 / 0 | 0 / — / 0 |
| glance_cost.median | 1 / — / 1 | 1 / 1 / 1 | 1 / 1 / 1 | 1 / — / 1 |
| glance_cost.p95 | 2 / — / 1 | 2 / 2 / 1 | 2 / 2 / 1 | 2 / — / 1 |
| keyboard.tabs_to_cue | 9 / — / 9 | 9 / 9 / 9 | 9 / 9 / 9 | 9 / — / 9 |
| keyboard.tabs_to_decide | 0 / — / — | 0 / 0 / — | 0 / 0 / — | 0 / — / — |
| layout_shift.per_minute | 0.01 / — / 0.01 | 0.01 / 0.01 / 0.01 | 0.01 / 0.01 / 0.01 | 0 / — / 0 |
| reading_load.max | 98 / — / 49 | 98 / 98 / 49 | 107 / 107 / 58 | 107 / — / 58 |
| reading_load.median | 98 / — / 49 | 98 / 98 / 49 | 107 / 107 / 58 | 107 / — / 58 |
| screen_only.actions | 2 / — / 1 | 2 / 2 / 1 | 2 / 2 / 1 | 2 / — / 1 |
| screen_only.decision_refused | 1 / — / — | 1 / 1 / — | 1 / 1 / — | 1 / — / — |
| screen_only.looks | 8 / — / 4 | 8 / 8 / 4 | 8 / 8 / 4 | 8 / — / 4 |
| screen_only.steers | 0 / — / 0 | 0 / 0 / 0 | 0 / 0 / 0 | 0 / — / 0 |
| screen_only.time_to_act_steps.max | 0 / — / 0 | 0 / 0 / 0 | 0 / 0 / 0 | 0 / — / 0 |

### agent

| metric | 1280x720 | 1366x768 | 1600x1000 | 1920x1080 |
|---|---|---|---|---|
| actions_per_fix.max | 1 / — / 1 | 1 / 1 / 1 | 1 / 1 / 1 | 1 / — / 1 |
| actions_per_fix.median | 1 / — / 1 | 1 / 1 / 1 | 1 / 1 / 1 | 1 / — / 1 |
| completion.false | 0 / — / 0 | 0 / 0 / 0 | 0 / 0 / 0 | 0 / — / 0 |
| completion.miss | 0 / — / 0 | 0 / 0 / 0 | 0 / 0 / 0 | 0 / — / 0 |
| contract.axe_serious | 0 / — / 61 | 0 / 0 / 63 | 0 / 0 / 87 | 0 / — / 89 |
| contract.hidden | 0 / — / 0 | 0 / 0 / 0 | 0 / 0 / 0 | 0 / — / 0 |
| contract.scroll | 0 / — / 1 | 0 / 0 / 1 | 0 / 0 / 0 | 0 / — / 0 |
| contract.truncated_hidden | 28 / — / 28 | 28 / 28 / 28 | 12 / 12 / 12 | 12 / — / 12 |
| cue_churn.changes | 0 / — / 0 | 0 / 0 / 0 | 0 / 0 / 0 | 0 / — / 0 |
| cue_churn.flip_flops | 0 / — / 0 | 0 / 0 / 0 | 0 / 0 / 0 | 0 / — / 0 |
| glance_cost.median | 1 / — / 1 | 1 / 1 / 1 | 1 / 1 / 1 | 1 / — / 1 |
| glance_cost.p95 | 2 / — / 1 | 2 / 2 / 1 | 2 / 2 / 1 | 2 / — / 1 |
| layout_shift.per_minute | 0.45 / — / 0.5 | 0.07 / 0.07 / 0.04 | 0.06 / 0.06 / 0.06 | 0.05 / — / 0.05 |
| reading_load.max | 78 / — / 78 | 78 / 78 / 78 | 89 / 89 / 89 | 89 / — / 89 |
| reading_load.median | 52 / — / 63 | 76 / 70 / 78 | 63 / 81 / 89 | 63 / — / 81 |
| screen_only.actions | 1 / — / 1 | 1 / 1 / 1 | 1 / 1 / 1 | 1 / — / 1 |
| screen_only.looks | 6 / — / 5 | 6 / 6 / 5 | 6 / 6 / 5 | 6 / — / 5 |
| screen_only.steers | 0 / — / 0 | 0 / 0 / 0 | 0 / 0 / 0 | 0 / — / 0 |
| screen_only.time_to_act_steps.max | 0 / — / 0 | 0 / 0 / 0 | 0 / 0 / 0 | 0 / — / 0 |

### two

| metric | 1280x720 | 1366x768 | 1600x1000 | 1920x1080 |
|---|---|---|---|---|
| actions_per_fix.max | 0 / — / 0 | 0 / 0 / 0 | 0 / 0 / 0 | 0 / — / 0 |
| actions_per_fix.median | 0 / — / 0 | 0 / 0 / 0 | 0 / 0 / 0 | 0 / — / 0 |
| completion.false | 0 / — / 0 | 0 / 0 / 0 | 0 / 0 / 0 | 0 / — / 0 |
| completion.miss | 0 / — / 0 | 0 / 0 / 0 | 0 / 0 / 0 | 0 / — / 0 |
| contract.axe_serious | 0 / — / 65 | 0 / 0 / 67 | 0 / 0 / 89 | 0 / — / 91 |
| contract.hidden | 0 / — / 0 | 0 / 0 / 0 | 0 / 0 / 0 | 0 / — / 0 |
| contract.scroll | 1 / — / 1 | 0 / 0 / 1 | 0 / 0 / 1 | 0 / — / 0 |
| contract.truncated_hidden | 26 / — / 26 | 26 / 26 / 26 | 12 / 12 / 12 | 12 / — / 12 |
| cue_churn.changes | 1 / — / 1 | 1 / 1 / 1 | 1 / 2 / 1 | 1 / — / 1 |
| cue_churn.flip_flops | 0 / — / 0 | 0 / 0 / 0 | 0 / 0 / 0 | 0 / — / 0 |
| glance_cost.median | 1 / — / 1 | 1 / 1 / 1 | 1 / 1 / 1 | 1 / — / 1 |
| glance_cost.p95 | 1 / — / 1 | 1 / 1 / 1 | 1 / 1 / 1 | 1 / — / 1 |
| layout_shift.per_minute | 3.06 / — / 2.87 | 3.41 / 3.61 / 7.25 | 3.03 / 3.81 / 6.64 | 3.38 / — / 3.61 |
| reading_load.max | 122 / — / 133 | 122 / 122 / 132 | 133 / 125 / 142 | 137 / — / 129 |
| reading_load.median | 99 / — / 75 | 99 / 85 / 61 | 110 / 84 / 73 | 114 / — / 86 |
| screen_only.actions | 0 / — / 0 | 0 / 0 / 0 | 0 / 0 / 0 | 0 / — / 0 |
| screen_only.looks | 4 / — / 4 | 4 / 4 / 4 | 4 / 4 / 4 | 4 / — / 4 |
| screen_only.steers | 0 / — / 0 | 0 / 0 / 0 | 0 / 0 / 0 | 0 / — / 0 |
| screen_only.time_to_act_steps.max | 0 / — / 0 | 0 / 0 / 0 | 0 / 0 / 0 | 0 / — / 0 |

### decider

| metric | 1280x720 | 1366x768 | 1600x1000 | 1920x1080 |
|---|---|---|---|---|
| actions_per_fix.max | 0 / — / 0 | 0 / 0 / 0 | 0 / 0 / 0 | 0 / — / 0 |
| actions_per_fix.median | 0 / — / 0 | 0 / 0 / 0 | 0 / 0 / 0 | 0 / — / 0 |
| completion.false | 0 / — / 0 | 0 / 0 / 0 | 0 / 0 / 0 | 0 / — / 0 |
| completion.miss | 0 / — / 0 | 0 / 0 / 0 | 0 / 0 / 0 | 0 / — / 0 |
| contract.axe_serious | 0 / — / 62 | 0 / 0 / 63 | 0 / 0 / 90 | 0 / — / 92 |
| contract.hidden | 0 / — / 0 | 0 / 0 / 0 | 0 / 0 / 0 | 0 / — / 0 |
| contract.scroll | 0 / — / 1 | 0 / 0 / 1 | 0 / 0 / 0 | 0 / — / 0 |
| contract.truncated_hidden | 9 / — / 10 | 9 / 9 / 10 | 5 / 5 / 5 | 4 / — / 4 |
| cue_churn.changes | 0 / — / 0 | 0 / 0 / 0 | 0 / 0 / 0 | 0 / — / 0 |
| cue_churn.flip_flops | 0 / — / 0 | 0 / 0 / 0 | 0 / 0 / 0 | 0 / — / 0 |
| glance_cost.median | 1 / — / 1 | 1 / 1 / 1 | 1 / 1 / 1 | 1 / — / 1 |
| glance_cost.p95 | 1 / — / 1 | 1 / 1 / 1 | 1 / 1 / 1 | 1 / — / 1 |
| layout_shift.per_minute | 0.02 / — / 0.33 | 0.02 / 0.03 / 0.29 | 0.01 / 0.03 / 0.33 | 0.01 / — / 0.3 |
| reading_load.max | 96 / — / 69 | 96 / 96 / 69 | 108 / 108 / 77 | 108 / — / 81 |
| reading_load.median | 96 / — / 59 | 96 / 96 / 59 | 108 / 108 / 71 | 108 / — / 71 |
| screen_only.actions | 0 / — / 0 | 0 / 0 / 0 | 0 / 0 / 0 | 0 / — / 0 |
| screen_only.looks | 4 / — / 2 | 4 / 4 / 2 | 4 / 4 / 3 | 4 / — / 3 |
| screen_only.steers | 0 / — / 0 | 0 / 0 / 0 | 0 / 0 / 0 | 0 / — / 0 |
| screen_only.time_to_act_steps.max | 0 / — / 0 | 0 / 0 / 0 | 0 / 0 / 0 | 0 / — / 0 |

### closed

| metric | 1280x720 | 1366x768 | 1600x1000 | 1920x1080 |
|---|---|---|---|---|
| actions_per_fix.max | 0 / — / 0 | 0 / 0 / 0 | 0 / 0 / 0 | 0 / — / 0 |
| actions_per_fix.median | 0 / — / 0 | 0 / 0 / 0 | 0 / 0 / 0 | 0 / — / 0 |
| completion.false | 0 / — / 0 | 0 / 0 / 0 | 0 / 0 / 0 | 0 / — / 0 |
| completion.miss | 0 / — / 0 | 0 / 0 / 0 | 0 / 0 / 0 | 0 / — / 0 |
| contract.axe_serious | 0 / — / 62 | 0 / 0 / 63 | 0 / 0 / 90 | 0 / — / 92 |
| contract.hidden | 0 / — / 0 | 0 / 0 / 0 | 0 / 0 / 0 | 0 / — / 0 |
| contract.scroll | 0 / — / 1 | 0 / 0 / 1 | 0 / 0 / 0 | 0 / — / 0 |
| contract.truncated_hidden | 36 / — / 36 | 36 / 36 / 36 | 15 / 15 / 15 | 15 / — / 15 |
| cue_churn.changes | 0 / — / 0 | 0 / 0 / 0 | 0 / 0 / 0 | 0 / — / 0 |
| cue_churn.flip_flops | 0 / — / 0 | 0 / 0 / 0 | 0 / 0 / 0 | 0 / — / 0 |
| glance_cost.median | 1.5 / — / 1.5 | 1.5 / 1.5 / 1.5 | 1.5 / 1.5 / 1.5 | 1.5 / — / 1.5 |
| glance_cost.p95 | 2 / — / 2 | 2 / 2 / 2 | 2 / 2 / 2 | 2 / — / 2 |
| layout_shift.per_minute | 0.01 / — / 0.01 | 0.01 / 0.01 / 0.01 | 0.01 / 0.01 / 0.01 | 0 / — / 0 |
| reading_load.max | 39 / — / 40 | 39 / 39 / 40 | 47 / 47 / 48 | 47 / — / 48 |
| reading_load.median | 39 / — / 40 | 39 / 39 / 40 | 47 / 47 / 48 | 47 / — / 48 |
| screen_only.actions | 0 / — / 0 | 0 / 0 / 0 | 0 / 0 / 0 | 0 / — / 0 |
| screen_only.looks | 3 / — / 3 | 3 / 3 / 3 | 3 / 3 / 3 | 3 / — / 3 |
| screen_only.steers | 0 / — / 0 | 0 / 0 / 0 | 0 / 0 / 0 | 0 / — / 0 |
| screen_only.time_to_act_steps.max | 0 / — / 0 | 0 / 0 / 0 | 0 / 0 / 0 | 0 / — / 0 |

### changes

| metric | 1280x720 | 1366x768 | 1600x1000 | 1920x1080 |
|---|---|---|---|---|
| actions_per_fix.max | 3 / — / 1 | 1 / 1 / 1 | 1 / 1 / 1 | 1 / — / 1 |
| actions_per_fix.median | 2 / — / 1 | 1 / 1 / 1 | 1 / 1 / 1 | 1 / — / 1 |
| completion.false | 0 / — / 0 | 0 / 0 / 0 | 0 / 0 / 0 | 0 / — / 0 |
| completion.miss | 0 / — / 0 | 0 / 0 / 0 | 0 / 0 / 0 | 0 / — / 0 |
| contract.axe_serious | 0 / — / 58 | 0 / 0 / 60 | 0 / 0 / 87 | 0 / — / 89 |
| contract.hidden | 0 / — / 0 | 0 / 0 / 0 | 0 / 0 / 0 | 0 / — / 0 |
| contract.scroll | 0 / — / 1 | 0 / 0 / 1 | 0 / 0 / 0 | 0 / — / 0 |
| contract.truncated_hidden | 18 / — / 16 | 18 / 18 / 16 | 6 / 6 / 5 | 5 / — / 5 |
| cue_churn.changes | 0 / — / 0 | 0 / 0 / 0 | 0 / 0 / 0 | 0 / — / 0 |
| cue_churn.flip_flops | 0 / — / 0 | 0 / 0 / 0 | 0 / 0 / 0 | 0 / — / 0 |
| glance_cost.median | 1 / — / 1 | 1 / 1 / 1 | 1 / 1 / 1 | 1 / — / 1 |
| glance_cost.p95 | 3 / — / 2 | 2 / 2 / 2 | 2 / 2 / 2 | 2 / — / 2 |
| layout_shift.per_minute | 0.01 / — / 0.01 | 0.01 / 0.01 / 0.01 | 0.01 / 0.01 / 0.01 | 0 / — / 0 |
| reading_load.max | 84 / — / 84 | 84 / 84 / 84 | 94 / 94 / 94 | 98 / — / 98 |
| reading_load.median | 84 / — / 84 | 84 / 84 / 84 | 94 / 94 / 94 | 98 / — / 98 |
| screen_only.actions | 2 / — / 2 | 3 / 3 / 2 | 3 / 3 / 2 | 3 / — / 2 |
| screen_only.looks | 11 / — / 8 | 11 / 11 / 8 | 11 / 11 / 8 | 11 / — / 8 |
| screen_only.steers | 0 / — / 0 | 0 / 0 / 0 | 0 / 0 / 0 | 0 / — / 0 |
| screen_only.time_to_act_steps.max | 0 / — / 0 | 0 / 0 / 0 | 0 / 0 / 0 | 0 / — / 0 |

### engine

| metric | 1280x720 | 1366x768 | 1600x1000 | 1920x1080 |
|---|---|---|---|---|
| cue.ahead | — | — | 0 / 0 / 0 | — |
| cue.flip_flops | — | — | 0 / 0 / 0 | — |
| cue.inaccuracy_pct | — | — | 0 / 0 / 0 | — |
| cue.missing | — | — | 0 / 0 / 0 | — |

## Worst moments

- contract.scroll: 1 in **two** at 1280x720 · ![](shots/1280x720-two-0.png)
- contract.truncated_hidden: 36 in **closed** at 1280x720 · ![](shots/1280x720-closed-0.png)
- layout_shift.per_minute: 3.41 in **two** at 1366x768 · ![](shots/1366x768-two-0.png)
- reading_load.max: 137 in **two** at 1920x1080 · ![](shots/1920x1080-two-0.png)
- glance_cost.p95: 3 in **changes** at 1280x720 · ![](shots/1280x720-changes-0.png)
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
