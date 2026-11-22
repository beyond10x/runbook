# Parity of a run under the old driver and under Loom

`story:parity-run-on-loom`. The comparison asks whether a case tree reaches the same facts under the old
driver (`sdc-drive`) and under Loom. When it does, the driver's runtime copy leaves Runbook. This page
holds the summary, the rules, the commands, the table of what each Loom member becomes, and what the
first real run under Loom still has to confirm.

## The summary

`runbook.parity-summary/1`, JSON, stable key order, no absolute path and no time. Members:

- `format`, `source` (the runtime and its run settings), `not_recorded` (member paths the runtime
  records nothing for; for the driver: the question and the options of a decision request; for Loom:
  the rows of the table below marked `(not recorded)`). A member listed there holds null or an empty
  list, and that is no fact: `compare` does not compare it.
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
- Feedback copies (`product_tree[.sdc/loom-feedback/**]`). In Loom mode `bindings/tools/llm-node.sh`
  copies the retry-feedback document and the log tails of a retry into `.sdc/loom-feedback/<16 hex>/` in
  the workspace; the driver hands the same feedback to the writer in a variable and writes no file. The
  listing of the product tree leaves out everything under `.sdc/loom-feedback/`, on both sides, where
  the summary is written (`walk_product`); it is the transport of feedback, not product. Another
  directory under `.sdc/` is listed as before.

A third answer besides equal and different: a member exactly one side lists under `not_recorded` (or
lies below a member it lists) is not compared, and `compare` names it on a line
`not recorded on one side: <member> (not recorded by a|b)`. When nothing differs the first line then
reads `equal on what both sides record:` instead of `equal:`, and the exit is 0. A member both sides
list is skipped and not named. The driver lists `decision_requests[].question` and `.options`, Loom
lists `decision_requests`: the driver's two are below Loom's, so only Loom's is named.

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

```
RUNS=<dir>/runs scripts/run-full-cycle.sh stub-cycle <dir>/store-stub-cycle
runbook parity summarize --driver <dir>/runs/stub-cycle --store <dir>/store-stub-cycle --output a.json
b10x-loom tree status --store <loom store> > status.json
runbook parity summarize --loom-status status.json --product-tree <product dir> --output b.json
runbook parity compare --product-digests a.json b.json
runbook parity approvals a.json
```

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

Read at Loom commit `1016c251` (`crates/loom-tree/src/status.rs`, `encode.rs`, `read.rs`). The table
is `RULES` in `runbook/src/parity.rs` printed by `runbook parity compare --rules`: the columns
`Loom member` and `Rule` are its last two, and a test fails when this page and the code differ. The
`Driver` column is where the driver reader finds the member. `[]` is any list index. A Loom member
named `cases[].x` is a member `x` of one entry of `cases[]`. A rule that names words (`Allow = granted`)
is the one place the two vocabularies meet: another word is an error, not a default.

Not one-to-one: `cases[].stop` (Loom keeps the last stop even after the case went on, and has one
`BudgetSpent` where the driver names the budget), `cases[].evidence[].action` (a child's `case_outcome`
record has none in Loom), `program_runs[].attempt` (counted, not read), `person_steps[].order` and
`.answer` (by the instant recorded; the answer of an approval with an action is the record's result),
`copies[]` (three members not recorded), `protocols[].digest` and `decision_requests` (not recorded).

| Summary member | Driver | Loom member | Rule |
|---|---|---|---|
| `format` | constant | format | must be loom.tree-status/1, else refused by name; the summary's format is its own |
| `source` | `run-settings.jsonl`, first line | format, version | runtime loom, status_version the version; no path of the store is copied |
| `source.run_settings` | `run-settings.jsonl`, first line | (not recorded) | tree status holds no run settings |
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
| `cases[].person_steps[].actor` | `action_finished` with a principal, `authority_recorded`, `ir.json` | cases[].approvals[].actor | as is; equal only when the Loom run passed the principal the driver run used |
| `copies[].action` | `action_started` with `copy`, then `action_finished` | (not recorded) | a copy entry holds source_case, target_case and record_kind only |
| `copies[].kind` | `action_started` with `copy`, then `action_finished` | cases[].copies[].record_kind | as is |
| `copies[].from_protocol` | `action_started` with `copy`, then `action_finished` | cases[].copies[].source_case | protocol_id of that case; compared by registration stem |
| `copies[].from` | `action_started` with `copy`, then `action_finished` | cases[].copies[].source_case | as is |
| `copies[].to` | `action_started` with `copy`, then `action_finished` | cases[].copies[].target_case | as is |
| `copies[].subject` | `action_started` with `copy`, then `action_finished` | (not recorded) | the entry names no record |
| `copies[].subject_revision` | `action_started` with `copy`, then `action_finished` | (not recorded) | the entry names no record |
| `decision_requests` | `decision_recorded` events | (not recorded) | tree status holds no decision record (stop_next names the open ones only) |
| `decision_requests[].id` | `decision_recorded` events | (under a member not recorded) | an empty list |
| `decision_requests[].case` | `decision_recorded` events | (under a member not recorded) | an empty list |
| `decision_requests[].question` | `decision_recorded` events | (under a member not recorded) | an empty list |
| `decision_requests[].options[]` | `decision_recorded` events | (under a member not recorded) | an empty list |
| `decision_requests[].answer` | `decision_recorded` events | (under a member not recorded) | an empty list |
| `decision_requests[].who` | `decision_recorded` events | (under a member not recorded) | an empty list |
| `protocols[].id` | `opened` event | cases[].protocol_id | the distinct protocols of the cases, by id then revision; compared by registration stem |
| `protocols[].revision` | `opened` event | cases[].protocol_revision | as for the case |
| `protocols[].digest` | `opened` event | (not recorded) | the status's protocols[] is empty and is not read |
| `product_tree[].path` | the root case's `work/` | --product-tree, else workspace | the directory walked as the driver's work/ is |
| `product_tree[.sdc/loom-feedback/**]` | the root case's `work/` | --product-tree, else workspace | left out of the listing of both sides when the summary is written |
| `product_tree[].sha256` | the root case's `work/` | --product-tree, else workspace | the same digest of the same file |

Not recorded, and what compare says: `source.run_settings`, `copies[].action`, `copies[].subject`,
`copies[].subject_revision`, `decision_requests`, `protocols[].digest`. `compare` of a driver summary and
a Loom summary names each as `not recorded on one side` and compares nothing there. Two of them do real
work on the driver side and are lost on Loom's: a copy's `subject_revision` (the evidence lists still
carry the revision of the copied record in the target case) and the stand-in settings of the run.

## What the first real run needs

Each line is something the reader assumes about `loom.tree-status/1` that Loom's code at `1016c251`
does not settle. A first real run confirms it or the reader changes. File and line are Loom's. The
first real run (`stub-cycle`, 8 cases, 69 evidence records, 54 program runs, 5 person steps, equal
counts on both sides) settled the items marked **Settled**; the others were not exercised by it.

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
   evaluation, so a parent over a waiting child may show a stop on one side and none on the other.
4. Stop words. `TreeStop` is `Terminal, AwaitingPerson, AwaitingAuthority, Stuck, BudgetSpent`
   (`ess/domains/tree.yaml:31`); the status holds the variant name (`status.rs:183`, `tree_run.rs:664`).
   `Stuck` is read as the driver's fallback `stopped` and `BudgetSpent` as `budget_spent`. The driver
   names the budget (`step_budget_spent`, `wall_time_spent`, ...); the status does not
   (`encode.rs:225-232`, the name sits in the run's `detail`), so a run stopped on a budget differs on
   `stop.cause`. `Terminal` on a case with no outcome is an error.
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
   no `retry_of` starts at 1; the summary counts the runs of the action instead and does not read it.
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
    alone.
12. Copies. An entry holds `source_case`, `target_case` and `record_kind` (`encode.rs:120-126`). The
    record is not named, so `action`, `subject` and `subject_revision` are not recorded. An entry whose
    kind cannot be found is left out by Loom (`status.rs:343-351`) and shows up as a missing copy. **Settled:** the 4 copies are equal in `kind`, `from` and `to`, and
    none went missing.
13. Protocols. `protocol_id` and `protocol_revision` come from splitting `protocol` at the last `@`
    (`status.rs:130-133`); a protocol with no `@` has an empty revision and the reader refuses it. The
    status's own `protocols[]` is always empty (`status.rs:165`) and is not read, so no digest. **Settled:** `sdc-intake@9` splits into `sdc-intake` and 9; the revisions
    are equal, and the id is the registration stem (rule "registration stem" above).
14. The product tree. `workspace` is `manifest.workspace` (`status.rs:162`). The test fixture of Loom
    copies the bindings files and the stubs into its workspace (`tree_run.rs:66-79`); if a real
    workspace holds files the driver's `work/` does not, `product_tree[]` differs by path. Pass the
    directory that holds the product alone with `--product-tree`. **Settled:** the workspace held the
    product and `.sdc/loom-feedback/` (3 files) and nothing else; the rule "feedback copies" leaves that
    directory out.
15. A damaged log. A run log that does not verify reads as empty (`status.rs:171-174`): the stop,
    the approvals and the actions are then missing without a word. Run `tree verify` before reading.
16. Decisions. The status holds no decision that was recorded; `stop_next` names the open ones only
    (`status.rs:129,151`). The driver's `decision_recorded` events have nothing to be compared to.
17. The document. The reader takes the whole of what `tree status` printed as a file
    (`tree_run.rs:795-807`), checks `format` (`read.rs:175-191`) and needs `version`
    (`read.rs:194`); a status of another format, or none, is refused by name. **Settled:** the status of
    the run (version 0.58.0) is read whole.
