---
format: aep.planning-md/3
id: story:run-page-watches-a-live-run
kind: story
status: draft
title: The run page watches a live run
summary: The daemon's run page shows the steps of a live run, readable and without clutter
revision: 1
---
## Outcome

The operator watches a live run on the page of `runbook serve --ui` and sees what the run is doing,
readable and without clutter. The operator's order of 2026-11-26: "can you please make the UI look
good", for the run page at a live run; earlier: "why do I never observe an actual real run ?!".

While the daemon serves no graph of a case, the centre shows the run's steps in order (name, role,
state, start time, duration, try), the running step marked and live, the stop or end reason in
words, and the counts the page has (model calls, tokens, cost) where it has them. The data is the
run's event stream the page already reads; no new daemon endpoint.

## Acceptance

Each finding below has a regression asset that fails on the page before the change and passes after
it (`ui/ux/findings/UXF-nnn.md` names it), over a synthetic daemon fixture of a live run (no real
data). The loop's hard gates hold for the run page at the gating viewports (1600x1000, 1366x768,
805x1270, 390x844): no horizontal scroll, the always-visible controls inside the viewport, axe-core 0
serious or critical, the steps reachable by keyboard. The ratchets of `ui/ux/baseline.json` move in the
same change, never worse.

## Findings (before screenshots of a live run, 1366x768, 1920x1080, 390x844)

1. The centre (70 % of the width) is empty during a live run: "The daemon serves no graph of a
   case". No current step, no step list, no progress.
2. Times are US 12 h with seconds ("07:54:50 PM"), no zone. Show 24 h local time of the browser with
   its zone abbreviation (`19:54:50 CET`), dates ISO.
3. Run events print raw JSON with hashes (`{"seq":19,"prev":"3ccc4b…`).
4. Event names mix forms (`runbook.action-started`, `Evaluated`, `Stopped`, `ActionFinished`);
   Evaluated and Stopped repeat per step and bury the rest.
5. The title is the full run UUID; the runbook name is small grey under it.
6. The case card truncates both lines; the summary line above it has no left padding.
7. Two empty bands under the header (a 45 px blank band, a "PRODUCT" strip with one sentence).
8. The right rail of rotated tab labels is hard to read; the closed drawer sits off-screen.
9. The wide view shows a TRUE/FALSE/UNKNOWN legend for a graph that is not shown.
10. Phone: the header controls wrap and cover the mode badge; "running a step" overflows its pill.

An event `Stopped` with `stop: BudgetSpent` while the run is still running is shown in plain words
(one step per round, the run goes on); what it means for the run is another story's defect.
