---
format: aep.planning-md/3
id: story:the-routes-view-row-answers-start-wait-seconds
kind: story
status: draft
title: The routes view row answers start_wait_seconds, as the catalog's route does
summary: start_wait_seconds on the row of the view routes and of list_routes when declared; the catalog takes it from the row
tags:
- consumer-asks-2026-11-28-pin-0-15
relations:
- informed_by: story:the-routes-view-answers-the-days-wake-figures
revision: 1
---
## Outcome

A row of the view `routes` (and of the MCP tool `list_routes`) answers `start_wait_seconds` when the daemon's file
declares it, as the catalog's route row already does. A client reads a route's wait, which bounds a run's wait and the
join of a wake, from the view it reads the day's figures from.

## Context

A consumer repository that moved its pin to runbook 0.15.0 (2026-11-28, relayed by the host's operator session). It
reads the view `routes` for the day's figures and has to ask the catalog as well, only for the wait.

What runbook has on origin/main (`df6cd197`):

- The row of `routes` is built by `rows_of` (`runbook/src/routes.rs:753-795`): `route`, `gateway`, `alias`, `kind`,
  `wires`, `status`, `phase`, `max_model_len`, `tool_calling`, `read_at`. `Daemon::routes`
  (`runbook/src/commands.rs:2598-2617`) adds the day's figures. Neither adds `start_wait_seconds`.
- The catalog adds it to each route it takes from `Daemon::routes`, with the two prices
  (`Daemon::catalog`, `runbook/src/commands.rs:2648-2671`, the key at `runbook/src/commands.rs:2659`;
  `docs/daemon-api.md:704-709`).
- The route's value: `Route.start_wait_seconds` (`runbook/src/routes.rs:163`, read at `runbook/src/routes.rs:343`).
  It bounds a run's wait for its route (`route_wait`, `runbook/src/commands.rs:1188-1204`) and the join of a wake
  (`wake_sent_within` with the route's wait, `runbook/src/commands.rs:1108` and `runbook/src/commands.rs:3019`).
- ESS: `sdc.daemon.CatalogRoute` has `start_wait_seconds` (`ess/domains/daemon.yaml:275`); `sdc.daemon.ModelRoute`
  (`ess/domains/daemon.yaml:453`) and the view `sdc.daemon.Routes` (`ess/domains/daemon.yaml:1888-1918`) do not.
  `docs/daemon-api.md:168` lists the row's members without it.

## What this story does

1. ESS first: `start_wait_seconds`, `Optional<Integer>`, on `sdc.daemon.ModelRoute` and the view `sdc.daemon.Routes`,
   placed before the day's figures; `sdc.daemon.CatalogRoute` keeps it.
2. The row of `routes` answers it when declared (in `rows_of` or `Daemon::routes`); the catalog takes it from the row
   and no longer adds it itself, so the two never differ. The prices stay the catalog's: a row of `routes` names no
   price (`ess/domains/daemon.yaml:1892`, "No address, token, token file or price is in a row").
3. `docs/daemon-api.md:168` and "The catalog" name the member.

## Acceptance

With the daemon's test clock and a stand-in gateway on loopback (`runbook/tests/daemon/catalog.rs` or
`runbook/tests/daemon/route_wake.rs`):

- A route with `start_wait_seconds = 600`: its row of `routes`, its route of `catalog` and its row of `list_routes`
  answer `start_wait_seconds: 600`; each key appears once in the catalog's route.
- A route without it: none of the three answers the member.
- Hosted, local and managed routes answer it alike.
- The OpenAPI document carries the member on the row of `routes` (generated from ESS).
- Each case runs under 5 s. No model call, no money, no network beyond loopback.

## Relations

- Informed by `story:the-routes-view-answers-the-days-wake-figures` (the row the figures went into; it names the
  catalog's `start_wait_seconds`).
- Not the same change of the row as the three other stories of these consumer asks: this one changes the gateway's
  part of the row (`rows_of` or `Daemon::routes`) and the catalog, not `wake_figures`.
  `story:the-routes-row-says-whether-a-new-run-would-join-a-wake` reads the same wait; either may land first.
