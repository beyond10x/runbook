---
format: aep.planning-md/3
id: story:spec-author-format-on-system-source
kind: story
status: proposed
title: The spec.author prompt says format is declared on the system source
summary: 'A prompt change measured on its own: spec.author reruns after a format-version refusal (5 of 12 spec.validate refusals); redundant once ESS''s fixed hints are pinned'
tags:
- eval
- improvement-loop
relations:
- decomposes: epic:improvement-loop
scope:
- confidence: cited
  path: driver/examples/sdc/tools/llm-node.sh
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T16:04:35Z", actor: "human:operator", revision: 2, executor: "agent:sdc-improve", correlation: "operator-standing-acceptance-2026-11-21"}
---
# Story: spec-author-format-on-system-source

## Outcome

The `spec.author` prompt states that `format:` is read only on the source that declares the
system (`spec/system.yaml`), never on a domain file, so a writer that is refused for a format
version raises it in `system.yaml`.

## Context

In the 0.1.0 baseline, 5 of the 12 `spec.validate` refusals were a format version, each with a
hint (`experiments/baseline-0.1.0/losses.md`). `trial-0.1.0-haiku-3` stopped `writer_spent` after
the hint "declare `format: ess/18`": `llm/014-spec.author` wrote `format: ess/18` into
`spec/domains/list.yaml`, `spec/system.yaml` stayed at `ess/1`, and the next refusal added
`missing_declaration ... does not declare system:`. ESS confirmed (2026-11-21) that 74 hints of
its validator say "declare `format: ess/N`" without naming the source, that `format:` is read only
on the source that declares the system, and filed a story for the hints. This story becomes
redundant when the ESS release with the fixed hints is pinned; it is then dropped with that
reason.

## Smallest change

One sentence in the `spec.author` prompt (`driver/examples/sdc/tools/llm-node.sh`, the spec
writer's instructions): "`format: ess/N` is read only on the source that declares the system
(`spec/system.yaml`); a refusal that asks for a newer format is fixed there, with the highest
version any refusal names." Prompt change; no protocol or driver change.

## Hypothesis and metric

- Hypothesis: with the rule in the prompt, `spec.author` reruns after a format-version refusal
  fall.
- Primary metric: `spec/spec.author` reruns caused by `spec.validate:invalid` whose refusal is
  `unsupported_format_version` (baseline: 5 such refusals in 9 runs; haiku-3 twice in a row);
  the run metric that carries it is `edge.spec: spec.validate:invalid -> spec.author.count`.
- Maximum gain: one of the three Haiku stops (haiku-3); the format-version share of the
  `spec.validate` loop, under 0.35 USD over the 9 runs.
- Guards: the spec gate pass rate (`epic/spec.gate` passing per run), clean passes of strong and
  mixed, `run.cost_usd`.

## Acceptance

An experiment record with the Haiku setting, n ≥ 3 per variant, the prompt sentence behind a run
setting, reaches a verdict by `sdc-experiment report`; or the story is dropped when the ESS
release with the fixed hints is pinned first.
