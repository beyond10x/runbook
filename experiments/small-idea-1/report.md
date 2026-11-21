# small-idea-1: A small idea (a word-count tool) on acd6f12, all Haiku against Sonnet work

Written by `sdc-experiment report small-idea-1` from `experiments/small-idea-1.yaml` and the run summaries in `experiments/small-idea-1/runs/`; `sdc-experiment check --all` fails when it is stale. Definitions: `docs/workflow-metrics.md`.

- Kind: retrospective; status: complete; created 2026-11-21.
- Purpose: Run a deliberately small idea from idea to close with no person, to see whether the loops of the to-do runs come from the idea's size.
- Hypothesis: none; the record describes its runs.
- Input `wordcount` (idea, sha256 0492efa4896a): A command-line tool that reads a text file and prints how many lines, words and characters it has. One command, one argument (the file path). A missing or unreadable file prints an error and exits with a non-zero code. No network, no storage, no user accounts, standard library only.
- Tree: `acd6f12c4924e7a552d0ab518287d1c873618915`
  - trial/3b at acd6f12, worktree sdc-trial-3b, the batch tree; started beside the batch
- Runs per variant: 1; pass rule: clean-delivery (closed, delivered, no reopen, forced step or unstick).
- Decision rule (descriptive): No hypothesis. One run per setting; the record describes the runs.

## Settings

| setting | haiku | sonnet |
|---|---|---|
| `adapter` | metaharness | metaharness |
| `autonomy` | A5 | A5 |
| `decider.adapter` | claude-cli | claude-cli |
| `decider.escalate` | final | final |
| `decider.kind` | haiku | haiku |
| `decider.max_decider_calls` | 20 | 20 |
| `decider.model` | claude-haiku-5-5 | claude-haiku-5-5 |
| `max_llm_calls` | 150 | 150 |
| `max_wall_seconds` | 5400 | 5400 |
| `models.implement.model` | claude-haiku-5-5 | claude-sonnet-5-5 |
| `models.other.model` | claude-haiku-5-5 | claude-sonnet-5-5 |
| `models.review.model` | null | claude-sonnet-5-5 |
| `models.writer.model` | claude-haiku-5-5 | claude-sonnet-5-5 |

## Runs

| run | variant | end | clean pass | cost USD | wall s | looping factor | note |
|---|---|---|---|---|---|---|---|
| trial-3-small-haiku-1 | haiku | stopped: writer_spent in IDEA-1.counter.count | no | 0.366 | 1349 | 1.8 |  |
| trial-3-small-sonnet-1 | sonnet | stopped: decider_budget_spent in IDEA-1.count.spec | no | 1.113 | 1092 | 2 |  |

## Per variant

Median [min, max] (n) over the ended runs.

| | haiku | sonnet |
|---|---|---|
| runs listed | 1 | 1 |
| ended | 1 | 1 |
| clean passes | 0 | 0 |
| `run.cost_usd` | 0.3655 [0.3655, 0.3655] (1) | 1.1125 [1.1125, 1.1125] (1) |
| `run.llm_calls` | 18 [18, 18] (1) | 16 [16, 16] (1) |
| `run.decider_calls` | 12 [12, 12] (1) | 20 [20, 20] (1) |
| `run.wall_s` | 1349 [1349, 1349] (1) | 1092 [1092, 1092] (1) |
| `run.driver_s` | 662 [662, 662] (1) | 221 [221, 221] (1) |
| `run.real_takes` | 122 [122, 122] (1) | 122 [122, 122] (1) |
| `run.rework_takes` | 72 [72, 72] (1) | 88 [88, 88] (1) |
| `run.looping_factor` | 1.8 [1.8, 1.8] (1) | 2 [2, 2] (1) |
| `run.first_pass_yield` | 0.6 [0.6, 0.6] (1) | 0.75 [0.75, 0.75] (1) |
| `run.rework_cost_usd` | 0.1113 [0.1113, 0.1113] (1) | 0.4384 [0.4384, 0.4384] (1) |
| `run.furthest_stage_index` | 6 [6, 6] (1) | 4 [4, 4] (1) |
| `run.findings_raised` | 13 [13, 13] (1) | 4 [4, 4] (1) |
| `run.waiting_on_person_s` | 0 [0, 0] (1) | 0 [0, 0] (1) |
| `run.clean` | 0 [0, 0] (1) | 0 [0, 0] (1) |

## Loops

Per variant, over the ended runs: of the writer and approval nodes taken again at least once, the three with the highest median looping factor; and the three edges with the highest loop cost.

- haiku (n 1):
  - `story/change.implement` (llm-writer): looping factor median 5 (max 5, n 1), rework takes 4, rework cost 0.078 USD
  - `spec/spec.author` (llm-writer): looping factor median 3 (max 3, n 1), rework takes 2, rework cost 0.027 USD
  - `direction/roadmap.write` (llm-writer): looping factor median 2 (max 2, n 1), rework takes 1, rework cost 0.003 USD
  - `story: merge.approve:rejected -> change.implement` (approval): count 2 in 1 runs, loop cost 0.117 USD, loop driver 155 s
  - `story: finding@story -> intake` (finding): count 3 in 1 runs, loop cost 0.082 USD, loop driver 122 s
  - `direction: finding@context -> intake` (finding): count 1 in 1 runs, loop cost 0.026 USD, loop driver 16 s
- sonnet (n 1):
  - `spec/spec.author` (llm-writer): looping factor median 8 (max 8, n 1), rework takes 7, rework cost 0.395 USD
  - `intake/questions.write` (llm-writer): looping factor median 2 (max 2, n 1), rework takes 1, rework cost 0.044 USD
  - `spec: finding@spec -> intake` (finding): count 2 in 1 runs, loop cost 0.417 USD, loop driver 91 s
  - `intake: questions.check:invalid -> questions.write` (check): count 1 in 1 runs, loop cost 0.044 USD, loop driver 11 s

## Verdict

descriptive: no hypothesis: the record describes its runs and decides nothing.

## Facts

- trial-3-small-haiku-1 stopped writer_spent in IDEA-1.counter.count (change.implement 3 runs): the decider rejected merge.approve twice because the change was Rust while r5 says Python 3; the decided questions say Rust; the second rework added a Python package beside the Rust crate.
- trial-3-small-sonnet-1 stopped decider_budget_spent in IDEA-1.count.spec: 15 of its 20 decider calls were the trace gaps p-trace-r5 (8) and p-trace-r7 (7), the trial-3-sonnet-1 pattern (fix: fix/superseded-answers b4aa0d1, not merged).
- A small idea did not remove the loops: both runs stopped, one on a requirement conflict, one on a repeated finding.

## Links

- docs/trial-3.md on branch docs/trial-3 (commit 243a22f; not on main when this record was written): section 6, Small idea
