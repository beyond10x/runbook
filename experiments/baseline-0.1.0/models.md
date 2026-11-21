# baseline-0.1.0: which nodes can run on a cheaper model, and the cheapest valid test

The operator, 2026-11-21: "figure out where we can use cheeper models". Budget and target:
`costs.md`; inputs: `inputs.md`. Sources: the nine runs of `baseline-0.1.0` (strong, mixed,
Haiku; 3 each, tree 4468bfd) and the trial-3 records `work-haiku-vs-sonnet` and
`decider-haiku-vs-opus` (confounded, see their reports). Haiku costs a twentieth of Sonnet per
token up to 100K tokens per prompt and a quarter above.

## Per node class

USD per run (median of three), reruns summed over the three runs with first takes in brackets.

| class | strong | mixed | Haiku | trial 3 |
|---|---|---|---|---|
| implement | Sonnet 1.98; 7 reruns [8 first takes] | Sonnet 1.51; 3 [9] | Haiku 0.51 (`trial-0.1.0-haiku-1`; 1.66 recorded: prompts over 100K); 5 [4]; two runs stopped before or at their first story | looping factor Haiku 2 [1.33, 4.5], Sonnet 1.67 [1, 2.5]; clean 1 of 7 each |
| writer (`spec.author`, `plan.decompose`) | Sonnet 0.53; `spec.author` 5 [3] | Sonnet 0.28; 2 [3] | Haiku 0.10 to 0.14; 9 [3]; `writer_spent` stops in haiku-1 and haiku-3 | Haiku `spec.author` looping factor median 3 (max 7) |
| review | Sonnet 0.75; 5 rejections, all real gaps | Sonnet 0.59; 2, real | Haiku 0.08 (haiku-1, 6 calls); 3 rejections, real (q9's Postgres against r1; `EditTitle` sets a field the spec does not) | in the Haiku runs a program check, not a model |
| other (intake, direction) | Sonnet 0.36 | Haiku 0.04; `vision.write` 5 reruns on `context.check` findings (0.013) | Haiku 0.035 | Haiku |
| decider | Opus 2.03, 13 calls, no rejection | Haiku 0.18, 11 to 15 calls, 1 rejection (real); 693 to 952 s | Haiku 0.08 to 0.24; haiku-2 stopped `decision_not_takeable` | Haiku clean 1 of 7, Opus 2 of 2; 1.49 against 2.96 USD |
| clean passes | 3 of 3, 5.50 USD | 3 of 3, 2.55 USD | 0 of 3, 0.28 USD | Haiku work 1 of 7 (`trial-3-haiku-7`, 1.03 USD) |

**Haiku matches** on the intake and direction nodes: all three mixed runs passed clean with them,
for 0.04 instead of 0.36 USD. Its review rejections in haiku-1 named real gaps, and the Haiku
decider's one merge rejection (mixed-2, nothing offline, after the Sonnet review had passed it)
was real. Neither the review nor the decider has been compared with the strong model on the same
inputs.

**Haiku fails**, by refusal class:

- `spec.validate` refused 12 times over the nine runs, 5 for the format version (haiku-1 twice,
  haiku-3 twice, mixed-2): the hint does not say that `system.yaml` declares it (`losses.md`,
  `story:spec-author-format-on-system-source`). haiku-3 stopped `writer_spent` on it.
- `tests.run` failed with three E0308 type errors (haiku-1 list.shared).
- A default against the build: q9 "Postgres database", taken by the non-interactive rule and
  rejected by the review against r1 (`story:default-answers-within-build-constraints`).
- Price: haiku-1's implement prompts passed 100K tokens: 1.66 USD recorded against 0.51 at the
  short price, near a Sonnet run's implement (1.51 to 1.98).

**Never tried cheaper**: the review on Haiku with Sonnet implement (the review class always moved
with the others); `plan.decompose` on Haiku alone; the decider at an effort below `xhigh`
(`effort-decider-mixed`, planned); approvals by a program; `run.unstick` (Opus in every run); the
requester.

## Decider: who should take each decision

| decision | calls per run | strong USD/run (Opus) | mixed USD/run, s (Haiku) | verdicts, six runs | taker |
|---|---|---|---|---|---|
| `context.approve` | 1 | 0.142 | 0.007, 23 | approve 6 | program |
| `vision.approve` | 1 to 1.3 | 0.164 | 0.013, 47 | approve 7 | program |
| `roadmap.approve` | 1 | 0.161 | 0.011, 41 | approve 6 | program |
| `release.publish` | 1 | 0.175 | 0.029, 96 | approve 6 | program |
| `scope.answer` | 3 to 3.7 | 0.319 | 0.021, 102 | approve 20 | Haiku at a lower effort |
| `amend.upstream` | 2.3 to 2.7 | 0.470 | 0.025, 137 | approve 14, one escalation suppressed | Haiku at a lower effort |
| `merge.approve` | 2.7 to 3.3 | 0.547 | 0.093, 343 | approve 17, reject 1 (real) | a strong model, or Haiku at `xhigh` |

- **Program**: these four approve an artifact whose checks already passed. The decider's
  `checks.json` lists every claim of the action with its value and the open obligations
  (`release-0.1.2.strong.1`, `decider/009-merge.approve`). A rule approves when every claim the
  action requires is true, no obligation is open and no earlier rejection exists, and asks the
  model otherwise, the way `rule:autopilot` already takes the close. Saving per run: strong 0.64
  USD, mixed 0.06 USD and 207 s (`story:decider-program-approvals`).
- **Haiku, lower effort**: `scope.answer` and `amend.upstream` choose among options the question
  writer listed, and a program check refuses an answer outside them. In the strong setting they are
  0.79 USD per run on Opus, 0.05 on Haiku.
- **Strong model**: `merge.approve` is the one decision that caught what the review missed.

## Saving per run, ranked

| change | strong USD/run | mixed USD/run | evidence and risk |
|---|---|---|---|
| implement, writer and review on Haiku | −3.10 at most | −2.26 at most | 0 of 3 clean in the baseline, 1 of 7 in trial 3; needs the refusal classes above fixed and prompts under 100K |
| decider on Haiku | −1.85 | taken | the mixed setting, 3 of 3 clean; slower |
| review on Haiku | −0.71 | −0.56 | never compared on the same inputs; under 100K only with the smallest fact set (`inputs.md`) |
| approvals by program | −0.64 | −0.06, −207 s | 25 of 25 approved |
| intake and direction on Haiku | −0.32 | taken | the mixed setting |

Head: **the review on Haiku**, because today's replay tests it on recorded inputs for under 0.2
USD. Then approvals by program (an offline test, no model). The implement and writer classes
wait for shadow calls.

## The cheapest valid test of one class on a cheaper model

`sdc-replay` re-drives a run with the recorded replies at every model call (`docs/driver.md`,
"Recorded replay").

What it can do today:

- **Switch one class**: a model the caller sets wins (`SDC_LLM_MODEL_<CLASS>`, `driver/src/bin/sdc-replay.rs:754`).
- **Every call of that class diverges**: `model` stays in the request digest (`normalise_request`).
  With `--on-divergence live` those calls go to the real adapter.
- **Divergence policy is global**: a later call whose inputs changed because the live node wrote
  something else also goes live, on its source model, paid and counted against `--live-cap`.

So today's replay is a valid single-class test only for a class that reaches later steps through a
verdict: the review (an approval leaves the tree as it was; the merge decider's `checks.json`
carries `change.reviewed: true`, not the findings) and the decider classes. A verdict that differs
sends the run down another path, and the cap stops it. That is itself the measurement.

For the writer and implement classes, every later call diverges. Two capabilities are missing:

1. **Shadow calls** (`--shadow <class>=<model>`): call the cheaper model live in a copy of the
   call's directory and run the node's own checks on its `out/` (`spec.validate` and `spec.gate`
   for `spec.author`; `cargo build` and `tests.run` for implement). Record pass or fail, cost and
   tokens in `replay.json`, then continue with the recorded reply. Later calls match by
   construction; n is every call of the class (`story:replay-shadow-class`).
2. **A per-class policy** (`--live-class <class>`): live for that class, `stop` for every other
   divergence. This is the test of what follows once the shadow test passes.

Sources: the strong runs of `release-0.1.2` (tree 72a3eef). The baseline runs are on 4468bfd: from
0.1.1 on they diverge at their first call (`request.effort`), so a replay must run on the source's
tree.

Each test, n, and pass criterion:

| test | n | passes when | cost |
|---|---|---|---|
| review on Haiku, today (record `review-haiku-replay`) | 3 replays of one strong source, 4 to 5 reviews each | every live review returns the recorded decision, a rejection names the same requirement ids, `sdc-replay compare` exits 0, `live` equals the review count | about 15 Haiku calls, under 0.2 USD (inferred) |
| decider approvals by program | the 25 approvals of six runs (none rejected), offline from `decisions.jsonl` and `checks.json` | the rule approves exactly where the decider approved; the reject side needs injected faults (`--inject check-fail@…`) | none |
| implement on Haiku, shadow | every implement call of 3 strong sources, about 15 | `cargo build` and `tests.run` pass at the first try at least as often as the recorded Sonnet first tries | about 15 Haiku calls |
| `spec.author` on Haiku, shadow | every call of 3 sources, about 8 | `spec.validate` and `spec.gate` pass at the first try at least as often as the source | about 8 Haiku calls |

## Commands

```console
# the review class on Haiku against a strong source, live only where the model changed
SDC_LLM_MODEL_REVIEW=claude-haiku-5-5 scripts/run-trial.sh review-haiku-replay-1 --autopilot \
  --recorded runs/release-0.1.2.strong.1 --on-divergence live --live-adapter claude-cli --live-cap 5
# (--live-adapter: the source's review adapter; --live-cap: its number of change.review calls)
driver/target/debug/sdc-replay compare runs/review-haiku-replay-1
jq -c 'select(.replay.mode=="live") | {action, model, cost_usd}' runs/review-haiku-replay-1/llm-calls.jsonl
# the decider: action, model, verdict and cost per call
jq -c '{action, model, decision, cost_usd, duration_ms}' runs/<run>/decisions.jsonl
```
