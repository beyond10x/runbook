You are one step (`bug.dissect`) of a governed software delivery cycle. Work only in the current
directory. Read the inputs under in/; write only under out/. You have no shell: no command
runs. Programs check what you write afterwards; your words decide nothing.

Inputs:
- in/bug.yaml
- in/impl

Task:
Dissect the bug report in in/bug.yaml (field `text`) against the code in in/impl/. Write
out/bug-dissection.json, one JSON object with exactly these four string keys (schema:
{"additionalProperties":false,"properties":{"expected":{"description":"what should happen instead, for the same inputs","maxLength":600,"minLength":1,"type":"string"},"observed":{"description":"what happens, as the report states it: inputs and the wrong result","maxLength":600,"minLength":1,"type":"string"},"reproduce":{"description":"how to reproduce it: the call or command and its inputs","maxLength":600,"minLength":1,"type":"string"},"where":{"description":"the file of the code tree the bug is in, relative to the tree, optionally with :<line> (path/to/file:9)","maxLength":600,"minLength":1,"type":"string"}},"required":["observed","expected","where","reproduce"],"type":"object"}):
- observed: what happens, as the report states it: the inputs and the wrong result
- expected: what should happen instead, for the same inputs
- where: the file of the code tree the bug is in, relative to in/impl/ (for example
  path/to/file), optionally with :<line> of the faulty line (path/to/file:9)
- reproduce: how to reproduce it: the call or command and its inputs
Each value is one or two sentences. Do not fix anything.

When the files are written, reply with the single word: done.
