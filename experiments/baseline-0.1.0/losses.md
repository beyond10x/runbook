# baseline-0.1.0: where the nine runs lost money and time

Loss here means what a run spent on reruns: takes of a node after its first, and the loop an edge
opened. Counted over every run, not sampled. The runs are not in git (`runs/` is ignored), so every
figure below comes from run files that are not committed; their committed summaries are
`experiments/baseline-0.1.0/runs/*.json`.

## How it was computed

- Binary: `sdc-run-metrics` (debug build of tree `72a3eef`, release 0.1.2), one run per command:
  `sdc-run-metrics runs/trial-0.1.0-<setting>-<n> --json <file>`. It gives each node's
  `rework_takes`, `rework_cost_usd`, `rework_driver_s` and `retries_by_cause`, each case's nodes,
  and each edge's `count`, `loop_cost_usd`, `loop_driver_s`.
- Sums per setting: `jq -s` over the three metric documents of a setting, grouped by `.node` or
  `.edge` (sum of the fields above).
- Decider share: the sum of `cost_usd` and `duration_ms` over every line of `decisions.jsonl`.
  Review share: the same over the `class == "review"` lines of `llm-calls.jsonl`. Wall and cost:
  `metrics.wall_s` and `metrics.cost_usd`. Rerun seconds: the sum of the nodes' `rework_driver_s`.
- Rerun causes: every check event in `events/*.jsonl` with an `exit` other than `0`, and its log
  blob (`store/**/blobs/<log>`, up to 20 kB read), sorted into classes by hand from the text. The
  review rejections are the `sdc.rejection/1` documents of the `change.review` logs.
- Decider seconds are outside `driver_s`: `wall_s - driver_s` is the decider's seconds plus at most
  40 s in every run (for example `trial-0.1.0-mixed-1`: 1480 - 741 = 739, decider 717 s).

## Share of each run

| run | cost USD | wall s | reruns cost | reruns s (of wall) | decider cost | decider s (of wall) | review cost | review s |
|---|---|---|---|---|---|---|---|---|
| strong-1 | 5.432 | 1075 | 1.067 (19 %) | 286 (26 %) | 2.031 (37 %) | 262 (24 %) | 0.689 (12 %) | 77 (7 %) |
| strong-2 | 6.194 | 1247 | 2.077 (33 %) | 489 (39 %) | 2.082 (33 %) | 254 (20 %) | 0.746 (12 %) | 75 (6 %) |
| strong-3 | 5.503 | 1430 | 1.144 (20 %) | 473 (33 %) | 1.819 (33 %) | 227 (15 %) | 0.964 (17 %) | 142 (9 %) |
| mixed-1 | 2.497 | 1480 | 0.530 (21 %) | 167 (11 %) | 0.177 (7 %) | 717 (48 %) | 0.588 (23 %) | 74 (5 %) |
| mixed-2 | 3.883 | 2091 | 1.727 (44 %) | 543 (25 %) | 0.251 (6 %) | 952 (45 %) | 0.761 (19 %) | 78 (3 %) |
| mixed-3 | 2.549 | 1489 | 0.519 (20 %) | 227 (15 %) | 0.169 (6 %) | 693 (46 %) | 0.551 (21 %) | 65 (4 %) |
| haiku-1 | 2.153 | 3790 | 1.381 (64 %) | 1602 (42 %) | 0.244 (11 %) | 958 (25 %) | 0.077 (3 %) | 224 (5 %) |
| haiku-2 | 0.205 | 938 | 0.004 (1 %) | 18 (1 %) | 0.082 (40 %) | 383 (40 %) | 0 | 0 |
| haiku-3 | 0.283 | 1213 | 0.114 (40 %) | 422 (34 %) | 0.104 (36 %) | 436 (35 %) | 0 | 0 |

Per setting, reruns took 4.29 of 17.13 USD (strong, 25 %), 2.78 of 8.93 USD (mixed, 31 %) and
1.50 of 2.64 USD (Haiku, 57 %).

The Haiku decider is cheap and slow. With the default effort `xhigh` it thought 19,684 to 35,402
tokens on each `merge.approve` and `release.publish` of `trial-0.1.0-mixed-1` (97, 132, 173,
96 s). The Opus decider of `trial-0.1.0-strong-1` thought 29 to 640 tokens and took 11 to 29 s per
decision (`decisions.jsonl`, `tokens.output_tokens_details.thinking_tokens`, `duration_ms`).

## Reruns by node (model nodes with a cost; summed over the three runs)

| setting | node | reruns | cost USD | driver s | causes |
|---|---|---|---|---|---|
| strong | story/change.implement | 7 | 2.951 | 768 | change.review:rejected 5, arrival:tests.run:finding 2 |
| strong | story/change.review | 5 | 0.907 | 143 | arrivals after the implement reruns |
| strong | spec/spec.author | 5 | 0.381 | 107 | spec.validate:invalid 3, arrival:spec.trace:finding 2 |
| mixed | story/change.implement | 3 | 2.109 | 670 | change.review:rejected 2, merge.approve:rejected 1 |
| mixed | story/change.review | 3 | 0.502 | 52 | arrivals |
| mixed | spec/spec.author | 2 | 0.147 | 47 | spec.validate:invalid 2 |
| mixed | direction/vision.write | 5 | 0.013 | 86 | arrival:context.check:finding 5 |
| haiku | story/change.implement | 5 | 1.268 | 1130 | change.review:rejected 3, merge.approve:rejected 1, tests.run:failed 1 |
| haiku | spec/spec.author | 9 | 0.177 | 692 | spec.validate:invalid 6, arrival:spec.trace:finding 2, spec.synthesize:refused 1 |

The check nodes rerun far more often (`requirements.check`, `questions.check`, `context.check`,
`tests.run`, `spec.*`: 9 to 16 takes per setting) at no model cost and under 200 s each in total.
Most are `arrival:` takes after an upstream amendment.

## Reruns by edge (summed over the three runs)

| setting | edge | count | loop cost USD | loop driver s |
|---|---|---|---|---|
| strong | story: change.review:rejected -> change.implement | 5 | 3.271 | 781 |
| strong | story: finding@story -> intake | 11 | 1.033 | 105 |
| strong | spec: finding@spec -> intake | 4 | 0.424 | 42 |
| strong | spec: spec.validate:invalid -> spec.author | 3 | 0.111 | 66 |
| mixed | story: merge.approve:rejected -> change.implement | 1 | 1.621 | 471 |
| mixed | story: change.review:rejected -> change.implement | 2 | 1.037 | 266 |
| mixed | spec: spec.validate:invalid -> spec.author | 2 | 0.147 | 48 |
| haiku | story: change.review:rejected -> change.implement | 3 | 0.944 | 826 |
| haiku | story: tests.run:failed -> change.implement | 1 | 0.319 | 242 |
| haiku | story: merge.approve:rejected -> change.implement | 1 | 0.113 | 154 |
| haiku | spec: spec.validate:invalid -> spec.author | 6 | 0.090 | 359 |
| haiku | spec: finding@spec -> intake | 19 | 0.072 | 253 |

The edge loops include the decider and overlap the node rerun costs: the two tables are not to
be added.

## The largest single items, per setting (run, case, node: reruns, USD, driver s, trigger)

- strong: strong-2 todo.tasks implement 2, 0.982, 216 (review: offline not wired); strong-1
  todo.sync implement 2, 0.779, 199 (review: a failed send recorded twice); strong-2 todo.sync
  implement 1, 0.595, 173 (review: no offline page); strong-3 todo.sync implement 2, 0.594, 179
  (review: `Cache-Control: no-store` and no service worker; remove and edit order); then the
  review and `spec.author` reruns of the same cases (0.118 to 0.391 USD each).
- mixed: mixed-2 list.items implement 1, 1.353, 451 (decider's merge rejection: data on a local
  file instead of the sync service of r5, nothing offline, after the review had passed it);
  mixed-1 todo.access implement 1, 0.431, 138 (review: the link only reads); mixed-3 todo.list
  implement 1, 0.325, 80 (review: no offline support); mixed-2 list.spec `spec.author` 2, 0.147,
  47 (YAML colon, then format version); the rest under 0.03 USD.
- haiku: haiku-1 list.shared implement 3, 1.220, 976 (review: Postgres of q9 against r1; tests.run
  failed with three E0308; review: EditTitle sets a field the spec does not); haiku-3 todo.spec
  `spec.author` 5, 0.109, 396; haiku-1 list.spec `spec.author` 4, 0.068, 296; the rest under
  0.04 USD.

## What triggered the reruns

- Review rejections, 10 in 9 runs (`sdc.rejection/1`): an offline requirement not built in the
  story that covers it 4 (strong-2 twice, strong-3, mixed-3); sync logic wrong 2 (strong-1,
  strong-3); the story's scope and what it built disagree 2 (mixed-1, haiku-1 list.offline); the
  Postgres default q9 1 (haiku-1); the build diverges from the spec 1 (haiku-1).
- `spec.validate` refusals, 12: the format version 5 (haiku-1 twice, haiku-3 twice, mixed-2), each
  with a hint naming the fix; an unquoted `: ` in a YAML `summary:` 3 (strong-1, strong-2,
  mixed-2), which the message does not name; content 4 (unreachable branch, unconditional
  outcomes, a payload field with no source, a field read as a literal).
- Content refusals in all: 92 (`spec.validate` 12, `spec.synthesize` 2, `spec.trace` 15,
  `tests.run` 27, `context.check` findings 14, `requirements.check` findings 12, `change.review`
  10). Of these 5 named an exact fix a program could apply, all five a format version. Another 9
  `spec.trace` refusals carried a `p-trace` finding that lists the behaviours to trace (3 of them
  nothing else). Every `tests.run`, `context.check` and `requirements.check` finding asks for a
  decision.

## The three Haiku stops, from the run files

- haiku-1, `writer_spent` in IDEA-1.list.shared. q9 "Postgres database" was not decided by the
  decider: `spec.author` raised it as finding f8 with that default, and the non-interactive rule
  accepted it (`work/questions.yaml`: `answered_by: non-interactive`, "changes no scope"). No
  line of `decisions.jsonl` names q9. The implement node was then sent back by the review (r1
  carries q9 and the build keeps a data file), by `tests.run` failed (E0308) and by the review
  again (EditTitle). The gate findings f24 and f25 (a database URL, a hand-written PostgreSQL wire
  protocol) followed from q9.
- haiku-2, `decision_not_takeable` in IDEA-1.list.add: the program check refused the decider's
  amendment for f11, which made `context.deployment` and `context.platform` contradict r6
  (`run-stopped.json`). Route R3 of 0.1.1 puts such a refusal to the decider again.
- haiku-3, `writer_spent` in IDEA-1.todo.spec. `spec.validate` refused four times: an unreachable
  branch, two unconditional outcomes, then the format version twice. The writer applied the hint:
  `llm/014-spec.author` wrote `format: ess/18` into `spec/domains/list.yaml`, and
  `spec/system.yaml` stayed at `format: ess/1`. ESS reads `format:` only on the source that
  declares the system, and this hint does not say so; the hints of haiku-1 and mixed-2 ("write
  `format: ess/22` on the source that declares the system") do.

The refusal (`trial-0.1.0-haiku-3`, case IDEA-1.todo.spec, evidence `e0204-spec.validate`, subject
revision `t229de63a49142d23`, of the output of `llm/013-spec.author`), as the validator printed it:

```text
spec was refused:
  - [unsupported_format_version] command.todo.list.ChangeItem.outcomes: subject-state outcome guards require specification format ess/3
  - [unsupported_format_version] command.todo.list.ChangeItem.outcomes.changed.when_subject_state: a list of held states in `when_subject_state` requires specification format ess/18 (hint: declare `format: ess/18`)
  - error[ESS-COMMAND-009]: subject-state outcome guards require specification format ess/3
  `ess-domain` refuses this as `unsupported_format_version`
  --> domains/list.yaml:74:5
  - error[ESS-COMMAND-009]: a list of held states in `when_subject_state` requires specification format ess/18
  `ess-domain` refuses this as `unsupported_format_version`
  help: declare `format: ess/18`
  --> domains/list.yaml:93:9
```

The next refusal (`e0217-spec.validate`, of `llm/014-spec.author`) adds
`[missing_declaration] domains/list.yaml.system: this source sets system-level fields but does not
declare \`system:\``, the result of `format:` written into the domain file.

## Loss each candidate of `epic:improvement-loop` addresses

| story | loss in the 9 runs |
|---|---|
| 8 effort per class (decider) | Haiku decider seconds per mixed run 717, 952, 693 (45 to 48 % of wall) |
| 4 build constraints, superseded answers | haiku-1 list.shared 1.220 USD, 976 s and its stop; q9 came through the non-interactive rule |
| 6 cycle ends at delivered | the release stage is one `release.publish` decision: strong 0.158 to 0.195 USD, about 20 s; mixed 0.029 USD, about 96 s |
| 2 implement runs the tree's check | one `tests.run:failed` in 9 runs: 0.319 USD, 242 s |
| 3 silent check output | 0: every content refusal had its own text |
| 5 third merge rejection ends | 0: no run had more than one merge rejection |
| 7 intake on Sonnet | intake model reruns: 1 (`questions.write`, 15 s); no stop in intake |
| none | review rejection loop: 5.25 USD and 1,873 s over the 9 runs (strong 3.27 USD, 19 % of its cost) |

## Confounds and limits

- Every run used the canon binary of the canon checkout at run time; no record pins it.
- The runs were started by a scheduler script, not by `sdc-experiment run`; the first scheduler
  ran two runs at once until 12:48Z.
- `trial-0.1.0-strong-1` and `trial-0.1.0-mixed-1` waited about 2 minutes for the build pool
  before their first event; wall times start at the first event.
- The metrics binary is of tree 0.1.2, the runs of tree 0.1.0; the metric definitions did not
  change between them (`docs/workflow-metrics.md`).
- Findings carry no take, so finding edges share their loop cost by count
  (`docs/workflow-metrics.md` § 9). The classes of review and gate texts were assigned by reading
  them, not by a program.
- No oracle judges the delivered app; a rejection that was right and one that was wrong count
  alike.
