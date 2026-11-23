---
format: aep.planning-md/3
id: story:ess-0-98-0-pin
kind: story
status: active
title: The ESS pins move from 0.97.4 to 0.98.0
summary: 'The operator asked for the most recent ESS version. ESS 0.98.0 was published 2026-11-23: the pin file, the inputs file and the toolchain default name it, and the generated outputs are the ones it writes.'
scope:
- confidence: cited
  path: bindings/tools/sdc-tool.sh
- confidence: cited
  path: docs/conformance.md
- confidence: cited
  path: driver/ess-pin.json
- confidence: cited
  path: ess/ess-inputs.yaml
- confidence: cited
  path: ess/generated
revision: 8
transitions:
- {from: "draft", to: "proposed", at: "2026-11-23T09:14:48Z", actor: "human:operator", revision: 2, executor: "agent:runbook"}
- {from: "proposed", to: "active", at: "2026-11-23T09:14:49Z", actor: "human:operator", revision: 3, executor: "agent:runbook"}
---
## Outcome

The repository pins ESS 0.98.0: `driver/ess-pin.json`, `ess/ess-inputs.yaml` and the default of
`ESS_TOOLCHAIN` name it, the specification in `ess/` validates under it, and the committed generated
outputs are the ones 0.98.0 writes.

## Source

The operator's request of 2026-11-23: "upgrade to the most recent ESS version". ESS 0.98.0 was published
2026-11-23T09:09:12Z (https://github.com/beyond10x/ess/releases/tag/0.98.0). The notes of the ESS session:
the branch-order refusal this repository answered on `main` (`story:submitrun-outcome-order`) is in it; a
copy of this repository's specification validated on the release candidate; synthesis on that copy gave 118
scenarios and 95 refusals against 112 and 101 with 0.97.4, none removed; one new refusal at every format: an
ordering (`<`, `<=`, `>`, `>=`) over an enum, written as `in: [...]`.

## Scope

- `driver/ess-pin.json` (version, release, commit, per platform the sha256 of the archive and of the
  binary), `ess/ess-inputs.yaml`, `bindings/tools/sdc-tool.sh` (`ESS_TOOLCHAIN`), `Taskfile.yml`,
  `docs/conformance.md`, `CHANGELOG.md`.
- `ess/domains/*.yaml` only where 0.98.0 refuses a declaration; `ess/generated/` as 0.98.0 writes it.

## Acceptance

- Each archive's sha256 in the pin equals the release's `SHA256SUMS`, and each binary sha256 is the hash of
  the member of that archive.
- `task check:ess-generated` exits 0 with the 0.98.0 binary; the synthesized suite holds no fewer scenarios
  than before, and the counts before and after are in the change's text.
- The guard of the pin is unchanged: a binary whose bytes differ from the pin is refused, and one case shows
  the 0.97.4 binary refused after the pin moved.
- No statement of the release ledger changes: the pin moves no run metric unless a record shows it.

## Not in this story

- The shared `ess` binary of the host: the ESS session replaces it.
- Re-packing a committed recording of the replay lane; a recording that needs it gets its own decision.
