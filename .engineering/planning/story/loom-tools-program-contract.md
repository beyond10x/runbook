---
format: aep.planning-md/3
id: story:loom-tools-program-contract
kind: story
status: active
title: The tools the Loom bindings call work under Loom's program contract, the driver unchanged
summary: 'Step 2 of the move onto Loom: tools under bindings/tools/, LOOM_CASE selects Loom mode (arguments on stdin, LOOM_RETRY_FEEDBACK, siblings from PATH or the tool''s directory, argv for action, principal, stubs, model, adapter); driver mode byte for byte.'
tags:
- conductor-succession
- loom
relations:
- decomposes: epic:conductor-succession
- informed_by: story:loom-bindings-files
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T22:31:29Z", actor: "human:operator", revision: 2, executor: "agent:feat-loom-tools", correlation: "operator-standing-acceptance-2026-11-21"}
- {from: "proposed", to: "active", at: "2026-11-21T22:31:29Z", actor: "human:operator", revision: 3, executor: "agent:feat-loom-tools", correlation: "operator-standing-acceptance-2026-11-21"}
---
# Story: loom-tools-program-contract

## Outcome

Step 2 of the move of Runbook onto Loom (the operator, 2026-11-21: "runbook runs on loom
entirely"). The tools the nine `bindings/*.yaml` files call work under Loom's program contract,
and the driver keeps running them unchanged until it is deleted ("dual mode"):

- The tools live under `bindings/tools/`, one copy each, moved from `driver/examples/sdc/tools/`;
  every reference to the old path is changed. A bindings file reaches no program outside its own
  directory: every `argv` and `arguments_from` entry of `bindings/*.yaml` names `tools/<file>`
  relative to the bindings file, with no `..`; a program from `PATH` carries no `/`.
- `LOOM_CASE` set means Loom mode; unset means driver mode, today's behaviour byte for byte. In
  Loom mode a tool reads its arguments as one JSON object on standard input, the case id from
  `LOOM_CASE`, retry feedback from the `loom.retry-feedback/1` document `LOOM_RETRY_FEEDBACK`
  names, the sibling binaries (`sdc-findings`, `sdc-bugfix`, `sdc-reply`) from `PATH` or relative
  to its own directory and never from `HOME`, and the action id, principal, stub fixtures, stub
  counter, model and adapter from argv entries the binding states.
- What Loom's integration head cannot express yet stays on the driver-mode path, one line per item
  in `bindings/PENDING.md` naming the Loom story it waits for.

## Acceptance

1. `grep -n '\.\./' bindings/*.yaml` prints nothing; every program path in them exists under
   `bindings/tools/` and is executable (`scripts/check-bindings.sh`); the load check loads all
   nine files with Loom's loader at the newest Loom integration head.
2. Driver mode unchanged: `scripts/check-bindings.sh` passes, the driver crate's tests that touch
   the tools pass, and a canary of three finding cycles (`bugfix-never-red`, one `replay-*`, one
   that runs `llm-node.sh` with the stub adapter) gives the same verdict lines on the base commit
   and on the head.
3. Loom mode: a test in the driver crate's integration-test binary starts `sdc-tool.sh` and
   `llm-node.sh` as Loom does (`env_clear`, only `PATH` and `LOOM_CASE`, the arguments as JSON on
   standard input, start directory a temporary workspace) for one check subcommand,
   `list-stories`, one `llm-node.sh` action on the stub adapter and one retry with
   `LOOM_RETRY_FEEDBACK`, and each asserts the same output files and exit code as the driver-mode
   run of the same input.
4. `docs/loom-bindings.md` says per row of "Tool changes for Loom's program contract" done or
   pending (with the Loom story); `bindings/PENDING.md` is updated; generated site data that holds
   the paths is regenerated; `aep plan artifact validate` and the site-data check pass.

## Scope

`bindings/`, `driver/examples/sdc/tools/` (moved), `driver/examples/sdc/*.bindings.json`,
`driver/src/bin/` (paths only), `driver/tests/`, `scripts/`, `docs/`, `website/`, `AGENTS.md`.
No new Rust runtime code; the 14 prompts inside `llm-node.sh` stay byte for byte.
