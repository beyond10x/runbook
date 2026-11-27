---
format: aep.planning-md/3
id: story:a-route-limits-its-calls-in-flight
kind: story
status: draft
title: A route limits its calls in flight
summary: The relay sends at most a route's limit of calls at once; the others wait in a queue
relations:
- informed_by: story:daemon-relays-and-books-model-calls
revision: 1
---
## Outcome

A route can name how many model calls it takes in flight at once. The relay sends at most that many to the route's
gateway; the others wait in a queue in the order they came, and a call's time at the gateway starts when it is sent.

## Context

The same review of a consumer's refresh rounds (2026-11-27), item ranked third by the host's operator session.
Nothing limits the calls in flight per route today: the gateway has one global bound, and the daemon's `serial_key`
runs submits one after another only when a submit names it.

## Evidence

- 8 investigate runs and 1 report run sent 9 first calls within 2 minutes, and their second calls while the first ones
  still ran, to a managed route whose model serves 8 sequences at once. Gateway durations were 155 s to 665 s; 15 of
  16 judge calls ended on their client's 270 s timeout.
- In the review's count this change prevents 2 of 15 hard failures (inferred) and 5 degraded runs.

## Acceptance

- A stand-in gateway records how many calls it holds at once. 8 runs submitted at once on a route that takes 2 calls
  in flight: the gateway never holds more than 2, and every run ends with its protocol's outcome.
- A route that names no limit keeps today's behaviour.
- A waiting call whose caller hangs up leaves the queue and is never sent.
- The tests make no model call and use no network beyond loopback.

## Open

- Whether the limit is the daemon's alone or the gateway's too (it has only a global bound), and whether time spent
  in the queue counts against a run's wall cap: design questions for the operator before code.
