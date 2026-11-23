---
format: aep.planning-md/3
id: story:inspector-dot-directories-by-real-path
kind: story
status: draft
title: The inspector refuses a dot-directory by the real path of what it opens
summary: 'Three warnings of security pass 5 that stay inside the run: the dot rule of lastError and artifactPath looks at the name, the planning route walks dot-named directories, and the inventory spec sees one spelling of a file read.'
relations:
- informed_by: story:inspector-run-answer-is-bounded
revision: 1
---
## Outcome

The inspector refuses a dot-directory by the real path of what it opens, on every read and every walk of a
run, and a test fails when a module of `ui/` gains a way to reach the file system that the checked open does
not cover.

## Source

Security review pass 5 of `story:inspector-shows-step-products` (2026-11-23,
`review-result:security-inspector-step-products-pass-5`, verdict green). Three warnings that stay inside the
run:

- `ui/decisions.mjs:159,166` (`lastError`, `artifactPath`): the refusal of a dot part looks at the name that
  was asked for. The guard the rule needs: the checked open refuses a real path that has a dot part. Older
  than the unit.
- `ui/server.mjs:624` (`workFacts`, the route of the planning files of `work/`): the walk enters dot-named
  sub-directories, against the rule the file route holds. Older than the unit.
- `ui/test/runfiles-inventory.spec.mjs:12`: the inventory of file reads counts one spelling of a call in
  `ui/*.mjs`; other ways to import or call the file system or a child process are not seen. Introduced by the
  unit.

The reviewer's three failing cases for the first warning are kept outside the repository until this story is
built (`<scratch>/ui-products/review-5/backlog-specs/security-products-5-lasterror.spec.mjs`).

## Scope

- `ui/runfiles.mjs` (the checked open: the dot rule on the real path), `ui/decisions.mjs` (`lastError`,
  `artifactPath`), `ui/server.mjs` (`workFacts`).
- `ui/test/runfiles-inventory.spec.mjs` and the three cases above.

## Acceptance

- A read of a run's file whose real path has a dot part is refused by the checked open, whatever name was
  asked for; the three kept cases pass.
- The route of the planning files of `work/` returns no file under a dot-named directory; if the one fixed
  planning directory stays readable, the route names it as its one exception and a test holds that no other
  dot-named directory is walked.
- The inventory spec holds an allow-list, per module of `ui/`, of the imports of `node:fs`, `node:fs/promises`
  and `node:child_process`, and fails on a dynamic import of them; a mutant that adds a direct open under
  another spelling turns it red.
- The inspector stays read-only: no route that acts on a run, no token field, no proxy.

## Not in this story

- Bounds of time and bytes per request (`story:inspector-run-answer-is-bounded`).
