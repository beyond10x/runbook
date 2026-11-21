# Changelog

All notable changes to this repository are recorded here. One version covers the repository
(`sdc-drive`, `sdc-watch`, `sdc-goals`, `ui/`), released under bare-version tags such as `0.1.0`.
The crates are `publish = false`; releases are source releases at bare-version tags
(`docs/releasing.md`).

## [Unreleased]

### Added

- Conformance observation after close (`docs/conformance.md`, `story:conformance-observe-after-close`):
  `sdc-conform observe runs/<run>` holds the delivered product to the conformance suite of the
  run's own spec through `ess verify conform run --target process` and the generic HTTP adapter
  `sdc-conform-adapter`, with route tables written after the run by the node `conformance.table`
  (read-only, response schema, program check, two retries) and writes `runs/<run>/oracle.json`:
  `conformance: spec scenarios proven passed: k of n (lower bound); f failed; u not measurable;
  tables agree on a of n`.
  The suite runs once per table (`--tables`, default 3); a scenario passes only when every counted
  table that reached it passed it, fails only when every one failed it, and is disputed when they
  disagree (cause `disputed` when nothing failed). Several statuses may mean one outcome
  (`status: [409, 404]`). The check refuses a literal in an event payload or error field, and a
  check of a field filled from the input is recorded unverified (`unverified_checks`). The start
  command cites the README or manifest line that states it (`start.from`; `cargo run` and `go run`
  refused), and an argument or header a README describes as a test, demo or mock mode is refused.
  A refusal of a command on an id the scenario never created is read by the adapter as the
  command's unknown-instance answer (ESS: not-found, else `wrong_state`), and 404 for an id it
  created never; a start value the README leaves to the operator can come from the scenario
  (`start.args`, `from_scenario`); a slot refused after its retry gets one more session (five per
  slot). The figure goes into a release as an observation: `passed` is a lower bound, judged against
  the run's own specification; nothing else is claimed. A table whose run shows its own fault (a capture
  that never appears, a request naming a value the scenario leaves out, 401 or 403) is refused and
  asked again once with that evidence. `held` only when k = n > 0 (u > 0 is never held); a run
  without a counted table is `unknown` with a named cause and prints no counts
  (`not measured (<cause>), n scenarios`). A scenario is not
  measurable when it needs what the table declares `none` of kind
  `not_reachable_over_the_interface`; `not_implemented` fails. The table can capture values the
  product prints at start (`captures`), send them in paths, headers and bodies, and bind them to a
  spec input (`bindings`). `scripts/run-trial.sh` runs it after a close when `SDC_CONFORMANCE=observe`.
  Observation only: no gate. A scenario one counted table saw fail and the others declared
  unreachable counts failed. When every slot's table is refused only for the product's 401 or 403 in
  the same scenarios, none of which had a success before its refusal (a sign-in the specification
  does not model), those scenarios count failed with cause `precondition_not_in_spec`
  ("f failed, of which p refused for a precondition the specification does not model").
  `sdc-conform recompute` takes the figure again from stored table runs, `sdc-conform summary`
  prints the sanitized summary the progress ledger reads from
  `experiments/<id>/conformance/<run>.json`; the ledger shows conformance always (`not observed`
  where a run has none), with the baseline-0.1.0 figures. One text form in oracle.json,
  run-metrics, run-compare and the ledger. About 1 to 3 USD of node cost per observed product.
  A recorded re-drive (`run-trial.sh --recorded`) skips the observation: its recording holds no
  table call, and each one counted as a divergence.
- `driver/ess-pin.json`: ess 0.97.0 pinned by archive and binary sha256 per platform, resolved
  through `ESS_BIN` or a cache path (`sdc-conform ess-install`); the `ess` on PATH is not used.
- The row `conformance` in `sdc-run-compare`, `metrics.conformance*` and the text line in
  `sdc-run-metrics`; a conformance column, line and per-run figure in the progress ledger.
- Site, Evaluation: the chart "Spec scenarios proven passed (lower bound)" per release and setting
  (median, min to max, n; a run without an observation is "not observed", never 0), the
  observer's line and the failed count with failures for a precondition the specification does not
  model split out per run, and three cited sentences on what is checked, why it is a lower bound and
  what it costs.
- Site, "The LLM is not the runtime": the comparison of record `free-agent-0.1.0` drawn from the
  committed record, run summaries, timelines, `sizes.json` and conformance summaries: the result in
  the first paragraph, the timelines on one time axis and cost scale, medians, the hypothesis and
  its verdict per pair, product sizes, conformance of both sides, what the SDC run records that the
  free run does not, what was held equal and the confounds; quality evaluation pending.

## [0.1.3] - 2026-11-21

The documentation site and its generator. No protocol, binding or case changed. The longer note is
`docs/releases/0.1.3.md`.

### Added

- The documentation site, `website/` (Docusaurus with the beyond10x docs-system theme), published
  at https://beyond10x.github.io/sdc/ by `.github/workflows/pages.yml` and
  `.github/workflows/b10x-docs-site.yml`: what the SDC is, the main flow and a browser with one
  page per protocol, what it is built on, the evaluation per release and model setting, and one
  run step by step ("The LLM is not the runtime") (`3e93276`, `3c2930b`, `b7497fa`).
- `sdc-protocol-site`: writes the site's protocol data, `website/data/protocols/`, from the
  protocol YAML, the bindings and the committed check outputs; `check` fails on stale data and runs
  in `scripts/check-bindings.sh`; `site-manifest` writes the site's `.well-known/b10x-site.json`
  (`19209b8`, `3ba6bd6`).
- Each run in `docs/progress.json` carries its metrics (cost, wall time, model and decider calls,
  rework takes, review and merge rejections, looping factor and first-pass yield per node, stop
  cause; null where the run summary has none) (`c45bc91`).
- `task docs`, `docs:generate`, `docs:build`, `docs:test`: the site's data check, build and smoke
  test (every page at 1440 and 390 px in both themes, axe, internal links).
- Two experiment records registered before their runs, on the 0.1.2 tree: `release-0.1.2` (the
  baseline of 0.1.2 against `baseline-0.1.0`) and `effort-decider-mixed` (the mixed setting's Haiku
  decider at effort medium against xhigh) (`f619fd4`). Both are planned; no run has been made.

### Changed

- The three crates, their lock files and `ui/package.json` are at 0.1.3.

### Known limits

- No run has used the 0.1.2 or the 0.1.3 tree; the two registered records are planned, not run.
- A check of the delivered product against its specification is in work and not in this release.

## [0.1.2] - 2026-11-21

Corrections and tools after 0.1.1: recorded replay, the timeline of a run, one rework cap, the
LLM decider's principal renamed, and the root protocol's description. No rule of a protocol
changed. The longer note is `docs/releases/0.1.2.md`.

### Added

- Recorded replay: `scripts/run-trial.sh <run> --autopilot --recorded <source>` re-drives a
  recorded run with the real driver and programs and the recorded model replies (`sdc-replay`,
  `332d3aa`, `b5ead4c`, merged again in `8a96b50`), with a committed stub recording and seven
  re-drive cycles. `--set` changes a condition (`max_rejections`, timeouts, retries, run caps),
  `--inject` puts a decision in place of a recorded one, and `--on-divergence stop|nearest|live`
  says what a call the source never made gets. `sdc-replay` answers
  `--capabilities` (`26b8b85`). A re-driven run has `run.unstick` off unless the source called it,
  because a node the source never called has no recorded reply; `--set unstick=on` decides
  otherwise (`f717443`).
- `sdc-run-timeline`: the timeline of one run (`sdc.run-timeline/1`, `f461e61`), and the timelines
  of `trial-0.1.0-strong-1` and `trial-0.1.0-mixed-1` under `experiments/baseline-0.1.0/timelines/`
  (`89581f0`).

### Changed

- The LLM decider's principal is `decider:model`, whatever the model; its model id is recorded
  beside it in `run-settings.jsonl` and on each `decisions.jsonl` line (D149, `41d0023`). Before,
  the strong runs of trial 3 recorded an Opus decider as `decider:haiku`. `SDC_DECIDER=haiku` stays
  an alias of `model`. For a reader of old runs: `decider:haiku` in a run before 0.1.2 is the same
  principal as `decider:model`; the tools and the live view read both, and `sdc-experiment` counts
  `haiku` and `model` as one setting.
- One effective rework cap (D184, `9940eed`): `rework_limit` of `rule-ends.json` is the default,
  `SDC_MAX_REJECTIONS` overrides it when set, and a value that is not a whole number exits 64.
  `run-settings.jsonl` records it as `rework_limit: {value, source}`; the driver and the writer's
  rework prompt both read that value. Before, the prompt said "of at most 2" whatever the setting.
- "SDC" stands for "software delivery cycle" (the operator, 2026-11-21). The root protocol
  `protocol/sdc.protocol.yaml` said "software development cycle" in its description; it now says
  "delivery". That changes its compiled IR (`protocol/compiled/sdc.ir.json`, sha256
  `41c6acc7…` to `8a05e12f…`) and so the IR hash a new case records, but no rule: no action,
  condition, outcome or binding changed, and `canon simulate` gives the pinned report byte for byte.
  Runs recorded before keep their own IR in their logs; the committed replay recording still
  re-drives with every call matched (`1109656`).
- The three crates, their lock files and `ui/package.json` are at 0.1.2.

### Corrections to 0.1.0

`docs/releases/0.1.0.md` and the GitHub Release of 0.1.0 are not edited; this section holds the
corrected figures.

- Decider cost: `sdc-run-compare` summed the cost of the `decisions.jsonl` lines that are not
  `escalation_suppressed`, so every decider call whose escalation was suppressed (a real model
  call, its cost sits on the suppressed line) was missing from the decider cost and the total.
  The 0.1.0 Trial 3 figures were taken from the tool. Recomputed for the seven passes as the sum
  of `cost_usd` over `llm-calls.jsonl` and over every line of `decisions.jsonl`; the decider
  figure also equals the sum of `decider/*/response.json`.

  | Run | Published (USD) | Recomputed (USD) | Difference |
  |---|---|---|---|
  | `trial-3-strong-2` | 3.00 | 3.00 (2.996) | 0 |
  | `trial-3-strong-1` | 2.93 | 2.93 (2.926) | 0 |
  | `trial-3-opus-1` | 5.09 | 5.09 (5.088) | 0 |
  | `trial-3-mixed-6` | 1.86 | 1.86 (1.859) | 0 |
  | `trial-3-haiku-7` | 1.00 | 1.03 (1.034) | +0.03: its decider cost is 0.371, not 0.335 (1 suppressed line) |
  | `trial-3-strong-6` | 4.05 | 4.05 (4.055) | 0 |
  | `trial-3-strong-5` | 5.34 | 5.34 (5.338) | 0 |

  Only `haiku-7` has a suppressed-escalation line among the seven; the others have none, so their
  published figures stand. Other runs of `docs/trial-3.md` changed the same way (`haiku-6`
  0.540 to 0.560, `mixed-5` 1.476 to 1.588, and the attempts), and `docs/trial-3.md` is corrected.
- The fix of the tool is in 0.1.1 (`df58754`, `9ff2996`; `docs/releases/0.1.1.md`, "A defect in a
  figure tool"); this section is the correction of the 0.1.0 figures that note promises
  (`8e61332`, `956f1df`).

### Known limits

- No run has used the 0.1.2 tree. The baseline is still `experiments/baseline-0.1.0`: 6 of 9
  clean passes on the 0.1.0 tree.
- A check of the delivered product against its specification is in work and not in this release.
- Recorded replay re-drives the committed stub recording; a re-driven run reproduces the source's
  calls and has no reply for a node the source never called.

## [0.1.1] - 2026-11-21

Fixes and tools after the first release; no protocol of the nine changed. The longer note is
`docs/releases/0.1.1.md`.

### Added

- Keep-going (`scripts/run-trial.sh <run> --autopilot --keep-going`, D142): trial and eval runs
  force past autopilot's stops and record each defect in `defects.jsonl`; a forced run never
  counts as an end-to-end pass (`2a95956`).
- `run.unstick` (D147, `sdc-unstick`): under autopilot, before a stop that is no cap, a model picks
  a legal move (run a writer again, ask again, end), checked by program (`b8a0116`); its class
  `unstick` has the default effort `xhigh` (`0f7eb8f`).
- `sdc-drive reopen` and `run-trial.sh --reopen`: a step that ran at a case's current state but
  recorded nothing runs once more on the unchanged state, and a refused decision is put back
  (`3cbda08`, `6c4886a`).
- `sdc-check-routes`, run from `scripts/check-bindings.sh` (D148): the static check of the reject
  and cap routes, whose findings must equal the list in `docs/reject-routes.md` (`a9a1185`).
- The simulate step of `scripts/check-protocols.sh`: `canon simulate` on every protocol, compared
  byte for byte with `protocol/simulate/<name>.txt` (`c8747ef`), pinned to Canon `0.10.0`
  (commit `2c7a1ea6`), the first Canon release with `simulate`.
- Workflow metrics and experiment records: `sdc-run-metrics`, `sdc-experiment` (records under
  `experiments/`, their reports and `INDEX.md`, checked by `check-bindings.sh`) and the progress
  ledger `docs/progress.md` with its image `docs/img/progress.svg`, written from
  `experiments/releases.yaml` (`a0f8e9e`, `6296229`; `docs/workflow-metrics.md`,
  `docs/improving.md`).
- The live view's run picker: a search over recent runs with their facts, grouped by experiment
  (`009b8fd`, `4e09b4c`, `ab8472f`).
- Three more `sdc-bugfix` seeds and four scripted bug-fix cycles (`99238c0`, `e108d5e`).
- The first baseline on a released tree: `experiments/baseline-0.1.0`, nine runs on the 0.1.0
  tree, recorded in the progress ledger.

### Changed

- `docs/releasing.md` § Standing rule: under the operator's standing order of 2026-11-21 a patch
  or minor release of a measured improvement needs no further word; the orchestrating session
  orders the cut once `main` is green on the release commit, and the merging session tags and
  publishes on that order.
- The three crates, their lock files and `ui/package.json` are at 0.1.1.

### Fixed

- Reject and cap routes of the audit: a writer's failed run is fed back to its next run (R9,
  `5297ef0`); a writer's runs are capped per retry kind (R7, `a2f363e`); a failed check's own
  output reaches the writer (R13, `97024ce`); an unattributed scope-changing answer goes to the
  decider as a question (R2, `a919898`); a refused finding decision is put to the decider again
  with the refusal (R3, `306fde9`).
- Verdicts and counts are read from stdout alone; an unreadable result is a tool error
  (`7af5a97`).
- `sdc-bugfix` attributes the reproducing test by its own run: the red-check defect named in the
  0.1.0 limits (`99238c0`).
- The driver counts no not-called run as an LLM call (`9ce86bf`).
- `sdc-run-compare` and the live view's KPI strip count every decider call made, once per record
  (`df58754`, `9ff2996`). Before, `sdc-run-compare` left out the cost of every decider call whose
  escalation was suppressed (`trial-3-haiku-9`: 0.159 USD reported, 0.325 USD spent).

### Known limits

- No run has used the 0.1.1 tree. The baseline of 0.1.0 is `experiments/baseline-0.1.0`: 6 of 9
  clean passes (strong 3 of 3, median 5.50 USD and 1,247 s; mixed 3 of 3, median 2.55 USD and
  1,489 s; Haiku 0 of 3, stopped `writer_spent` twice and `decision_not_takeable` once).
- Recorded replay is not in this release: merged and reverted before it (`7c5dd5e`), it follows in
  a later one.

## [0.1.0] - 2026-11-21

The first release: a prototype. No interface is stable. The longer note, with the Trial 3 runs and
the limits, is `docs/releases/0.1.0.md`.

### Added

- Nine Canon `protocol/1` protocols: the eight of the cycle from an idea to a built, tested
  product (idea, intake, direction, epic, spec, plan, story, release) and `sdc-bugfix`, one bug fix
  red first (`protocol/*.protocol.yaml`, `docs/bugfix.md`).
- `sdc-drive` (`driver/`): evaluates a case tree on Canon, runs the tool bound to one admissible
  action, records the result as `canon-evidence/1`. Autopilot (a decider model in place of a
  person, final), a review node as a model call (`change.review`, `50d90b9`), rework after a
  rejected approval (at most two reworked, the third ends the run), a run that ends by rule
  (`rule:autopilot`), a finding re-raised after a decision stops the run `finding_reraised`
  (`6c7ba78`), a model per node class (`SDC_LLM_MODEL_*`), a hermetic `claude-cli` adapter with
  an explicit default effort per node class (`f747085`), and the requester-side decider
  (`decider:requester`, `goals/`).
- `watch/` (`sdc-watch`: file changes of a run) and `ui/` (the live view of a run).
- `scripts/run-finding-cycles.sh` (68 expected cycles and 7 pending on this tree) and
  `scripts/evaluate-cases.sh` (52 situations in 12 fixture cases).
- `docs/reject-routes.md` (the audit of the reject and cap routes), `docs/simulate-findings.md`
  (the triage of Canon's 71 `simulate` findings, 5 real dead ends) and `docs/trial-3.md`.
- Trial 3, 2026-11-21, recorded in runs that are not in git: five of eight batch runs of the
  to-do idea closed delivered, unattended, with an Opus decider and Sonnet work, a Haiku decider
  and Opus work, a mixed Sonnet and Haiku, or an all-Haiku setting; two more runs, with the review
  model call, closed delivered with an Opus decider and Sonnet for all work and the review
  (`docs/trial-3.md`, `docs/releases/0.1.0.md`).
- `CHANGELOG.md`, `docs/releasing.md`, `docs/releases/0.1.0.md` and a root `Taskfile.yml`
  (`task check`, `task release:check VERSION=<version>`).

### Changed

- `ui/package.json` and `ui/package-lock.json` carry the repository version, 0.1.0. The three
  crates and their lock files were already at 0.1.0.
- `README.md` § State names the release; `docs/driver.md` no longer lists `change.review` among
  the program stubs.

### Known limits

- Earlier runs' decisions were taken with the host's settings (the decider's call was not
  hermetic), so the passes are not reproducible on another machine as recorded; no pass ran on
  the hermetic adapter.
- Only two runs through the fixed review node have passed (`trial-3-strong-6`, `trial-3-strong-5`, on `main` plus the
  review fix, with a non-hermetic decider); five passes ran with the review step a stub.
- Nobody started the delivered product.
- The decider's principal is `decider:haiku` whatever model decides, and the driver's "LLM calls so
  far" counts runs that called no model.
- The bug-fix runs ran on a branch of which only `d2e4896` is in this release; the red-check defect
  fixed in `99238c0` is not, and no scripted cycle covers `sdc-bugfix`.
- The driver's runtime parts are a temporary copy of what the Loom runtime must provide.
- Release and operate are being split out of the cycle; that is not in this release.
