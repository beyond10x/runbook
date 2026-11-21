# baseline-0.1.0: where a run's dollars go, against one free agent session

The operator, 2026-11-21: "so, this is our target then 10% costs". The free agent
(`free-agent-0.1.0`, three runs of one Sonnet session each, same idea and deliverables) cost 0.47,
0.41 and 0.48 USD; the SDC's strong setting 5.50 USD (median) and mixed 2.55 USD. The operator's
direction for the way there: "reduce context to minimal fact sets / inputs - dont collect big
conversations, figure out where we can use cheeper models" (`inputs.md`, `models.md`).

## How it was computed

- Each `llm-calls.jsonl` line (one model session of a work node) and each `decisions.jsonl` line
  (one decider call) carries the session's token usage; the free runs' totals are the `result`
  event of `events.jsonl`. The runs are not in git.
- USD per token kind at list prices, per million tokens: Sonnet 5.5 input 2, output 10, cache write
  (1 h) 4, cache read 0.10; Opus 5.5 4, 20, 8, 0.20; Haiku 5.5 0.10, 0.50, 0.20, 0.01 (prompts up
  to 100K tokens; above that Haiku costs five times as much). These prices reproduce the recorded
  `cost_usd` of every run to the cent, except `trial-0.1.0-strong-2`'s decider (1.896 against
  2.082 recorded: lines without token counts) and the Haiku sessions over 100K tokens.
- Six closed runs (`trial-0.1.0-{strong,mixed}-{1,2,3}`); medians over three.

## Budget per run, by token kind

| | strong | mixed | free agent |
|---|---|---|---|
| model sessions | 34 (21 work, 13 decider) | 34 (19 to 24 work, 11 to 15 decider) | 1 (13 to 14 turns) |
| cache-write tokens | 684,241 | 862,776 | 40,939 |
| output tokens | 158,925 | 288,138 | 25,122 |
| cache-read tokens | 3,132,624 | 3,670,593 | 353,410 |
| USD cache write (loading context) | 3.41 (62 %) | 1.40 (55 %) | 0.164 (35 %) |
| USD output | 1.84 (33 %) | 0.98 (38 %) | 0.251 (53 %) |
| USD cache read (turns re-reading) | 0.37 | 0.20 | 0.035 |
| USD total | 5.50 | 2.55 | 0.47 |

Gap factors (SDC median / free median): sessions 34×; cache-write tokens 17× (strong) and 21×
(mixed), in USD 21× and 8.5×; output tokens 6.3× and 11.5× (the Haiku decider's thinking is most
of mixed's output, 145k to 225k tokens, at 0.07 to 0.11 USD), in USD 7.3× and 3.9×; decider 2.03
USD (strong) and 0.18 USD (mixed) against none; total 11.7× and 5.4×.

## Budget per run, by node class (USD, median of three)

| class | strong | of it cache write | mixed | of it cache write |
|---|---|---|---|---|
| implement (`change.implement`, 4 to 5 sessions) | 1.98 | 0.90 | 1.51 | 0.68 |
| decider (11 to 15 calls) | 2.03 | 1.27 | 0.18 | 0.09 |
| review (`change.review`, 4 to 5 calls) | 0.75 | 0.64 | 0.59 | 0.48 |
| writer (`spec.author`, `plan.decompose`) | 0.53 | 0.34 | 0.28 | 0.15 |
| other (intake and direction, 7 to 11 sessions) | 0.36 | 0.21 | 0.04 | 0.02 |
| reruns (part of the above; `losses.md`) | 1.14 | | 0.53 | |

- Opening a session costs before it does anything: every metaharness session's first request
  reads about 15,000 cached tokens (the agent harness's system prompt and the definitions of about
  23 offered tools) and writes 3,600 to 5,400; a short intake session re-reads that prefix on each
  of its 3 to 8 turns (55k to 60k cache-read tokens per session, `inputs.md`).
- The Opus decider writes about 12,500 tokens per call (0.10 USD); the review writes 33k to 36k
  per call (0.13 to 0.15 USD on Sonnet): its whole input, 108 to 119 KB, is inlined.
- The product differs: `trial-0.1.0-strong-1` delivered 1,722 non-blank Rust lines with 25 tests,
  `free-agent-0.1.0-sonnet-1` 658 with 17 (`work/impl`, counted with `find`, `grep`, `wc`).

## Levers, by the most they could save per run (from the tables above)

1. Decider on Haiku under Sonnet work: strong −1.85 USD (2.03 → 0.18, the mixed decider's
   cost). The mixed setting already takes it, with the intake nodes on Haiku (−0.32). Risk:
   decision quality has no oracle; the Haiku decider is slower (decider 693 to 952 s per mixed
   run against 227 to 262 s, `effort-decider-mixed`).
2. Work nodes on Haiku with every request under 100K tokens: mixed −2.2 USD at most (implement,
   review and writer 2.38 USD at Sonnet prices, about 0.12 at Haiku's). Risk: 0 of 3 Haiku clean
   passes in the baseline (1 of 7 in trial 3, `trial-3-haiku-7` at 1.03 USD); Haiku over 100K
   costs five times as much (`trial-0.1.0-haiku-1`: implement 1.66 USD recorded, 0.51 at the short
   price). `models.md`.
3. Implement as one request per story with a cap of two follow-ups: −0.53 (mixed) to −0.70
   (strong) estimated; the whole input cost after its first request, the most it could save, is
   0.81 to 1.07 (`inputs.md`). Risk: a node that cannot look something up writes wrong code;
   reruns.
4. Reruns removed by the queued changes: −0.53 (mixed) to −1.14 (strong). `losses.md`.
5. Decider approvals taken by program where a check already passed (context, vision, roadmap,
   release): strong −0.64, mixed −0.06 and −207 s (`models.md`).
6. Review on the smallest fact set (the changed files, the covered requirements and commands):
   −0.36 (mixed) to −0.46 (strong) if its input falls to a third (inferred, `inputs.md`). No
   review where a program check covers the point: up to −0.59 / −0.75, but the review made all 10
   rejections that found real gaps.
7. Intake, direction and `plan.decompose` as one schema request each: strong −0.17, mixed −0.04.
8. `spec.author` as one request with one follow-up: −0.10 (mixed) to −0.13 (strong).

## The floor with the current protocols

Mixed setting, no reruns, every lever that keeps Sonnet on the writer, implement and review nodes:
three stories' first implement sessions about 1.07 USD (mixed-1's four sessions, 1.42, less its
one rerun), three reviews about 0.45, two writer sessions 0.28, Haiku intake, direction and
decider 0.22: about 2.0 USD, 4.4 times the target. Without a review call 1.57; with the implement
conversation halved about 1.2. **0.45 USD is not reachable with Sonnet on the implement node**:
three implement sessions alone cost about 1.07. It is reachable in arithmetic only with Haiku
work nodes that pass: at Haiku prices the same first-pass sessions cost about 0.12, plus the Haiku
decider 0.18 to 0.24 and intake 0.04, about 0.4 USD (inferred; Haiku has passed once in 10
runs, at 1.03 USD). The structural part of the gap: a workflow of 34 sessions pays 34 context
loads (cache write 0.69 to 3.41 USD per run against 0.16), so the target also needs fewer
sessions: decider calls replaced by programs and one request per node instead of a conversation.

## Commands

```console
# token kinds per class, one run per command; USD = tokens × the prices above
{ jq -c '{class, model, u: .usage}' runs/<run>/llm-calls.jsonl
  jq -c '{class: "decider", model, u: .tokens}' runs/<run>/decisions.jsonl; } \
| jq -s 'group_by(.class) | map({class: .[0].class, sessions: length,
    output: (map(.u.output_tokens // 0) | add), cache_write: (map(.u.cache_creation_input_tokens // 0) | add),
    cache_read: (map(.u.cache_read_input_tokens // 0) | add), input: (map(.u.input_tokens // 0) | add)})'
# the free runs
jq 'select(.type=="result") | {usage, total_cost_usd, num_turns}' runs/free-agent-0.1.0-sonnet-<n>/events.jsonl
```
