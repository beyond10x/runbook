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

- Eight Canon `protocol/1` protocols for the cycle from an idea to a built, tested product: idea,
  intake, direction, epic, spec, plan, story, release (`protocol/*.protocol.yaml`).
- `sdc-drive` (`driver/`): evaluates a case tree on Canon, runs the tool bound to one admissible
  action, records the result as `canon-evidence/1`. Autopilot (a decider model in place of a
  person, final), a review node as a model call (`change.review`), rework after a rejected
  approval (at most two reworked, the third ends the run), a run that ends by rule
  (`rule:autopilot`), a model per node class (`SDC_LLM_MODEL_*`), and the requester-side decider
  (`decider:requester`, `goals/`).
- `watch/` (`sdc-watch`: file changes of a run) and `ui/` (the live view of a run).
- `scripts/run-finding-cycles.sh` (56 expected cycles on this tree) and
  `scripts/evaluate-cases.sh` (45 fixture cases).
- Trial 3, 2026-11-21, recorded in runs that are not in git: five of eight runs of the to-do idea
  closed delivered, unattended, with an Opus decider and Sonnet work, a Haiku decider and Opus
  work, a mixed Sonnet and Haiku, or an all-Haiku setting (`docs/trial-3.md`,
  `docs/releases/0.1.0.md`).
- `CHANGELOG.md`, `docs/releasing.md`, `docs/releases/0.1.0.md` and a root `Taskfile.yml`
  (`task check`, `task release:check VERSION=<version>`).

### Changed

- `ui/package.json` and `ui/package-lock.json` carry the repository version, 0.1.0. The three
  crates and their lock files were already at 0.1.0.
- `README.md` § State names the release.

### Known limits

- On this release a real run with default settings stops at the first code review: three runs from
  `main` at `8db4b93` with the real review node stopped `stuck` at their first `change.review`
  call, its reply file unreadable (`docs/releases/0.1.0.md`).
- The decider's model call is not hermetic: it loads the host's Claude Code setup and takes its
  effort level from the host's settings, so the passing runs' decisions are not reproducible on
  another machine as recorded (`docs/releases/0.1.0.md`).
- The five passing runs ran on branch `trial/3b` at `acd6f12` with the review step a stub; no
  review model call ran in them. Nobody started the delivered product.
- The decider's principal is `decider:haiku` whatever model decides, and the driver's "LLM calls so
  far" counts runs that called no model.
- The driver's runtime parts are a temporary copy of what the Loom runtime must provide.
- Release and operate are being split out of the cycle; that is not in this release.
