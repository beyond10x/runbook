---
format: aep.planning-md/3
id: epic:decisions-a-person-can-take-cold
kind: epic
status: proposed
title: Decisions a person can take cold
summary: A decision request carries its own context, option consequences and grounds, so a person who has not followed the run can decide.
relations:
- decomposes: initiative:roadmap
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T01:30:34Z", actor: "human:operator", revision: 2}
---
# Epic: decisions-a-person-can-take-cold

## Status note

Finding from trial 2, 2026-11-21. The run stopped for the person's scope answers and the operator
could not decide. His words: "I have absolutely no context for Q1 and Q3 and what this is about. I'm
not able to make any decision based on what you presented."

What he was shown is `runs/trial-2-train/store/IDEA-1/children/IDEA-1.intake/work/questions.yaml`:
each question has `question`, `options`, `default` and `scope_changing`, and nothing else
(`questions.yaml:1-36`). Drafted 2026-11-21 under the work order's "decision contract" item
(AGENTS.md, "Work order"); nothing here changes `driver/` or `scripts/` until the running trial ends.

## Outcome

A person who has not followed the run can take any decision the run stops for, from the one request
shown to them, without asking what it is about. The request names the product and idea, the
dissection item and the words of the idea that raised it, what each option changes in what gets
built and at what rough cost, why the default is recommended and on what grounds, and what happens
to the run after the answer.

## Why Now

Trial 2 stopped on a decision nobody could take. Missing today: what product and idea the question
is about, which dissection item and which words raised it, the consequence and rough cost per
option, why the default is recommended, and what the run does next. The decision contract already
has most of these fields (`ess/domains/decision.yaml`: options with `does`, `cost`, `consequence`;
`recommendation`, `strength`, `grounds`; `docs/decisions.md`), but intake's questions do not fill
them (`driver/examples/sdc/tools/llm-node.sh:136-147`), and neither the UI Decisions drawer
(`ui/web/inbox.js`, `ui/decisions.mjs`) nor the stop message (`scripts/run-trial.sh:156`) shows them.

## Scope

- Interview output carries the idea excerpt, the dissection item, a consequence per option and the
  reason for the default; `questions.check` refuses a scope-changing question without them.
- A program renders a scope-changing question as a decision request in the contract's shape.
- The CLI stop message, the UI Decisions drawer and the Now band's waiting state show that request.
- A bounded LLM "cold read" refuses a request that cannot be restated before a person sees it.
- A KPI counts decisions returned for more context, per run, target 0.

## Out of Scope

- Approvals of manual actions and outcome closes (`docs/decisions.md:205-209`): their requests
  already come from `tools/decision-from-record.sh`; this epic covers scope-changing questions, the
  kind that failed.
- Changing `ess/domains/decision.yaml` or the autonomy rules for who may decide.
- The decider's own prompt (`driver/examples/sdc/tools/decider.sh`).

## Risks

- The files of the trial in progress (`driver/`, `scripts/`) are named by stories 1, 3 and 5; they
  start after the trial ends.
- A cold-read LLM can approve a request that is still unclear to a person; the KPI (story 6) is the
  check on the check.

## Ambiguities

- "What happens next" and the "run, idea, stage" context line are not fields of the contract
  (`ess/domains/decision.yaml` has no such field); stories render them from case facts. Whether they
  become contract fields: requires-stakeholder-input (the operator).
- Where a person returns a request for more context (CLI only, or a UI button too): story 6 takes
  the CLI; a UI act is a follow-up.

## Done When

A fresh trial run that stops for scope answers prints, and the drawer shows, a request a person can
decide from; the cold-read check has refused a deliberately thin request in a recorded case; and
the per-run count of decisions returned for more context is reported (target 0).
