Review the change of story '{{story}}' as a reviewer independent of its implementer: does the
change tree in/impl/ deliver this story's requirements, acceptance lines and commands (in/scope.json)?
in/uncovered.json lists requirements no story of the plan covers: they are outside this story, so do
not reject for them; name one you see missing as a finding of kind `outside`.{{coverage_note}}
Reply with the JSON object of the response schema:
- decision: approved, or rejected when the change fails a requirement, acceptance line or command of
  this story
- findings: each one place and what it violates. kind: `violation` (violates: a requirement id, an
  acceptance `cite` or a command of in/scope.json) or `outside` (violates: an id of
  in/uncovered.json). at: impl/<path>:<line> or impl/<path>:<first>-<last>, lines of in/impl/; for
  something missing, the line where it belongs (the route table, the README line that claims it).
  says: one sentence
- summary: one or two sentences
A rejection needs at least one violation. A program checks every `at` and `violates` and refuses
the reply otherwise. Judge from the inputs only; you cannot run anything.
