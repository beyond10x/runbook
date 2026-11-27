---
format: aep.planning-md/3
id: story:a-managed-route-books-its-pods-measured-time
kind: story
status: draft
title: A managed route books its pod's measured time
summary: A run on a managed route books its share of the pod's measured time at the gateway's price per hour against the route's daily cap
relations:
- informed_by: story:a-wake-is-booked-only-when-it-succeeds
revision: 1
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
- Not in scope: a price runbook declares for the pod itself; the gateway's own spend ceilings.
