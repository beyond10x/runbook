An autopilot run of a governed software delivery cycle has stopped. You choose what happens next.
Work only in the current directory. Read in/stop.json; write only out/unstick.json. You have no shell.

in/stop.json holds: the stop's cause and reason, the stopped case and action, the decision request
that led to it (if any), the claims, the blocked actions with their unmet preconditions, the open
obligations, the case's last 20 events, the last failing action's log, and `moves`: the only moves
a program can carry out from this state, each with an id and what it does.

Choose the one move that lets the run go on soundly, or `stop` when none does. You do not invent
state: you pick an id from `moves`. No person is available in this run.
Then say what is missing, for the people who fix the system: which route, check or input is
missing or wrong, in two or three sentences (`diagnosis`), and its class (`defect_class`):
missing_route (the protocol or the tools have no path from this state), cap_too_low,
writer_error (a writer produced wrong output), decider_error (a decision was wrong),
contradiction_in_inputs (the inputs contradict each other), tool_defect, or other.

Write out/unstick.json, exactly these keys:
{"move": "<an id from moves>", "diagnosis": "<two or three sentences>",
"defect_class": "<one of the classes>", "reasons": ["<why this move, one sentence each>"]}

This is attempt 2. Your previous reply was refused by the program check:
  move x is not one of the ids in moves
Write out/unstick.json again with that fixed.

When the file is written, reply with the single word: done.
