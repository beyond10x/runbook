# Parity of a run under the old driver and under Loom

`story:parity-run-on-loom`. The comparison asks whether a case tree reaches the same facts under the old
driver (`sdc-drive`) and under Loom. When it does, the driver's runtime copy leaves Runbook. This page is
the driver's half: the summary, the rules and the commands. Loom's reader of the same summary is the
story's next step.

## The summary

`runbook.parity-summary/1`, JSON, stable key order, no absolute path and no time. Members:

- `format`, `source` (the runtime and its run settings), `not_recorded` (member paths the runtime
  records nothing for; for the driver: the question and the options of a decision request).
- `cases[]`, root first, then each case in the order the log opened it: `id`, `link` (the name of the
  case link: the driver names a child `<parent id>.<name>`; null for the root), `parent`, `protocol`,
  `protocol_revision`, `outcome` (null while the case runs on), `stop` (null, or `{cause}`:
  `awaiting_person`, `awaiting_authority`, a spent budget), `open_obligations[]`, and per case:
  - `evidence[]` in the order received (the case log: the record of a child opened, then the records of
    the action finished): `index`, `kind`, `result`, `subject`, `subject_revision`,
    `upstream_revisions`, `action` (the action that produced it).
  - `program_runs[]`: `action`, `attempt` (n-th run of that action in the case), `exit`,
    `retry_followed` (the next run of the action was sent back with feedback). An approval is a person
    step, not a program run.
  - `person_steps[]`: `order` (position among all person steps of the run, by the log's virtual
    clock), `action`, `capability` (what the protocol requires), `answer` (the result the step
    recorded), `actor` (the principal; under the stub settings `person:stand-in`). A capability a
    person granted without an action of their own (`approve` without `--action`; the driver then runs
    the action itself, as for `release.publish`) is a step with `action` null and `answer` `granted`.
- `copies[]`: a record of one case recorded into another (`action`, `kind`, `from_protocol`, `from`,
  `to`, `subject`, `subject_revision`).
- `decision_requests[]`: `decision_recorded` events (`id`, `case`, `question`, `options`, `answer`, `who`).
- `protocols[]`: `id`, `revision`, `digest` (of the compiled protocol).
- `product_tree[]`: sorted relative paths of the product (the root case's `work/`, without `target/`,
  `node_modules/` and `.git/`, and aep's `.aep-planning-writer-*.lock`, whose name is a digest of the absolute
  path) with the sha256 of each. In a text file every stamp `YYYY-MM-DDTHH:MM:SSZ` is read as `<time>`
  before digesting: `aep` writes the wall clock into the transitions of every planning file of the
  product, so two stub runs differ in those files and in nothing else (found on `stub-cycle`).

## The rules

`runbook parity compare --rules` prints the table the code compares by (`RULES` in
`runbook/src/parity.rs`). Three ways: equal by value (kind, result, action, outcome, order, exit
status, file list); equal by pattern (revisions, case ids, request ids and protocol digests are values
a run generates: each distinct value is renamed `r1`, `r2`, ... per class in order of first appearance,
on both sides, then compared, so that "the green names the red's revision" is compared and the digests
are not); never compared (`source`, `not_recorded`). File digests of the product tree are compared only
with `--product-digests`: a stub run is deterministic, a model run is not. A test fails when a member of
a summary has no row.

## How to run it

```
RUNS=<dir>/runs scripts/run-full-cycle.sh stub-cycle <dir>/store-stub-cycle
runbook parity summarize --driver <dir>/runs/stub-cycle --store <dir>/store-stub-cycle --output a.json
runbook parity compare --product-digests a.json b.json
runbook parity approvals a.json
```

`compare` exits 0 and prints one `equal:` line, or exits 1 and prints one line per difference: the member
path, a's value, b's value. Exit 2 is an input that cannot be read. `approvals` prints the person steps in
order (`path` from the root, `case` link name, `action`, `capability`, `answer`, `actor`): the
`tree approve` calls a run under Loom must make to follow the same path.
