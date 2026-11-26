---
name: delegation
description: "Hand sub-agent work to Runbook in a Claude Code session that runs with the Runbook plugin: the runbook MCP tool `delegate` runs the task as a Runbook run on a named model route and returns its result, and the Agent and Workflow tools are denied. Use when a task would go to a sub-agent, when an Agent or Workflow call was denied with a reason naming `delegate`, or when asked which routes or runbooks a delegation may name. Not for a session without the runbook MCP tools (the plugin is not enabled there), and not for starting a runbook by its id with an intake (the `submit_run` tool)."
---

# delegation

In this session Runbook serves every sub-agent. The plugin's `PreToolUse` hook denies each Agent and Workflow call, so a
task sent there runs nowhere. Send it to the runbook MCP tool `delegate` instead.

## Route

The session's route is the value of `RUNBOOK_MODEL_ROUTE` in its environment, named in the denial reason and in
the context line the plugin adds when the session starts. When it is not set, leave `model_route` out: the
daemon's default route applies. A `delegate` call may name another route; the daemon admits or refuses it.

## Delegate

Claude Code lists the tool as `mcp__plugin_runbook_runbook__delegate`. When the session shows that name only
(tool search defers MCP tools), load it first with ToolSearch, query `select:mcp__plugin_runbook_runbook__delegate`;
`delegation_result` and `describe_catalog` load the same way.

1. Call `delegate` with `request` = the full task text, the text an Agent call's prompt would have held, and
   `model_route` = the session's route when it has one. Name a `protocol` only when `describe_catalog` lists a
   runbook for the task.
2. The call answers with the run's result, the row of `delegation_result`: `run_id`, `run_state`, `outcome`,
   `route`, `path`, `protocol`, `calls`, `cost_micro_usd` and the run's final text in `result_text` (at most
   40,000 characters; `result_truncated` says it was cut). Claude Code shows that row; a client that reads the
   tool's text gets a first line with the run id, route, path, protocol, calls and cost, then the final text.
   A call that runs past two minutes moves to a background task, and its result arrives as a task notification.
3. When `delegate` answered a run id and a state without a result, read the result with `delegation_result` and
   that run id.

This step is done when the task's result is in the conversation, or a tool error names why it is not.

## A refusal

- A route the daemon does not declare comes back as the tool error `route-refused` with the declared routes; a
  route whose gateway is not ready as `route-unavailable` naming the route (HTTP status 503); a route that fails
  the run's call ends the run `runtime-failure`, a tool error whose `failure` names `route-unavailable` and the
  route. Pick a route from `list_routes` or `describe_catalog`, or report the error.
- A run whose agent step failed ends `failed: step <action> <end>` (`exit <n>`, `signal <n>`, `timed out after <s> s`
  or `ended with no exit`), a tool error whose `failure` names the step and how it ended. Report it, or delegate again with a clearer task. The Agent and Workflow tools
  stay denied; the session's own model takes no sub-agent work.
- `daemon unreachable` (status 502): the daemon named by `RUNBOOK_DAEMON` does not answer. Report it.

## The catalog

`describe_catalog` lists the routes with their kind and status, the runbooks with their inputs, and the limits of
a delegation. Read it before naming a route or a protocol the session has not used yet.
