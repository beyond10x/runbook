---
format: aep.planning-md/3
id: story:child-way-runner-ends-on-a-signal-while-an-orphan-holds-its-pipes
kind: story
status: draft
title: The child way's runner ends on SIGTERM while a left-behind process holds its output pipes
summary: child::run joins its pipe readers after the child ended and no longer reads the cancel flag; a process that keeps stdout open holds the runner past a SIGTERM
tags:
- backlog
revision: 1
---
## Context

`runbook run --loom` (the child way) starts each `b10x-loom` call through `child::run`
(`runbook/src/loom_run/child.rs:66-126`). The child gets its own process group (`child.rs:83`) and two
threads read its stdout and stderr to their end (`pump`, `child.rs:48-54`, started at `child.rs:87-88`).
The loop at `child.rs:90-119` checks the cancel flag, which the SIGTERM, SIGINT and SIGHUP handlers set
(`runbook/src/loom_run/run_loop.rs:45-62`), only while the child itself is alive.

Once `try_wait` has reaped the child (`child.rs:92`), or after the group was sent SIGTERM and SIGKILL
(`child.rs:100-114`), the function joins both readers (`child.rs:122-123`). `read_to_end` returns only
when every holder of the write end has closed it. A process that the child started and that is still
alive keeps the pipes open: a member of the group when the child ended on its own, since no signal goes
to the group on that path, or a process that left the group (`setsid`, `setpgid`) after a signal. The
runner then blocks in `join` and no longer reads the cancel flag. A SIGTERM or SIGINT sent to the runner
in that state sets the flag and nothing else: the run does not end as `stopped: signal` (exit 7,
`docs/loom-run.md`) until the left-behind process ends.

Found in the wave 6 review. The same join-after-exit shape stands in `runbook/src/loom_run/answers.rs:305-308`
(a declared answer's program) and in the decider's program reader (`runbook/src/loom_run/decider.rs:325-347`);
those are outside this story and are named here for whoever scopes it.

## Evidence

- `child.rs:90-119`: the only reads of `cancel` are inside the loop that runs while the child is alive.
- `child.rs:122-123`: `out.join()` and `err.join()` have no bound and no cancel check.
- `child.rs:113`: the group is sent SIGKILL only on the signal and wall-time path, not when the child ended
  on its own.
- *Inferred*, not yet reproduced: a bound program that starts a background process which keeps stdout open
  (for example `sleep 600 &` without redirection) holds the runner past a SIGTERM.

## Acceptance

- A red-first case under `runbook/tests/loom_run/` against the fake `b10x-loom`: the fake starts a process
  that keeps the inherited stdout open and leaves the group, the fake exits, the case sends SIGTERM to the
  runner and the runner does not end within 1 s. Recorded red before the change.
- After the change the same case sees the runner end as `stopped: signal` with exit 7 within 1 s of the
  SIGTERM, and the left-behind process is not waited for. The case runs under 5 s.
- The change is a product change backed by that deterministic case; no timeout of an existing case is raised.
