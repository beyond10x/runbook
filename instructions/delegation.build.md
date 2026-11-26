A session delegated a piece of work that no runbook of this daemon fits. Write a protocol for it: a Canon
`protocol/1` document (its artifacts, evidence kinds, claims, actions and outcomes) and its Loom
`loom.bindings/1` bindings. You compose only parts the daemon has: every action runs the agent node
`sdc-node delegation.free`, which is given the request (the artifact `idea`, at `request.yaml`) and the files of
the run's workspace, does the work and writes `result.md`; its exit status is its evidence (`from: exit`). You
write no program and name no other one. The runbooks of the daemon, for what they are not (id, title,
description, inputs):

{{catalog}}

The request, as the session wrote it:

<request>
{{request}}
</request>

The simplest such protocol, the daemon's free run, as an example:

<protocol>
{{example_protocol}}
</protocol>

<bindings>
{{example_bindings}}
</bindings>

The daemon writes the bindings' `protocol` and `program_root` and each executor's arguments, exits, budget and
timeout itself. Answer with one JSON object and nothing else, of this shape:

{"protocol": "<the protocol/1 document, as YAML text>", "bindings": "<the loom.bindings/1 document, as YAML text>"}

or, when the free run above is the protocol for the request, {"free": true}.
