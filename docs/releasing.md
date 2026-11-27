# Releasing

A release is a source release at a bare-version tag (`0.1.0`), the process of
`../engineering-protocols` (`AGENTS.md` § Releases) and `../canon` sized to this repository. Nothing
is published to crates.io (the four crates `driver/`, `watch/`, `goals/` and `runbook/` are
`publish = false`). The GitHub Release carries the notes and, from the release workflow
(`.github/workflows/release.yml`, below), two archives of an installed prefix.

## The archives

A push of a bare-version tag runs `.github/workflows/release.yml`. It stops when the tag is not the version of
`runbook/Cargo.toml`, and builds one archive per target, each with a `.sha256` file (`sha256sum` format):

- `runbook-<version>-x86_64-unknown-linux-gnu.tar.gz`: `scripts/install.sh --prefix <dir>` on the runner, which
  builds for the runner (glibc);
- `runbook-<version>-x86_64-unknown-linux-musl.tar.gz`, static: `cargo build --locked --release --target
  x86_64-unknown-linux-musl` of `watch/`, `driver/` and `runbook/` (with `musl-tools` for the C code), the binaries
  copied to `<crate>/target/release/`, then `scripts/install.sh --prefix <dir> --from-built release`. The script
  lays out the prefix and builds no cross build itself.

Each archive holds one directory `runbook-<version>-<target>/` with `bin/`, `libexec/runbook/` and
`share/runbook/` as the script wrote them. The musl job fails unless `file` and `ldd` call every program of the
prefix static, each one answers `--version` with exit 0, its name and the release version (`<name> <version>`; `runbook` adds its
commit) in a plain `alpine` container without network, and its
`runbook` holds `mi_malloc_aligned` (what the `GlobalAlloc` of the crate `mimalloc` calls; a linked musl `runbook` has no symbol `mi_malloc`); the gnu job fails when its `runbook` holds `mi_malloc_aligned`. Every step runs with `pipefail`, so the
listings these checks read (`nm`, `tar -t`) are written to a file under `$RUNNER_TEMP` first: a `grep -q` or `head`
reading a pipe can end its writer early and fail a check that held (`install::release_workflow` in
`runbook/tests/install/` refuses such a reader in the workflow). The `publish` job checks the
four files against each other and uploads them to the tag's Release; when the Release does not exist yet, it
makes it with the notes `docs/releases/<version>.md` (the body step 4 gives it).

mimalloc is the global allocator of a musl build of `runbook` only (musl's own allocator is slow for work that
allocates much): the cargo feature `mimalloc` of `runbook/Cargo.toml`, on by default, adds the crate only for a
`musl` target, so the gnu build compiles and links no mimalloc. `--no-default-features` turns it off.

The `sdc-conform` of the musl archive installs a pinned tool's `<arch>-unknown-linux-musl` build when the pin names
one, and the `-linux-gnu` build when it does not (`conformance::platform_for` in `driver/src/conformance.rs`). The aep
pin (`driver/aep-pin.json`, aep 0.81.0) names both musl archives, so `sdc-conform aep-install` installs a static aep
that needs no glibc (`driver/tests/aep_pin.rs`, `a_musl_build_takes_the_pinned_musl_aep`). The ess pin
(`driver/ess-pin.json`, ess 0.98.0) names none, because that ess release has none: `sdc-conform ess-install`
installs the gnu ess, which needs glibc.

A pull request that changes the workflow, a `Cargo.toml`, a `Cargo.lock` or `rust-toolchain.toml` runs the musl job
once (build, layout, static and Alpine checks) and uploads nothing. The workflow is no required check.

## The binary

`scripts/install.sh --prefix <dir>` is how the binary is made: it builds `runbook`, the helper programs a run
starts and `sdc-watch` with `cargo build --locked --release` and lays them out under `<dir>` (`bin/`,
`libexec/runbook/`, `share/runbook/` with `VERSION` and `MANIFEST`). It is the source of the release asset of
`story:prebuilt-release-binary`: that asset is the prefix of one run of the script on the release commit, packed,
and nothing else; the release workflow calls the script and does not lay files out itself (§ The archives).
`task test:install` (the slow lane, in the
background) runs the real release build into `runbook/target/install-slow` and starts `runbook list`,
`runbook doctor` and one run from it, and the acceptance run `runbook run sdc --dry` from a prefix made from the
debug builds (`--from-built debug`).

The commit `runbook --version` prints, and `share/runbook/VERSION` with it, is HEAD of the build; `VERSION` ends
in ` dirty` when `git status` reports a change of a tracked file (index or work tree), not only of a copied one (the bytes installed are those of the
work tree), so a prefix whose `VERSION` has the word is not the release commit as it stands. The script reads HEAD
before it lists the files and again after its own build and stops when it moved, checks the commit of the built
`runbook` against it, and stops for a target triple, whose build it does not take.
With `--from-built` the script refuses a `runbook` whose commit is not HEAD of the
checkout it copies the files from; the helper programs print no commit, so only `runbook` is checked.
A build made outside git prints `unknown` and keeps it when a `.git` appears later, until another input of the
build changes (cargo watches no path that does not exist: a missing one would rebuild the package on every
call).

## Where the version lives

One version for the repository, written in these files and nowhere else:

- `driver/Cargo.toml` (`sdc-tools`), `watch/Cargo.toml` (`sdc-watch`), `goals/Cargo.toml`
  (`sdc-goals`), `runbook/Cargo.toml` (`runbook`, the command; it prints the version with
  `--version`), and the `sdc-*` or `runbook` package entry of each crate's `Cargo.lock`;
- `ui/package.json` and the root entry of `ui/package-lock.json`;
- `plugin/.claude-plugin/plugin.json` and `plugin/.codex-plugin/plugin.json` (the `runbook` plugin;
  `.claude-plugin/marketplace.json` carries no version).

`task release:check VERSION=<version>` fails when any of these differs from `<version>`, when
`CHANGELOG.md` has no `## [<version>] - <date>` entry, when `docs/releases/<version>.md` is missing,
or when the planning store does not validate.

## Who takes which step

| Step | Who | How |
|---|---|---|
| 1. Release preparation: the version in the files above, the `CHANGELOG.md` entry moved out of `[Unreleased]`, `docs/releases/<version>.md`, `README.md` § Status | bot commit | one commit by `b10x-bot[bot]`, `b10x-gates bot -- commit -F <message file>`, merged through the merge queue |
| 2. Gates on that commit | bot, any session | `task check` (runs the driver test step and the UI test step side by side), `scripts/check-protocols.sh` (fast lane, under 1 second, also part of `task check`; after a change to a protocol, the Canon pin or the script, run it with `--record` and commit `protocol/check-ledger.json`). The slow lane, `task check:protocols:slow` (in the background), is required when `git diff --name-only <last tag>..HEAD -- protocol runbook/Cargo.lock runbook/src/canon_cmd.rs scripts/check-protocols.sh` lists a file: a release with such a change waits for it to pass (CI runs it on such a change, `.github/workflows/protocols.yml`), `task release:check VERSION=<version>`, `b10x-gates check --receipt <file>`; the CI `Shared source gates` check green on `main` |
| 3. The tag | operator's decision | an annotated tag by the bot on the merged commit, after the operator's word to release |
| 4. The GitHub Release | operator's decision | created by the bot with the notes `docs/releases/<version>.md`, after the tag; the release workflow adds the archives to it (or makes it with those notes when it ends first) |
| 5. Record | bot commit | the operator's word as an `approval-record` through `aep plan artifact`, as `engineering-protocols` does (`approval-record:release-0-17-0`) |

Between 1 and 3 `main` can move: before the tag, read the counts and the Limits of
`docs/releases/<version>.md` against the merged tree, and change the notes in a bot commit when a
statement has become false.

Steps 1 and 2 need no decision. Steps 3 and 4 are the operator's: no session tags, publishes or
opens a release on its own judgement, and a request to prepare a release is not the word to release.
The operator's standing rule below is that word for one kind of release.

## Standing rule: a release after every improvement (the operator, 2026-11-21)

The operator, 2026-11-21: "Now steadily improve the workflow. Document your gains, document it
accurately, and prepare for putting this onto a nice image on the public documentation. Cut
releases after every improvement."

- Under that standing order no further word of the operator is needed for a patch or minor
  release of a measured improvement: one merged into `main` with its ledger entry (its line in
  `experiments/releases.yaml`, and `docs/progress.md`, `docs/progress.json` and
  `docs/img/progress.svg` regenerated, with `sdc-experiment check --all` passing,
  `docs/improving.md`).
- The orchestrating session orders the cut once `main` is green on the release commit (steps 1
  and 2 done). The merging session tags and publishes on that order (steps 3 and 4), and step 5
  records the standing order's approval record instead of a new word.
- Not covered, and still waiting for the operator's word: a major version, and a release that
  deletes data, rewrites history or changes what is published elsewhere than this repository.
- The version follows `docs/improving.md` § Versioning: patch for fixes and tools, minor for a
  protocol or behaviour change.

## Steps 3 and 4

Run from a checkout of `main` at the merged release commit. `<commit>` is that commit, `<policy>` the
absolute path of `../gates-policy/policy.json`.

```console
git tag -l <version>
git ls-remote --tags origin <version>
b10x-gates --repository beyond10x/runbook --policy <policy> bot -- tag -a <version> -m "SDC <version>" <commit>
b10x-gates --repository beyond10x/runbook --policy <policy> bot -- push origin <version>
```

The first two print nothing when the tag does not exist yet. The release is created with
`b10x-gates api`, which reads a JSON body from a file (the notes file is `docs/releases/<version>.md`)
and writes the response to a file:

```console
jq -n --arg tag <version> --arg name <version> --rawfile body docs/releases/<version>.md \
  '{tag_name: $tag, name: $name, body: $body}' > <scratch>/release.json
b10x-gates --repository beyond10x/runbook --policy <policy> api --method POST \
  --path /repos/beyond10x/runbook/releases --input <scratch>/release.json --output <scratch>/release-response.json
```

The tag push also starts the release workflow. When its `publish` job ended first, the Release exists with the same
notes and the POST is answered 422 (`already_exists`): step 4 is then done.

## Completion

The release is complete when the peeled tag is the release commit, the `Shared source gates` check
on the tag is green and the GitHub Release shows the notes. A pushed tag with an unfinished check is
queued, not released. Bare `gh` reads only; every write goes through `b10x-gates`.

## Notes and changelog

`CHANGELOG.md` follows the sibling repositories: `## [Unreleased]`, then `## [<version>] - <date>`
with `### Added`, `### Changed`, `### Fixed`; the date is the day of the tag. Each entry states
facts with their source, and what was observed only in runs that are not in git says so.
`docs/releases/<version>.md` is the longer note and the body of the GitHub Release; a limit that
the release does not remove is written in it as plainly as what works.

## The unreferenced-files check

`task check:unreferenced` (part of `task check`, about 1 s) runs `driver/src/bin/sdc-check-unreferenced.rs`
over `git ls-files` and prints the files of `scripts/`, `bindings/`, `instructions/`, `cycles/`,
`cases/`, `tools/`, `watch/`, `views/`, `decision/`, `goals/`, `eval/` and `driver/tests/fixtures/` that no
other tracked text file refers to; it exits 1 when there are any. A file refers to another by its
path, by a path relative to its own directory or crate, by a directory plus a name below it, by
`mod x;`, or as a Cargo target. The history files (`.engineering/`, `experiments/`, `CHANGELOG.md`,
`docs/releases/`, and `docs/progress.md` and `docs/progress.json`, which `sdc-experiment progress` writes from the experiment records) do not count as referrers. A directory that a program reads as a whole is a line
of `driver/unreferenced-whole.txt` (`<dir> <referrer> <reason>`); a file that stays unreferenced on
purpose is a line of `driver/unreferenced-keep.txt` (`<path> <reason>`), and the check refuses a
line whose file is gone or is referred to now. Before a deletion,
`driver/target/debug/sdc-check-unreferenced --without <path>…` prints the files that only the
deleted paths refer to, each with the paths that name it. `sdc-check-unreferenced --help` has the
full rules.
