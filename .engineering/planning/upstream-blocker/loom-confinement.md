---
format: aep.planning-md/3
id: upstream-blocker:loom-confinement
kind: upstream-blocker
status: open
title: 'Loom and Substrate: bound commands and child launches run confined'
tags:
- conductor-succession
- loom
- substrate
relations:
- blocks: story:confined-bound-commands
revision: 1
---
Waiting on Loom (with Substrate adapters): bound commands run confined, confinement opened from the
engine configuration, child launches through the confinement adapters (`docs/conductor-comparison.md`
§ 7 gap 7; `docs/layers.md` § 5 items 8-10). Planned there; not a new request.
