# Intake: from an idea to a validated ESS specification and stories

The intake stage of the sdc protocol (`protocol/sdc.protocol.yaml`): it covers the actions
`idea.clarify`, `spec.draft`, `spec.validate` and `spec.review`, and ends where `story.open`
starts. An LLM only writes artifacts. A program decides every check, and each check's exit
status becomes one Canon evidence record (`canon-evidence/1`).

Everything under "Real run" was run on 2026-11-20 with `ess 0.96.0` and `aep 0.80.0` in
`$HOME/.cache/scratch/018bcc85-fcd9-48d9-8a9c-aee96a7d6ab4/intake` (shortened to
`$S` below). The toy idea was "shared shopping list". Lines marked *inferred* were not observed.

Not the same thing: `beyond10x/intake` is archived. It did intent routing (which protocol a
request runs under) and moved into Loom. This intake turns an idea into a specification.

## Rules

- An artifact counts only after a program has checked it. An LLM may write the idea record,
  the questions, the ESS draft, the story bodies and a review. It never writes a result.
- Each check is one command with an exit status. Exit 0 records `result: pass` (or the value
  named below), and any other exit records the failing value. The command's own output goes into
  the evidence body unchanged.
- Each evidence record's `subject_revision` is a content digest: the ESS `spec_digest` for the
  spec (printed by `ess verify diff` and stored in the suite's `provenance.spec_digest`), and
  the file's sha256 for any other artifact.
- `producer` names the tool and its release (`tool:ess@0.96.0`, `tool:aep@0.80.0`), never the
  model. That lets `independent_on: [different_principal]` tell a tool record apart from the
  author.
- `upstream_revisions` names the idea and questions revisions the spec was written from. When
  either changes, Canon invalidates the spec records (Canon `invalidation-rules`).

## Two modes: three words or a paragraph

The mode is set by the word count of `idea.text`. A program counts it, not an LLM.

- Seed mode, under 20 words (for example "shared shopping list", 3 words). The text names nouns
  but no relations, states or rules, so:
  - there is no extraction step. The LLM writes `draft0`: one entity for each noun in the text,
    with each state `Active`/`Open`, and a `# UNMAPPED:` marker wherever it had to choose.
  - clarification is required: every marker becomes one question, and the spec is drafted only
    after all of them are answered.
  - scope ceiling: one system, one domain, one epic. Any entity, command, actor or component the
    idea text does not name must come from an answered question.
- Brief mode, 20 words or more (a short paragraph):
  - the LLM first writes `extraction.yaml`: each candidate entity, relation, state and rule,
    with the exact span of `idea.text` it came from. A program checks that every span is a
    substring of the text (gap G3).
  - questions are asked only about what the spans leave open, and about two spans that
    contradict each other.
  - there is no fixed scope ceiling. Each declaration in the spec cites a span or a question.
- Both modes share one provenance rule. `ess verify diff --from draft0 --to spec` lists every
  `entity/*/added`, `command/*/added`, `actor/*/added` and `component/*/added` change id. Each
  one must be claimed by an idea span or an answered question.

## Who decides

- LLM, as author: `draft0` and its markers, `questions.yaml` (options and the recommended
  answer), `extraction.yaml`, the spec, how a refused spec is fixed, and the story bodies.
- LLM, as answerer: only questions that change no scope, and only with no operator present
  (`answered_by: non-interactive`, with an AEP `approval-record` tagged `non-interactive`).
- Human, required: every scope-changing question (`scope_changing: true`). A question changes
  scope when its answer accounts for an `entity|command|actor|component/*/added` change id in the
  diff. Example: Q1 "How is a list shared?" adds `entity/shopping.list.Membership/added` and
  `command/shopping.list.ShareList/added`. Without a human answer, the case stays at
  `questions_settled: needs_human`. Nothing auto-approves it.
- Human or another principal: `spec_review`, which is the existing `spec.approved` claim with
  `independent_on: [different_principal]`.
- Human: abandoning the idea, which is the existing `explicitly_abandoned` decision.

## Stages

### I0 Capture

- Input: the raw text exactly as given. No LLM touches it.
- Output: `idea.yaml`, an instance of `sdc.intake.Idea`, with `schema`, `id`, `text`, `words`,
  `submitted_by` and `submitted_at`. Its schema is projected from the ESS model in
  `$S/sdc-model`.
- Check: `ess generate schema validate --schemas <intake-schemas> idea.yaml`
- Evidence: `idea_shape`. Subject `idea`, result `valid|invalid`, producer `tool:ess@0.96.0`.
  The mode (seed or brief) is read from `words`. Nothing yet recomputes `words` from `text`
  (gap G4).

### I1 Extraction and draft0

- Input: `idea.yaml`.
- Output: `draft0/`, an ESS tree with `# UNMAPPED:` markers. In brief mode also
  `extraction.yaml`.
- Checks:
  - `ess specify validate --path draft0`. Exit 0 is required, and markers are allowed here.
  - `grep -rn 'UNMAPPED:' draft0` gives the marker list that I2 must cover.
  - Brief mode: a span check (gap G3).
- Evidence:
  - `draft_validation`: subject `spec`, result `valid|invalid`.
  - `open_markers`: subject `spec`, result `none|present`, with the marker count in the body.

### I2 Clarification

- Input: the markers from I1.
- Output: `questions.yaml`, an instance of `sdc.intake.Questions`. Each question has `id`,
  `marker` (`file:line`), `asks`, `options`, `recommended`, `scope_changing`, `answer` and
  `answered_by` (`human|non-interactive|unanswered`). Questions are typed data, not chat.
- Checks:
  - `ess generate schema validate --schemas <intake-schemas> questions.yaml` checks the shape.
  - Gap G2: every marker has exactly one question, and every question names a live marker.
  - Gap G1: no question has `scope_changing: true` unless `answered_by: human`. The ESS
    invariant that says this is projected only as `x-ess-invariants` and is not enforced (run R3
    below).
- Evidence:
  - `questions_shape`: subject `questions`, result `valid|invalid`.
  - `questions_settled`: subject `questions`, result `settled|open|needs_human`.

### I3 ESS draft and the hard gate

- Input: `idea.yaml`, plus `questions.yaml` with every question answered.
- Output: `spec/`, with `ess-inputs.yaml` pinning `requires: ess 0.96.0`. This is Canon's hard
  gate (Atlas ADR 0076), and each line is one check:
  - `ess specify validate --path spec --strict-requires` gives `spec_validation`
    (`valid|invalid`). This is the evidence kind `sdc.protocol.yaml` already declares.
  - `ess specify compile --path spec --out ir.json` gives `spec_compile`
    (`compiled|refused`).
  - `grep -rn 'UNMAPPED:' spec` gives `open_markers` (`none|present`). Exit 1 here means
    `none`.
  - `ess verify conform synthesize --path spec --suite-format 5 --out suite.json` gives
    `spec_synthesis` (`clean|refused`), with the refusal and scenario counts in the body.
  - `ess verify diff --from draft0 --to spec --format json` gives `scope_trace`
    (`traced|untraced`). Every added change id must be claimed by a span or a question (gap G5).
- Proposed claim: `spec.valid` should be `all` of `spec_validation=valid`,
  `spec_compile=compiled`, `open_markers=none`, `spec_synthesis=clean` and `scope_trace=traced`.
  Today it reads `spec_validation` alone.

### I4 Self-conformance of the model

- Input: `spec/` and `suite.json`.
- Check: `ess verify conform run --path spec --target interpreted --suite suite.json --report-format 2 --report-out report.json --strict`
- Evidence: `spec_self_conformance`. Subject `spec`, result `passed|failed|inconclusive`, read
  from the report's `conformance_status`.
- This checks the model against itself, and its scenarios are reachable and consistent. It is
  not implementation conformance. The AEP `executable-system-specification` must not move to
  `conforming` on it (see "AEP accepts an interpreted report" below).

### I5 Review

- Input: `spec/`, `idea.yaml` and `questions.yaml`.
- Evidence: `spec_review` (`approved|rejected`), which already exists. The producer must be a
  different principal. This is a judgement, not a program. A program only checks that a record
  exists and is independent.

### I6 Decomposition

- Input: the approved spec (`spec.done`).
- Output: in the repository's AEP store, an `executable-system-specification`, one `epic`, and
  one `story` per group of commands. Each story has `## ESS first` (the commands it covers) and
  `## Acceptance` (the scenario ids from `suite.json` that must pass).
- Checks:
  - `aep plan artifact validate` gives `plan_validation` (`valid|invalid`).
  - Gap G6: every command in `ir.json` is claimed by exactly one story, and every scenario id in
    `suite.json` is named by at least one story. This gives `decomposition_coverage`
    (`covered|uncovered`).
  - `aep plan artifact waves --kind story` gives `stories_scoped` (`scoped|unassessed`). It
    needs a typed scope on each story.
- After that, the plan critics (LLM) review the plan. Then `story.open` runs once per story.

## Evidence kinds

Every kind is `canon-evidence/1`, with `producer` set to the tool and its release, and
`observed_at` set to when the tool ran.

- `idea_shape`: subject `idea`, result `valid|invalid`. The `ess generate schema validate` exit
  status.
- `draft_validation`: subject `spec` (draft0 revision), result `valid|invalid`. Upstream: `idea`.
- `open_markers`: subject `spec`, result `none|present`. Body: `file:line` of each marker.
- `questions_shape`: subject `questions`, result `valid|invalid`. Upstream: `spec` (draft0).
- `questions_settled`: subject `questions`, result `settled|open|needs_human`.
- `spec_validation` (exists): subject `spec`, result `valid|invalid`. Upstream: `idea` and
  `questions`.
- `spec_compile`: subject `spec`, result `compiled|refused`.
- `spec_synthesis`: subject `spec`, result `clean|refused`. Body: the scenario count and the
  refusal count.
- `scope_trace`: subject `spec`, result `traced|untraced`. Body: each untraced change id.
- `spec_self_conformance`: subject `spec`, result `passed|failed|inconclusive`.
- `spec_review` (exists): subject `spec`, result `approved|rejected`. Independent on
  `different_principal`.
- `plan_validation`: subject `plan`, result `valid|invalid`. Upstream: `spec`.
- `decomposition_coverage`: subject `plan`, result `covered|uncovered`. Upstream: `spec`.
- `stories_scoped`: subject `plan`, result `scoped|unassessed`.

These need two new artifacts in `sdc.protocol.yaml`, `questions` and `plan`, beside `idea`,
`spec` and `release`.

## Real run

### R1 Capture and shape checks

`$S/idea.yaml` holds `text: shared shopping list` and `words: 3`. `$S/sdc-model` is an ESS model
of `Idea`, `Question` and `Questions`.

```console
$ ess specify validate --path sdc-model
sdc v1 — 2 file(s), valid
exit=0

$ ess generate --path sdc-model --kind schema --schema-id-base https://sdc.invalid/ --out out/sdc-schema --check
6 artifact(s), current in out/sdc-schema
exit=0

$ ess generate schema validate --schemas out/sdc-schema/schema idea.yaml questions.yaml
6 schema(s), 2 instance(s): valid
exit=0

$ ess generate schema validate --schemas out/sdc-schema/schema bad/idea-empty.yaml
[invalid_instance] bad/idea-empty.yaml/words: 0 is less than the minimum of 1
6 schema(s), 0 valid instance(s), 1 issue(s): invalid
exit=1
```

### R2 draft0 with open markers

```console
$ ess specify validate --path draft0
shopping v1 — 2 file(s), valid
exit=0

$ grep -rn UNMAPPED: draft0
draft0/domains/list.yaml:17:      # UNMAPPED: "shared" names no mechanism. Members on the list, a share link, or a
draft0/domains/list.yaml:33:      # UNMAPPED: does an item carry a quantity? Open question Q3.
draft0/domains/list.yaml:38:      # UNMAPPED: is a bought item checked off, removed, or both? Open question Q2.
exit=0

$ ess verify conform synthesize --path draft0 --out draft0.suite.json
0 scenario(s) (0 authored), 0 refusal(s), written to draft0.suite.json
exit=0
```

### R3 Clarification

`$S/questions.yaml` has three questions, Q1 to Q3. Q1 has `scope_changing: true` and
`answered_by: non-interactive`, which the rule above forbids. The schema check still passes it
(R1, exit 0), even though the model declares
`invariants: [{any: [scope_changing == false, answered_by == human]}]`. The projected schema
carries that only as an annotation:

```console
"x-ess-invariants": [
  "(scope_changing == false or answered_by == human)"
]
```

### R4 Spec, first attempt refused

```console
$ ess specify validate --path spec --strict-requires
spec was refused:
  - [missing_declaration] command.shopping.list.AddItem.outcomes.added.payload.shopping.list.ItemAdded.item_id: emitted payload field has no source; declare a mapping or {generated: true} ownership
  - [missing_declaration] command.shopping.list.CreateList.outcomes.created.payload.shopping.list.ListCreated.list_id: emitted payload field has no source; declare a mapping or {generated: true} ownership
  - [missing_declaration] command.shopping.list.ShareList.outcomes.shared.payload.shopping.list.ListShared.membership_id: emitted payload field has no source; declare a mapping or {generated: true} ownership
  ...
exit=1
```

The fix was `{generated: true}` on the three identity payload fields. The refused copy is in
`$S/spec-r1-refused`.

### R5 The hard gate on the fixed spec

```console
$ ess specify validate --path spec --strict-requires
shopping v1 — 3 file(s), valid
exit=0

$ ess specify compile --path spec --out out/ir.json
shopping v1 — 3 file(s), 16 declaration(s), compiled to out/ir.json
exit=0

$ ess verify conform synthesize --path spec --out out/suite.json
note: every declared actor may invoke `shopping.list.AddItem`, so no actor is refused it and no `shopping.list.AddItem/grant/denied` scenario is owed
... (5 notes)
10 scenario(s) (0 authored), 0 refusal(s), written to out/suite.json
exit=0

$ grep -rn UNMAPPED: spec
exit=1

$ ess verify conform synthesize --path spec --suite-format 5 --out out/suite5.json --check
10 selected scenario(s), 0 authored source(s), 0 refusal occurrence(s)
exit=0
```

- `ess specify validate --path spec --format json` reports
  `completeness.counts = {"unscenarioed": 0, "outside": 0, "unanswered": 5}`.
- Compiling twice gave the same `out/ir.json` sha256 both times:
  `a5c7a7d7e8037c93ddb21907ef86434676a085ce35fb7c845a27d52e89e358fa`.

### R6 Scope trace

```console
$ ess verify diff --from draft0 --to spec
shopping v1 → v1
  before  b31748f063c4637ca37b44f84b7f0066d2a5e578afcb1e5274de5fbb32295fb1
  after   1af9ef9d1bdd082378225324b338fed111d25debcbba195ae825fbf6b7271c1c

33 change(s): 1 widening, 0 narrowing, 32 other
compatibility: 1 breaking, 9 unknown, 23 compatible
           entity/shopping.list.Membership/added
           command/shopping.list.AddItem/added
           command/shopping.list.CreateList/added
           command/shopping.list.MarkBought/added
           command/shopping.list.ShareList/added
           actor/shopping.list.Member/added
           component/list-service/added
exit=0
```

The diff above is an excerpt. `after` equals the suite's `provenance.spec_digest`, which makes it
the spec revision.

### R7 Self-conformance

```console
$ slot ess verify conform run --path spec --target interpreted --report-out out/report.json
error: suite/8 through /65 require explicit --report-format 2 before execution
exit=1

$ slot ess verify conform run --path spec --target interpreted --suite out/suite5.json --report-format 2 --report-out out/report5.json --strict
shopping v1 against interpreted 0.96.0 — passed
  10 scenarios: 10 passed, 0 failed, 0 error, 0 unsupported
exit=0
```

- With the ordinary suite (`--suite-format 4`), the report says `conformance_status:
  inconclusive` and `coverage.knowledge: unknown`, even though all 10 scenarios passed.
- With `--suite-format 5`, it says `passed` and `complete_inventory`. Intake uses format 5.

### R8 Decomposition in a scratch AEP store (`$S/repo`)

```console
$ aep plan reverse init --protocols git+https://github.com/beyond10x/aep#af11af152de1732383a5edee0da723bb72d9acb4 --profile development.standard --planning-scope shopping
  store: git (aep.project/5), planning_scope: shopping (from --planning-scope)
exit=0

$ aep plan artifact new executable-system-specification shopping ...   # created (draft)
$ aep plan artifact new epic shared-shopping-list ...                  # created (draft)
$ aep plan artifact new story create-and-share-list --relate decomposes:epic:shared-shopping-list ...
$ aep plan artifact new story add-items ...
$ aep plan artifact new story check-off-items ...
$ aep plan artifact new approval-record q1-sharing-mechanism --tag non-interactive ...

$ aep plan artifact validate
6 file(s) in .../repo/.engineering/planning: 6 artifact(s)
valid
exit=0

$ aep plan artifact move executable-system-specification:shopping --to validated
executable-system-specification:shopping moved draft -> validated (revision 2)
exit=0

$ aep plan artifact evidence executable-system-specification:shopping --from conformance/report.json --suite conformance/suite.json
executable-system-specification:shopping: ess_conformance_coverage_v1 recorded from {...}
  this record: does not count: this specification records no `model_digest`, so no run can be shown to be of this revision — `aep plan artifact set <id> --model-digest <hex>` records it
exit=0

$ aep plan artifact set executable-system-specification:shopping --model-digest 1af9ef9d1bdd082378225324b338fed111d25debcbba195ae825fbf6b7271c1c
$ aep plan artifact move executable-system-specification:shopping --to conforming
executable-system-specification:shopping moved validated -> conforming (revision 4)
exit=0

$ aep plan artifact waves --kind story
no wave: nothing selected declares a scope; `aep artifact scope <id> --add <path>` records one
0 wave(s), 0 collision(s), 3 unassessed
exit=0
```

- The full logs are `$S/run-ess.log`, `$S/run-capture.log`, `$S/run-aep.log`,
  `$S/run-aep2.log` and `$S/gaps.log`.
- The scripts that produced them are `$S/run-*.sh`.

## Gaps

- G1, the scope-changing answer rule is not enforced. `ess generate schema validate --schemas
  out/sdc-schema/schema idea.yaml questions.yaml` exits 0 on a `scope_changing: true` question
  answered `non-interactive`. Struct invariants project only to `x-ess-invariants`. A checker is
  needed that evaluates the invariant on the instance.
- G2, open questions are not typed in ESS. `ess specify validate --path draft0` exits 0 with 3
  `UNMAPPED:` markers. `epic:typed-open-questions` in beyond10x/ess is `draft`. Until it ships,
  the `grep` scan is the check, as Canon does. No command links each marker to its question:
  `ess specify questions --path draft0` fails with
  `error: unrecognized subcommand 'questions'` (exit 2).
- G3, there is no span check for brief mode. No tool checks that an extraction cites a substring
  of `idea.text` (*inferred* to need a small Rust checker; no command exists).
- G4, there is no idea capture tool. `aep plan intake idea.yaml` fails with
  `error: unrecognized subcommand 'intake'` (exit 2). Nothing writes `idea.yaml` with a computed
  `words`, and nothing recomputes it.
- G5, there is no scope tracer. `ess verify diff --format json` lists the change ids, but nothing
  joins them to spans and questions. `ess specify provenance --path spec` fails with
  `error: unrecognized subcommand 'provenance'` (exit 2).
- G6, decomposition coverage is not checked. `aep plan artifact validate` passes stories that
  claim no command, and nothing joins `ir.json` commands or `suite.json` scenario ids to story
  bodies. `aep plan artifact waves` needs typed scopes and reported 3 unassessed.
- G7, there is no Canon CLI on this machine. `canon --help` fails with
  `command not found: canon` (exit 127). The evidence records above cannot be evaluated here.
- G8, AEP accepts an interpreted report. AEP moved the ESS artifact to `conforming` on a report
  from `--target interpreted`, which is the model checked against itself. The
  `development.standard` lifecycle requires only `ess_conformance*`, at least 1, and nothing
  about the implementation. Intake must stop at `validated`.
- G9, the interpreted run has a synthetic clock. `report5.json` has `completed_at`
  `1700000003300`, and AEP recorded it as observed 2023-11-14T22:13:23Z. A Canon `max_age` on
  `spec_self_conformance` would expire it at once. Use the wrapper's own clock for
  `observed_at`.
- G10, `validate` prints no digest. `ess specify validate` prints no spec digest, so
  `spec_validation.subject_revision` has to come from `ess verify diff` (`after`) or from the
  suite's `provenance.spec_digest`.
- G11, there is no `validated` evidence. In `development.standard`, AEP `draft -> validated`
  needs no record ("moved draft -> validated" with no evidence). `aep plan artifact explain`
  then listed the conformance record under `draft -> validated` and "no record" under
  `validated -> conforming`.
