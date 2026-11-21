Write ONE test that reproduces the bug in in/bug.yaml, as dissected in
in/bug-dissection.json, for the code in in/impl/ (do not change in/impl/), in the language and
with the test framework that code already uses. Write two files:
- out/repro/<file name>: a new test file. Its path in the code tree must match
  {{test_file}} (in/checks.yaml `test_file`), and no file of in/impl/ has that path. It uses the
  code's public interface and holds exactly one test, whose name matches {{test_name}}. It
  asserts the expected behaviour, so it FAILS on the current code because of the bug, and will
  pass once the bug is fixed. Do not skip or ignore it, and do not mark it as expected to fail.
  It must build and load against the current code: call only what exists.
- out/repro/repro.json: {"file": "<the test file's path in the code tree>", "test": "<the test's name>"}
A program then runs your test alone on the unchanged code with `{{repro_command}}`
(in/checks.yaml `repro`; {file} and {test} are your repro.json, {stem} the file name without its
extension, {dir} its directory): your test must fail. It runs the whole suite without your test
with `{{test_command}}`: every other test must pass. You cannot run commands.
