---
format: aep.planning-md/3
id: story:model-call-does-not-copy-the-tree
kind: story
status: draft
title: A model call of a run works on the tree without a full copy in and out
summary: A model call on a tree of several thousand files costs no full copy per call and leaves no copy per call in the run directory; the bound of the pinned Loom on an artifact is named where a user sets the size.
relations:
- decomposes: epic:ticket-to-merge-request-on-a-real-repository
revision: 1
---
## What is known

Read from the tree at release 0.8.0: for each model call `sdc-node` copies the whole `impl/` tree into the
call's directory with no exclusion and no bound (`driver/src/bin/sdc-node/util.rs`, `cp_r`); the fix writer
gets two copies in, a delete and a copy back; every copy stays under `.sdc/llm/` of the run; each check run
adds one or two copies more. The seed refuses a ticket's tree above 2000 files or 16 MiB by default, with
the reason "every model call copies the tree in and out". The pinned Loom refuses an artifact above 64 MiB
or a tree above 10,000 entries. A consumer's repository had 4901 files and about 65 MB (2026-11-23).

## Outcome

A model call on a tree of several thousand files costs no full copy in and out: the call works on the tree
in place under the confinement, or on a copy made once per run and reused, or on a link tree; the story
that takes this measures the three on a tree of 5000 files and says which it took. No per-call copy stays
in the run directory after the call. Where a user sets a size bound, the docs name Loom's two bounds.

## What a guard must hold

A model call still cannot change a file outside the tree it was given, and a call that fails leaves the
tree as it was before the call.

## Acceptance

On a fixture tree of 5000 files a stub run of the bug-fix runbook writes less than twice the tree's size
into its run directory, measured in a case, and each model step's time on the stub is under 5 s.

## Origin

Pre-existing; measured for a consumer on 2026-11-23.
