# R-01: first round on the SDC run inspector

The UI: `ui/web/` (dependency-free, served by `ui/server.mjs`). One theme (dark). Adapter:
`ux/adapter.mjs`. Gating viewports 1600x1000 and 1366x768; measured only 1280x720 and 1920x1080.

## Trial

No person. A driver bot (`test/fixture-run.mjs`) plays each run as the driver and its adapters would
write it, at 3x speed; the screen-only user (`ux/specs/loop.spec.mjs`) works the page with only
what it can see. L4, L5, L6 and L8 skipped (paid or people). No rrweb recordings: the screenshots
in `shots/` stand in (`<viewport>-<scenario>-0.png` at the scenario's truth, `-diff`, `-decided`,
`-end` after the tasks).

## Scenario cards (the truth of each)

| card | anchor | what runs now | who is waited on | last change | completion | tasks |
|---|---|---|---|---|---|---|
| agent | yes | change.implement, LLM agent, claude-haiku-5-5 | nobody | a code file | running | name the action and its executor; open the diff of the last change |
| two | | spec.author (LLM agent) and plan.cover (Program) | nobody | spec and planning files | running | name both actions and their executors |
| decider | | nothing | Decider (claude-haiku-5-5) on roadmap.approve | roadmap revised | waiting | name who decides, for which action |
| person | yes | nothing | Person (operator) on context.approve | context written | waiting on a person | find why the run waits and take the decision |
| closed | | nothing | nobody | the last revision | closed, outcome operating | read the outcome and the total cost |
| changes | rotating | spec.author and plan.cover | nobody | `broken.md created` (planning, not parsed) | running | open the last change and read what it means; show only the specification changes |

Always visible (hard gate 2): `#now`, `#kpis`, `#source`, `#decbtn`. Guidance cue: the Now band's
waiting card and its Decide control. Scroll exceptions: decision D-001 in `ux/baseline.json`.

## Operator notes

- Harness defects fixed before this measurement (the earlier attempts are not kept): the tests ran
  in serial mode, so one failure skipped the rest; the screen-only user read the truth after 7 s of
  sampling, when the run had moved on; a control name matched as a substring ("Decide" matched a KPI
  tile that reads "decider"); text cut by an ellipsis counted as read.
- The `closed` card is the recorded run `runs/trial-todo-1`; the others are played per test.
