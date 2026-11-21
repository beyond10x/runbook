---
format: aep.planning-md/3
id: upstream-blocker:ess-target-process-release
kind: upstream-blocker
status: open
title: 'ESS: release carrying --target process'
tags:
- ess
relations:
- blocks: story:conformance-against-implementation
revision: 1
---
Waiting on ESS: a release carrying `ess verify conform run --target process` (protocol
`ess-conformance-target/1`), ESS pull request 797, head `153cac0ec6`, not merged and unreleased on
2026-11-21. Merge and release both wait for the operator (the ESS session, 2026-11-21). Design:
ESS `docs/design/conformance-target-process.md`; guide: `website/docs/guides/verify/target-process.md`.
