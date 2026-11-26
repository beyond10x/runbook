---
format: aep.planning-md/3
id: story:one-harness-checks-a-directory-of-canon-protocols
kind: story
status: draft
title: One harness that checks a directory of Canon protocols
summary: runbook check --protocols runs canon check, simulate, check-set and declared guard probes with the pinned Canon; one summary file; exit 0/1/2.
tags:
- consumer-asks-2026-11-26-after-ui-wave
revision: 1
---
# One harness that checks a directory of Canon protocols

## Origin

Request of a consumer repository, relayed by the operator session on 2026-11-26. Ordered after the run-page wave.

## Problem

A consumer checks its Canon protocols with several separate commands and keeps its guard probes in prose.

## Contract

`runbook check --protocols <dir>`, with the pinned Canon:

- runs `canon check`, `canon simulate` and `check-set` over all files of the directory;
- runs the guard probes declared in a file (a protocol and an expression that must be unreachable);
- writes one summary file;
- exits 0 when clean, 1 on a finding, 2 when the installed Canon is not the pinned version.

## Acceptance

1. Three protocols with one broken guard exit 1 and name that probe.
2. With the guard fixed, the run exits 0.
