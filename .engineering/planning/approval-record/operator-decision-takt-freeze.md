---
format: aep.planning-md/3
id: approval-record:operator-decision-takt-freeze
kind: approval-record
status: draft
title: 'Operator decision: takt frozen, forge, git and validators ported (decision A, 2026-11-21)'
tags:
- operator-decision
- session-sdc
- standing
relations:
- decides: epic:conductor-succession
revision: 1
---
# Operator decision: takt is frozen and harvested (decision A)

- Decision, verbatim: "decision A", on the options of `docs/takt-comparison.md` § 5 answer 4.
- By: the operator. Session: `sdc`. Date: 2026-11-21.
- Meaning: takt is frozen; its forge, git and validator code is ported into Loom and SDC (path A).
- First: a probe runs `sdc.story` once through takt; its result goes to `docs/takt-probe.md`.
- Standing: it holds for later sessions in this repository until the operator revokes it.
- Planned under `epic:conductor-succession` ("Source to port" sections of the stories takt's code
  serves; `story:takt-freeze-notice`). New drafts stay drafts until the operator approves them.
