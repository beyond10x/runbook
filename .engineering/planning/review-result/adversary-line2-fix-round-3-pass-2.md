---
format: aep.planning-md/3
id: review-result:adversary-line2-fix-round-3-pass-2
kind: review-result
status: active
title: 'Adversary pass 2 on line 2 fix round 3 (units 1 and 2) at 3dd6c7f: red, 1 pre-existing finding'
summary: 'Second attack on fix/line2-review-3: findings 1, 3, 4, 5, 6 and 7 of pass 1 fixed; 1 pre-existing warning (a null line in events.jsonl stops the inspector in its timer scan, outside the request guard).'
tags:
- wave-2026-11-22-w1
relations:
- reviews: story:driver-runtime-removed
revision: 1
---
unit: units 1 and 2 of line 2 at 3dd6c7f, pass 2
verdict: red
cases: executed 0→1, red 1
origin: introduced 0 / pre-existing 1 / undecided 0
wrote-outside-worktree: `<scratch>/` only (see 6)
needs-coordinator: no

Six of the seven pass-1 findings (1, 3, 4, 5, 6, 7) are fixed at `3dd6c7f`. Finding 2 is left out, as you said. One new finding is open, and it is on `main` too.

## 1. Diff stat

```
 ui/test/adv2-hostile-run.spec.mjs | 31 +++++++++++++++++++++++++++++++
 1 file changed, 31 insertions(+)
```

The only path is a test file. `git diff --stat -- '*/src/*' ui/server.mjs ui/web ui/runfiles.mjs` printed nothing. I used no mutants.

## 2. Case and its red output

**`ui/test/adv2-hostile-run.spec.mjs`**: a run whose `events.jsonl` holds the line `null` (valid JSON, not an event). I run the case alone, `--project=wide`: 1 test, 2.6 s, red.
```
Error: the server did not stay up: server.mjs exited with 1 before it listened: file://<trees>/b10x/runbook/line2/ui/server.mjs:329
TypeError: Cannot read properties of null (reading 'seq')
```
The cause is `ui/server.mjs:329`, in `observe`, which is called from `tick()`. It reads `evs[evs.length - 1].seq`. `tick()` runs once before `listen` and then on a timer. The new request guard does not cover it.

What reaches it: any run directory whose event log holds a `null` line stops the inspector, at start and on every rescan. I tried ten other hostile files (`case.json`, `run-stopped.json`, `status.json`, `run-settings.jsonl`, `llm-calls.jsonl`, `decisions.jsonl`, `experiment.json`, and a string line in `events.jsonl`) and the process stayed up with `/api/sources` answering 200. The event log is written by the runner, not by the model. This is on `main`: base `199f311ffb:ui/server.mjs` has the same expression.

## 3. Runs after the case existed

- `slot npx playwright test --workers=2 test/daemon-decide.spec.mjs test/daemon-ui.spec.mjs test/read-only.spec.mjs`: 12 passed, 12 skipped (wide-only specs in the laptop project), 17.8 s. The longest test was 5.5 s.
- `adv2-hostile-run.spec.mjs` alone: 1 failed.
- Probes started without the `slot` prefix: `run.sh` (links, Host and Origin, throwing request lines), `run2.sh` (daemon markers, doctor, `runbook canon`), `run3.sh` (hostile files), `race.mjs` (swap race).

## 4. Findings and their state at 3dd6c7f

| pass-1 # | state at 3dd6c7f | evidence |
|---|---|---|
| 1 doctor needs a canon binary | fixed at `runbook/src/doctor.rs:77` and `:200` | The row reads `0.11.0 (linked, revision e81f970d)`, required and passing with no canon binary on `PATH`. A fake `canon 9.9.9` first on `PATH`, a broken one first on `PATH` (exit 3), and `CANON` set to the fake change nothing. The revision `e81f970d` matches `e81f970decf2…` in `runbook/Cargo.lock`. |
| 3 bare `runbook canon` and `-V` | fixed at `runbook/src/main.rs:71` and `:176` | The bare command prints the five subcommand names and exits 64. `-V` and `--version` print `canon 0.11.0`, exit 0. `--help` lists the five. `bogus` prints clap's error and exits 64. |
| 4 request line kills the server | fixed at `ui/server.mjs:809-813` | After each of these, the next request answers 200: `%`, `%zz`, `%00`, `//a:99999/` (400), `//[::` (400), `http://x:99999/` (400), a 100 000-character path (the socket is closed, the server lives), a raw NUL (400), `source=%zz`, `source=1e999`, `seq=1e999`, `cases=%25%zz`, a GET with a body, a POST with a body (405), an unknown method (400), CONNECT. But a timer-run path still kills the process: see the new finding. |
| 5 links out of the run | fixed at `ui/runfiles.mjs:24` (`runPath`) | Not served: a linked directory mid-path, a link to another run of the same store, a link to a file outside the run, a link in `blobs/` to another run's blob (404), a dangling link, a link in `fs/` to `changes.jsonl`, a link in place of `run-settings.jsonl`, a `..` in `?case=` (400, also `%2e%2e%2f`, `C1/%2e%2e`). Served: a link inside the run, and a run directory that is itself a link, which gives its own data. With `--runs`, source 0 shows only `SECRET-OF-A` and source 1 only `SECRET-OF-B`. The swap race: 10 s of a separate process swapping a real directory with a link to outside, 174 894 swaps against 3 491 requests, 0 leaks. The check is not atomic with the read, so the race is possible in principle. |
| 6 Host and Origin | fixed at `ui/server.mjs:798` (`addressedToThisServer`), used at `:810` | Refused with 421: another port, trailing dot, `127.0.0.1.example`, `localhost:port@x`, no Host (HTTP/1.0), `Origin: null`, another port, `https://`. Static files, `/api/*`, `/events` and POST are refused alike. Allowed: upper-case `LOCALHOST`, `[::1]`, same-origin `Origin`. The page's own requests pass: `read-only.spec.mjs`, `daemon-ui.spec.mjs` and `daemon-decide.spec.mjs` passed in a browser. |
| 7 act code served by the inspector | fixed at `ui/daemon/decide.js:1` and `ui/web/app.js:17` | The inspector answers 404 for `/decide.js`, `/decide.js?x`, `/daemon/decide.js`, `/../daemon/decide.js`, `/%2e%2e/daemon/decide.js`, `/..%2fdaemon/decide.js`, `/web/../../daemon/decide.js` and `/daemon/`. No served `ui/web` file holds `answer-decision`, `Bearer`, `dd-token` or `/daemon/commands`. The inspector sends no `Server-Timing` header, so `app.js` never imports the act code. Under the daemon the page still acts: `daemon-decide.spec.mjs` passed. |

**Calls that read a run file without `runPath`** (from `git grep` over `ui/*.mjs` outside `test/` and `ux/`). Only unguarded reads with a non-run owner remain:
- the checkout files (`server.mjs:77-81`: `readRepoJson`, `readRepoText`, `repoStatSig`, `listRepoDir`);
- the experiment-record names, which come from the operator's `expDirs` (line 105 lists the operator's `--runs` directory);
- the view files named on the command line (lines 57 and 62);
- the web files served (lines 868-869);
- `/proc/<pid>` (line 401);
- start-up checks on paths the operator names (lines 52, 70, 108, 165, 342, 773, 846 and 880);
- `vclock.mjs`.

The `fs/changes.jsonl` reads at `server.mjs:514, 518, 559` take a path that `runPath` has already resolved. Neither the experiment-record reads nor the view-file reads take a name from a request.

**New finding (the only one left)**

| file:line | verdict | origin | what reaches it |
|---|---|---|---|
| `ui/server.mjs:329` | NEEDS-CHANGE | pre-existing (the same expression at base `199f311ffb`) | A `null` line in `events.jsonl` makes `observe` throw `Cannot read properties of null (reading 'seq')` in `tick()`. The inspector exits before it listens, or dies on the next rescan. The fix round guarded requests only; the timer and the start-up scan are outside the guard. Reached by opening any recorded run whose event log has such a line. Fix: skip a non-object line in `readEvents`, and wrap `tick` in try/catch. |

Smaller points, not findings:
- A hard link inside a run to a file outside it is served (`hard.md` in `/api/workfacts` shows `OUTSIDE-SECRET`). No check can see it, since it is the same inode; the confinement has to stop it.
- Node keeps the first `Host` header when two are sent, so an ok-then-evil pair passes. A browser cannot send two.
- The `Host` check demands `:port`, so a server on port 80 would refuse every request. The server defaults to 7490.
- `5311844` moved `decide.js` one commit before `app.js` stopped importing it statically, so that commit alone has a broken page. `3dd6c7f` is whole.

## 5. Attacked and not broken

- The `runPath` ways out (linked directory, another run, a file outside, dangling, rename race, run directory as a link, `..` after decoding).
- Every Host and Origin variant listed.
- Every throwing request line listed. All were answered, and the next request was answered too.
- Ten hostile run files other than the `null` event line.
- All the act-code URLs on the inspector.
- The doctor, with no `canon`, a fake `canon` and a broken `canon` on `PATH`, and with `CANON` set.
- `runbook canon` with no subcommand, `bogus`, `-V`, `--version` and `--help`.

## 6. Paths written outside the worktree

All under `<scratch>/removal/line2/adversary-3/pass-2/`:
- `cases.patch`
- `mkfx.sh`, `probe.mjs`, `run.sh`, `run2.sh`, `run3.sh`, `race.mjs`, `flip.mjs`, `run3.out`
- `fx/`, `race/`, `hx/`, `bogus/`, `broken/`
- the server logs `server*.log` and `hx.log`, in the `adversary-3/` directory one level up

Inside the worktree (`<trees>/b10x/runbook/line2/`), git-ignored: `ui/test/fixtures/runs/adv2-hostile-run/` and `ui/node_modules/.pw-results/`.

The patch is the 31-line diff above. After `git apply -R` and `git reset -q`, `git status --porcelain` printed nothing, and `git apply --check` succeeded. The tree is still on `fix/line2-review-3` at `3dd6c7f`.

```findings
[
  {"file": "ui/server.mjs", "line": 329, "category": "boundary", "severity": "warning", "verdict": "NEEDS-CHANGE", "origin": "pre-existing", "message": "A null line in events.jsonl makes observe() throw in tick(), which runs at start-up and on a timer outside the request guard, so the inspector exits before it listens or dies on the next rescan."}
]
```
