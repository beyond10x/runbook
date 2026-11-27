---
format: aep.planning-md/3
id: story:written-reader-chains-case-counts-every-link-under-load
kind: story
status: draft
title: The written-reader chains case counts 4000 link components in both answers under load
summary: 'written-reader.spec.mjs:237 expected 4000 resolved link components and got 3997 once: reproduce, then fix the test or show the product defect red first'
tags:
- backlog
revision: 1
---
## Context

The UI case `500 names of 8 link components that are chains of 37 links each are all listed, and an answer
resolves 8 components per name, whatever the load` (`ui/test/written-reader.spec.mjs:237-253`) writes 500
names of 8 `c0/` components (each `c0` a chain of 37 links ending at `.`), asks `/api/products` twice
(`written-reader.spec.mjs:244-245`) and expects each answer to have resolved `500 * 8` link components
(`written-reader.spec.mjs:250-251`). The count is `tally.links` (`ui/written.mjs:173`, `:196`), raised once
per link component in `describe` (`ui/written.mjs:208-228`, `:217`).

## Evidence

- Seen once in a `task check`: expected 4000, received 3997. The log of that run is not kept, so which of
  the two answers was short is not recorded.
- *Inferred* from the code: the first answer cannot be the short one, because `written-reader.spec.mjs:246`
  asserts first that it listed all 500 names, and a listed name has resolved all 8 of its components. So the
  second answer (`:251`) resolved 3 components fewer, which means at least one name stopped early in
  `describe`: `lstatSync` failed (`written.mjs:214`, answered `gone`) or `runPath` returned null
  (`ui/runfiles.mjs:45-55`, which turns every error, an `EMFILE` or `ENOMEM` among them, into null, answered
  `refused`).

## Acceptance

- A loop makes the failure happen on demand (the case repeated under the load of the whole UI step, or under
  an injected error in the second answer), shown red before any change.
- If the cause is in the test, the test is fixed or quarantined with its reason and a story that brings it
  back. If the cause is the product (an error on a name of the run answered as `gone` or `refused`), the
  product change comes with a deterministic red-first case that shows it.
- After the fix the case passes 50 of 50 runs under load; no timeout or bound is raised as the fix.
