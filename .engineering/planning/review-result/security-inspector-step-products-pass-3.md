---
format: aep.planning-md/3
id: review-result:security-inspector-step-products-pass-3
kind: review-result
status: active
title: 'Security review pass 3 of the inspector unit (step products) at a4b4d2f5: red, pass 2 fixed, 3 older warnings, 4 notes'
summary: 'Findings 1, 7 and N1 to N7 of pass 2 are fixed (N7 by reading only). New: three warnings in code older than the unit (readText accepts a file with two names; /api/run lists llm/ once per line of the call log; the 64 MiB limit is per file) and four notes; 10 of 13 new tests pass.'
relations:
- reviews: story:inspector-shows-step-products
revision: 1
---
verdict: NEEDS-CHANGE

unit: `story:inspector-shows-step-products` at `a4b4d2f5` (specs `ba6e1532`, guards `a4b4d2f5`), tree `<trees>/b10x/runbook/runbook-ui-any-runbook`, branch `feat/inspector-step-products`

cases: 13 new tests in two spec files (`ui/test/security-products-3-{roots,reads}.spec.mjs`), 10 passed and 3 failed. Every finding of pass 2 is fixed; each guard went red under mutation except N7, which no spec holds. Load average at the runs: 95 to 182.

origin: introduced 3 notes / pre-existing 3 warnings and 1 note

coordinator: the refusal of a file with more than one name in `readText`, a spec for N7, the cleanup of the listing spec and the message for a link into a dot-directory go back to the implementor (fix round 3). The two bounds of `/api/run` and the confinement of `fin.log` to `blobs/` are older than the unit and are filed as a backlog draft.

**New specs.** These are untracked: `ui/test/security-products-3-roots.spec.mjs` and `ui/test/security-products-3-reads.spec.mjs`. Of 13 tests, 10 passed and 3 failed.
- `3-roots`: 9 tests, 9 passed.
- `3-reads`: 4 tests, 1 passed (the control), 3 failed.

I never saw 3-roots all green in one run of its own. In the combined run the 2-markdown and 3-roots files gave `21 passed (1.1m)`. An earlier 3-roots run had two red tests, both mistakes in my own assertions, which I corrected.

**Earlier specs on the fixed tree.** All pass:
- 2-roots and 2-blob (8 tests, in one run with 3-roots).
- First-pass paths, output, runlog and routes: `19 passed (1.1m)`.
- Listing: `3 passed`. Markdown-cost: `16 passed`. Page: `5 passed`. `step-products-bounds`: `9 passed`.
- The race specs and `runfiles-fail-closed` passed in one run that then hit the 120 s limit; 9 of its 15 tests ran. `race 2` read 2200 times and `race` 5200 times, with no text served.

**Earlier findings.** Each fixed item was checked by mutating the guard in place and restoring it. The `sha256` of `products.mjs`, `runfiles.mjs`, `server.mjs`, `web/markdown.js` and `web/products.js` matches the start state.

| item | state | evidence |
|---|---|---|
| Finding 1 | fixed | Dropping `accept:` from `fileAnswer` turns `2-race` red after 5 reads (store text served). `runfiles-fail-closed` stays green. |
| Finding 7 | fixed | Removing the real-`work` root check turns both `2-roots` finding-7 tests red, and `3-roots` (a) `store/work` red. A `work` linked to another run's `work` stays refused because `runPath` returns null. |
| N1 | fixed | Dropping the `accept` on the blob route turns the dot tests red (`2-blob`, `3-roots` (d)). Spellings `.cache`, `./.cache` and `.cache/` are all refused. |
| N2 | fixed for `/api/blob` only | Raising the limit to 400 MiB gives memory +898 MiB, red. Disabling the `nlink` refusal turns the hard-link and in-run-link tests red. Other reads are open: see new finding 1. |
| N3 | fixed | Removing the `leftOut++` lines turns the `cp -al` test red. |
| N4 | fixed | Removing `deeper = true` turns the depth test red. |
| N5 | fixed | Removing `if (over()) break;` gives 65001 nodes, red. |
| N6 | fixed | Making `nameOf(fd) ?? real` fail open turns "a descriptor that the system cannot name is not read" red. |
| N7 | fixed by reading only | `opendirSync` stops at `MAX_ENTRIES - entries` (`products.mjs:145`). No spec of mine or the implementor's goes red when it is removed, and I did not mutate it. |

**Attack on what round 2 added**
- A root that is a link, `work` linked to another run's `work`, a case `work` linked to the run `work`, and a `work` linked to `store/work`: all hold.
- A descriptor opened before a rename: the name is read from `/proc` after the open, so a rename before the open fails the check. A rename after the check cannot change the bytes checked. Held by reading and by the race specs; no dedicated spec.
- Blob route path check and bounded reader: hold.
- Left-out count: holds. It is a number and names nothing under a dot-directory (`3-roots` (c)).
- Fail-closed branch: holds (file route 403, `readText` null).
- Your specs: assertions are unchanged (`git diff ba6e1532 a4b4d2f5 -- ui/test`).
  - Race deadlines went from 25 s to 10 s and from 20 s to 10 s. The race hit in 5 reads under mutation, so 10 s still detects it.
  - The Markdown battery is split into one test per shape.
  - Large fixtures are built with `xargs`.

**The 64 MiB reads.** One of them stalls the server, and one reads outside the run through a second name. A third reads a dot-directory by reading only (new finding 4).
- `/api/run` is the stall. A `llm-calls.jsonl` of 1500 lines next to an `llm/` of 300 directories gave no answer in 55 s, and a small request waited 55060 ms. The cause is `listDir(llm)` per line (`server.mjs:805`). Eight 60 MiB files in `traces/` held a small request 7606 ms (`server.mjs:811`): the limit is per file, not per request.
- A second name of a file outside the run is read through `console.txt` and `expected-llm-calls.txt` (`server.mjs:813,815`).
- By reading, not tested: `run.log` in `/api/run` makes one object per `$ ` line, so 64 MiB gives millions of objects. `yamlJson` runs `yq` synchronously for up to 10 s on a path it opens again (`decisions.mjs:50`).

**New findings**

| severity | file:line | origin | what |
|---|---|---|---|
| warning | `ui/runfiles.mjs:62-80`, `ui/server.mjs:813,815` | pre-existing | `readText` has no link-count refusal. N2's fix covers only the blob route. Spec `3-reads` "(a) a second name of a file outside the run is not read" is red: the outside text is in the `/api/run` answer. Needs `nlink > 1` refused in `readText`. |
| warning | `ui/server.mjs:805` | pre-existing | `/api/run` calls `listDir(llm)` once per line of `llm-calls.jsonl`, so it is quadratic. The server answers nothing for 55 s (spec `3-reads` "llm-calls.jsonl of 1500 lines", red). |
| warning | `ui/server.mjs:811`, `ui/runfiles.mjs:276` | pre-existing | The 64 MiB limit is per file; the number of files is unbounded. Eight `traces/*.jsonl` of 60 MiB: small request waited 7606 ms (spec `3-reads` "eight files of 60 MiB", red). `events.jsonl` per case costs the same by reading. |
| note | `ui/decisions.mjs:157` | pre-existing | Not tested. `fin.log` comes from a model-written event and is joined to `blobs/`. `readText` confines it to the run but not to `blobs/` or away from dot-directories, so the last line of a file under a dot-directory can be shown. |
| note | `ui/test/security-products-listing.spec.mjs` | introduced | The listing spec leaves about 43k entries under `ui/test/fixtures/`, which Playwright walks at every start. With the stale fixtures present, the combined listing and markdown-cost run timed out at 120 s with 6 of 19 tests run (load 125 to 175). The same files ran in 27 s and 28 s after deleting them. |
| note | `ui/products.mjs:76-77` | introduced | A link into a dot-directory gets "a name that starts with a dot is neither listed nor served" for a request that names no dot. This tells the reader the link target is a dot-path. The name stays hidden. |
| note | `ui/products.mjs` (`productOf`) | introduced | A view's product note says "this run has not written it" or "the inspector does not serve it", which tells whether the path exists outside `work/`. The view file is trusted, so this is minor. |

The load average ranged from 95 to 182, and one Playwright start took about 70 s.

`runbook/target` existed at the end, so I ran `cargo clean` in `runbook/` (1.2 GiB removed). My large fixtures are removed. Nothing is committed and nothing was written under `.engineering/`.

```findings
[
 {"file":"ui/runfiles.mjs","line":62,"category":"read-outside-run","severity":"warning","verdict":"CONFIRMED","origin":"pre-existing","message":"readText has no link-count refusal; a second name of a file outside the run in console.txt or expected-llm-calls.txt is returned by /api/run. N2 fixed for /api/blob only. Spec security-products-3-reads (a) red."},
 {"file":"ui/server.mjs","line":805,"category":"denial-of-service","severity":"warning","verdict":"CONFIRMED","origin":"pre-existing","message":"/api/run lists llm/ once per line of llm-calls.jsonl: 1500 lines x 300 directories gave no answer in 55 s. Spec security-products-3-reads red."},
 {"file":"ui/server.mjs","line":811,"category":"denial-of-service","severity":"warning","verdict":"CONFIRMED","origin":"pre-existing","message":"The 64 MiB readText limit is per file and the number of files is unbounded: 8 x 60 MiB in traces/ held a small request 7606 ms. Spec security-products-3-reads red."},
 {"file":"ui/decisions.mjs","line":157,"category":"dot-directory","severity":"note","verdict":"CONFIRMED","origin":"pre-existing","message":"Not tested. fin.log from a model-written event is joined to blobs/ and read by readText; it is confined to the run but not to blobs/ and not away from dot-directories; the last line is shown."},
 {"file":"ui/test/security-products-listing.spec.mjs","category":"test-budget","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"The spec leaves about 43k entries under ui/test/fixtures that Playwright walks at each start; a run that included them timed out at 120 s, the same files ran in 27 s after deleting them."},
 {"file":"ui/products.mjs","line":76,"category":"information","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"A link into a dot-directory is refused with the dot-name message although the request names no dot; this tells the reader what kind of path the link targets, not its name."},
 {"file":"ui/products.mjs","category":"information","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"productOf's note for a view product says whether the named path exists outside work/; the view file is trusted, so low value."}
]
```
