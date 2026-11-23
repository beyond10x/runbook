---
format: aep.planning-md/3
id: epic:ticket-to-merge-request-on-a-real-repository
kind: epic
status: draft
title: A bug ticket becomes a merge request on a real repository, with the runbook's own tools
summary: The bug-fix runbook takes a ticket of a tracker to a merge request on a repository that is not Rust and not small, with tools that live in this repository once. Seven gaps from a consumer's first such run.
revision: 1
---
## Why

A consumer ran the bug-fix runbook of release 0.8.0 for a real ticket on two repositories that are not
Rust (2026-11-23). It met seven gaps and filled each with a stand-in in its own scratch directory. The
operator's word to that session the same day: no custom runtimes and no second set of core tools; core
tools and the core driver live in one central place, and the runbook is driven by Loom throughout. So each
gap is closed here, once, and a consumer carries only what is its own: its repository's check commands.

## Outcome

`runbook run --bugfix-ticket <key>` on a repository of a consumer ends with a merge request a person
reviews, with no stand-in on the consumer's side:

1. The model programs reach the gateway the host names (`story:model-programs-reach-a-gateway`).
2. A check command names its pool time, and the run takes a checks file and the seed's size bounds
   (`story:bugfix-check-names-its-pool-time`).
3. A writer is sent back on Loom before the case ends (`story:bugfix-writer-is-sent-back-on-loom`).
4. The ticket is fetched through the connectors command (`story:bugfix-ticket-is-fetched-through-connectors`).
5. A fixed run hands back a branch, a commit and a merge request
   (`story:bugfix-hands-back-a-branch-and-a-merge-request`).
6. A model call does not copy the tree (`story:model-call-does-not-copy-the-tree`).

What stays with a consumer: the check commands of its repository in `.runbook/checks.yaml` (the documented
place); no detection rows for its build tools are added here.

## Acceptance

A run on a fixture repository without `Cargo.toml`, with a stand-in tracker, a stand-in forge and the stub
of the model program, goes from a ticket key to a recorded merge request call, in the cycles suite, with no
tool outside this repository on its `PATH` except the stand-ins.

## Order

After the consumer's open gaps of the daemon, unless the operator orders otherwise. Items 1 and 2 first:
they stopped the first real run.
