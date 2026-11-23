---
format: aep.planning-md/3
id: story:gateway-under-substrate-confinement
kind: story
status: draft
title: 'Open question: a confined run reaches one gateway address'
summary: At the confinement level substrate a gateway is refused because the bound program has no network; one egress to one address is a change of Loom's confinement and waits for the operator.
relations:
- informed_by: story:model-programs-reach-a-gateway
revision: 1
---
## Open question, not accepted work

`story:model-programs-reach-a-gateway` works at the confinement level `local`. At `substrate` a gateway is
refused by name: the bound program runs in a workspace profile without a network, so an address on loopback
is not reachable from it (read at the pinned Loom, `loom-intake-slice/src/confinement.rs`).

A run that must be confined and must reach a gateway has no way today.

## What it would take

One egress from the confinement to one address the operator names. That is a change of what Loom's
confinement gives a bound program, and of the rule that a bound program gets no network. It is a change of
architecture and of a security boundary: it waits for the operator's decision and for a story on Loom's
side. No work starts from this draft.

## What a guard would have to hold

One address and port, named by the operator and never by a run, a submit, a ticket text or a model-written
file; nothing else reachable; the token as in `story:model-programs-reach-a-gateway`.
