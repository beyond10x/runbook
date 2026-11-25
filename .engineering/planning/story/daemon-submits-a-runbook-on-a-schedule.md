---
format: aep.planning-md/3
id: story:daemon-submits-a-runbook-on-a-schedule
kind: story
status: draft
title: The daemon submits a named runbook on a schedule, under the daily spend cap
revision: 1
---
## Outcome

A daemon's own config lists schedules (runbook id, intake text, time of day, time zone, caps). At that time the daemon
submits the runbook once, skips a day whose run is still open or whose daily cap is spent, and records why it skipped.

## Source

A consumer, 2026-11-25 (relayed by the host operator). The consumer needs one run per day at a fixed time with a fixed
intake; the daemon starts runs only on a submit, so a person or an outside timer must start each.

## Acceptance

- With a stub clock set to the time: exactly one submit, and the run ends `Ended` for a stub protocol.
- A second tick the same day submits nothing.
- With the daily cap at 0 it submits nothing and records the cause `cap`.
- No model.

## Status

Draft. A daemon that starts runs on its own, and spends against the daily cap without a submit, is a change of
architecture: it waits for the operator's decision.
