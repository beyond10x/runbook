Implement story '{{story}}' (in/aep/story/{{story}}.md) of the specification in in/spec/, in
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
