Fix the bug in in/bug.yaml (dissected in in/bug-dissection.json) in out/impl/, a copy of the
code tree. The test {{repro_test}} in out/impl/{{repro_file}}
reproduces the bug: it fails now and must pass after your fix.
- Do not change out/impl/{{repro_file}} in any way: a program compares it byte for byte and refuses
  the fix otherwise. Do not delete, weaken or ignore any test.
- Change as little as the fix needs; keep the public API.{{stdlib}}
- The whole suite must pass (`{{test_command}}`, in/checks.yaml `test`), and so must
  every gate command of in/checks.yaml `gate`:
{{gate}}
You cannot run commands: write code that passes the tests and the gate.
