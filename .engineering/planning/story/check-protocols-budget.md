---
format: aep.planning-md/3
id: story:check-protocols-budget
kind: story
status: active
title: check-protocols.sh ends within the 120 s gate-step budget without dropping a check
summary: The protocol check was 576 s and 628 s; it caches each protocol's passed checks by the digest of their sources and enforces its own budget
relations:
- decomposes: epic:conductor-succession
scope:
- confidence: cited
  path: AGENTS.md
- confidence: cited
  path: CHANGELOG.md
- confidence: cited
  path: Taskfile.yml
- confidence: cited
  path: docs/releasing.md
- confidence: cited
  path: protocol/check-ledger.json
- confidence: cited
  path: scripts/check-protocols.sh
revision: 5
transitions:
- {from: "draft", to: "proposed", at: "2026-11-22T01:37:46Z", actor: "human:operator", revision: 4, correlation: "operator-standing-acceptance-2026-11-22"}
- {from: "proposed", to: "active", at: "2026-11-22T01:37:46Z", actor: "human:operator", revision: 5, correlation: "operator-standing-acceptance-2026-11-22"}
---
# Story: check-protocols-budget

## Outcome

`scripts/check-protocols.sh` ends within the 120 s budget of a gate step and drops no check. It was
576 s and 628 s in its last two full runs on this machine (592 s in the measurement below).

Rule served (`../AGENTS.md`, "Tests stay fast"): "The test step of a repository's gate ends within
2 minutes of wall time"; "An expensive product is made once, never per test"; "Full regeneration from
sources runs in a separate lane named `slow`, only when the sources changed, and compares bytes with
the committed file".

## Where the 592 s went (measured before any change)

`canon` timed per call (wrapper around the 0.10.0 binary, `scripts/check-protocols.sh` unchanged,
load average 37 on 20 CPUs). Seconds per call:

| protocol | `canon check` | gate probe (`canon check` of the probe copy) | `canon simulate` | validate + compile |
|---|---|---|---|---|
| sdc-plan | 131.8 | 133.2 | 0.2 | 0.0 |
| sdc-spec | 66.9 | 73.9 | 0.3 | 0.0 |
| sdc-direction | 62.6 | none | 0.0 | 0.0 |
| sdc-story | 18.8 | 28.9 | 0.1 | 0.0 |
| sdc-intake | 27.0 | none | 1.6 | 0.0 |
| sdc-epic | 15.2 | 23.8 | 0.1 | 0.0 |
| sdc-release | 1.9 | 1.6 | 0.1 | 0.0 |
| sdc | 1.5 | none | 0.1 | 0.0 |
| sdc-bugfix | 1.2 | none | 0.0 | 0.0 |
| sum | 327.0 | 261.4 | 2.5 | 0.0 |

The 14 `canon check` calls are 588.3 s of the 592 s (99 %); `sdc-plan` alone is 265.0 s (45 %). The
cause is the known shape: the same product is made again on every run although its sources did not
change. A protocol's check result is a function of the protocol file, the pinned Canon and the probe
definition, and nothing in the gate reads it twice.

## Change

- `protocol/check-ledger.json` (`runbook.protocol-ledger/1`, generated, committed): per protocol the
  digest of its sources (the Canon commit of `driver/canon-pin.json`, the sha256 of the script, of
  the protocol file and of its pinned simulate report) and the lines `canon check`, the gate probe
  and `canon simulate` printed.
- Fast lane (default): validate and compile every protocol (about 0.1 s together; the IR is written
  to `protocol/compiled/`), regenerate the probe files, compare digests. A protocol with a current
  digest takes `canon check`, probe and `canon simulate` from the ledger. A protocol whose digest
  differs gets the real checks now, one `slot` job per costly `canon` call, and the run is named a
  slow lane run; it exits 1 when the ledger is not current. `--record` writes the ledger entries of
  the protocols it re-checked.
- Slow lane (`--slow`, `task check:protocols:slow`; 651 s, 387 s and 224 s wall in three cold runs at load average 34 to 61, the spread is the CPU share and the slot queue; 133 s is its floor on an idle machine): the real checks of every protocol, the ledger
  rebuilt from them and compared byte for byte with the committed one. `--repin` implies it. It runs
  when a protocol, `driver/canon-pin.json` or the script changed.
- The script prints the time per protocol and the total, names the lane, and exits 1 when a fast lane
  run takes longer than 120 s.

## Checks, none dropped

| check | fast lane | slow lane |
|---|---|---|
| CANON_REV equals the pin | runs | runs |
| canon version equals the pin (`canon_resolve`) | runs | runs |
| `canon validate` per protocol (9) | runs | runs |
| `canon compile` per protocol (9) | runs | runs |
| `canon check` per protocol (9) | ledger digest; real run when the digest differs | runs |
| gate probe per protocol of spec, plan, epic, story, release (5): exactly `probe.passed_without_gate` unreachable | ledger digest; real run when the digest differs | runs |
| `canon simulate` per protocol (9) against `protocol/simulate/<name>.txt` | ledger digest (the report's sha256 is in the digest); real run when the digest differs | runs |
| across protocols: none exists in the script | | |

## Acceptance

1. `scripts/check-protocols.sh` on a tree whose ledger is current runs in at most 120 s, three runs
   in a row (0.4 s each on this machine at load average 26 to 30 on 20 CPUs), prints the time per protocol and its total, and
   exits 0.
2. A protocol file changed so that `canon check` reports a finding makes the script run the real
   check for that protocol, print the finding, name the slow lane and exit 1; restored, it exits 0.
3. `scripts/check-protocols.sh --slow` runs every check for real and exits 0 only when the ledger it
   regenerates equals `protocol/check-ledger.json` byte for byte.
4. `scripts/check-protocols.sh` exits 1 above 120 s in the fast lane (`SDC_PROTOCOLS_BUDGET=0` shows
   it).

## Scope

`scripts/check-protocols.sh`, `Taskfile.yml` (tasks `check:protocols`, `check:protocols:slow`),
`protocol/check-ledger.json` (new, generated), `docs/releasing.md`, `AGENTS.md`, `CHANGELOG.md`.

## Found, not built

- The slow lane is bound by `canon check` of `sdc-plan` (131.8 s) and its probe (133.2 s): at 70 %
  of a CPU each, about 133 s on an idle machine however many protocols run beside it. The cost is
  Canon's state exploration (65536 states); it is a request to Canon, not a change here.
- The probe's `canon check` repeats the plain check of the same protocol plus one outcome; one call
  could serve both. Not done: it changes what the plain check proves.
- `docs/protocol-notes.md` ("Simulate") still describes the simulate step as a step of the script; it
  is outside this unit's file list.
