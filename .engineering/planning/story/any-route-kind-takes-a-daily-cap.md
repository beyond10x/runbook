---
format: aep.planning-md/3
id: story:any-route-kind-takes-a-daily-cap
kind: story
status: draft
title: Any route kind takes a daily spend or call cap, booked from the relay's cost
summary: daily_cap_micro_usd and daily_max_llm_calls on a managed, hosted or local route; the relay refuses a call past the day's figure
tags:
- consumer-asks-2026-11-27-agents
relations:
- informed_by: story:daemon-caps-are-keys-of-one-file
- informed_by: story:route-wake-behind-a-daily-cap
- informed_by: story:a-managed-route-books-its-pods-measured-time
revision: 1
---
## Outcome

Any route, whatever its kind (managed, hosted, local), can name a daily cap: `daily_cap_micro_usd`, and optionally
`daily_max_llm_calls`. The relay books every call's cost against the route's day, as it books the run's ledger. A call
that would pass the route's daily cap is refused before it is sent, and the run ends with a cap outcome that names
the route and the key. A consumer can then offer a hosted route on a page without an open-ended spend.

## Context

Gap G4 of a consumer repository that dispatches agents through the daemon (2026-11-27, relayed by the host's
operator session).

- The only per-day figure in the daemon is a managed route's wake cap, `Wake.daily_cap_micro_usd`
  (`runbook/src/routes.rs:95-99`). The wake keys are refused on a hosted or local route: "a key of a managed route
  only" (`runbook/src/routes.rs:289-293`, keys `runbook/src/routes.rs:45`).
- The relay knows each call's cost: `cost_of` (tokens times the route's declared prices, `runbook/src/relay.rs:593-606`)
  and `charge` (a failed call 0, an unknown cost the reservation, `runbook/src/relay.rs:613-621`). `Call::line` writes
  the ledger line with `route` and `micro_usd` (`runbook/src/relay.rs:732-765`).
- The ledger has no day key (`runbook/src/ledger.rs:1-10`). Spend is per run and per ledger, not per route and day.
- `story:daemon-caps-are-keys-of-one-file` (active, not built: no `day_cap` in `runbook/src/`) plans daemon-wide day
  figures `day_cap_micro_usd` and `day_max_llm_calls`. It has no figure per route.
- A route with no declared price books its reservation for a call whose cost is unknown
  (`runbook/src/delegation.rs:54-56`, `docs/model-routes.md` section 5). A hosted subscription route may declare no
  price, so its daily figure would count reservations. The unit says how such a route is counted (calls, or a declared
  price per call).

## What this story does

1. A route table in `runbook.toml` takes `daily_cap_micro_usd` and `daily_max_llm_calls` for every kind. A value of 0,
   or one that is not a whole number, is refused at start by name. The managed wake cap stays a separate key.
2. The relay keeps the route's day figures (spend booked, calls sent) under the ledger's lock. The day is the UTC
   date of the daemon's clock, as the wake's day is (`runbook/src/commands.rs:2509`). The figures are rebuilt from the
   ledger at start.
3. Before a call is sent, the relay checks the route's day figures with the call's reservation. A call that would pass
   is answered `429 cap-reached` with `cap_key` `route.<name>.daily_cap_micro_usd` (or `daily_max_llm_calls`), and
   nothing is sent. The run ends as a run at a cap ends.
4. The view `routes` answers `daily_booked_micro_usd`, `daily_cap_micro_usd`, `daily_llm_calls` and
   `daily_max_llm_calls` (null when not declared).

## Acceptance

With the daemon's test clock and a stand-in gateway on loopback that answers a fixed usage:

- A hosted route with `daily_cap_micro_usd = 3000` and calls that cost 1000 each: the third call is sent, the fourth
  is refused with `cap_key` naming the route's key, and the stand-in saw 3 calls.
- After 00:00 UTC on the test clock a call on the same route is sent.
- A local route with `daily_max_llm_calls = 2`: the third call of the day is refused, with the same view and outcome.
- Two runs on one route share its day figure.
- A daemon started again the same day keeps the figure.
- `wake_daily_cap_micro_usd` on a hosted route is still refused. `daily_cap_micro_usd` on it is taken.
- Each case runs under 5 s. No model call, no network beyond loopback.

## Evidence

- The consumer's design, section 8, item G4: a hosted route (a gateway's subscription models) has no daily cap on the
  daemon, so the consumer cannot offer it on a page without an open-ended spend.

## Relations

- Informed by `story:daemon-caps-are-keys-of-one-file` (the daemon-wide day figures; one ledger with a day key serves
  both), `story:route-wake-behind-a-daily-cap` and `story:a-managed-route-books-its-pods-measured-time` (what a managed
  route books per day).
