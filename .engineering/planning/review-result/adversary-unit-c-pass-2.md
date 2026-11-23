---
format: aep.planning-md/3
id: review-result:adversary-unit-c-pass-2
kind: review-result
status: active
title: 'Adversary pass 2 on unit C (documents) at 8e8e143: red, 6 findings'
summary: 'Second attack on impl/line3-docs: 23 of 25 pass-1 rows fixed, row 4 waits on unit B, row 6 is in the backlog; 4 new findings (3 introduced, 1 pre-existing), no blocker.'
tags:
- wave-2026-11-22-w1
relations:
- reviews: story:driver-runtime-removed
revision: 1
---
unit: C (documents) at 8e8e143, pass 2
verdict: red
cases: executed 5→6 (sh checks run by path; no suite exists for text and none was run), red 4
origin: introduced 4 / pre-existing 2 / undecided 0
wrote-outside-worktree: 6 paths
needs-coordinator: (1) Pass-1 row 4 still holds at this head: `driver/examples/sdc/*.bindings.json` name `sdc-node` on 0 lines, so `docs/driver.md:1125` becomes true only when unit B's change lands. (2) Pass-1 row 6 still stands at `docs/driver.md:1106`; you routed it to the backlog, and it stays in the block so the two passes compare.

No blocker is left. Of the 25 pass-1 rows, 23 are fixed, row 4 waits on unit B and row 6 is in the backlog; the fix round introduced four new findings: two warnings (two "Now" lines on the context page; 24 line citations into pages it shifted) and two notes.

## 1. `git --no-pager diff --stat`

Empty. `git status --porcelain` is also empty (0 lines), before and after, at head `8e8e143` on `impl/line3-docs` in `<trees>/b10x/runbook/line3-c`. I edited nothing in the tree and wrote nothing to the store.

## 2. Cases

The five pass-1 scripts are in `<scratch>/removal/line3-c/adversary/`; the new one is in `<scratch>/removal/line3-c/adversary/pass-2/`.

| Case | Now | What is left |
|---|---|---|
| `case-01-history-list.sh` | green, exit 0 | nothing |
| `case-02-driver-sections.sh` | red, `fails: 11` | (a) 0 lines in live sections; (b) 2: the JSON bindings (row 4) and the `--instructions` sentence (row 6); (c) 9 named citations into record sections, all now worded as history |
| `case-03-shifted-citations.sh` | green, exit 0 | nothing; the four citations are rewritten |
| `case-04-live-pages.sh` | red, `fails: 4` | `website/docs/context.mdx` 143, 147, 155, 167; each is past tense with a "Now", judged history. The `docs/cycles-suite.md` checks pass at this head |
| `case-05-paths-in-added-lines.sh` | red, `fails: 35` | deleted files named as deleted, `runbook/target/debug/runbook` (build output, twice), and paths already missing at the base; judged no defect |
| `pass-2/case-06-moved-citations.sh <tree> <base>` (new) | red, `fails: 24` | finding N3 |

`case-06` asserts that every unpinned `page.md:N` citation of the base still opens on the same text at the head. Its red output, from the run when it was written:
```
FAIL docs/canon-requests.md:15 still cites driver.md:110; its text is at driver.md:146 now
FAIL docs/canon-research/01-event-stream-and-resume.md:53 still cites driver.md:175; its text is at driver.md:211 now
FAIL docs/canon-research/04-surface-reduction-techniques.md:410 still cites layers.md:128; its text is at layers.md:136 now
FAIL docs/canon-research/04-surface-reduction-techniques.md:191 still cites layers.md:140; its text is at layers.md:148 now
FAIL docs/engineering-protocols-boundary.md:103 still cites layers.md:187; its text is at layers.md:195 now
FAIL docs/conductor-comparison.md:89 still cites kpis.md:255; its text is at kpis.md:260 now
FAIL docs/conductor-comparison.md:70 still cites autonomy-levels.md:427; its text is at autonomy-levels.md:429 now
FAIL docs/decisions.md:96 still cites autonomy-levels.md:242; its text is at autonomy-levels.md:243 now
FAIL docs/decisions.md:57 still cites autonomy-levels.md:244; its text is at autonomy-levels.md:245 now
FAIL docs/decisions.md:85 still cites autonomy-levels.md:257; its text is at autonomy-levels.md:258 now
FAIL docs/decisions.md:114 still cites autonomy-levels.md:277; its text is at autonomy-levels.md:279 now
FAIL docs/decisions.md:296 still cites autonomy-levels.md:297; its text is at autonomy-levels.md:299 now
FAIL docs/decisions.md:234 still cites autonomy-levels.md:499; its text is at autonomy-levels.md:503 now
FAIL docs/kpis.md:336 still cites autonomy-levels.md:271; its text is at autonomy-levels.md:273 now
FAIL docs/orchestration.md:189 still cites autonomy-levels.md:271; its text is at autonomy-levels.md:273 now
FAIL docs/orchestration.md:305 still cites autonomy-levels.md:417; its text is at autonomy-levels.md:419 now
FAIL docs/orchestration.md:307 still cites autonomy-levels.md:418; its text is at autonomy-levels.md:420 now
FAIL docs/requester.md:26 still cites autonomy-levels.md:235; its text is at autonomy-levels.md:236 now
FAIL docs/requester.md:37 still cites autonomy-levels.md:242; its text is at autonomy-levels.md:243 now
FAIL docs/requester.md:27 still cites autonomy-levels.md:244; its text is at autonomy-levels.md:245 now
FAIL docs/requester.md:89 still cites autonomy-levels.md:374; its text is at autonomy-levels.md:376 now
FAIL docs/requester.md:137 still cites autonomy-levels.md:483; its text is at autonomy-levels.md:487 now
FAIL docs/requester.md:38 still cites autonomy-levels.md:520; its text is at autonomy-levels.md:524 now
FAIL docs/requester.md:181 still cites autonomy-levels.md:527; its text is at autonomy-levels.md:531 now
same: 16  moved: 12  rewritten: 44  text changed or repeated: 7  blank: 11
fails: 24
```
My first run of this script also matched `AGENTS.md:N` and printed 21 more lines. Those mix this repository's `AGENTS.md` with other repositories' (`loom/AGENTS.md:146`, `C/AGENTS.md:66`), so I took that page name out and count none of them.

## 3. Suite run

None. No cargo, no build and no docs build was run; every check is a grep, a `git show` or a `git diff`.

## 4. Findings at head `8e8e143`

### The 25 pass-1 rows

| # | pass-1 `file:line` | state | where now |
|---|---|---|---|
| 1 | `AGENTS.md:120` | fixed | `AGENTS.md:118-124` gives `--run <run name>` and says what the command does without it; `--run` is read at `driver/src/bin/sdc-experiment.rs:2690` |
| 2 | `website/docs/context.mdx:117` | fixed | `:119-120` "Every lever that existed at the pinned commit"; the note at `:18-25` covers the page from "What one node sees" on |
| 3 | `<scratch>/removal/line3-c/history.md` | fixed | rows for `docs/trial-*.md` (52) and `docs/progress.*` (32); case 01 exit 0 |
| 4 | `docs/driver.md:1127` | **not fixed** | `docs/driver.md:1125`; the JSON bindings hold `sdc-node` on 0 lines at this head (unit B's files) |
| 5 | `docs/driver.md:1090` | fixed | the section "Instructions" names no `sdc-unstick` and no `run.unstick` |
| 6 | `docs/driver.md:1108` | **not fixed**, in the backlog | `docs/driver.md:1106`, unchanged |
| 7 | `docs/driver.md:36` | fixed | the citing lines say "record in" or "the former driver" (`docs/autonomy-levels.md:219, 261, 479`, `docs/run-compare.md:50-51`, `docs/bugfix.md:410-411`, `docs/conductor-comparison.md:67`); `docs/decisions.md:210` and `docs/fs-changes.md:83` now cite the runner's pages |
| 8 | `AGENTS.md:85` | fixed | cites `docs/layers.md:55-60`, where the table stands |
| 9 | `docs/decisions.md:233` | fixed | both `kpis.md` citations rewritten |
| 10 | `docs/takt-comparison.md:85` | fixed at that line | the rest of that row's count is N3 |
| 11 | `docs/bugfix.md:172` | fixed | `:171-174` say which cycles are in the suite, which are pending and which is a Rust test |
| 12 | `docs/conformance.md:17` | fixed | the recorded re-drive clause is gone |
| 13 | `docs/reject-routes.md:3` | fixed | `:3-9` name the live check and its gate |
| 14 | `docs/parity.md:4` | fixed | `:3-12` say status `active`, what compared equal and that five items are open |
| 15 | `website/src/data/context.ts:138` | fixed | `:134` separates the pinned commit from today |
| 16 | `website/src/routes/Evaluation.tsx:586` | fixed | `:585-589`, `rule:loom-run`, citations at `main` |
| 17 | `website/docs/run-it.md:35` | fixed | `:41-43` cite `docs/loom-run.md`, "A dry run" |
| 18 | `README.md:83` | fixed | `README.md:78-83` and `website/docs/run-it.md:12-20` give the two build commands; both are lines of `Taskfile.yml:63,65` |
| 19 | `README.md:96` | fixed | `:103` lists the 11 subcommands of `runbook/src/main.rs:25-76`; `:112` |
| 20 | `website/docs/built-on.mdx:13` | fixed | `:11-18` |
| 21 | `website/src/data/layers.ts:97` | fixed | Loom is `use: 'used'`; Canon and Substrate entries match `docs/loom-run.md:31` |
| 22 | `docs/fs-changes.md:167` | fixed | `:167-169` say the launcher is deleted |
| 23 | `docs/loom-run.md:295` | fixed | `driver/src/main.rs` stands on 0 lines of the pages that row named |
| 24 | `docs/cycles-suite.md:58` | fixed | the page is byte-equal to unit A's at `e9a5df5` |
| 25 | `AGENTS.md:64` | fixed | `:64` starts with `loom-run.md` |

### New findings

| # | file:line | verdict | origin | what reaches it |
|---|---|---|---|---|
| N1 | `website/docs/context.mdx:151` | NEEDS-CHANGE | introduced | The "Now" of "A node's binding" says `bindings/<protocol>.yaml` has `llm` members; line 155 says `llm.instructions` of the binding names the instruction files. `bindings/*.yaml` hold `llm:` on 0 lines; the options are in the `argv` of `sdc-node` (`--instructions <json>`, `bindings/sdc-direction.yaml:16`). A reader who follows it finds no such key. |
| N2 | `website/docs/context.mdx:163` | CONFIRMED | introduced | The "Now" of "Effort per node class" says the settings page "says whether an effort is a key there". `docs/loom-run-settings.md:90-91` already answers it: `sdc-node` has no option for an effort under Loom. |
| N3 | `docs/requester.md:26` | NEEDS-CHANGE | introduced | 24 unpinned line citations open 1 to 36 lines off. 18 go into `docs/autonomy-levels.md`, which fix commit `71849de` shifted by 1 to 4 lines; 3 into `docs/layers.md`, 2 into `docs/driver.md`, 1 into `docs/kpis.md`. Three sampled were on target at the base (`docs/requester.md:26`, `docs/decisions.md:96`, `docs/orchestration.md:305`). List in case 06. |
| N4 | `plugin/skills/harden/SKILL.md:103` | CONFIRMED | pre-existing | The skill lists `sdc-experiment stop <id> --reason "<why>"` without `--run` and without the word that it stops every open run and abandons the record. `AGENTS.md:120` and `docs/workflow-metrics.md:432` now give `--run`. The line is on `4677c2b`. |

## 5. Attacked, not broken

- Merge `a4f291e`, `AGENTS.md`: unit A's only own lines there (the `eval/` and `scripts/` entries) are carried in the merged wording.
- Merge, `docs/eval-design.md`, `docs/orchestration.md`, `docs/kpis.md`, `docs/parity.md`: each of unit A's 38 lines not in the merge is replaced by a line with the same fact; unit C lost one line in `docs/parity.md`, replaced by unit A's fuller sentence.
- `CHANGELOG.md` from `## [0.5.0]` on has the same sha256 at the head and at `4677c2b` (`30e59765…`).
- `AGENTS.md` base to head: 16 hunks, none in a rule on money, logins, confinement or what never enters git; the only edit inside the layering rule is the line number at `:85`.
- The new stop rule: `--run`, `--reason` and `--runs-dir` are the options `stop_cmd` reads; without `--run` the code does what the rule says.
- `README.md` build step: `driver/Cargo.lock` and `runbook/Cargo.lock` exist, `runbook` has its own `[workspace]`, so the binary is `runbook/target/debug/runbook`; `runbook/src/assets.rs:12` looks for the helper programs in the checkout's `driver/target/{release,debug}`.
- Every option of the `runbook run` commands in `README.md` and `website/docs/run-it.md` is in clap, as in pass 1; no new option was added.
- `website/docs/built-on.mdx` and `website/src/data/layers.ts` against `docs/layers.md:3-9` and `docs/loom-run.md`: no contradiction; `use: 'partly'` is a value of the type and has a label.
- The `Cite` component of `Evaluation.tsx` takes `at`; `sdc-experiment.rs:2115` is `loom_argv`, `docs/loom-run-directory.md:76` holds `rule:loom-run`.
- The "Now" lines of the context page for idea text, goals, rejection, rework cap, model per class and autonomy match `docs/loom-run.md:27,116-117`, `docs/loom-run-settings.md:25,49` and `docs/driver.md` "Now".
- The past-tense lines left on the context page (143 to 167) and the 9 citing pages read as history; none tells the reader to use a deleted file.
- The 304 lines the fix round added hold no home path, no `file://`, no `[patch]`, no person name and no new speed, quality or cost statement.
- All 13 commits are author and committer `b10x-bot[bot]`.

## 6. Paths written outside the worktree

- `<scratch>/removal/line3-c/adversary/pass-2/case-06-moved-citations.sh`
- `<scratch>/removal/line3-c/adversary/pass-2/case-06.out`
- `<scratch>/removal/line3-c/adversary/pass-2/case-06.run`
- `<scratch>/removal/line3-c/adversary/pass-2/case-02.out`
- `<scratch>/removal/line3-c/adversary/pass-2/added-fix.txt`
- `<scratch>/removal/line3-c/adversary/case-05.out` (overwritten by the rerun)

The `worktree` lease of session `adversary-line3-c-p2` is released (exit 0).

## 7. Findings block

```findings
[
  {"file": "docs/driver.md", "line": 1127, "category": "contract-drift", "severity": "warning", "verdict": "NEEDS-CHANGE", "origin": "introduced", "message": "a section declared live says the agent nodes of driver/examples/sdc/*.bindings.json run sdc-node, and those files name llm-node.sh on 14 lines and sdc-node on none"},
  {"file": "docs/driver.md", "line": 1108, "category": "contract-drift", "severity": "note", "verdict": "CONFIRMED", "origin": "pre-existing", "message": "the sentence that bindings/*.yaml names no instruction file is false, the files carry --instructions on 14 lines"},
  {"file": "website/docs/context.mdx", "line": 151, "category": "contract-drift", "severity": "warning", "verdict": "NEEDS-CHANGE", "origin": "introduced", "message": "two Now lines say bindings/<protocol>.yaml has llm members and llm.instructions, and the YAML bindings hold no llm key, the options are in the argv of sdc-node"},
  {"file": "website/docs/context.mdx", "line": 163, "category": "judgement", "severity": "note", "verdict": "CONFIRMED", "origin": "introduced", "message": "the Now line for effort sends the reader to the settings page to find out, and that page already says sdc-node has no option for an effort under Loom"},
  {"file": "docs/requester.md", "line": 26, "category": "contract-drift", "severity": "warning", "verdict": "NEEDS-CHANGE", "origin": "introduced", "message": "24 unpinned line citations open off their text, 18 of them into docs/autonomy-levels.md, which the fix round shifted by 1 to 4 lines"},
  {"file": "plugin/skills/harden/SKILL.md", "line": 103, "category": "contract-drift", "severity": "note", "verdict": "CONFIRMED", "origin": "pre-existing", "message": "the skill lists sdc-experiment stop without --run and without saying that it then stops every open run and abandons the record"}
]
```
