---
format: aep.planning-md/3
id: story:the-free-runs-turns-follow-the-runs-call-cap
kind: story
status: active
title: The free run's agent node gets no more turns than its run has calls left
summary: The agent node of a free run or a generated protocol takes the smaller of its --max-turns and the run's calls left
tags:
- consumer-asks-2026-11-27-agents
relations:
- informed_by: story:daemon-takes-a-delegation
- informed_by: story:delegation-classifies-or-builds-a-protocol
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-11-27T18:50:51Z", actor: "agent:runbook", revision: 3}
- {from: "proposed", to: "active", at: "2026-11-27T18:50:51Z", actor: "agent:runbook", revision: 4}
---
## Outcome

The agent node of a delegation's free run, and of every action of a generated protocol, gets a turn bound no larger
than the model calls its run has left. A run with a call cap below 40 ends its node on the node's own turn bound,
not on the relay's `cap-reached` refusal in the middle of a call.

## Context

Gap G3 of a consumer repository that dispatches agents through the daemon (2026-11-27, relayed by the host's
operator session).

- The free run's bindings start the agent node with a fixed `--max-turns "40"` and `--max-budget-usd "2"`
  (`runbook/src/delegation.rs:383`). A generated protocol's actions take that executor unchanged
  (`runbook/src/delegation.rs:268`, `bind_built`).
- A delegation's settings already take `max_llm_calls`: `delegate_for` resolves the caps from its settings up to the
  daemon's ceiling and writes them back (`runbook/src/commands.rs:1601-1605`; `runbook/src/caps.rs:6-8`,
  `runbook/src/serve.rs:108-110`). So the second form of the ask is there. The turn bound does not follow it.
- The relay refuses a call when the run's calls reach the cap: `used.llm_calls >= max_llm_calls`
  (`runbook/src/engine/gate.rs:600-601`). The count holds steps and decisions together (`runbook/src/config.rs:92`),
  so classify and build calls come before the node's turns.
- `sdc-node` takes `--max-turns` from the binding, else the node's default of 30 (`driver/src/bin/sdc-node/call.rs:266`,
  `driver/src/bin/sdc-node/nodes.rs:79`). The `claude-cli` adapter passes it to `claude -p --max-turns`
  (`bindings/tools/llm-adapter-claude-cli.sh:100`).
- *Inferred*: the CLI reads the relay's 429 `cap-reached` as a rate limit and may retry before the node ends. That
  costs wall time and leaves no `result.md`.

## What this story does

1. When the agent node of a free run or of a generated protocol starts, its turn bound is the smaller of the
   binding's `--max-turns` and the calls the run has left (cap minus used). The unit picks one way:
   - (a) the engine writes the number into the argv of the run's pinned copy of the bindings before each start;
   - (b) the route file names `calls_left` and `sdc-node` takes the smaller value.

   Neither way lets a request or a model raise the bound.
2. A run with no calls left starts no node. That is the case today (`story:spend-cap-with-nothing-left-starts-nothing`
   for spend); this story adds the same for calls if the unit finds it missing.
3. The node's record (`argv.json`, `request.json` of the call) names the bound it got.

## Acceptance

With the daemon in process, a stand-in model program on the run's route file and no network beyond loopback:

- A free run of a delegation with `settings.max_llm_calls = "5"`: the stand-in records `--max-turns 5`, and the run
  ends without a `cap-reached` answer from the relay.
- With `max_llm_calls = "100"` the stand-in records `--max-turns 40`.
- A classified delegation that spent 2 calls on classify with `max_llm_calls = "5"`: the node records `--max-turns 3`.
- A generated protocol's action records the same bound as the free run's.
- Each case runs under 5 s, with no model call.

## Evidence

- The consumer's design, section 8, item G3: the daemon served with `--max-llm-calls 30`, the free run ran
  `--max-turns 40`, and the relay refused calls 31 to 40, so the run ended on the cap.

## Relations

- Informed by `story:daemon-takes-a-delegation` (the free run) and `story:delegation-classifies-or-builds-a-protocol`
  (generated protocols take the free run's executor).

## The way and a cap of 0 or 1

- The way taken (2026-11-27): neither (a) nor (b) as written. The runner that starts every program of a daemon run and
  reserves its budget (`runbook/src/engine/gate.rs`) gives the agent node of a delegation (`sdc-node delegation.free`
  or `sdc-node delegation.change`, so every action of a generated protocol too) the variable `RUNBOOK_CALLS_LEFT` as
  it starts: the run's call cap (the delegation's `max_llm_calls`, else the daemon's `--max-llm-calls`) minus the
  calls the run has used, at least 1. `sdc-node` takes the smaller of its `--max-turns` and that number, and its
  `request.json` names the bound it got. The daemon sets the variable and the program can only lower its bound with
  it; nothing a request or a model writes reaches it. The route file of (b) is written once when the run takes its
  route, and the calls left change with every call.
- Without the variable (no daemon run, no cap) the node keeps the binding's `--max-turns`, 40 for the free run.
- A cap of 0 is refused before a run exists, as today: `runbook serve --max-llm-calls 0` does not start (the option
  takes 1 or more), and a delegation whose `settings.max_llm_calls` is `"0"` is refused at submit as a value that is
  not a whole number of 1 or more (`CapPolicy::resolve`, `runbook/src/caps.rs`). `sdc-node` refuses a
  `RUNBOOK_CALLS_LEFT` that is not a whole number of at least 1, as it refuses such a `--max-turns`.
- A cap of 1: the node starts with `--max-turns 1` when the run has used no call. A run with no calls left starts no
  node: it ends on the calls cap before its next step (`Gate::admit`, `runbook/src/engine/gate.rs`), so item 2 holds
  for calls already.
- Further cases: a cap of 30 gives `--max-turns 30`; a delegation that names no cap on a daemon started with
  `--max-llm-calls 7` gives `--max-turns 7`.
