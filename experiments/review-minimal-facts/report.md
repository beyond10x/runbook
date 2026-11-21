# review-minimal-facts: change.review on the smallest fact set against the whole change tree, by recorded replay

Written by `sdc-experiment report review-minimal-facts` from `experiments/review-minimal-facts.yaml` and the run summaries in `experiments/review-minimal-facts/runs/`; `sdc-experiment check --all` fails when it is stale. Definitions: `docs/workflow-metrics.md`.

- Kind: prospective; status: planned; created 2026-11-21.
- Purpose: Whether the review keeps its decisions when the program gives it the changed files, the story's requirements and commands and the test summary instead of the whole tree and the diff text, at a third of the input.
- Hypothesis: none; the record describes its runs.
- Input `todo-app` (idea, sha256 f384a752a2cb): A to-do list web app for me and my partner that also works on the train without a connection
- Tree: not pinned yet
- Runs per variant: 3; pass rule: clean-delivery (closed, delivered, no reopen, forced step or unstick).
- Decision rule (descriptive): Treatment: story:review-minimal-facts built on the tree of the source runs (tree_commit pinned when it is), the review's in/ cut to scope.json, the covered entries of commands.json, the changed files of impl/ with line numbers, the changed line ranges and the test summary. One replay of each of release-0.1.2.strong.1, -2 and -3 on that tree, launched by hand: scripts/run-trial.sh review-minimal-facts-<n> --autopilot --recorded runs/release-0.1.2.strong.<n> --on-divergence live --live-adapter claude-cli --live-cap <its review calls>. Every review diverges, because its inputs changed, and goes live on Sonnet; any other live call means a decision differed. Primary: the review's cache-write tokens per call, from llm-calls.jsonl, against the same calls of the source; registered direction decrease, expected to about a third (33k to 53k now). Pass, per replay: live calls = review calls, every review returns the recorded decision, a rejection names the same requirement ids, sdc-replay compare exits 0. Adopt when all 3 replays pass and the input falls by at least half; any differing decision is published with both replies.

## Settings

| setting | minimal-facts |
|---|---|
| `adapter` | metaharness |
| `autonomy` | A5 |
| `decider.adapter` | claude-cli |
| `decider.escalate` | final |
| `decider.kind` | model |
| `decider.max_decider_calls` | 40 |
| `decider.model` | claude-opus-5-5 |
| `max_llm_calls` | 150 |
| `models.implement.model` | claude-sonnet-5-5 |
| `models.other.model` | claude-sonnet-5-5 |
| `models.review.model` | claude-sonnet-5-5 |
| `models.writer.model` | claude-sonnet-5-5 |
| `review.adapter` | claude-cli |

Known confounds:

- `tree`: the change must be built on the source runs' tree 72a3eef; a replay on a later tree diverges at other calls as well.
- `same-model`: Sonnet in both, so a differing decision is the inputs' doing or Sonnet's own variance; three replays of one source would separate the two, at three times the cost.

## Runs

| run | variant | end | clean pass | cost USD | wall s | looping factor | note |
|---|---|---|---|---|---|---|---|

## Per variant

Median [min, max] (n) over the ended runs.

| | minimal-facts |
|---|---|
| runs listed | 0 |
| ended | 0 |
| clean passes | 0 |
| `run.cost_usd` | - |
| `run.llm_calls` | - |
| `run.decider_calls` | - |
| `run.wall_s` | - |
| `run.driver_s` | - |
| `run.real_takes` | - |
| `run.rework_takes` | - |
| `run.looping_factor` | - |
| `run.first_pass_yield` | - |
| `run.rework_cost_usd` | - |
| `run.furthest_stage_index` | - |
| `run.findings_raised` | - |
| `run.waiting_on_person_s` | - |
| `run.clean` | - |

## Loops

Per variant, over the ended runs: of the writer and approval nodes taken again at least once, the three with the highest median looping factor; and the three edges with the highest loop cost.

- minimal-facts (n 0):
  - no node taken again, no edge

## Verdict

descriptive: no hypothesis: the record describes its runs and decides nothing.

## Facts

- experiments/baseline-0.1.0/inputs.md: the review is one request with no tools; in/impl/ is the whole tree (24 KB at the first story of trial-0.1.0-strong-1, 84 KB at its fourth) plus change.diff (24 to 26 KB); 0.69 USD of a strong run's 0.80 review USD is input.
- Expected saving per run: strong 0.46, mixed 0.36 USD (inferred, input to a third).

## Links

- experiments/baseline-0.1.0/inputs.md: the fact sets and the ranking
- experiments/review-haiku-replay.yaml: the review class on Haiku, the same replay method
