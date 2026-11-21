---
format: aep.planning-md/3
id: story:conformance-observe-after-close
kind: story
status: proposed
title: Conformance figure per ended run, observed after close
summary: sdc-conform observe holds the delivered product to its spec's suite after the root closes and records k of n scenarios in oracle.json
tags:
- conductor-succession
relations:
- decomposes: epic:conductor-succession
scope:
- confidence: cited
  path: docs/conformance.md
- confidence: cited
  path: driver/src/bin/sdc-conform-adapter.rs
- confidence: cited
  path: driver/src/bin/sdc-conform.rs
- confidence: cited
  path: driver/src/bin/sdc-experiment/progress.rs
- confidence: cited
  path: driver/src/bin/sdc-run-compare.rs
- confidence: cited
  path: driver/src/conformance.rs
- confidence: cited
  path: driver/src/run_metrics.rs
- confidence: cited
  path: scripts/run-trial.sh
revision: 10
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T14:25:21Z", actor: "human:operator", revision: 2, executor: "agent:conformance-observe", correlation: "operator-standing-acceptance-2026-11-21"}
---
# Story: conformance-observe-after-close

## Outcome

Every ended run carries a conformance figure: `sdc-conform observe runs/<run>` holds the run's
delivered product (`work/published/`) to the conformance suite of the run's own spec with
`ess verify conform run --target process` and writes `runs/<run>/oracle.json` with
`conformance: k of n scenarios`, `held` only when ess says `passed` and k = n > 0. The figure shows
in `sdc-run-compare`, `sdc-run-metrics` and the progress ledger. Observation only: no gate, no
protocol change; it applies to runs that already ended.

## Context

Known limit of 0.1.0: nobody starts the delivered product (`docs/releases/0.1.0.md`). ESS 0.97.0
(tag `0.97.0`, commit `58242e0bc7`) carries `--target process`. Probe on `trial-0.1.0-strong-1`:
17 scenarios; a hand-written HTTP adapter got 13 of 17 against the started product.

## Rulings (the operator, through the coordinating session, 2026-11-21)

- The route table is written after the root closes by a separate node `conformance.table`: a fresh
  model session, read-only on the delivered tree and the compiled spec, replying through a response
  schema checked by a program (two retries with the check's reasons, then a named cause). Class
  other; model `SDC_LLM_MODEL_CONFORMANCE` (default Sonnet); effort explicit and recorded. The
  implementing node gets no new obligation.
- Observation after close (`run-trial.sh` calls it), not a protocol step.
- A deviation counts: a table that maps one (command, method, path, status) to two outcomes without
  a body discriminator, or that does not cover every command, outcome and view of the compiled
  spec, is refused; a refused table is `unknown` with a cause. Events built from the response body
  are marked `events_checked_from: response_body`.
- ess 0.97.0 is pinned in `driver/ess-pin.json` (sha256 per platform archive and binary), resolved
  through `ESS_BIN` or a cache path and verified; the `ess` on PATH is never used.

## Acceptance

Scripted cycles with a stub product and a stub table node record `held` (all pass), `contradicted`
(one deviation, with expected and observed of the failed check), and `unknown` with causes
`suite_empty`, `product_not_started`, `table_refused`, `product_build_failed`, `ess_pin_mismatch`;
and `sdc-conform observe` on `trial-0.1.0-strong-1` and `trial-0.1.0-mixed-1` with the real table
node records k of n for each.

## Scope

- `driver/src/conformance.rs`, `driver/src/bin/sdc-conform.rs`, `driver/src/bin/sdc-conform-adapter.rs`,
  `driver/examples/conformance-stub-product.rs`, `driver/tests/conformance.rs`, `driver/ess-pin.json`
- `driver/src/run_metrics.rs`, `driver/src/bin/sdc-run-compare.rs`,
  `driver/src/bin/sdc-experiment/progress.rs`, `docs/progress.*`
- `scripts/run-trial.sh` (after close), `docs/conformance.md`, `CHANGELOG.md`

## Not in this story

Gating (`story:conformance-against-implementation`, which now depends on this one), confinement of
the started product through Substrate, products without an HTTP JSON surface.
