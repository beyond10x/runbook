---
format: aep.planning-md/3
id: story:a-failed-call-names-the-adapter-error
kind: story
status: active
title: A failed call names the adapter error
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-11-26T06:30:54Z", actor: "agent:runbook", revision: 2}
- {from: "proposed", to: "active", at: "2026-11-26T06:30:54Z", actor: "agent:runbook", revision: 3}
---
## Outcome

A run that ends because a model call left no cost record says why in words: the adapter, its exit and the client's
own error. A value the call's record holds is named; a value it does not hold is written "not recorded", never `?`.

## Found

A daemon run on a route ended `runtime-failure` with "model step proposals.write: model call 001-proposals.write has
no cost record (adapter ?, exit ?): [claude-code:unrecognized_model] {...}". The call's `request.json` named the
adapter (`claude-cli`), `response.json` was empty, and `stderr.txt` held Claude Code's error: it did not know the model
it had asked for (`default[1m]`). The text named neither the adapter nor the model, and gave no hint which role names
the run's route file offered.

## Order

The operator, 2026-11-26 07:14 and 07:30 CET (relayed by the host operator): "all runs I observe fail"; the fixes for
the top failure causes are the head of the next wave, one pull request. That order is this story's acceptance.

## Acceptance

- A call directory with no cost line names the adapter of its `response.json`, else of its `request.json`, else
  "not recorded" (case `the_failure_of_a_call_without_a_cost_record_names_the_adapter_of_its_request`).
- It names the adapter's exit of its `response.json` (`adapter_exit`, else `exit`), else "exit not recorded" (cases
  `the_failure_of_a_call_without_a_cost_record_names_the_adapter_of_its_request` and
  `the_failure_of_a_call_without_a_cost_record_names_the_exit_of_its_response`).
- A cost line without an action or an adapter names the action of its call directory and the adapter of its
  `request.json`, else "not recorded" (case `a_cost_line_without_action_or_adapter_says_not_recorded_or_takes_them_from_the_call`).
- Claude Code's `unrecognized_model` error is said in words: the model the client asked for and the role names (with
  their models) of the run's route file, or that the run has no route file
  (cases `claude_codes_unrecognized_model_is_said_in_words_with_the_roles_of_the_route_file` and
  `failed_call_a_run_on_a_route_whose_call_left_no_cost_record_names_adapter_exit_and_the_clients_error`).
- No failure text of a call without a cost record holds `?` for a value (the daemon case compares the whole reason).

## Scope

- `runbook/src/cost.rs`: the reasons of a call without a usable cost (cited: `cost.rs:126-127`, `:190-195`).
- `runbook/src/engine/gate.rs`, `runbook/src/engine/route.rs`, `runbook/src/engine/mod.rs`: the gate's reader learns
  the role names of the run's route file (cited: `gate.rs:375-379`, `mod.rs:414-420`).
- Cases: unit cases in `runbook/src/cost.rs`; one daemon case on a route with a stand-in gateway,
  `runbook/tests/daemon/failed_call.rs`.
