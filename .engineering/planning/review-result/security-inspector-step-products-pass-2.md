---
format: aep.planning-md/3
id: review-result:security-inspector-step-products-pass-2
kind: review-result
status: active
title: 'Security review pass 2 of the inspector unit (step products) at d0d16777: red, 2 open from pass 1, 8 new'
summary: 'Findings 2 to 6 of pass 1 hold. Open: the check and the read use two real paths (1), the root of the file route is not held to the work directory of the run (7). New: three warnings (the blob route and dot-directories, the blob route reads without a size limit, a listing that leaves files out without saying so) and five notes; 12 of 21 new tests pass.'
relations:
- reviews: story:inspector-shows-step-products
revision: 1
---
verdict: NEEDS-CHANGE

unit: `story:inspector-shows-step-products` at `d0d16777` (specs `da10cabc`, guards `d0d16777`), tree `<trees>/b10x/runbook/runbook-ui-any-runbook`, branch `feat/inspector-step-products`

cases: 21 new tests in four spec files (`ui/test/security-products-2-{race,blob,roots,markdown}.spec.mjs`), 12 passed and 9 failed. The 33 tests of pass 1 all pass on this head. Load average at the runs: 88 to 173.

origin: introduced 3 (N3, N4, N5) / pre-existing 2 (N1, N2) / open from pass 1: 2 (findings 1 and 7, each for one input)

## Findings of pass 1

1. Check and read use two real paths: open, warning. The check "under `work/`" is made on one real path (`ui/products.mjs:72-75`) and the read computes the real path again (`ui/runfiles.mjs:115`); the descriptor is compared with the second path only. Missing guard: the descriptor's real path is checked against the roots too. The case of pass 1 (a link that leaves the run) holds.
2. Self-link loop in the listing: holds (`ui/products.mjs:105-135`).
3. Table amplification: holds (`ui/web/markdown.js:353-381`); at most 13,107 nodes, under 2 s.
4. Quadratic heading and whitespace: holds (`markdown.js:240-262`); each shape under 5 s.
5. Run log read whole: holds (`ui/products.mjs:157-182`).
6. Dot-linked blob: holds (`ui/products.mjs:201-211`).
7. The file route is confined to `work/`: open for one input. Missing guard: the real path of the root must equal `<run>/work` (`ui/products.mjs:70-75`, roots from `ui/server.mjs:665`). Two specs are red.

## New findings

- N1, warning, pre-existing: the blob route (`ui/server.mjs:974`) answers for a `case` value with a dot-directory component. The rule "a dot-directory path is neither listed nor served" holds on the file route, the listing and the output route, and not here.
- N2, warning, pre-existing: the blob route reads with `readText` (`ui/runfiles.mjs:62-73`), which has no size limit and no link-count check. A 300 MB blob raised the server's peak memory by 595 MiB in 8.5 s. The file route and the output route refuse the same file. The other `readText` reads of the server (events, call records, run log, traces) are unbounded too and were not run.
- N3, warning, introduced: when every file of `work/` has two names, the listing answers with 0 files, `truncated false` and `cut null` (`ui/runfiles.mjs:130`, `ui/products.mjs:126`), and by reading the page names a product that the file route then refuses (`ui/products.mjs:150`). Missing: a count of the files left out, or a refusal the page shows.
- N4, note, introduced: a file deeper than `MAX_DEPTH` 6 is left out with `truncated false` (`ui/products.mjs:123`).
- N5, note, introduced: `list()` (`markdown.js:303`) makes one item node per line without the node bound: 65,001 nodes against a stated 30,000. Bounded by the 131,072-character limit.
- N6, note, not run: `ui/runfiles.mjs:51` falls back to a real path made from the path text when the descriptor's path cannot be read; the guard of finding 1 then does not hold.
- N7, note: a directory is read whole before the entry cap counts (`ui/products.mjs:107-109`).
- N8, note: a blob is refused when `blobs/` is a link inside the run (`ui/products.mjs:208-210`); no evidence that a run is laid out so.

Refusal texts do not tell an existing path from a missing one (same 403, same 404 text).

## Rule set

- GET and HEAD only, 405 and 421 on other methods and a wrong Host or Origin: holds (5 route specs).
- No route acts on a run: holds.
- A dot-directory path is neither listed nor served: holds except on the blob route (N1).
- No run content as markup: holds; no `innerHTML`, `document.write`, `eval` or `new Function` in `markdown.js` or `products.js`.
- No library from a network address: holds. No content of `runs/` in a fixture: holds.

## Load and bounds

The Playwright step reached its 120 s limit twice at a load of 150; the specs that had not run passed on a rerun. One test of the Markdown battery took 56 s against the 60 s bound.

## Not checked

The failing `lstat` and `readdir` branches of the listing (read, not run); a host without `/proc`; `ui/README.md`; the size of the other `readText` reads.

wrote-outside-worktree: `<scratch>/ui-products/review-2/`
needs-coordinator: no
