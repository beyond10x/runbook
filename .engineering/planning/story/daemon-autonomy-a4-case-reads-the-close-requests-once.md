---
format: aep.planning-md/3
id: story:daemon-autonomy-a4-case-reads-the-close-requests-once
kind: story
status: draft
title: The A4 autonomy case compares requests from one read, after every close request is open
summary: at_a4 compared an open list read when the first of two close requests appeared with a decisions list read later; fix the wait, the daemon answered right
tags:
- backlog
revision: 1
---
## Context

The case `daemon::autonomy::at_a4_the_close_stays_a_persons_although_the_level_answers_the_rest`
(`runbook/tests/daemon/autonomy.rs:165-177`) waits with `until_open(&l, &run, "close")`
(`autonomy.rs:86-98`), which returns as soon as one open request for `close` is listed. The close of the
fixture `runbook/tests/fixtures/autonomy-runbook` makes two requests (request ids `<run>-2-0` and
`<run>-2-1`). The case then reads the decisions view (`autonomy.rs:173`) and asserts
`answered.len() == given.len() - open.len()` (`autonomy.rs:175`) with the `open` list it read before. When
the second close request is made between the two reads, the decisions view holds four requests and the
open list one, and the case fails although the daemon did what the level asks.

## Evidence

A forge-way run of `test:runbook` on 2 CPUs (nextest profile `forge`, `taskset`, load average 9.49 at the
start), 2026-11-26, base head of the model-routes wave; `Summary [ 272.138s] 1902 tests run: 1901 passed,
1 failed, 32 skipped`:

```
FAIL [   3.074s] (221/1902) runbook::cli daemon::autonomy::at_a4_the_close_stays_a_persons_although_the_level_answers_the_rest
panicked at tests/daemon/autonomy.rs:175:5:
every request that is not open was answered: [("…-0-0", "Answered", "rule:approve"), ("…-1-0", "Answered", "rule:approve"), ("…-2-0", "Open", Null), ("…-2-1", "Open", Null)]
  left: 2
 right: 3
```

Both close requests are open and both earlier requests were answered by the rule: the product's answer is
the one the case wants; the case compared two reads made at different moments.

## Acceptance

- The case waits for the state it asserts: every request the close makes is open (read once, from one view,
  after the run is `WaitingOnDecision` with no request in a state other than `Answered` or `Open`), and the
  count it compares is taken from that same read. The race is closed by construction, not by a longer wait.
- The repaired case passes 50 of 50 runs of `--stress-count 50` under the load of the step (judged by the
  nextest Summary line), each run under 5 s.
- No product code changes, unless a deterministic red-first case shows the daemon itself lists a request in
  the wrong state.
