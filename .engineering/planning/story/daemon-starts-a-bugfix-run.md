---
format: aep.planning-md/3
id: story:daemon-starts-a-bugfix-run
kind: story
status: active
title: The daemon starts a bug-fix run from a repository, a ref and a checks file
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-11-25T14:02:01Z", actor: "agent:runbook", revision: 2}
- {from: "proposed", to: "active", at: "2026-11-25T14:02:01Z", actor: "agent:runbook", revision: 3}
---
## Outcome

A bug-fix run can be started through the daemon: the daemon accepts a submit of the bug-fix runbook with the
repository, the ref and a checks file as inputs, and the run works on that repository at that ref with those checks.

## Source

A consumer, 2026-11-25 (relayed by the host operator; accepted by the operator's delegation). A submit of the
bug-fix runbook to a 0.12.0 daemon answers `NoSuchRunbook`: the bug-fix protocol is not in the daemon's runbook home,
and a daemon submit takes no repository, ref or checks input.

## Acceptance

- The daemon lists the bug-fix runbook among the runbooks it can start.
- A submit naming `repository`, `ref` and a checks file starts a bug-fix run on that repository at that ref; the run's
  record names all three.
- A submit missing one of them, or naming a repository or checks file the daemon may not read, is refused before the
  run starts, naming the input.
- The same inputs work for `runbook run` as they do today.

## Open (for scoping)

- Submit inputs read by the daemon, or case inputs the bug-fix protocol declares and the intake fills: decide when
  scoped (the second keeps the protocol the source of truth, like the person-action switch).
- Which repositories a daemon may check out (a ceiling in the daemon's config, as for caps and switches).
