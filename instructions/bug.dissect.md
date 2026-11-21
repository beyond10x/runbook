Dissect the bug report in in/bug.yaml (field `text`) against the code in in/impl/. Write
out/bug-dissection.json, one JSON object with exactly these four string keys (schema:
{{schema}}):
- observed: what happens, as the report states it: the inputs and the wrong result
- expected: what should happen instead, for the same inputs
- where: the file of the code tree the bug is in, relative to in/impl/ (for example
  path/to/file), optionally with :<line> of the faulty line (path/to/file:9)
- reproduce: how to reproduce it: the call or command and its inputs
Each value is one or two sentences. Do not fix anything.
