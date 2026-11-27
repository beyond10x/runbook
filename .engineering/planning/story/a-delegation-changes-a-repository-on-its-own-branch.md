---
format: aep.planning-md/3
id: story:a-delegation-changes-a-repository-on-its-own-branch
kind: story
status: active
title: A delegation changes a repository on its own branch and hands back a merge request
summary: A delegation names an allowed repository; its agent works in a managed worktree on runbook/<run-id>; the daemon commits, pushes and opens a draft merge request
tags:
- consumer-asks-2026-11-27-agents
relations:
- informed_by: story:runner-worktree-step
- informed_by: story:bugfix-hands-back-a-branch-and-a-merge-request
- informed_by: story:daemon-takes-a-delegation
- informed_by: story:route-runs-at-linux-confinement
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-11-27T14:13:26Z", actor: "agent:runbook", revision: 2}
- {from: "proposed", to: "active", at: "2026-11-27T14:13:26Z", actor: "agent:runbook", revision: 3}
---
## Outcome

A delegation can change code. It names a repository the operator allowed in `runbook.toml`, and its agent works in a
managed worktree of that repository on the run's own branch. When the run delivers, the daemon (not the model's
process) commits the change, pushes the branch and opens a draft merge request as the hand-back. A consumer's page can
then dispatch a write agent without a second runtime.

## Context

Gap G2 of a consumer repository that dispatches agents through the daemon (2026-11-27, relayed by the host's
operator session). It asks for a protocol, or a delegate member, that works in a managed worktree on its own branch,
pushes it and opens a merge request: the hand-back of the bug-fix runbook `sdc-bugfix`.

- A delegation takes `{request, model_route, protocol?, workspace?, settings?, ...}` (`runbook/src/commands.rs:728-754`).
  `workspace` is copied into W (`runbook/src/commands.rs:2649-2650`, `copy_dir` `:3088-3101`). Nothing of a run
  reaches a repository.
- The bug-fix runbook stops at `handback.describe` and `handback.spec-check` (`docs/bugfix.md:52-78`). The action that
  opens the code merge request is named as later (`docs/bugfix.md:71-74`). A ticket's `branch` is "the branch a later
  hand-back step will create" (`docs/bugfix.md:351`). Its code tree is a copy, `work/impl/`, not a worktree
  (`docs/bugfix.md:403`).
- No `worktree` call exists in `runbook/src/` today. `story:runner-worktree-step` (active, not built) plans one for
  `runbook run`.

## Design

Terms: **T** is the run's worktree (its root directory). **W** is the run's workspace, `<state>/workspaces/<run-id>`
(`runbook/src/state.rs:346-348`). **R** is the run's directory, `<state>/runs/<run-id>` (`runbook/src/state.rs:342-344`).
**E** is the operator's repository entry in `runbook.toml`.

Today's boundary (origin/main `76551cdd`):

- A delegation's agent node runs `claude -p` with cwd `W/.sdc/llm/<n>-<node>/cwd`
  (`driver/src/bin/sdc-node/call.rs:102-109`; the adapter `cd`s there, `bindings/tools/llm-adapter-claude-cli.sh:173`).
  Its inputs are copied into `in/` (`driver/src/bin/sdc-node/nodes.rs:26-38`, `:185-190`). Its one output is copied
  back into W (`driver/src/bin/sdc-node/call.rs:428-431`).
- The model's tools are Read, Write, Edit, Glob and Grep. Bash, WebFetch, WebSearch, Task and Skill are disallowed,
  and no MCP server is loaded (`bindings/tools/llm-adapter-claude-cli.sh:96-101`).
- Routes serve the level `local` only (`docs/model-routes.md:362-364`; `story:route-runs-at-linux-confinement`). At
  `local`, Loom applies no OS confinement and keeps no driver record (Loom `b0a0dfda`,
  `crates/loom-intake-slice/src/confinement/engine.rs:159-174`).

So today the write fence is the CLI's tool set and its working directory, not a sandbox. The design keeps that kind
of fence at `local` and names what `linux` needs.

1. **Where the worktree lives, who makes and removes it, which repositories.**
   - The daemon makes T with the host's `worktree` CLI when the run enters, before the run thread starts:
     `worktree create --repo <E.path> --base <base commit> --id runbook-<run-id> --purpose "runbook run <run-id>"`.
     It holds a lease while the run is live (`worktree hook session-start --session runbook-<run-id> --path T`;
     `session-end` at the run's end). T's path is the CLI's, recorded in R and hidden from every view (it joins the
     `Hide` list, `runbook/src/engine/mod.rs:1001-1004`).
   - A delegation names a repository only by E's name: the new member `repository: "<name>"`. Never a path. E is a
     table `[repositories.<name>]` in `runbook.toml`, an allow-list that is empty by default, with `path`, `base`,
     `remote`, `default_branch` and the hand-back keys (items 4 and 5). Neither a request nor a model can add an entry.
   - This reuses the create/finish calls of `story:runner-worktree-step`; that story is for `runbook run`, this one
     for the daemon.
2. **What the delegation may write.**
   - Added: the agent node of the new daemon-internal protocol `delegation-change` (written like `delegation-free`,
     `runbook/src/delegation.rs:402-411`) runs with cwd T in place of its call directory. T's files are not copied
     into `in/`.
   - Still written as today: W and the node's record directory under W. That is all.
   - Denied: the main checkout `E.path` and its common git directory, T's `.git` file, other worktrees, R (the
     run's pinned copy and store), `~`, the daemon's state directory, every credential file.
   - At `local` this is the CLI's fence (cwd T, no Bash). After the run the daemon refuses the hand-back when T's
     `.git` file differs from what it wrote at create.
   - At `linux` it is Substrate's fence, item 7.
3. **The branch.**
   - The name is `runbook/<run-id>`. The run id is the daemon's UUIDv7 (`runbook/src/commands.rs:1557`), so no
     request text reaches the name.
   - The base is `E.base`, resolved to a commit at the submit and recorded in R and in the event `runbook.delegated`.
   - The daemon refuses a hand-back whose ref is not exactly `refs/heads/runbook/<run-id>`, so the push never names
     `E.default_branch`.
4. **The push.**
   - The daemon does it, as a hand-back step after the run's protocol outcome is `delivered`. No `git push` runs
     inside the run, and no program of the run asks for it in the first slice.
   - The daemon commits T's changes with `git --git-dir <recorded> --work-tree T`, under the identity `E.commit_name`
     and `E.commit_email`. The commit has `core.hooksPath` set to an empty directory, `GIT_CONFIG_GLOBAL=/dev/null`
     and `GIT_CONFIG_NOSYSTEM=1`, so the operator's own identity, hooks and credential helpers are never used.
   - It refuses a commit that changes a path in `E.deny_paths` (default `.github/`, `.gitlab-ci.yml`, `.gitmodules`):
     a branch in the same repository runs CI with the repository's secrets. It also refuses a link that leaves T and
     a change over `E.max_changed_files` or `E.max_changed_bytes`.
   - Push: `git push <E.remote URL> HEAD:refs/heads/runbook/<run-id>`. No `+`, no `--force*`, a new ref only.
   - The credential is `E.push_token_file`, a bot or deploy identity the operator configures, checked like a route's
     `token_file` (`runbook/src/config.rs:267-293`). The daemon process holds it and gives it to its own git child
     through an askpass of the `runbook` binary, never in an argv, a log, R, W or any process of the run.
5. **The merge request.**
   - After the push the daemon opens a **draft** merge request from `runbook/<run-id>` to `E.default_branch`
     through a configured forge client (`E.forge`: kind, API base, project, `E.forge_token_file`).
   - Title: the first line of the request, bounded. Body: the run id, `result.md` (bounded, as the view answers it,
     `runbook/src/delegation.rs:18`), route, tokens and cost. The text goes in a JSON body, never an argv.
   - The URL goes into the event `runbook.handed-back` and the view `delegation-result` (`handback: {branch, commit,
     merge_request}`).
   - Runbook has no merge call at all.
6. **Network.**
   - The model's process needs the relay on loopback only (the route file, `docs/model-routes.md:440-450`).
   - The new egress is the daemon's own: git over https to `E.remote` and the forge's API. No process of a run gets
     it.
   - At `local` the model's network is not enforced. The only fence is the tool set (no Bash, WebFetch or
     WebSearch).
7. **What changes in confinement.**
   - `local`: no AppliedConfinement exists. The one change is the agent node's cwd from `W/.sdc/llm/<n>-<node>/cwd`
     to T. The added write path is exactly T, minus `T/.git`.
   - `linux` (after `story:route-runs-at-linux-confinement`): the node's `ChildCommand` gets T as a writable
     directory. `.git` is not named, so it stays unwritable (Loom `crates/loom-intake-slice/src/confinement.rs:1366-1372`).
     The common git directory is not mounted. The driver record's `writable_scopes`
     (`crates/loom-intake-slice/src/confinement.rs:1199-1201`) must then list W and T, and nothing more.
   - Loom requests no writable path outside the workspace (`confinement.rs:1389-1391`). T outside W therefore needs
     a Loom request (a writable root besides the workspace, which Substrate offers), recorded in
     `docs/canon-requests.md` per the layers rule.
8. **Failure and cleanup.**
   - A run that ends in any other way (failed, cancelled, stopped on a cap or a route, orphaned) commits and pushes
     nothing.
   - After a hand-back that pushed, the daemon calls `worktree finish runbook-<run-id>` and then `worktree gc --apply
     --id runbook-<run-id>`.
   - Otherwise T is kept for inspection for `E.keep_failed_days` (default 7). The daemon then calls `worktree finish
     --archive --discard-cache`, then `gc`. Runbook never deletes T itself.
   - A daemon that stops with a live run leaves T under its lease. The `worktree reconcile`/`gc` lifecycle handles it.
9. **Architecture changes for the operator to approve.**
   - a. The daemon runs git and the `worktree` CLI on operator-named repositories: it writes outside its state
     directory.
   - b. A new credential kind held by the daemon: a push token and a forge API token per repository entry.
   - c. A new egress of the daemon: git over https and a forge API.
   - d. The model's write fence widens from its call directory to a worktree of an operator-named repository.
   - e. A publication without a person's word per run: the entry in `runbook.toml` is the standing approval for
     draft merge requests. The alternative is a decision request before each push, as
     `story:bugfix-hands-back-a-branch-and-a-merge-request` requires for the bug-fix runbook.
   - f. A Loom request for a writable root outside the workspace, for `linux`.
10. **The smallest useful first slice.**
    - At `local`: E with `path`, `base`, `remote`, `default_branch`, `commit_*`, `push_token_file` and `deny_paths`.
    - The member `repository`, the protocol `delegation-change` (agent node with cwd T), and T made and finished by
      the daemon.
    - After `delivered`: the daemon's commit and the push of `runbook/<run-id>`. The branch and commit go into the
      event and the view.
    - Left for later slices:
      - the draft merge request (needs the forge client and its credential);
      - `linux` (needs `story:route-runs-at-linux-confinement` and the Loom request);
      - a hand-back that a bound program asks for through the run's token, which is how the bug-fix runbook's
        hand-back action would share this step;
      - a base a delegation may choose from a list of E;
      - a fetch before create.

## Acceptance (first slice, item 10)

With a local bare repository as `E.remote` (a `file://` URL), a stand-in `worktree` command first on `PATH` that
records its arguments and makes a linked worktree with `git worktree add`, a stand-in model program on the run's route
file, and no network beyond loopback:

- A delegation with `repository: "demo"` whose stand-in model writes `src/a.txt` in its cwd:
  - the run ends `delivered`;
  - the bare repository holds `refs/heads/runbook/<run-id>` with one commit on `E.base`, whose author and committer
    are `E.commit_*` and whose difference is `src/a.txt`;
  - the main checkout is unchanged, and the view `delegation-result` names the branch and the commit.
- The stand-in `worktree` saw `create` with `--id runbook-<run-id>` and the recorded base before the node started,
  and `finish` after the push.
- A delegation naming a repository that is not in `runbook.toml` is refused before a run exists, and `worktree` is
  never called.
- A run the stand-in model fails, and a run cancelled while the node runs, push nothing. Their T is kept, and
  `finish` is not called.
- A model that writes `.github/workflows/x.yml`, or replaces T's `.git` file, gets no push. The run's record names
  the refusal.
- The push credential (a token file read by the askpass) is in no argv, no event, no file of R or W, and no
  environment of the model's process.
- A delegation without `repository` runs as today, and `worktree` is never called.
- Each case runs under 5 s. No model call.

## Evidence

- The consumer's design, section 8, item G2 (read 2026-11-27).
- A delegation's agent reads and writes only its call directory and W (`driver/src/bin/sdc-node/call.rs:102-109`,
  `:428-431`).

## Relations

- Informed by:
  - `story:runner-worktree-step` (the worktree lifecycle);
  - `story:bugfix-hands-back-a-branch-and-a-merge-request` (one hand-back step for both, item 10);
  - `story:daemon-takes-a-delegation`;
  - `story:route-runs-at-linux-confinement` (item 7).
- The operator decides item 9 before this story is proposed.

## Decision

The operator's pick A, 2026-11-27 15:2x CET (relayed by the host's operator session): build the first slice only:

- the worktree made and finished by the daemon with the host's `worktree` CLI (design items 1, 8);
- the branch `runbook/<run-id>` from the entry's configured `base` (item 3);
- the daemon's commit under the entry's bot identity and the push of a new ref only: no force, no `+`, refused when a
  changed path is in `deny_paths` (CI files) (item 4);
- at confinement level `local` only, for repositories listed in `runbook.toml` (`[repositories.<name>]`, empty by
  default).

Not built; each comes back to the operator first:

- the draft merge request and any forge client or forge token;
- `linux` confinement and any Loom request;
- sharing the step with the bug-fix runbook;
- a base chosen by a delegation;
- a fetch before create.
