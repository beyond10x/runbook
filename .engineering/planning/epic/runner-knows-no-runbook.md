---
format: aep.planning-md/3
id: epic:runner-knows-no-runbook
kind: epic
status: active
title: 'The runner and the model node know no runbook: SDC is protocol, bindings and instructions'
summary: Move what is SDC only out of the runner and the node program into the bindings and bound programs; prove it with a second runbook that has a model node and a check that fails on a runbook's name in the runner.
tags:
- backlog-2026-11-23
relations:
- informed_by: epic:runbook-one-binary-and-daemon
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-11-23T01:52:21Z", actor: "human:operator", revision: 2, executor: "agent:runbook", correlation: "operator-2026-11-23-generic-runner"}
- {from: "proposed", to: "active", at: "2026-11-23T04:01:59Z", actor: "human:operator", revision: 3, executor: "agent:runbook", correlation: "operator-2026-11-23-d628-a"}
---
## Outcome

A runbook is its protocol, its bindings and its instruction texts, and nothing else. The runner
(`runbook`) and the program of a model node know no runbook by name: a second runbook with model
nodes runs without a line of Rust written for it. The software delivery cycle (SDC) is the first
runbook and is stated in those three kinds of file.

## Source

- The operator, 2026-11-23, typed in the runbook session: "Shouldn't runbook be pretty much generic
  or agnostic? So the SDC, so the software delivery lifecycle, should, I think, be something
  completely specifiable or definable just by the protocol and some bindings. So I'm not sure
  whether it is a good design to have this in Rust code, the SDC stuff, but maybe there's a good
  reason for this."
- Read at `main` `566a0c3f` (counts from `wc -l`; the split is a reading, not a measurement):
  - `sdc-node` (the program every model node starts) has 2,821 lines; about 1,325 of them name a
    concrete SDC action, artifact or file layout: the table of inputs and outputs per action
    (`driver/src/bin/sdc-node/nodes.rs:128-221`), `implement.rs`, `review.rs`, `findings.rs`,
    `store.rs`. The library it links for replies (`driver/src/reply/`, 3,091 lines) is SDC only.
  - The runner (`runbook/src`, 18,456 lines without tests): 3,336 lines sit in files that exist
    only for SDC (`loom_run/bugfix.rs`, `goals.rs`, `watch.rs`, `conformance.rs`, the decider,
    `doctor.rs`); 3,523 more are generic files that name SDC (`list.rs:7` the startable names,
    `loom_run/here.rs:18` the case id `IDEA-1`, `assets.rs:9`, `bindings_copy.rs:21-42`,
    `decider/routing.rs:19-40` the decision classes, `decider/inputs.rs:78-97` the artifact files).
  - No protocol other than SDC has a model node; the two non-SDC fixtures run shell programs only.
  - No document gives a reason for SDC logic in Rust. The reasons on record are for Rust over
    shell: the rule "anything new that runs is Rust" (`AGENTS.md:68-71`) and the port of the old
    shell node (the prompt test went from 31-38 s to 0.5 s).

## What it takes (to decompose)

1. **The binding states what a model node reads and writes.** Inputs, the output artifact, the
   node class and the model role move from `nodes.rs` and `node-classes.json` into the binding of
   the action. A model node is marked as one in the binding; the runner stops finding it by the
   program name.
2. **One generic node program.** Prompt frame, adapter call, caps, effort, retry feedback, the
   records of a call. What is SDC only (the implement and review replies, the promotion of
   findings, the writes to the planning store) becomes bound programs of SDC's own actions.
3. **The runner loses its SDC options.** `--bugfix`, `--bugfix-ticket`, `--goals`, `--conformance`
   become inputs and actions a runbook declares; the root case id and whether a protocol can be
   started come from the protocol.
4. **Decisions by data.** The classes of decisions, their holders and the files a decider reads
   are stated by the runbook; the runner keeps the routing rule and the autonomy levels.
5. **Proof.** A second runbook with a model node, written by hand with invented text, runs through
   `runbook run` and `runbook test` with a stub adapter; a check fails when `runbook/src` or the
   node program names an action, an artifact or a protocol of a runbook.
6. **Names.** Programs, variables and format ids that still say `sdc` for generic things follow
   `story:project-rename`.

## Acceptance

- The check of item 5 is in `task check` and green.
- The SDC cycles of `cycles/tree-suite.yaml` keep their results; the prompts of the 14 model nodes
  are byte-identical before and after (the golden prompt test).
- No measured claim changes without a record in `experiments/`.

## Not in this epic

- New runbooks of a consumer.
- The configuration file and the router (`story:runbook-toml-carries-defaults`,
  `story:runbooks-load-from-configured-paths`, `story:router-picks-a-runbook-for-an-input`).

## Corrections

Found by the decomposition and its scope critic (read at `main` `826b09c7` and again at `3c0eb863`). Where a
sentence above disagrees with a line here, this line holds.

1. `runbook/src/loom_run/watch.rs` is not SDC code. It is a generic watcher whose helper program is named
   `sdc-watch`. Only the name is left to do (item 6).
2. `decider/routing.rs` and `decider/inputs.rs` hold SDC data (class names, file names, one path) that item 4
   moves to the runbook. The decider's request is still built through the SDC reply library, which stays; the
   code stays too.
3. Item 3, "`--bugfix`, `--bugfix-ticket`, `--goals`, `--conformance` become inputs and actions a runbook
   declares": they become declared inputs and programs a runbook names, not actions. A new action changes the
   revision of a protocol.
4. Item 3, "the root case id and whether a protocol can be started come from the protocol": `protocol/1` of
   the pinned Canon is closed and has no member for either. The root case id comes from the declarations of the
   runbook (`decision-blocker:runbook-declarations-without-a-format-member`). Whether a protocol can be started
   is with `story:runbooks-load-from-configured-paths`.
5. Outcome, "A runbook is its protocol, its bindings and its instruction texts, and nothing else": a runbook
   is its protocol, its bindings, its instruction texts and the data files it declares (today: the decision
   classes, the autonomy levels, the node classes and the answers file).
6. Item 2 is complete with `story:node-program-built-ins-removed`, not with the last story that binds a
   program.
