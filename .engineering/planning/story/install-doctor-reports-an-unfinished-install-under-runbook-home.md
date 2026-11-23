---
format: aep.planning-md/3
id: story:install-doctor-reports-an-unfinished-install-under-runbook-home
kind: story
status: draft
title: doctor, list and check report an unfinished install also when RUNBOOK_HOME is set
summary: With RUNBOOK_HOME set, doctor prints no install line for a prefix that holds the marker, and list exits 0. From adversary pass 3 of the install unit, left open by fix round 3.
relations:
- informed_by: story:install-script-lays-out-a-prefix
revision: 1
---
## Outcome

`runbook doctor`, `list` and `check` report an unfinished install whenever the files they read lie in a prefix that holds the marker of an unfinished install, also when `RUNBOOK_HOME` names that place.

## Source

Adversary pass 3 of `story:install-script-lays-out-a-prefix` measured that with `RUNBOOK_HOME` set, `runbook doctor` of a prefix that has the marker prints no install line and gives the same count as for a whole prefix, and `list` exits 0. This holds also when the variable names the stopped prefix's own `share/runbook` (`runbook/src/doctor.rs:154`). That `RUNBOOK_HOME` wins over the binary's own prefix is older (`docs/cli.md:110`). Fix round 3 left it open.

## Scope

- `runbook/src/doctor.rs`, `runbook/src/assets.rs` (the lookup of the marker).
- `docs/cli.md` (the sentence on `RUNBOOK_HOME`).
- `runbook/tests/install/half_install.rs`.

## Acceptance

- With `RUNBOOK_HOME` naming a directory inside a prefix that holds the marker, `doctor` prints the install line and exits 1, and `list` and `check` exit 1, as they do without the variable. A case in `runbook/tests/install/` shows it. With `RUNBOOK_HOME` naming a directory outside any prefix, nothing changes.

## Origin

Introduced by the install unit; the precedence of `RUNBOOK_HOME` is pre-existing. Held by the review record of its adversary pass 3 (finding N6); fix round 3 did not touch it.
