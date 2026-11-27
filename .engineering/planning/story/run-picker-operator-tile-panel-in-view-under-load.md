---
format: aep.planning-md/3
id: story:run-picker-operator-tile-panel-in-view-under-load
kind: story
status: draft
title: The run picker's panel is in view at 805 px also under load
summary: 'run-picker.spec.mjs:165 saw the panel''s right edge at 814 > 805 once at load 20: the case measures before the fonts settle and the panel is not kept in the viewport'
tags:
- backlog
revision: 1
---
## Context

The UI case `in an 805x1270 desktop window the button fits the header beside Decisions and the Live pill;
the panel is in view (D-008)` (`ui/test/run-picker.spec.mjs:163-182`) opens the run picker and asserts the
panel's right edge is at most 805 (`run-picker.spec.mjs:177`). The panel is `position: absolute; left: 0`
inside `.picker` with `width: min(620px, calc(100vw - 24px))` (`ui/web/style.css:57`, `:87`), so its right
edge is the picker's left edge plus 620 px: it fits at 805 px only while the header content left of the
picker (the brand, `ui/web/index.html:21-22`) is at most about 185 px wide. Nothing keeps the panel inside
the viewport by itself.

The brand's width depends on the font: `Inter` is loaded with `font-display: swap` (`ui/web/style.css:1`).
The case measures after the run name shows (`run-picker.spec.mjs:167`) and after `open`
(`run-picker.spec.mjs:74`); it does not wait for `settled` (`ui/test/settled.mjs:4-10`, which awaits
`document.fonts.ready`).

## Evidence

- Failed once in a `task check` under load average about 20: the panel's right edge at 814, 9 px past 805.
  Passed 3 of 3 alone.
- *Inferred*: the header was measured with a font other than the one the page settles on (the swap had not
  happened, or happened between the measures), which moved the picker 9 px to the right.

## Acceptance

- A loop makes the failure happen on demand (the case under load, or the page measured before the web font
  has loaded), shown red before the change.
- The case waits for `settled` before it measures, or the panel is placed so its right edge is in the
  viewport whatever the header's width; the second is a product change and comes with a deterministic
  red-first case (a header wider than the one the case sees now).
- After the fix the case passes 50 of 50 runs under load. No timeout is raised as the fix.
