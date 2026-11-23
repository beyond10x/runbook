---
format: aep.planning-md/3
id: review-result:security-run-without-model-node-needs-no-caps-pass-4
kind: review-result
status: active
title: 'Security pass 4 of the unit run-without-model-node-needs-no-caps at 019002fd: green, 1 warning and 1 note in older code, 2 notes introduced'
summary: No guard moved in fix round 3 and seven mutants are each killed by a committed case. Two points of the spend cap in code older than the unit are held by ignored measurements and are their own accepted story. Two notes on wording and an exit code were corrected in a final step with no change of the source.
relations:
- reviews: story:run-without-model-node-needs-no-caps
revision: 1
---
verdict: approve

unit: `story:run-without-model-node-needs-no-caps` at `019002fd`, tree `<trees>/b10x/runbook/zero-caps`, branch `impl/run-without-model-node-needs-no-caps`

cases: executed 130 to 133, red 0 in the suite; two ignored measurements of the reviewer fail by design (F1, F2). Suite `no_model_node login bindings_copy ledger budget review4`: `Summary [  43.626s] 133 tests run: 133 passed, 691 skipped`, exit 0, 93 s wall, load 78.54 before and 79.59 after. New cases alone: `Summary [   4.019s] 3 tests run: 3 passed, 669 skipped`. Ignored measurements: `Summary [   2.720s] 2 tests run: 0 passed, 2 failed, 670 skipped`. The slowest test of the suite is 22.2 s and is not of this unit.

origin: introduced 2 / pre-existing 2 / undecided 0, counted from the findings block.

What held: no guard moved in the diff of fix round 3 (16 files; in the source `run_loop.rs` 63 lines and `here.rs` 7 lines). No assertion of the pass-3 cases is lost in the four files that took them. The usage refusal has one producer and one consumer, and both kinds of refusal return before the run directory is made; with a model node each of the three options missing alone gives exit 64, and a value that is present and unusable ends as on the base. Under real `substrate` inside a pool job a bound program saw only a loopback interface, no route, no resolver and no socket. The decided sentence stands verbatim once on the page. The merge of the released base changed no file of the unit beside `CHANGELOG.md`, whose part from the first released heading on is byte-equal to the base. Seven mutants (the page sentence in other words, the manifest check before a `tree status` skipped, the two kinds of refusal swapped, the refusal for a run with a model node removed, the caps refusal as a usage refusal, both refusals after the run directory is made, `local` given where `substrate` was asked) are each killed by a committed case.

What did not hold: two points of the spend cap in code older than the unit, both the same on the base (read there, not run): a cap with nothing left does not stop a run whose model tool names no budget option before a `tree run`; after a step booked past the cap, a decision the same step printed is still answered when the decider costs nothing. Three sentences of the unit (the page, `docs/cli.md`, `CHANGELOG.md`) said "a cap of 0 stops the run before its first `tree run`" without that condition. The page said "its last recorded call is booked whole", which holds only for a step that records one call: the runner bounds neither the amount nor the count of what one step records. With a model node, no ledger option and another refusal that also applies, the exit is that refusal's own and not 64.

coordinator: the verdict is the reviewer's green: the unit changed nothing at the two older points. A final step with no change of the source committed the reviewer's three cases and the two measurements (ignored, each naming the story that builds the guard), corrected the three sentences and the booking sentence, and wrote the exit-code sentence with a case that holds it. The two older points are `story:spend-cap-with-nothing-left-starts-nothing`, accepted on 2026-11-23 to be built as its own unit ahead of the next feature, with a security pass before its merge. The guard that story must hold: a cap with nothing left starts no `tree run`, whatever the bindings name, and after the cap is passed the runner makes no call that can start a program. No release of this unit before this pass: met.

```findings
[
 {"file":"runbook/src/loom_run/run_loop.rs","line":474,"category":"spend-cap","severity":"warning","verdict":"CONFIRMED","origin":"pre-existing","message":"A cap with nothing left must stop a run with a model node before a tree run, whatever the bindings name. At the head the stop reads the amount the step wants, and for a model tool that names no budget option that amount is what the cap has left. The committed bindings all name the option. Same on the base. Held by an ignored measurement that names the story spend-cap-with-nothing-left-starts-nothing."},
 {"file":"runbook/src/loom_run/run_loop.rs","line":742,"category":"spend-cap","severity":"note","verdict":"CONFIRMED","origin":"pre-existing","message":"After the cap is passed the runner must make no call that can start a program. At the head a decision printed by the step that passed the cap is still answered once when the decider costs nothing, and the run ends after it with exit 4. Same on the base. Held by an ignored measurement that names the same story."},
 {"file":"docs/loom-run.md","line":349,"category":"documentation","severity":"note","verdict":"NEEDS-CHANGE","origin":"introduced","message":"The page said the last recorded call is booked whole and a case comment said the figures differ by one call; both hold only for a step that records one call. What one step records is booked whole, and the runner bounds neither its amount nor its count. Corrected in the final step, with a case that holds the sentence."},
 {"file":"runbook/src/loom_run/run_loop.rs","line":146,"category":"exit-code","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"The usage refusal for a missing ledger option stands after the other refusals of a run. With a model node, no ledger option and another refusal that also applies, the exit is that refusal's own, where the base's parser gave 64 first; nothing is written either way. The page and docs/cli.md say so since the final step, with a case."}
]
```
