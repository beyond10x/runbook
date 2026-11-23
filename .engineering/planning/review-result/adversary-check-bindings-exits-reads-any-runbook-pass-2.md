---
format: aep.planning-md/3
id: review-result:adversary-check-bindings-exits-reads-any-runbook-pass-2
kind: review-result
status: active
title: 'Adversary pass 2 of the unit check-bindings-exits-reads-any-runbook at 54de83df: red, 0 blockers, 2 warnings, 4 notes'
summary: A script copied into another repository made that repository this repository, and the root path was read as a glob. An empty file and wrongly typed entries were not refused by name. The coordinator's located-script rule was removed and the round took all six findings.
relations:
- reviews: story:check-bindings-exits-reads-any-runbook
revision: 1
---
verdict: NEEDS-CHANGE

unit: `story:check-bindings-exits-reads-any-runbook` at `54de83df`, tree `<trees>/b10x/runbook/exits-root`, branch `impl/check-bindings-exits-reads-any-runbook`

cases: executed 20 to 26, red 4. `cargo test --locked --test main check_bindings` in `driver/`: `test result: FAILED. 22 passed; 4 failed; 0 ignored; 0 measured; 446 filtered out; finished in 0.37s`, exit 101. The 20 committed cases are green and the 4 failures are the reviewer's red cases. The slowest test is under 1 s.

origin: introduced 4 / pre-existing 2 / undecided 0, counted from the findings block; the header of the report gives other counts.

What held: the located-script rule holds for every start form: a link to the tree, a relative start, `.`, `./`, `..` from `scripts/`, another working directory, and `driver/..`. A tree copy with the script in `scripts/` still fails when a pool file is removed, when `--pool-wait` is cut or when `gate.run` is renamed. The full check against the base gives identical output for the real tree and for four variants. Only a whole argv element equal to `--pool-wait` makes the five codes the want for another runbook, and the refusal lines match the page. A comment-only file, a scalar, a list, `actions` null, `actions: {}`, `actions` a string and a second YAML document that is a scalar each give exit 1 and one line naming the file; a root with only `.yml` files, no `bindings/` or an empty one gives one line naming the root. Every pool-list mutant and the extra mutants are killed by committed cases. The changelog tail is byte-equal to the base and `bash -n` passes.

What did not hold: a script copied or linked into another repository's `scripts/` made that repository "this repository", so its runbook was asked for the five pool bindings again, the original defect and the story's first acceptance line. The check for no `bindings/*.yaml` read brackets, star and question mark of the root path as a glob, so a valid root with brackets in its name was refused with a false "no bindings/*.yaml". An empty file beside a good one passed silently and alone was refused with a blank name. An `executor`, `argv` or an action of the wrong type gave a raw jq error with exit 5 and no file named, as on the base. The page and the changelog overstated what is refused by name. A runbook that gets the codes 69 and 70 from the `SDC_POOL_WAIT` environment variable without the flag was refused.

coordinator: the coordinator's own rule of round 1, that the pool list applies to the repository the script lies in, is removed (decision D671 of the operator's delegate: round 2 takes all six findings, then pass 3, merge on green): the pool list applies only when `bindings/` holds one of the three named files, and the claim that this repository loses no pool check is held by the pin and by a full-check case. Fix round 2 takes all six findings. The reviewer's six cases are committed first under a name that says what they hold, four red at that commit, and the test-only environment hook of the earlier adversary file is removed. The root path is no longer globbed, an empty file and each unreadable shape get one `FAIL` line, the page and the changelog are made true, and for another runbook a binding without the flag may list the five codes for the environment variable. Adversary pass 3 follows the round.

```findings
[
 {"file":"scripts/check-bindings.sh","line":20,"category":"acceptance","severity":"warning","verdict":"NEEDS-CHANGE","origin":"introduced","message":"A script copied or linked into another repository's scripts/ made that repository this repository, so its runbook was asked for the five pool bindings again (driver/tests/check_bindings_exits_adv2.rs, one case red). The rule had been decided by the coordinator and the page did not name this consequence. A guard must hold that the pool list applies only when bindings/ holds a named file. Fixed in round 2 by removing the located-script rule."},
 {"file":"scripts/check-bindings.sh","line":44,"category":"boundary","severity":"warning","verdict":"NEEDS-CHANGE","origin":"introduced","message":"The check for no bindings/*.yaml read characters of the root path as a glob, so a valid root with brackets in its name was refused with a false no-bindings line. A guard must test the files without globbing the root path."},
 {"file":"scripts/check-bindings.sh","line":46,"category":"boundary","severity":"note","verdict":"CONFIRMED","origin":"pre-existing","message":"An empty bindings file beside a good one passed silently, and alone was refused with a blank name, while the page and the changelog say a file that is no mapping with actions is refused by name."},
 {"file":"scripts/check-bindings.sh","line":60,"category":"boundary","severity":"note","verdict":"CONFIRMED","origin":"pre-existing","message":"An executor, argv or action of the wrong type gave a raw jq error with exit 5 and no file named, and actions holding null or an empty mapping passed; the same on the base. A guard must refuse each by one line naming the file and the action."},
 {"file":"docs/loom-bindings.md","line":154,"category":"contract-drift","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"The page and the changelog said a bindings file that is no mapping with an actions list is refused by one line naming the file; empty files, non-YAML text and unreadable shapes were not."},
 {"file":"docs/loom-bindings.md","line":231,"category":"judgement","severity":"note","verdict":"INFEASIBLE","origin":"introduced","message":"For another runbook the want of five codes followed the --pool-wait flag in argv alone, so a runbook that gets 69 and 70 through the SDC_POOL_WAIT environment variable, which wins over the flag per the page, and has no flag was refused. The story is silent. Decided in round 2: a binding without the flag may list the five codes."}
]
```
