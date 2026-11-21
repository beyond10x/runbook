---
format: aep.planning-md/3
id: upstream-blocker:ess-target-process-release
kind: upstream-blocker
status: cleared
title: 'ESS: release carrying --target process'
tags:
- ess
relations:
- blocks: story:conformance-against-implementation
revision: 2
transitions:
- {from: "open", to: "cleared", at: "2026-11-21T14:25:21Z", actor: "human:operator", revision: 2, executor: "agent:conformance-observe", correlation: "operator-standing-acceptance-2026-11-21"}
---
Waiting on ESS: a release carrying `ess verify conform run --target process` (protocol
`ess-conformance-target/1`), ESS pull request 797, head `153cac0ec6`, not merged and unreleased on
2026-11-21. Merge and release both wait for the operator (the ESS session, 2026-11-21). Design:
ESS `docs/design/conformance-target-process.md`; guide: `website/docs/guides/verify/target-process.md`.
