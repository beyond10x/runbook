# release-0.1.2: two results explained from the run files

The runs are in `runs/release-0.1.2.*` and `runs/trial-0.1.0-*` of the SDC checkout (not in git).
USD per token kind at the list prices of `experiments/baseline-0.1.0/costs.md`. These prices
reproduce each run's recorded `cost_usd`.

## 1. Strong cost fell from 5.50 to 4.70 USD: reruns, not a change of the tree

Three strong runs per tree, summed over the three, by node class (sessions, cache-write tokens,
USD):

| class | 0.1.0 (4468bfd) | 0.1.2 (72a3eef) | difference USD |
|---|---|---|---|
| implement | 15 sessions, 731,802, 6.290 | 13, 502,260, 4.705 | −1.585 |
| review | 13, 502,862, 2.399 | 13, 375,557, 1.792 | −0.607 |
| writer (`spec.author`, `plan.decompose`) | 11, 222,675, 1.462 | 7, 132,995, 0.958 | −0.504 |
| decider | 39 calls, 479,387, 5.932 | 36, 443,435, 5.568 | −0.364 |
| other (intake, direction) | 23, 158,224, 1.047 | 21, 144,666, 0.950 | −0.097 |
| total (recorded `cost_usd`) | 101 sessions, 2,094,950, 17.129 | 90, 1,598,913, 13.971 | −3.158 |

- **Reruns carry 2.71 of the 3.16 USD.** `rework_cost_usd` per run was 1.067, 2.077 and 1.144 on
  0.1.0, and 0.572, 0.513 and 0.497 on 0.1.2 (`sdc-run-metrics`, the run summaries). Without
  reruns the runs overlap: 4.37, 4.12 and 4.36 USD against 4.12, 4.30 and 3.96.
- **Fewer reruns, counted per node over the three runs:**
  - `change.implement`: 7 reruns on 0.1.0 (5 after a review rejection, 2 arrival takes after a
    finding went to intake), 4 on 0.1.2 (all after a review rejection).
  - `spec.author`: 5 reruns on 0.1.0 (3 `spec.validate:invalid`, 2 after a spec finding), 1 on
    0.1.2 (a spec finding).
  - Review rejections: 5 on 0.1.0 (1, 2, 2), 4 on 0.1.2 (2, 1, 1). The review sent the build back
    about as often.
  - Decider calls: 39 on 0.1.0, 36 on 0.1.2. `release-0.1.2.strong.2` asked 10 decisions instead
    of 13.
- **The reruns that remained were cheaper.** Implement reruns cost 2.951 USD over 7 on 0.1.0
  (0.42 each, median 26 turns) and 0.968 over 4 on 0.1.2 (0.24 each, median 18 turns).
- **The review read less.** Per call, its tools returned a median of 83 KB on 0.1.0 and 54 KB on
  0.1.2, over the same 13 calls. `in/` was of the same size: `in/impl/` 18 to 116 KB on 0.1.2
  against 24 to 103 KB on 0.1.0. The review picks what it reads (a `claude-cli` session with
  `Glob`, `Read` and `Grep`, 10 to 17 turns).
- **No change between the trees makes a first take smaller.** In `llm-node.sh`, 4468bfd..72a3eef
  touches only a rerun's inputs:
  - R9 (`5297ef0`) feeds back a writer run that exited non-zero; no strong run of 0.1.2 had one;
  - R13 (`97024ce`) sends the failing check's own output; no strong run of 0.1.2 had a failed
    check;
  - the rework cap's number in the rework prompt (`9940eed`) is still 2;
  - error codes for replay.

  The review prompt is unchanged (1,907 to 1,912 bytes on both trees). R2 and R3 (`a919898`,
  `306fde9`) route decider questions, and the principal rename (`41d0023`) changes a name. The
  `spec.validate` refusals of 0.1.0 were first takes, which R13 does not reach.
- **Reading**: the separation is real for these six runs, and it comes from 5 rather than 12 reruns of
  `change.implement` and `spec.author`, cheaper remaining reruns and shorter review reads. No prompt or
  input of a first take got smaller. With three runs per tree this is most likely chance
  (inferred). The registered "no separation expected" stands as the expectation for first-take
  cost.

## 2. `release-0.1.2.mixed.1`: `writer_spent` at `change.implement` of IDEA-1.sharedlist.list

What happened (`llm/014` to `016-change.implement`, `run-stopped.json`, `unstick/001-writer_spent`):

- **First take** (`014`, 24 turns, 0.403 USD): built the story. Its own tests passed (13 tests,
  `task check` green).
- **`tests.run` failed**: `ess verify conform run --target interpreted --strict` gave 20
  scenarios, 12 passed and 8 unsupported. Each unsupported one was "binding command has an unmet
  outcome obligation", all for `todo.offline.*`, `todo.sync.ReceiveEdit` and the binding
  `send-offline-edit`, none for the story's commands `todo.list.AddTask` and
  `todo.list.EditTask`.
- **Both reruns** (`015`, 14 turns; `016`, 16 turns; 0.148 USD each):
  - `in/feedback.md` held the failing scenarios and the conformance log, but not what to change.
  - Tool calls: 11 and 13 `Read`, 1 `Glob`, and 1 `Bash` (`find` and `cat` of the inputs), which
    the step refused (it admits no shell). The node did not try to run tests.
  - The refusal did not end the session: each ended with `end_turn`, `terminal_reason:
    completed`.
  - The last assistant text of each said that the failing scenarios belong to other stories, that
    the story's own scenarios and tests pass, and that it left the tree unchanged and raised no
    finding, ending with "done".
  - `out/impl/` equals `in/impl/` in both, so there was no new revision and an empty apply log.
  - `run.unstick` chose stop.

Cause: **the interpreted target checks the specification, not the implementation.**
`tools/sdc-tool.sh` says so at `conform-run`: ESS 0.96.0 has no target that runs an outside
program. The 8 unsupported scenarios are a defect of the spec that `spec.author` wrote, a binding
whose command leaves an outcome obligation unmet. The spec stage did not catch it:
`spec.synthesize` only requires 0 refusals in `ess verify conform synthesize`, and the interpreted
run first happens in the story's `tests.run`. A failed `tests.run` sends the story's writer back,
and the writer cannot change the spec. Of the 18 runs on both trees, only this one has an
unsupported scenario (the others 11 to 23 scenarios, all passed).

Would 0.1.0 have done the same? Yes:

- On 4468bfd, `conform-run` runs the same strict interpreted run and exits 1, and the failed
  `tests.run` goes to `change.implement` in the same way.
- R13 changed only what the rerun reads: merged stderr, no `slot: job` lines, 80 instead of 60
  log lines, an exit code.
- R9 does not apply (the reruns exited 0).
- The one change of 0.1.2 that touches it, the rework cap read from `run-settings.jsonl`, is still
  2.

Classification: **a defect present before 0.1.1**, a spec fault routed to the implementation's
writer, met for the first time by this spec. Model behaviour adds to it: the node saw that the
failures were outside its story but raised no finding against the spec, which would have gone to
intake.

None of the queued stories addresses it:

- `story:implement-one-request` does not, because the writer still cannot change the spec;
- `story:implement-cites-requirements` does not, because its check would pass (the story's
  requirements are built);
- `story:implement-runs-tree-check` does not, because `task check` passes and the failure is in
  the interpreted run.

The fix belongs to the spec stage: run the strict interpreted conformance there, so a failure
returns to `spec.author` (a check beside `spec.synthesize`), or route an interpreted-target failure
of `tests.run` to the spec as a finding.

## Commands

```console
# token kinds per class and run: experiments/baseline-0.1.0/costs.md, Commands
jq '.metrics.rework_cost_usd' experiments/release-0.1.2/runs/<run>.json
jq -r '.nodes[] | select(.node | test("change.implement|change.review|spec.author")) | [.node, .real_takes, .rework_takes, .rework_cost_usd] | @tsv' experiments/release-0.1.2/runs/<run>.json
# the review's reads per call (claude-cli stream-json)
jq -s '[.[] | select(.type=="user") | .message.content[]? | select(.type=="tool_result") | (.content | tostring | length)] | add' runs/<run>/llm/<i>-change.review/events.jsonl
# the two reruns of mixed.1: tools, refusals, end, unchanged tree
jq -r 'select(.event=="tool.requested") | .name' runs/release-0.1.2.mixed.1/llm/015-change.implement/events.jsonl | sort | uniq -c
jq -c 'select(.event=="session.ended") | {stop_reason, terminal_reason, num_turns, permission_denials: (.permission_denials | map(.tool_name))}' runs/release-0.1.2.mixed.1/llm/015-change.implement/events.jsonl
diff -rq runs/release-0.1.2.mixed.1/llm/015-change.implement/cwd/in/impl runs/release-0.1.2.mixed.1/llm/015-change.implement/cwd/out/impl
grep -rhoE '[0-9]+ scenarios: .* unsupported' runs/<run>/store | sort -u
```
