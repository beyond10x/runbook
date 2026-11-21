# The LLM is not the runtime: one free agent against the SDC of 0.1.0

On the to-do idea, one free-running Sonnet session was cheaper and faster than the SDC of release
0.1.0. Each of its three runs passed the program checks it was measured by. Median cost was 0.470
USD, 11.7 times less than the strong setting (5.503 USD) and 5.4 times less than mixed (2.549 USD).
Median wall time was 164 s, 7.6 and 9.1 times shorter. Median `run.tokens_total` was 413,432,
7.9 and 7.4 times fewer. The registered hypothesis, that the free agent costs more, is
**contradicted** on all four pairs of the record `experiments/free-agent-0.1.0.yaml`. Its products
are smaller: about two fifths of the Rust product lines (half with its page files counted), under
half the tests, and two thirds of the requirements and of the scenarios their specs synthesize
(below).

Record and data:

- the record: `experiments/free-agent-0.1.0.yaml`, registered in fd3a673 before the first run
- the report: `experiments/free-agent-0.1.0/report.md`
- the run summaries: `experiments/free-agent-0.1.0/runs/`
- the timelines: `experiments/free-agent-0.1.0/timelines/` and `experiments/baseline-0.1.0/timelines/`
- the product sizes: `experiments/free-agent-0.1.0/sizes.json`
- the tools: `sdc-free-agent` (launcher, outcome and size) and `sdc-run-timeline`

## The verdict, in the record's words

The rule is complete separation, n = 3 per variant. The hypothesis is held when, on
`run.cost_usd` and on `run.tokens_total`, every free run lies above every run of strong and every
run of mixed. It is contradicted when any pair separates the other way. It is inconclusive
otherwise.

| metric | against strong | against mixed |
|---|---|---|
| `run.cost_usd` | contradicted: free 0.414 to 0.478, strong 5.432 to 6.194 | contradicted: free 0.414 to 0.478, mixed 2.497 to 3.883 |
| `run.tokens_total` | contradicted: free 405,147 to 435,634, strong 3,182,911 to 4,705,034 | contradicted: free 405,147 to 435,634, mixed 2,556,660 to 4,191,346 |

Every pair separates against the expected direction. The exact one-sided p is 1/20 for each pair.
The `run.delivered` guard holds: all nine runs delivered by their own measures. Wall time was the
operator's expectation (the free agent may be faster). It was faster in all three runs. Wall time
decides nothing.

## One run each, side by side

From `sdc-run-timeline --text`. Tokens here count every model call, the decider included.

| | trial-0.1.0-strong-1 | trial-0.1.0-mixed-1 | free-agent-0.1.0-sonnet-1 |
|---|---|---|---|
| wall | 1075.17 s | 1480.01 s | 164.43 s |
| cost | 5.432 USD | 2.497 USD | 0.470 USD |
| tokens in / out | 354 / 136,559 | 304 / 288,138 | 26 / 25,122 |
| cache read / write | 3,130,449 / 684,241 | 3,115,123 / 798,154 | 333,634 / 46,365 |
| API requests | 163 | 145 | 13 |
| model sessions | 21 (768.5 s) | 18 (728.9 s) | 1 (151.0 s in requests) |
| decider calls | 13 (285.7 s) | 11 (736.0 s) | 0 |
| program steps | 106 (14.6 s) | 79 (11.1 s) | 0 |
| tool calls | 254 (6 denied) | 193 (3 denied) | 13 (12.1 s) |
| idle | 6.4 s | 4.0 s | 1.7 s |

## Three-run medians

| | strong | mixed | free (sonnet) | strong / free | mixed / free |
|---|---|---|---|---|---|
| `run.cost_usd` | 5.503 | 2.549 | 0.470 | 11.7 | 5.4 |
| `run.tokens_total` (registered) | 3,260,861 | 3,046,058 | 413,432 | 7.9 | 7.4 |
| tokens of every call (timeline) | 3,964,498 | 4,805,876 | 413,432 | 9.6 | 11.6 |
| `run.wall_s` | 1247 | 1489 | 164.4 | 7.6 | 9.1 |
| API requests | 163 | 157 | 13 | 12.5 | 12.1 |
| tool calls | 254 | 220 | 13 | | |
| program steps | 101 | 89 | 0 | | |
| decider calls | 13 | 12 | 0 | | |

On the SDC side, `run.tokens_total` comes from `sdc-run-metrics` and counts the model-node
sessions only. The decider's tokens are not in it, although its cost is in `run.cost_usd`. The
timeline row counts every call. Both figures put the free agent below every SDC run.

## What was delivered: the size of the products

`sdc-free-agent size` counts both sides with the same rule (`sizes.json` has the rule). The SDC
side is the run's published tree (`store/IDEA-1/work/published`, the same bytes as `release`),
with its `requirements.yaml` and spec. The free side is `work/impl`, with its `requirements.yaml`
and `spec/`. Scenarios are those ess 0.96.0 synthesizes from the spec.

| run | Rust product lines | web asset lines | tests | requirements | scenarios | files |
|---|---|---|---|---|---|---|
| strong-1 | 1235 | 0 | 25 | 5 | 17 | 9 |
| strong-2 | 1522 | 0 | 50 | 6 | 17 | 13 |
| strong-3 | 1292 | 0 | 42 | 7 | 20 | 12 |
| mixed-1 | 977 | 0 | 38 | 7 | 19 | 11 |
| mixed-2 | 1527 | 0 | 37 | 6 | 16 | 14 |
| mixed-3 | 1222 | 0 | 34 | 6 | 14 | 11 |
| free-1 | 488 | 138 | 17 | 4 | 8 | 10 |
| free-2 | 449 | 149 | 16 | 5 | 11 | 10 |
| free-3 | 604 | 136 | 19 | 4 | 13 | 11 |
| median, six SDC | 1263.5 | 0 | 37.5 | 6 | 17 | 11.5 |
| median, free | 488 | 138 | 17 | 4 | 11 | 10 |

The SDC products embed their page in Rust strings. The free products keep it in `src/index.html`
and `src/sw.js`. Even with those lines added (median 626), a free product has about half the code,
under half the tests, two thirds of the requirements and about two thirds of the scenarios. Every
README on both sides says how to start the product. The sizes do not say which product does more
of what the idea asks. They do say that the two sides did not deliver the same amount: the cost
factors above compare a smaller job with a larger one.

## What the SDC run has that the free run has not, as far as a program measured it

| | strong (3 runs) | mixed (3 runs) | free (3 runs) |
|---|---|---|---|
| program steps, checked by the driver, per run | 83 to 106 | 79 to 93 | 0 |
| Canon decision records (`evaluations/`) per run | 253 to 305 | 237 to 276 | 0 |
| evidence records (`evidence/`) per run | 103 to 127 | 96 to 111 | 0 |
| reviews by a separate node (`change.review`) per run | 4 to 5 | 4 | 0 |
| reviews rejected, then implemented again | 5 in 3 runs | 2 in 2 runs, plus 1 `merge.approve` rejected | 0 |
| rework takes, median | 56 | 40 | 0 |
| rework cost, median | 1.144 USD | 0.530 USD | 0 |
| cases replayed to the logged decision and state digests | 29 of 29 | 30 of 30 | no replay exists |

The free agent checked itself. Through the broker it ran `cargo build` and `cargo test` 3 or 4
times per run, all exit 0, and it ran `ess` commands 4 to 8 times. No program or second model
checked its work during the run. `sdc-free-agent measure` checked it once afterwards. Rework
cost is the money the SDC spent on second takes; all of it is part of the SDC's cost above.

## What nobody measured

Nobody measured whether either product does what the idea asks.

Conformance was observed afterwards, on both sides, with the same tool: `sdc-conform observe`
from branch `feat/conformance-observe` at df11e56, 3 tables per product, table node Sonnet at
effort high. It holds a product to the conformance suite of **its own run's spec**. It is not a
check against the idea, and it decides nothing in the record. The free products were observed on
copies of their trees, laid out as the tool reads them (`store/IDEA-1/work/spec`, and
`work/published` holding `work/impl`). Table cost was 1.48 USD of a 5 USD budget. The SDC figures
are from that branch's `docs/conformance.md`. The free summaries are
`experiments/free-agent-0.1.0/conformance/`.

| run | scenarios proven passed (lower bound) | failed | not measurable | tables counted |
|---|---|---|---|---|
| strong-1 | 16 of 17 | 1 | 0 | 3 |
| strong-2 | 11 of 17 | 2 | 4 | 3 |
| strong-3 | 7 of 20 | 13 | 0 | 3 |
| mixed-1 | 6 of 19 | 5 | 8 | 3 |
| mixed-2 | 2 of 16 | 8 (all refused for a precondition the spec does not model) | 6 | 3 |
| mixed-3 | 11 of 14 | 2 | 1 | 3 |
| free-1 | 6 of 8 | 2 | 0 | 3 |
| free-2 | 10 of 11 | 1 | 0 | 2 (one slot refused) |
| free-3 | 10 of 13 | 3 | 0 | 3 |

None of the nine products conforms to its own spec; every one fails at least one scenario. The
free products pass a larger share of their own, smaller suites (6 to 10 scenarios passed). The
SDC products pass 2 to 16, of 14 to 20. Each figure is measured against a different spec, so the
table does not rank the products against the idea. Each free run's work tree (spec and product)
is kept in its run directory, so the observation can be repeated on those trees with the same tool
as on the baseline products.

## What was held equal, and what was not

Held equal:

- the idea text (sha256 f384a752…)
- the deliverables: requirements, an ESS spec that passes `ess specify validate
  --strict-requires` with ess 0.96.0, a plan with stories, a Rust std-only implementation with
  passing tests, a self-review
- the implementing model (claude-sonnet-5-5) at effort high
- the Claude Code binary (2.1.330)
- builds through slot
- the same day, the same host

Not equal (the record's `known_confounds`):

- **The decider.** Strong decides through an Opus decider at effort xhigh. Mixed decides through a
  Haiku decider, with Haiku also on the other class. The free agent answers its own questions.
  Hence the report against both settings.
- **The shell.** The free agent had a shell. It built and tested its own code, and could read its
  own login and reach the network. No SDC node has a shell.
- **The adapter.** The SDC's work nodes ran through metaharness. The free agent ran `claude -p`
  with the claude-cli adapter's hermetic flags.
- **The tree.** The SDC runs are on 4468bfd. The free runs are on 3ad3391, with one prompt
  (`driver/examples/sdc/free-agent/prompt.md`) that holds the cycle's deliverables and
  implementation constraints.
- **The delivery measure.** The SDC delivers by closing operating after its gates: requirement
  trace, ESS compile and synthesize, plan cover, story tests, review, merge, release. A free run
  counted as delivered when it ended by itself, all five deliverables were present, the spec
  validated, and a copy built and passed its tests with at least one test.
- **Wall time.** An SDC run is timed from its first case event to its last; a free run from the
  launcher's start to the session's end.

## Limits

- n = 3 per variant, one idea, one model family, one day. The verdict covers this idea and these
  settings.
- The free prompt was written for this experiment. A different prompt can give a different
  product and a different cost.
- The products differ in size (above). The cost factors compare unequal jobs. No figure here
  normalises cost by size.
- Costs are the figures the CLI and the harness report (`total_cost_usd`, the metaharness usage).
  Neither side's cost was recomputed from a price list.
- `run.tokens_total` leaves the decider out on the SDC side and counts everything on the free
  side. That understates the SDC; the timeline totals are given beside it.
- The SDC runs ran earlier the same day (times in `experiments/baseline-0.1.0.yaml`). The free
  runs ran from 15:07 to 15:14Z, two at a time. Prompt-cache state across runs was not
  controlled.
- The size count is by rule, not by reading. Test lines are cut by brace count. Page code embedded
  in Rust strings counts as Rust lines. `readme_says_how_to_start` only looks for a start command.
- Conformance was observed after the verdict, against each product's own spec, with an instrument
  still being rebuilt (its figure varied with the route table between node runs). It is reported
  and decides nothing.

## Safety of the free runs

The confinement, recorded in the record's facts:

- bubblewrap, with new mount, PID, IPC and UTS namespaces
- writable only: the run's `work/`, `home/` and `cargo-home/`
- the rest of the root read only or empty, with no repository checkout and no other run visible
- an empty environment plus what the session needs
- cargo jobs run through slot in a second confinement, with no network and without the login

The copied login in `<run>/home` was removed after every run. Each run had a systemd guard unit
outside the launcher that removed `<run>/home` when the run's unit stopped, for any reason.
Later trees do the same in the launcher and in the unit's `ExecStopPost`. After the last run,
`find runs/free-agent-0.1.0-* -path '*/home/*'` printed nothing.

No token traces were found: the `sk-ant-` prefix, the login's tokens and mentions of
`.credentials` all count 0 in each run's event stream, outcome, run summary and timeline. No tool
input of the three sessions names the config home, `$HOME`, `~/` or a `/home/` path outside the
run's `work/`.
