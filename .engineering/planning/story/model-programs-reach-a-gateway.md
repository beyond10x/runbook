---
format: aep.planning-md/3
id: story:model-programs-reach-a-gateway
kind: story
status: draft
title: The model programs of a run reach a gateway the settings file names
summary: 'The run settings file names a gateway address and token for the model programs; with one named, no login file is copied. Draft: the way waits for the operator''s answer.'
revision: 1
---
## What a consumer met

A consumer's host sends every batch model call through a local gateway: an address and a token for the
model program, and no login file copied. The model programs of a run have no route to it (2026-11-23, read
from the tree at release 0.8.0 and at the default branch): the claude-cli adapter copies a login file or
reads an API key, Loom hands a bound program `PATH` and `LOOM_*` only, and no file of `bindings/`,
`driver/src` or `runbook/src` names a gateway address or a gateway token. The consumer's stand-in is a
program of its own first on the runner's `PATH` and a login file with no content.

## Outcome (a draft: the way is not decided)

The run settings file can name a gateway: an address, and a token or the path of a file that holds it. The
runner hands it to the model programs the way it hands the login today (through its own copy of the
bindings, never through the environment Loom gives a program), and an adapter sets it for the model process
only. With a gateway named, no login file is linked, copied or required.

Open, and the operator's to answer (question D697): this way, or nothing in the runbook, or waiting for
Loom's story on a declared environment of a bound program.

## What a guard must hold

- The token value is never in an argv, a log line, an event, a run directory, a refusal of the settings
  reader or a committed file.
- A gateway address that is not loopback is a decision of the user, stated in the settings file; the runner
  adds no default address.
- Loom's program contract does not change: nothing is passed through Loom's environment.
- Under a confinement level without network beyond loopback, a loopback gateway is reachable or the run is
  refused before a run directory exists, by the name of the setting.

## Acceptance

A run on the stub of the model program shows the two values reaching that program and no login file in the
runner's private directory; a security review pass holds the four guards above before the merge.

## Origin

A gap named by a consumer session on 2026-11-23. Not started: it waits for the answer to D697.
