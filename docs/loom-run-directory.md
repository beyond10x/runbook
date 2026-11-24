# The run directory of a Loom run

`runbook run` writes one directory per run, `<runs-dir>/<name>/`. This page says which files in
it `sdc-experiment report` and `runbook parity summarize` read, where each value comes from in a Loom
run, what a Loom run does not record, and which quality measures of
`experiments/release-0.1.2/report.md` the directory can give. The writer is
`runbook/src/loom_run/run_dir.rs`; the options and the loop are in `docs/loom-run.md`.

```
<run>/settings.json      redacted settings (the loop)
      events.jsonl       the loop's events (the loop)
      runner.json        the runner process: pid, start time, open or closed (see "The runner and the step in flight")
      outcome.json       how the run ended (the loop)
      tree-status.json   what `b10x-loom tree status` printed (the loop)
      tree-verify.json   what `tree verify` printed (the loop)
      bindings/ protocols/   the bindings copy; the protocol files (see "What R1 must hand over")
      work/              the workspace: the product tree
      store/             Loom's case store (runlog-case-<hex of the case id>.jsonl, case-*.json, ...)
      store/<root>/      the case store the report reads, written by the writer (below)
      loom/status.json   the status without absolute paths (written at the end)
      loom/verify.json   the verify result without absolute paths
      run-settings.jsonl llm-calls.jsonl decisions.jsonl rounds.jsonl   append-only records
      run-stopped.json   the stop record, when the run did not close
      dry-commands.txt   the commands a --dry run printed, one per line (only with --dry)
      status.json        the close record, when the tree ended
```

`free-agent.json` is never written (it would make the report read the directory as a free-agent run).
`experiment.json` and a `run-stopped.json` the launcher wrote are not touched.

## Two kinds of file

Append-only records, one line per event, written when the event arrives, one `write` per line, so a
killed run leaves whole lines: `run-settings.jsonl` (one line at the start), `llm-calls.jsonl` (one
per costed model call of a step), `decisions.jsonl` (one per decision answered), `rounds.jsonl` (one
per `tree run` step). `llm-calls.jsonl` and `decisions.jsonl` are created empty at the start: an empty
file says "none yet", and a run records every call it pays for.

A projection, rewritten whole through a temporary file and a rename after every step, only when its
content changed: `store/<root>/` (the driver's case store layout, made from Loom's run logs) and
`loom/`. A reader sees the old file or the new one, never half of one. The projection reads
`store/runlog-case-<hex>.jsonl` of every case (the root from `store/tree.json`, the children from the
`ChildOpened` lines), `tree-status.json` when it exists, the bindings and protocols copies, and
`work/findings.yaml`.

One writer writes every record of the run directory once the first program of the run has started
(`runbook/src/loom_run/record_write.rs`): a whole file is made as a new temporary file, under a dot name the runner
chooses for that write (its process id and a counter), opened without following a link and without waiting for a reader,
and replaces the record by a rename; a line is appended only to a regular file with one name, opened the same way. A
program of the run can write this directory (`--confinement local`), so a name it took (a link, a pipe, a directory, a
second name of a file outside the run) is never opened for writing: the writer takes the next of 16 temporary names, and
when all 16 are taken, or the file to append to is not a regular file with one name, the run ends within seconds with an
error that names a file in the run directory, and nothing is written outside the run. This holds for `status.json`,
`run-stopped.json`, `loom/`, `store/<root>/`, the append-only records, `dry-commands.txt`, `runner.json`, `outcome.json`,
the tree records and the files of the decider's calls (`decider/`, `decider-cases/`). The files of a decider call are
written through the open directory of the call, which the runner made and kept open (`Held` in the same file): they are
made relative to that descriptor, so a program that renames the call's directory while the adapter runs and puts a link or
another directory at its name does not change where a record goes; the records land in the directory the runner made.
The bindings copy (`bindings/`) is held the same way once the first program has started: before each step the runner
opens it without following a link at its name, lists it, reads its files and rewrites the call budget through that
descriptor, and the answer steps, the principal and the results of an action are read through it too; the bound actions
that the start of a program is announced with (`runbook::engine::announce`, for `runbook run` and the daemon) are read
through a descriptor of the directory opened the same way, as regular files with one name of at most 64 MiB.
A link or a file at the name `bindings`, or a link at a bindings file, ends the run with an error that names the file
inside the run, and nothing is written outside the run.

The watcher (`sdc-watch`) opens its output directory (`fs/`) once, at start, without following a link at its name, and
makes, reads, renames and removes every file of it through that descriptor: the blobs under `fs/blobs/`, `changes.jsonl`,
`snapshot.json`, and the pid record `sdc-watch.pid`, which it writes through a temporary file and a rename. A program that
renames `fs/` while the watcher runs and puts a link at its name does not change where a record goes. The temporary files
of the watcher are created as new files that follow no link and wait for no reader: a blob takes the next of 16 temporary
names when a name is taken, and does not remove the name that is taken; `fs/.snapshot.json.tmp` and
`fs/.sdc-watch.pid.tmp` have one name, and what lies at it is removed first. The writer does not write what is made before the first
program starts (`settings.json`, the first `events.jsonl`, the bindings and protocols copies, `fs/watch.log`) and Loom's
own files (`store/runlog-case-*.jsonl`, `store/case-*.json`), which Loom writes.

The directories of the run directory that the runner makes once the first program has started (`loom/`, `store/<root>/`,
`decider/`, `decider-cases/`, a call's `cwd/in` and `cwd/out`, the directories of a copy into `in/`, `work/.sdc/reasons`)
are made by `make_dir_in` of the same file, without following a link: every component below the run directory is made
with `mkdirat` and opened with `O_NOFOLLOW | O_DIRECTORY`, and a link or a file at a component ends the run, or the call,
with an error that names that directory inside the run; nothing is made beyond it.

A name in the run directory that the runner reads and a program can reach (`work/findings.yaml`, `structured-output.json`,
`out/decision.json`, the file a copy into a call's `in/` takes, the files of `instructions/decider/`, the files of the
bindings copy read for the call budget, the answer steps, the bound actions, the principal of an action and the results
of an action, Loom's case file, and the call directories' records the decider reads) is read as a regular file with one
name: it is opened without following a link and without waiting for a writer, and at most 64 MiB are read
(`READ_LIMIT`). A link, a pipe, a directory, a file with a second name or a file of more than 64 MiB is not read: the copy
of `work/findings.yaml` is left out; a copy into the `in/` of a call of the model decider ends the call with an error that
names the file inside the run; the requester decider skips such a file, which is then not in the list of inputs the model
reads, and goes on. The records of a re-drive (`replay/diverged.json`, `replay/plan.json`) are read the same way; one that
is not a regular file is no record. The directory in which such a read opens its file is reached by its path, except
the bindings copy and the directory of a decider call, which are held.

The watcher reads every name of the run the same way: without following a link, without waiting for a writer, a regular
file, at most a bound of bytes, and once SIGTERM or SIGINT came it ends within its bound. The records of a call
(`status.json`, `request.json`, `decide-request.json`, the call lists `llm-calls.jsonl` and `calls.jsonl`) and the lines
it follows (`events.jsonl`, `rounds.jsonl`) are read for at most 64 MiB, and one read of a log takes at most 64 MiB. A
file of a root (a product file) is read for at most 64 MiB; of a longer file the first 64 MiB are hashed with its length
(`docs/fs-changes.md`), and once the watcher is stopping a read over 1 MiB ends and nothing is recorded for that file. Of
its own records, read through the held output directory as regular files with one name: a stored blob at most 1 MiB, the
pid record at most 64 bytes, `snapshot.json` at most 64 MiB, and on restart `changes.jsonl` at most 1 GiB (a longer
record ends the start with an error, so no `seq` is given twice; a line over 64 MiB is skipped). A link or a pipe at the
name of a blob is replaced when the blob is stored, and is never read into a diff.

Nothing sensitive is written: a string that starts with the run directory's path is written relative
to it, a string that still holds an absolute path (`/home/`, `/Users/`, `/root/`, `/tmp/`, `/var/`,
`/run/user/`, `/dev/shm/`) is written as `<absolute path removed>`, and the value of a member whose
name holds `login`, `credential`, `secret`, `password`, `api_key`, `authorization` or `bearer`, or is
`token` or ends in `_token`, is written as `named` whatever it held. No file reads a login file, the
environment of the runner or the `command` of a decision (it names the store path).

## The runner and the step in flight

Two records say what a run does while it runs. They give no program's output and no environment, argv or
path outside the run directory.

`runner.json` is written when the run directory is made, before the first step, and rewritten through a
temporary file and a rename, so a reader never sees half of it. The temporary file is created as new, under a name the
runner chooses for each write (a dot name that the inspector never lists), and what lies at `runner.json` is replaced by
the rename: a pipe or a link that a program of the run left at a temporary name of the runner's, or put at `runner.json`,
neither holds the runner nor receives the record. Every record the runner writes the same way (`outcome.json`,
`tree-status.json`, `tree-verify.json`) holds the same:

```json
{"format": "runbook.runner/1", "pid": 48213, "start_ticks": 91827364, "started_at": 1795300000, "closed_at": null}
```

`pid` is the process of `runbook run`. `start_ticks` is the start time the kernel gave that process (field 22 of
`/proc/<pid>/stat`, clock ticks since boot): a pid the kernel gives to another process later has another start time,
so a reader compares both and never takes a recycled pid for the runner. `closed_at` (whole seconds) is null while
the runner lives; the runner sets it when the run ends by any exit after the run directory exists (a closed tree, a
refusal, a failed run, a signal it handles: SIGTERM and SIGINT). The file is kept, not removed. A runner killed by
SIGKILL or lost with its machine cannot close it: its `closed_at` stays null, and what tells is the process: a pid that
no process has, a process that has another start time, and a process that has ended and has not yet been collected by its
parent (state `Z` or `X` in `/proc/<pid>/stat`: a runner killed by SIGKILL and not yet waited for) are not the runner.

`events.jsonl` has one more line, `action_started`, written when an action starts and before it ends (one append,
whole line):

```json
{"at": 1795300042, "event": "action_started", "source": "program_start", "role": "program", "case": "CASE-1", "action": "notes.write", "try": 2}
```

`at` is whole seconds, as in every line of the file. The runner appends the line only to a regular file with one name
that it opens without following a link and without waiting for a reader, and never creates it. When `events.jsonl` is
anything else (a pipe, a link, a file with a second name, or gone), the line is not written, the run goes on and the next
program starts at its time; the inspector, which reads only a regular file with one name, then shows the run with no step
(*between steps* while the runner lives). What is known and what is derived depends on `source`:

| `source` | when | `role` | `case`, `action`, `try` |
|---|---|---|---|
| `program_start` | in-process Loom (no `--loom`): the runner's own confinement runner is called for a bound program, just before the program starts | `program`, or `check` for a read-only program (a handoff check) | derived by the runner, see below; each may be `null` |
| `step_call` | a `b10x-loom` child (`--loom`): the runner starts `tree run --max-steps 1`, the unit it can see | `null` | all `null`: Loom's child names no action before it ends |

The pinned Loom gives no event before a bound program starts: its run log writes `ActionFinished` when the action ends
(`Loom logs no start`, below), and the one point before a program runs is the call of the confinement runner with the
command about to start. The `program_start` line is written from that point and from what the runner knows, so three of
its fields are derived, not read from Loom:

- `case` is the `LOOM_CASE` variable of the command's environment (the case Loom runs it for); `null` when absent.
- `action` is the action of the bindings copy of the run (`bindings/*.yaml`, `executor.kind: program`, or the
  `handoff_when` check) whose `argv` the command runs (the program is argv[0] by name or by its path's end, and the
  arguments are argv[1..]); `null` when no binding or more than one binding matches. It is a match by command line, not
  Loom's own naming.
- `try` is counted by the runner: the number of `program_start` events of the same case and action so far, this one
  included, for a program that is not a check (a check does not count a try); `null` when case or action is `null`.

There is no record of the last line of the step in flight. The pinned Loom reads a bound program's standard output and
error in its own process through pipes, keeps the last bytes of each in memory and returns them when the program ends
(`run_bounded` of `loom-intake-slice/src/executor.rs`); the confinement runner returns them the same way. The only file
that grows while a step runs is the model tool's record under `work/.sdc/llm/<call>/` (`events.jsonl`), a dot-directory
that the inspector never serves. The inspector therefore gives no last line: it would need a file under `work/`
without a dot component that a bound program grows while it runs, or a streaming capture in Loom.

The reader of the files (the inspector) says the run is *running* when `runner.json` is open, its pid has its start
time, and the last of `action_started`, `step_made` and `ended` in `events.jsonl` is an `action_started`; *between
steps* when it is open and alive and the last is not; *not live* when it is closed, or no process of that pid has that
start time. A run directory without `runner.json` (an older runner) keeps the answer it had.

`runner.json` and the `action_started` lines are what files of the run directory claim: under `--confinement local` a
program of the run can write them, so the answer is a reading of those files and of `/proc/<pid>/stat`, shown as text,
and nothing decides on it. A read of a file of a run by the inspector holds its limit in the read itself: it takes at most
the limit plus one byte from the one checked descriptor, whatever size the file has at that moment, and a file that
needs more is answered as a file over the limit (not read), never as its first bytes. A `runner.json` over 4 KiB (a record is about 150 bytes) is not read: the run is *unknown*.
`start_ticks` of 0 or below is no start time: the run is *not live*. A read of `/proc/<pid>/stat` that fails for another
reason than "no such process" makes the run *unknown*, with that reason, and never "not live". The `action`, `case` and
`source` of a start line are cut to 256 characters in the answer and the band, at a character, never inside one.

## Where each member comes from

Contract: `driver/src/run_metrics.rs` `load` and `load_case` (the table of unit R4's
`run-dir-contract.md`). "Run log" is `store/runlog-case-<hex>.jsonl` of the case.

| file | member | source in a Loom run |
|---|---|---|
| `store/<root>/events.jsonl` | the file | one per case; the root is `store/<root case id>/`, a child `.../children/<case id>/` below its parent (from `ChildOpened`) |
| | `opened` `protocol.id`, `.revision` | run log `CaseOpened.protocol` (`sdc-intake@9`); the id from `protocols/*.protocol.yaml` (`protocol.id`), else `sdc-<stage>` read back as `sdc.<stage>` |
| | `action_started` `action` | run log `ActionFinished.action`: Loom logs no start, so start and finish are written together |
| | `action_started` `llm` | the binding's `executor.argv[0]` is `sdc-node` (`bindings/<protocol>.yaml` of the run) |
| | `action_started` `retry_of` | `tree-status.json` `cases[].runs[n].retry_of` of the n-th program run of the case; empty until a status exists |
| | `action_finished` `action`, `exit` | run log `ActionFinished.action`, `.exit` (written as text) |
| | `action_finished` `principal` | run log `AuthorityRecorded.principal` (decision `Allow`) before the `ActionFinished` of a step that is not a program |
| | `action_finished` `evidence` | run log `ActionFinished.evidence` |
| | `terminated` `outcome` | run log `Terminated.reason`; for the root, `Ended.reason` when the log has none |
| `store/<root>/observed.jsonl` | `index`, `observed_at` | the runner's clock: a line first seen after a step is observed in that step's span (start = the end of the event before it, end = `StepMade`); a decision's lines in the span from `DecisionAsked` to `DecisionAnswered`; whole seconds |
| `.../ir.json` | `actions.<a>.effect` | `protocols/*.protocol.yaml` `actions.<a>.effect`; the file is absent when no protocol file is in the run directory |
| `.../bindings.json` | `<a>.manual`, `<a>.copy` | binding `executor.kind` `person` is `manual`, `copy` is `copy` |
| `.../evidence/<id>.json` | `kind`, `result` | `tree-status.json` `cases[].evidence[]` (`evidence_id`, `kind`, `result`, `subject`, `subject_revision`, `producer`, `action`); absent until a status exists |
| `.../work/findings.yaml` | the file | a copy of `work/findings.yaml` of the workspace |
| `llm-calls.jsonl` | one line per costed step call | `CostedCall` of source `Step`: `record` is the line the model tool wrote (`work/.sdc/llm/calls.jsonl`: `action`, `case`, `started`, `model`, `effort`, `duration_ms`, `usage`, ...) cleaned; `cost_usd` is the booked cost (`cost_micro_usd / 1e6`) |
| `decisions.jsonl` | one line per answered decision | `DecisionAnswered`: `time`, `case`, `action`, `capability`, `kind`, `revision`, `subject` from the decision; `class` and `type` `approval` (`close` for a `Close`); `decision` `approve` or `reject`; `principal`; `cost_usd`; `model_calls`; `model` is the decider's model name when `model_calls` > 0 and absent (`null`) for a rule's answer, taken from `settings.decider.model`, else the last `CostedCall` of source `Decider` (`record.model`), else the word `unknown` |
| `status.json` | `state`, `outcome`, `delivered` | at `Ended` when the tree ended: `closed`, the tree's outcome, and `delivered` for the outcomes `operating`, `fixed` and `delivered` (the driver's rule, with the name `story:root-outcome-delivered` gives the root) |
| `run-stopped.json` | `at`, `by`, `cause`, `case`, `reason` | `CapReached` or an `Ended` that is not `ended`; `by` is `rule:loom-run`; `cause` is `spend_cap`, `llm_calls_spent`, `wall_time_spent`, `signal`, `decider_cannot_decide`, `stuck`, `ended_without_outcome` or `error`, or, when the outcome is `refused` (exit 12), the reason's name in snake case, one of 29 (`tree_stopped`, `person_program_failed`, `bindings_refused`, ...; the list is in `docs/loom-run.md`), or `program_signalled` or `program_error` (the same `cause` is in `outcome.json`); `case` the case of the last new run log line, else the root; `reason` the cap's detail, cleaned, at most 600 characters |
| `rounds.jsonl` | `seconds`, `verdict`, `started_epoch`, `ended_epoch` | one line per `StepMade`: its span on the runner's clock, `verdict` the stop name in `snake_case` (`awaiting_person`) |
| `run-settings.jsonl` | the last line | `RunStarted.settings` as given, cleaned, with `format`, `at` and `leg` `loom`; `check --runs-dir` reads `loom.sha256`, `loom.confinement`, `loom.cap_micro_usd`, `max_llm_calls`, `max_wall_seconds`, `models.<role>.model`, `decider.model` |
| `loom/status.json` | the file | `tree-status.json` with `store` written `store`, `workspace` `work` and every other string that starts with either path rebased, then cleaned |
| `loom/verify.json` | the file | `tree-verify.json`, cleaned |

`b10x-loom tree open` refuses a store directory that holds anything but its lock files, so the writer
makes `store/` empty when it opens and writes `store/<root>/` only after a `StepMade`, which is after
`tree open`. Loom reads its own files by name (`case-<hex>.json`, `runlog-case-<hex>.jsonl`, ...) and
`tree verify` accepts a store with the extra `store/<root>/` directory.

A run that is killed leaves: every line written so far, the case store as of the last finished step
(the projection runs after each `StepMade` and after each answered decision), and no `status.json`
and no `run-stopped.json`, so the report reads it as `open` (not finished, no summary). The reader
needs `store/<root>/events.jsonl` and the writer makes it after the first step that finds the root's
run log. Before that, `report` fails with `cannot read ...events.jsonl`.

## Not recorded by a Loom run

A member with no source in a Loom run is absent (the file or the member), never a made-up value.

| member | why | measure of the report it affects |
|---|---|---|
| `defects.jsonl`, `status.json` `forced`, `unstuck`, `reopens`, `run-stopped.<at>.json` | `runbook run` has no keep-going mode, no unstick and no reopen | `run.clean` is `closed` alone; the driver runs' `unstick` confound is absent; the clean-pass count is not the same test |
| `events.jsonl` `revised`, `authority_recorded` | Loom's run log has no such line | rework cause labels `decision:<action>` and `arrival:outside` read `unexplained` (Loops table, edge names); causes through `retry_of` are unaffected |
| `action_finished` `not_called`, `refused.kind` | Loom has no not-called action and no run-own postcondition; a model node that was not called exits 79 (`exits.not_called`), which Loom records as a run of class `not_called` (observation `loom/bound-program`), not as a result | `not_called` is 0; the cause `<kind>:refused` cannot occur |
| start of an action | the run log has one line, at the finish | node `driver_s`, `loop_driver_s` and `loop_wall_s` come from the step's span: every action of one step has the same span (not the totals `driver_s` and `wall_s`) |
| `decisions.jsonl` `duration_ms`, `usage`, `cites`, `reasons`, `invalid_reply`, `escalation_suppressed`, `attempt` | the decider returns the answer, the cost and the number of model calls | `run.decider_s` 0; `decision.<class>.invalid_replies`, `retries`, `escalated`, `refused_by_check` 0; `run.decider_calls` counts answered decisions with a model, once each whatever `model_calls` says |
| `rounds.jsonl` `case_dir` | `tree run --max-steps 1` names no case dir | none (`waiting_on_person_s` is 0: one leg, and the decider answers inside the run) |
| `oracle.json` | written only by `runbook run --conformance observe`, after a run that ended closed (`runbook/src/loom_run/conformance.rs`, `docs/conformance.md`): absent without the option, and for a run that did not close | without the file `conformance` reads `not observed` |
| `decision.json`, `replay.json`, `run.log`, `decider/`, `llm/` at the top | the driver's own files | none read by the report (`sdc-run-timeline` reads `decider/`); Loom's tool records are in `work/.sdc/llm/` |
| `run-settings.jsonl` `rework_limit`, `rule_approvals`, `unstick`, `tree` | driver settings with no Loom counterpart | none: `check --runs-dir` compares only the keys the record sets for a `loom` variant. `adapter`, `autonomy`, `decider.*` and `review.adapter` are the settings file as the runner read it (one format, `docs/loom-run-settings.md`), so they are in the line, and `check --runs-dir` passes on a run the launcher started |
| `run-settings.jsonl` `loom.bin`, `loom.login_file` | never recorded: a path of the binary, a login | not compared |
| `evidence/<id>.json`, `ir.json`, `bindings.json` | `tree-status.json` is written at the end; the protocol files are in the run directory only when R1 copies them | until then: rework causes read `<kind>:?`, every node reads as kind `other`; see "What R1 must hand over" |

## The two evaluation tools

`sdc-finding-kpis <run dir> [--json]` (`docs/kpis.md` § 4) and `sdc-run-timeline <run dir>`
(`docs/llm-is-not-the-runtime.md`) read this directory as they read a run directory written by the former driver (below),
with the same reader for the case store as `sdc-experiment report` (`driver/src/run_metrics.rs`
`root_case_dir`, `case_dirs`). The kind is read from the files (`driver/src/run_kind.rs`), no flag:

- `settings.json` in the run directory: a Loom run (the loop writes it first);
- else `store/<root>/events.jsonl`: a driver run (`<root>` is `IDEA-1`, else the one directory of
  `store/` that holds an `events.jsonl`);
- neither: exit 2 and one line that names `settings.json` and `store/<case>/events.jsonl`.

Both print the same shape for both kinds. A figure a Loom directory has no source for is `null`,
never 0 and never a guess; in `sdc-finding-kpis` its object carries a `why`, in `sdc-run-timeline`
the document's `notes` say so. `sdc-run-metrics` and `sdc-experiment report` read the same absent
members as 0 (`run.clean` is `closed` alone, above): the two tools here say "no source", those say
"none occurred".

### `sdc-finding-kpis`

| KPI | source in a Loom run | under Loom |
|---|---|---|
| L1 `person_actions` | `store/<root>/**/events.jsonl` `action_finished.principal` (run log `AuthorityRecorded`) | counts `action_finished` only: Loom's run log has no `authority_recorded` event |
| L1 `person_answers` | the product tree's `work/questions.yaml`, `findings.yaml`, `context.yaml` (the driver reads its copy in `store/<root>/work/`) | as the driver; a missing file counts 0 |
| L1 `stand_in`, `stand_in_by` | `run-settings.jsonl` has no such member | `null` |
| L2 | `rounds.jsonl` | as the driver |
| L7 | `rounds.jsonl`, `run-settings.jsonl`, `run-stopped.json`, and `status.json` `state: closed` where the driver has `decision.json` | as the driver; one leg, so a stop never waits on a person |
| `forced`, `unstuck`, `stopped_for_fix`, `clean_run` | `defects.jsonl` and `run-stopped.<at>.json` are never written | `null` (`reopens`, `seconds`, the `forced`, `unstuck` and `reopens` of `clean_run` too), with `why` |
| L3, L4, L5 | `work/findings.yaml`, `questions.yaml`, `context.yaml` | as the driver |
| L6 | `events.jsonl` `revised` is never written | `value` `null`, `by_artifact` `{}`, with `why` |

### `sdc-run-timeline`

| field | source in a Loom run | under Loom |
|---|---|---|
| tree, program and rule spans, times | `store/<root>/**/events.jsonl`, `observed.jsonl` | a take's start is the start of its step: Loom logs one line, at the finish |
| take `cause` | `evaluated` events and the decision files they name | `null` on every take: Loom's run log has no evaluation record (`ir.json` has the effect only) |
| model spans | `llm-calls.jsonl` joined to the take; call records in `work/.sdc/llm/<call>/` (`status.json`, `events.jsonl`) | `api_requests`, `tool_calls`, `tool_calls_denied` are `null` for a call whose record directory is missing; `sessions` 0 and no cost for a model step that started no adapter |
| decider spans | `decisions.jsonl` lines that name a model, call records in `decider/<call>/` | as the driver; a line without a record directory takes its time from `time` and has no duration |
| idle spans, `waiting_on_person_s` | `rounds.jsonl`, `run-settings.jsonl`, `run-stopped.json`, `status.json` | as the driver (0 for a person: the decider answers inside the run) |
| `settings` | last line of `run-settings.jsonl` | `efforts` is what the settings file gives (`{}`: Loom runs the class defaults); `max_llm_calls`, `max_wall_seconds`, `autonomy` as the settings file says |
| `end` | `status.json` | `null` while the run is open |

`--check` holds the cost to `sdc-run-metrics`' for a Loom run too. The tests are
`driver/tests/finding_kpis.rs` and the last tests of `driver/tests/run_timeline.rs`, on the
hand-written directory `driver/tests/fixtures/loom-run/` (invented text; the member shapes are the
table above).

## What R1 must hand over (`requests-to-r1.md`)

The writer is built on the interface as it is. Four members it needs from the loop are not there yet,
and until they are the cells in the table above are absent:

- `RunStarted.settings` carries `loom.{sha256, confinement, cap_micro_usd}`, `max_llm_calls`,
  `max_wall_seconds`, `case` and `root_protocol`. Without them `check --runs-dir` refuses the run.
- `protocols/*.protocol.yaml` of `--protocols` are copied into the run directory. Without them there is
  no node kind, and `looping_factor`, `first_pass_yield`, `rework_cost_usd` and the Loops table are
  not computed.
- `tree-status.json` names absolute paths (store, workspace, case files, run logs) and so may
  `tree-verify.json` and Loom's own `store/tree.json`; `loom/status.json` and `loom/verify.json` do not.
- Optionally `tree-status.json` after every step.

## `runbook parity summarize` reads the directory

`parity summarize --loom-run <run dir>` reads the directory whole:

```
runbook parity summarize --loom-run <run> --output <summary.json>
```

It takes `tree-status.json` and `work/` (the product tree) and, for what the status does not hold,
`run-settings.jsonl` (the first line: decider kind, adapter, stand-in flag and principal),
`decisions.jsonl` and `protocols/*.protocol.yaml` (the digest of the file per id and revision); the
copies' action, subject and revision come from the target case's evidence in the status. A summary
read with `--loom-status` and `--product-tree` alone still works and lists those members as not
recorded (`docs/parity.md`, "Where the seven values come from now"). The test
`the_run_directory_names_the_files_of_parity_summarize_and_the_counts_line_is_the_fixtures` makes the
directory with the writer from the committed fixture status and gets the fixture's counts line,
`summary: 3 cases, 9 evidence records, 5 program runs, 2 person steps, 2 product files`.

## Quality measures of the comparison

`story:parity-run-on-loom` compares each quality measure of the Loom runs with the three driver runs
of `release-0.1.2` (strong). The rows of `experiments/release-0.1.2/report.md`, and whether the
directory above gives each one. "Needs protocols" means the member needs `protocols/` of the run
(request 2 to R1); "needs status" means it needs `tree-status.json`, which exists once the run ended.

| measure (report row) | computable from a Loom run directory | file and member; what is missing |
|---|---|---|
| runs table: `end` | yes, the words differ | root `terminated.outcome` (`closed: operating`, or `closed: delivered` when the tree names it so) or `run-stopped.json` `cause`; the driver's stop causes (`writer_spent`) are the driver's, Loom's are `spend_cap`, `stuck`, ...: a stop is compared as stopped, not by its word |
| runs table: `clean pass` | yes, a weaker test | closed, delivered (`status.json`) and `clean`; no unstick, no forced step, no reopen can occur in a Loom run |
| runs table: `cost USD`, `run.cost_usd` | yes | `llm-calls.jsonl` `cost_usd` (booked cost of each step call) plus `decisions.jsonl` `cost_usd`; a call the ledger does not know stops the run |
| runs table: `wall s`, `run.wall_s` | yes | `observed.jsonl` of every case, last minus first, whole seconds of the runner's clock |
| runs table: `looping factor`, `run.looping_factor` | yes, needs protocols | per llm-writer node: takes of one action in one case; `llm` from the bindings, `effect` `write` from the protocol files |
| `runs listed`, `ended`, `clean passes`, `run.clean` | yes | derived by `sdc-experiment` from `finished`, `closed`, `delivered`, `clean` |
| `run.llm_calls` | yes | line count of `llm-calls.jsonl` (step calls; the decider's are apart, as in the driver) |
| `run.decider_calls` | partly | `decisions.jsonl` lines with a model: one per answered decision; a decision that took two model calls counts once (`model_calls` holds 2); the decider's escalations and invalid replies are not lines |
| `run.driver_s` | yes | sum of `rounds.jsonl` `seconds`: time inside `tree run` steps, decider time excluded, as the driver's rounds exclude the decider's |
| `run.real_takes` | yes | actions finished, over every case of the tree, `exit` not 75, 126, 127; approvals and copies are takes in both runtimes |
| `run.rework_takes` | yes | a take of an action already taken in the same case |
| `run.first_pass_yield` | yes, needs protocols | writers taken once over writers taken |
| `run.rework_cost_usd` | yes, needs protocols | calls joined to takes by `started` inside the step's span; a call outside every span of its action is counted as unmatched in the node, not lost from `run.cost_usd` |
| `run.furthest_stage_index` | yes | stage of the protocol id of every opened case (`opened.protocol.id`) |
| `run.findings_raised` | yes, when the product holds `work/findings.yaml` | the copy at `store/<root>/work/findings.yaml` (`findings[]`, `status` not `rejected`) |
| `run.waiting_on_person_s` | trivially 0 | one leg, the decider answers inside the run: the measure says that no person waited, which the driver's A5 runs say too |
| Loops: node `looping factor`, `rework takes`, `rework cost` | yes, needs protocols | as the run rows, per node |
| Loops: edges `<check>:<result> -> <writer>` (kind check, approval) | yes, needs status and protocols | `retry_of` of the retaking program run, the result of the evidence of that kind (`evidence/<id>.json`); `loop_driver_s` from the steps' spans |
| Loops: edges `finding@<stage> -> <stage>` | yes, when the product holds `work/findings.yaml` | `findings[].stage`, `resolution.amends[]`; their loop cost needs the same joins |
| Loops: causes `decision:<action>`, `arrival:outside` | no | no `revised` event; the cause reads `unexplained`. Loom's run log holds `Evaluated` lines with `case_revision`: a later version could write `revised` where that revision changes after a person's step |
| Settings: `models.<role>.model`, `decider.model`, `decider.kind` | yes | `run-settings.jsonl` from the run's settings (the decider's settings are R2's) |
| Settings: `max_llm_calls`, `max_wall_seconds` | yes, needs request 1 | `run-settings.jsonl` |
| Settings: `adapter`, `autonomy`, `decider.adapter`, `decider.escalate`, `decider.max_decider_calls`, `review.adapter` | no | driver settings; Loom has levels of confinement and a budget per action, not autonomy levels. The settings file may carry them as text (then they are copied); nothing is derived |
| not a row of the report, but of the metrics: `conformance`, `decider_s`, per-take duration | no | `oracle.json` is the driver's conformance run; Loom's `tree run` and the decider give no per-call duration of the answer (request 5) |

What the comparison can say honestly: cost, calls, wall and driver time, takes, rework, looping and
first-pass figures, stages reached, findings and the clean pass are computed from the same member
shapes, by the same code (`run_metrics::load`), from a Loom run; once `protocols/` is in the run
directory the node figures are too. The stop causes, `waiting_on_person_s`, the unstick-free clean
pass and the decider's escalation statistics are not comparable in kind: the comparison states them as
such.
