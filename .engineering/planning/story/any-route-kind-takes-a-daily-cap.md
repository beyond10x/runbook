---
format: aep.planning-md/3
id: story:any-route-kind-takes-a-daily-cap
kind: story
status: active
title: Any route kind takes a daily spend or call cap, booked from the relay's cost
summary: daily_cap_micro_usd and daily_max_llm_calls on a managed, hosted or local route; the relay refuses a call past the day's figure
tags:
- consumer-asks-2026-11-27-agents
relations:
- informed_by: story:daemon-caps-are-keys-of-one-file
- informed_by: story:route-wake-behind-a-daily-cap
- informed_by: story:a-managed-route-books-its-pods-measured-time
revision: 5
transitions:
- {from: "draft", to: "proposed", at: "2026-11-27T23:21:44Z", actor: "agent:runbook", revision: 2}
- {from: "proposed", to: "active", at: "2026-11-27T23:21:44Z", actor: "agent:runbook", revision: 3}
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

## Rules taken

Taken by the unit that built the story (2026-11-28), each reversible by editing this story:

- The spend rule holds the booked figure, not a reservation. A call is sent while the route's booked spend of the day
  is below `daily_cap_micro_usd`; its cost is booked into the day when its answer has ended, before its caller sees
  that end. So no call is sent once the booked figure has reached the cap, and the third of three calls of 1000 under a
  cap of 3000 goes out. The last call sent below the cap, with any call in flight beside it, may take the figure above
  the cap by its own cost, which is not known before its answer ends. A reservation rule would refuse a call whenever
  the step's reservation passes what is left, which the Acceptance does not want.
- The calls rule: a call is counted as it is admitted, so `daily_llm_calls` never passes `daily_max_llm_calls`. The cap
  of calls is read before the spend cap. The route's figures are read before the run's own caps, and the call is
  counted on the route only after those took it: a call its run refuses never holds a count of the route's day while
  its run decides, so it takes no other run's last call. A call the route refuses at that count (another run took the
  last call in between) is given back to the run's caps.
- A route with no declared price is counted by its calls. Its calls book their reservation (cost unknown), so
  `daily_cap_micro_usd` on a route that declares neither `price_in_micro_usd_per_mtok` nor
  `price_out_micro_usd_per_mtok` is refused at start by name, and `daily_max_llm_calls` caps it. Its
  `daily_booked_micro_usd` still answers what its lines booked.
- An answer with no usage at all is a call of unknown cost on a priced route too, the same as a call on a route with no
  price: it books its reservation (the coordinator's decision D17, 2026-11-28), in the run's ledger and in the route's
  day alike, so the two never differ and a daemon started again rebuilds the same figure. Booking 0 for it would leave
  `daily_cap_micro_usd` unfilled against a gateway that reports no usage.
- The day key: the relay writes the day a call counts on (the UTC date of the daemon's clock as the call was admitted)
  into its ledger line and its `runbook.model-call` as `day`. A daemon started again rebuilds the figures from the
  model-call lines of every ledger under its `ledgers/`. `story:daemon-caps-are-keys-of-one-file` can read the same key
  for its daemon-wide day figures. A call in flight when the daemon was killed has no line and is not counted; a ledger
  that cannot be read at start refuses every call on a route with a daily cap.
- The run at a route's cap ends as a run at a cap ends: `stopped: llm calls` for `daily_max_llm_calls`,
  `stopped: spend cap` for `daily_cap_micro_usd`, after the step that made the call, at once for a call of its
  decider (not in a wait for a person) and for a classify call of a delegation (`Ended`, not `FailedToStart`); its
  `runbook.cap-reached` carries `route` and `cap_key` beside `cap` and `detail`.
- The view `routes` answers `daily_booked_micro_usd` and `daily_llm_calls` for every route, `daily_cap_micro_usd` and
  `daily_max_llm_calls` where declared (absent, read as null, otherwise), from the same figures the relay admits a call
  by.
