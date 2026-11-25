#!/bin/sh
# The Runbook plugin's delegation hook for Claude Code.
#
#   delegation.sh pre-tool-use   denies the Agent call on standard input, naming the runbook
#                                `delegate` tool, the prompt's first line and the session's route
#   delegation.sh session-start  says the same as context at the start of the session
#
# Input: the hook's JSON on standard input. Output: one JSON object on standard output, exit 0.
# It calls sed and tr and nothing else: no network, no other program. Any event other than
# session-start, and any input it cannot read, is answered with the deny.

LC_ALL=C
export LC_ALL

event=${1-}

hi=$(printf '\200-\377')
cont=$(printf '\200-\277')
lead2=$(printf '\302-\337')
lead3=$(printf '\340-\357')
lead4=$(printf '\360-\364')
utf8_char="[$lead2][$cont]|[$lead3][$cont][$cont]|[$lead4][$cont][$cont][$cont]"

# Prints $1 when it is UTF-8 throughout, else nothing.
utf8() {
	printf '%s\n' "$1" | sed -n -E "/^([^$hi]|$utf8_char)*\$/p"
}

# $1 as the body of a JSON string: control characters become spaces, backslash and double quote
# are escaped; text that is not UTF-8 becomes "(not UTF-8)".
quote() {
	q=$(printf '%s' "$1" | tr '\000-\037' '                                ' | sed 's/\\/\\\\/g; s/"/\\"/g')
	if [ -n "$q" ] && [ -z "$(utf8 "$q")" ]; then
		q='(not UTF-8)'
	fi
	printf '%s' "$q"
}

route=${RUNBOOK_MODEL_ROUTE-}
if [ -n "$route" ]; then
	route_use="\`model_route\` = \`$(quote "$route")\` (RUNBOOK_MODEL_ROUTE of this session)"
else
	route_use="no \`model_route\`, so the daemon's default route applies (RUNBOOK_MODEL_ROUTE is not set)"
fi

input=$(tr '\000-\037' '                                ')

if [ "$event" = session-start ]; then
	context="This session has the Runbook plugin: its sub-agent work is served by Runbook. Call the runbook MCP tool \`delegate\` with \`request\` = the task and $route_use; the Agent tool is denied. \`delegation_result\` reads a delegation that outlasted its call; \`describe_catalog\` lists the routes and runbooks a delegation may name."
	printf '{"hookSpecificOutput":{"hookEventName":"SessionStart","additionalContext":"%s"}}\n' "$context"
	exit 0
fi

# A call of another tool passes: the hook answers nothing. A tool name it cannot read, or any
# "tool_name" of Agent in the input, is an Agent call.
tool=$(printf '%s\n' "$input" | sed -n 's/.*"tool_name"[[:space:]]*:[[:space:]]*"\([A-Za-z0-9_]*\)".*/\1/p')
agent_named=$(printf '%s\n' "$input" | sed -n '/"tool_name"[[:space:]]*:[[:space:]]*"Agent"/p')
if [ -n "$tool" ] && [ "$tool" != Agent ] && [ -z "$agent_named" ]; then
	exit 0
fi

# The prompt's first line, still JSON-escaped: the escapes up to the first \n or \r, or to the
# string's end; at most 200 characters, cut on a character boundary.
line=$(printf '%s\n' "$input" | sed -n -E 's/.*"prompt"[[:space:]]*:[[:space:]]*"(([^"\\]|\\u[0-9a-fA-F]{4}|\\[^nru])*).*/\1/p')
line=$(utf8 "$line")
line=$(printf '%s\n' "$line" | sed -E "s/^(([^\\\\$hi]|$utf8_char|\\\\u[0-9a-fA-F]{4}|\\\\[^u]){200}).+\$/\\1 .../")
line=$(printf '%s' "$line" | sed 's/^[[:space:]]*//; s/[[:space:]]*$//')

if [ -n "$line" ]; then
	prompt="The prompt begins: $line"
else
	prompt="The prompt could not be read from the hook's input: pass the prompt this Agent call was given."
fi

reason="Sub-agents in this session are served by Runbook: this Agent call is denied and runs nowhere. Call the runbook MCP tool \`delegate\` instead, with \`request\` = this Agent call's full prompt and $route_use. \`describe_catalog\` lists the routes and runbooks. $prompt"
printf '{"hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"deny","permissionDecisionReason":"%s"}}\n' "$reason"
