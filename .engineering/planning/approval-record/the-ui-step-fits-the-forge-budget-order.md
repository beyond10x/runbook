---
format: aep.planning-md/3
id: approval-record:the-ui-step-fits-the-forge-budget-order
kind: approval-record
status: approved
title: The UI step's forge budget is a story worked now (Tests stay fast; the operator 2026-11-24 11:17 CET)
relations:
- decides: story:the-ui-step-fits-the-forge-budget
revision: 3
transitions:
- {from: "draft", to: "approved", at: "2026-11-24T18:28:41Z", actor: "human:operator", revision: 2, executor: "agent:runbook"}
---
# Approval: the UI step's forge budget is worked as a story now

## Decision

`story:the-ui-step-fits-the-forge-budget` moves from `draft` to `proposed` and from `proposed` to `active` on two
words, cited here because a move carries no text of its own:

- `../AGENTS.md` § "Tests stay fast": "A step over the budget is a defect. It gets a story and is fixed before the
  next feature. Raising a timeout, adding shards or waiting longer is not a fix."
- the operator, 2026-11-24 11:17 CET: "The local gate equals CI: put them into the CI gate, or write in the
  repository what the runner does differently and why".

The CI-parity unit (`impl/the-forge-runs-the-driver-and-ui-tests`, head `07c03f42`) took the second branch of that
order for `test:ui` (the `OUTSIDE` line of `task check:ci-paths`, `docs/forge-gate.md`): at the forge's CPU count the
UI run does not end within 120 s. That makes `test:ui` a step over the budget, and the first rule makes it a story
that is fixed before the next feature. The story is accepted on creation under the standing rule of this store
(`approval-record:operator-standing-acceptance-2026-11-21`; `AGENTS.md`, "Sessions").

## What it covers

The two moves of this story on 2026-11-24, both carrying this record's id as their correlation. It covers no move to
`implemented`: that rung needs a `test_result` record.
