# The run directory of a Loom run

`runbook loom-run` writes one directory per run, `<runs-dir>/<name>/`. This page says which files in
it `sdc-experiment report` and `runbook parity summarize` read, where each value comes from in a Loom
run, what a Loom run does not record, and which quality measures of
`experiments/release-0.1.2/report.md` the directory can give. The writer is
`runbook/src/loom_run/run_dir.rs`; the options and the loop are in `docs/loom-run.md`.

```
<run>/settings.json      redacted settings (the loop)
      events.jsonl       the loop's events (the loop)
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

Nothing sensitive is written: a string that starts with the run directory's path is written relative
to it, a string that still holds an absolute path (`/home/`, `/Users/`, `/root/`, `/tmp/`, `/var/`,
`/run/user/`, `/dev/shm/`) is written as `<absolute path removed>`, and the value of a member whose
name holds `login`, `credential`, `secret`, `password`, `api_key`, `authorization` or `bearer`, or is
`token` or ends in `_token`, is written as `named` whatever it held. No file reads a login file, the
environment of the runner or the `command` of a decision (it names the store path).

## Where each member comes from

Contract: `driver/src/run_metrics.rs` `load` and `load_case` (the table of unit R4's
`run-dir-contract.md`). "Run log" is `store/runlog-case-<hex>.jsonl` of the case.

| file | member | source in a Loom run |
|---|---|---|
| `store/<root>/events.jsonl` | the file | one per case; the root is `store/<root case id>/`, a child `.../children/<case id>/` below its parent (from `ChildOpened`) |
| | `opened` `protocol.id`, `.revision` | run log `CaseOpened.protocol` (`sdc-intake@9`); the id from `protocols/*.protocol.yaml` (`protocol.id`), else `sdc-<stage>` read back as `sdc.<stage>` |
| | `action_started` `action` | run log `ActionFinished.action`: Loom logs no start, so start and finish are written together |
| | `action_started` `llm` | the binding's `executor.argv[0]` ends in `llm-node.sh` (`bindings/<protocol>.yaml` of the run) |
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
| `run-stopped.json` | `at`, `by`, `cause`, `case`, `reason` | `CapReached` or an `Ended` that is not `ended`; `by` is `rule:loom-run`; `cause` is `spend_cap`, `llm_calls_spent`, `wall_time_spent`, `signal`, `decider_cannot_decide`, `stuck`, `ended_without_outcome` or `error`; `case` the case of the last new run log line, else the root; `reason` the cap's detail, cleaned, at most 600 characters |
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
| `defects.jsonl`, `status.json` `forced`, `unstuck`, `reopens`, `run-stopped.<at>.json` | `runbook loom-run` has no keep-going mode, no unstick and no reopen | `run.clean` is `closed` alone; the driver runs' `unstick` confound is absent; the clean-pass count is not the same test |
| `events.jsonl` `revised`, `authority_recorded` | Loom's run log has no such line | rework cause labels `decision:<action>` and `arrival:outside` read `unexplained` (Loops table, edge names); causes through `retry_of` are unaffected |
| `action_finished` `not_called`, `refused.kind` | Loom has no not-called action and no run-own postcondition | `not_called` is 0; the cause `<kind>:refused` cannot occur |
| start of an action | the run log has one line, at the finish | node `driver_s`, `loop_driver_s` and `loop_wall_s` come from the step's span: every action of one step has the same span (not the totals `driver_s` and `wall_s`) |
| `decisions.jsonl` `duration_ms`, `usage`, `cites`, `reasons`, `invalid_reply`, `escalation_suppressed`, `attempt` | the decider returns the answer, the cost and the number of model calls | `run.decider_s` 0; `decision.<class>.invalid_replies`, `retries`, `escalated`, `refused_by_check` 0; `run.decider_calls` counts answered decisions with a model, once each whatever `model_calls` says |
| `rounds.jsonl` `case_dir` | `tree run --max-steps 1` names no case dir | none (`waiting_on_person_s` is 0: one leg, and the decider answers inside the run) |
| `oracle.json` | the conformance observation is the driver's | `conformance` reads `not observed` |
| `decision.json`, `replay.json`, `run.log`, `decider/`, `llm/` at the top | the driver's own files | none read by the report; Loom's tool records are in `work/.sdc/llm/` |
| `run-settings.jsonl` `adapter`, `autonomy`, `decider.adapter`, `decider.escalate`, `decider.max_decider_calls`, `review.adapter`, `rework_limit`, `rule_approvals`, `unstick`, `tree` | driver settings with no Loom counterpart | `check --runs-dir` compares only the keys the record sets for a `loom` variant (unit R4); a record that sets one of these reads `not recorded` |
| `run-settings.jsonl` `loom.bin`, `loom.login_file` | never recorded: a path of the binary, a login | not compared |
| `evidence/<id>.json`, `ir.json`, `bindings.json` | `tree-status.json` is written at the end; the protocol files are in the run directory only when R1 copies them | until then: rework causes read `<kind>:?`, every node reads as kind `other`; see "What R1 must hand over" |

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

`parity summarize` takes `--loom-status <file>` and, for the product tree, `--product-tree <dir>`
(the status's `workspace` when none is given). The directory holds both:

```
runbook parity summarize --loom-status <run>/loom/status.json --product-tree <run>/work --output <summary.json>
```

`loom/status.json` names the workspace as `work`, relative to the run directory, so `--product-tree`
is always given. The test `the_run_directory_names_the_files_of_parity_summarize_and_the_counts_line_is_the_fixtures`
makes the directory with the writer from the committed fixture status and gets the fixture's counts
line, `summary: 3 cases, 9 evidence records, 5 program runs, 2 person steps, 2 product files`.

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
