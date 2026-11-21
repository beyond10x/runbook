# R-01 report

Written by `ux/core/round.mjs` at 2026-11-21T01:21:25.020Z. Suite exit 1. Viewports: 1280x720, 1366x768, 1600x1000, 1920x1080.

## Gates

- **1600x1000** · agent: layout contract and the screen-only user: Error: hard gate 5: axe serious or critical expect(received).toEqual(expected) // deep equality - Expected - 1 + Received + 23
- **1366x768** · agent: layout contract and the screen-only user: Error: hard gate 1: nothing scrolls but what the baseline allows expect(received).toEqual(expected) // deep equality - Expected - 1 + Received + 3
- **1366x768** · agent: layout contract and the screen-only user: Error: hard gate 5: axe serious or critical expect(received).toEqual(expected) // deep equality - Expected - 1 + Received + 23
- **1600x1000** · two: layout contract and the screen-only user: Error: hard gate 1: nothing scrolls but what the baseline allows expect(received).toEqual(expected) // deep equality - Expected - 1 + Received + 3
- **1600x1000** · two: layout contract and the screen-only user: Error: hard gate 5: axe serious or critical expect(received).toEqual(expected) // deep equality - Expected - 1 + Received + 23
- **1366x768** · two: layout contract and the screen-only user: Error: hard gate 1: nothing scrolls but what the baseline allows expect(received).toEqual(expected) // deep equality - Expected - 1 + Received + 3
- **1366x768** · two: layout contract and the screen-only user: Error: hard gate 5: axe serious or critical expect(received).toEqual(expected) // deep equality - Expected - 1 + Received + 23
- **1600x1000** · decider: layout contract and the screen-only user: Error: hard gate 5: axe serious or critical expect(received).toEqual(expected) // deep equality - Expected - 1 + Received + 22
- **1366x768** · decider: layout contract and the screen-only user: Error: sees claude-haiku-5-5 expect(received).toBe(expected) // Object.is equality Expected: true Received: false
- **1366x768** · decider: layout contract and the screen-only user: Error: hard gate 1: nothing scrolls but what the baseline allows expect(received).toEqual(expected) // deep equality - Expected - 1 + Received + 3
- **1366x768** · decider: layout contract and the screen-only user: Error: hard gate 5: axe serious or critical expect(received).toEqual(expected) // deep equality - Expected - 1 + Received + 23
- **1280x720** · decider: layout contract and the screen-only user: Error: sees claude-haiku-5-5 expect(received).toBe(expected) // Object.is equality Expected: true Received: false
- **1600x1000** · person: layout contract and the screen-only user: Error: hard gate 5: axe serious or critical expect(received).toEqual(expected) // deep equality - Expected - 1 + Received + 22
- **1600x1000** · person: layout contract and the screen-only user: Error: the approve control is in view expect(received).toBe(expected) // Object.is equality Expected: true Received: false
- **1366x768** · person: layout contract and the screen-only user: Error: hard gate 1: nothing scrolls but what the baseline allows expect(received).toEqual(expected) // deep equality - Expected - 1 + Received + 3
- **1366x768** · person: layout contract and the screen-only user: Error: hard gate 5: axe serious or critical expect(received).toEqual(expected) // deep equality - Expected - 1 + Received + 22
- **1366x768** · person: layout contract and the screen-only user: Error: the approve control is in view expect(received).toBe(expected) // Object.is equality Expected: true Received: false
- **1280x720** · person: layout contract and the screen-only user: Error: the approve control is in view expect(received).toBe(expected) // Object.is equality Expected: true Received: false
- **1920x1080** · person: layout contract and the screen-only user: Error: the approve control is in view expect(received).toBe(expected) // Object.is equality Expected: true Received: false
- **1600x1000** · closed: layout contract and the screen-only user: Error: hard gate 5: axe serious or critical expect(received).toEqual(expected) // deep equality - Expected - 1 + Received + 23
- **1366x768** · closed: layout contract and the screen-only user: Error: hard gate 1: nothing scrolls but what the baseline allows expect(received).toEqual(expected) // deep equality - Expected - 1 + Received + 3
- **1366x768** · closed: layout contract and the screen-only user: Error: hard gate 5: axe serious or critical expect(received).toEqual(expected) // deep equality - Expected - 1 + Received + 23
- **1600x1000** · changes: layout contract and the screen-only user: Error: hard gate 5: axe serious or critical expect(received).toEqual(expected) // deep equality - Expected - 1 + Received + 23
- **1600x1000** · changes: layout contract and the screen-only user: Error: finds the last change as a control it can use expect(received).toBe(expected) // Object.is equality Expected: true Received: false
- **1366x768** · changes: layout contract and the screen-only user: Error: hard gate 1: nothing scrolls but what the baseline allows expect(received).toEqual(expected) // deep equality - Expected - 1 + Received + 3
- **1366x768** · changes: layout contract and the screen-only user: Error: hard gate 5: axe serious or critical expect(received).toEqual(expected) // deep equality - Expected - 1 + Received + 23
- **1366x768** · changes: layout contract and the screen-only user: Error: finds the last change as a control it can use expect(received).toBe(expected) // Object.is equality Expected: true Received: false
- **1280x720** · changes: layout contract and the screen-only user: Error: finds the last change as a control it can use expect(received).toBe(expected) // Object.is equality Expected: true Received: false
- **1920x1080** · changes: layout contract and the screen-only user: Error: finds the last change as a control it can use expect(received).toBe(expected) // Object.is equality Expected: true Received: false

## Metrics (lower is better)

Each cell: this round / baseline / no previous round.

### person

| metric | 1280x720 | 1366x768 | 1600x1000 | 1920x1080 |
|---|---|---|---|---|
| actions_per_fix.max | 1 / — / — | 1 / — / — | 1 / — / — | 1 / — / — |
| actions_per_fix.median | 1 / — / — | 1 / — / — | 1 / — / — | 1 / — / — |
| completion.false | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| completion.miss | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| contract.axe_serious | 64 / — / — | 64 / — / — | 89 / — / — | 93 / — / — |
| contract.hidden | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| contract.scroll | 1 / — / — | 1 / — / — | 0 / — / — | 0 / — / — |
| contract.truncated_hidden | 8 / — / — | 8 / — / — | 4 / — / — | 4 / — / — |
| cue_churn.changes | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| cue_churn.flip_flops | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| glance_cost.median | 1 / — / — | 1 / — / — | 1 / — / — | 1 / — / — |
| glance_cost.p95 | 1 / — / — | 1 / — / — | 1 / — / — | 1 / — / — |
| keyboard.tabs_to_cue | 9 / — / — | 9 / — / — | 9 / — / — | 9 / — / — |
| layout_shift.per_minute | 0.01 / — / — | 0.01 / — / — | 0.01 / — / — | 0 / — / — |
| reading_load.max | 49 / — / — | 49 / — / — | 58 / — / — | 58 / — / — |
| reading_load.median | 49 / — / — | 49 / — / — | 58 / — / — | 58 / — / — |
| screen_only.actions | 1 / — / — | 1 / — / — | 1 / — / — | 1 / — / — |
| screen_only.looks | 4 / — / — | 4 / — / — | 4 / — / — | 4 / — / — |
| screen_only.steers | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| screen_only.time_to_act_steps.max | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |

### agent

| metric | 1280x720 | 1366x768 | 1600x1000 | 1920x1080 |
|---|---|---|---|---|
| actions_per_fix.max | 1 / — / — | 1 / — / — | 1 / — / — | 1 / — / — |
| actions_per_fix.median | 1 / — / — | 1 / — / — | 1 / — / — | 1 / — / — |
| completion.false | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| completion.miss | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| contract.axe_serious | 61 / — / — | 63 / — / — | 87 / — / — | 89 / — / — |
| contract.hidden | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| contract.scroll | 1 / — / — | 1 / — / — | 0 / — / — | 0 / — / — |
| contract.truncated_hidden | 28 / — / — | 28 / — / — | 12 / — / — | 12 / — / — |
| cue_churn.changes | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| cue_churn.flip_flops | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| glance_cost.median | 1 / — / — | 1 / — / — | 1 / — / — | 1 / — / — |
| glance_cost.p95 | 1 / — / — | 1 / — / — | 1 / — / — | 1 / — / — |
| layout_shift.per_minute | 0.5 / — / — | 0.04 / — / — | 0.06 / — / — | 0.05 / — / — |
| reading_load.max | 78 / — / — | 78 / — / — | 89 / — / — | 89 / — / — |
| reading_load.median | 63 / — / — | 78 / — / — | 89 / — / — | 81 / — / — |
| screen_only.actions | 1 / — / — | 1 / — / — | 1 / — / — | 1 / — / — |
| screen_only.looks | 5 / — / — | 5 / — / — | 5 / — / — | 5 / — / — |
| screen_only.steers | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| screen_only.time_to_act_steps.max | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |

### two

| metric | 1280x720 | 1366x768 | 1600x1000 | 1920x1080 |
|---|---|---|---|---|
| actions_per_fix.max | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| actions_per_fix.median | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| completion.false | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| completion.miss | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| contract.axe_serious | 65 / — / — | 67 / — / — | 89 / — / — | 91 / — / — |
| contract.hidden | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| contract.scroll | 1 / — / — | 1 / — / — | 1 / — / — | 0 / — / — |
| contract.truncated_hidden | 26 / — / — | 26 / — / — | 12 / — / — | 12 / — / — |
| cue_churn.changes | 1 / — / — | 1 / — / — | 1 / — / — | 1 / — / — |
| cue_churn.flip_flops | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| glance_cost.median | 1 / — / — | 1 / — / — | 1 / — / — | 1 / — / — |
| glance_cost.p95 | 1 / — / — | 1 / — / — | 1 / — / — | 1 / — / — |
| layout_shift.per_minute | 2.87 / — / — | 7.25 / — / — | 6.64 / — / — | 3.61 / — / — |
| reading_load.max | 133 / — / — | 132 / — / — | 142 / — / — | 129 / — / — |
| reading_load.median | 75 / — / — | 61 / — / — | 73 / — / — | 86 / — / — |
| screen_only.actions | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| screen_only.looks | 4 / — / — | 4 / — / — | 4 / — / — | 4 / — / — |
| screen_only.steers | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| screen_only.time_to_act_steps.max | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |

### decider

| metric | 1280x720 | 1366x768 | 1600x1000 | 1920x1080 |
|---|---|---|---|---|
| actions_per_fix.max | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| actions_per_fix.median | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| completion.false | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| completion.miss | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| contract.axe_serious | 62 / — / — | 63 / — / — | 90 / — / — | 92 / — / — |
| contract.hidden | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| contract.scroll | 1 / — / — | 1 / — / — | 0 / — / — | 0 / — / — |
| contract.truncated_hidden | 10 / — / — | 10 / — / — | 5 / — / — | 4 / — / — |
| cue_churn.changes | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| cue_churn.flip_flops | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| glance_cost.median | 1 / — / — | 1 / — / — | 1 / — / — | 1 / — / — |
| glance_cost.p95 | 1 / — / — | 1 / — / — | 1 / — / — | 1 / — / — |
| layout_shift.per_minute | 0.33 / — / — | 0.29 / — / — | 0.33 / — / — | 0.3 / — / — |
| reading_load.max | 69 / — / — | 69 / — / — | 77 / — / — | 81 / — / — |
| reading_load.median | 59 / — / — | 59 / — / — | 71 / — / — | 71 / — / — |
| screen_only.actions | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| screen_only.looks | 2 / — / — | 2 / — / — | 3 / — / — | 3 / — / — |
| screen_only.steers | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| screen_only.time_to_act_steps.max | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |

### closed

| metric | 1280x720 | 1366x768 | 1600x1000 | 1920x1080 |
|---|---|---|---|---|
| actions_per_fix.max | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| actions_per_fix.median | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| completion.false | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| completion.miss | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| contract.axe_serious | 62 / — / — | 63 / — / — | 90 / — / — | 92 / — / — |
| contract.hidden | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| contract.scroll | 1 / — / — | 1 / — / — | 0 / — / — | 0 / — / — |
| contract.truncated_hidden | 36 / — / — | 36 / — / — | 15 / — / — | 15 / — / — |
| cue_churn.changes | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| cue_churn.flip_flops | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| glance_cost.median | 1.5 / — / — | 1.5 / — / — | 1.5 / — / — | 1.5 / — / — |
| glance_cost.p95 | 2 / — / — | 2 / — / — | 2 / — / — | 2 / — / — |
| layout_shift.per_minute | 0.01 / — / — | 0.01 / — / — | 0.01 / — / — | 0 / — / — |
| reading_load.max | 40 / — / — | 40 / — / — | 48 / — / — | 48 / — / — |
| reading_load.median | 40 / — / — | 40 / — / — | 48 / — / — | 48 / — / — |
| screen_only.actions | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| screen_only.looks | 3 / — / — | 3 / — / — | 3 / — / — | 3 / — / — |
| screen_only.steers | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| screen_only.time_to_act_steps.max | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |

### changes

| metric | 1280x720 | 1366x768 | 1600x1000 | 1920x1080 |
|---|---|---|---|---|
| actions_per_fix.max | 1 / — / — | 1 / — / — | 1 / — / — | 1 / — / — |
| actions_per_fix.median | 1 / — / — | 1 / — / — | 1 / — / — | 1 / — / — |
| completion.false | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| completion.miss | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| contract.axe_serious | 58 / — / — | 60 / — / — | 87 / — / — | 89 / — / — |
| contract.hidden | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| contract.scroll | 1 / — / — | 1 / — / — | 0 / — / — | 0 / — / — |
| contract.truncated_hidden | 16 / — / — | 16 / — / — | 5 / — / — | 5 / — / — |
| cue_churn.changes | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| cue_churn.flip_flops | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| glance_cost.median | 1 / — / — | 1 / — / — | 1 / — / — | 1 / — / — |
| glance_cost.p95 | 2 / — / — | 2 / — / — | 2 / — / — | 2 / — / — |
| layout_shift.per_minute | 0.01 / — / — | 0.01 / — / — | 0.01 / — / — | 0 / — / — |
| reading_load.max | 84 / — / — | 84 / — / — | 94 / — / — | 98 / — / — |
| reading_load.median | 84 / — / — | 84 / — / — | 94 / — / — | 98 / — / — |
| screen_only.actions | 2 / — / — | 2 / — / — | 2 / — / — | 2 / — / — |
| screen_only.looks | 8 / — / — | 8 / — / — | 8 / — / — | 8 / — / — |
| screen_only.steers | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |
| screen_only.time_to_act_steps.max | 0 / — / — | 0 / — / — | 0 / — / — | 0 / — / — |

### engine

| metric | 1280x720 | 1366x768 | 1600x1000 | 1920x1080 |
|---|---|---|---|---|
| cue.ahead | — | — | 0 / — / — | — |
| cue.flip_flops | — | — | 0 / — / — | — |
| cue.inaccuracy_pct | — | — | 0 / — / — | — |
| cue.missing | — | — | 0 / — / — | — |

## Worst moments

- contract.scroll: 1 in **person** at 1280x720 · ![](shots/1280x720-person-0.png)
- contract.axe_serious: 93 in **person** at 1920x1080 · ![](shots/1920x1080-person-0.png)
- contract.truncated_hidden: 36 in **closed** at 1280x720 · ![](shots/1280x720-closed-0.png)
- layout_shift.per_minute: 7.25 in **two** at 1366x768 · ![](shots/1366x768-two-0.png)
- reading_load.max: 142 in **two** at 1600x1000 · ![](shots/1600x1000-two-0.png)
- glance_cost.p95: 2 in **closed** at 1280x720 · ![](shots/1280x720-closed-0.png)
- screen_only.actions: 2 in **changes** at 1280x720 · ![](shots/1280x720-changes-0.png)

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
