# Changelog

All notable changes to this repository are recorded here. One version covers the repository
(`sdc-drive`, `sdc-watch`, `sdc-goals`, `ui/`), released under bare-version tags such as `0.1.0`.
The crates are `publish = false`; releases are source releases at bare-version tags
(`docs/releasing.md`).

## [Unreleased]

## [0.1.0] - 2026-11-21

The first release: a prototype. No interface is stable. The longer note, with the Trial 3 runs and
the limits, is `docs/releases/0.1.0.md`.

### Added

- Nine Canon `protocol/1` protocols: the eight of the cycle from an idea to a built, tested
  product (idea, intake, direction, epic, spec, plan, story, release) and `sdc-bugfix`, one bug fix
  red first (`protocol/*.protocol.yaml`, `docs/bugfix.md`).
- `sdc-drive` (`driver/`): evaluates a case tree on Canon, runs the tool bound to one admissible
  action, records the result as `canon-evidence/1`. Autopilot (a decider model in place of a
  person, final), a review node as a model call (`change.review`, `50d90b9`), rework after a
  rejected approval (at most two reworked, the third ends the run), a run that ends by rule
  (`rule:autopilot`), a finding re-raised after a decision stops the run `finding_reraised`
  (`6c7ba78`), a model per node class (`SDC_LLM_MODEL_*`), a hermetic `claude-cli` adapter with
  an explicit default effort per node class (`f747085`), and the requester-side decider
  (`decider:requester`, `goals/`).
- `watch/` (`sdc-watch`: file changes of a run) and `ui/` (the live view of a run).
- `scripts/run-finding-cycles.sh` (68 expected cycles and 7 pending on this tree) and
  `scripts/evaluate-cases.sh` (52 situations in 12 fixture cases).
- `docs/reject-routes.md` (the audit of the reject and cap routes), `docs/simulate-findings.md`
  (the triage of Canon's 71 `simulate` findings, 5 real dead ends) and `docs/trial-3.md`.
- Trial 3, 2026-11-21, recorded in runs that are not in git: five of eight batch runs of the
  to-do idea closed delivered, unattended, with an Opus decider and Sonnet work, a Haiku decider
  and Opus work, a mixed Sonnet and Haiku, or an all-Haiku setting; two more runs, with the review
  model call, closed delivered with an Opus decider and Sonnet for all work and the review
  (`docs/trial-3.md`, `docs/releases/0.1.0.md`).
- `CHANGELOG.md`, `docs/releasing.md`, `docs/releases/0.1.0.md` and a root `Taskfile.yml`
  (`task check`, `task release:check VERSION=<version>`).

### Changed

- `ui/package.json` and `ui/package-lock.json` carry the repository version, 0.1.0. The three
  crates and their lock files were already at 0.1.0.
- `README.md` § State names the release; `docs/driver.md` no longer lists `change.review` among
  the program stubs.

### Known limits

- Earlier runs' decisions were taken with the host's settings (the decider's call was not
  hermetic), so the passes are not reproducible on another machine as recorded; no pass ran on
  the hermetic adapter.
- Only two runs through the fixed review node have passed (`trial-3-strong-6`, `trial-3-strong-5`, on `main` plus the
  review fix, with a non-hermetic decider); five passes ran with the review step a stub.
- Nobody started the delivered product.
- The decider's principal is `decider:haiku` whatever model decides, and the driver's "LLM calls so
  far" counts runs that called no model.
- The bug-fix runs ran on a branch of which only `d2e4896` is in this release; the red-check defect
  fixed in `99238c0` is not, and no scripted cycle covers `sdc-bugfix`.
- The driver's runtime parts are a temporary copy of what the Loom runtime must provide.
- Release and operate are being split out of the cycle; that is not in this release.
