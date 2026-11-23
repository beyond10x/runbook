---
format: aep.planning-md/3
id: review-result:adversary-check-bindings-exits-reads-any-runbook-pass-3
kind: review-result
status: active
title: 'Adversary pass 3 of the unit check-bindings-exits-reads-any-runbook at 365da575: red, 0 blockers, 1 warning, 4 notes'
summary: A dangling link in the bindings directory was skipped, so the exits check passed and the full check exited 0 where the base failed. Four notes are on the page and on older script lines. The round took the warning and the four notes.
relations:
- reviews: story:check-bindings-exits-reads-any-runbook
revision: 1
---
verdict: NEEDS-CHANGE

unit: `story:check-bindings-exits-reads-any-runbook` at `365da575`, tree `<trees>/b10x/runbook/exits-root`, branch `impl/check-bindings-exits-reads-any-runbook`

cases: executed 32 to 42, red 5. `cargo test --locked --test main check_bindings` in `driver/`: `test result: FAILED. 37 passed; 5 failed; 0 ignored; 0 measured; 446 filtered out; finished in 6.87s`, exit 101. All 32 committed cases are green and the 5 failures are the reviewer's red cases.

origin: introduced 2 / pre-existing 3 / undecided 0, counted from the findings block; the header of the report gives other counts.

What held: a root holding one of the three named files gets the pool list, with exactly five lines, and nothing else makes the list apply. With the story file removed, `--exits .` prints one pool line and exits 1; the full check names the missing file. Brackets, a space, star, question mark, a newline, braces, a tilde, a quote, a dollar sign, a backtick, a leading dash as `./-dash`, a trailing slash, a link to a directory and `bindings` as a link give exit 0. A `bindings` file, an unreadable `bindings/`, only `.yml` files and only a dangling link give one line naming the root. A list document, a scalar, a null document, a comment-only or whitespace-only file, an empty second document, `actions` a mapping, `exits` a list and `exits.error` a string are each refused by one line naming the file. Another runbook's code lists hold for every flag spelling against every code list tried, and a flag in a nested member does not count. The full check on the committed files is identical to the base. The mutants, including the empty-file refusal and the wrong-type lines, are killed by committed cases. The changelog tail is byte-equal to the base and the diff holds no home path or name.

What did not hold: a dangling link in `bindings/` was skipped silently, so `--exits` passed where the base failed, and the full check printed a raw yq error from the registrations step but exited 0 where the base exited 1, because the step's process substitution dropped the failure; the gate was weaker. A relative root that starts with a dash was read as an option by `cd` and exited 2, as on the base. A null `executor` passed although the page says an `executor` that is no mapping is refused. Wrong-type `evidence` gave a raw jq error with exit 5 and no file named, as on the base. The page did not say what another runbook that holds one of the three names sees. Further, with all three named files removed from this repository `--exits .` prints `pass exits`, which follows the decision of pass 2; the pin and the full check carry the claim, and the committed case that this repository passes the exits check cannot fail for a removal. The pin of the three names is not itself killed by a mutant that empties its loop; other cases that read the files fail.

coordinator: fix round 3 takes the warning and the four notes (decision D675 of the operator's delegate: round 3 takes all five findings, then a fourth pass on the diff, merge on green). The reviewer's ten cases are committed first, five red at that commit. A dangling link is a named refusal in `--exits` and in the full check, and the registrations step fails the script on a read failure. The root is read after `--`. A present and null `executor` is refused, wrong-type `evidence` is refused by one line naming the file and the action, and the page says what another runbook that holds one of the three names sees.

```findings
[
 {"file":"scripts/check-bindings.sh","line":41,"category":"boundary","severity":"warning","verdict":"NEEDS-CHANGE","origin":"introduced","message":"A dangling link in bindings/ was skipped, so --exits passed where the base failed with exit 1, and the full check printed a raw yq error but exited 0 (driver/tests/check_bindings_exits_adv3.rs, two cases red). A guard must hold that a dangling link is a named refusal, in --exits and in the full check. Fixed in round 3."},
 {"file":"scripts/check-bindings.sh","line":113,"category":"boundary","severity":"note","verdict":"CONFIRMED","origin":"pre-existing","message":"A relative root that starts with a dash was read as an option by cd and exited 2; the same on the base. A guard must end the options before the root operand."},
 {"file":"scripts/check-bindings.sh","line":72,"category":"boundary","severity":"note","verdict":"CONFIRMED","origin":"pre-existing","message":"An executor that is null passed although the page says an executor that is no mapping is refused; the same on the base. A guard must refuse it or the page must say present and no mapping."},
 {"file":"scripts/check-bindings.sh","line":128,"category":"boundary","severity":"note","verdict":"CONFIRMED","origin":"pre-existing","message":"Evidence of the wrong type gave a raw jq error with exit 5 and no file named; the same on the base. The page did not claim this shape."},
 {"file":"docs/loom-bindings.md","line":147,"category":"contract-drift","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"The page says the pool list applies when a root holds one of the three names but not that another runbook that holds one gets exactly five not-in-bindings lines. Pinned by a committed case; the page is corrected in round 3."}
]
```
