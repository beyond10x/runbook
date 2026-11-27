---
format: aep.planning-md/3
id: story:a-wake-is-booked-only-when-it-succeeds
kind: story
status: draft
title: A wake is booked only when it succeeds
summary: A refused or failed wake of a managed route books nothing against the daily wake cap
relations:
- informed_by: story:route-wake-behind-a-daily-cap
revision: 1
---
## Outcome

A wake of a managed route counts against the daily wake cap only when the gateway accepted it (a 2xx answer). A wake
the gateway refused or could not serve (a 5xx, `no-capacity`, `target-unavailable`) books nothing, the run's record
says why with the gateway's code, and the run's wait for its route may send the wake again after a pause.

## Context

The same review of a consumer's refresh rounds (2026-11-27), item ranked second by the host's operator session. Story
`route-wake-behind-a-daily-cap` books a wake's figure when the wake is sent, whatever the answer.

## Evidence

- Three wakes on one evening were each answered 503 `target-unavailable`; the gateway logged a failed pod start for
  `no-capacity` three times. Each wake booked the route's wake figure of 1.11 USD: 3.33 of a 3.33 USD daily cap. No
  model was ever started by them.
- Three runs submitted later that evening found "wake cap reached" and ended `stopped_on_route` after waiting 900 s.
- In the review's count this change prevents 3 of 15 hard failures (verified from the figures), and stops booking
  money for wakes that started nothing.

## Acceptance

- A stand-in gateway that answers the first two wakes 503 and the third 202: the run's record shows the wake figure
  booked once, the run starts, and a fourth run of the same day still gets its wake under the cap.
- A wake answered 5xx or `no-capacity` books 0, and the run's record names the gateway's code.
- The tests make no model call, start no pod and use no network beyond loopback.
