---
format: aep.planning-md/3
id: story:check-bindings-daemon-checks-what-submit-refuses
kind: story
status: draft
title: check-bindings.sh --daemon checks what the daemon refuses at submit
summary: Per-call budget of every program action, the artifact idea in the root bindings, a timeout row beside every * row; pass/FAIL lines, exit 0/1/2.
tags:
- consumer-asks-2026-11-26-after-ui-wave
revision: 1
---
# check-bindings.sh --daemon checks what the daemon refuses at submit

## Origin

Request of a consumer repository, relayed by the operator session on 2026-11-26. Ordered after the run-page wave.

## Problem

A consumer learns only at submit that the daemon refuses its runbook. The local check should refuse the same things
first.

## Contract

`check-bindings.sh --daemon <root>` checks:

- every program action names its per-call budget as the daemon reads it (0 allowed);
- the root bindings place the artifact `idea`;
- every result table with a `*` row has a `timeout` row.

Output: `pass <check>` or `FAIL <check>`, plus one line per finding. Exit 0 on pass, 1 on a finding, 2 on a usage
error.

## Acceptance

1. A fixture tree with an unbudgeted program, one without `idea` and one with `*` but no `timeout` gives 3 `FAIL`
   lines and exit 1.
2. The corrected tree exits 0.
