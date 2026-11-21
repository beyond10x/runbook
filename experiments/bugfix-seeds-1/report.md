# bugfix-seeds-1: The bug-fix protocol on four seeds, all Haiku against all Sonnet

Written by `sdc-experiment report bugfix-seeds-1` from `experiments/bugfix-seeds-1.yaml` and the run summaries in `experiments/bugfix-seeds-1/runs/`; `sdc-experiment check --all` fails when it is stale. Definitions: `docs/workflow-metrics.md`.

- Kind: retrospective; status: complete; created 2026-11-21.
- Purpose: Run sdc.bugfix (red first, then the fix) unattended on four seeded bugs, once per seed on each model, to see whether each model writes a red test and a fix that passes the gate.
- Hypothesis: none; the record describes its runs.
- Input `pager` (seed `driver/examples/sdc/fixtures/bugfix-pager`, sha256 9bb5b5f4519e): With 11 items and 5 items per page, pager::page_count(11, 5) returns 2 and pager::page(&items, 3, 5) returns an empty slice, so item 11 is never shown. Expected: 3 pages, the third holding item 11.
- Input `kvparse` (seed `driver/examples/sdc/fixtures/bugfix-kvparse`, sha256 50eb0c43a189): kvparse::parse_line("timeout") returns Ok(("timeout", "")) and parse_all("timeout\n") succeeds, so a config line that lost its value is silently taken as an empty value. Expected: a line without = is refused with Err(ParseError::MissingSeparator), as the documentation says.
- Input `percent` (seed `driver/examples/sdc/fixtures/bugfix-percent`, sha256 0f9f14dd7642): percent::is_valid(100) returns false and percent::bar(100, 10) returns None, so a finished task cannot be shown. Expected: 100 is a valid percentage (0 to 100, both included), and bar(100, 10) returns Some("##########").
- Input `tally` (seed `driver/examples/sdc/fixtures/bugfix-tally`, sha256 1faf3ae5bb3b): After t.add("a", 2), t.add("b", 3) and t.remove("a"), tally::Tally::total() returns 5. Expected 3: the total is the sum of the counts that are still there, so remove must take its count out of the total.
- Tree: `several`
  - the work tree of impl/bugfix-cycle (worktree sdc-impl-bugfix-cycle), uncommitted while the runs ran, on dfe4a3f; committed after them as d2e4896 and 99238c0 (seed runs written up in e108d5e). No run pins a revision.
  - bugfix-tally-sonnet-2 ran after the close by rule:autopilot was merged; it is the only run that names rule:autopilot
- Runs per variant: 4; pass rule: clean-delivery (closed, delivered, no reopen, forced step or unstick).
- Decision rule (descriptive): No hypothesis. One run per seed and model (two for tally on Sonnet); the record describes the runs.

## Settings

| setting | haiku | sonnet |
|---|---|---|
| `adapter` | metaharness | metaharness |
| `autonomy` | A5 | A5 |
| `decider.escalate` | final | final |
| `decider.kind` | haiku | haiku |
| `decider.max_decider_calls` | 20 | 20 |
| `decider.model` | claude-haiku-5-5 | claude-sonnet-5-5 |
| `max_llm_calls` | 15 | 15 |
| `max_wall_seconds` | 5400 | 5400 |
| `models.implement.model` | claude-haiku-5-5 | claude-sonnet-5-5 |
| `models.other.model` | claude-haiku-5-5 | claude-sonnet-5-5 |
| `models.review.model` | claude-haiku-5-5 | claude-sonnet-5-5 |
| `models.writer.model` | claude-haiku-5-5 | claude-sonnet-5-5 |

Known confounds:

- `tree`: the protocol and sdc-bugfix changed between runs in the uncommitted work tree; bugfix-tally-sonnet-2 ran after the status fix and the close by rule

## Runs

| run | variant | input | end | clean pass | cost USD | wall s | looping factor | note |
|---|---|---|---|---|---|---|---|---|
| bugfix-pager-haiku | haiku | pager | closed: fixed | yes | 0.013 | 83 | 1.333 |  |
| bugfix-kvparse-haiku | haiku | kvparse | closed: fixed | yes | 0.012 | 97 | 1.333 |  |
| bugfix-percent-haiku | haiku | percent | closed: fixed | yes | 0.023 | 185 | 1.667 |  |
| bugfix-tally-haiku | haiku | tally | closed: fixed | yes | 0.009 | 44 | 1 |  |
| bugfix-1 | sonnet | pager | closed: fixed | yes | 0.167 | 43 | 1 | the first seed run |
| bugfix-kvparse-sonnet | sonnet | kvparse | closed: fixed | yes | 0.163 | 74 | 1 |  |
| bugfix-percent-sonnet | sonnet | percent | closed: fixed | yes | 0.126 | 51 | 1 |  |
| bugfix-tally-sonnet | sonnet | tally | closed: not_fixed | no | 0.269 | 111 | 1.667 | not_fixed on the sdc-bugfix status defect |
| bugfix-tally-sonnet-2 | sonnet | tally | closed: fixed | yes | 0.127 | 142 | 1 | the same seed and model after the fix |

## Per variant

Median [min, max] (n) over the ended runs.

| | haiku | sonnet |
|---|---|---|
| runs listed | 4 | 5 |
| ended | 4 | 5 |
| clean passes | 4 | 4 |
| `run.cost_usd` | 0.0128 [0.0085, 0.0225] (4) | 0.1627 [0.1259, 0.2689] (5) |
| `run.llm_calls` | 4 [3, 5] (4) | 3 [3, 5] (5) |
| `run.decider_calls` | - | - |
| `run.wall_s` | 90 [44, 185] (4) | 74 [43, 142] (5) |
| `run.driver_s` | 90 [43, 183] (4) | 74 [44, 140] (5) |
| `run.real_takes` | 12.5 [10, 14] (4) | 10 [10, 10] (5) |
| `run.rework_takes` | 4.5 [2, 6] (4) | 2 [2, 4] (5) |
| `run.looping_factor` | 1.333 [1, 1.667] (4) | 1 [1, 1.667] (5) |
| `run.first_pass_yield` | 0.667 [0.667, 1] (4) | 1 [0.667, 1] (5) |
| `run.rework_cost_usd` | 0.004 [0, 0.0119] (4) | 0 [0, 0.1518] (5) |
| `run.furthest_stage_index` | - | - |
| `run.findings_raised` | 0 [0, 0] (4) | 0 [0, 0] (5) |
| `run.waiting_on_person_s` | 0 [0, 0] (4) | 0 [0, 0] (5) |
| `run.clean` | 1 [1, 1] (4) | 1 [1, 1] (5) |

## Loops

Per variant, over the ended runs: of the writer and approval nodes taken again at least once, the three with the highest median looping factor; and the three edges with the highest loop cost.

- haiku (n 4):
  - `bugfix/change.fix` (llm-writer): looping factor median 1.5 (max 3, n 4), rework takes 3, rework cost 0.016 USD
  - `bugfix/test.reproduce` (llm-writer): looping factor median 1 (max 2, n 4), rework takes 1, rework cost 0.004 USD
  - `bugfix: gate.run:failed -> change.fix` (check): count 3 in 2 runs, loop cost 0.016 USD, loop driver 80 s
  - `bugfix: repro.run:not_red -> test.reproduce` (check): count 1 in 1 runs, loop cost 0.004 USD, loop driver 21 s
- sonnet (n 5):
  - `bugfix/change.fix` (llm-writer): looping factor median 1 (max 3, n 5), rework takes 2, rework cost 0.152 USD
  - `bugfix: tests.run:failed -> change.fix` (check): count 2 in 1 runs, loop cost 0.152 USD, loop driver 55 s

## Verdict

descriptive: no hypothesis: the record describes its runs and decides nothing.

## Facts

- Each model wrote a red test at once for every seed.
- The one not_red (kvparse-haiku) and the one stop as not_fixed (tally-sonnet) were the same defect in sdc-bugfix: it read the test's status from the full run, where through slot the test line landed under the wrong Running header. Fixed: the test now has its own run; tally-sonnet-2 is the same seed and model after the fix.
- The two gate failures were real: a Haiku fix was not formatted (percent-haiku, fmt) or had a clippy finding (pager-haiku). The gate record sent change.fix back, and the next fix passed.
- Seed runs per docs/bugfix.md: percent-sonnet 3 calls 0.126 USD; percent-haiku 5 calls 0.023 USD; kvparse-sonnet 3 calls 0.163 USD; kvparse-haiku 4 calls 0.012 USD; tally-sonnet 5 calls 0.269 USD; tally-haiku 3 calls 0.009 USD; pager-haiku 4 calls 0.013 USD; tally-sonnet-2 3 calls 0.127 USD.
- bugfix-1 (pager on Sonnet) is the first seed run; docs/bugfix.md does not list it.

## Links

- docs/bugfix.md on branch impl/bugfix-cycle (commit e108d5e): Seed runs
