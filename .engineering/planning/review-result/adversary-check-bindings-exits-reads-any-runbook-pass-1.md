---
format: aep.planning-md/3
id: review-result:adversary-check-bindings-exits-reads-any-runbook-pass-1
kind: review-result
status: active
title: 'Adversary pass 1 of the unit check-bindings-exits-reads-any-runbook at 3b58b28d: red, 0 blockers, 2 warnings, 3 notes'
summary: With the three named pool files removed or emptied, the exits check passed for this repository where the base failed, and two apply-condition mutants were killed by no committed case. A scalar file and a file without actions were not refused by name. The fix round took all five findings.
relations:
- reviews: story:check-bindings-exits-reads-any-runbook
revision: 1
---
verdict: NEEDS-CHANGE

unit: `story:check-bindings-exits-reads-any-runbook` at `3b58b28d`, tree `<trees>/b10x/runbook/exits-root`, branch `impl/check-bindings-exits-reads-any-runbook`

cases: executed 7 to 13, red 4. `cargo test --locked --test main check_bindings` in `driver/`: `test result: FAILED. 9 passed; 4 failed; 0 ignored; 0 measured; 446 filtered out; finished in 0.33s`, exit 101. All 7 committed cases are green and the 4 failures are the reviewer's red cases. The slowest test took well under 5 s.

origin: introduced 4 / pre-existing 1 / undecided 0, counted from the findings block; the header of the report gives other counts.

What held: with the three named pool files removed one or two at a time, renamed, linked, or replaced by a file that holds the pool bindings under another name, `--exits` still exits 1 as the base does. The full check without `--exits` is held. No `bindings/` directory, an empty one, non-YAML text, `exits.error: []`, a code in both classes, `exits` that is no mapping, a trailing slash, a relative path, a link and a space in the root name each exit 1 or pass as the base does, and the refusals name the file and the action. A file with no program binding passes. The changelog tail is byte-equal to the base, the diff holds no home path, name or organisation, the fixtures use invented text, and the page and the changelog match the script on the apply rule, the exits lines and the `--pool-wait` refusal. The mutants of the apply condition that invert it, make it always true, make it always false or check only `sdc-release.yaml` are killed by committed cases.

What did not hold: the apply condition for the pool list was true only if a named file was among the files read, so with the three named files removed or emptied from this repository, `--exits .` printed `pass exits` and exited 0 where the base exited 1; the full check and the gate still failed. A bindings file that is a scalar passed, and a file with no `actions` key gave a raw jq error with exit 5 and no file named; both exist on the base and outside roots now reach them. No committed case killed the mutants that check only `sdc-story.yaml` or only `sdc-bugfix.yaml` in the apply condition. A comment line read `# # mapping`. The story is silent on what an outside root with `--pool-wait` in a binding gets, and the page documents a refusal that is a line about the build pool.

coordinator: fix round 1 takes all five findings (decision D669). The pool list applies when the root is the repository the script lies in, or when `bindings/` holds one of the three named files. For an outside root, a binding that carries `--pool-wait` wants the five codes and one without it the three. The reviewer's six cases are committed first with assertions unchanged, four of them red at that commit. After the round, two of the reviewer's temporary-root cases stayed red, because a copy of the bindings without the named files is another runbook by the rule; decision D670: those two cases assert the rule's result, and the claim that this repository loses no pool check is held by the located-script cases and by a pin that the three named files exist.

```findings
[
 {"file":"scripts/check-bindings.sh","line":37,"category":"boundary","severity":"warning","verdict":"NEEDS-CHANGE","origin":"introduced","message":"With the three named pool files removed or emptied from this repository's bindings, --exits . printed pass exits and exited 0 where the base exited 1; the full check and the gate still failed, and nothing outside the tests calls --exits . (driver/tests/check_bindings_exits_adversary.rs, two cases red). A guard must hold a fact that does not vanish with the files, such as a pin that the three files exist."},
 {"file":"scripts/check-bindings.sh","line":28,"category":"judgement","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"The comment line read # # mapping, a stray mark the diff added. Fixed in round 1."},
 {"file":"scripts/check-bindings.sh","line":33,"category":"boundary","severity":"note","verdict":"CONFIRMED","origin":"pre-existing","message":"A bindings YAML file with no actions key gave a raw jq error (exit 5, no file named) and a scalar YAML file passed with exit 0; the same on the base with this repository's bindings plus one extra file, and outside roots now reach it. A guard must refuse a file that is no mapping with actions, by name."},
 {"file":"driver/tests/check_bindings_exits.rs","line":119,"category":"mutant","severity":"warning","verdict":"NEEDS-CHANGE","origin":"introduced","message":"No committed case killed the mutants that check only sdc-story.yaml or only sdc-bugfix.yaml in the apply condition, that is a root holding only one of the three named files. The reviewer's case for a root holding only one named file kills both and is committed."},
 {"file":"docs/loom-bindings.md","line":155,"category":"acceptance","severity":"note","verdict":"INFEASIBLE","origin":"introduced","message":"The story does not say what an outside root with --pool-wait in a binding gets; the script refused it with a line about the build pool that the runbook cannot clear, and the page documented that refusal. Decided in round 1: such a binding wants the five codes."}
]
```
