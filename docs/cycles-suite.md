# The finding cycles as data: `cycles/suite.yaml`

`story:cycles-suite-as-data`, step 5 of the move of Runbook onto Loom. The finding cycles (each: a
stub setup, a run, an expected verdict) are code in `scripts/run-finding-cycles.sh`. `cycles/suite.yaml`
(format `runbook.cycles/1`) is the same list as data. Loom's `tree test --suite` is to read it and the
script is then deleted. Until then the script runs the cycles and the suite is checked against it.

Loom has no suite format yet (no `tree test` on its wave branches), so `runbook.cycles/1` is defined
here. Its open points go to Loom: the distinct `unexpressed` kinds below are the list of things a
declarative format must be able to say.

## Shape

One entry per cycle, in the order of the script:

- `id`, `state` (`expected`, `pending`, or `unlisted`: a cycle the script runs outside both arrays;
  none now), `when` (the condition when the script adds the id conditionally),
  `protocol` (root protocol id), `driver` (`full-cycle`, `trial`, `redrive`, `other`).
- `stub`: fixture files and directories, the llm or decider adapter, scripted replies.
- `switches`: every environment variable the cycle sets. `arguments`: run arguments such as
  `--autopilot`. `setup`: declarative setup steps.
- `expect`: `exit` (or `exit_not`), the script's own `pass_line`, and `checks`, each one comparison:
  `event_count`, `json_count`, `json_equals`, `json_matches`, `yaml_equals`, `yaml_matches`,
  `yaml_not_matches`, `yaml_has_item`, `yaml_lacks_item`, `file_contains`, `file_absent_text`,
  `file_exists`, `file_absent`, `file_empty`, `file_same_as`, `file_count_lines`. Paths use
  `{scratch}`, `{run}`, `{store}`, `{work}` and `{console}`.
- `unexpressed`: every assertion or setup that needs more than one comparison or a program, with its
  `kind`, `lines` (the script's line numbers, as `"412"`, `"412-418"` or a comma list of both, or the name
  of the script expression that makes the assertion, such as `"red_then_green"`) and a short `note`. Kinds: `aggregate`, `event-order`, `tree-search`,
  `second-run`, `cross-record`, `field-compare`, `derived-value`, `setup-shell`, `mutated-store`,
  `console-regex`, `conditional`, `computed-input`.
- `source`: the line range of the cycle's block in the script.

No shell in the data. An assertion that cannot be said declaratively stays in `unexpressed`; nothing
is dropped.

## The check

`runbook check` (run by `task check:cycles-suite`, part of `task check`) reads the suite and fails when
it is not format `runbook.cycles/1`, repeats an id, has an entry without an `id` or with a state other
than `expected`, `pending` or `unlisted`, or names a fixture (`stub.fixtures`) that does not exist
under the asset root. While `scripts/run-finding-cycles.sh` exists it also reads the script text:
every `expected=(...)`, `expected+=(...)`, `pending=(...)` and `pending+=(...)` array, a conditional
append included, and fails when the suite differs from them in the set of ids or in a state. An entry
with state `unlisted` is not compared with the arrays; it fails only when the script lists its id
after all, and the check prints a note with the number of such entries. Without the script the
comparison is skipped and one note says so. `RUNBOOK_HOME=<root>` points the check at another tree
(the task defaults to this checkout; the old `CYCLES_SUITE` variable is gone). `runbook check` also
checks the rest of the text layout: the parts `runbook doctor` lists, the bindings and the
instruction files they name, the placeholders of the instruction files, and the view files.

## Running named cycles

A full run of `scripts/run-finding-cycles.sh <scratch dir>` takes over an hour. To repeat one cycle
or a few, name them after the scratch directory:

```
scripts/run-finding-cycles.sh <scratch dir> --only <name>[,<name>...]
task cycles:only SCRATCH=<scratch dir> NAMES=<name>[,<name>...]
```

A name is an id of `cycles/suite.yaml` (an `expected` or a `pending` cycle of the script); `--only` may
be repeated. A selected cycle runs the same code with the same checks as in a full run. A cycle that is
not selected neither runs nor is counted.

- A name the script does not list ends it with exit 2, naming it, before anything runs.
- A cycle that reads the output of an earlier cycle (its run directory, store or log) runs that cycle
  too and says so in one line: `cycles: also running autopilot (needed by decider-input: it reads that
  cycle's output)`. The script holds that list in `needs`: `run-stop-decider` needs
  `decider-finding-refused`; `decider-later-obligation` needs `autopilot`; `decider-input` needs
  `autopilot` and `autopilot-close-a3`; `conformance-default-off` needs `autopilot-close`;
  `principals` needs `finding-person` and `finding-person-stop`; `run-metrics` needs `decider-rework`;
  `rework-cap` needs `decider-rework-spent`; `rework-pending-rejection` needs `rework-not-called`;
  `writer-not-called` needs `replay-sibling`. `rule-approvals-default`, `review-mode-default` and
  `implement-coverage-default` read one shared re-drive (`switches-default`), which runs once for any
  of them. The helper runs of a cycle (`rework-cap-prompt`, `models-all`, `efforts-bad`,
  `review-facts-once`, ...) run with it.
- The verdict line says how many were selected, so that it is never taken for a full run:
  `cycles: selected 2 of 126, 2 as expected, 0 failed: none`, or with a pulled-in cycle
  `cycles: selected 1 of 126 (+1 needed: autopilot), 2 as expected, 0 failed: none`. It compares the
  selected and pulled-in cycles only. A selected `pending` cycle is reported on a `pending: selected K
  of P (...)` line and, as in a full run, never fails the script. The line of a full run, `cycles: N
  expected, ...`, is not printed.
- The exit code is 1 when a selected cycle failed, 0 when all are as expected.
- The log is `<scratch dir>/cycles-only.log`; the `cycles.log` of a full run in the same directory
  stays. The run and store directories of a selected cycle are rewritten, as in a full run.

A cycle added to the script stands in its own `if sel <name>; then` ... `fi` block, and one that
reads another cycle's output gets an entry in `needs`.

## The same cycles as a Loom tree suite: `cycles/tree-suite.yaml`

`runbook test cycles/tree-suite.yaml --confinement local` runs the file in the `runbook` process
(Loom's `run_suite`, format `loom.tree-suite/1`); the run holds `runbook` and the bound programs
only. `task test:cycles-loom` runs it. `cycles/suite.yaml` and `scripts/run-finding-cycles.sh` stay as
they are until the table below has no `pending` and no `not converted` left.

Every entry of `cycles/suite.yaml` is in one of three states:

- **converted**: a cycle in `cycles/tree-suite.yaml`, run through `runbook test`, as expected.
- **pending**: a cycle in the file marked `pending: true`. A cycle whose step or member the pinned
  Loom does not run yet prints `pending, waits for story:<id>`; a cycle that runs prints
  `pending, still failing: <difference>`, which is the evidence that the mark is right, and
  `pending, passing` when it can move to converted.
- **not converted**: not in the file; the class says which kind of Loom story it needs.

How the environment switches of the old driver are said in the file, with no environment passed
through:

- Fixture directories and the `SDC_STUB_FINDINGS` list are seeds into `.stub/` of the workspace
  (`.stub/finding.yaml`, `.stub/finding-2.yaml`, the rest of the directory is the fixture
  directory); `bindings/tools/llm-node.sh` reads that directory when it runs under Loom.
- Seeds are copies in `cycles/seeds/`: Loom refuses a seed that is a symbolic link.
- `cycles/bindings` is a symbolic link to `../bindings`, because the suite's `bindings:` path stays
  inside the suite's directory.
- Approvals of person actions are `decide:` entries in `defaults`, bound to the principal
  `person:owner`.
- Scripted replies of a model node (`SDC_*_REPLIES`, `SDC_LLM_ADAPTER`, `SDC_LLM_NODES`) are the
  `fixtures:` member of a cycle, which the pinned Loom does not run yet.
- What the old driver took from `amend.upstream`'s `handoff_when` is `handoff_when: [tools/sdc-tool.sh,
  findings-wait, --action, amend.upstream]` in the five `bindings/sdc-*.yaml` files that bind
  `amend.upstream` (Loom `5bcc26d9`). Only Loom reads these files; the old driver reads the JSON
  bindings, which keep their own `handoff_when`.
- A bugfix cycle's workspace is the one `sdc-bugfix seed` writes, kept as `cycles/seeds/bugfix-pager/`
  (`bug.yaml`, `checks.yaml`, `impl/`, `mainline/`, `aep/`); the model stubs come from
  `driver/examples/sdc/fixtures/bugfix-pager/`, and a cycle with its own stubs seeds
  `cycles/seeds/fixtures/bugfix-<name>/` into `.stub/`.

### Totals and time

130 entries: 14 converted, 25 pending (12 wait for a story the pinned Loom has not shipped and are
not run; 13 run and still fail), 91 not converted (1 full-cycle, 18 re-drive and other, 72 trial).

The whole file is a slow lane, `task test:cycles-loom`, not part of the gate: one measured run took
279 s wall (build included) and ended `10 of 10 as expected, 25 pending`. The ten slowest cycles
took 29.7 s (`stub-cycle`), 21.0, 20.3, 19.8, 19.6, 19.5, 19.4, 16.5, 15.1 and 13.5 s. The time is
the bound programs, one cycle after another: a cycle that goes through to the end runs 213 bound
programs (read from `observations.json` of two cycles), about 0.1 s each, so about 20 s. `runbook
test` itself adds under 0.5 s a cycle. A 60 s wall bound per cycle is Loom's; no cycle is near it.
A later full run (with tests of the gate running beside it, so slower per cycle) ended `14 of 14 as
expected, 25 pending`; its slowest were `stub-cycle` 40.3 s, `spec-duplicate` 40.2 s and
`superseded-answer` 36.4 s. The four bugfix cycles took 1.3 to 3.5 s each once the cargo cache was
warm (the first, cold run of `bugfix-full` took 20.8 s). `finding-program` took 16.8 to 23.4 s and `offline-decision` 4.4 to 6.3 s. To get under the gate's 120 s the file would have to run its cycles in parallel (it does not) or
cycles would have to share an opened tree (the format has no member for it).

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
| story-decision | pending | same: a person's decision written into `findings.yaml` between two runs (gap) |
| story-late-person | pending | same (gap) |
| trace-gap-drop | pending | same (gap) |
| trace-constraint-decided | pending | same (gap) |
| scope-answer | pending | stops at `scope.answer` of `root.intake`: a program on a person action's approval is in Loom's drafting, no story id yet |
| scope-changed | pending | same: `scope.answer` as a program on a person action's approval |
| intake-decision | pending | same: `scope.answer` as a program on a person action's approval |
| review-reject-once | pending | scripted model replies: waits for story:engine-agents-section (`fixtures`) |
| review-minimal-input | pending | same, story:engine-agents-section |
| review-invalid | pending | same, story:engine-agents-section |
| review-spent | pending | same, story:engine-agents-section |
| spec-own-default | pending | same, story:engine-agents-section |
| plan-in-project | pending | same, story:engine-agents-section |
| spec-interp-unsupported | pending | same, story:engine-agents-section |
| replay-sibling | pending | story:engine-agents-section (`fixtures`) and story:tree-replay-command (`replay`) |
| pool-wait | not converted | class: environment of a bound program (a stand-in `slot` first on `PATH`); the format has no member for it |

### Re-drive entries (9)

These run `sdc-drive` with `--recorded <recording>` over the recorded model calls, with `--inject`
and `--on-divergence`. A `loom.tree-suite/1` cycle opens a tree and runs it; it has no recording to
replay model calls from and no injected rejection, timeout or failed check.

| entry | state | reason |
|---|---|---|
| replay-unchanged | not converted | class: replay of recorded model calls (`--recorded`) and `sdc-replay compare` as a second tool over the run |
| rule-approvals-default | not converted | class: replay of recorded model calls with a divergence policy (`--on-divergence nearest`) |
| review-mode-default | not converted | same class |
| implement-coverage-default | not converted | same class |
| replay-reject-stop | not converted | class: an injected decision (`--inject reject@...`) into a replay |
| replay-reject-nearest | not converted | same class, with the call modes of the replay as the check |
| replay-timeout | not converted | class: an injected timeout of one call (`--inject timeout@...`) into a replay |
| replay-live-dry | not converted | class: a replay that goes live on divergence (`--live-adapter`, `--live-cap`) |
| replay-check-fail | not converted | class: an injected failed check (`--inject check-fail@...`) into a replay |

### Other entries (9)

| entry | state | reason |
|---|---|---|
| run-stop-decider | not converted | class: a second tool reads a finished run (`run-trial.sh --stop`, then `sdc-finding-kpis`) |
| run-metrics | not converted | class: a second tool reads a finished run (`sdc-run-metrics`) |
| rework-pending-rejection | not converted | class: the store is edited between two runs (a case cut at the rejection) |
| decider-later-obligation | not converted | class: the contents of the decider's call directories (`prompt.md`, the open obligations it was shown) |
| writer-not-called | not converted | class: the store is edited between two runs (a program replaced, a finding dismissed) |
| decider-input | not converted | class: the contents of the decider's call directories (the files of its input) |
| principals | not converted | class: the store is edited between two runs (a decision written by hand), then a refusal of a second tool |
| efforts | not converted | class: dry runs and a stand-in model adapter written by the cycle: the environment of a bound program |
| replay-recording-clean | not converted | class: the recording file of the replay tool (`recording.json`) |

### Trial entries (83)

These run the whole cycle under Runbook's autopilot (`--autopilot`, a decider that answers each stop) or
with a model adapter's replies scripted. Eleven are in the file: seven pending cycles with `decide:` entries
and `steps` the pinned Loom does not run yet, and the four bugfix cycles (two converted, two pending); the other 72 are not converted. The classes:

- Runbook's decider answers the stops: 32 entries.
- scripted replies of a model adapter or a stand-in model binary: 20.
- Runbook's autopilot loop after a stop: 9.
- the rule approver: 5.
- bindings the pinned Loom refuses (the bugfix cycles): 4, now in the file. `bindings/sdc-bugfix.yaml`
  has `gate.run` at 3600 s (D531), and the bugfix protocol records `dissection_written` and
  `repro_written` so that no check runs before its writer (`docs/bugfix.md`).
- the requester decider (2), the environment of a bound program (2), a `--dry` listing (1), a ticket
  read before the tree opens (1).

| entry | state | reason |
|---|---|---|
| decider-scope-answer | not converted | class: Runbook's decider answers the stops (`DECIDER_REPLIES`); the suite's `decide:` is a fixed list, with no model or rule decider whose prompt, retries, citations or limits a cycle can check |
| decider-model-name | not converted | class: Runbook's decider answers the stops (`LLM_MODEL_DECIDER`, `DECIDER_REPLIES`); the suite's `decide:` is a fixed list, with no model or rule decider whose prompt, retries, citations or limits a cycle can check |
| decider-scope-final | not converted | class: Runbook's decider answers the stops (`DECIDER_ESCALATE`, `DECIDER_REPLIES`); the suite's `decide:` is a fixed list, with no model or rule decider whose prompt, retries, citations or limits a cycle can check |
| decider-finding | not converted | class: Runbook's decider answers the stops (`DECIDER_REPLIES`); the suite's `decide:` is a fixed list, with no model or rule decider whose prompt, retries, citations or limits a cycle can check |
| decider-finding-final | not converted | class: Runbook's decider answers the stops (`DECIDER_ESCALATE`, `DECIDER_REPLIES`); the suite's `decide:` is a fixed list, with no model or rule decider whose prompt, retries, citations or limits a cycle can check |
| decider-finding-refused | not converted | class: Runbook's decider answers the stops (`--autopilot`); the suite's `decide:` is a fixed list, with no model or rule decider whose prompt, retries, citations or limits a cycle can check |
| run-stop | pending | the `stop`, `resume` and `reopen` steps: waits for story:tree-stop-record-and-labels |
| autopilot | not converted | class: Runbook's decider answers the stops (`RULE_APPROVALS`, `--autopilot`); the suite's `decide:` is a fixed list, with no model or rule decider whose prompt, retries, citations or limits a cycle can check |
| trace-reraised | not converted | class: Runbook's decider answers the stops (`DECIDER_REPLIES`, `--autopilot`); the suite's `decide:` is a fixed list, with no model or rule decider whose prompt, retries, citations or limits a cycle can check |
| decider-budget-reason | not converted | class: Runbook's decider answers the stops (`MAX_DECIDER_CALLS`, `--autopilot`); the suite's `decide:` is a fixed list, with no model or rule decider whose prompt, retries, citations or limits a cycle can check |
| autopilot-person | not converted | class: Runbook's decider answers the stops (`--autopilot`); the suite's `decide:` is a fixed list, with no model or rule decider whose prompt, retries, citations or limits a cycle can check |
| autopilot-stop | not converted | class: Runbook's decider answers the stops (`--autopilot`); the suite's `decide:` is a fixed list, with no model or rule decider whose prompt, retries, citations or limits a cycle can check |
| decider-cite-address | not converted | class: Runbook's decider answers the stops (`RULE_APPROVALS`, `DECIDER_REPLIES`, `--autopilot`); the suite's `decide:` is a fixed list, with no model or rule decider whose prompt, retries, citations or limits a cycle can check |
| decider-cite-retry | not converted | class: Runbook's decider answers the stops (`RULE_APPROVALS`, `DECIDER_REPLIES`, `--autopilot`); the suite's `decide:` is a fixed list, with no model or rule decider whose prompt, retries, citations or limits a cycle can check |
| decider-invalid-approval | not converted | class: Runbook's decider answers the stops (`RULE_APPROVALS`, `DECIDER_REPLIES`, `--autopilot`); the suite's `decide:` is a fixed list, with no model or rule decider whose prompt, retries, citations or limits a cycle can check |
| decider-invalid-scope | not converted | class: Runbook's decider answers the stops (`DECIDER_REPLIES`, `--autopilot`); the suite's `decide:` is a fixed list, with no model or rule decider whose prompt, retries, citations or limits a cycle can check |
| decider-escalate-nobest | not converted | class: Runbook's decider answers the stops (`RULE_APPROVALS`, `DECIDER_REPLIES`, `--autopilot`); the suite's `decide:` is a fixed list, with no model or rule decider whose prompt, retries, citations or limits a cycle can check |
| decider-reject | not converted | class: Runbook's decider answers the stops (`RULE_APPROVALS`, `DECIDER_REPLIES`, `--autopilot`); the suite's `decide:` is a fixed list, with no model or rule decider whose prompt, retries, citations or limits a cycle can check |
| decider-rework-context | not converted | class: Runbook's decider answers the stops (`RULE_APPROVALS`, `DECIDER_REPLIES`, `--autopilot`); the suite's `decide:` is a fixed list, with no model or rule decider whose prompt, retries, citations or limits a cycle can check |
| decider-rework-model | not converted | class: Runbook's decider answers the stops (`RULE_APPROVALS`, `DECIDER_REPLIES`, `--autopilot`); the suite's `decide:` is a fixed list, with no model or rule decider whose prompt, retries, citations or limits a cycle can check |
| decider-rework | pending | the scripted rejection and approval of `merge.approve` need `approve` with `result` and `reasons`: waits for story:tree-stop-record-and-labels |
| decider-rework-spent | pending | three scripted rejections of `merge.approve`: waits for story:tree-stop-record-and-labels (`approve.result`) |
| rework-cap | not converted | class: Runbook's decider answers the stops (`MAX_REJECTIONS`, `DECIDER_REPLIES`, `--autopilot`); the suite's `decide:` is a fixed list, with no model or rule decider whose prompt, retries, citations or limits a cycle can check |
| rework-not-called | not converted | class: Runbook's decider answers the stops (`DECIDER_REPLIES`, `--autopilot`); the suite's `decide:` is a fixed list, with no model or rule decider whose prompt, retries, citations or limits a cycle can check |
| review-no-rework-prompt | not converted | class: Runbook's decider answers the stops (`DECIDER_REPLIES`, `--autopilot`); the suite's `decide:` is a fixed list, with no model or rule decider whose prompt, retries, citations or limits a cycle can check |
| cite-dark-spot | pending | the intake finding is decided by the decider in the old cycle; here it stops for a person's decision written into `findings.yaml` (gap, no Loom story yet) |
| cite-fault-sent-back | pending | same: the intake finding stops for a person's decision (gap, no Loom story yet) |
| autopilot-close | not converted | class: Runbook's decider answers the stops (`DECIDER_REPLIES`, `--autopilot`); the suite's `decide:` is a fixed list, with no model or rule decider whose prompt, retries, citations or limits a cycle can check |
| autopilot-close-a3 | not converted | class: Runbook's decider answers the stops (`AUTONOMY`, `DECIDER_REPLIES`); the suite's `decide:` is a fixed list, with no model or rule decider whose prompt, retries, citations or limits a cycle can check |
| conformance-default-off | not converted | class: Runbook's decider answers the stops (`CONFORMANCE`, `DECIDER_REPLIES`, `--autopilot`); the suite's `decide:` is a fixed list, with no model or rule decider whose prompt, retries, citations or limits a cycle can check |
| rule-approvals | not converted | class: the rule approver of Runbook (rule approvals on or off, decided by a rule of the driver); the suite's `decide:` has no rule approver to switch |
| rule-approvals-check-failed | not converted | class: the rule approver of Runbook (a tampering `sdc-drive` on the drive path); the suite's `decide:` has no rule approver to switch |
| rule-approvals-revision-changed | not converted | class: the rule approver of Runbook (a tampering `sdc-drive` on the drive path); the suite's `decide:` has no rule approver to switch |
| rule-approvals-off | not converted | class: the rule approver of Runbook (rule approvals on or off, decided by a rule of the driver); the suite's `decide:` has no rule approver to switch |
| rule-approvals-a3 | not converted | class: the rule approver of Runbook (rule approvals on or off, decided by a rule of the driver); the suite's `decide:` has no rule approver to switch |
| keep-going-three | not converted | class: Runbook's autopilot loop after a stop (`--keep-going`, the unstick step, `--reopen` of a refused decision): it re-runs the tree after a stop, which a cycle's `steps` cannot decide from a model's reply |
| keep-going-off | not converted | class: Runbook's autopilot loop after a stop (`--keep-going`, the unstick step, `--reopen` of a refused decision): it re-runs the tree after a stop, which a cycle's `steps` cannot decide from a model's reply |
| keep-going-repeat | not converted | class: Runbook's autopilot loop after a stop (`--keep-going`, the unstick step, `--reopen` of a refused decision): it re-runs the tree after a stop, which a cycle's `steps` cannot decide from a model's reply |
| unstick-end | not converted | class: Runbook's autopilot loop after a stop (`--keep-going`, the unstick step, `--reopen` of a refused decision): it re-runs the tree after a stop, which a cycle's `steps` cannot decide from a model's reply |
| unstick-rerun | not converted | class: Runbook's autopilot loop after a stop (`--keep-going`, the unstick step, `--reopen` of a refused decision): it re-runs the tree after a stop, which a cycle's `steps` cannot decide from a model's reply |
| unstick-invalid | not converted | class: Runbook's autopilot loop after a stop (`--keep-going`, the unstick step, `--reopen` of a refused decision): it re-runs the tree after a stop, which a cycle's `steps` cannot decide from a model's reply |
| unstick-limit | not converted | class: Runbook's autopilot loop after a stop (`--keep-going`, the unstick step, `--reopen` of a refused decision): it re-runs the tree after a stop, which a cycle's `steps` cannot decide from a model's reply |
| unstick-off | not converted | class: Runbook's autopilot loop after a stop (`--keep-going`, the unstick step, `--reopen` of a refused decision): it re-runs the tree after a stop, which a cycle's `steps` cannot decide from a model's reply |
| reopen-refused-decision | not converted | class: Runbook's autopilot loop after a stop (`--keep-going`, the unstick step, `--reopen` of a refused decision): it re-runs the tree after a stop, which a cycle's `steps` cannot decide from a model's reply |
| decider-release-a3 | not converted | class: Runbook's decider answers the stops (`AUTONOMY`, `DECIDER_REPLIES`); the suite's `decide:` is a fixed list, with no model or rule decider whose prompt, retries, citations or limits a cycle can check |
| decider-release-withheld | not converted | class: Runbook's decider answers the stops (`RULE_APPROVALS`, `DECIDER_REPLIES`, `--autopilot`); the suite's `decide:` is a fixed list, with no model or rule decider whose prompt, retries, citations or limits a cycle can check |
| requester-decider | not converted | class: the requester decider answers from goals (`--goals`, `REQUESTER_REPLIES`); no suite member says goals |
| requester-no-ground | not converted | class: the requester decider answers from goals (`--goals`, `REQUESTER_REPLIES`); no suite member says goals |
| models | not converted | class: a `--dry` listing of the models per class, not a run of a tree |
| decider-question-reject | not converted | class: Runbook's decider answers the stops (`DECIDER_REPLIES`, `--autopilot`); the suite's `decide:` is a fixed list, with no model or rule decider whose prompt, retries, citations or limits a cycle can check |
| rework-fresh-allowance | pending | two scripted rejections of `merge.approve`: waits for story:tree-stop-record-and-labels (`approve.result`) |
| scope-unattributed-reject | not converted | class: Runbook's decider answers the stops (`DECIDER_REPLIES`, `--autopilot`); the suite's `decide:` is a fixed list, with no model or rule decider whose prompt, retries, citations or limits a cycle can check |
| finding-refused-reask | not converted | class: Runbook's decider answers the stops (`--autopilot`); the suite's `decide:` is a fixed list, with no model or rule decider whose prompt, retries, citations or limits a cycle can check |
| rework-spent-dropped | not converted | class: Runbook's decider answers the stops (`DECIDER_REPLIES`, `--autopilot`); the suite's `decide:` is a fixed list, with no model or rule decider whose prompt, retries, citations or limits a cycle can check |
| rework-spent-abandoned | not converted | class: Runbook's decider answers the stops (`RULE_APPROVALS`, `DECIDER_REPLIES`, `--autopilot`); the suite's `decide:` is a fixed list, with no model or rule decider whose prompt, retries, citations or limits a cycle can check |
| review-invalid-named | not converted | class: scripted replies of a model adapter or a stand-in model binary (`SDC_LLM_NODES`): the `fixtures` member waits for story:engine-agents-section and the `calls` source for story:agent-node-run-records |
| writer-own-error | not converted | class: scripted replies of a model adapter or a stand-in model binary (`SDC_LLM_NODES`): the `fixtures` member waits for story:engine-agents-section and the `calls` source for story:agent-node-run-records |
| implement-percause | pending | stops at the finding that `amend.upstream` raises after a test failure, exit 3: a person's decision written into `findings.yaml` between two runs (gap, no Loom story yet) |
| writer-sees-test-failure | not converted | class: scripted replies of a model adapter or a stand-in model binary (`SDC_LLM_NODES`): the `fixtures` member waits for story:engine-agents-section and the `calls` source for story:agent-node-run-records |
| implement-request-build | not converted | class: scripted replies of a model adapter or a stand-in model binary (`SDC_IMPLEMENT_MODE`, `SDC_LLM_NODES`): the `fixtures` member waits for story:engine-agents-section and the `calls` source for story:agent-node-run-records |
| implement-request-tests-rework | not converted | class: scripted replies of a model adapter or a stand-in model binary (`SDC_IMPLEMENT_MODE`, `SDC_LLM_NODES`): the `fixtures` member waits for story:engine-agents-section and the `calls` source for story:agent-node-run-records |
| implement-request-review-rework | not converted | class: scripted replies of a model adapter or a stand-in model binary (`SDC_IMPLEMENT_MODE`, `SDC_LLM_NODES`): the `fixtures` member waits for story:engine-agents-section and the `calls` source for story:agent-node-run-records |
| implement-request-invalid-retry | not converted | class: scripted replies of a model adapter or a stand-in model binary (`SDC_IMPLEMENT_MODE`, `SDC_LLM_NODES`): the `fixtures` member waits for story:engine-agents-section and the `calls` source for story:agent-node-run-records |
| implement-request-invalid-named | not converted | class: scripted replies of a model adapter or a stand-in model binary (`SDC_IMPLEMENT_MODE`, `SDC_LLM_NODES`): the `fixtures` member waits for story:engine-agents-section and the `calls` source for story:agent-node-run-records |
| implement-request-path-escape | not converted | class: scripted replies of a model adapter or a stand-in model binary (`SDC_IMPLEMENT_MODE`, `SDC_LLM_NODES`): the `fixtures` member waits for story:engine-agents-section and the `calls` source for story:agent-node-run-records |
| coverage-complete | not converted | class: scripted replies of a model adapter or a stand-in model binary (`SDC_IMPLEMENT_COVERAGE`, `SDC_LLM_NODES`): the `fixtures` member waits for story:engine-agents-section and the `calls` source for story:agent-node-run-records |
| coverage-not-built | not converted | class: scripted replies of a model adapter or a stand-in model binary (`SDC_IMPLEMENT_COVERAGE`, `SDC_LLM_NODES`): the `fixtures` member waits for story:engine-agents-section and the `calls` source for story:agent-node-run-records |
| coverage-line-outside | not converted | class: scripted replies of a model adapter or a stand-in model binary (`SDC_IMPLEMENT_COVERAGE`, `SDC_LLM_NODES`): the `fixtures` member waits for story:engine-agents-section and the `calls` source for story:agent-node-run-records |
| coverage-test-missing | not converted | class: scripted replies of a model adapter or a stand-in model binary (`SDC_IMPLEMENT_COVERAGE`, `SDC_LLM_NODES`): the `fixtures` member waits for story:engine-agents-section and the `calls` source for story:agent-node-run-records |
| coverage-spent | not converted | class: scripted replies of a model adapter or a stand-in model binary (`SDC_IMPLEMENT_COVERAGE`, `SDC_LLM_NODES`): the `fixtures` member waits for story:engine-agents-section and the `calls` source for story:agent-node-run-records |
| coverage-reply-retry | not converted | class: scripted replies of a model adapter or a stand-in model binary (`SDC_IMPLEMENT_COVERAGE`, `SDC_LLM_NODES`): the `fixtures` member waits for story:engine-agents-section and the `calls` source for story:agent-node-run-records |
| review-real-path | not converted | class: scripted replies of a model adapter or a stand-in model binary (`METAHARNESS` and `CLAUDE`, `SDC_LLM_NODES`): the `fixtures` member waits for story:engine-agents-section and the `calls` source for story:agent-node-run-records |
| review-real-path-facts | not converted | class: scripted replies of a model adapter or a stand-in model binary (`METAHARNESS` and `CLAUDE`, `SDC_REVIEW_MODE`, `SDC_LLM_NODES`): the `fixtures` member waits for story:engine-agents-section and the `calls` source for story:agent-node-run-records |
| review-invalid-stop | not converted | class: scripted replies of a model adapter or a stand-in model binary (`METAHARNESS` and `CLAUDE`, `SDC_LLM_NODES`): the `fixtures` member waits for story:engine-agents-section and the `calls` source for story:agent-node-run-records |
| review-adapter-refused | not converted | class: scripted replies of a model adapter or a stand-in model binary (`METAHARNESS` and `CLAUDE`, `SDC_REVIEW_ADAPTER`, `SDC_LLM_NODES`): the `fixtures` member waits for story:engine-agents-section and the `calls` source for story:agent-node-run-records |
| reopen-review-invalid | not converted | class: scripted replies of a model adapter or a stand-in model binary (`METAHARNESS` and `CLAUDE`, `SDC_REVIEW_ADAPTER`, `SDC_LLM_NODES`): the `fixtures` member waits for story:engine-agents-section and the `calls` source for story:agent-node-run-records |
| bugfix-full | converted | the seeds are `cycles/seeds/bugfix-pager/` (the workspace `sdc-bugfix seed` writes); the case ends `fixed` with one red, one fix and one passed suite run |
| bugfix-not-red | pending | still failing: `expect.outcome (outcome, OutcomeDiffers): expected fixed, found not_reproduced`: the first `repro_run` `not_red` makes `not_reproduced` legitimate and Loom ends the case before the test writer is sent back; the protocol has no claim for "the writer is spent" |
| bugfix-edited-test | pending | still failing: `expect.outcome (outcome, OutcomeDiffers): expected fixed, found not_fixed`: the first `test_run` is `failed` and Loom ends the case `not_fixed` before `change.fix` is sent back; same missing "writer is spent" |
| bugfix-never-red | converted | ends `not_reproduced` with no red, no fix and no suite run; Loom ends the case at the first legitimate outcome, after one `test.reproduce` run (the old cycle ran the writer to its allowance) |
| bugfix-from-ticket | not converted | class: a ticket read by `sdc-bugfix` before the tree opens (`--bugfix-ticket`) and a computed input; the format seeds files only |
| bugfix-pool-wait | not converted | class: environment of a bound program (a stand-in `slot` first on `PATH`) on a bugfix run; the format has no member for it, and the bugfix bindings are refused as above |
| bugfix-over-time | not converted | class: environment of a bound program (a stand-in `slot` first on `PATH`) on a bugfix run; the format has no member for it, and the bugfix bindings are refused as above |
