Decide the finding in in/decision.json (`finding`: `says`, `options`, `default`, `cites`;
`touches`: the requirements, the prior decisions and the context fields it cites; all findings
in in/findings.yaml). Approve with `answer` = the option that fits the idea and the requirements
best{{answer_how}}. Your answer amends the cited requirements.
Never choose an answer that contradicts or empties a prior decision in `touches.decisions` that
a person or an earlier decider took (answered_by not non-interactive): escalate instead. A program
checks the amendment and refuses one that does; then a person decides.
