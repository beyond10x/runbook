#!/bin/sh
# The Runbook plugin's delegation hook for Claude Code.
#
#   delegation.sh pre-tool-use   denies the Agent or Workflow call on standard input, naming the
#                                runbook `delegate` tool, the prompt's first non-blank line and the
#                                session's route
#   delegation.sh session-start  says the same as context at the start of the session
#
# Input: the hook's JSON on standard input. Output: one JSON object on standard output, exit 0.
# It calls sed and tr and nothing else: no network, no other program. Any event other than
# session-start, and any input it cannot read, is answered with the deny.

LC_ALL=C
export LC_ALL

event=${1-}

# One UTF-8 character of two to four bytes (RFC 3629, table 3-7 of Unicode): no overlong form,
# no surrogate, nothing above U+10FFFF.
hi=$(printf '\200-\377')
c=$(printf '\200-\277')
utf8_char=$(printf '[\302-\337][%s]|\340[\240-\277][%s]|[\341-\354\356\357][%s][%s]|\355[\200-\237][%s]|\360[\220-\277][%s][%s]|[\361-\363][%s][%s][%s]|\364[\200-\217][%s][%s]' \
	"$c" "$c" "$c" "$c" "$c" "$c" "$c" "$c" "$c" "$c" "$c" "$c")

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
	context="This session has the Runbook plugin: its sub-agent work is served by Runbook. Call the runbook MCP tool \`delegate\` with \`request\` = the task and $route_use; the Agent and Workflow tools are denied. \`delegation_result\` reads a delegation that outlasted its call; \`describe_catalog\` lists the routes and runbooks a delegation may name."
	printf '{"hookSpecificOutput":{"hookEventName":"SessionStart","additionalContext":"%s"}}\n' "$context"
	exit 0
fi

# Both tools start sub-agents: Agent one, Workflow many from a script. A call of another tool
# passes: the hook answers nothing. A tool name it cannot read, or any "tool_name" of Agent or
# Workflow in the input, is denied.
tool=$(printf '%s\n' "$input" | sed -n 's/.*"tool_name"[[:space:]]*:[[:space:]]*"\([A-Za-z0-9_]*\)".*/\1/p')
spawner_named=$(printf '%s\n' "$input" | sed -n -E '/"tool_name"[[:space:]]*:[[:space:]]*"(Agent|Workflow)"/p')
case $tool in
Agent | Workflow) call="$tool call" ;;
'') call=call ;;
*)
	[ -z "$spawner_named" ] && exit 0
	call=call
	;;
esac

# The prompt's first non-blank line, still JSON-escaped: blanks and \n, \r, \t escapes at its
# start are skipped, then the escapes up to the next \n or \r, or to the string's end; at most
# 200 characters, cut on a character boundary.
line=$(printf '%s\n' "$input" | sed -n -E 's/.*"prompt"[[:space:]]*:[[:space:]]*"([[:space:]]|\\[nrt])*(([^"\\]|\\u[0-9a-fA-F]{4}|\\[^nru])*).*/\2/p')
line=$(utf8 "$line")
line=$(printf '%s\n' "$line" | sed -E "s/^(([^\\\\$hi]|$utf8_char|\\\\u[0-9a-fA-F]{4}|\\\\[^u]){200}).+\$/\\1 .../")
line=$(printf '%s' "$line" | sed 's/^[[:space:]]*//; s/[[:space:]]*$//')

if [ -n "$line" ]; then
	prompt="The prompt begins: $line"
else
	prompt="No prompt could be read from the hook's input: pass the task this $call was given."
fi

reason="Sub-agents in this session are served by Runbook: this $call is denied and runs nowhere. Call the runbook MCP tool \`delegate\` instead, one call per sub-agent task, with \`request\` = the task's full text and $route_use. \`describe_catalog\` lists the routes and runbooks. $prompt"
printf '{"hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"deny","permissionDecisionReason":"%s"}}\n' "$reason"
