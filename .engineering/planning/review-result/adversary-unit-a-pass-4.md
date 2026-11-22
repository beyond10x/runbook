---
format: aep.planning-md/3
id: review-result:adversary-unit-a-pass-4
kind: review-result
status: active
title: 'Confirmation of fix round 3 of unit A (deletion) at ded17af: green, no finding'
summary: 'The one finding of the narrow check is fixed: the guard fails a scanned file with code after its inline test module; 7 mutants in the real tree, 16 of 16 guard tests in 2.4 s.'
tags:
- wave-2026-11-22-w1
relations:
- reviews: story:driver-runtime-removed
revision: 1
---
unit: A (deletion) at ded17af, confirmation of fix round 3
verdict: green
origin: introduced 0 / pre-existing 0 / undecided 0
wrote-outside-worktree: `<scratch>/removal/line3-a/adversary/pass-3/mutants.sh`, plus `orig.rs`, the scratch copy it restores each mutated file from
needs-coordinator: none
cases: none added; I ran 7 mutants in the real tree and put each back

## (1) The pass-3 finding: fixed

- **Fixed.** `non_test_text` is at `driver/tests/bugfix_prompt.rs:719`. The walk `program_sources` (`:735`) now collects the refused files and `guarded_sources` (`:754`) returns them. The guard fails on any refused file and names it.
- **Test.** `a_file_with_code_after_its_inline_test_module_fails_the_guard` is at `:780`.
- **One run.** `cd driver && SLOT_TIMEOUT=10m cargo nextest run --locked --test main bugfix_prompt` ran 16 tests: 16 passed, 0 failed, 380 skipped, in 2.4 s. The guard `the_rust_binaries_hold_no_instruction_text` took 1.6 s.

## (2) Mutants in the real tree

Each mutant ran against the guard alone, with a copy restored afterwards.

- **Instruction line after the test module: caught.**
  - I appended `const ADVERSARY_PASTED: &str = "<164-character decider instruction line>";` after the closing brace of the test module of `runbook/src/loom_run/budget.rs`.
  - The guard fails in 0.2 s: `files the guard cannot cut into non-test code: ["<trees>/runbook/src/loom_run/budget.rs: code after the inline test module: const ADVERSARY_PASTED: …`.
  - It names the file and the first line after the module.
- **Brace as the last item of a test module: the guard passes.** I put each of these before the module's final `}` in `runbook/src/loom_run/bugfix.rs`. All pass, so the count is not fooled into seeing code after the module:
  - `"}"` in a string literal;
  - `r#"}"#`, a raw string;
  - `'}'`, a char literal;
  - `// }`, a line comment;
  - `/* } */`, a block comment;
  - `"\"}"`, an escaped quote then a brace in a string.

After the run `git status --porcelain` is empty.

## (3) Diff, author, tree

```
 driver/tests/bugfix_prompt.rs | 226 ++++++++++++++++++++++++++++++++++++++----
 1 file changed, 208 insertions(+), 18 deletions(-)
```

- `git diff b726f25 ded17af --stat` shows one file, a test file.
- The commit `ded17af` is the only one over `b726f25`. Author and committer are both `b10x-bot[bot]`.

```findings
[]
```
