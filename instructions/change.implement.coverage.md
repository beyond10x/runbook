Coverage. Besides the files, write out/coverage.json, one JSON object with one key
`requirements`, a list with exactly one entry per requirement of in/story-requirements.json:
- built: {"id", "status": "built", "at": [{"file", "line"}, ...], "test"}: `at` the
  places in out/impl/ that build it (file relative to out/impl/, for example src/lib.rs; line from
  1; at least one), `test` the name of a #[test] function in out/impl/ that exercises it
- not_built: {"id", "status": "not_built", "reason"}: one sentence why it is not built
A program checks it before any test runs: an id that is missing, named twice or not_built, a file
that is not in out/impl/, a line outside its file, or a test name no #[test] function has sends
the change back to you. Schema: {{schema}}
