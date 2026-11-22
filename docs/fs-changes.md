# File changes of a run

The operator, 2026-11-21: the live view shows the effective change of every command, whoever made it (an
agent's Edit tool, a script it wrote, a formatter, a linter, the apply step), as a colorized,
syntax-highlighted diff, attributed to the command that caused it. Tracking is by filesystem
events, not by tool names.

## Layers

- Prototype (here): `sdc-watch` (Rust, `sdc/watch/`) watches a run directory with inotify and
  records each effective change with its attribution. Attribution is by time window: exact per
  driver action (per `tree run` step under `runbook run`), inferred per tool call inside one agent
  call (agent tools run one at a time).
- Target (Substrate): every process runs through Substrate, so the change record belongs there,
  per exec id, exact for concurrent processes too. Requested from the `substrate` session
  2026-11-21. Loom's run log then references the exec's change record; `sdc-watch` retires.

## Record: `runs/<run>/fs/changes.jsonl`

Append-only, one JSON object per line, ordered by `seq`. Content in `runs/<run>/fs/blobs/<sha256>`.

```
{
  "format": "sdc.fs-change/1",
  "seq": 17,
  "at": "2026-11-21T10:02:03.412Z",
  "root": "store/IDEA-1/work" | "llm/007-change.implement/cwd" | ...,
  "path": "impl/src/lib.rs",
  "op": "create" | "modify" | "delete" | "rename",
  "from_path": null,
  "before": "<sha256>" | null,
  "after": "<sha256>" | null,
  "binary": false,
  "too_large": false,
  "lines_added": 12 | null,
  "lines_removed": 3 | null,
  "diff": "<unified diff, 3 lines context>" | null,
  "attribution": {
    "case": "IDEA-1.todolist.storage" | null,
    "action": "change.implement" | null,
    "record": "llm/007-change.implement" | null,
    "tool": {"seq": 41, "name": "Bash", "input": "rustfmt src/lib.rs"} | null,
    "basis": "action-window" | "tool-window" | "record" | "none",
    "step": 3
  }
}
```

`step` is in a record of a Loom run only (see "What is attributed under each runner"), where `basis`
is `step-window`, `tool-window` or `record`.

- `root` is the watched tree the path is relative to: a case's `work/` or an agent call's `cwd/`.
  Only files under a root are recorded: `store/<C>/work/` (a child case's `work` is a symlink to
  its parent's and is not walked, so the whole case tree shares one root), `llm/<NNN-action>/cwd/`,
  `decider/<NNN-id>/cwd/`, and `work/.sdc/llm/<NNN-action>/cwd/` (the agent-call record place when
  `SDC_LLM_RECORD` is unset; the innermost root wins). Everything else in the run dir (run.log,
  prompts, case.json, ...) is read for attribution or ignored, never recorded.
- `op`: `rename` only when a file the shadow knew disappears and reappears under another path of
  the same root with one `mv`; `before` is the content at `from_path`. A rename over an existing
  file is a `delete` of the target followed by the `rename`. A write to a new temp file renamed over
  the target (`sed -i`, editors, atomic writers) is a `modify` of the target, and the temp file
  leaves no record. A rename across roots is a `delete` plus a `create`.
- One record per path per burst: events on a path are debounced until it is quiet for 100 ms (at
  most 2 s); `at` is the burst's first event. A path that ends where it started yields no record.
- `binary` (a NUL byte in the first 8 KiB, or not UTF-8) and `too_large` (either side over 1 MiB)
  have `diff`, `lines_added` and `lines_removed` null. A too-large content is hashed but not
  stored in `blobs/`. A rename with unchanged content has `diff` null and 0 lines.
- `basis`:
  - `tool-window`, inferred: the change's wall time lies in a tool call's [`tool.requested.at`,
    `tool.result.at` + 250 ms] in the agent call's metaharness stream (`<record>/events.jsonl`);
    a call without a result yet is open. When several windows hold it, the one whose input names
    the file wins, else the call requested last.
  - `action-window`, exact by order: an action of a case of the tree was open between
    `action_started` and `action_finished` in its `events.jsonl`. Ordering is by inotify queue
    position (the case log's append and the file's write travel through the same queue), not by
    the log's second-resolution `at`. If several cases of a tree have an open action, the one
    started last wins. `record` is the agent call started inside that action for that case, if any.
  - `record`: the change is in an agent call's `cwd/` whose case has no matching open action and
    no tool window holds it (a decider call, which runs outside any action).
  - `none`: nothing was in flight; `case` and `action` are null.
- A record's case: from `decide-request.json` (`case_dir`, `action`) for a decider call; for an
  agent call from its `request.json` (`case`, written by the model node before the adapter runs,
  `docs/driver.md` "Observation records"), else from `llm-calls.jsonl` (`record` → `case`) once
  the call is finished. Only when neither names it (a record from before 2026-11-21, or the first
  moments before `request.json` is written): the case whose open action equals the record's
  action, a guess that the request or the log replaces once either names the case.
- Watched: the whole run dir, recursively, new directories included. Not watched at any depth:
  `target/`, `.git/`, `node_modules/`, `transcripts/`; not watched: the output dir (`fs/`), and
  outside a root the case store's `evidence/`, `blobs/`, `evaluations/`.

## What is attributed under each runner

The kind of run directory is detected as `driver/src/run_kind.rs` does (or given with `--kind
driver|loom`): `settings.json` in it marks a Loom run, `runbook run`'s.

| | run directory of the former driver | Loom run (`runbook run --watch`) |
|---|---|---|
| roots | `store/<C>/work`, `llm/<NNN-action>/cwd`, `decider/<NNN-id>/cwd`, `work/.sdc/llm/<NNN-action>/cwd` | `work/` (the product tree of the whole run), `work/.sdc/llm/<NNN-action>/cwd`, `decider/<NNN-id>/cwd` |
| not product | the case store's `evidence/`, `blobs/`, `evaluations/` | `store/` (Loom's case store and the projection of it), the run's own files, and under `work/.sdc/llm/` everything outside a `cwd/`: a call record's `request.json`, `status.json`, `response.json`, ... and `calls.jsonl` |
| what was in flight | an action: `action_started` to `action_finished` of `store/**/events.jsonl` | a `tree run` step: the lines already in `rounds.jsonl` plus one |
| `case` | the case whose action was open, or the call's `request.json` | the open call's `request.json` (`case`) when it names one, else `null` |
| `action` | the open action | the open call's action (its directory name, `001-change.implement` is `change.implement`), `null` with no call open |
| `step` | not in the record | the step number, 1 for the first `tree run` |
| `basis` | `action-window`, `tool-window`, `record`, `none` | `step-window`, `tool-window`, `record` |

Why a step and not an action under the Loom runner: the engine's run log has one line per action,
written at its finish (`docs/loom-run-directory.md`, "Not recorded by a Loom run"), and the store
projection is rewritten after each step, so no start marker of an action exists while its program
runs. What the runner writes while a step runs is read instead: `rounds.jsonl` gets one line when a
step ends, and the call record of a model step (`work/.sdc/llm/<NNN-action>/`) has its `status.json`
(`running`, then `finished` or `failed`), `request.json` and the metaharness stream
`events.jsonl`.

- `step-window`, exact by order: the change's first inotify event came after the `rounds.jsonl`
  lines that made it step N and before the line that ends step N. A change made by a hand-off
  program between two steps (the person step the decider answered) falls in the step that comes
  next. A change of the final `tree status` after the last step is step N + 1 of a step that never
  was.
- With a call record open (`status.json` not yet `finished` or `failed`, the one opened last), the
  change carries that record and its action and case; the tool window applies as for a driver run
  and gives `tool-window`.
- A field a Loom run cannot give is `null`, and a reader shows the change: `ui/web/now.js` groups a
  change without case and action under its step (`ui/test/now-model.spec.mjs`).
- Limit: a `rounds.jsonl` line read late (the watcher lagging behind a fast step) moves the step
  boundary to where it was read, as a case log line does for a driver run.

## Late attribution: `sdc.fs-attribution/1`

The metaharness stream is written live: `metaharness run` prints each event with `println!` as it
polls it (`metaharness/crates/metaharness-cli/src/lib.rs:507` `poll_event`, `:557-559` `emit`;
Rust's stdout is line-buffered), and the adapter redirects that stdout straight into the record
(`sdc/bindings/tools/llm-adapter-metaharness.sh:44`). In `runs/trial-todo-1` every
file the agent wrote has an mtime inside its `Write` call's [requested, result] window.
`tool.requested` carries `at` and `input`, `tool.result` carries `at`.

So a change in an agent call waits up to 1.5 s for the stream to reach the change's time. If it
does not (the stream lags), the change is written with what is known, and when the stream later
shows a tool call that held it, one line is appended; `changes.jsonl` is never rewritten:

```
{"format": "sdc.fs-attribution/1", "change": 17, "at": "<when it was learnt>",
 "attribution": {...same shape, "basis": "tool-window"}}
```

A reader applies it over the `attribution` of the change with `seq` = `change`. These lines have
no `seq`.

## Other files in `fs/`

- `snapshot.json` (`sdc.fs-snapshot/1`): every tracked file's sha256 when the first watch (or a
  `scan`) started, and `seq`, the first change it does not include. Snapshot plus the changes
  from `seq` on is the last known state.
- `sdc-watch.pid`: written once the initial scan is done and every dir is watched; removed on exit.
- A restarted `sdc-watch run` rebuilds the shadow from `snapshot.json` and `changes.jsonl`,
  records what changed while nobody watched (attributed as of start), and continues `seq`.

## CLI

- `sdc-watch run <run-dir> [--out DIR]`: foreground until SIGTERM/SIGINT, then flushes, exit 0.
- `sdc-watch scan <run-dir> [--out DIR]`: writes `snapshot.json` and blobs once; no record.
- `sdc-watch run <run-dir> --kind driver|loom`: the kind of run directory (default: `settings.json` in it
  marks a Loom run).
- `sdc-watch run <run-dir> --parent <pid>`: also ends, as on SIGTERM, when `pid` is no longer its parent
  (a parent-death signal and a poll every 200 ms); refused when `pid` is not the parent at start.
  `runbook run --watch` starts it so, with `--kind loom`: the watcher ends with the runner, also
  when that is killed with SIGKILL (`docs/cli.md`, `run --watch`).
- `scripts/watch-run.sh start|stop|status <run-dir>`: background start (returns once watching),
  stop by PID (waits for the flush). The driver's launcher uses it; `runbook run` does not.

## Limits of the prototype

- Tool attribution is by time: a background process started by an earlier tool, or a write by
  another tool call running in parallel, is attributed to whichever call's window holds it.
- An agent call's own `cwd/` is attributed to the case its `request.json` names, even when
  several cases have the same action open. In a case's `work/`, several agent calls running at
  once (or several cases of one tree with an open action) still cannot be told apart; the latest
  started wins. Without a `case` in `request.json` the record falls back to the open-action guess.
- A case log line read late (the watcher lagging behind a fast action) moves the action boundary
  to where it was read.
- A path written continuously for over 2 s is recorded in 2 s slices. Symlinks and special files
  are not tracked. Changes made while the watcher is starting are part of the initial state.
- inotify queue overflow is recovered by a full rescan; changes in it are recorded, but their
  ordering and attribution are as of the rescan.
