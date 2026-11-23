# The 130 entries of the old finding-cycle suite, and what holds each one now

`story:cycles-suite-as-data` made the finding cycles of `scripts/run-finding-cycles.sh` (each: a stub
setup, a run, an expected verdict) into data, `cycles/suite.yaml` (format `runbook.cycles/1`).
Loom's `loom.tree-suite/1` then took the cycles that can be said in it over, as
`cycles/tree-suite.yaml`. `story:driver-runtime-removed` deleted the script, `cycles/suite.yaml` and
the driver the script ran. This file stays as the table of the 130 entries of the deleted suite: each
row says what holds the entry now. The 130 names, and `spec-person-wait` (a cycle added after the deletion, so the list holds 131 names), are committed in `cycles/old-suite-names.txt`.

## What holds an entry now

Every entry is in one of four states. The `state` cell of its row says exactly these words
(`converted`, `pending`, `held by a Rust test`, `dropped with the driver`); a qualification such as
"in part" goes in the third cell:

- **converted**: a cycle in `cycles/tree-suite.yaml`, run through `runbook test`, as expected.
- **pending**: a cycle in the file marked `pending: true`. A cycle whose step or member the pinned
  Loom does not run yet prints `pending, waits for story:<id>`; a cycle that runs prints
  `pending, still failing: <difference>`, which is the evidence that the mark is right, and
  `pending, passing` when it can move to converted.
- **held by a Rust test**: a test of the runner (`runbook/`) or of a tool of `driver/` that the row
  names; the cycle is not in the file.
- **dropped with the driver**: the cycle tested a behaviour of the deleted driver (its autopilot loop,
  `--keep-going`, the unstick step, the rule approver, stand-in binaries through the environment, a
  second tool reading a finished run). `runbook run` does not have that behaviour, or has it in a form
  that no cycle of the suite file states. Decision: it is not ported with the removal of the driver, and
  no test of the runner named in the row holds it. The reason of the row keeps the class the entry had
  while the driver still ran it.

## The check

`runbook check` (run by `task check:cycles-suite`, part of `task check`) reads `cycles/tree-suite.yaml`
and fails when it is not format `loom.tree-suite/1`, repeats a cycle name, has an entry without a
`name`, or names a seed (`seed: [{from: ...}]`, in `defaults` or in a cycle) that is not under
`cycles/`. It reads `cycles/old-suite-names.txt` and fails when a cycle of the file is not one of the
131 names. While this file exists it reads the rows of its tables headed `| entry | state |` and fails
when a name has no row or more than one, when a row names an entry that is not in the list, when the state of a row is none of the four words
above (the refusal names the row and the word), when a row
says converted or pending and the suite file says otherwise, or when a row of another state names a
cycle that is in the file. Without this file the comparison is skipped and one note says so.
`RUNBOOK_HOME=<root>` points the check at another tree (the task defaults to this checkout). It also
checks the rest of the text layout: the parts `runbook doctor` lists, the bindings and the
instruction files they name, the placeholders of the instruction files, and the view files.

## The same cycles as a Loom tree suite: `cycles/tree-suite.yaml`

`runbook test cycles/tree-suite.yaml --confinement local` runs the file in the `runbook` process
(Loom's `run_suite`, format `loom.tree-suite/1`); the run holds `runbook` and the bound programs
only. `task test:cycles-loom` runs it; `task test:cycles-loom ONLY=<name>[,<name>...]` runs the named
cycles alone.

How the environment switches of the old driver are said in the file, with no environment passed
through:

- Fixture directories and the `SDC_STUB_FINDINGS` list are seeds into `.stub/` of the workspace
  (`.stub/finding.yaml`, `.stub/finding-2.yaml`, the rest of the directory is the fixture
  directory); `sdc-node` reads that directory when it runs under Loom.
- Seeds are copies in `cycles/seeds/`: Loom refuses a seed that is a symbolic link.
- `cycles/bindings` is a symbolic link to `../bindings`, because the suite's `bindings:` path stays
  inside the suite's directory.
- Approvals of person actions are `decide:` entries in `defaults`, bound to the principal
  `person:owner`.
- Scripted replies of a model node (`SDC_*_REPLIES`, `SDC_LLM_ADAPTER`, `SDC_LLM_NODES`) are seeds
  too, in the layout below. Loom has no `fixtures:` member and will not get one.
- What the old driver took from `amend.upstream`'s `handoff_when` is `handoff_when: [tools/sdc-tool.sh,
  findings-wait, --action, amend.upstream]` in the five `bindings/sdc-*.yaml` files that bind
  `amend.upstream` (Loom `5bcc26d9`). Only Loom reads these files; the old driver reads the JSON
  bindings, which keep their own `handoff_when`.
- A bugfix cycle's workspace is the one `sdc-bugfix seed` writes, kept as `cycles/seeds/bugfix-pager/`
  (`bug.yaml`, `checks.yaml`, `impl/`, `mainline/`, `aep/`); the model stubs come from
  `driver/examples/sdc/fixtures/bugfix-pager/`, and a cycle with its own stubs seeds
  `cycles/seeds/fixtures/bugfix-<name>/` into `.stub/`.

### The layout of `.stub/`

A bound program under Loom gets `PATH` and `LOOM_*` only, so what a cycle scripts reaches
`sdc-node` as files of the workspace. The tool reads the layout only under Loom and
only when `.stub/` exists, in one block that sets the variables of the old driver; a variable
already set wins, a run without `.stub/` is unchanged. Text and YAML only, no shell in the data. An
unknown key or file name is refused (exit 64).

| file | the old variable | what it holds |
|---|---|---|
| `.stub/finding.yaml`, `.stub/finding-2.yaml` | `SDC_STUB_FINDINGS` | the findings the stub raises, as before |
| `.stub/` (the directory) | `SDC_STUB_FIXTURES` | the fixture directory: trees such as `billing-spec.0`, files such as `plan-stories.json`, as before |
| `.stub/settings.yaml`, key `adapter` | `SDC_LLM_ADAPTER` | `fixture` (the fixture adapter beside the tool), `dry`, `stub`, or an adapter program |
| key `nodes` | `SDC_LLM_NODES` | a list of node names that go to the adapter; every other node runs its stub |
| key `review_adapter` | `SDC_REVIEW_ADAPTER` | as `adapter`, for the review class |
| keys `review_mode`, `implement_mode`, `implement_coverage` | `SDC_REVIEW_MODE`, `SDC_IMPLEMENT_MODE`, `SDC_IMPLEMENT_COVERAGE` | the value as text (quote `"on"`) |
| `.stub/replies/review.yaml` | `SDC_REVIEW_REPLIES` | the reviews the fixture adapter answers |
| `.stub/replies/implement.yaml` | `SDC_IMPLEMENT_REPLIES` | the replies of the implementer in request mode (`stub` is the stub tree) |
| `.stub/replies/coverage.yaml` | `SDC_COVERAGE_REPLIES` | the coverage documents of the implementer |
| `.stub/replies/fixture-stub.yaml` | `SDC_FIXTURE_STUB` | a node name to `[fixture, artifact]` for a node whose stub is another subcommand |

A reply file is YAML of the same shape as the JSON the variable held: a mapping `rounds:` whose
n-th member is the reply of the n-th earlier rejection or call, the last one after that; a member
that is a list is taken by the attempt inside the call (a reply refused by a program check is
asked again with the reason, and the next list member answers). A review reply is
`{decision, summary, findings}`; a coverage reply is `{requirements: [{id, status, at, test|reason}]}`.
The seeds are in `cycles/seeds/stub/`: `settings/<name>.yaml` and `replies/<name>.yaml`, composed
per cycle (`{from: seeds/stub/settings/review.yaml, to: .stub/settings.yaml}`). They are written by
hand from the old entries; no recording of a model call is a seed.

### Totals and time

131 entries (the 130 of the deleted suite and `spec-person-wait`): 41 converted, 14 pending, 39 held by a Rust test, 37 dropped with the driver (29 full-cycle, 9 re-drive, 9 other and 83 trial entries).
`scope-answer` moved from pending to converted at Loom `70b0e6e0` (`scope-changed` and `intake-decision` run further and stay pending).
At Loom `db2726ff` every `pending` entry was run once. `run-stop`, `replay-sibling` and `decider-rework` run as expected and lost
`pending` (`story:tree-suite-stop-resume-reopen-steps`, `story:tree-replay-command`). `decider-rework-spent` and
`rework-fresh-allowance` run to their end and still fail on `RunsSpent` (the rows below give the difference). The other pending entries
keep their difference; no entry changed from expected to another state. All 37 entries that were not pending were run again at a quiet
host (one-minute load under 50) and are as expected. Run together at a load of 100 to 177, 11 entries ended on Loom's 60 s wall bound
(exit 5), so the counts above are of single runs at a quiet host.
At Loom `c9b4c177` every `pending` entry was run once alone (`runbook test cycles/tree-suite.yaml --only <name>`) on a host whose one-minute load was 83 to 139 during those runs. What was observed, in the order of the run (seconds, one-minute load at the start):
`spec-person-wait` 51.0 s, 97, exit 3 `AwaitingPerson` (expected 0); `cite-fault-sent-back` 18.6 s, 92, exit 3 (expected 4);
`intake-decision` 29.1 s, 130, exit 3 (expected 0); `cite-dark-spot` 19.5 s, 120, exit 3 (expected 0);
`bugfix-not-red` 6.6 s, 122, outcome `not_reproduced` (expected `fixed`); `bugfix-edited-test` 8.6 s, 122, outcome `not_fixed` (expected `fixed`);
`decider-rework-spent` 60.6 s, 92; `rework-fresh-allowance` 61.2 s, 83; `finding-person` 61.3 s, 107; `story-decision` 60.8 s, 112;
`story-late-person` 61.2 s, 105; `trace-gap-drop` 60.7 s, 132; `trace-constraint-decided` 60.6 s, 123; `scope-changed` 61.1 s, 132;
`implement-percause` 60.4 s, 139: these nine ended `CycleTimedOut` on Loom's 60 s bound, so what they would end as is not observed here (their difference at `db2726ff` is the row's). A second run of `spec-person-wait` (41.3 s, load 85) and of `cite-fault-sent-back` (21.5 s, load 75) ended as in the first.
Eight entries that the layout change may touch (the `change.review` and request entries) were run the same way at a load of 94 to 156. Seven ended `CycleTimedOut` at the bound: `review-reject-once` (wall 62 s, load 156), `review-minimal-input` (61 s, 145), `review-invalid` (61 s, 151), `review-spent` (61 s, 137), `implement-request-review-rework` (61 s, 125), `coverage-complete` (60 s, 111), `implement-request-path-escape` (62 s, 94). `review-invalid-named` ended as expected in 49 s (load 155). A rerun of `review-reject-once` (60.6 s, load 79) and `review-minimal-input` (60.4 s, load 117) ended on the bound again, and the adversary's runs of `review-reject-once` (60.9 s), `coverage-complete` (60.4 s) and `review-spent` (60.6 s) at a load of 76 to 88 did too; the work directory of `review-reject-once` shows the cycle still progressing (files written until 1 s before the bound), not spinning.
No entry lost `pending`. That no entry changes from expected to another state is not observed on this host at this load: seven of the eight entries above never ended inside the bound. It is observed by one run of the suite at a load where a cycle ends inside Loom's 60 s bound; that run has not been made.
The driver reads the new layout (`Facts::open`, `sdc-node`, `llm-adapter-fixture.sh`: `sdc_tools::reply::records::case_work`): `sdc-reply rejections` on a workspace at `work/<n>` counted 0 before the change and 1 after, and `driver/tests/workspace_layout.rs` holds it; no whole `change.review` cycle has shown it yet.
At Loom `d8944d49` (a rejection gives an action a new allowance of runs) `decider-rework-spent` and `rework-fresh-allowance` were run alone with the binding of `change.implement` at `rework_limit: 2` and at `3` (`bindings/sdc-story.yaml`, `runs_per: rejection`). Observed, as `entry, state, seconds, one-minute load at the start`: limit 2 `decider-rework-spent` pending 43.5 s, 98, `three-rejections: expected 3, found 2`; `rework-fresh-allowance` pending 37.5 s, 94, `exit: expected 0, found 4`; limit 3 `rework-fresh-allowance` pending 35.0 s, 38, same difference; `decider-rework-spent` pending 53.6 s, 35, same difference. Both limits give the same end: the second rejection's rework leaves no revision (the stub wrote the same `.rework` again), and the writer is taken three times until `RunsSpent` (second text) leaves the tree `Stuck`. The stub now numbers a person's nth rejection (`driver/src/bin/sdc-node/rework.rs`); with that, both entries ended on Loom's 60 s bound in four runs each (60 s to 64 s, load 51 to 89), so the value of the limit is not told apart by a run, and the two entries keep `pending`. Between 2 and 3 the entries ask for 2 by Loom's rules (`docs/loom-bindings.md`, "The budget"). The other entries that name rejections of `change.implement` or its budget were run once alone (seconds, load at the start): `story-spent` as expected 41.5 s, 72; `review-spent` 40.9 s, 86; `review-reject-once` 45.0 s, 93; `decider-rework` 45.6 s, 111; `coverage-spent` 30.0 s, 116; `implement-request-review-rework` 41.9 s, 118; `writer-sees-test-failure` 53.7 s, 90; `coverage-not-built` 46.2 s, 96; `implement-request-tests-rework` 50.5 s, 93; `implement-request-invalid-retry` 40.6 s, 102; `coverage-reply-retry` 34.7 s, 105. `story-decision` (pending) ended exit 4 for the expected 0 in two runs (33.0 s, 54.9 s at load 108 and 130; `change.implement` ends exit 79 on its second run and the tree is `Stuck`; the gap its row names) and on the bound in two. `implement-percause` ran as expected and lost `pending` (its row). No entry that was as expected changed state.

The whole file is a slow lane, `task test:cycles-loom`, not part of the gate: one measured run of all
50 cycles took 764 s of cycle time (about 12.7 min wall) and ended `20 of 20 as expected, 30
pending`. The ten slowest cycles took 35.9 s (`implement-percause`, pending), 27.5 s (`trace-gap`),
26.8, 26.8, 26.3, 26.1, 24.2, 24.0, 21.4 and 20.8 s. Every cycle converted by then took between 10.6 s and 27.5 s,
so all of them are over 5 s. The five converted later (the four coverage cycles and `writer-own-error`) took 13.2, 8.5, 10.3, 10.1 and 3.1 s in single runs; `writer-own-error` is the only cycle of the file under 5 s. The seven cycles converted since (the `change.review` ones, run
beside other work of the host, not as part of a whole run) took 14.1 s to 25.3 s each in the last run (`review-reject-once`
25.3 s, `review-minimal-input` 23.3 s, `implement-request-review-rework` 19.0 s, `review-spent` 18.4 s,
`coverage-complete` 17.5 s, `review-invalid` 14.4 s, `review-invalid-named` 14.1 s; 28.3 s to 46.9 s in the first); the totals of the whole
file were not measured again. The time is
the bound programs, one cycle after another: a cycle that goes through to the end runs 213 bound
programs (read from `observations.json` of two cycles), about 0.1 s each, so about 20 s. `runbook
test` itself adds under 0.5 s a cycle. A 60 s wall bound per cycle is Loom's (`SuiteSettings::cycle_timeout`, 60 s by default and 60 s at most; since Loom `c9b4c177` `runbook test --cycle-timeout <seconds>` lowers it, nothing raises it, and a cycle past it ends `CycleTimedOut`). At Loom `db2726ff` the slowest cycle took 50.9 s at a quiet host and 54.0 s at a higher load, and the cycles run under load of the host end on the bound.
A later full run (with tests of the gate running beside it, so slower per cycle) ended `14 of 14 as
expected, 25 pending`; its slowest were `stub-cycle` 40.3 s, `spec-duplicate` 40.2 s and
`superseded-answer` 36.4 s. The four bugfix cycles took 1.3 to 3.5 s each once the cargo cache was
warm (the first, cold run of `bugfix-full` took 20.8 s). `finding-program` took 16.8 to 23.4 s and `offline-decision` 4.4 to 6.3 s. The merged file (54 cycles, 49 of them run) took 940 s of cycle
time with the gate's tests and two slow lanes running beside it, and ended `24 of 24 as expected, 30
pending`; the ten slowest cycles took 51.6 s (`implement-request-path-escape`), 41.4 s
(`writer-sees-test-failure`), 37.3 s (`spec-own-default`), 35.4, 34.5, 34.5, 33.2, 33.0, 32.3 and 30.4 s,
so the scripted-reply cycles come within 8 s of Loom's 60 s bound. To get under the gate's 120 s the file would have to run its cycles in parallel (it does not) or
cycles would have to share an opened tree (the format has no member for it).

### Entries a Rust test of the runner holds (class d) and the ones that wait for the engine

The entries that test the old driver's own code (the decider's prompt, retries, citations and limits,
the stops and refusals of a run, the principals, `sdc-run-metrics`) are Rust tests of the runner
(`runbook/src/loom_run/`) or of `sdc-run-metrics`, so the entry went with the driver's scripts. The rows
below say which test holds each one, `held by a Rust test`. Where the runner differs from what the
cycle expects, the row says what differs and the test holds what the runner does (no such test is `#[ignore]`d);
the runner names its own exit codes and causes
(exit 8 and `decider_cannot_decide` for a stop at the decider, exit 2 for a refused settings file); a
row says so where the cycle's numbers differ and the point is the same.

Waiting for the engine, not a Rust test of the runner yet (an engine feature is missing, the runner's half
is held by the tests named):

| entry | why |
|---|---|
| decider-rework, decider-rework-spent, rework-fresh-allowance, decider-rework-context, decider-rework-model, review-no-rework-prompt | the engine sending a writer back from a recorded `rejected` result with its reasons (`story:tree-stop-record-and-labels`, `approve.result`); the runner's half (the result and the reasons file of a rejection) is held by `runbook/tests/loom_run/decisions.rs` `a_rejection_is_a_named_result_with_the_reasons_in_a_file_of_the_run_directory` and by `runbook/tests/loom_run/decider.rs` `a_rejection_by_the_model_decider_is_a_named_result_with_the_deciders_reasons`; the figures of such a run are held by `driver/tests/run_metrics.rs` |

### Full-cycle entries (29)

| entry | state | reason |
|---|---|---|
| stub-cycle | converted | |
| finding-agent | converted | |
| finding-person-stop | converted | |
| offline-sync | converted | the absence of a finding is `check: none` with a `where`, because an absent file selects no value |
| story-spent | converted | the run count is the `change.implement` runs that ended with exit 0 (3); a fourth action finishes with no exit when the allowance is spent |
| superseded-answer | converted | |
| spec-duplicate | converted | |
| trace-gap | converted | |
| trace-constraint | converted | |
| spec-review-rework | converted | |
| finding-program | converted | `amend.upstream` of `sdc-direction` takes `retry_when: {context_defaults: [finding]}`, so the writer runs again after the check that found the finding; `handoff_when` finds no finding that waits for a person |
| offline-decision | converted | `handoff_when` (`findings-wait`) exits 0 for the finding that waits for a person: the run stops `AwaitingPerson` at the handoff, exit 3 |
| finding-person | pending | the person's decision is written into `findings.yaml` between two runs; the suite has no step that writes a workspace file (gap, no Loom story yet) |
| story-decision | pending | same: a person's decision written into `findings.yaml` between two runs (gap). At Loom `d8944d49`, alone: exit 4 for the expected 0 (33.0 s and 54.9 s, load 108 and 130); the second run of `change.implement` ends exit 79 and the tree is `Stuck`, so `fresh-allowance-four-runs` is not reached |
| story-late-person | pending | same (gap) |
| trace-gap-drop | pending | same (gap) |
| trace-constraint-decided | pending | same (gap) |
| scope-answer | converted | `scope.answer` is a person step with a program (`argv: [tools/sdc-tool.sh, scope-answer, "{principal}", --action, scope.answer]`, Loom `70b0e6e0`): the approval of `decide:` as `person:owner` runs it; 49.5 s |
| scope-changed | pending | runs to the end of the cycle (29.7 s at Loom `db2726ff`, load 35) and fails on its check, `q2-changed (equals, CheckFailed): expected changed, found 8 bytes`: q2 stays `accepted` with its default. The changed answer in the old cycle is a text of the decider or a person's edit of `questions.yaml`, which the suite cannot give (no step writes a workspace file, gap, no Loom story yet); on a loaded host the cycle took 46.6 s to 60.6 s (once it ended at Loom's 60 s wall bound, exit 5) |
| intake-decision | pending | `scope.answer` runs; the run then stops `Stuck` (exit 4) at `amend.upstream`: the intake finding waits for a person's decision written into `findings.yaml` between two runs (gap, no Loom story yet) |
| review-reject-once | converted | the scripted reviews are seeds (`.stub/`); the inputs of `change.review` are read from record files of the workspace (`.sdc/records/`, `docs/loom-bindings.md`); the producer `llm-review` is read from the case file of the store (`doc` in `store`), since a `records` item carries no producer |
| review-minimal-input | converted | with `review_mode: facts` |
| review-invalid | converted | the refusal of the cited line 9999 is read from the call's `review-check.err` |
| review-spent | converted | the three rejections are counted from the records of the workspace and the case |
| spec-own-default | converted | the one model call is a line of `.sdc/llm/calls.jsonl`; the absent `writer_spent` is no `Stopped` event of the case |
| plan-in-project | converted | the project is a seed `.engineering/project.yaml` (a hand-written `development.standard` file) |
| spec-interp-unsupported | converted | the refusal's scenario and reason are read from the second author call's `in/feedback.md`; the synthesize exits are `[1, 0]` |
| replay-sibling | converted | the `replay` step (Loom `db2726ff`, `story:tree-replay-command`, merged): the scripted replies are seeds, and every case of the replay matches its log (`state_matches_log`, `decision_matches_log`); 43.3 s at a one-minute load of 62 |
| spec-person-wait | pending | the writer loop is gone at Loom `c9b4c177` (`story:writer-not-called-leaves-the-due-set`; the loop was `spec.author` taken again about twice a second until the 60 s bound, exit 5, at `db2726ff`). The state of run 1 of record `parity-loom-strong-4`, no model call (a stub spec case, a `needs_person` finding, `spec.author` not called). Run once alone at `c9b4c177` (51.0 s, load 97): `spec.author` finishes twice (exit 0, then exit 79) and is not started a third time, the tree stops `AwaitingPerson` (exit 3) and the entry still fails on `steps[1].exit (exit, ExitDiffers): expected 0, found 3`. It keeps `pending`: the person's decision is out of reach of the suite until a step writes a workspace file (no Loom story yet); the no-model test `runbook/tests/test_suite/notcalled.rs` holds the stopped state |
| pool-wait | dropped with the driver | class: environment of a bound program (a stand-in `slot` first on `PATH`); the stand-in is an executable script, which is shell and no seed (no shell in the data), and the format has no member to put a directory first on `PATH` |

### Re-drive entries (9)

These ran `sdc-drive` with `--recorded <recording>` over the recorded model calls, with `--inject`
and `--on-divergence`. A `loom.tree-suite/1` cycle opens a tree and runs it; it has no recording to
replay model calls from and no injected rejection, timeout or failed check. The ten replay entries
(these nine and `replay-recording-clean`) are Rust tests of `runbook run --recorded`
(`runbook/tests/loom_run/replay.rs`), not cycles of the file: six runs of the committed recording
`driver/examples/sdc/fixtures/recorded/stub`, made once per run of the slow lane `task test:replay` by its setup step, read by the nine
tests that need a run (`#[ignore]`d in the default step; `replay-recording-clean` needs none and runs there).

| entry | state | reason |
|---|---|---|
| replay-unchanged | held by a Rust test | `runbook/tests/loom_run/replay.rs` `replay_unchanged_closes_operating_on_recorded_replies_only` (`runbook run --recorded` on the committed recording `driver/examples/sdc/fixtures/recorded/stub`, strict policy) |
| rule-approvals-default | held by a Rust test | `replay.rs` `rule_approvals_default_is_off_and_every_decision_is_the_deciders` (the run of `replay-unchanged`; no `rule-approvals` directory, every decision is a decider's) |
| review-mode-default | held by a Rust test | `replay.rs` `review_mode_default_reads_the_tree_with_list_read_and_search`. The runner records no `review_mode` setting, so the test checks what the mode is: the operations (`dir.list`, `file.read`, `search`) and the turns (30) of every `change.review` call |
| implement-coverage-default | held by a Rust test | `replay.rs` `implement_coverage_default_is_off` |
| replay-reject-stop | held by a Rust test | `replay.rs` `replay_reject_stop_ends_recording_diverged_when_the_rework_asks_a_call_the_source_never_made`. The old exit 9 is exit 11 (9 is `denied`) |
| replay-reject-nearest | held by a Rust test | `replay.rs` `replay_reject_nearest_reuses_the_recorded_reply_for_the_rework_and_stops_stuck`. The old driver ended `writer_spent` (exit 4); the tree is `stuck` (exit 3) when the writer's runs are spent |
| replay-timeout | held by a Rust test | In part: `replay.rs` `replay_timeout_kills_the_call_on_the_virtual_clock_and_the_tree_stops_on_the_unknown_result`: the call is killed on the virtual clock (1500 s). Not given by the engine: the old driver retried the writer and closed; the binding of `change.implement` has no result for exit 124, Loom refuses it (`ResultNotInTable`) and the tree stops `stuck` (exit 3) |
| replay-live-dry | held by a Rust test | `replay.rs` `replay_live_dry_makes_one_unpaid_live_call_and_stops_at_the_cap` (one live call on the adapter `dry`, paid 0, then exit 11 at the cap) |
| replay-check-fail | held by a Rust test | `replay.rs` `replay_check_fail_fails_tests_run_once_and_the_writer_is_spent_at_the_condition` (`max_runs.implement=2`; the old exit 4 is `stuck`, exit 3) |

### Other entries (9)

| entry | state | reason |
|---|---|---|
| run-stop-decider | dropped with the driver | class: a second tool reads a finished run (`run-trial.sh --stop`, then `sdc-finding-kpis`) |
| run-metrics | held by a Rust test | `driver/tests/run_metrics.rs` `a_run_directory_of_the_loom_runner_with_a_rejected_approval_gives_the_same_figures` (the hand-written `tests/fixtures/loom-run` with a story case in which `merge.approve` sent `change.implement` back once) |
| rework-pending-rejection | dropped with the driver | class: the store is edited between two runs (a case cut at the rejection) |
| decider-later-obligation | held by a Rust test | `runbook/src/loom_run/decider/tests.rs` `an_obligation_only_a_later_step_meets_is_listed_apart_not_open_and_the_prompt_says_so` (on `prepare` and `prompt` with the compiled `sdc-release` and `sdc-direction` IR: a full `decide` of the runner sends an empty position and the case has no `ir.json`) |
| writer-not-called | dropped with the driver | class: the store is edited between two runs (a program replaced, a finding dismissed) |
| decider-input | held by a Rust test | `runbook/src/loom_run/decider/tests.rs` `a_close_reads_the_published_readme_and_no_aep_file`, `a_tree_input_keeps_its_cargo_lock_and_leaves_out_other_lock_files_target_and_git`, `the_prompt_of_an_aep_input_says_the_files_belong_to_the_planning_tool`, `under_final_every_prompt_calls_the_answer_the_owners_own_and_offers_no_escalation_to_a_person` |
| principals | held by a Rust test | `runbook/tests/loom_run/settings.rs` `a_stand_in_names_its_principal_the_run_directory_labels_it_and_no_principal_is_invented` and `a_scripted_decider_takes_the_principal_of_its_answers_from_the_settings`; the refusal of a decision with no principal is the findings tool's (`sdc-findings amend`), not the runner's: the runner runs the printed command and invents no principal |
| replay-recording-clean | held by a Rust test | `runbook/tests/loom_run/replay.rs` `replay_recording_clean_holds_no_home_path_prompt_or_transcript` (the committed recording holds no home path, no prompt text, no transcript) |
| efforts | held by a Rust test | `runbook/tests/loom_run/dry.rs`: `efforts_the_decider_s_effort_is_on_the_dry_command_its_default_is_the_class_default_and_a_bad_level_is_refused` (the decider's effort and its refused level), `efforts_the_class_effort_goes_to_each_model_node_of_its_class_and_none_given_leaves_the_class_default` (`efforts.<class>.effort` of the settings is `--effort` on every model node of the class in the bindings copy, none where none is set) and `efforts_a_level_outside_the_set_or_a_class_that_is_none_is_refused_by_name_before_the_run_directory_is_made`; `driver/tests/sdc_node.rs` `an_effort_option_is_on_the_dry_command_and_in_the_request_of_a_node_of_every_class` (the dry command of a node of each class shows the level it was given, the class default where none is); `loom_run::bugfix::bugfix_from_ticket_with_dry_ends_fixed_and_the_fix_prompt_carries_the_constraints` (`#[ignore]`d, run by the slow lane `task test:bugfix-slow`: a real dry run, the dry commands of `bug.dissect`, `test.reproduce` and `change.fix` carry the level of their class); and `a_level_outside_the_set_is_refused_before_the_node_makes_a_call`. The stand-in claude adapter run (the environment of a bound program) is a class of its own |

### Trial entries (83)

These run the whole cycle under Runbook's autopilot (`--autopilot`, a decider that answers each stop) or
with a model adapter's replies scripted. Twenty-six are in the file: seventeen converted and nine
pending cycles (the scripted-reply entries among them have their replies as seeds; the four bugfix
cycles are two converted and two pending); the other 57 are held by a Rust test or dropped with the driver. The classes of the 57 and
of the four bugfix cycles that were in the first count and are in the file now:

- Runbook's decider answers the stops: 32 entries.
- scripted replies of a stand-in model binary through the environment: 5.
- Runbook's autopilot loop after a stop: 9.
- the rule approver: 5.
- bindings the pinned Loom refuses (the bugfix cycles): 4, now in the file. `bindings/sdc-bugfix.yaml`
  has `gate.run` at 3600 s (D531), and the bugfix protocol records `dissection_written` and
  `repro_written` so that no check runs before its writer (`docs/bugfix.md`).
- the requester decider (2), the environment of a bound program (2), a `--dry` listing (1, held as a Rust test), a ticket
  read before the tree opens (1).

| entry | state | reason |
|---|---|---|
| decider-scope-answer | held by a Rust test | `runbook/tests/loom_run/answers.rs` `two_scope_questions_are_answered_one_by_one_and_the_step_is_taken_by_the_decider` |
| decider-model-name | held by a Rust test | `runbook/src/loom_run/decider/tests.rs` `the_model_of_the_settings_reaches_the_call_and_the_answers_and_the_principal_is_decider_model` |
| decider-scope-final | held by a Rust test | `runbook/src/loom_run/decider/tests.rs` `a_question_that_escalates_under_final_takes_its_default_and_the_log_says_suppressed` |
| decider-finding | held by a Rust test | `runbook/tests/loom_run/answers.rs` `a_finding_that_needs_a_person_is_answered_at_its_handoff_and_the_run_goes_on` |
| decider-finding-final | held by a Rust test | `runbook/src/loom_run/decider/tests.rs` `a_finding_that_escalates_under_final_takes_the_best_answer_else_its_default` |
| decider-finding-refused | dropped with the driver | class: Runbook's decider answers the stops (`--autopilot`); the suite's `decide:` is a fixed list, with no model or rule decider whose prompt, retries, citations or limits a cycle can check |
| run-stop | converted | the steps `stop`, `resume` (refused `TreeStopped`) and `reopen`, and the check that the case log holds `StopRecorded` (Loom `db2726ff`); the cycle runs 44.3 s at a host load of 125 to 177 |
| autopilot | dropped with the driver | class: Runbook's decider answers the stops (`RULE_APPROVALS`, `--autopilot`); the suite's `decide:` is a fixed list, with no model or rule decider whose prompt, retries, citations or limits a cycle can check |
| trace-reraised | dropped with the driver | class: Runbook's decider answers the stops (`DECIDER_REPLIES`, `--autopilot`); the suite's `decide:` is a fixed list, with no model or rule decider whose prompt, retries, citations or limits a cycle can check |
| decider-budget-reason | held by a Rust test | `runbook/tests/loom_run/answers.rs` `the_decider_call_cap_ends_the_run_when_the_answers_would_exceed_it`: the stop reason names the step, the case and the cap (cause `decider_cannot_decide`, the runner's name for it) |
| autopilot-person | held by a Rust test | `runbook/tests/loom_run/settings.rs` `a_decider_kind_person_is_refused_by_name_before_any_run_directory_or_call` (exit 2 in the runner, 64 in the driver; no run directory, no call) |
| autopilot-stop | held by a Rust test | `runbook/tests/loom_run/decisions.rs` `a_decision_the_decider_has_no_reply_for_stops_the_run_naming_step_and_case_with_no_path_in_the_reason` (exit 8, `by: rule:loom-run`, cause `decider_cannot_decide`: the runner's names for exit 7, `rule:autopilot`, `decision_not_takeable`); `the_reason_of_a_stop_at_a_decision_the_decider_cannot_take_names_no_person` (the reason names the step, the case and what could not be decided, and no person) |
| decider-cite-address | held by a Rust test | `runbook/src/loom_run/decider/tests.rs` `an_entry_a_line_range_and_an_id_of_the_context_resolve_and_the_first_reply_is_taken` (the address of an artifact resolves through the `bindings.json` that `make_case_dir` writes since this change) |
| decider-cite-retry | held by a Rust test | `runbook/src/loom_run/decider/tests.rs` `a_cite_that_does_not_resolve_is_asked_again_with_the_reason_and_the_second_reply_is_taken` |
| decider-invalid-approval | held by a Rust test | `runbook/src/loom_run/decider/tests.rs` `three_invalid_replies_to_an_approval_end_cannot_decide_naming_the_last_reason_after_three_calls`; `three_invalid_replies_to_an_approval_whose_checks_are_true_stay_cannot_decide_and_are_never_approved_on_the_checks`: the runner has no `approved_on_checks` (no claim values to check, and an approval nobody validly gave stays `CannotDecide`; `docs/loom-run-decisions.md`) |
| decider-invalid-scope | held by a Rust test | `runbook/src/loom_run/decider/tests.rs` `three_invalid_replies_to_a_question_take_its_default_as_invalid_reply_default` |
| decider-escalate-nobest | held by a Rust test | `runbook/src/loom_run/decider/tests.rs` `an_escalate_without_best_is_refused_under_final_and_asked_again` |
| decider-reject | held by a Rust test | `runbook/tests/loom_run/decisions.rs` `a_rejection_where_the_action_has_no_rejected_result_stops_the_run_naming_step_and_case_and_sends_nothing` (exit 10, outcome `stopped: rejected`, nothing sent to Loom); `a_rejection_without_a_result_to_record_it_names_the_case_and_no_person` (cause `rejected_without_rework_path`, reason `<step> in <case>: …`; the driver's exit 6 is taken by the runner's model-call cap, so the runner's exit is 10) |
| decider-rework-context | dropped with the driver | class: Runbook's decider answers the stops (`RULE_APPROVALS`, `DECIDER_REPLIES`, `--autopilot`); the suite's `decide:` is a fixed list, with no model or rule decider whose prompt, retries, citations or limits a cycle can check |
| decider-rework-model | dropped with the driver | class: Runbook's decider answers the stops (`RULE_APPROVALS`, `DECIDER_REPLIES`, `--autopilot`); the suite's `decide:` is a fixed list, with no model or rule decider whose prompt, retries, citations or limits a cycle can check |
| decider-rework | converted | the scripted rejection and approval of `merge.approve` (`approve` with `result` and `reasons`, Loom `db2726ff`, `story:tree-suite-stop-resume-reopen-steps`): the writer is sent back once and the case ends well; 24.7 s at a one-minute load of 34 (the same cycle ended on Loom's 60 s wall bound, exit 5, in three runs at a load of 100 to 177) |
| decider-rework-spent | pending | three scripted rejections of `merge.approve` (Loom `db2726ff`, `story:tree-suite-stop-resume-reopen-steps`). At Loom `d8944d49` the binding of `change.implement` has `runs_per: rejection` and `rework_limit: 2`, so a rejection gives the writer a new allowance of three runs. Run alone, with the limit at 2 (43.5 s, load 98) and at 3 (53.6 s, load 35), it fails the same way, `three-rejections (count, CheckFailed): expected 3, found 2`, so the limit is not the cause: after the second rejection the writer runs three times and leaves no revision (`revisions: []`), then `RunsSpent` in its second text (`change.implement has run 3 times in case root.billing.invoices since the rejection ... by person:owner, its budget.max_runs is 3`) and the tree ends `Stuck` (exit 4). The cause is the stub: `stub-tree` removes `impl` and copies the fixture, `sdc-node` writes `.rework` with the same reasons, so the tree after the second rework is the one the first rework left and Loom makes no revision, so `tests.run`, the review and the approval are not offered again. Since this change the stub numbers the nth rejection of an approval by a person in its note (`rework 2 of the rejection of ...`; `driver/src/bin/sdc-node/rework.rs`, test `the_second_rejection_of_an_approval_by_a_person_is_numbered_in_the_note`). With the numbering the entry ended on Loom's 60 s bound in four runs (60.5 s at the last, load 70 to 89), so what it ends as is not observed. `rework_limit` is 2 by reading Loom's rules, not by a run of the entry: at 2 the third rejection is the last (the writer is refused with the first text), so the entry counts three; at 3 the writer runs a fourth time and a fourth rejection is possible. The entry keeps `pending` until a run ends inside the bound |
| rework-cap | dropped with the driver | class: Runbook's decider answers the stops (`MAX_REJECTIONS`, `DECIDER_REPLIES`, `--autopilot`); the suite's `decide:` is a fixed list, with no model or rule decider whose prompt, retries, citations or limits a cycle can check |
| rework-not-called | dropped with the driver | class: Runbook's decider answers the stops (`DECIDER_REPLIES`, `--autopilot`); the suite's `decide:` is a fixed list, with no model or rule decider whose prompt, retries, citations or limits a cycle can check |
| review-no-rework-prompt | dropped with the driver | class: Runbook's decider answers the stops (`DECIDER_REPLIES`, `--autopilot`); the suite's `decide:` is a fixed list, with no model or rule decider whose prompt, retries, citations or limits a cycle can check |
| cite-dark-spot | pending | the intake finding is decided by the decider in the old cycle; here it stops for a person's decision written into `findings.yaml` (gap, no Loom story yet) |
| cite-fault-sent-back | pending | the loop of `requirements.normalise` (exit 79, taken again 97 times in 60 s at `db2726ff`) is gone at Loom `c9b4c177` (`story:writer-not-called-leaves-the-due-set`). Run once alone (18.6 s, load 92): `requirements.normalise` finishes twice (exit 0, then exit 79) and it fails on `steps[1].exit (exit, ExitDiffers): expected 4, found 3`: the tree stops `AwaitingPerson` on the intake finding, the same as the entries above (gap, no Loom story yet). It keeps `pending` |
| autopilot-close | held by a Rust test | `runbook/tests/loom_run/decisions.rs` `at_a5_with_a_model_decider_the_close_is_taken_by_rule_and_the_decider_is_not_asked` (principal `rule:autopilot`, no model call); `at_a5_the_close_through_an_outcome_the_class_does_not_list_is_asked_of_the_model_decider_and_run_as_printed` holds the close of an outcome the close class does not list |
| autopilot-close-a3 | held by a Rust test | `runbook/tests/loom_run/decisions.rs` `at_a3_the_close_is_a_persons_and_the_decider_is_not_asked` (the table of `tools/autonomy-levels.json` gives the close to a person at A3: exit 8, no model call; the driver asked the decider there, and a level never widens what its table gives); `at_a4_the_decider_is_asked_for_the_close_rejects_and_the_run_stops_unclosed` holds the decider's rejection of the close where the table gives it (A4: exit 9, nothing closed) |
| conformance-default-off | held by a Rust test | `runbook/tests/loom_run/conformance.rs::conformance_default_off`: a closed run without `--conformance observe` (and with `--conformance off`) writes no `oracle.json` and starts no observer; `--conformance observe` on a closed run writes it (`observe_on_a_closed_run_writes_oracle_json_from_the_run_workspace_through_the_pinned_tool`), the observation of a failure is written into it and never changes the exit (`a_failed_observation_is_written_into_oracle_json_and_the_run_exit_stays`). The entry stays in `cycles/suite.yaml` until the script is deleted |
| rule-approvals | dropped with the driver | class: the rule approver of Runbook (rule approvals on or off, decided by a rule of the driver); the suite's `decide:` has no rule approver to switch |
| rule-approvals-check-failed | dropped with the driver | class: the rule approver of Runbook (a tampering `sdc-drive` on the drive path); the suite's `decide:` has no rule approver to switch |
| rule-approvals-revision-changed | dropped with the driver | class: the rule approver of Runbook (a tampering `sdc-drive` on the drive path); the suite's `decide:` has no rule approver to switch |
| rule-approvals-off | dropped with the driver | class: the rule approver of Runbook (rule approvals on or off, decided by a rule of the driver); the suite's `decide:` has no rule approver to switch |
| rule-approvals-a3 | dropped with the driver | class: the rule approver of Runbook (rule approvals on or off, decided by a rule of the driver); the suite's `decide:` has no rule approver to switch |
| keep-going-three | dropped with the driver | class: Runbook's autopilot loop after a stop (`--keep-going`, the unstick step, `--reopen` of a refused decision): it re-runs the tree after a stop, which a cycle's `steps` cannot decide from a model's reply |
| keep-going-off | dropped with the driver | class: Runbook's autopilot loop after a stop (`--keep-going`, the unstick step, `--reopen` of a refused decision): it re-runs the tree after a stop, which a cycle's `steps` cannot decide from a model's reply |
| keep-going-repeat | dropped with the driver | class: Runbook's autopilot loop after a stop (`--keep-going`, the unstick step, `--reopen` of a refused decision): it re-runs the tree after a stop, which a cycle's `steps` cannot decide from a model's reply |
| unstick-end | dropped with the driver | class: Runbook's autopilot loop after a stop (`--keep-going`, the unstick step, `--reopen` of a refused decision): it re-runs the tree after a stop, which a cycle's `steps` cannot decide from a model's reply |
| unstick-rerun | dropped with the driver | class: Runbook's autopilot loop after a stop (`--keep-going`, the unstick step, `--reopen` of a refused decision): it re-runs the tree after a stop, which a cycle's `steps` cannot decide from a model's reply |
| unstick-invalid | dropped with the driver | class: Runbook's autopilot loop after a stop (`--keep-going`, the unstick step, `--reopen` of a refused decision): it re-runs the tree after a stop, which a cycle's `steps` cannot decide from a model's reply |
| unstick-limit | dropped with the driver | class: Runbook's autopilot loop after a stop (`--keep-going`, the unstick step, `--reopen` of a refused decision): it re-runs the tree after a stop, which a cycle's `steps` cannot decide from a model's reply |
| unstick-off | dropped with the driver | class: Runbook's autopilot loop after a stop (`--keep-going`, the unstick step, `--reopen` of a refused decision): it re-runs the tree after a stop, which a cycle's `steps` cannot decide from a model's reply |
| reopen-refused-decision | dropped with the driver | class: Runbook's autopilot loop after a stop (`--keep-going`, the unstick step, `--reopen` of a refused decision): it re-runs the tree after a stop, which a cycle's `steps` cannot decide from a model's reply |
| decider-release-a3 | held by a Rust test | `runbook/tests/loom_run/decisions.rs` `at_a3_the_decider_does_not_answer_and_the_run_stops_for_a_person_without_calling_the_model` (the table gives the release to a person up to A4); `a_context_the_decider_rejects_at_a3_ends_without_a_rework_path_and_records_nothing` holds the decider's rejection at A3 on a class the table gives it (exit 10). The second run of `sdc-drive decide` in the entry is the engine's |
| decider-release-withheld | dropped with the driver | class: Runbook's decider answers the stops (`RULE_APPROVALS`, `DECIDER_REPLIES`, `--autopilot`); the suite's `decide:` is a fixed list, with no model or rule decider whose prompt, retries, citations or limits a cycle can check |
| requester-decider | held by a Rust test | the port is built: `--goals` and `decider.requester` (`docs/loom-run-decisions.md`); held as fake-Loom tests `requester_decider_*` in `runbook/tests/loom_run/goals.rs`, which test the runner's requester decider alone. The real engine now takes the program of a person action's approval (Loom `70b0e6e0`): `runbook/tests/loom_run/inproc.rs` `a_declared_step_with_a_program_gets_the_decider_as_principal_from_the_engine_and_no_then_runs` runs a declared step with a program and a decider principal through it. What stops the entry is that the suite's `decide:` is a fixed list with no requester decider, as for the other entries of that class |
| requester-no-ground | held by a Rust test | as `requester-decider`: held as `requester_no_ground_*` in `runbook/tests/loom_run/goals.rs` (the runner alone); the engine's half is the same in-process test |
| models | held by a Rust test | held as the Rust test `loom_run::dry::models_each_call_names_the_binding_s_model_or_the_one_for_all_or_the_one_of_its_class` (`runbook/tests/loom_run/dry.rs`, `runbook run --dry`): a `--dry` listing of the models per class is not a run of a tree, so the entry has no cycle in the file |
| decider-question-reject | held by a Rust test | `runbook/src/loom_run/decider/tests.rs` `a_reject_of_a_question_is_refused_with_its_reason_asked_again_and_answered` |
| rework-fresh-allowance | pending | two scripted rejections of `merge.approve` (Loom `db2726ff`), the fixture starting with a failing test. At Loom `d8944d49` the binding of `change.implement` has `runs_per: rejection` and `rework_limit: 2`. Run alone with the limit at 2 (37.5 s, load 94) and at 3 (35.0 s, load 38) it ends `steps[1].exit (exit, ExitDiffers): expected 0, found 4`, the same way, so the limit is not the cause. The writer runs twice before the first rejection (the first run, the run after the failing test), once after it with a new revision, and after the second rejection three times with no revision (`revisions: []`); then `RunsSpent` in its second text (`has run 3 times in case root.billing.invoices since the rejection ... by person:owner, its budget.max_runs is 3`) and the tree ends `Stuck`. The cause is the stub, as at `decider-rework-spent` (the second rework writes the same `.rework`, so no new revision, so the approval is not asked again). With the numbering of the stub (`rework 2 of the rejection of ...`, this change) the entry ended on Loom's 60 s bound in four runs (61 s to 62 s, load 64 to 80), so what it ends as is not observed. The entry keeps `pending` until a run ends inside the bound. |
| scope-unattributed-reject | dropped with the driver | class: Runbook's decider answers the stops (`DECIDER_REPLIES`, `--autopilot`); the suite's `decide:` is a fixed list, with no model or rule decider whose prompt, retries, citations or limits a cycle can check |
| finding-refused-reask | held by a Rust test | In part: `runbook/tests/loom_run/answers.rs` `a_request_that_comes_back_a_fourth_time_ends_the_run_naming_it` holds the stop; the refusal text is not put to the decider again (the refusal is the findings tool's) |
| rework-spent-dropped | dropped with the driver | class: Runbook's decider answers the stops (`DECIDER_REPLIES`, `--autopilot`); the suite's `decide:` is a fixed list, with no model or rule decider whose prompt, retries, citations or limits a cycle can check |
| rework-spent-abandoned | dropped with the driver | class: Runbook's decider answers the stops (`RULE_APPROVALS`, `DECIDER_REPLIES`, `--autopilot`); the suite's `decide:` is a fixed list, with no model or rule decider whose prompt, retries, citations or limits a cycle can check |
| review-invalid-named | converted | |
| writer-own-error | converted | `questions.answer` records `answer_applied` (`sdc.intake` revision 10: exit 0 empty, exit 1 `refused`) and its binding has `retry_when: {answer_applied: [refused]}`: the refused apply is a retry record, the next run reads its log in `in/feedback.md`; the fixture never fixes the answer, so the run ends after the third refusal (`RunsSpent`, stop at `scope.answer`, exit 3) |
| implement-percause | converted | expects exit 0, outcome `operating`, four `change.implement` runs that each end exit 0 and none without an exit (`four-implement-runs`, `no-writer-run-without-exit`). At `db2726ff` it stopped at the finding that `amend.upstream` raises after a test failure (exit 3, a person's decision written into `findings.yaml` between two runs); at `c9b4c177` its one run ended on Loom's 60 s bound (load 139). At Loom `d8944d49` it ran as expected once, alone, in 53.8 s at a one-minute load of 100 (`pending, passing`), and ended on the bound in three further runs at a load of 117 to 131, so it lost `pending` on one run; what changed it (the set-aside action idle in the due set of this pin, or the budget of `change.implement`) is not told apart here |
| writer-sees-test-failure | converted | the failing test's message and the absence of a pool line are read from the second call's `in/feedback.md` (`files`, `follow: file`) |
| implement-request-build | converted | the request record (`reply-schema.json`, `implement-inputs.json`) is read the same way; not said: the written `impl/src/lib.rs` equal to the fixture's file, and a non-empty `commands` in the inputs |
| implement-request-tests-rework | converted | the second request is read from `.sdc/llm/002-change.implement/prompt.md` |
| implement-request-review-rework | converted | |
| implement-request-invalid-retry | converted | |
| implement-request-invalid-named | converted | the cause `implement_reply_invalid` is a field of the call line; not said: the old driver's `run-stopped.json`, here the stop is exit 4 |
| implement-request-path-escape | converted | |
| coverage-complete | converted | |
| coverage-not-built | converted | `sdc.story` revision 8: `sdc-node` runs `sdc-reply check-coverage` after the writer under Loom (`SDC_IMPLEMENT_COVERAGE` from `.stub/settings.yaml`) and exits 8 on a refusal; the binding records exit 8 as `implementation_change` `coverage_refused` and `retry_when` lists it; `tests.run` is not admissible on a revision with that record |
| coverage-line-outside | converted | same form |
| coverage-test-missing | converted | same form |
| coverage-spent | converted | same form; the three runs are the `change.implement` runs that ended with exit 8 (a fourth action finishes with no exit when the allowance is spent) |
| coverage-reply-retry | converted | the node's own shape check and its second request run in `sdc-node`, so they hold under Loom; the refused reply and the reason in the prompt are read from the call record |
| review-real-path | dropped with the driver | class: stand-in model binary through the environment (`METAHARNESS`, `CLAUDE`, `FAKE_MODEL_*`: executable stand-ins and variables of a bound program, which the layout of `.stub/` does not carry), and the same `change.review` inputs as review-reject-once |
| review-real-path-facts | dropped with the driver | same |
| review-invalid-stop | dropped with the driver | same |
| review-adapter-refused | dropped with the driver | same, and the refusal is the exit 64 of `sdc-drive` before a run, not a cycle's outcome |
| reopen-review-invalid | dropped with the driver | same, and a `--reopen` of a stopped run |
| bugfix-full | converted | the seeds are `cycles/seeds/bugfix-pager/` (the workspace `sdc-bugfix seed` writes); the case ends `fixed` with one red, one fix and one passed suite run |
| bugfix-not-red | pending | still failing: `expect.outcome (outcome, OutcomeDiffers): expected fixed, found not_reproduced`: the first `repro_run` `not_red` makes `not_reproduced` legitimate and Loom ends the case before the test writer is sent back; the protocol has no claim for "the writer is spent" |
| bugfix-edited-test | pending | still failing: `expect.outcome (outcome, OutcomeDiffers): expected fixed, found not_fixed`: the first `test_run` is `failed` and Loom ends the case `not_fixed` before `change.fix` is sent back; same missing "writer is spent" |
| bugfix-never-red | converted | ends `not_reproduced` with no red, no fix and no suite run; Loom ends the case at the first legitimate outcome, after one `test.reproduce` run (the old cycle ran the writer to its allowance) |
| bugfix-from-ticket | held by a Rust test | `runbook/tests/loom_run/bugfix.rs`: `bugfix_from_ticket_ends_fixed_on_a_git_repository_and_a_dirty_one_is_refused` (the real helper and bindings, stub adapter, no `--dry`: fixed, tracked files only, base recorded, story, no ticket text on the console or outside `work/`, a dirty worktree refused; `#[ignore]`d, run by the slow lane `task test:bugfix-slow`, needs `driver/target/debug`, 19.7 s); the seed step on a stand-in helper in the gate lane is `a_ticket_seeds_the_root_case_before_the_tree_opens` and `the_ticket_text_reaches_the_work_directory_and_no_other_file_stream_or_event`; the constraints in the fix's prompt are held by `bugfix_from_ticket_with_dry_ends_fixed_and_the_fix_prompt_carries_the_constraints` (a real run on the adapter `dry`, which writes each prompt: the constraints are in the `change.fix` prompt, a constraint the ticket lacks is not; `#[ignore]`d, run by the same lane `task test:bugfix-slow`, measured 17.1 s at load 44 (this round), 26.1 s and 27.8 s (the reviewer's run), 30.4 s at load 45, and 49 s and 61.5 s at load 130 (earlier runs); the runner ends a test at 60 s (`slow-timeout` 30 s, `terminate-after` 2), so at a load near 130 the lane can fail, and the timeout is not raised; it is out of the default step; the golden `instructions/golden/llm-node/change.fix.ticket.md` also holds the prompt text) |
| bugfix-pool-wait | dropped with the driver | class: environment of a bound program (a stand-in `slot` first on `PATH`) on a bugfix run; the format has no member for it, and the bugfix bindings are refused as above |
| bugfix-over-time | dropped with the driver | class: environment of a bound program (a stand-in `slot` first on `PATH`) on a bugfix run; the format has no member for it, and the bugfix bindings are refused as above |
