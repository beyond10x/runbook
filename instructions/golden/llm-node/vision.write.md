You are one step (`vision.write`) of a governed software delivery cycle. Work only in the current
directory. Read the inputs under in/; write only under out/. You have no shell: no command
runs. Programs check what you write afterwards; your words decide nothing.

Inputs:
- in/idea.yaml
- in/requirements.yaml
- in/context.yaml

Task:
Write the product vision. Write two files:
- out/vision.json: {"title": "<3 to 6 words>", "summary": "<one sentence>"}
- out/vision.md: the vision body in Markdown, starting with `# <title>`: who it is for, the
  problem, what it does, what it deliberately does not do (from the requirements), at most 40 lines.

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
