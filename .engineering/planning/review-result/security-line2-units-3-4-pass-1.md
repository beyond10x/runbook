---
format: aep.planning-md/3
id: review-result:security-line2-units-3-4-pass-1
kind: review-result
status: active
title: 'Security review of line 2 units 3 and 4 at c413209: red, 4 pre-existing findings'
summary: 'Ten invariants checked: eight enforced, two in part. Nothing introduced. Pre-existing: under local confinement a bound program can write the run copy (decider instructions, adapter, replay plan and call log); a live call of a replay that ends before it finishes is not counted against the cap; one note already with the implementor. Under substrate confinement the run directory is neither visible nor writable.'
tags:
- wave-2026-11-22-w1
relations:
- reviews: story:driver-runtime-removed
revision: 1
---
```
unit: line 2 units 3 and 4, security review, at c413209
verdict: red
cases: executed 635→641 in the runbook binary (6 added), red 1
origin: introduced 0 / pre-existing 4 / undecided 0
wrote-outside-worktree: <scratch>/removal/line2/security-1/ (cases.patch, run1.log to run4.log, suite.log, build.log, build-driver.log, main-sdc-replay.rs); build output under runbook/target/ and driver/target/ (ignored by git)
needs-coordinator: no
```

## Diff stat (cases only)

```
 runbook/tests/loom_run/mod.rs               |   1 +
 runbook/tests/loom_run/security_line2.rs    | 320 +++
 2 files changed, 321 insertions(+)
```

No file under `src/` is in the diff.

## Cases

All six are in `runbook/tests/loom_run/security_line2.rs`.

1. `under_local_a_program_that_appends_to_the_decider_instructions_changes_what_the_model_is_told` is green and records current behaviour. A hook appends a marker to `instructions/decider/*.md` and the adapter then receives the marker.
2. `under_local_a_program_that_replaces_the_decider_adapter_is_started_by_the_runner_with_the_login_link` is green and records current behaviour. The replaced adapter is started by the runner with `SDC_CLAUDE_CREDENTIALS` set. The case records only `link-set`, never the value.
3. `the_adapter_obeys_a_plan_and_a_call_log_rewritten_after_the_run_began` is green and records current behaviour. A plan with the stop policy and cap 0 gives exit 75 and 0 live starts. After the plan is rewritten and `calls.jsonl` is truncated, the result is exit 0 and 1 live start.
4. `under_local_a_bound_program_writes_the_run_copy_and_creates_the_replay_plan` is green and records current behaviour. It uses the real Loom library with a POSIX sh writer. The writer changes the instructions, the adapter, the routing table, the classes and `replay/plan.json`. The `read readable` control passes. It took 9.6 s.
5. `under_substrate_a_bound_program_writes_none_of_the_run_copy_and_no_replay_plan` is green. The linux level is offered under `slot`. The host files are unchanged, no `<run>/replay` exists, and the control reads `unreadable`. The probe wrote five files only inside the sandbox's private view. The run ends `stuck`, because the writer's `draft.txt` is refused. It took 4.5 s.
6. `a_live_call_that_is_ended_before_it_finishes_counts_against_the_cap` is RED (cap 1).
   - The first adapter process is killed mid-call, then the stub is killed.
   - Red output: `assertion left == right failed: ... (live starts: 2, exit 0)  left: (2, 0)  right: (1, 75)`.
   - A call that ends before `finish` writes no line to `calls.jsonl`, so the second call is also allowed live.

Cases 4 and 5 run over 5 s. Cases 1, 2, 3 and 6 are fast.

## Suite run

Command: `cargo nextest run --locked --no-fail-fast` with `-E` filters on `loom_run::security_line2`, `loom_run::decisions`, `loom_run::replay_boundaries`, `decider::tests` and `replay::tests`, through the pool.

- Result: 125 tests, 124 passed, 1 failed (case 6), 7.8 s.
- The failing case is the only red one.
- Longest test: case 4, 9.6 s, under the 60 s bound.

## Invariants

1. Principals: enforced.
   - `decider.rs` sets `self.principal` and `Given.principal`.
   - `child_host.rs:77-82` (`command_argv`) replaces the value after `--principal` with the decider's.
   - A typed `answered_by` stays a label.
   - Case: `decisions.rs`, passes.
2. A level below A5 never widens, and a missing table stops for a person: enforced.
   - Guard: `decider.rs:290-293`, with the pinned read at `decider.rs:551` (`pin_routing`).
   - Case: `a_table_that_is_unreadable_at_the_start_stops_for_a_person_at_every_level`, 15 runs, green.
3. The routing table is pinned: enforced for what is read, but not for the run copy under `local`.
   - The read happens once at the start, so a change after the start does not alter who decides.
   - Under `local` a bound program can write the run copy (case 4). `docs/loom-run-decisions.md:69` states this.
   - Under `substrate` it cannot (case 5).
   - The pin does not cover the decider's instructions, its adapter or the declared answer programs (findings A and B).
4. The level comes from the caller's settings only: enforced. No model-written file feeds the level. I read `run_loop.rs` and `decider.rs` and found no such path. I built no separate case for this one.
5. The spend cap fails closed and counts before the call: partly enforced.
   - The fail-closed part holds. Without a readable cap, `sdc-replay.rs:1227` gives no live call (`live_cap.is_some_and(...)` at `:1238`).
   - The count is not made before the call. It is made after, in `finish` (`:1262`), and the live call starts at `:1290-1295`. A call that ends before `finish` is not counted (case 6, finding C).
6. The plan is the runner's path only: partly enforced.
   - The adapter reads the plan only from `SDC_REPLAY` (`sdc-replay.rs:1063`, `ctx_at`). No plan in the work tree or above it is read.
   - Under `local` a bound program can write `<run>/replay/plan.json` and `calls.jsonl` (cases 3 and 4). Under `substrate` it cannot (case 5).
7. `settings.replay` without `--recorded` is refused: enforced.
   - Guard: `replay.rs` `preset` (the refusal at `:120-125` and `:133` per pass 2). It exits 2 before the run directory exists.
   - `binding_plan` requires the adapter file name `sdc-replay`.
   - Case: `replay_boundaries.rs`, green.
8. No login or token in argv, logs, the run directory or recordings: enforced.
   - The login reaches the decider adapter only as a link (`decider.rs:386`, `SDC_CLAUDE_CREDENTIALS`).
   - Case 2 records only `link-set`.
   - A bound program receives `PATH` and `LOOM_*` only. Case 4 shows the program's own environment.
   - I found no token in the recording or the run output.
9. The recording is invented text: enforced.
   - I checked the 35 files under `driver/examples/sdc/fixtures/recorded/stub`. They hold no model output, no home paths and no secrets, and prompts appear only as digests.
   - The file set equals the one on `main`.
10. `--confinement`: enforced.
    - The clap field is required with no default.
    - A recorded run does not skip it. I found no path that lowers it.

## The two answers

Finding 4: can a bound program or a model write a file the decider reads as its instruction? The answer is yes under `local` and no under `substrate`. This is verified at step 4 or 5, with the real code, the real Loom library and real bound programs, and not by reading alone.

- Path written: `<run>/instructions/decider/*.md` and `<run>/bindings/tools/llm-adapter-<name>.sh` (case 1, case 4).
  - The decider reads them per decision (`decider.rs:399`, `:467`; `ask.rs:85`) via `instructions_root` (`decider.rs:619`). That function walks up to any ancestor with `instructions/`.
  - The runner then starts the replaced adapter unconfined, with the login link set (case 2).
  - The declared answer programs and `decision-grounds.sh` are in the same run copy and run the same way.
- A model node can reach the same files if its tools allow writing outside the work tree. I did not run a model; this part is inferred.
- What stops it: `substrate` only. The linux level gives a private view with `/usr`, `/bin`, `/lib`, `/lib64`, `/proc`, `/dev`, a private `/tmp` and the workspace, where only `target/` is writable. The run directory is neither readable nor writable there (case 5). `local` is declared insecure (`docs/loom-run.md:31`) and has no stop.

Finding 5: can a bound program write `<run>/replay/plan.json`, and what follows? The answer is yes under `local` and no under `substrate`. This is verified (cases 3, 4 and 5).

- Path written: `<run>/replay/plan.json` and `<run>/replay/calls.jsonl`.
- What follows (case 3):
  - A live call passes the cap. The plan with cap 0 stops the run. After the plan is rewritten and the log truncated, one live call starts and exit is 0.
  - The adapter starts the program named in `live.bin` as the live adapter. The rewritten plan can name any program.
- Under `substrate`, no `<run>/replay` exists after the run and the host files are unchanged.
- The substrate tools and table point (invariant 10's inferred item): verified. The run's `tools/` directory and the routing table copy lie outside what a bound program can write. Only the program file is mounted, read-only, and the run directory is not visible.
- The linux level is offered only if the process may delegate a cgroup. Jobs under `slot` can. A bare shell cannot, but `systemd-run --user --scope -p Delegate=yes` works.

## Judgement findings

A. **Under `local`, the decider's instructions and adapter are not pinned.** The docs concede the table (`docs/loom-run-decisions.md:69`). Instructions, adapter and declared answer programs are read or started per decision from the run copy. A bound program that rewrites them controls what the model is told or runs a program of its choice with the login link. This is the same as `main` (`instructions_root`, same code). Fix: pin the instruction text at the start like the table, or state it beside the table in the doc and refuse to offer a decider under `local`. The pin choice is a design call.

B. **Under `local`, `replay/plan.json` and `calls.jsonl` are writable by a bound program.** The adapter reads `live.bin`, `cap` and the policy per call (`sdc-replay.rs:1076` on `main`; the plan is read in `ctx_at`, `:1063`). A rewritten plan makes any program the live adapter, and a truncated log resets the live count. Same read on `main`. Fix: have the runner pass the policy, cap and live program in the process's own environment, or hold the count in the runner. The doc should name this under `local`.

C. **A live call ended before it finishes is not counted against the cap.** `calls.jsonl` is appended only in `finish` (`sdc-replay.rs:1262`), after the live call (`:1290-1295`). The live count at `:1225` therefore misses a call killed by the action timeout, a signal or a retry. Case 6 is red: 2 live starts at cap 1. The same count is on `main`; the head only made an unreadable cap fail closed. Fix: append the count line before starting the live program, and mark it finished afterwards.

D. **Optional note.** The divergence message `in/tests.txt changed` does not name the cause for a recording packed at another label. This is the finding the implementor already holds, so I am not repeating it here.

## What was reviewed and could not be faulted

- The principal replacement in `child_host.rs:77-82`. The daemon's `answered_by` stays a label.
- The routing table read once at the start. A deletion or change after the start changes nothing (pass 2 attack and my reading agree).
- The unreadable-table stop at all five levels (15 runs).
- `SDC_REPLAY` as the only plan source. A plan in the work tree or above it is never read.
- The `settings.replay` refusal, in the forms pass 2 tried.
- `--confinement` required, with no default, and not lowered by `--recorded`.
- The login link: it reaches only the decider adapter's environment and appears in no log or recording that I read.
- The committed recording: invented text, same file set as `main`.
- The substrate level: it blocks writes to the run copy and the plan (case 5).

## Paths written outside the worktree

`<scratch>/removal/line2/security-1/` (the files named in the header). I made no commit, no stash and no store write. No model call, no network and no login file were used.

## cases.patch and clean tree

- Patch: `<scratch>/removal/line2/security-1/cases.patch` (338 lines).
- After `git apply -R` and `git reset -q`: `git status --porcelain` is empty (0 lines) and HEAD is `c413209` (detached).
- `git apply --check` accepts the patch again.

Severity mapping: `major` became `warning` for A and B (a bound program controls the decider's text or the live adapter, but only under `local`, which is declared insecure). `minor` became `warning` for C, because it breaks invariant 5 (the count before the call) on every confinement level. The note stays `note`.

```findings
[
  {"file": "runbook/src/loom_run/decider.rs", "line": 619, "category": "acceptance", "severity": "warning", "verdict": "NEEDS-CHANGE", "origin": "pre-existing", "message": "under --confinement local a bound program can rewrite the decider's instructions (<run>/instructions/decider/*.md, read per decision at decider.rs:399, :467, ask.rs:85 via instructions_root, which walks up to any ancestor with instructions/), its adapter <run>/bindings/tools/llm-adapter-<name>.sh and the declared answer programs; the runner then starts the replaced adapter unconfined with SDC_CLAUDE_CREDENTIALS set to the login link (runbook/tests/loom_run/security_line2.rs under_local_a_program_that_appends_to_the_decider_instructions_changes_what_the_model_is_told and under_local_a_program_that_replaces_the_decider_adapter_is_started_by_the_runner_with_the_login_link, green, and under_local_a_bound_program_writes_the_run_copy_and_creates_the_replay_plan); under --confinement substrate the run directory is neither visible nor writable (under_substrate_a_bound_program_writes_none_of_the_run_copy_and_no_replay_plan, verified at step 4/5); docs/loom-run-decisions.md:69 concedes only the routing table; same code on main 17c00ab57f; pin the instruction text and the adapter at the start like the table, or say so in the doc and refuse a decider under local."},
  {"file": "driver/src/bin/sdc-replay.rs", "line": 1076, "category": "acceptance", "severity": "warning", "verdict": "NEEDS-CHANGE", "origin": "pre-existing", "message": "under --confinement local a bound program can create or overwrite <run>/replay/plan.json and <run>/replay/calls.jsonl, which the adapter reads on every call (ctx_at at :1063, live.bin, cap and policy, live count at :1225); a rewritten plan names any program as the live adapter and a truncated log resets the live count, so a live call passes the cap (the_adapter_obeys_a_plan_and_a_call_log_rewritten_after_the_run_began, green: plan stop cap 0 gives exit 75 and 0 starts, after the rewrite exit 0 and 1 live start); under substrate no <run>/replay exists after the run; same read on main 17c00ab57f; hand the policy, cap and live program to the adapter in a way the bound program cannot write, or keep the count in the runner."},
  {"file": "driver/src/bin/sdc-replay.rs", "line": 1262, "category": "acceptance", "severity": "warning", "verdict": "NEEDS-CHANGE", "origin": "pre-existing", "message": "the cap counts after the call, not before it: calls.jsonl is appended only in finish (:1262) and the live program starts at :1290-1295, so a live call that ends before finish (action timeout, signal, retry) is not counted at :1225 and a second call also runs live; runbook/tests/loom_run/security_line2.rs a_live_call_that_is_ended_before_it_finishes_counts_against_the_cap is red: left (2, 0), right (1, 75) at cap 1; same count on main 17c00ab57f (the head only made an unreadable cap fail closed); append the count line before starting the live program."},
  {"file": "runbook/src/loom_run/replay.rs", "line": 222, "category": "contract-drift", "severity": "note", "verdict": "NEEDS-CHANGE", "origin": "pre-existing", "message": "the divergence message for a recording packed at another ESS label says 'in/tests.txt changed' and names no cause; the implementor already holds this note, so it is listed only to keep the block complete."}
]
```
