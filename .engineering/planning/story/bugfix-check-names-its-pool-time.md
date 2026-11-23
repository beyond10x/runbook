---
format: aep.planning-md/3
id: story:bugfix-check-names-its-pool-time
kind: story
status: active
title: A check command of a bug-fix run names its pool time, and the run takes a checks file
summary: sdc-bugfix passes a time a check command names to the pool (at most 30 minutes), and runbook run --bugfix takes --checks <file> for a repository that is not Rust.
scope:
- confidence: cited
  path: docs/bugfix.md
- confidence: cited
  path: docs/loom-run.md
- confidence: cited
  path: driver/src/bin/sdc-bugfix.rs
- confidence: cited
  path: runbook/src/loom_run/bugfix.rs
- confidence: cited
  path: runbook/src/main.rs
revision: 8
transitions:
- {from: "draft", to: "proposed", at: "2026-11-23T11:33:53Z", actor: "human:operator", revision: 2, executor: "agent:runbook"}
- {from: "proposed", to: "active", at: "2026-11-23T11:33:53Z", actor: "human:operator", revision: 3, executor: "agent:runbook"}
---
## What a consumer met

A consumer ran the bug-fix runbook from release 0.8.0 on two repositories that are not Rust, with check
commands of its own. Two things stood in its way (2026-11-23, read from the tree at the tag):

- `sdc-bugfix` starts each check command in the build pool without naming a time
  (`driver/src/bin/sdc-bugfix.rs`, the `slot` call of `run`). The pool ends a job after 10 minutes of run
  time; the step then exits 70 and Loom ends the run `ProgramError` (`docs/driver.md`, the pool section). A
  suite that needs more than 10 minutes cannot pass, and `checks.yaml` has no member that names a longer time.
- `runbook run --bugfix` hands the seed no `--checks` (`runbook/src/loom_run/bugfix.rs`). A supplied file is
  read only as `.runbook/checks.yaml` inside the tree, so a consumer has to put a file into a repository it
  does not own. `docs/loom-run.md` says the bug-fix runbook works on Rust repositories only.

## Outcome

1. A check command names its time: an optional member per command in `checks.yaml` (and one for the file as
   the value of every command that names none), passed to the pool as `slot --timeout`. No member means the
   pool's own limit, as today. A value above the pool's maximum of 30 minutes is refused when the file is
   read, by the name of the member, before anything runs. The sum of the named times of one action above the
   action's `timeout_seconds` is refused the same way, or stated as a warning line: the implementor says
   which and why.
2. `runbook run --bugfix <dir> --checks <file>` hands the file to the seed. The file is read once, by the
   runner, and reaches the seed as its `--checks`; it is not copied into the tree under test.
3. `docs/bugfix.md` and `docs/loom-run.md` say what a repository that is not Rust needs (the supplied checks)
   and what still holds only for Rust, each sentence checked against the code.

## Acceptance

- A case with a stand-in `slot` on its own `PATH` shows the `--timeout` argument of each command, for a
  command that names a time, for the file's value and for none.
- A case shows the refusal of a value above 30 minutes and of a value that is not a whole number of seconds,
  exit 2, with the member's name and nothing started.
- A case of the runner shows `--checks <file>` reaching the seed and a run that uses the supplied commands
  on a tree without `Cargo.toml`, on the stub adapter, with no file added to the tree.
- No test starts the host's pool, a model or the network; each case stays under 60 s.

## Origin

A gap named by a consumer session on 2026-11-23, before a hotfix run. Pre-existing since the bug-fix
runbook moved to the pool.
