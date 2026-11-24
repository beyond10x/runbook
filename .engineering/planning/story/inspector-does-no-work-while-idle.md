---
format: aep.planning-md/3
id: story:inspector-does-no-work-while-idle
kind: story
status: active
title: The inspector's server reads no run file while no page is connected
summary: An idle server uses under 1 % of a core; with a page it reads only the shown run and only what changed; the cost does not grow with the runs not shown.
relations:
- informed_by: story:daemon-page-watches-run-directories
scope:
- confidence: cited
  path: ui/server.mjs
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-11-23T12:03:53Z", actor: "human:operator", revision: 2, executor: "agent:runbook"}
- {from: "proposed", to: "active", at: "2026-11-23T12:03:54Z", actor: "human:operator", revision: 3, executor: "agent:runbook"}
---
## Defect

Reported by a consumer session on 2026-11-23, measured on a host with 20 cores at 0 % idle:

- An inspector server over a runs directory of about 50 run directories (5.8 GB), with no page open, had
  used 63 minutes of CPU in 2 hours 10 minutes of run time, 65 % of one core at the moment of the sample.
- A second server on the same directory used 64.8 % of one core 7 minutes after its start.
- A server over this repository's own runs directory had used 8 hours 40 minutes of CPU in 1 day 11 hours,
  24 % of one core.

Cause as read on the default branch: `ui/server.mjs` ends with `setInterval(tick, 700)`; `tick` runs
whether or not a page is connected, and what it reads grows with the number of runs under `--runs`.

## Outcome

1. A server with no page connected and no request in flight reads no run file. Measured over 60 s on a runs
   directory of 50 fixture runs: under 1 % of one core.
2. With a page connected, the server reads only the run that page shows, and only what changed: a file whose
   size and modification time are those of the last read is not read again.
3. A change of the shown run still reaches the page within 2 s (the page's live behaviour stays).
4. The cost of one round does not grow with the number of runs the page does not show: measured with 5 and
   with 50 fixture runs, the round for one shown run takes the same time within 20 %.
5. The test clock of the UI gate (`--clock`) keeps working as it does.

## What a guard must hold

Every read stays behind the run-path check and the one checked open. A cache holds what a checked read
returned for one run and is never served for another run or another path. No new route, no write.

## Acceptance

A test starts the server on a directory of 50 fixture runs, counts the reads of run files through a counter
the server exposes only under a test option, and finds 0 reads over 3 idle rounds with no page connected;
with one page on one run it finds reads of that run only, and none in a round where nothing changed. The
timing of outcome 4 is written into the report of the change with both numbers.

## Origin

A consumer session's defect report of 2026-11-23, sent on the operator's order.
