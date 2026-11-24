# Parity of a run under the old driver and under Loom

`story:parity-run-on-loom` (status `active`). The comparison asks whether a case tree reaches the
same facts under the old driver and under Loom. The runs it measured, on the cycle `stub-cycle`
with stub executors, compared equal on what both sides record (the sections from "First equal
comparison" on); five items of "What the first real run needs" are still marked **Open**. The
driver's runtime copy is deleted from Runbook (`story:driver-runtime-removed`): no new driver run
can be made, and the driver side of a comparison is a run directory the driver recorded earlier.
The sections from "First equal comparison" on are records of measured runs, written when the driver
binary and its scripts existed; they name those files as they were then. This page holds the
summary, the rules, the commands, the table of what each Loom member becomes, and what the first
real run under Loom still has to confirm.

## The summary

`runbook.parity-summary/1`, JSON, stable key order, no absolute path and no time. Members:

- `format`, `source` (the runtime and its run settings), `not_recorded` (member paths the runtime
  records nothing for; for the driver: the question and the options of a decision request; for Loom:
  the rows of the table below marked `(not recorded)`, and, when the summary is read from the status
  alone (`--loom-status`), the rows that name the run directory). A member listed there holds null
  or an empty list, and that is no fact: `compare` does not compare it. A summary read from the run
  directory of `runbook run` (`--loom-run`) records the run-directory members and lists only
  `decision_requests[].question` and `.options`, as the driver does.
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
  path; and everything under `.sdc/loom-feedback/`, see "The rules") with the sha256 of each. In a text file every stamp `YYYY-MM-DDTHH:MM:SSZ` is read as `<time>`
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

Two named rules came from the first real run under Loom (`stub-cycle`); a third, for the revision of a
child's `case_outcome` record, is described under "Known differences of the two runtimes" below:

- Registration stem (`cases[].protocol`, `copies[].from_protocol`, `protocols[].id`). The driver records
  a protocol's declared id (`sdc.intake`), Loom the name the protocol is registered under (`sdc-intake`:
  the id with `.` and `_` written as `-`). `compare` reads both sides as stems, so the two spellings are
  equal; the summaries keep what each runtime recorded. The rule cannot tell apart two ids that differ
  only in `.`, `_` and `-` (`sdc.intake`, `sdc_intake` and `sdc-intake` are one id to it). Two ids that
  differ in a letter or a digit still differ.
- Feedback copies (`product_tree[.sdc/loom-feedback/**]`). In Loom mode `sdc-node`
  copies the document that `LOOM_RETRY_FEEDBACK` names, and the log tails it cites, into
  `.sdc/loom-feedback/<16 hex>/` in the workspace (`sdc-node`; the directory
  name is the first 16 hex digits of the document's sha256, so it differs between runs). Runbook's own
  script writes the files. Loom sets the variable on a writer step whenever the case holds records of
  the kinds and results the writer's `retry_when` lists, at a revision of their subject that is not
  `none` (Loom `2c0a6f48`, `crates/loom-bindings/src/retry.rs:48-84`, the variable set at `:191-206`
  and `child.rs:120`). It does not wait for a retry: in the `stub-cycle` run the first run of the plan
  writer (`plan.decompose`) found `decomposition_coverage` `uncovered`, `stories_scoped` `unassessed`
  and `story_states` `draft` (`bindings/sdc-plan.yaml:33-37`), and the three files are that
  document and the tails of its two logs. `retry_followed` counts retries taken (a next run of the
  action), which is not the same fact. The driver hands the same records to the writer in the variable
  `SDC_FEEDBACK` as a file in the case's `blobs/` directory, not in the workspace
  (`driver/src/main.rs:549-565` chooses the records, `:1512-1547` hands them over); its first
  `plan.decompose` run names the same three kinds in `retry_of`. The listing of the product tree
  leaves out everything under `.sdc/loom-feedback/`, on both sides, where the summary is written
  (`walk_product`); it is the transport of feedback, not product. Another directory under `.sdc/` is
  listed as before, except `.sdc/records/` and the one file `.sdc/rework-seen`.
- Record files (`product_tree[.sdc/records/**]`). Under Loom a bound program gets its working
  directory and no case store, so `sdc-tool.sh` writes the records that the inputs of `change.review`
  read (a test run's log, the gate's log) with `sdc-reply record` into `.sdc/records/` of the
  workspace (`docs/loom-bindings.md`). The driver keeps the same facts in its case store and writes
  no file. The log of a test run holds the order of the tests and the elapsed time of that one run,
  so two runs of the same cycle differ in it (the child and the library run of `task
  test:loom-in-process` differed in `.sdc/records/conformance_run/<16 hex>.log`, 2026-11-22). The
  listing of the product tree leaves out `.sdc/records/` on both sides, like `.sdc/loom-feedback/`.
- The rework ledger (`product_tree[.sdc/rework-seen]`). Under Loom the stand-in model program
  (`sdc-node`) numbers a person's nth rejection of one approval in its note by the evidence ids of
  the rejections it numbered before, and keeps them in `.sdc/rework-seen` of the tree, one
  `<approval> <id>` per line (`docs/driver.md`, "The ledger of the stub's numbering"). It writes the file
  only when `LOOM_RETRY_FEEDBACK` names rejection records, which only Loom sets; the driver numbers a
  rework from its own rejection record and writes no file, so a run that rejects a person step lists
  this one file on Loom's side only. The file lies in the tree of the workspace: it moves the revision
  only of an artifact that places the whole workspace or `.sdc`, and no committed binding places either
  (`docs/driver.md`). The listing of the product tree leaves out this one path on both sides, where
  the summary is written (`walk_product`, `LOOM_ONLY_FILES`); the rule names the path and not `.sdc/`,
  so any other file under `.sdc/` is listed as before.

A third answer besides equal and different: a member exactly one side lists under `not_recorded` (or
lies below a member it lists) is not compared, and `compare` names it on a line
`not recorded on one side: <member> (not recorded by a|b)`. When nothing differs the first line then
reads `equal on what both sides record:` instead of `equal:`, and the exit is 0. A member both sides
list is skipped and not named. The driver lists `decision_requests[].question` and `.options`, a
Loom summary read from the status alone lists `decision_requests`: the driver's two are below Loom's,
so only Loom's is named. A member of the entries of a list that both summaries hold none of
(`decision_requests[].question` when both lists are empty) is not named: nothing is lost.

The last line of `compare` closes it: `parity: identical` (no difference and no member named: exit
0), `parity: equal on what both sides record` (no difference, a member named: exit 0) or `parity:
different` (exit 1).

## Known differences of the two runtimes

Differences that are not a defect of either side and that `compare` is built to read past. Each goes
away with the driver's runtime.

- The record a child's outcome leaves in its parent (`case_outcome`; named rule
  `cases[].evidence[case_outcome].subject_revision` and `.upstream_revisions.*`). The driver writes it
  with the parent's revision of the subject at the moment the child is opened and keeps it
  (`driver/src/main.rs:1022-1028`). Loom writes it at the opening too and replaces its subject
  revision at the child's termination with the revision the terminated child holds for that subject
  (Loom at `64e8f5e2`, `crates/loom-governor/src/children.rs:480-484,605-614,707-718,734-754`). Loom's
  reading is the intended one (the outcome is about what the child produced) and the driver is not
  changed. Example: epic case `billing`, evidence index 2, the `case_outcome` on `spec`: the driver
  holds no revision (`none`), Loom the specification's tree revision. `compare` takes the
  `subject_revision` and the `upstream_revisions` of every evidence record of the kind `case_outcome`
  out of both summaries before it names the labels `r1`, `r2`, ... in order of first appearance, so
  such a record is neither compared nor counted and the labels of the records after it stay aligned.
  The summary holds no mark of which action opened a child (it keeps no child id on a record), so the
  records are found by their kind; both readers name it so, and in the first run all 7 `case_outcome`
  records belong to the 7 child cases and no other record has that kind. The summaries themselves keep
  what each runtime recorded.
- Directory revisions differ in value between the runtimes because the algorithms differ (the driver's
  `tree_digest`, `driver/src/main.rs:274-293`; Loom's `crates/loom-bindings/src/revision.rs:80-157`);
  the comparison by order of first appearance is what makes them comparable.
- File digests with `--product-digests` need no rule for the wall clock: a stamp
  `YYYY-MM-DDTHH:MM:SSZ` in a text file is read as `<time>`, so the two files of the plan that carry
  an `at:` (`aep/epic/billing.md`, `aep/story/invoices.md`) compare equal in the first run.

## How to run it

The driver side reads the run directory and case store that the driver recorded earlier. The script
that made them, `scripts/run-full-cycle.sh`, is deleted, so only a run that already exists can be
read:

```
runbook parity summarize --driver <dir>/runs/stub-cycle --store <dir>/store-stub-cycle --output a.json
b10x-loom tree status --store <loom store> > status.json
runbook parity summarize --loom-status status.json --product-tree <product dir> --output b.json
runbook parity compare --product-digests a.json b.json
runbook parity approvals a.json
```

For a run of `runbook run` read the run directory instead of the status alone:

```
runbook parity summarize --loom-run <runs-dir>/<name> --output b.json
```

`--loom-run <dir>` reads `tree-status.json` and `work/` (the product tree) of the directory, and
`run-settings.jsonl`, `decisions.jsonl` and `protocols/*.protocol.yaml`, which hold what the status
does not (table below, "run directory" in the `Loom member` column). A file missing, a settings line
that does not parse, a protocol file with no `protocol.id` or `revision`, a protocol a case runs for
which `protocols/` holds no file, a decision of another kind than an approval (`Approve`,
`ApproveAction`), and a copy whose record is not in the target case's evidence are errors that name
the file or the case, never a default. `--loom-run` does not go with `--loom-status`,
`--product-tree`, `--driver` or `--store` (exit 64).

`--loom-status <file>` is the JSON `tree status` printed. The reader takes nothing else of Loom: no store
file, no run log. `--product-tree <dir>` is the directory walked for `product_tree[]`; without it the
status's `workspace` is walked. `--loom-status` does not go with `--driver` or `--store`.

`compare` exits 0 and prints one `equal:` line, or exits 1 and prints one line per difference: the member
path, a's value, b's value. Exit 2 is an input that cannot be read. `summarize` exits 2 and names the
member and the case when `tree status` leaves out a member the reader needs (a case without `name`, a
run without `exit`, ...), and names the format when it is not `loom.tree-status/1`. `approvals` prints the person steps in
order (`path` from the root, `case` link name, `action`, `capability`, `answer`, `actor`): the
`tree approve` calls a run under Loom must make to follow the same path.

## From Loom's status to the summary

Read at Loom commit `1016c251` (`crates/loom-tree/src/status.rs`, `encode.rs`, `read.rs`); the first
equal comparison read a status of Loom `2c0a6f48` with the same reader. The table
is `RULES` in `runbook/src/parity.rs` printed by `runbook parity compare --rules`: the columns
`Loom member` and `Rule` are its last two, and a test fails when this page and the code differ. The
`Driver` column is where the driver reader finds the member. `[]` is any list index. A Loom member
named `cases[].x` is a member `x` of one entry of `cases[]`. A rule that names words (`Allow = granted`)
is the one place the two vocabularies meet: another word is an error, not a default.

Not one-to-one: `cases[].stop` (Loom keeps the last stop even after the case went on, and has one
`BudgetSpent` where the driver names the budget), `cases[].evidence[].action` (a child's `case_outcome`
record has none in Loom), `program_runs[].attempt` (counted, not read), `person_steps[].order` and
`.answer` (by the instant recorded; the answer of an approval with an action is the record's result),
`copies[]` (the entry names no record: the three members come from the target case's evidence), `protocols[].digest`, `source.run_settings` and `decision_requests` (the run directory, not the status).

| Summary member | Driver | Loom member | Rule |
|---|---|---|---|
| `format` | constant | format | must be loom.tree-status/1, else refused by name; the summary's format is its own |
| `source` | `run-settings.jsonl`, first line | format, version | runtime loom, status_version the version; no path of the store is copied |
| `source.run_settings` | `run-settings.jsonl`, first line | run directory: run-settings.jsonl | the first line, read as decider kind, adapter, stand_in (a decider that is not a model) and its principal; tree status alone holds no run settings (not recorded) |
| `not_recorded` | constant: the question and the options of a decision request | the rows of this table marked not recorded | listed in the order of this table |
| `cases[].id` | `case.json` `id` | cases[].case | as is; Loom ids (root, root.first) and the driver's compare by pattern |
| `cases[].link` | the case id without the parent's id and a dot | cases[].name | as is; none on the root (null); another case without a name is an error |
| `cases[].parent` | the case directory it lies below | cases[].parent | as is; none on the root only; a parent that is not a case of the tree is an error |
| `cases[].protocol` | `case.json` `protocol` | cases[].protocol_id | as is (the id without @revision); compared by registration stem |
| `cases[].protocol_revision` | `opened` event, `protocol.revision` | cases[].protocol_revision | text to integer; anything else is an error |
| `cases[].outcome` | `case.json` `termination` | cases[].outcome | as is; none is null (the case runs on) |
| `cases[].stop` | newest evaluation, `bindings.json`, last budget event | cases[].stop | null when the case has an outcome (its last stop is then old); none without an outcome is null |
| `cases[].stop.cause` | newest evaluation, `bindings.json`, last budget event | cases[].stop | the stop's name in the words of the summary; another name is an error (AwaitingPerson = awaiting_person, AwaitingAuthority = awaiting_authority, Stuck = stopped, BudgetSpent = budget_spent) |
| `cases[].open_obligations[]` | newest evaluation, obligations with status open | cases[].open_obligations[] | as is |
| `cases[].evidence[].index` | position in the order the log names the records | cases[].evidence[].index | as is |
| `cases[].evidence[].kind` | `evidence/<id>.json` | cases[].evidence[].kind | as is |
| `cases[].evidence[].result` | `evidence/<id>.json` | cases[].evidence[].result | as is; none is null |
| `cases[].evidence[].subject` | `evidence/<id>.json` | cases[].evidence[].subject | as is |
| `cases[].evidence[].subject_revision` | `evidence/<id>.json` | cases[].evidence[].subject_revision | as is |
| `cases[].evidence[].upstream_revisions.*` | `evidence/<id>.json` | cases[].evidence[].upstream_revisions.* | as is |
| `cases[].evidence[case_outcome].subject_revision` | `evidence/<id>.json` | cases[].evidence[].subject_revision | taken out of both summaries before the labels are named; a known difference of the two runtimes (docs/parity.md) |
| `cases[].evidence[case_outcome].upstream_revisions.*` | `evidence/<id>.json` | cases[].evidence[].upstream_revisions.* | taken out of both summaries before the labels are named, as the subject_revision |
| `cases[].evidence[].action` | the action started or finished that names the record | cases[].evidence[].action | as is, none is null; a case_outcome record with none takes the action of the next record (the opening step's own) |
| `cases[].program_runs[].action` | `action_finished` events with an argv and no principal | cases[].runs[].action | as is |
| `cases[].program_runs[].attempt` | `action_finished` events with an argv and no principal | (counted) | n-th run of the action in the case's runs[]; Loom's attempt counts within a retry chain and is not read |
| `cases[].program_runs[].exit` | `action_finished` events with an argv and no principal | cases[].runs[].exit | as is; none is an error (no status to compare) |
| `cases[].program_runs[].retry_followed` | `action_finished` events with an argv and no principal | cases[].runs[].retried | as is |
| `cases[].person_steps[].order` | `action_finished` with a principal, `authority_recorded`, `ir.json` | cases[].approvals[].at | position by (at, case position, approval position) over every approval of the tree |
| `cases[].person_steps[].action` | `action_finished` with a principal, `authority_recorded`, `ir.json` | cases[].approvals[].action | as is; none is null (a grant) |
| `cases[].person_steps[].capability` | `action_finished` with a principal, `authority_recorded`, `ir.json` | cases[].approvals[].capability | as is |
| `cases[].person_steps[].answer` | `action_finished` with a principal, `authority_recorded`, `ir.json` | cases[].approvals[].answer | Allow with an action: the result of the n-th record of that action in the case; Allow without: granted; Deny: denied (Allow = granted, Deny = denied) |
| `cases[].person_steps[].actor` | `action_finished` with a principal, `authority_recorded`, `ir.json` | cases[].approvals[].actor | as is; equal only when the Loom run answered with the principal the driver run used: under `runbook run` that is `decider.principal` of the settings for a scripted decider (default of its answers), `decider:model` or `decider:<verdict>` for a model or rule decider, and the principal Loom printed (`operator` for a capability approval) when the settings name none |
| `copies[].action` | `action_started` with `copy`, then `action_finished` | cases[].evidence[].action | the n-th record of copies[].kind in the target case whose action the source case's records of that kind do not name; the entry itself names source_case, target_case and record_kind only |
| `copies[].kind` | `action_started` with `copy`, then `action_finished` | cases[].copies[].record_kind | as is |
| `copies[].from_protocol` | `action_started` with `copy`, then `action_finished` | cases[].copies[].source_case | protocol_id of that case; compared by registration stem |
| `copies[].from` | `action_started` with `copy`, then `action_finished` | cases[].copies[].source_case | as is |
| `copies[].to` | `action_started` with `copy`, then `action_finished` | cases[].copies[].target_case | as is |
| `copies[].subject` | `action_started` with `copy`, then `action_finished` | cases[].evidence[].subject | that record of the target case |
| `copies[].subject_revision` | `action_started` with `copy`, then `action_finished` | cases[].evidence[].subject_revision | that record of the target case |
| `decision_requests` | `decision_recorded` events | run directory: decisions.jsonl | one entry per answered decision that is a question; the runner answers approvals only, so the list is empty; a row of type question or finding (the decider answering a request of the answers file) is read and compared to nothing, a row of another type is an error; tree status alone holds no decision record (not recorded) |
| `decision_requests[].id` | `decision_recorded` events | (under a member not recorded) | an empty list |
| `decision_requests[].case` | `decision_recorded` events | (under a member not recorded) | an empty list |
| `decision_requests[].question` | `decision_recorded` events | (under a member not recorded) | an empty list |
| `decision_requests[].options[]` | `decision_recorded` events | (under a member not recorded) | an empty list |
| `decision_requests[].answer` | `decision_recorded` events | (under a member not recorded) | an empty list |
| `decision_requests[].who` | `decision_recorded` events | (under a member not recorded) | an empty list |
| `protocols[].id` | `opened` event | cases[].protocol_id | the distinct protocols of the cases, by id then revision; compared by registration stem |
| `protocols[].revision` | `opened` event | cases[].protocol_revision | as for the case |
| `protocols[].digest` | `opened` event | run directory: protocols/*.protocol.yaml | sha256 of the file the runner copied for that id and revision; the status protocols[] is empty and is not read (not recorded) |
| `product_tree[].path` | the root case's `work/` | --product-tree, else workspace | the directory walked as the driver's work/ is |
| `product_tree[.sdc/loom-feedback/**]` | the root case's `work/` | --product-tree, else workspace | left out of the listing of both sides when the summary is written |
| `product_tree[.sdc/records/**]` | the root case's `work/` | --product-tree, else workspace | left out of the listing of both sides when the summary is written |
| `product_tree[.sdc/rework-seen]` | the root case's `work/` | --product-tree, else workspace | left out of the listing of both sides when the summary is written; this one path, no other file under .sdc/ |
| `product_tree[].sha256` | the root case's `work/` | --product-tree, else workspace | the same digest of the same file |

Where the seven values come from now (each row of the table above names its source): the six
members that `tree status` holds nothing for, and the principal.

1. `source.run_settings`: the first line of `run-settings.jsonl` of the run directory (decider kind,
   adapter, stand-in flag, stand-in principal).
2. `copies[].action`, 3. `copies[].subject`, 4. `copies[].subject_revision`: the record of that kind
   in the target case's evidence. The status entry names `source_case`, `target_case` and
   `record_kind` only, and the evidence of the target case holds the copied record with the three
   members. The n-th copy of a kind into a case is the n-th record of that kind in the case that the
   source case's own action did not write.
5. `decision_requests`: `decisions.jsonl`; the runner answers approvals only, which are no request, so
   the list is empty; a row of type `question` or `finding` (the decider's answer to a request of the
   answers file) is read and compared to nothing; a row of another type or kind is an error.
6. `protocols[].digest`: the sha256 of the copy of the protocol file in `protocols/` of the run
   directory, per id and revision.
7. The principal of an answer (`cases[].person_steps[].actor`): `decider.principal` of the settings
   for a scripted decider, `decider:model` or `decider:<verdict>` for a model or rule decider; nothing
   is fixed in the runner for the cycle. A capability approval Loom prints for a principal (`operator`
   for `release.publish`) keeps that principal when the settings name none.

Read from the status alone (`--loom-status`), the six are not recorded and `compare` names each as
`not recorded on one side` and compares nothing there.

## First equal comparison (2026-11-22)

`story:parity-first-run`. The cycle `stub-cycle` was run twice from one export of Runbook commit
`ee90434` (the tools and the bindings), once by the old driver and once under Loom, and the two
summaries compare equal on what both sides record.

- Driver run: `scripts/run-full-cycle.sh stub-cycle` (since deleted), exit 0, 108 s. The driver binaries were built
  from the driver sources of `5d9e679`. `git diff --stat 5d9e679 1b81110 -- driver/src` is not empty:
  it lists five files (`sdc-conform-adapter.rs`, `sdc-conform.rs`, `sdc-reply/implement.rs`,
  `sdc-unstick.rs`, `instructions.rs`), the move of the model instructions of three binaries to
  files, which goldens hold byte for byte. The run does not show whether the cycle reaches them.
- Loom run: a binary built from Loom commit `2c0a6f48` (version 0.58.0; the merge of `7d071ad8` into
  the integration branch). Six `tree run` calls and five `tree approve` calls; the last `tree run`
  exit 0; `tree verify` exit 0 with `{"cases":8}`; 43 s for the whole script (protocol
  registration, open, runs, approvals, status, verify).
- Both sides: principal `person:owner`; no wrapper around `ess` or `aep`; no `HOME` for the programs
  under Loom.
- Summaries: written by the `runbook` binary of commit `45287aa` (the parity rules of this page).
  Both print `8 cases, 69 evidence records, 54 program runs, 5 person steps, 62 product files`.
- `runbook parity compare` and `runbook parity compare --product-digests`: exit 0 and the same
  output. The first line is `equal on what both sides record: 8 cases, 69 evidence records, 54
  program runs, 5 person steps, 62 product files`; then six lines `not recorded on one side: <member>
  (not recorded by b)`, for `source.run_settings`, `copies[].action`, `copies[].subject`,
  `copies[].subject_revision`, `decision_requests` and `protocols[].digest`.
- With Loom `64e8f5e2` the same comparison printed one more line,
  `cases[2].evidence[6].upstream_revisions.vision`: the driver records the revision of the vision on
  the evidence of `roadmap_approval`, and that Loom commit recorded none on a person step. Loom's
  commit `7d071ad8` (in `2c0a6f48`) records it, and the line is gone.
- Process names below `b10x-loom` over the six `tree run` calls, sampled every 20 ms: `bash`,
  `cargo`, `ess`, `rustc`, `sccache`, `sdc-findings`, `slot`; never `sdc-drive` and never `canon`.
  The limit: a program that lives shorter than the sampling interval can be missed, so this shows
  that no `sdc-drive` and no `canon` process lived long enough to be sampled, and no more.

What "equal on what both sides record" does not cover:

- The six members Loom's status holds nothing for, which are not compared: the run settings (so the
  stand-in settings of the driver's run), a copy's action, subject and subject revision, the
  decision requests, the digest of a protocol.
- The revision of a child's `case_outcome` record in its parent, by rule: it is neither compared nor
  counted (see "Known differences of the two runtimes").
- Directory revisions, compared by pattern (order of first appearance), not by value; the same for
  case ids, request ids and protocol digests. A protocol id is read as its registration stem.
- What `.sdc/loom-feedback/` holds: it is left out of the product tree on both sides.
- A model. This is the stub cycle: no model call on either side. It says nothing about the two
  runtimes on a run whose actions a model writes, on a retry, on a refusal, on a spent budget or on
  a person's `Deny`.

Two things the run showed that are not differences of the comparison:

- The export of a commit has no `driver/target/debug/`, and the driver's tools look for
  `sdc-findings` and `sdc-reply` there, so the driver run needed that directory linked to prebuilt
  binaries (`story:driver-tools-find-binaries-without-build-dir`).
- Three files under `.sdc/loom-feedback/<16 hex>/` although neither run shows a retry
  (`retry_followed` is 0 in both summaries). Not a defect: Loom hands retry feedback to a writer
  whose `retry_when` records exist, not only after a retry, and `sdc-node` copies it there;
  the driver hands the same three records over as well ("The rules", feedback copies).

The comparison was made against Loom `2c0a6f48` (the table below was read at `1016c251`).

## Identical comparison through `runbook loom-run` (2026-11-22)

`story:parity-run-on-loom`, last line of "Acceptance of the evaluation run", for the cycle
`stub-cycle`. The run was made by `runbook loom-run` (there is no `runbook run` yet), not by hand:
the real `b10x-loom` (commit `2c0a6f48`), `--confinement local`, a scripted decider that approves
five times with the principal `person:owner`, the stub adapter, no model call, and a spend ledger
that stayed at 0.

```
runbook parity summarize --loom-run <runs-dir>/stub-cycle --output summary-loom.json
runbook parity compare --product-digests summary-driver.json summary-loom.json
```

```
equal: 8 cases, 69 evidence records, 54 program runs, 5 person steps, 62 product files
parity: identical
```

Exit 0. No line says `not recorded on one side`: the six members the status holds nothing for are
read from the run directory, and the principal comes from the settings, so the seven values of
"From Loom's status to the summary" (the table and the list below it) are recorded on both sides.
The driver summary is the one of the first equal comparison (`parity-run-2/final`), kept as
`runbook/tests/fixtures/parity/stub-cycle-driver-summary.json` (no path, no time, no name of a
person).

The slow lane makes the same run and the same comparison: `task test:loom-real` (test
`loom_run::slow::the_stub_cycle_on_the_real_loom_compares_identical_with_the_driver_run`, ignored
unless run with `--ignored`, needs `RUNBOOK_LOOM_BIN` and `RUNBOOK_LOOM_WORK`); it took 191 s on a
loaded pool, most of it the `cargo test` of the product crate that the cycle's `tests.run` runs.

Process tree of the runner, sampled twice a second over the whole run (136 samples; a scratch script,
not committed). Distinct processes seen, by program name:

| Name | Processes | Started by |
|---|---|---|
| `runbook` | 1 | the launcher |
| `b10x-loom` | 68 | `runbook` (`protocols add`, `tree open`, `tree run`, `tree approve`, `tree status`, `tree verify`) |
| `bash` running `sdc-tool.sh` | 12 | `b10x-loom` (bindings `requirements.check`, `spec.synthesize`, `spec.trace`, `spec.gate`, `amend.upstream`, `plan.cover`, `tests.run`) |
| `bash` running `llm-node.sh` | 2 | `b10x-loom` (bindings `vision.write`, `roadmap.write`, stub adapter) |
| `sdc-findings` | 2 | `sdc-tool.sh` of `requirements.check` and of `spec.trace` |
| `ess` | 1 | `sdc-tool.sh spec-synthesize` of `spec.synthesize` |
| `slot`, `cargo`, `rustc`, `sccache` | 2, 1, 1, 1 | `sdc-tool.sh story-tests` of `tests.run` (`slot cargo test --offline` of the product crate) |
| `sleep` | 2 | `sdc-tool.sh` of `tests.run` (the pause of its pool wait, line `sleep "$pause"`) |

The names beyond `runbook` and `b10x-loom` are bound programs and what they start. Three more names
came from the launcher script of the measurement, not from the runner: its own shell, `cut` and
`sha256sum` (the digest of the binary, computed in the forked shell before it became `runbook`).
No `sdc-drive`, `run-trial.sh` or `run-finding-cycles.sh` was seen, and no `claude` or `metaharness`.
The limit is that of the sampler: a process that lives under half a second can be missed.

## Identical comparison on Loom `5af3f9c2` (2026-11-22)

`story:parity-run-on-loom`. The runner moved from `2c0a6f48` to Loom `5af3f9c2` (`b10x-loom 0.58.0`,
sha256 `074a9e86…`), which prints every decision command so that it runs as printed and records a
person's result by name (`docs/loom-run.md`, "The decisions"; `docs/loom-run-decisions.md`).

Run 1, the cycle `stub-cycle` through `runbook loom-run` on the new binary: `--confinement local`,
five scripted approvals with the principal `person:owner`, the stub adapter, no model, a ledger that
stayed at 0. `loom-run` exit 0 in 81 s. Then `runbook parity summarize --loom-run <dir> --output
summary-loom.json` and `runbook parity compare --product-digests summary-driver.json
summary-loom.json`, where `summary-driver.json` is the driver summary of the first equal comparison
(`parity-run-2/final`):

```
equal: 8 cases, 69 evidence records, 54 program runs, 5 person steps, 62 product files
parity: identical
```

Exit 0. No difference line, so none to place in Loom. The slow lane `task test:loom-real` on the new
binary (`loom_run::slow::the_stub_cycle_on_the_real_loom_compares_identical_with_the_driver_run`)
passed: 199 s of the test, 201 s of the task, on a loaded pool.

Run 2, the same cycle with the first person step rejected: the scripted answer for `context.approve`
is `approve: false` with reasons, the other answers approve. The runner ran `tree approve --action
context.approve --result rejected --reasons-file work/.sdc/reasons/001-context.approve.txt` as printed
(the file holds the decider's text and no path); the decision log records `decision: reject`. Loom sent
`context.default` back (`retry_when: {context_approval: [rejected]}`) with `LOOM_RETRY_FEEDBACK`, and
`llm-node.sh` copied the reasons to `.sdc/loom-feedback/<hash>/`. Under the stub adapter it also
records them as a trailing comment of `context.yaml`, so that the rework is a new revision: before
that change the stub rewrote the same file, the revision did not move, Loom sent the writer back
until its `max_runs` of 3 was spent and the tree ended `stuck`. Then `context.check` passed, the
person approved the new revision and the tree ended `closed: operating`, `delivered` true (70 s). The
summary reads 8 cases, 71 evidence records, 56 program runs, 6 person steps, 63 product files;
`compare` against the driver summary prints `parity: different`, as it must: the driver run has no
rejection, so the rejected person step, the extra program runs and evidence records of the rework
and the reasons file as a product file are differences. That comparison is no parity claim.

Not covered: a run with a model. The three evaluation runs of the story are the first.

## What the first real run needs

Each line is something the reader assumes about `loom.tree-status/1` that Loom's code at `1016c251`
does not settle. A first real run confirms it or the reader changes. File and line are Loom's. The
first real run (`stub-cycle`, 8 cases, 69 evidence records, 54 program runs, 5 person steps, equal
counts on both sides) settled the items marked **Settled**; the others were not exercised by it. The
equal comparison above settles the last difference the first run left on item 11 and confirms 14
again; items 3, 4, 8, 15 and 16 are still open, and each says why.

1. The link name. The brief said a case's name is joined from `children[].name` and `outcome_evidence`,
   and that an evidence entry may carry a `name` later. At `1016c251` neither exists: the name is the
   member `name` of the case entry (`status.rs:148`, `encode.rs:48`) and an evidence entry has no
   `name` (`encode.rs:71-94`). The reader reads the case's `name`. A later `name` on the evidence entry
   is not read. **Settled:** all 7 child cases carry `name` and no `link` differs.
2. Case order. `status.rs:38-39` says root first and each case before its children in the order opened;
   `tree` itself (`CanonGovernor::tree`) was not read. The reader orders by `parent` links, siblings in
   the order the status lists them, and assumes that is the order they were opened. **Settled:** all 8
   cases sit at the driver's positions (no `id`, `parent` or `link` line differs).
3. The stop of a parent. `last_stop` takes the last `stopped` line of the case's own log
   (`status.rs:177-188`); a run that waits on a child names the child (`tree_run.rs:463-475`). Whether the
   parent's log holds a `stopped` line is not settled. The driver derives the parent's stop from its own
   evaluation, so a parent over a waiting child may show a stop on one side and none on the other. **Open:** the
   equal comparison read the status after the root had its outcome, so no parent was read while it
   waited.
4. Stop words. `TreeStop` is `Terminal, AwaitingPerson, AwaitingAuthority, Stuck, BudgetSpent`
   (`ess/domains/tree.yaml:31`); the status holds the variant name (`status.rs:183`, `tree_run.rs:664`).
   `Stuck` is read as the driver's fallback `stopped` and `BudgetSpent` as `budget_spent`. The driver
   names the budget (`step_budget_spent`, `wall_time_spent`, ...); the status does not
   (`encode.rs:225-232`, the name sits in the run's `detail`), so a run stopped on a budget differs on
   `stop.cause`. `Terminal` on a case with no outcome is an error. **Open:** the equal comparison read
   a status in which every case has an outcome, so no `stop` is read and no run stopped on a budget.
5. Outcomes and obligations. Outcome names (`status.rs:140-143`, examples `arrived`, `passed` at
   `tree_run.rs:650,657`) and obligation ids (`status.rs:118-123`) are the protocol's and Canon's; the
   reader assumes the driver's `termination` and obligation ids are the same strings for the same
   protocol. **Settled:** outcomes and open obligations of all 8 cases are equal.
6. Evidence order and the opening record. The records are in the order received (`status.rs:190-191`).
   A child's `case_outcome` record has no `action` (`tree_run.rs:691-692`); the reader gives it the
   action of the next record that has one, taken to be the step that opened the child, as the driver
   does. A `case_outcome` record with no such record after it is an error. **Settled:** 7 `case_outcome`
   records came without `action`, each found one after it, and every `evidence[].action` is equal.
7. Run order and `exit`. Runs are "in the order the store keeps their observations" (`status.rs:237`);
   taken as chronological. `exit` is left out when the payload holds no integer (`status.rs:272`,
   `encode.rs:99`), for example a program a signal ended: the reader refuses that status. **Settled
   for runs that exit:** 54 runs in the driver's order, 13 with a non-zero integer `exit`, no line
   differs.
8. Retries. `retried` (`status.rs:256-269`) is read as the driver's `retry_followed`: the next run of the
   action carries a non-empty `retry_of`. Loom's `attempt` counts within a retry chain, so a rerun with
   no `retry_of` starts at 1; the summary counts the runs of the action instead and does not read it. **Open:**
   `retry_followed` is 0 in all 54 program runs of both summaries; no retry occurred.
9. The instant of an approval. `at` is the line's time (`status.rs:322`); every test passes one fixed
   instant with `--at` (`tree_run.rs:36`). The reader orders person steps by `at` as text, then by the
   case's place and the approval's. When a run passes one instant, `order` is by case, not by time, and
   may differ from the driver's. **Settled:** the run passed a distinct instant per approval (five,
   two minutes apart) and `order` equals the driver's; a run with one instant is still unchecked.
10. The answer of an approval with an action. `action` is set only for an allowed approval followed at
    once by the finish of a person action (`status.rs:308-316`). The reader takes the result of the
    n-th record with that action in the case (the label is `status.rs:212-216`) as the driver's word
    (`approved`), which assumes one record per finish. No such record is an error. A denial never has an
    action, and `Deny` is read as the driver's `denied` (`driver/src/main.rs:1885`). **Settled for
    `Allow`:** the 4 approvals with an action and the grant without one give the driver's `approved` and
    `granted`; no `Deny` occurred.
11. The actor. `actor` is the principal passed on the command line (`status.rs:321`); it equals the
    driver's only when the Loom run was given the principal the driver run recorded (`person:stand-in`).
    **Settled:** the run passed `person:owner` and `operator`, and the 5 actors differ for that reason
    alone. **Settled again:** in the equal comparison both runs passed `person:owner` and the 5
    actors are equal.
    **Settled for `runbook loom-run`:** the principal comes from `decider.principal` of the settings
    for a scripted decider; the five actors of the identical comparison are `person:owner`.
12. Copies. An entry holds `source_case`, `target_case` and `record_kind` (`encode.rs:120-126`). The
    record is not named, so `action`, `subject` and `subject_revision` are not recorded. An entry whose
    kind cannot be found is left out by Loom (`status.rs:343-351`) and shows up as a missing copy. **Settled:** the 4 copies are equal in `kind`, `from` and `to`, and
    none went missing.
    **Settled for `runbook loom-run`:** the three members are read from the copied record in the target
    case's evidence (`copies[].action`, `.subject`, `.subject_revision`) and equal the driver's.
13. Protocols. `protocol_id` and `protocol_revision` come from splitting `protocol` at the last `@`
    (`status.rs:130-133`); a protocol with no `@` has an empty revision and the reader refuses it. The
    status's own `protocols[]` is always empty (`status.rs:165`) and is not read, so no digest. **Settled:** `sdc-intake@9` splits into `sdc-intake` and 9; the revisions
    are equal, and the id is the registration stem (rule "registration stem" above).
    **Settled for `runbook loom-run`:** the digest is the sha256 of the protocol file the runner copied
    into `protocols/`, compared by pattern.
14. The product tree. `workspace` is `manifest.workspace` (`status.rs:162`). The test fixture of Loom
    copies the bindings files and the stubs into its workspace (`tree_run.rs:66-79`); if a real
    workspace holds files the driver's `work/` does not, `product_tree[]` differs by path. Pass the
    directory that holds the product alone with `--product-tree`. **Settled:** the workspace held the
    product and `.sdc/loom-feedback/` (3 files) and nothing else; the rule "feedback copies" leaves that
    directory out. **Settled again:** the equal comparison holds 62 product files, equal in path and,
    with `--product-digests`, in sha256; the workspace again held 3 files under `.sdc/loom-feedback/`
    without a retry (feedback is handed over without one, "The rules").
15. A damaged log. A run log that does not verify reads as empty (`status.rs:171-174`): the stop,
    the approvals and the actions are then missing without a word. Run `tree verify` before reading. **Open:** the equal comparison ran `tree verify` before it read the
    status and it exited 0 with `{"cases":8}`; no damaged log was read.
16. Decisions. The status holds no decision that was recorded; `stop_next` names the open ones only
    (`status.rs:129,151`). The driver's `decision_recorded` events have nothing to be compared to. **Open:** the driver's summary of `stub-cycle` holds no decision request
    (`decision_requests` is empty), so the member was not exercised; `compare` names it as not
    recorded by Loom.
    **Settled for `runbook loom-run`:** `decisions.jsonl` of the run directory is read; the runner
    answers approvals only, so the list is empty on both sides; a row of type `question` or `finding`
    (an answer of the decider to a request of the answers file) is read and compared to nothing, and a
    row of another type or kind is an error.
17. The document. The reader takes the whole of what `tree status` printed as a file
    (`tree_run.rs:795-807`), checks `format` (`read.rs:175-191`) and needs `version`
    (`read.rs:194`); a status of another format, or none, is refused by name. **Settled:** the status of
    the run (version 0.58.0) is read whole.
