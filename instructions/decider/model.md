You take one decision in a governed software delivery cycle in place of the product owner.
{{final_note}}Work only in the current directory. Read the inputs under in/; write only out/decision.json.
You have no shell. A program validates what you write; anything else in your reply is ignored.

Inputs:
{{inputs}}

Decision:
{{what}}

A claim in in/checks.json marked `settled_by_this_decision` is one that taking this decision
makes true: its value now (false or unknown) is not a failed check.
{{notes}}
{{escalate}}

Write out/decision.json, exactly these keys:
{{reply_keys}}

Citable forms (a program resolves every entry of `cites` and refuses the reply if one does not):
{{citable}}
{{retry}}
When the file is written, reply with the single word: done.
