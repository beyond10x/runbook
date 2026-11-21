# Trial 1: the first SDC cycle with a real LLM

Idea (the operator, 2026-11-21, verbatim): "I want to build a to-do list web application". Run name
`trial-todo-1`. Executor: `tools/llm-node.sh` through the metaharness adapter (Claude Code,
operator login), model `claude-haiku-5-5` on every agent node. See `driver.md`, "LLM executor".

## Start and resume

```console
$ scripts/run-trial.sh trial-todo-1 "I want to build a to-do list web application"
```

Run it again with only the name after each of your decisions (`scripts/run-trial.sh
trial-todo-1`). Each round is one `sdc-drive run --at <now> --max-llm-calls 30
--max-wall-seconds 5400` on the root case. Exit 3: waiting on you, with the command printed;
exit 4: stopped otherwise (stuck or a cap), with the reason; exit 0: terminable or closed.

Dry run (stub idea, stubs write the artifacts, every `metaharness` command printed, nothing
called, approvals scripted as principal `dry-run`):

```console
$ scripts/run-trial.sh --dry trial-dry
```

Overrides: `SDC_LLM_ADAPTER` (`metaharness` default, `claude-cli`, `dry`, a program),
`SDC_LLM_MODEL`, `SDC_LLM_ENDPOINT` (a gateway root), `SDC_MAX_LLM_CALLS` (30),
`SDC_MAX_WALL_SECONDS` (5400), `SDC_PRINCIPAL` (who the printed commands name; default `$USER`).

## Continue with the Haiku decider

`trial-todo-1` waits on `roadmap.approve` in `IDEA-1.direction`. To let Haiku take that and every
later person's decision (merge, release, close) instead of the operator:

```console
$ SDC_DECIDER=haiku scripts/run-trial.sh trial-todo-1
```

Run it in the background (agent nodes take minutes). One bounded call per decision through the
same adapter (`tools/decider.sh`; model `claude-haiku-5-5`, 8 turns, $0.25 each, at most
`SDC_MAX_DECIDER_CALLS` = 20 per run). Each decision is taken as principal `decider:haiku` and
logged in `runs/trial-todo-1/decisions.jsonl` (time, case, action or question, revision,
decision, principal, reasons, cites, model, tokens, cost); the LLM's record is in `decider/NNN-<id>/`.
Exit 0: closed through `operating`, replay done; 3: Haiku escalated (or replied invalid JSON), the
person's command is printed as before; 6: Haiku rejected (recorded, stopped); 5: decider budget;
4: stuck or a driver cap. For an eval with no person at all add `SDC_DECIDER_ESCALATE=final`
(an escalate is logged `escalation_suppressed` and Haiku's `best` decision is taken).

Checked on a copy of this store (2026-11-21, `~/.cache/scratch/…/runs/todo-copy`): the first prompt
rejected the roadmap as "an empty stub" (it did not know the epics are separate AEP files); with the
prompt that says so it approved, citing r1, r2, r3, $0.0042, 11.6 s.

Other deciders: `SDC_DECIDER=approve` (rule: approve all, accept every default, close),
`reject` (rule: refuse and stop), `person` (default, today's behaviour).

**Open for a new run (not trial-todo-1):** `sdc-tool.sh check-questions` (line 55) and the
`sdc.intake` protocol accept a scope-changing answer only with `answered_by: human`, so the
decider's `answered_by: decider:<kind>` gets `needs_human` (exit 3) and the run stops at intake.
Needed change, for the owner of `sdc-tool.sh`: also accept `answered_by` starting with `decider:`.

## Expected LLM calls

One call per agent node on the happy path (the script prints this before starting):

| Case | Agent nodes | Calls |
|---|---|---|
| intake | `idea.dissect`, `questions.write`, `questions.answer`, `requirements.normalise` | 4 |
| direction | `context.default`, `vision.write`, `roadmap.write` | 3 |
| spec | `spec.author` | 1 |
| plan | `plan.decompose` | 1 |
| story (one per story, 1 to 3) | `change.implement` | 1 to 3 |
| **total** | | **10 to 12** |

Each failed check sends its writer back once more (`retry_when`, at most 3 runs per node, 4 for
`spec.author`); the tree is capped at 30 calls. The dry run printed exactly 10 commands (one
story).

## Where you act

Every stop prints what you decide, the path to read, and the exact command. The commands have
this form (case directories under `runs/trial-todo-1/store/IDEA-1/children/`):

| # | When | What you decide | Command |
|---|---|---|---|
| 1 | intake, only if a question is `scope_changing: true` | answer it: in `work/questions.yaml` set `answer`, `status: accepted` or `changed`, `answered_by: human` | edit the file, then `scripts/run-trial.sh trial-todo-1` |
| 2 | direction, after `context.default` | approve the context (`work/context.yaml`) | `sdc-drive --canon … --case-dir …/IDEA-1.direction approve --at "$(date -u +%Y-%m-%dT%H:%M:%SZ)" --capability owner.approve --principal operator --action context.approve` |
| 3 | direction, after `vision.write` | approve the vision (`work/aep/vision/product.md`) | the same with `--action vision.approve` |
| 4 | direction, after `roadmap.write` and its validation | approve the roadmap (`work/aep/initiative/roadmap.md`, `work/aep/epic/*.md`) | the same with `--action roadmap.approve` |
| 5 | each story, after tests and review | grant the merge of `impl/` into `mainline/` | `… --case-dir …/IDEA-1.<epic>/children/IDEA-1.<epic>.<story> approve … --capability change.merge --principal operator` |
| 6 | release, after build and acceptance | grant the release (`release/` to `published/`) | `… --case-dir …/IDEA-1.<epic>/children/IDEA-1.<epic>.release approve … --capability release.approve --principal operator` |
| 7 | root terminable | close the idea as `operating` | `sdc-drive --canon … --case-dir …/IDEA-1 close --at "$(date -u …)" --outcome operating` |

Known limit: one `owner.approve` grant admits all three direction approvals (gap 1); the driver
still takes each only through its own `approve --action`.

## What to look at afterwards

- `runs/trial-todo-1/console.txt` and `run.log`: every round and every `sdc-drive` output.
- `llm-calls.jsonl`: one line per call: adapter, model, tokens (`usage`), cost, turns,
  duration, allowed operations, offered tools, tool calls with their decisions, retry or not.
- `llm/NNN-<action>/`: `prompt.md`, `request.json`, `frame.json`, `argv.json`,
  `events.jsonl` (metaharness events), `transcripts/`, `cwd/in` and `cwd/out` (what the agent saw
  and wrote), `apply.log`.
- `store/IDEA-1/`: the case tree (`work/` holds the artifacts; per case `evidence/`,
  `events.jsonl`, `evaluations/`). `sdc-drive --case-dir runs/trial-todo-1/store/IDEA-1 replay`
  must exit 0.
- What to judge: did each check catch a bad draft (records `invalid` followed by a retry), how
  many retries each node needed, whether `spec.author` reached an ESS spec that passes the gate,
  and whether `impl/`'s own tests pass (gap 16: no ESS conformance of the implementation).

## Result (2026-11-21, round 1)

`scripts/run-trial.sh trial-todo-1 "I want to build a to-do list web application"`: exit 3 after
round 1, waiting on the operator in `IDEA-1.intake`.

- LLM calls: 3 (`idea.dissect`, `questions.write`, `questions.answer`), all metaharness,
  `claude-haiku-5-5`, exit 0; together $0.0069, 16.6 s, 2,800 output tokens, 0 tool calls
  denied (Read and Write only).
- Checks: `idea_shape` valid; `dissection_shape` valid (4 items: 1 wish, 1 ambiguity, 2 dark
  spots, every span a substring of the idea); `questions_shape` valid, `questions_settled` open.
- Why it stopped: `questions.write` marked all 3 questions `scope_changing: true` (single user or
  shared; sign-in or not; where items are kept). `questions.answer` correctly answered none of them
  (it only read), so the interview is a person's decision. Defaults: "Single user only", "No
  sign-in; one anonymous user", "Kept in browser storage".
- Next for the operator: edit `runs/trial-todo-1/store/IDEA-1/work/questions.yaml` (per question
  `answer`, `status: accepted` or `changed`, `answered_by: human`), then
  `scripts/run-trial.sh trial-todo-1`. The next stop after that is the context approval.
