---
format: aep.planning-md/3
id: epic:requester-side-and-goals-context
kind: epic
status: proposed
title: Requester side and goals context
summary: The requester's goals document drives the answers to the delivering side's questions, by a requester agent or the requester person.
tags:
- requester
relations:
- decomposes: initiative:roadmap
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T01:30:35Z", actor: "human:operator", revision: 2}
---
# Epic: requester-side-and-goals-context

## Status note

Operator requirement (the operator, 2026-11-21), after trial 2 stopped on two scope questions he had no
context for: "Somehow we do need to inject company goals, or higher level context, we could call
it, which would, for example, drive these interview decisions. So the implementing or planning side
would find these ambiguities, and then the requester, in that case also played by an agent — or not
really played, it's a valid real-life use case — would then answer these questions based on its own
values and this maybe higher level context."

Design: `docs/requester.md`. Drafted 2026-11-21; stories that touch `driver/`, `scripts/` or
`protocol/` start after the running trial ends.

## Outcome

A run receives the requester's goals document (`sdc.goals/1`: goals, principles, constraints,
preferences in company, product and request layers) as a revisioned input, and the requester side
answers the delivering side's questions from it: a requester agent (`decider:requester`) in the
decider cells of the requester-side classes, the requester person in the person cells, both seeing
which goal entry an answer rests on. Answers a goal states are not asked; an answer resting on a
changed entry is set aside and answered again.

## Why Now

Trial 2 stopped for scope answers the operator could not give from what he was shown
(`epic:decisions-a-person-can-take-cold`). The decider today receives idea, interview, requirements,
context and the run's own vision and roadmap, and nothing about the requester's goals
(`driver/examples/sdc/tools/decider.sh:127-133`), so at A3 and up it answers scope questions with
no values to answer from, and every eval run (A5) does the same.

## Scope

- The goals format, its check and resolution of layers.
- Goals as an input artifact of the root case, copied into intake and direction.
- The requester agent as a decider configuration with its own inputs, and a `side` per decision class.
- Grounds citing goal entries in the decision contract and the grounding check; free answers.
- Interview answers derived from goals, and a check of their basis.
- Consistency rules and stage findings against constraints.
- The eval persona (goals plus hidden truth) and KPIs G1 to G6.
- The UI showing which goal an answer rests on.
- The Loom port of the requester decider configuration (blocked upstream).

## Out of Scope

- Changing the autonomy table's holders (question 1 in `docs/requester.md` § 10 may change one cell).
- Turning a person's answer into a new goal entry ("remember as a preference"): a follow-up.
- Writing the operator's real goals (question 2).

## Risks

- A requester agent with goals but no ground answers by guesswork; G3 and G2 (uncovered) show it.
- `ess/domains/decision.yaml` is being edited by another session; the contract story coordinates.

## Ambiguities

- Whether a goal-stated scope answer may be derived without the person below A3:
  requires-stakeholder-input (the operator, `docs/requester.md` § 10 question 1).

## Done When

A stub trial-2 run with the example goals at A3 stops at intake for no person, every scope answer
cites a goal entry that passes the grounding check, a changed entry sets its answers aside, and an
eval persona item reports G1 to G6.
