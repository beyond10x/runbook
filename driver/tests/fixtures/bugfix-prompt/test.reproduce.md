You are one step (`test.reproduce`) of a governed software delivery cycle. Work only in the current
directory. Read the inputs under in/; write only under out/. You have no shell: no command
runs. Programs check what you write afterwards; your words decide nothing.

Inputs:
- in/bug.yaml
- in/bug-dissection.json
- in/impl
- in/checks.yaml

Task:
Write ONE test that reproduces the bug in in/bug.yaml, as dissected in
in/bug-dissection.json, for the code in in/impl/ (do not change in/impl/), in the language and
with the test framework that code already uses. Write two files:
- out/repro/<file name>: a new test file. Its path in the code tree must match
  ^tests/[a-z][a-z0-9_]*\.rs$ (in/checks.yaml `test_file`), and no file of in/impl/ has that path. It uses the
  code's public interface and holds exactly one test, whose name matches ^[a-z_][a-z0-9_]*$. It
  asserts the expected behaviour, so it FAILS on the current code because of the bug, and will
  pass once the bug is fixed. Do not skip or ignore it, and do not mark it as expected to fail.
  It must build and load against the current code: call only what exists.
- out/repro/repro.json: {"file": "<the test file's path in the code tree>", "test": "<the test's name>"}
A program then runs your test alone on the unchanged code with `cargo test --offline --test {stem} -- --exact {test}`
(in/checks.yaml `repro`; {file} and {test} are your repro.json, {stem} the file name without its
extension, {dir} its directory): your test must fail. It runs the whole suite without your test
with `cargo test --offline --no-fail-fast`: every other test must pass. You cannot run commands.

When the files are written, reply with the single word: done.
