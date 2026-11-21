---
format: aep.planning-md/3
id: upstream-blocker:loom-child-cases
kind: upstream-blocker
status: open
title: 'Loom: child cases (case_open, case_outcome)'
tags:
- loom
- orchestration
relations:
- blocks: story:loom-concurrent-children
revision: 1
---
Capability requested from the Loom session: open child cases and write their outcomes back as
`case_outcome` records. Loom pins Canon 0.8.0; `case_open` arrived in 0.9.0; no
`case_open`/`case_outcome` in `crates/` (`docs/orchestration.md` § 1.1; `docs/boundary.md`
item 6). Planned as Loom `story:child-case-runtime`.
