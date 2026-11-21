---
format: aep.planning-md/3
id: upstream-blocker:loom-standing-trees
kind: upstream-blocker
status: open
title: 'Loom: standing trees rest without a slot and wake on revision or expiry'
tags:
- loom
- orchestration
relations:
- blocks: story:loom-concurrent-children
revision: 1
---
Capability requested from the Loom session: a quiescent tree holds no run slot and wakes on an
artifact revision or at the earliest `max_age` expiry; per-action run caps count per subject
revision, not per case life; the governor never completes a case that has no legitimate outcome
(`docs/orchestration.md` § 1.2, § 3).
