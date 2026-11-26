---
format: aep.planning-md/3
id: story:one-replay-word-for-a-composed-run
kind: story
status: active
title: One replay word for a composed run
relations:
- informed_by: story:program-calls-are-recorded-and-replayed
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-11-25T17:14:14Z", actor: "agent:runbook", revision: 2}
- {from: "proposed", to: "active", at: "2026-11-25T17:14:15Z", actor: "agent:runbook", revision: 3}
---
## Outcome

A composer run given one `replay=<composer run>` makes each child case answer from that run's recording of the same
child id; a child with no recording fails loudly, naming the child.

## Source

A consumer, 2026-11-25 (relayed by the host operator; accepted under the operator's delegation). Each stage of a
composer reads its own `replay=<run>` naming a run of its own kind, while its children share one intake file, so a
whole round cannot be replayed from one recording.

## Acceptance

- Record a stub composer run; replay it with the one word: calls counted 0 against the stub sources, outputs byte-equal
  to the recording. No model.
- A child whose id has no recording in the named run ends refused, naming the child and the run.

## Related

- `story:program-calls-are-recorded-and-replayed` (active) records and replays one program's calls; this is its
  composer-level word.
