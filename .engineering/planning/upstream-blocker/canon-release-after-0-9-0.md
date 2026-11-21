---
format: aep.planning-md/3
id: upstream-blocker:canon-release-after-0-9-0
kind: upstream-blocker
status: open
title: 'Canon: release after 0.9.0 with slice-wise bound and --import'
tags:
- canon
relations:
- blocks: story:canon-slice-bound-uptake
- blocks: story:protocol-imports-uptake
revision: 1
---
Waiting on Canon: the first release after 0.9.0 carrying wave 2 (closed on its branch, canon
commit `c76fb90`, gate at `ac46b8c`, 968 tests): `story:check-subject-slices` (the 65,536 bound
held per slice in `canon check`), `story:protocol-imports` (`--import`, pinned by revision and
sha256 of the compiled IR) and the evaluation-cost fix. Accepted by Canon; not a new request.
