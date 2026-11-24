---
format: aep.planning-md/3
id: story:a-bug-fix-run-hands-its-checks-git-trees
kind: story
status: active
title: A bug-fix run hands each check command a git tree with one commit of the copied code
summary: Each copy a check command of a bug-fix run runs in is a git repository with one commit of the copied files, made with a fixed identity and none of the host's git configuration; the diff, the artifacts and their digests stay as they are.
relations:
- decomposes: epic:ticket-to-merge-request-on-a-real-repository
- informed_by: story:bugfix-check-reaches-the-pool
scope:
- confidence: cited
  path: CHANGELOG.md
- confidence: cited
  path: docs/bugfix.md
- confidence: cited
  path: docs/cycles-suite.md
- confidence: cited
  path: driver/src/bin/sdc-bugfix.rs
- confidence: cited
  path: driver/src/bin/sdc-recorded-pool.rs
- confidence: cited
  path: driver/tests/bugfix_git_trees.rs
- confidence: cited
  path: driver/tests/main.rs
- confidence: cited
  path: driver/tests/recorded_pool.rs
revision: 6
transitions:
- {from: "draft", to: "proposed", at: "2026-11-24T16:26:55Z", actor: "human:operator", revision: 3, executor: "agent:runbook", correlation: "consumer-order-2026-11-24-checks-get-git-trees"}
- {from: "proposed", to: "active", at: "2026-11-24T16:26:55Z", actor: "human:operator", revision: 4, executor: "agent:runbook", correlation: "consumer-order-2026-11-24-checks-get-git-trees"}
---
## Request

From the consumer of the bug-fix runbook, 2026-11-24, after a decision of the operator the same day:
the check commands of a bug-fix run go to the build pool's remote mode, and that mode needs a git
repository in the tree of each job. Today a bug-fix run copies the code tree a check command runs in
without any `.git` (`driver/src/bin/sdc-bugfix.rs`, `skip` and `copy_to`): the copy of `impl/` that
`repro-run`, `tests-run` and `gate-run` run in, and the copy of the base (`mainline/`, or `bug.yaml`
`base`) that `repro-run` runs the suite in.

## Outcome

Each tree a bug-fix run hands to a check command is a git tree: when the run copies the tree, it runs
`git init` there and makes one commit of the copied files.

- The commit identity is fixed and generic (`runbook <runbook@localhost>`), with fixed author and
  committer dates, so the commit id depends on the copied files only. Nothing of the host's git
  configuration is read (no global or system file, no `GIT_*` variable of the environment).
- In that repository git maintenance is off (`maintenance.auto=false`, `gc.auto=0`) and no hook runs
  (`core.hooksPath=/dev/null` in its own configuration, `--no-verify` on the commit).
- No remote, no history of the source repository: one commit, on the branch `main`.
- A copy made again (a second step, a retry) gets its own repository, made once for that copy; two
  commands of one step share the copy they run in.
- The fix's diff is taken as before: the byte compare of the base and `impl/`. The new `.git` lies only
  in the copies under `.sdc/bugfix/`, so it never enters a diff, an artifact (`impl/`, `mainline/`,
  `repro/`), a digest of an artifact or a copy back.
- A `.slot/` directory a check command writes stays in the copy it was written in and out of the diff,
  as before; with a git tree, the pool's client writes it into `.git/info/exclude` itself.
- Every reader that takes a digest of or copies a tree a check runs in leaves the `.git` out. All
  but one did already (`sdc-bugfix`'s own diff and copy, the parity listing, the decider's inputs, the
  conformance and review readers, the file watcher); the cycles suite's recorded pool did not, and
  its index and objects differ with every copy, so it is changed in this story.

## Acceptance

1. A check command run by `repro-run`, `tests-run` or `gate-run` finds its working directory a git
   repository whose `git rev-list --count HEAD` is 1, whose `git status --porcelain --ignored` prints
   nothing, whose `git ls-files` lists exactly the files of the copied tree, and whose `git remote`
   prints nothing; its local configuration holds `maintenance.auto=false`, `gc.auto=0` and
   `core.hooksPath=/dev/null`.
2. The commit's author and committer are `runbook <runbook@localhost>` at the fixed instant, and the
   commit id of the same base is the same in two runs.
3. With a hostile git configuration in the environment (`GIT_CONFIG_GLOBAL` naming a file with another
   identity, commit signing through a program that fails, and a hooks directory whose hooks fail, plus
   `GIT_AUTHOR_*` and `GIT_COMMITTER_*` variables), the commit id is the same as without it and no hook
   ran.
4. With the `.git` present in the copies, the bytes of `impl/`, `mainline/`, `repro/`, `bug.yaml` and
   `checks.yaml` are the same after each step as before it, no `.git` lies anywhere in the work
   directory outside `.sdc/bugfix/`, and `repro-run`'s diff of a changed `impl/` names exactly the
   changed code file.
5. A `.slot/` directory a check command writes (and adds to `.git/info/exclude`) is not in `impl/` or
   `mainline/` afterwards and is not in the copy of the next step.
6. The cycles suite's recorded pool (`sdc-recorded-pool`, the stand-in pool that answers check
   commands from recordings keyed by the digest of the tree they run in) gives a tree with a `.git`
   at its top the same digest as the tree without it, also when the bytes in `.git` change, and its
   record mode runs the command in a copy with no `.git`.
7. Each case runs under 5 s.

## Origin

The consumer's request of 2026-11-24 about 17:00 CET, relayed by its operator session after the
operator's decision that the check jobs of a bug-fix run go to the build node.
