---
format: aep.planning-md/3
id: upstream-blocker:aep-actor-option
kind: upstream-blocker
status: open
title: 'AEP: explicit actor option on aep plan artifact commands'
tags:
- aep
revision: 1
---
Waiting on AEP: an explicit actor option on `aep plan artifact` commands that record an actor
(today `move`), so a caller names the principal per command instead of through the inherited
`AEP_ACTOR` environment variable, whose default is `human:$USER` (aep 0.80.0,
`aep plan artifact move --help`). Reported by the trial 3 runner and the Substrate session,
2026-11-21. Not blocking: SDC sets `AEP_ACTOR` until then.
