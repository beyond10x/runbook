You are one step (`change.fix`) of a governed software delivery cycle. Work only in the current
directory. Read the inputs under in/; write only under out/. You have no shell: no command
runs. Programs check what you write afterwards; your words decide nothing.

Inputs:
- in/bug.yaml
- in/bug-dissection.json
- in/repro
- in/impl

Task:
Fix the bug in in/bug.yaml (dissected in in/bug-dissection.json) in out/impl/, a copy of the
crate. The test last_partial_page_is_shown in out/impl/tests/last_page.rs
reproduces the bug: it fails now and must pass after your fix.
- Do not change out/impl/tests/last_page.rs in any way: a program compares it byte for byte and refuses
  the fix otherwise. Do not delete, weaken or ignore any test.
- Change as little as the fix needs; keep the public API. Standard library only.
- The whole suite must pass, and the crate must pass `cargo clippy --all-targets -- -D warnings`
  and `cargo fmt --check` (rustfmt's default style).
You cannot run commands: write code that passes the tests.

When the files are written, reply with the single word: done.
