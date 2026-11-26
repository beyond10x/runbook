A session delegated a piece of work that no runbook of this daemon fits. Write a protocol for it: a Canon
`protocol/1` document (its artifacts, evidence kinds, claims, actions and outcomes) and its Loom
`loom.bindings/1` bindings. You compose only parts the daemon has: every action runs the agent node
`sdc-node delegation.free`, which is given the request (the artifact `idea`, at `request.yaml`) and the files of
the run's workspace, does the work and writes `result.md`; its exit status is its evidence (`from: exit`). You
write no program and name no other one. The runbooks of the daemon, for what they are not (id, title,
description, inputs):

{"id":"answer-demo","title":"Answers a question in result.md","description":"One program answers the idea in result.md.","inputs":["idea"]}

The request, as the session wrote it:

<request>
Write one sentence about tea.
</request>

The simplest such protocol, the daemon's free run, as an example:

<protocol>
format: protocol/1
protocol:
  id: delegation-free
  revision: 1
  description: >-
    The free run of a delegation: one agent node whose instruction is the request, on the delegation's route, in the
    run's workspace, writing result.md.
artifacts:
  idea: {}
  result: {}
evidence_kinds:
  result_written: {}
claims:
  result.written:
    true_when:
      evidence: {kind: result_written, subject: result, result: written}
actions:
  delegation.free:
    effect: write
    may_produce:
      - evidence: result_written
outcomes:
  delivered:
    requires: {claim: result.written}
</protocol>

<bindings>
format: loom.bindings/1
artifacts:
  idea: request.yaml
  result: result.md
actions:
  - action: delegation.free
    executor: {kind: program, argv: [sdc-node, delegation.free]}
    evidence:
      - {kind: result_written, subject: result, producer: sdc-node, from: exit, results: {"0": written, timeout: failed, "*": failed}}
</bindings>

An action of the bindings may carry `instruction`: the text its agent node takes as its own step beside the
request (at most 20000 characters), for a protocol of more than one step. The daemon writes the bindings'
`protocol` and `program_root` and each executor's arguments, exits, budget and timeout itself. Answer with one JSON object and nothing else, of this shape:

{"protocol": "<the protocol/1 document, as YAML text>", "bindings": "<the loom.bindings/1 document, as YAML text>"}

or, when the free run above is the protocol for the request, {"free": true}.