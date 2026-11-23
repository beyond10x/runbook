---
format: aep.planning-md/3
id: story:model-programs-reach-a-gateway
kind: story
status: active
title: The model programs of a run reach a gateway the settings file names
summary: The run settings file names a gateway address and token for the model programs in a [gateway] table; with one named, no login file is linked, copied or required.
relations:
- decomposes: epic:ticket-to-merge-request-on-a-real-repository
scope:
- confidence: cited
  path: bindings/tools/llm-adapter-claude-cli.sh
- confidence: cited
  path: docs/loom-run-settings.md
- confidence: cited
  path: driver/src/bin/sdc-node/setup.rs
- confidence: cited
  path: runbook/src/loom_run/bindings_copy.rs
- confidence: cited
  path: runbook/src/loom_run/private.rs
- confidence: cited
  path: runbook/src/loom_run/settings.rs
revision: 11
transitions:
- {from: "draft", to: "proposed", at: "2026-11-23T11:37:51Z", actor: "human:operator", revision: 4, executor: "agent:runbook"}
- {from: "proposed", to: "active", at: "2026-11-23T11:37:51Z", actor: "human:operator", revision: 5, executor: "agent:runbook"}
---
## What a consumer met

A consumer's host sends every batch model call through a local gateway: an address and a token for the
model program, and no login file copied. The model programs of a run have no route to it (2026-11-23, read
from the tree at release 0.8.0 and at the default branch): the claude-cli adapter copies a login file or
reads an API key, Loom hands a bound program `PATH` and `LOOM_*` only, and no file of `bindings/`,
`driver/src` or `runbook/src` names a gateway address or a gateway token. The consumer's stand-in is a
program of its own first on the runner's `PATH` and a login file with no content.

## Outcome (the operator's answer to question D697, 2026-11-23: a gateway table in the settings)

The run settings file can carry a `[gateway]` table: `base_url`, and `token` or `token_file` (one of the
two). The runner hands it to the model programs the way it hands the login today: through a file in its
private directory and its own copy of the bindings, never through the environment Loom gives a program. An
adapter sets the address and the token for the model process only. With a gateway named, no login file is
linked, copied or required; a settings file that names both a gateway and a login file is refused.

## What a guard must hold

- The token value is never in an argv, a log line, an event, a run directory, a refusal of the settings
  reader or a committed file. It lies only in the runner's private directory (mode 700), which the runner
  removes with everything in it.
- A gateway address that is not loopback is a decision of the user, stated in the settings file; the runner
  adds no default address and no default token.
- Loom's program contract does not change: nothing is passed through Loom's environment.
- Under a confinement level through which the address cannot be reached, the run is refused before a run
  directory exists, by the name of the setting, or the docs name the levels at which a gateway works: the
  unit says which, from the pinned Loom.
- Every program of the runner that starts the model process takes the gateway: the model nodes, the
  decider, the review step. None keeps the login path when a gateway is named.

## Acceptance

Cases on a stand-in of the model process show the two values reaching that process and no other, no login
file in the runner's private directory, no token in any argv or file of the run directory, and the two
refusals. A security review pass holds the guards above before the merge.

## Origin

A gap named by a consumer session on 2026-11-23; the way was the operator's pick the same day.
