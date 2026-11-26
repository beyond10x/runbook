---
format: aep.planning-md/3
id: story:a-failed-call-that-ran-upstream-is-booked
kind: story
status: draft
title: A failed call that ran upstream is booked and counts against the spend cap
summary: charge books 0 for a 4xx/5xx even when the upstream ran; since a run goes on past failed calls, a client that retries is invisible to the spend cap.
tags:
- adversary-wave4
relations:
- informed_by: story:a-recovered-call-does-not-fail-the-run
revision: 1
---
## What is missing

A failed call that ran upstream books nothing. The spend cap does not see it.

Read on `wave/runs-green` at `c596429a6`:

- `charge` in `runbook/src/relay.rs` books 0 with `cost_known: true` for every failed call except `answer-broken`. That includes a 4xx or 5xx that the gateway sent after the upstream ran, such as a 502 `upstream-failed` after an upstream timeout. It also includes a stream that the upstream ended with an error event after `message_start` had reported usage.
- Before wave 4, a failed call usually stopped the run, so its cost was bounded by one call. Since story `a-recovered-call-does-not-fail-the-run`, a run goes on past failed calls. A client that retries many times can now spend upstream with every try, and the run's spend cap counts none of it.

Found by the wave 4 adversary (finding J2). The gap existed before the wave; the wave made it reachable many times in one run.

## Outcome

1. A failed call that the gateway reports as having reached upstream books the usage that the answer reports, at the route's prices. When the answer reports no usage, it books the call's reservation with `cost_known: false`.
2. The booked amount counts against the run's spend cap like an answered call: a run that retries past the cap is stopped with `spend-ceiling`.
3. A call that the gateway refused before upstream (a route refusal, a credential refusal, a gateway-side 4xx) and a call that never got an answer still book 0, as they do today.

## Acceptance

- A call answered 502 `upstream-failed` that the gateway marks as having reached upstream, with a usage object, is booked at that usage's cost. The booked line carries its `cause` and `code`.
- The same call with no usage is booked at its reservation with `cost_known: false`.
- A stream that ends in an error event after `message_start` reported usage is booked at that usage.
- A stand-in client that retries failed calls until the run's spend ceiling is reached is stopped with `spend-ceiling`. A test proves it over loopback with a stand-in gateway, and makes no model call.
- A gateway refusal that never reached upstream still books 0 (the current unit cases of `charge` stay green).

## Open question

How the gateway says that a call reached upstream: a response header, a field of its error body, or the code (`upstream-failed`, `upstream-timeout`) read as a list. Settle this against the gateway's error contract before implementation.
