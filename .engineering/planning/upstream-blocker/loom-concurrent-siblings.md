---
format: aep.planning-md/3
id: upstream-blocker:loom-concurrent-siblings
kind: upstream-blocker
status: open
title: 'Loom: drive admitted siblings concurrently, one writer per case'
tags:
- loom
- orchestration
relations:
- blocks: story:loom-concurrent-children
revision: 1
---
Capability requested from the Loom session: `tree run` drives the admitted siblings of one parent
concurrently, one run slot per running case (extending the tree daemon's slot from "one per tree"),
and one writer per case instead of one per store. Loom's planned `tree run` is sequential
(`docs/orchestration.md` § 1.1, § 2.4; `<loom-plan>/story/tree-run-command.md:61-81`,
`tree-daemon.md:6-7`, `:23-27`).
