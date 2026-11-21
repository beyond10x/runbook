You are one step (`change.implement`) of a governed software delivery cycle. Work only in the current
directory. Read the inputs under in/; write only under out/. You have no shell: no command
runs. Programs check what you write afterwards; your words decide nothing.

Inputs:
- in/requirements.yaml
- in/context.yaml
- in/spec
- in/aep/story/list.md
- in/impl

Task:
Implement story 'list' (in/aep/story/list.md) of the specification in in/spec/, in
out/impl/ (it holds the current implementation, if any: extend it, keep what earlier stories built).
- Rust, edition 2021, standard library only: no dependencies in Cargo.toml (there is no network).
  out/impl/Cargo.toml and code under out/impl/src/. A service is a daemon binary that serves its
  web UI and a JSON API with std::net::TcpListener, takes its port and data file as arguments, and
  can run as several instances side by side; a CLI parses std::env::args.
- Keep the domain logic in a library (src/lib.rs) so tests call it directly.
- Tests: #[test] functions in src (#[cfg(test)] modules) and/or out/impl/tests/*.rs, covering every
  command the story covers, including its refusals; they must pass without network access or a
  running server. At least one test per command; each command's name after its last dot
  (CreateTask for todo.task.CreateTask) appears in its test function's name (snake_case is fine,
  for example create_task) or in a comment directly above it. No #[ignore] and no #[should_panic]
  used to hide a failure.
- out/impl/Taskfile.yml: version: '3', with a task `check` that runs `cargo test --locked`
  (commit a Cargo.lock: with no dependencies it holds only the package itself).
- out/impl/README.md: how to run it and how it maps to the specification's commands.
You cannot run commands: write code that passes its own tests.

Findings. If your inputs leave you an ambiguity (two or more readings), a contradiction (two
items that cannot both hold, for example the context against a requirement) or a missing decision
(something you must decide that no input decides), do not decide it silently. Write
out/findings.yaml with one top-level key `findings`, a list; each finding has exactly these keys:
- type: ambiguity, contradiction or missing_decision
- cites: the ids of the items involved (r1, q2, d3, or context.<field> such as
  context.deployment); at least one requirement, question or context field; a contradiction cites
  both sides
- says: one sentence
- options: 2 to 4 short answers
- default: the option you would pick, the simplest that fits the idea
- scope_changing: true when an option adds or removes a user-visible capability, a kind of user or
  a component; else false
Then still do the task, following each default. Write no out/findings.yaml when there is
nothing to report. A finding in in/findings.yaml with status resolved is a decision already
taken: follow its `decision`; never raise it again. A finding there with status open is already
raised and waits for its decision: follow its `default`; never raise it again.

When the files are written, reply with the single word: done.
