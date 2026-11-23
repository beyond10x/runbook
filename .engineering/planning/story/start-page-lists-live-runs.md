---
format: aep.planning-md/3
id: story:start-page-lists-live-runs
kind: story
status: active
title: The page opens on the runs that are live now, one line each
summary: A start page lists every live run with its runbook, case, step in flight, since when and spend, and links to the detailed view; ended runs are below.
relations:
- depends_on: story:runner-records-a-step-in-flight
- informed_by: story:daemon-page-watches-run-directories
scope:
- confidence: cited
  path: ui/server.mjs
- confidence: cited
  path: ui/web/app.js
- confidence: cited
  path: ui/web/index.html
revision: 6
transitions:
- {from: "draft", to: "proposed", at: "2026-11-23T11:55:21Z", actor: "human:operator", revision: 2, executor: "agent:runbook"}
- {from: "proposed", to: "active", at: "2026-11-23T11:55:21Z", actor: "human:operator", revision: 3, executor: "agent:runbook"}
---
## What the operator asked for

2026-11-23: "this web UI of Runbook, it should have some front page or start page where we would have a
quick access to all the running stuff. So I mean, I guess there's a daemon, a Runbook daemon, right? And
that is handling all the case runs, and the UI should be capable of showing what is running and what
specifically is happening in that case right now. Plus the more detailed view, which we already have."

## Outcome

The page opens on a list of the runs that are live now, one line each: the runbook, the case, the step in
flight, since when, the spend so far, and a link to the detailed view of that run. Runs that ended are
below, most recent first. The list covers the runs of `runbook run` in the runs directories the page
watches and, under the daemon, the daemon's runs (`story:daemon-page-watches-run-directories` brings the two
kinds onto one page; this story is the list itself and works for the inspector first).

- A line updates without a reload when a step starts or ends.
- A run whose runner is gone is not listed as live.
- With nothing live the page says so in one line and shows the ended runs.

## What a guard must hold

- Read-only: the list has no control that acts on a run.
- The content of a run never becomes markup or code of the page.
- A run directory the page may not read is not listed and its name is not shown.

## Acceptance

Cases on fixture run directories with the records of `story:runner-records-a-step-in-flight`: two live runs
and one ended give two lines above and one below; a step that ends changes its line without a reload; the
link opens the detailed view on that run; both sizes pass the layout checks of the UI gate.

## Origin

The operator's words of 2026-11-23, relayed by the consumer session.
