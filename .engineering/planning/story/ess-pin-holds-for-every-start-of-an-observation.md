---
format: aep.planning-md/3
id: story:ess-pin-holds-for-every-start-of-an-observation
kind: story
status: draft
title: Every start of the pinned ess binary inside an observation runs the bytes that were compared
summary: 'Security pass 3 of the driver test-budget unit, P10, by reading: the digest is taken once per observation and the binary is started again by path after the product''s build step.'
relations:
- informed_by: story:driver-test-budget
revision: 1
---
## Outcome

Every start of the pinned `ess` binary inside one conformance observation runs the bytes whose digest was
compared with the pin.

## Source

Security review pass 3 of `story:driver-test-budget` (2026-11-23), finding P10, by reading, not measured, in
code whose origin the reviewer left undecided (`driver/src/conform.rs`): the digest of the binary is taken
once at the start of an observation; the binary is started again by path later in the same observation,
after the build step of the product ran. `docs/conformance.md` names the missing confinement of a build
step as a limit.

## Acceptance

- First a test that shows whether the limit is reachable: a build step that changes the file at the path of
  the binary between the digest and a later start. The story is rejected with that test's result when the
  later start refuses the changed file.
- If it is reachable: every start inside an observation runs bytes that were compared with the pin (the same
  open file, or a digest before each start), and a changed file ends the observation with the pin's refusal.
- No cache, file, environment variable, size or mtime check and no in-process global stands in for the hash.
- The driver test step stays within 120 s (the digest of 113 MB costs time: measure the step before and
  after and report both).
