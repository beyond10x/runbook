# free-agent-0.1.0: One free-running agent session against the SDC of release 0.1.0, the to-do idea

Written by `sdc-experiment report free-agent-0.1.0` from `experiments/free-agent-0.1.0.yaml` and the run summaries in `experiments/free-agent-0.1.0/runs/`; `sdc-experiment check --all` fails when it is stale. Definitions: `docs/workflow-metrics.md`.

- Kind: prospective; status: complete; created 2026-11-21.
- Purpose: Whether the SDC, where the LLM is not the runtime, costs less than one agent doing the whole cycle in one go with its own tool calls.
- Hypothesis: A free-running agent given the same idea and the same list of deliverables costs more (run.cost_usd and run.tokens_total) than the SDC run on 0.1.0.
- Input `todo-app` (idea, sha256 f384a752a2cb): A to-do list web app for me and my partner that also works on the train without a connection
- Tree: `3ad339130d8b4edc8b88f63a072ada4e6adc6c82`
  - the free runs on 3ad3391 (feat/run-timeline, on main 7c5dd5e of 0.1.1); the SDC runs on 4468bfd (release 0.1.0), from experiments/baseline-0.1.0.yaml
- Runs per variant: 3; pass rule: clean-delivery (closed, delivered, no reopen, forced step or unstick).
- Decision rule (complete-separation): n = 3 per variant. Held when, on run.cost_usd and on run.tokens_total, every free run lies above every run of strong and above every run of mixed (four pairs, each complete separation, exact one-sided p = 1/20 per pair); contradicted when any pair separates the other way; inconclusive otherwise. Every free run that ends counts, capped or failed, with its cost so far; a capped run counts as not delivered. No run is helped, repaired or rerun. run.wall_s and delivery are reported and decide nothing.

## Settings

| setting | strong | mixed | sonnet |
|---|---|---|---|
| `adapter` | metaharness | metaharness | - |
| `autonomy` | A5 | A5 | - |
| `decider.adapter` | claude-cli | claude-cli | - |
| `decider.escalate` | final | final | - |
| `decider.kind` | haiku | haiku | - |
| `decider.max_decider_calls` | 40 | 40 | - |
| `decider.model` | claude-opus-5-5 | claude-haiku-5-5 | - |
| `free.adapter` | - | - | claude-cli |
| `free.effort` | - | - | high |
| `free.max_turns` | - | - | 1000 |
| `free.max_usd` | - | - | 25 |
| `free.max_wall_s` | - | - | 5400 |
| `free.model` | - | - | claude-sonnet-5-5 |
| `max_llm_calls` | 150 | 150 | - |
| `max_wall_seconds` | 5400 | 5400 | - |
| `models.implement.model` | claude-sonnet-5-5 | claude-sonnet-5-5 | - |
| `models.other.model` | claude-sonnet-5-5 | claude-haiku-5-5 | - |
| `models.review.model` | claude-sonnet-5-5 | claude-sonnet-5-5 | - |
| `models.writer.model` | claude-sonnet-5-5 | claude-sonnet-5-5 | - |
| `review.adapter` | claude-cli | claude-cli | - |

Known confounds:

- `decider`: the SDC's strong setting decides through an Opus decider (effort xhigh), the mixed setting through a Haiku decider with Haiku also on the other class; the free agent has no decider and answers its own questions
- `shell`: the free agent has a shell and builds and tests its own code; the SDC's model nodes have no shell and programs check what they write
- `adapter`: the SDC's work nodes ran through metaharness (review and decider through claude-cli); the free agent runs claude -p directly with the claude-cli adapter's hermetic flags plus --include-partial-messages; the same Claude Code binary 2.1.330
- `delivery`: the SDC delivers by closing operating after its gates (requirement trace, ESS gate with compile and synthesize, plan cover, story tests, review, merge, release); the free agent is measured by program only as far as ended by itself, ess specify validate --strict-requires, cargo build and cargo test --locked of a copy, and the five deliverables present; conformance not measured unless stated in the facts
- `tree`: the SDC runs are on 4468bfd, the free runs on 3ad3391; the free prompt is driver/examples/sdc/free-agent/prompt.md (the cycle's deliverables and implementation constraints in one prompt); the example spec fixture is unchanged since 4468bfd
- `builds`: the free agent's cargo jobs run through slot in a confinement with a fresh cargo home, no network and no compiler cache wrapper; the SDC's builds ran through slot with the host's cargo configuration
- `wall`: an SDC run's wall time runs from its first case event to its last; a free run's from the launcher's start to the session's end
- `login-and-network`: the free session can read its own login (the copy in its home) and reach the network through its shell; no SDC node can (the implementing node has no shell, and programs run what is built)
- `size`: the two sides may deliver products of different size; both are counted by the same program (sdc-free-agent size, added after tree_commit as measurement only) on the SDC run's published tree (store/IDEA-1/work/published, the same bytes as release) with its requirements.yaml and spec, and on the free run's work/impl, requirements.yaml and spec, as product lines, web asset lines, tests, requirements, scenarios that ess 0.96.0 synthesizes from the spec, files, and whether the README says how to start it (experiments/free-agent-0.1.0/sizes.json); the counts are reported and decide nothing

## Runs

| run | variant | end | clean pass | cost USD | wall s | looping factor | note |
|---|---|---|---|---|---|---|---|
| trial-0.1.0-strong-1 | strong | closed: operating | yes | 5.432 | 1075 | 1.417 |  |
| trial-0.1.0-strong-2 | strong | closed: operating | yes | 6.194 | 1247 | 1.455 |  |
| trial-0.1.0-strong-3 | strong | closed: operating | yes | 5.503 | 1430 | 1.333 |  |
| trial-0.1.0-mixed-1 | mixed | closed: operating | yes | 2.497 | 1480 | 1.167 |  |
| trial-0.1.0-mixed-2 | mixed | closed: operating | yes | 3.883 | 2091 | 1.333 |  |
| trial-0.1.0-mixed-3 | mixed | closed: operating | yes | 2.549 | 1489 | 1.417 |  |
| free-agent-0.1.0-sonnet-1 | sonnet | completed | yes | 0.47 | 164.427 | - |  |
| free-agent-0.1.0-sonnet-2 | sonnet | completed | yes | 0.414 | 144.753 | - |  |
| free-agent-0.1.0-sonnet-3 | sonnet | completed | yes | 0.478 | 175.676 | - |  |

## Per variant

Median [min, max] (n) over the ended runs.

| | strong | mixed | sonnet |
|---|---|---|---|
| runs listed | 3 | 3 | 3 |
| ended | 3 | 3 | 3 |
| clean passes | 3 | 3 | 3 |
| `run.cost_usd` | 5.5029 [5.432, 6.1937] (3) | 2.5488 [2.4968, 3.883] (3) | 0.4701 [0.4137, 0.4779] (3) |
| `run.llm_calls` | 21 [20, 21] (3) | 20 [18, 21] (3) | - |
| `run.decider_calls` | 13 [13, 13] (3) | 12 [11, 15] (3) | 0 [0, 0] (3) |
| `run.wall_s` | 1247 [1075, 1430] (3) | 1489 [1480, 2091] (3) | 164.427 [144.753, 175.676] (3) |
| `run.driver_s` | 968 [788, 1168] (3) | 840 [741, 1107] (3) | - |
| `run.real_takes` | 121 [104, 127] (3) | 110 [97, 113] (3) | - |
| `run.rework_takes` | 56 [34, 57] (3) | 40 [28, 43] (3) | - |
| `run.looping_factor` | 1.417 [1.333, 1.455] (3) | 1.333 [1.167, 1.417] (3) | - |
| `run.first_pass_yield` | 0.75 [0.727, 0.75] (3) | 0.75 [0.75, 0.833] (3) | - |
| `run.rework_cost_usd` | 1.1438 [1.0674, 2.0766] (3) | 0.53 [0.5195, 1.7266] (3) | - |
| `run.furthest_stage_index` | 7 [7, 7] (3) | 7 [7, 7] (3) | - |
| `run.findings_raised` | 7 [5, 8] (3) | 8 [6, 12] (3) | - |
| `run.waiting_on_person_s` | 0 [0, 0] (3) | 0 [0, 0] (3) | 0 [0, 0] (3) |
| `run.clean` | 1 [1, 1] (3) | 1 [1, 1] (3) | 1 [1, 1] (3) |

## Loops

Per variant, over the ended runs: of the writer and approval nodes taken again at least once, the three with the highest median looping factor; and the three edges with the highest loop cost.

- strong (n 3):
  - `spec/spec.author` (llm-writer): looping factor median 3 (max 3, n 3), rework takes 5, rework cost 0.381 USD
  - `story/change.implement` (llm-writer): looping factor median 1.667 (max 2.5, n 3), rework takes 7, rework cost 2.951 USD
  - `direction/vision.write` (llm-writer): looping factor median 1 (max 2, n 3), rework takes 1, rework cost 0.048 USD
  - `story: change.review:rejected -> change.implement` (check): count 5 in 3 runs, loop cost 3.271 USD, loop driver 782 s
  - `story: finding@story -> intake` (finding): count 11 in 3 runs, loop cost 1.033 USD, loop driver 106 s
  - `spec: finding@spec -> intake` (finding): count 4 in 2 runs, loop cost 0.424 USD, loop driver 42 s
- mixed (n 3):
  - `direction/vision.write` (llm-writer): looping factor median 2 (max 4, n 3), rework takes 5, rework cost 0.013 USD
  - `story/change.implement` (llm-writer): looping factor median 1.333 (max 1.333, n 3), rework takes 3, rework cost 2.109 USD
  - `spec/spec.author` (llm-writer): looping factor median 1 (max 3, n 3), rework takes 2, rework cost 0.147 USD
  - `story: merge.approve:rejected -> change.implement` (approval): count 1 in 1 runs, loop cost 1.621 USD, loop driver 471 s
  - `story: change.review:rejected -> change.implement` (check): count 2 in 2 runs, loop cost 1.037 USD, loop driver 266 s
  - `spec: spec.validate:invalid -> spec.author` (check): count 2 in 1 runs, loop cost 0.147 USD, loop driver 48 s
- sonnet (n 3):
  - no node taken again, no edge

## Verdict

contradicted: held on every pair of primary metric and baseline is required for held; run.cost_usd, sonnet against strong: contradicted (ranges separate against the expected direction (median difference -5.0329)); run.cost_usd, sonnet against mixed: contradicted (ranges separate against the expected direction (median difference -2.0787)); run.tokens_total, sonnet against strong: contradicted (ranges separate against the expected direction (median difference -2847429)); run.tokens_total, sonnet against mixed: contradicted (ranges separate against the expected direction (median difference -2632626)).

Each pair of primary metric and baseline (median [min, max] (n)):

- `run.cost_usd` (increase), sonnet against strong: contradicted; sonnet 0.47 [0.414, 0.478] (3), strong 5.503 [5.432, 6.194] (3); ranges separate against the expected direction (median difference -5.0329)
- `run.cost_usd` (increase), sonnet against mixed: contradicted; sonnet 0.47 [0.414, 0.478] (3), mixed 2.549 [2.497, 3.883] (3); ranges separate against the expected direction (median difference -2.0787)
- `run.tokens_total` (increase), sonnet against strong: contradicted; sonnet 413432 [405147, 435634] (3), strong 3260861 [3182911, 4705034] (3); ranges separate against the expected direction (median difference -2847429)
- `run.tokens_total` (increase), sonnet against mixed: contradicted; sonnet 413432 [405147, 435634] (3), mixed 3046058 [2556660, 4191346] (3); ranges separate against the expected direction (median difference -2632626)
- guard `run.delivered` against strong: holds (worsening 0)
- guard `run.delivered` against mixed: holds (worsening 0)

## Facts

- Confinement of every free run (sdc-free-agent, bubblewrap): new mount, PID, IPC and UTS namespaces, killed with the launcher; writable only <run>/work (the working directory), <run>/home (HOME and the config home with a copy of the login, removed at the end) and <run>/cargo-home; /usr, /etc and the resolver stub /run/systemd/resolve read only; the Rust toolchain, claude, ess 0.96.0 and the cargo shim read only under /opt; the rest of the root an empty read-only tmpfs, so no repository checkout and no other run is visible; an empty environment plus PATH, HOME, the config home and cargo's. The session keeps the network (the model API needs it). cargo build, test, check, clippy and doc go through a broker to slot run, in a confinement of the same view without network and without the login. Probed before the runs with a Haiku session: writes to /etc, /usr, /opt and the run directory outside work/ fail, the other runs are not visible, cargo test runs through slot.
- Removal of the login home: the launcher of tree_commit removes the copied login file when it ends on its own path. Because a launcher killed by a signal runs no destructor, each run also had its own systemd user guard unit (sdc-exp-home-guard-<n>, started at 15:08Z beside the runs, outside the launcher), which waits for the run unit to stop for any reason and then removes <run>/home whole. Later trees remove <run>/home in the launcher and in the run unit's ExecStopPost. Each run's work/ (spec and product) is kept in its run directory.
- Conformance, observed after the verdict and deciding nothing: sdc-conform observe (feat/conformance-observe at df11e56, 3 tables, table node Sonnet at effort high) on copies of the free trees laid out as store/IDEA-1/work/{spec,published}; proven passed of the own spec's scenarios: sonnet-1 6 of 8 (2 failed), sonnet-2 10 of 11 (1 failed, 2 tables counted), sonnet-3 10 of 13 (3 failed); table cost 1.48 USD. Summaries in experiments/free-agent-0.1.0/conformance/; the baseline products' figures are in that branch's docs/conformance.md.
- After the last run, no run directory runs/free-agent-0.1.0-*/ held a home directory or anything in one (find printed nothing). The sk-ant- prefix, the login's tokens and the word .credentials count 0 in every event stream, outcome, run summary, timeline and conformance summary of the three runs.
