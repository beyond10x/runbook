---
format: aep.planning-md/3
id: upstream-blocker:loom-decider-per-class
kind: upstream-blocker
status: open
title: 'Loom: decider configuration per decision class, eligible principals per side'
summary: The host passes a decider configuration per class and eligible principals per side to Loom's decider executor port.
tags:
- loom
- requester
relations:
- blocks: story:requester-decider-loom
revision: 1
---
Capability requested from the Loom session: a decider configuration per decision class (principal,
input artifacts, prompt) and eligible principals per side, handed in by the host, so the requester
agent and the delivering-side decider are two configurations of the decider executor port already
requested (`docs/decisions.md` "To the Loom session" items 1 to 3; `docs/requester.md` § 7).
