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
  `kind`, the script's `lines` and a short `note`. Kinds: `aggregate`, `event-order`, `tree-search`,
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
