---
format: aep.planning-md/3
id: story:static-musl-release
kind: story
status: active
title: Static musl release archives
summary: A release tag builds gnu and static musl archives of an installed prefix, checked in Alpine, and uploads them to the Release.
relations:
- informed_by: story:prebuilt-release-binary
revision: 6
transitions:
- {from: "draft", to: "proposed", at: "2026-11-26T10:04:32Z", actor: "agent:runbook", revision: 2}
- {from: "proposed", to: "active", at: "2026-11-26T10:04:32Z", actor: "agent:runbook", revision: 3}
---
## Outcome

A release tag builds two archives of an installed prefix, one for `x86_64-unknown-linux-gnu` and one static for
`x86_64-unknown-linux-musl`, each with its sha256 file, and puts them on the tag's GitHub Release. The musl archive
runs in a plain Alpine container, and its `runbook` allocates through mimalloc.

## Found

The repository has no release workflow (workflows on `main`: `b10x-docs-site`, `pages`, `protocols`,
`shared-gates`) and its GitHub Releases carry the notes only (`docs/releasing.md`). A host without glibc (Alpine, a
distroless image) cannot run a build of `scripts/install.sh`, which builds for the machine it runs on and links
glibc. `sdc-conform` picks the platform of a pinned tool by `conformance::target_triple()`, which says
`<arch>-unknown-linux-gnu` on every Linux host, also on a musl one.

## Order

The operator, 2026-11-25: "A" (a static musl release), with the release workflow under `.github/workflows/` allowed
for wave 5 of 2026-11-26 (relayed by the host operator).

## Acceptance

- A tag push of a bare version runs `.github/workflows/release.yml`, which refuses a tag that differs from the
  version of `runbook/Cargo.toml`, builds `runbook-<version>-x86_64-unknown-linux-gnu.tar.gz` and
  `runbook-<version>-x86_64-unknown-linux-musl.tar.gz`, each the prefix `scripts/install.sh` writes (`bin/runbook`,
  `libexec/runbook/<helper>`, `share/runbook/`), with a `.sha256` file beside each, and uploads the four files to
  the tag's Release (case: the workflow's jobs `archive` and `publish`).
- The musl job fails unless `file` and `ldd` call every program of the prefix static and each one answers
  `--version` with its name and the version, exit 0, in a plain `alpine` container without network (case: the step
  "Each program is static and runs in a plain alpine container").
- Every program the prefix holds answers `--version` (case: `install::every_program_answers::every_program_of_the_prefix_answers_version_with_its_name_and_the_version`).
- The musl `runbook` carries mimalloc as its global allocator through the cargo feature `mimalloc`, on by default
  and compiled only for a `musl` target: the gnu build links no mimalloc (case: the steps "The musl runbook
  allocates through mimalloc" and "The gnu runbook links no mimalloc"; locally `cargo tree -i mimalloc` per target).
- A pull request that changes the release workflow or a `Cargo.toml` runs the musl job once (build, layout, static
  and Alpine checks) and uploads nothing (case: the job `archive` on this change's pull request).
- `conformance::target_triple(pin)` gives `<arch>-unknown-linux-musl` on a musl build when the pin names that
  platform, and `<arch>-unknown-linux-gnu` otherwise; the pins stay unchanged (case:
  `conformance::target_triple_takes_the_musl_platform_only_when_the_pin_names_it` in `driver/tests/conformance.rs`).
- `README.md` says how to install from a release archive; `docs/releasing.md` names the workflow and its assets.
- No tag and no release is made by this story, and no existing workflow job changes.

## Scope

- cited: `.github/workflows/release.yml` (new), `runbook/Cargo.toml`, `runbook/Cargo.lock`, `runbook/src/main.rs`
  (the global allocator), `driver/src/conformance.rs` (`target_triple`, `:994`), `driver/src/conform.rs`
  (`install_pinned`), `driver/tests/conformance.rs`, `driver/src/lib.rs` (`answer_version`), the ten helper programs
  of `driver/src/bin/` that `scripts/install.sh` lists, `watch/src/main.rs`, `runbook/tests/install/`, `README.md`,
  `docs/releasing.md`, `CHANGELOG.md`.
- inferred, checked: `scripts/install.sh` needs no change: the workflow places the musl build where
  `--from-built release` reads it, and the script lays out the prefix.

## Blocked upstream

The musl build of `runbook` stops in `b10x-substrate-host` (substrate rev `85da2e2b`, reached through Loom
`ed644a4d`): `crates/substrate-host/src/quota.rs:219` and `src/lib.rs:1114` call `nix::fcntl::renameat2` and
`RenameFlags`, which nix 0.31.4 defines for `target_env = "gnu"` only (`src/fcntl.rs:447`, `:474`). With
`--keep-going` no other crate failed; `driver/` and `watch/` build static. The musl `libc.a` that rustc 1.99.1
bundles has `renameat` and no `renameat2` (`nm`), so a fix through `libc::syscall(SYS_renameat2, ...)` links where
`libc::renameat2` would not. Until substrate builds for musl and the Loom pin moves, the musl job of the workflow is
red at its build step and a tag uploads nothing.
