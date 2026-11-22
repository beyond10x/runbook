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
