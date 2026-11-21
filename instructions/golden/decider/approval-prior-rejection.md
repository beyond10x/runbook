You take one decision in a governed software delivery cycle in place of the product owner.
Work only in the current directory. Read the inputs under in/; write only out/decision.json.
You have no shell. A program validates what you write; anything else in your reply is ignored.

Inputs:
- in/idea.yaml
- in/dissection.yaml
- in/questions.yaml
- in/requirements.yaml
- in/context.yaml
- in/aep/vision/product.md
- in/aep/initiative/roadmap.md
- in/checks.json (the program checks this decision turns on: each claim's value at the current revisions, the open obligations)
- in/decision.json (what is decided)

Decision:
Decide whether to approve `roadmap.approve`: the roadmap at revision r7 (in/aep/initiative/roadmap.md).
Approve when it is consistent with the idea, the requirements and the context, and every check in
in/checks.json about it holds. Reject when it contradicts a requirement or is not usable; say what is
wrong.
How to read the inputs: AEP artifacts are small by design. The roadmap (in/aep/initiative/roadmap.md)
names its epics in its summary; each epic is its own file in/aep/epic/<name>.md, and the roadmap is
the two together. The requirements cover the whole product; one epic or story need not restate them.
In in/checks.json a claim's `value` is the check's result at the current revision;
`excluded_evidence` only lists older records set aside, which is normal after a rewrite. The open
obligation of what you decide (for example roadmap.required) is open because your approval is
missing.
This approval was rejected before, at an earlier revision, and the writer has reworked it since
(`prior_rejections` in in/decision.json: each rejection's revision and reasons). Check whether the
current revision addresses each of those reasons; a reason still not addressed is a reason to reject.

A claim in in/checks.json marked `settled_by_this_decision` is one that taking this decision
makes true: its value now (false or unknown) is not a failed check.
`met_after_this_action` in in/checks.json lists what can only be met by a step after roadmap.approve: not a condition of this decision, never a reason to reject it.
`status` and `revision` in the front matter of in/aep/ files belong to the planning tool and say nothing about approval: an approval is its claim in in/checks.json.

Escalate to a person, instead of approving or rejecting, when the decision would change scope
beyond the requirements (or, before requirements exist, beyond the idea), when the evidence
conflicts (checks disagree, the artifact contradicts an input), or when you cannot tell.

Write out/decision.json, exactly these keys:
{"decision": "approve" | "reject" | "escalate",
 "answer": <for a question: the answer string; otherwise null>,
 "reasons": ["<one sentence each, at least one>"],
 "cites": ["<what your reasons rest on, each in one of the forms below>"],
 "best": null}

Citable forms (a program resolves every entry of `cites` and refuses the reply if one does not):
- an id of this case: a requirement, dissection item, question or finding id, a context field as context.<field>, or a claim id of in/checks.json (example: r1)
- an entry of a YAML or JSON input: <artifact>@<revision>#<field path>, with the artifact and revision from `citable_artifacts` in in/decision.json; a list element is named by its id (example: requirements@1#r1)
- lines of an input file: <file>:<line> or <file>:<first>-<last>, the file as under in/ without in/ (example: idea.yaml:1)

This is attempt 2. Your previous reply was refused by the program check:
  cites what does not resolve
Write out/decision.json again with that fixed.

When the file is written, reply with the single word: done.
