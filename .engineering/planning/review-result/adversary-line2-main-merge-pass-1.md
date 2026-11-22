---
format: aep.planning-md/3
id: review-result:adversary-line2-main-merge-pass-1
kind: review-result
status: active
title: 'Narrow check of the merge of main into line 2 at 3bac01d: red, 1 note'
summary: 'The reads main brought go through the run-path check, yq is started without a shell, the guards sit in front of every route; 1 note: a named pipe in the protocols directory of a run makes the server wait 10 s per pipe (code of main).'
tags:
- wave-2026-11-22-w1
relations:
- reviews: story:driver-runtime-removed
revision: 1
---
unit: merge of main into line 2 at 3bac01d, narrow check
verdict: red
cases: executed 5→6 (adv4-merge 4 tests, adv4-fifo 1 test), red 1
origin: introduced 1 / pre-existing 0 / undecided 0
wrote-outside-worktree: `<scratch>/removal/line2/adversary-3/pass-4/` only
needs-coordinator: no

1. **Reads go through `runPath`: green.** In `<run>/protocols/` and `<run>/bindings/`, a link to a file outside the run, a relative link to another run's copy, a whole directory that is a link, and a link-out `events.jsonl` all served nothing from outside. Eight routes were checked, `/api/tree` among them. The sentinel text `LEAK` appeared in no body except the run that owns it. A link-out `events.jsonl` with a `case.json` gives no case protocol from it. Names with `..` such as `..x.protocol.yaml` and `a..b.yaml` are plain names. A `/` cannot occur in a directory entry.
2. **Per-run catch: green for stopping, red for waiting (finding 1).** I used these broken copies:
   - non-YAML text;
   - YAML that is a list, `null` or `5`;
   - YAML with odd shapes (`actions: [null, 5, ...]`, an action named `__proto__`);
   - a 2 MB YAML;
   - a directory in place of a file;
   - dangling and looping links;
   - NUL bytes;
   - a run's copies turned into garbage while the server ran;
   - a new broken run appearing after start;
   - `yq` missing from `PATH`.

   The server stayed up at start and over two rescans in each case. The good runs were served with `parts` `copy`, and the broken runs say `unavailable` with the file names. One caveat: a FIFO named `*.protocol.yaml` makes `/api/tree` wait 10.07 s, because `yq` is started with `execFileSync` and blocks the event loop until its 10 s timeout.
3. **`yq` argv: green.** `decisions.mjs:50` runs `execFileSync('yq', ['-o=json', '.', file])`, with no shell. The file is the realpath, which is absolute. I tried names `-n.protocol.yaml`, `--version.yaml`, `-o=json.yaml`, `a b`, `q'q`, `q"q`, `$(touch PWNED)`, a name with `;`, and one with backticks. No `PWNED` file appeared anywhere, and the run was served with both copies read.
4. **Guards in front: green.** Eighteen paths (`/`, the `/api/*` routes, `/events`, `/decide.js`, `/app.js`, `/api/nonsense` and an encoded `..`) each got 421 for five wrong Host/Origin pairs. `/decide.js`, `/web/decide.js` and `/daemon/decide.js` answer 404 with the right Host. No file under `ui/web/` holds `answer-decision`, `Bearer`, `dd-token` or `/daemon/commands`. The merge's diff of `server.mjs` adds no route.

The cases leave the tree as `<scratch>/removal/line2/adversary-3/pass-4/cases.patch` (215 lines, two new spec files only). `git apply --check` passed. After removal, `git status --porcelain` and the ignored-file listing are empty. The tree is on `impl/loom-line-2` at `3bac01d`. The extra probes are in the same directory: `extra.sh` (no `yq`, FIFO) and `probe.mjs`.

```findings
[{"file":"ui/decisions.mjs","line":50,"category":"availability","severity":"note","verdict":"NEEDS-CHANGE","origin":"introduced","message":"New with the merge: ui/runfacts.mjs:28-29 hands each protocols/ and bindings/ copy to yamlJson, which runs yq with execFileSync. A FIFO named x.protocol.yaml in <run>/protocols/ passes runPath (its realpath is inside the run), so yq blocks on open until its 10 s timeout, and the whole server waits. /api/tree took 10.07 s (case ui/test/adv4-fifo.spec.mjs, expected < 3 s). The wait repeats once per FIFO and per change of the run's file signature; the process stays up. Fix: runFacts and sigOf read only regular files (statOf(...).isFile()) and skip the rest as unreadable, or run yq with the async execFile. Pre-existing and out of scope: readText/readJson (readFileSync) on a FIFO named like a run file blocks without a timeout."}]
```
