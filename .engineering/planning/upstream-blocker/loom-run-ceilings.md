---
format: aep.planning-md/3
id: upstream-blocker:loom-run-ceilings
kind: upstream-blocker
status: open
title: 'Loom: engine run ceilings and free slots readable by an admission program'
tags:
- loom
- orchestration
relations:
- blocks: story:loom-concurrent-children
revision: 1
---
Capability requested from the Loom session: the configured run ceilings (a binding may only lower
them) and the free run slots are readable by a program, so `wave.admit` rule 3 can count them
(`docs/orchestration.md` § 2.4 rule 3; `<loom-plan>/story/action-limits-enforced.md:18-27`).
