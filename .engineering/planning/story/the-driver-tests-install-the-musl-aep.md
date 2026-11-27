---
format: aep.planning-md/3
id: story:the-driver-tests-install-the-musl-aep
kind: story
status: active
title: The driver tests install the musl aep on every Linux host
summary: task test:driver builds sdc-conform for <arch>-unknown-linux-musl and runs the static aep it installs, so a host with glibc older than 2.39 runs the step
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-11-27T16:25:13Z", actor: "agent:runbook", revision: 2}
- {from: "proposed", to: "active", at: "2026-11-27T16:25:14Z", actor: "agent:runbook", revision: 3}
---
## Context

`task test:driver` puts the directory of the pinned aep (`driver/aep-pin.json`) first on the `PATH` of the driver
tests, the planning store the programs under test call. It took that directory from `cargo run --bin sdc-conform --
aep-path` (`Taskfile.yml:174`, `:178` before this story), a gnu build of `sdc-conform`, which names the pin's
`<arch>-unknown-linux-gnu` aep (`conformance::platform_for`, `driver/src/conformance.rs:1004`: a gnu build takes
the gnu platform). The forge installs the same gnu aep before the step (`.github/workflows/pages.yml:120-122`). aep
0.81.0, the pin, ships a gnu aep that needs glibc 2.39 or newer and a static musl aep for both Linux architectures. So
a build node whose glibc is older than 2.39 cannot run the step: the gnu aep does not start there.

## Decision

The operator session, 2026-11-27: the story is this repository's. `task test:driver` builds and installs the musl
`sdc-conform`, so the musl aep, on every Linux platform. It does not switch on the glibc version.

## Acceptance

- `task test:driver` passes on a host with glibc older than 2.39.
- On Linux the step runs `sdc-conform` built for `<arch>-unknown-linux-musl` (`cargo run --target
  <arch>-unknown-linux-musl --bin sdc-conform`); it adds that rustup target when it is missing, installs the pinned
  aep with that program when `aep-path` finds none (`aep-install`), and puts the directory `aep-path` names,
  `<cache>/<version>/<arch>-unknown-linux-musl/`, first on the `PATH` of the tests.
- The aep the driver tests run is static: `file` and `ldd` of the installed aep say static, and the case
  `aep_pin::the_aep_the_driver_tests_run_is_static` fails when the first `aep` on the tests' `PATH` has a
  `PT_INTERP` program header (red on the step before this story with the gnu aep).
