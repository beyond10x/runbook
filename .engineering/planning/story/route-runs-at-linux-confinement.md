---
format: aep.planning-md/3
id: story:route-runs-at-linux-confinement
kind: story
status: draft
title: Route runs at linux confinement
revision: 1
---
## Outcome

A run on a route at `linux` confinement finds its programs and reaches the daemon's relay on loopback, as it does at
`local`.

## Found

At `linux` confinement a delegation on a route fails at once: its program is not found,
`env: 'sdc-node': No such file or directory`, exit 127. This happens from an installed prefix and from a checkout. Route
runs work only at `local` today.

## Scope

- The confined run's `PATH` holds the directory of `sdc-node` and the other programs a runbook's bindings name, from an
  installed prefix and from a checkout.
- The confined run reaches the relay's loopback address named in its route file.
- A case at `linux` confinement: a delegation on a route ends with its answer, and its model call is booked.

## Acceptance

To be written when the story is taken up.
