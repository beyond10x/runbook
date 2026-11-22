## Task

Implement story `{{story}}` of the specification above in the product tree impl/ (shown above with its current text; extend it,
keep what earlier stories built).
- The build constraints above.
- Keep the domain logic in a library (src/lib.rs) so tests call it directly.
- Tests: #[test] functions in src (#[cfg(test)] modules) and/or tests/*.rs, covering every command the story covers,
  including its refusals; they must pass without network access or a running server. At least one test per command;
  each command's name after its last dot (CreateTask for todo.task.CreateTask) appears in its test function's name
  (snake_case is fine, for example create_task) or in a comment directly above it. No #[ignore] and no #[should_panic]
  used to hide a failure.
- Taskfile.yml: version: '3', with a task `check` that runs `cargo test --locked` (commit a Cargo.lock: with no
  dependencies it holds only the package itself).
- README.md: how to run it and how it maps to the specification's commands.
You cannot run commands and cannot read anything else: write code that compiles and passes its own tests against exactly
the files shown.

## Reply

The JSON object of the response schema, nothing else:
- files: every file you create or change, each {"path", "content"}: path relative to impl/ (for example src/lib.rs),
  content the file's complete new text. Never a path with `..`, an absolute path, a hidden entry or target/: a program
  refuses the whole reply.
- note: one or two sentences on what you built (on a rework: the lines asked for above).
- findings: If the inputs above leave you an ambiguity (two or more readings), a contradiction (two items
that cannot both hold, for example the context against a requirement) or a missing decision (something you must decide
that no input decides), do not decide it silently: put it in `findings`, each with type (ambiguity, contradiction or
missing_decision), cites (the ids involved: r1, q2, d3, or context.<field>; a contradiction cites both sides), says (one
sentence), options (2 to 4 short answers), default (the option you would pick, the simplest that fits the idea) and
scope_changing (true when an option adds or removes a user-visible capability, a kind of user or a component). Then
still do the task, following each default. A finding listed above with status resolved is a decision already taken:
follow its `decision`, never raise it again; one with status open waits for its decision: follow its `default`, never
raise it again. `findings` is [] when there is nothing to report.
- remaining: [] as a rule. Only when the files would not fit in one reply: write the files you can, each complete, and
  list here the paths still to write; you are then asked for exactly those (at most {{max_continuations}} more times), with
  the files you wrote shown.
