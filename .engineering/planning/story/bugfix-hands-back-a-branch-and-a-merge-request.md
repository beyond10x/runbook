---
format: aep.planning-md/3
id: story:bugfix-hands-back-a-branch-and-a-merge-request
kind: story
status: active
title: A fixed bug-fix run hands back a branch, a commit and a merge request
summary: One tool of this repository takes work/impl of a run that ended fixed to a branch, a commit and a merge request on the forge of the repository, behind an approval.
relations:
- decomposes: epic:ticket-to-merge-request-on-a-real-repository
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-11-25T12:41:40Z", actor: "agent:runbook", revision: 2}
- {from: "proposed", to: "active", at: "2026-11-25T12:41:40Z", actor: "agent:runbook", revision: 3}
---
## What is known

A bug-fix run that ends `fixed` leaves the changed tree under `work/impl` of its run directory;
`docs/bugfix.md` lists the hand-back under Limits. A consumer applied the difference by hand in a checkout
(2026-11-23). Another runbook of the same consumer has a tool of its own for the same step (a branch, a
commit and a request on a forge, with an https address and a credential helper, because a bound program has
no ssh agent). Two copies of that step are what the operator's word of that day rules out.

## Outcome

One tool under `bindings/tools/` takes the tree of a run that ended `fixed` to a branch from the base
commit, one commit and a merge request on the forge of the repository. It is an action of the bug-fix
runbook behind an approval (a person, or a rule at the autonomy level the run names), and other runbooks
bind the same tool. The forge call goes through the `connectors` command or the forge's own command the
host has; the tool holds no token.

## What a guard must hold

- Nothing is pushed before the approval is recorded, and the approval is never recorded under a person's
  name by a rule or a model.
- The branch name and the commit come from the run's own records; no text of a model reaches an argv
  unquoted.
- A token or a login is never in an argv, a log line or the run directory.
- The tool never pushes to the default branch and never forces.

## Acceptance

A case with a local bare repository as the remote and a stand-in forge command shows the branch, the one
commit with the difference of `work/impl` against the base, the recorded request call, and nothing pushed
when the approval is refused.

## Origin

A gap named by a consumer on 2026-11-23; a limit of `docs/bugfix.md`.
