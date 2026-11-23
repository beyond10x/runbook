---
format: aep.planning-md/3
id: review-result:security-inspector-step-products-pass-1
kind: review-result
status: active
title: 'Security review pass 1 of the inspector unit (step products) at a0f7ddc: red, 7 findings'
summary: Two blockers (a listing that follows links to its own directory; a Markdown table that makes one million nodes from 6 KB), three warnings, two notes; 26 of 33 reviewer specs pass.
relations:
- reviews: story:inspector-shows-step-products
revision: 1
---
verdict: NEEDS-CHANGE

unit: `story:inspector-shows-step-products`, `git diff 4acdbe4 a0f7ddc` (specs `1f2e585`, build `a0f7ddc`), tree `<trees>/b10x/runbook/runbook-ui-any-runbook`, branch `feat/inspector-step-products`
cases: executed 30→63, red 7 (30 is the unit's own specs on the `wide` project with my files deselected, run after mine existed; `daemon-ui.spec.mjs` was not run because it needs a cargo build)
origin: introduced 7 / pre-existing 0 / undecided 0 (two of the seven also have a pre-existing counterpart, noted below)
wrote-outside-worktree: 7 paths under `<scratch>/ui-products/review-1/`, plus git-ignored files inside the worktree (listed in 6)
needs-coordinator: none

`git --no-pager diff --stat` of the tree is empty. Nothing tracked changed; only the nine new `ui/test/security-products-*` files exist, untracked. No implementation file was touched.

## Invariants
1. **Run-path check: broken.**
   - Holds for:
     - `..`, encoded and double-encoded forms, absolute paths, backslashes, NUL.
     - Unicode look-alikes of `.` and `/`.
     - 5000-character and 3000-component paths.
     - A link to a directory outside the run.
     - Specs: `security-products-paths.spec.mjs` "inv 1" x2.
   - Broken: a time-of-check to time-of-use swap. `<trees>/ui/runfiles.mjs:87-91` stats the real path, then opens the same path string with `O_NOFOLLOW`, which guards only the last component. A directory on the way, swapped for a link that leaves the run, is followed.
   - Spec `security-products-race.spec.mjs` red: the text of a file outside the run was served after 23 to 28 reads.
   - Hard link to an outside file: served (200). A hard link is indistinguishable from a file of the run by path; only `st_nlink` shows it. Noted, not asserted.
2. **Regular files only: broken for one read.**
   - Holds for a FIFO, socket, directory and `/dev/null` link on `/api/file` and in the listing. Answered at once with 415 or 403 (`security-products-paths.spec.mjs` "inv 2").
   - Broken: `<trees>/ui/products.mjs:148` reads the run log with `readText` (`<trees>/ui/runfiles.mjs:40-44`), which calls `readFileSync` after the path check and has no type check. `security-products-runlog.spec.mjs` "inv 2" red: with a FIFO as the run log, `/api/output` does not answer in 4 s and the server then answers nothing (3 s) on `/api/meta`.
3. **Dot components: holds, except one route.**
   - Holds for:
     - No query parameter, header or view member turns the filter off (12 parameters, 5 headers, 4 view members).
     - Case, `.`, `//`, trailing-slash and encoding variants.
     - A plain-named link to a dot directory serves nothing and lists nothing (real path decides).
     - The listing leaks only the two counts.
     - Specs: `security-products-paths.spec.mjs` "inv 1" (link), "inv 3" x2.
   - Broken on `/api/output`: `<trees>/ui/products.mjs:162` has no dot check on the blob's real path. `security-products-output.spec.mjs` "inv 8 and 3" red: a `store/blobs/<64 hex>` link into `work/.cache` serves its text.
4. **Run content never becomes markup: holds for markup, broken for cost.**
   - Holds. `security-products-page.spec.mjs` (5 tests) opens hostile file names, an evidence id and fields, action output, JSON, YAML, Markdown and a 30-link scheme matrix; the DOM audit finds nothing.
     - No element that loads or runs, no `on*`/`src` attribute, no style except `--depth:N`, no unsafe `href`.
     - No script run, no request off the server. External links carry `target=_blank` and `rel="noopener noreferrer"`.
     - `linkOf` gives none, http(s) or an in-run path for every spelling tried.
     - Source greps find none of `innerHTML`, `outerHTML`, `insertAdjacentHTML`, `eval` or `new Function`, and no `url()` or content-driven class.
   - Broken, renderer cost, in `security-products-markdown-cost.spec.mjs`:
     - The heading regex `markdown.js:200`: `# a` followed by 131000 spaces and `b` took 7.7 s, then 16.3 s in the suite run. 65000 spaces took 1.9 s, so growth is quadratic.
     - Table amplification at `markdown.js:313-316`: a header of N columns and rows of a single `|` make N×rows cells. 6003 characters of input made 1,002,505 nodes.
     - Also quadratic, but 1.8 s at 131000 characters in the same run, so green: the trailing-whitespace replace at `markdown.js:329`.
5. **No route acts, guard covers all routes: holds.** `security-products-routes.spec.mjs`, 5 of 5.
   - 14 methods answer 405 on every new route and error path.
   - HEAD has no body.
   - 14 Host spellings and 7 Origins that only look like loopback answer 421, before the method, including the 404/400/403/415 file paths.
   - Odd request lines end in an answer.
6. **Size and type limits: broken for listing and run log.**
   - Holds:
     - A 2 GiB sparse `.md` is cut at 1 MiB by a bounded read; peak memory grew less than 64 MB.
     - `x.html` and `x.svg` are JSON with `nosniff`, no `Content-Disposition`, and run nothing even when navigated to.
     - 30000 files in one directory give 500 entries in under 5 s.
   - Broken, listing walk: `<trees>/ui/products.mjs:89` follows a directory link back into its own directory, and the 500-file cap counts files only. `security-products-listing.spec.mjs` red: eight links to `.` under `work/` left `/api/products` unanswered for 25 s. One thread blocks the whole inspector.
     - 20000 empty directories took 3.9 to 4.9 s, which passed barely. This is load-sensitive and not a finding.
   - Broken, run log in memory: `security-products-runlog.spec.mjs` "inv 6" red. A 150 MB run log grew the server's peak memory by 299 MB for one `/api/output` request, because `readText` reads it whole (`<trees>/ui/products.mjs:148`).
7. **`product` key: holds.**
   - `runbook check` (`<trees>/runbook/src/check.rs:393-417`) and the page both refuse a non-string, empty, absolute, `..`, NUL, backslash or dot-component value. I ran the built binary over 18 values with `RUNBOOK_HOME`; every refusal matched `ui/web/view.js` `productProblems`. A plain-named link into a dot directory, a FIFO, a directory or an outside link named as `product` is reported missing and not read.
8. **Blob digest from the log: holds.**
   - A digest of `../…` or with `/`, a 64-character non-hex value, a FIFO blob, a link to an outside file, a `blobs/` directory that is a link, and a run log that is a link all give `recorded:false` with no read and no hang (`<trees>/ui/products.mjs:161`, regex `^[0-9a-f]{64}$`).
   - The FIFO-as-run-log exception is invariant 2, and the dot-link blob exception is invariant 3.
9. **Committed files: holds.** The diff has no `/home`, UUID, token or key text, and the only `@1` protocol id is `example@1` (5 times); the one `runs/` mention is prose in the story.

## Answer to 7
`c68ca14` and `0.5.0` accept a view file with the member `product` in `runbook check`; that check does not refuse it.
- `<trees>/runbook/src/check.rs:400-405` at both: `match o.get("format").and_then(Value::as_str) { Some(VIEW_FORMAT) => {} …`. Only `format` is read, so any other member passes.
- The inspector at `c68ca14` does refuse it. `ui/web/view.js:76` at `c68ca14`: `if (!(k in s.members)) out.push(… view member "${k}" is not known to the inspector …)`, and `ui/server.mjs:65-66` exits 2 on any problem.

## Judgement findings (covering tree `a0f7ddc`)

| file:line | measured | reaches it |
|---|---|---|
| `ui/runfiles.mjs:91` | TOCTOU spec red | a program the run bound, running while the inspector reads. It can only show the outside file to the operator's screen. Pre-existing pattern in `readText`; the new route exposes it. |
| `ui/products.mjs:89` | 8 self-links hang the server for 25 s or more | any writer of `work/`, with `ln -s . a` eight times. |
| `ui/web/markdown.js:316` | 6 KB of input makes 1M nodes | any Markdown file in a run, opened in the page. |
| `ui/web/markdown.js:200` | 7.7 s to 16.3 s at 131000 spaces | same. Absolute time is load-sensitive, the 4x growth for 2x size is not. |
| `ui/products.mjs:148` | FIFO hangs the server; 150 MB log costs 299 MB | nothing found that lets a program of the run write `store/`. The unbounded read is real for a long genuine run log. |
| `ui/products.mjs:162` | dot-linked blob served | nothing found that creates a link in `store/blobs/`. `INFEASIBLE`; one-line change: apply `hasDotPart(runRel(...))` as `fileAnswer` does. |
| `ui/products.mjs:49-53`, `ui/server.mjs:951` | `/api/file` serves any non-dot file of the run (`settings.json`, `store/*`, `llm-calls.jsonl`), not only `work/` | from code reading only (step 2). The story says files under `work/`; its safety line says under the run. A note, not asserted. |

Missing guards:
- Re-check the opened descriptor's real path, or open without following intermediate links.
- Track visited real directories and cap directories as well as files.
- Bound the table work by rows and columns, and anchor or atomise the heading and trailing-whitespace regexes.
- Read the run log through `readRegular` with a limit.

Not checked:
- The Explain panel's `innerHTML` with a hostile evidence id (read only: `esc` escapes `& < > "`, attributes are double-quoted, `seq` is numeric).
- Deeply nested evidence JSON (stack depth in `nested()`).
- `ui/daemon` CSP behaviour.

## What I could not fault
- Dot filter by switch, by encoding and by plain-named link, apart from the output route.
- 405/421 on all new routes and error paths.
- Header and nosniff rules.
- Digest validation.
- View `product` parity between Rust and JS.
- Link scheme handling.
- No network at render.
- Fixture hygiene.

## Spec files (unit's own 4 specs: 30 passed)
Combined run of `security-products-*`, project `wide`: 33 tests, 26 passed, 7 failed.
- `security-products-paths.spec.mjs`: 8/8 pass.
- `security-products-routes.spec.mjs`: 5/5 pass.
- `security-products-page.spec.mjs`: 5/5 pass.
- `security-products-output.spec.mjs`: 3 pass, 1 red (dot-linked blob).
- `security-products-listing.spec.mjs`: 2 pass, 1 red (self-links).
- `security-products-markdown-cost.spec.mjs`: 3 pass, 2 red (heading regex, table amplification).
- `security-products-runlog.spec.mjs`: 0/2 (FIFO hang, 150 MB log).
- `security-products-race.spec.mjs`: 0/1 (TOCTOU).
- `security-products-lib.mjs`: helper, not a spec.

Each file was first run alone as written, with the same failures. Load average while they ran: 24 to 69, mostly 40 to 62. Per-test time stayed under 60 s; the slowest was 25.6 s (the self-link case, a hung listing cut at 25 s).

One extra test I wrote, a note on `/api/file` serving files outside `work/`, never ran: the pool job sat queued on low disk and I cancelled it, then removed the test from the file. The `/api/file` finding above rests on code reading only.

## Paths written outside the worktree
- `<scratch>/ui-products/review-1/cases.patch`
- `<scratch>/ui-products/review-1/make-patch.sh`
- `<scratch>/ui-products/review-1/suite.out`
- `<scratch>/ui-products/review-1/probe/re.mjs`
- `<scratch>/ui-products/review-1/probe/sock.mjs`
- `<scratch>/ui-products/review-1/probe/check-views.sh`
- `<scratch>/ui-products/review-1/probe/checkroot/`

Inside the worktree, all git-ignored: `<trees>/ui/test/fixtures/runs/security-products-*` (includes sparse 2 GiB and 150 MB files) and `<trees>/ui/node_modules/.pw-results`.

findings: [{"file":"ui/runfiles.mjs","line":91,"category":"concurrency","severity":"warning","verdict":"NEEDS-CHANGE","origin":"introduced","message":"readRegular opens the checked real-path string with O_NOFOLLOW (last component only), so a directory swapped for a link between check and open is followed and an outside file is served (security-products-race.spec.mjs red); the same pattern pre-exists in readText."},{"file":"ui/products.mjs","line":89,"category":"boundary","severity":"blocker","verdict":"NEEDS-CHANGE","origin":"introduced","message":"listFiles follows a link back to its own directory and the 500 cap counts files only, so eight self-links under work/ leave /api/products unanswered for over 25 s and block the whole inspector (security-products-listing.spec.mjs red)."},{"file":"ui/web/markdown.js","line":316,"category":"boundary","severity":"blocker","verdict":"NEEDS-CHANGE","origin":"introduced","message":"table rows create one cell per header column regardless of the row's own cells, so 6 KB of Markdown makes 1,002,505 nodes (security-products-markdown-cost.spec.mjs red)."},{"file":"ui/web/markdown.js","line":200,"category":"boundary","severity":"warning","verdict":"NEEDS-CHANGE","origin":"introduced","message":"the HEADING regex is quadratic on a long whitespace run: 65000 spaces 1.9 s, 131000 spaces 7.7 s to 16.3 s under load (security-products-markdown-cost.spec.mjs red); the trailing-whitespace replace at :329 has the same shape but measured 1.8 s."},{"file":"ui/products.mjs","line":148,"category":"integrity","severity":"warning","verdict":"NEEDS-CHANGE","origin":"introduced","message":"outputOf reads Loom's run log with readText (readFileSync after the path check): a named pipe in its place hangs the server and a 150 MB log grows peak memory by 299 MB for one request (security-products-runlog.spec.mjs red x2); nothing found that lets a program of the run write store/."},{"file":"ui/products.mjs","line":162,"category":"integrity","severity":"note","verdict":"INFEASIBLE","origin":"introduced","message":"the output blob read has no dot-component check on its real path, so a store/blobs/<hex> link into work/.cache is served (security-products-output.spec.mjs red); I built that state and found nothing that creates such a link."},{"file":"ui/products.mjs","line":49,"category":"judgement","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"GET /api/file serves any non-dot file of the run (settings.json, store/*, llm-calls.jsonl), not only work/ as the story says; found by code reading, not by a spec."}]
