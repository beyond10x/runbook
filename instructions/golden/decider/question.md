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
Answer the interview question in in/decision.json (`question`: its text, `options`, `default`;
`item`: the dissection item it was asked about; `idea_excerpt`: that item's words in the idea).
Approve with `answer` = the default, unless the default contradicts the idea; then `answer` is the
option (or a short better answer) that fits the idea. The question is scope-changing: escalate when
every answer, the default included, would add a capability, a kind of user or a component the idea
does not ask for.

A claim in in/checks.json marked `settled_by_this_decision` is one that taking this decision
makes true: its value now (false or unknown) is not a failed check.
`met_after_this_action` in in/checks.json lists what can only be met by a step after scope.answer: not a condition of this decision, never a reason to reject it.
`status` and `revision` in the front matter of in/aep/ files belong to the planning tool and say nothing about approval: an approval is its claim in in/checks.json.

Escalate to a person, instead of approving or rejecting, when the decision would change scope
beyond the requirements (or, before requirements exist, beyond the idea), when the evidence
conflicts (checks disagree, the artifact contradicts an input), or when you cannot tell.

Write out/decision.json, exactly these keys:
{"decision": "approve" | "escalate",
 "answer": <when approve: the answer string>,
 "reasons": ["<one sentence each, at least one>"],
 "cites": ["<what your reasons rest on, each in one of the forms below>"],
 "best": null}

Citable forms (a program resolves every entry of `cites` and refuses the reply if one does not):
- an id of this case: a requirement, dissection item, question or finding id, a context field as context.<field>, or a claim id of in/checks.json (example: r1)
- an entry of a YAML or JSON input: <artifact>@<revision>#<field path>, with the artifact and revision from `citable_artifacts` in in/decision.json; a list element is named by its id (example: requirements@1#r1)
- lines of an input file: <file>:<line> or <file>:<first>-<last>, the file as under in/ without in/ (example: idea.yaml:1)

When the file is written, reply with the single word: done.
