---
format: aep.planning-md/3
id: story:xdg-runtime-dir-in-step-env
kind: story
status: draft
title: A step's environment carries XDG_RUNTIME_DIR
revision: 1
---
## Outcome

The environment of a step the runner starts carries `XDG_RUNTIME_DIR`, because a host's build pool finds its socket
there.

## Source

A consumer (2026-11-25): the runner gives a check `PATH` alone; the consumer's seed script copies the variable into
every check's environment as a workaround, and the script dies when the directory is missing.

## Acceptance

- A check step run by the runner sees `XDG_RUNTIME_DIR` with no setting per check.
- When the variable is unset or its directory is missing, the step starts without it and the run says so once.
- The consumer can delete its workaround.
