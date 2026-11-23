---
format: aep.planning-md/3
id: story:bugfix-check-reaches-the-pool
kind: story
status: active
title: A check of the bug-fix runbook reaches the host's build pool, and a check that ran outside it is no result
summary: The runner hands its runtime directory to the pool client's process through the bindings copy; a check the client ran outside the pool ends the step as a program error, not as red or green.
relations:
- decomposes: epic:ticket-to-merge-request-on-a-real-repository
- informed_by: story:bugfix-check-names-its-pool-time
scope:
- confidence: cited
  path: driver/src/bin/sdc-bugfix.rs
- confidence: cited
  path: runbook/src/loom_run/bugfix.rs
revision: 5
transitions:
- {from: "draft", to: "proposed", at: "2026-11-23T12:06:07Z", actor: "human:operator", revision: 2, executor: "agent:runbook"}
- {from: "proposed", to: "active", at: "2026-11-23T12:06:08Z", actor: "human:operator", revision: 3, executor: "agent:runbook"}
---
## Defect

Reported from a live bug-fix run of release 0.8.0 on 2026-11-23: every check command of `sdc-bugfix`
started under `runbook run` ran outside the host's build pool, with no queue and no limit on its run time.

- The first line of the run's check log: the pool's client could not reach its daemon and "ran locally".
- The environment of the `sdc-bugfix` step held `PATH`, `LOOM_*`, `SDC_*`, `ESS_*`, `AEP_ACTOR`, `PWD` and
  `SHLVL`. The pool's client finds its daemon through `XDG_RUNTIME_DIR`, which was not there.
- The pool's status listed none of the run's jobs while they ran.

Cause: a bound program gets `PATH` and `LOOM_*` only (the contract of the runtime, which stays), and
`sdc-bugfix` starts the pool's client with what it got.

## Outcome

1. The runner, which has the runtime directory of its user, writes it into the run's bindings copy as one
   option of the `sdc-bugfix` steps that run a check. `sdc-bugfix` sets it in the environment of the pool
   client's process only. No other variable passes, and nothing passes through the runtime's environment.
2. A check whose pool client reports that it ran outside the pool is no result: the step ends as a program
   error that names the pool as unreachable. It is neither red nor green, and no test verdict is recorded
   from it.
3. With no pool on the host at all (the client is not installed), the step behaves as release 0.8.0
   documents it; the page of the bug-fix runbook says which of the two cases is which.
4. No general list of variables that pass to bound programs is added by this story: that changes the
   runtime's contract and is the operator's decision.

## What a guard must hold

The option holds a directory path of the runner's own user and nothing else; a value that is not an
absolute path of an existing directory owned by that user is refused before a step starts. The value is no
secret, and no other value travels the same way.

## Acceptance

With a stand-in pool client on the test's own `PATH`: (a) the client's process sees the variable with the
runner's value and the check's own process does not get any variable beyond what it had; (b) a stand-in
that prints the client's "ran locally" line makes the step end as a program error naming the pool, with no
verdict recorded; (c) a refused value starts no step.

## Origin

A consumer's defect report of 2026-11-23, relayed by the operator's host session.
