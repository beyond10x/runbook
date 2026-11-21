# sdc-run-compare: one report over recorded runs

Built for trial 3 (`story:clean-unattended-run-compare`): the same idea run unattended twice in two
model settings, one report. Source: `driver/src/bin/sdc-run-compare.rs`; tests:
`driver/tests/run_compare.rs`, fixtures `driver/tests/fixtures/`. Cite format `path:line`;
*(inferred)* marks what was not observed.

```console
$ driver/target/debug/sdc-run-compare --run a=runs/<run> --run b=runs/<run> [--run c=…] \
    --out <report.md> [--json <report.json>]
```

- Exit 0: report written. Exit 2: a run directory cannot be read; stderr names the file (the
  missing `store/IDEA-1/events.jsonl`, or `<file>:<line>` of a malformed record) and no report is
  written. Exit 64: usage (fewer than two runs, a label twice, no `--out`).
- Deterministic: the same run directories give the same bytes. A run is printed by its directory
  name, never a path; no clock reading enters the report (`sdc-run-compare.rs:188`, `:592`).
- Markdown: one table per run, then `## Comparison`, one line per metric with every run's value
  and `<label> - <first label>` for each later run (a signed number, `same` or `differs` for
  text, `n/a` when one side is not recorded). JSON (`sdc.run-compare/1`): `runs[].metrics`,
  `runs[].details` (stops, who decided, rework per artifact), `comparison[]`.
- Only `store/IDEA-1/events.jsonl` is required. An absent record reads `not recorded` (the oracle
  check reads `none`); a record that is present and malformed is exit 2.

## Fields and sources

All paths are relative to the run directory.

| Metric | Source | Rule |
|---|---|---|
| end state | `store/IDEA-1/events.jsonl` (root `terminated.outcome`); `run-stopped.json` (`sdc.run-stopped/1`: `at`, `by`, `reason`, `verdict_at_stop`, `case`); `rounds.jsonl` last `verdict`, `case_dir` | `closed: <outcome>` when the root has a `terminated` event (`sdc-run-compare.rs:236`); else `stopped: <verdict> in <case>[, by <by>: <reason>]` from the stop record, else from the last round; a stop record naming another case than the last round shows both (`:266`) |
| elapsed wall time (s) | every case's `events.jsonl` `at` (not `-`) and `observed.jsonl` `observed_at` | last minus first, whole seconds, floor (`:245`) |
| time inside the driver (s) | `rounds.jsonl` `seconds` | sum (`:262`; written by `scripts/run-trial.sh:289`) |
| time outside the driver (s) | derived | elapsed minus inside |
| time waiting on a person (s) | `rounds.jsonl`, `run-settings.jsonl` `at`, `run-stopped.json` `at` | `waiting` (`:428`), below |
| decisions by a person | `action_finished` and `authority_recorded` `principal` in `events.jsonl`; `work/questions.yaml` `answered_by`; `work/findings.yaml` `decided_by`; `work/context.yaml` `status` | person actions (an authority record taken with an action at the same `at` counts once) + questions a person answered (`accepted`, `changed`) + resolved findings a person decided + context fields `changed` (`:327`) |
| LLM calls, LLM cost (USD) | `llm-calls.jsonl` (`scripts/run-trial.sh` `SDC_LLM_LOG`; `llm-node.sh:425`) | lines, Σ `cost_usd` (`:332`) |
| LLM calls / cost `[model]`, `[step kind]` | same | grouped by `model` and by `action` (the step kind); a model or step another run used reads 0 |
| decider calls, decider cost (USD) | `decisions.jsonl` | a line that names a `model` and is not `escalation_suppressed` is one LLM call (rule principals `decider:approve`, `decider:reject` name none; a suppressed escalation repeats a call already written); Σ `cost_usd` of those (`:348`) |
| total cost (USD) | derived | LLM + decider |
| findings raised / late / early | `work/findings.yaml` | raised = status not `rejected`; late = `stage` not `requirements` (as L4 of `docs/kpis.md`); early = the rest (`:373`) |
| findings resolved by agent default / decider / person | `findings.yaml` `status: resolved`, `decided_by` | `decider:*` is the decider; a person per below; other resolved (`agent`) is the agent's default |
| findings dismissed, unresolved, rejected | `findings.yaml` `status` | `dismissed`; anything raised and neither resolved nor dismissed (`open`, `needs_person`); `rejected` (an LLM finding the program check refused, not counted as raised) |
| rework revisions | `revised.artifacts` in every case's `events.jsonl` | per artifact distinct non-`none` revisions minus 1, summed over the tree: L6 of `docs/kpis.md`, `sdc-finding-kpis.rs` (`:395`) |
| oracle check | `oracle.json` (or `store/oracle.json`), the oracle file of `docs/eval-design.md` (oracle file): `held`, `contradicted` or `unknown` per check | `none` when absent; `held (n of n checks)`; else the worst result and the three counts (`:460`). No run writes this file yet: `eval/oracle-spec.sh` prints one line for a spec directory, and trial 3 must save its result as `oracle.json` *(inferred)* |
| replay | `replay.json` (`scripts/run-trial.sh:160`) | `<n> of <m> cases match` (state and decision both match the log) or `not recorded` (`:483`). The tool never runs `sdc-drive replay` (it needs Canon; no library call exists) |
| forced continuations | `defects.jsonl` (`sdc.run-defect/1`, keep-going, `docs/driver.md`) | lines with `forced: true`; `not recorded` without the file. A run with one forced is not a clean run |
| defects | `defects.jsonl` | `<cause> at <action> in <case> (forced|stops)` per line, `none` for an empty file |
| setting | last line of `run-settings.jsonl` (`sdc.run-settings/1`) | `autonomy`, `stand-in`, decider kind and model, and `models` / `model` as written when present (the per-step model setting of `story:model-per-node-variable`, shape not yet fixed) (`:490`) |

A person is a principal that is not empty, `decider:*`, `rule:*`, `program*`, `dry-run*`, `agent*`,
`non-interactive` or `derived`: so `person:<id>`, and the bare id or `human` older drivers wrote
(`sdc-run-compare.rs:109`; same rule as `sdc-finding-kpis.rs`). `details.decisions_by_a_person.principals`
lists the principals counted.

### Time waiting on a person

Same reading as L7 of `sdc-finding-kpis` on `impl/unattended-run`. A stop waits on a person when
the run did not go on by itself:

- a round that starts at or after the first `run-settings.jsonl` line: a settings line lies
  between its end and the next round's start (the script had ended and was started again);
- an older round (no settings record yet): its verdict is `awaiting_person` or
  `awaiting_authority`;
- the last round, when the root is not closed: until `run-stopped.json` `at` (never below 0).
  With no stop record its end is open: it is not counted and `details.waiting_on_a_person`
  says `open_end_not_counted` (no clock reading is allowed in the report).

The wait is next round's start minus this round's end. Time outside the driver that is not
waiting (stops during repairs) is the difference of the two rows. At autonomy A5 with a decider
the value must be 0.

## trial-2-train against `docs/trial-2.md`

Run `sdc-run-compare` on `runs/trial-2-train` and `runs/trial-2b-fresh`:

| Figure | `docs/trial-2.md` | tool | Why they differ |
|---|---|---|---|
| work calls, cost | 27, $0.503 (`:19`) | 27, 0.503 | none |
| decider calls, cost | 5, $0.024 | 5, 0.024 | none |
| findings raised, late | 17, 14 | 17, 14 | none |
| resolved by agent default, person | 14, 2 | 14, 2 | none |
| rework | 58 | 58 | none |
| time in the driver | 1009 s over 18 legs | 1009 s | none |
| wall clock, outside the driver | 5274 s, 4265 s | 5274 s, 4265 s | none |
| replay | exit 1, 7 of 9, "not re-run" | not recorded | the run directory has no `replay.json`; that figure came from a hand run |
| end state | stopped at f16 in `IDEA-1.todo.lists` | `stopped: awaiting_person in IDEA-1.todo.access (last round ended in IDEA-1.todo.lists), by agent:sdc-orchestrator: …` | the run directory now holds `run-stopped.json`, whose `case` is `IDEA-1.todo.access`; the last round and the finding f16 are in `IDEA-1.todo.lists`. The record is shown as written, with the round's case beside it |
| decisions by a person | L1 9 (2 actions, 4 questions, 1 context field, 2 finding decisions) | 9 | none |

`docs/trial-2.md` says 18 driver legs; `story:clean-unattended-run-compare` says 15 legs and
76 min. `rounds.jsonl` has 18 lines and 5274 s is 87.9 min; the story text is the one that
differs.
