---
format: aep.planning-md/3
id: story:a-merge-cannot-move-unreleased-changelog-entries-into-a-released-section
kind: story
status: draft
title: A merge cannot move unreleased changelog entries into a released section
summary: The gate fails when a released section of CHANGELOG.md differs from that section at its tag.
revision: 1
---
## Outcome

A merge of the default branch into a unit branch can no longer leave entries of `## [Unreleased]`
inside a released section of `CHANGELOG.md` unnoticed: the gate fails.

## Source

- 2026-11-27: a unit branch merged the default branch after the 0.14.0 release; the merge applied
  without a conflict and placed two `[Unreleased]` entries under the released `## [0.14.0]`.
  Nothing caught it; the merge to main `e4ae61a6` corrected it by hand.
- The same defect after release 0.9.0 is why the implementors' rules ask for a `cmp` of the
  released sections after every merge of the default branch; that is an instruction, not a check.

## Acceptance

- A check in `task check` fails when any released section of `CHANGELOG.md` differs from that
  section at its release tag, and names the section and the first line that differs.
- The check passes on the current head and fails on a fixture where an entry was moved from
  `[Unreleased]` into the last released section.
