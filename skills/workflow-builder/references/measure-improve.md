# Raw material for Part B in the SDC record (retrospective)

Everything here is retrospective: computed by hand from `runs/trial-3-*` on 2026-11-21 between
10:45Z and 10:53Z, not by `sdc-run-metrics`, and across trees that changed between attempts. None
of it establishes an effect. Each line is a hypothesis to register and test with the procedure in
`SKILL.md` § B2 (n ≥ 3 per variant, one variable, same tree).

## Samples per setting

| Setting (work / decider) | Runs | Ended clean `closed:operating` | Other ends | Trees |
|---|---|---|---|---|
| all Haiku / Haiku | 7 (`trial-3-haiku`, `-2` to `-7`) | 0 | 4 stops, 1 stop then reopen, 2 open at 10:53Z | 6 different |
| mixed (Sonnet writer and implement) / Haiku | 6 (`trial-3-mixed`, `-2` to `-6`) | 0 | 3 stops, 2 `closed:not_delivered`, 1 open | 6 different |
| Sonnet / Haiku | 1 (`trial-3-sonnet-1`) | 0 | `stopped:decider_budget_spent` 10:50:37Z | `acd6f12` |
| Opus / Haiku | 1 (`trial-3-opus-1`) | 0 | open at 10:53Z | `acd6f12` |
| Sonnet / Opus | 2 (`trial-3-strong-1`, `-2`) | 2 | — | `acd6f12` |
| small idea, Haiku / Haiku | 1 | 0 | `stopped:writer_spent` (`change.implement`) 10:50:11Z | `acd6f12` |
| small idea, Sonnet / Haiku | 1 | 0 | `stopped:decider_budget_spent` 10:46:51Z | `acd6f12` |

The last three stops happened after this record ends (10:43Z) and are given as read at 10:53Z.

## Decider: Haiku against Opus

- Opus: 22 decisions in 2 runs, 22 approved, 0 rejected, 0 escalations, decider cost 1.955 and
  1.962 USD per run (`decisions.jsonl`, `decider/*/response.json`).
- Haiku: rejections in `trial-3-haiku-{3,4,5}` (1, 2, 2), `trial-3-mixed-{2,3,5}` (1, 3, 3),
  `trial-3-small-haiku-1` (1); `escalation_suppressed` 1 to 4 per run; decider cost 0.01 to 0.35
  USD per run.
- Hypothesis H1 (decision class `release`): the Opus decider reaches `closed:operating` more often
  than Haiku at equal work setting. Guard: product faults found by an independent review. Not
  testable from this record: no run pairs the deciders at one work setting and tree.
- Hypothesis H2: some Opus approvals are false approvals. Needs an injected-fault replay: a known
  missing requirement at the release, then count approvals.

## Work nodes: Haiku against Sonnet

- Attempt 2, one run each, same tree: Haiku 17 findings, 6 `spec.author` calls, stopped in spec;
  mixed 4 findings, 2 `spec.author` calls, three stages further (`trial3/attempt2-notes.txt`).
- Attempt 4, one run each, same tree: Haiku 15 findings, 0.669 USD; mixed 5 findings, 1.487 USD
  (`trial3/attempt4-notes.txt`).
- Hypothesis H3 (node `spec.author`): Sonnet has a lower `looping_factor` than Haiku. Two single
  pairs point that way; `trial-3-sonnet-1` contradicts it (below).

## Looping factor, approximated

Approximation: LLM calls of the action / distinct cases with a call (`llm-calls.jsonl`), read at
10:53Z. The real `looping_factor` counts real takes, including runs that called no model.

| Run | `spec.author` | `change.implement` |
|---|---|---|
| `trial-3-haiku-2` | 6/1 | — |
| `trial-3-haiku-4` | 2/1 | 5/2 |
| `trial-3-haiku-5` | 4/1 | 3/2 |
| `trial-3-mixed-3` | 1/1 | 5/3 |
| `trial-3-mixed-5` | 1/1 | 5/2 |
| `trial-3-sonnet-1` | 15/1 | 1/1 |
| `trial-3-small-sonnet-1` | 8/1 | — |
| `trial-3-small-haiku-1` | 3/1 | 5/1 |
| `trial-3-strong-1`, `-2` | 1/1 | 3/3 |

- The two clean passes are the only runs with `looping_factor` 1.0 on both nodes.
- `trial-3-sonnet-1` and `trial-3-small-sonnet-1` (Sonnet spec writer, Haiku decider) looped on
  the spec until the decider's budget of 20 calls ran out; their last decider records are trace
  findings (`trial-3-sonnet-1` `decider/018` to `020` all on `p-trace-r7`; `trial-3-small-sonnet-1` `decider/019-p-trace-r7`, `020-p-trace-r5`). Hypothesis H4 (edge
  `finding@spec -> spec`): the spec loop is driven by the decider's finding decisions, not by the
  writer's model. Test: the same run with the Opus decider, n = 3.

## Edges that cost the most (single observations)

- `merge.approve:rejected -> change.implement`: reworked 4 times in A4 and A5, each then approved.
- `spec.trace:finding -> spec.author`: the `writer_spent` of `trial-3-haiku-2`; 17 findings raised.
- `arrival:` from story-stage amendments: 8 of 14 late findings of trial 2 arose in the story
  stage (`docs/trial-2.md:53`).

## What the first-pass phase must leave behind for Part B

- Every run directory with run settings, call records, decision records, stop and defect records,
  status and replay (SKILL.md "What to record").
- The exact tree commit of each run; runs on different trees are not one group.
- Recorded model replies of the passing runs, for recorded replay (Phase 4.3).
- Effort per call. This record has the model per call but no effort level
  (`run-settings.jsonl` has no effort field; a branch `feat/effort-per-class` exists at `d241d21`
  with no commits of its own).
- The principal of every decision naming the model that decided (F12).
