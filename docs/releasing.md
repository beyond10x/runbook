# Releasing

A release is a source release at a bare-version tag (`0.1.0`), the process of
`../engineering-protocols` (`AGENTS.md` § Releases) and `../canon` sized to this repository. Nothing
is published to crates.io (the three crates are `publish = false`) and there is no release workflow
and no binary or archive asset; the GitHub Release carries the notes only.

## Where the version lives

One version for the repository, written in these files and nowhere else:

- `driver/Cargo.toml` (`sdc-drive`), `watch/Cargo.toml` (`sdc-watch`), `goals/Cargo.toml`
  (`sdc-goals`), and the `sdc-*` package entry of each crate's `Cargo.lock`;
- `ui/package.json` and the root entry of `ui/package-lock.json`.

`task release:check VERSION=<version>` fails when any of these differs from `<version>`, when
`CHANGELOG.md` has no `## [<version>] - <date>` entry, when `docs/releases/<version>.md` is missing,
or when the planning store does not validate.

## Who takes which step

| Step | Who | How |
|---|---|---|
| 1. Release preparation: the version in the files above, the `CHANGELOG.md` entry moved out of `[Unreleased]`, `docs/releases/<version>.md`, `README.md` § Status | bot commit | one commit by `b10x-bot[bot]`, `b10x-gates bot -- commit -F <message file>`, merged through the merge queue |
| 2. Gates on that commit | bot, any session | `task check`, `scripts/check-protocols.sh` (about 12 minutes, in the background), `task release:check VERSION=<version>`, `b10x-gates check --receipt <file>`; the CI `Shared source gates` check green on `main` |
| 3. The tag | operator's decision | an annotated tag by the bot on the merged commit, after the operator's word to release |
| 4. The GitHub Release | operator's decision | created by the bot with the notes `docs/releases/<version>.md`, after the tag |
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
