---
format: aep.planning-md/3
id: review-result:adversary-line2-fix-round-3-pass-3
kind: review-result
status: active
title: 'Narrow check of line 2 fix round 3 (units 1 and 2) at 4868620: green'
summary: The one finding of pass 2 (a null line in events.jsonl stopped the inspector) is fixed; the server stays up with lines that are no record in every run file; no finding.
tags:
- wave-2026-11-22-w1
relations:
- reviews: story:driver-runtime-removed
revision: 1
---
unit: units 1 and 2 of line 2 at 4868620, narrow check
verdict: green
cases: executed 0→1, red 0
origin: introduced 0 / pre-existing 0 / undecided 0
wrote-outside-worktree: `<scratch>/removal/line2/adversary-3/pass-3/` only
needs-coordinator: no

1. My pass-2 case (re-applied from `pass-2/cases.patch`) is green: `ui/test/adv2-hostile-run.spec.mjs`, `--project=wide`, 1 passed in 3.8 s. At `3dd6c7f` it was red.
2. The server stayed up at start and over two rescans with `null`, `"x"` and `5` lines, each in `run-settings.jsonl`, `llm-calls.jsonl` and `decisions.jsonl` (9 runs). I also fed one run `null` or string or number lines in `events.jsonl`, `status.json`, `run-stopped.json` and `case.json`. Requests for those runs all got an answer and the process stayed up.
   - The answers were 200 and 404 everywhere, except for runs whose `decisions.jsonl` holds a non-object line, plus the all-bad run. Their `/api/run` and `/api/inbox` answer 500 and log a stack trace (`decisions.mjs:230`, `decisionRecord`). That is allowed by your "any status" and I did not chase it.
3. One run with every file broken, beside one good run (`--runs`, 11 runs in all): the good run is listed with 1 case and state `closed`. Its `/api/tree`, `/api/run`, `/api/live`, `/api/inbox` and `/api/workfacts` all answer 200 with its data.
4. `git diff --stat 3dd6c7f..4868620` shows 5 files, all for this finding:
   - the CHANGELOG bullet;
   - `ui/server.mjs`: `readEvents` plus a per-run catch in the scan, the `tick()` list signature and `sourceRow`;
   - `ui/runfiles.mjs`: `readJson` falls back on `null`;
   - `ui/decisions.mjs`: one line, the `authority.json` grant, which now tolerates `null`;
   - the new `ui/test/event-log-lines.spec.mjs`.
   
   Added lines hold no `/home/`, no person's name and no organisation name (grep of the added lines for those strings printed nothing).

New cases leave the tree as `<scratch>/removal/line2/adversary-3/pass-3/cases.patch`. It is the same 31-line spec as in pass 2, not a new case. After `git apply -R` and `git reset -q`, `git status --porcelain` printed nothing. `git apply --check` succeeded. The tree is on `fix/line2-review-3` at `4868620`. My probe (`run.sh`) and its fixtures are in the same scratch directory.

```findings
[]
```
