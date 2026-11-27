---
format: aep.planning-md/3
id: story:a-managed-route-books-its-pods-measured-time
kind: story
status: active
title: A managed route books its pod's measured time
summary: A run on a managed route books its share of the pod's measured time at the gateway's price per hour against the route's daily cap
relations:
- informed_by: story:a-wake-is-booked-only-when-it-succeeds
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-11-27T13:46:47Z", actor: "agent:runbook", revision: 2}
- {from: "proposed", to: "active", at: "2026-11-27T13:46:48Z", actor: "agent:runbook", revision: 3}
---
## Outcome

A run on a managed route books the time the route's pod ran for it, not only the wake. The gateway's price per hour of
the pod, from the moment the wake reports the pod ready to the moment the pod stops, is split over the runs that used
the pod in that time, and each run's share is booked against the route's daily cap beside the wake.

## Context

The same review of a consumer's refresh rounds (2026-11-27), gap 6, second half (the first half is story
`a-wake-is-booked-only-when-it-succeeds`). `docs/model-routes.md`, section 5 under D-3, says a managed pod's hourly
cost is the gateway's (its daily and total spend ceilings) and that runbook books wakes only (section 3): the daily cap
of a managed route counts wake figures, and the hours a pod runs after its wake count nowhere in runbook.

## Evidence

- A managed route's daily cap is held against wake figures alone: a route whose pod ran for hours after one wake
  shows the cost of that one wake in runbook's ledger.
- `docs/model-routes.md`, section 5: "A managed pod's hourly cost is the gateway's (its daily and total spend
  ceilings); Runbook books wakes only (section 3)."

## Decision

The operator's pick A, 2026-11-27 12:35 CET: book each run's measured pod time against the route's daily cap,
replacing `docs/model-routes.md` D-3 "books wakes only".

## Acceptance

- A run on a managed route whose pod was ready from t0 to its stop at t1, used by that run alone, books
  (t1 - t0) times the gateway's price per hour against the route's daily cap, beside the wake's figure.
- Two runs that used the same pod in that time each book their share of it; the shares add up to the pod's measured
  time at its price.
- A pod that never became ready books no time.
- The daily cap counts wakes and pod time together: a run that would pass it waits for its route as a run past the
  wake cap does today.
- `docs/model-routes.md` D-3 and section 5 say what is booked.
- The tests make no model call, use a stand-in gateway and no network beyond loopback.

## Scope

- Built in one unit with story `a-wake-is-booked-only-when-it-succeeds`: the wake's booking and the pod's time read
  the same answers of the gateway.
- Where the price comes from (found while building, 2026-11-27): the gateway serves its `price_per_hour_usd` on no
  route a client reads (its wake, status and model-list answers carry none; only its dashboard's account panel holds a
  pod's `cost_per_hour_usd`, and only for a provider whose key is a keychain reference). So the route repeats the
  figure in `runbook.toml` as `price_per_hour_micro_usd`, a managed route's key; the unit's brief named that way for a
  gateway that does not expose the price. This replaces the earlier line "not in scope: a price runbook declares for
  the pod itself". A gateway route that serves a model's price per hour would let the key go.
- Where the pod's stop comes from: the gateway sends no stop event; the daemon reads the route's status every 5 s of
  its clock while a started run takes the route, and the first read of the pod as not ready ends the runs' use.
- Not in scope: the gateway's own spend ceilings; stopping a pod or a run already on it when the cap is reached.
