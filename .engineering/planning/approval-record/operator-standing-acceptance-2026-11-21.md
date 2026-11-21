---
format: aep.planning-md/3
id: approval-record:operator-standing-acceptance-2026-11-21
kind: approval-record
status: draft
title: 'Operator standing rule: drafted epics and stories are accepted (2026-11-21)'
tags:
- operator-decision
- session-sdc
- standing
relations:
- decides: epic:conductor-succession
- decides: story:authority-revision-binding-uptake
- decides: story:change-merge-git
- decides: story:clean-unattended-run-compare
- decides: story:confined-bound-commands
- decides: story:conformance-against-implementation
- decides: story:decider-finding-decision
- decides: story:forge-ci-evidence
- decides: story:forge-pr-evidence
- decides: story:forge-release-evidence
- decides: story:independent-reviews
- decides: story:model-per-node-variable
- decides: story:operating-record-report
- decides: story:pilot-real-repository
- decides: story:project-rename
- decides: story:release-tag
- decides: story:resume-cut-step
- decides: story:sdc-on-loom-tree-run
- decides: story:story-branch
- decides: story:takt-freeze-notice
- decides: story:verifier-independence
revision: 1
---
# Operator standing acceptance of drafted epics and stories

- Statement, verbatim: "Just note it down as a standing rule that all the stories you're basically
  creating here are accepted, and all the ones you're ad hoc sending to the Loom or the Canon
  session, they should be also automatically accepted by default, because we don't want to wait on
  this at all."
- By: the operator. Session: `sdc`. Date: 2026-11-21. Standing until the operator revokes it.
- Scope: every epic and story drafted in this store from the operator's requirements is accepted
  without a further word from him. Accepted is `proposed` in this store (draft → proposed →
  active); new artifacts are created and moved to `proposed` in the same step.
- Still required before a story is implemented: one acceptance statement, declared edges, a scope.
- Not covered: upstream blockers stay blockers; approval records are unchanged; nothing moves to
  `active` under this record.
- Applied on 2026-11-21 to every epic and story then in draft (the `decides` edges of this record).
