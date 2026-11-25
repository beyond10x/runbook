---
format: aep.planning-md/3
id: story:route-wake-behind-a-daily-cap
kind: story
status: draft
title: The daemon wakes a managed route only under a daily wake cap
revision: 1
---
## Outcome

wake_on_submit, wake_daily_cap_micro_usd and wake_cost_micro_usd on a managed route; shipped off.

## Acceptance

- wake_on_submit true without a cap above 0 is refused at start by name.
- With a cap, one wake is sent for a run on a stopped route and booked; a spent cap sends none and the run record says so.
- A spend-ceiling answer ends the run stopped: route naming the ceiling.

Design: `docs/model-routes.md`, section 3 (D-5). Tests use a stand-in gateway on loopback: no model call, no pod, no network beyond loopback.
