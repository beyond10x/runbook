#!/usr/bin/env bash
# LLM adapter: `claude -p` directly, for when metaharness cannot be used. Same contract as
# llm-adapter-metaharness.sh. The neutral operations map to Claude Code tool names, passed as an
# explicit --allowedTools list; everything else is disallowed.
set -uo pipefail
for p in jq claude; do command -v "$p" >/dev/null 2>&1 || { echo "llm-adapter-claude-cli.sh: $p is not on PATH" >&2; exit 127; }; done # program error, not a result
req=$(cat "$1")
r() { jq -r "$1" <<< "$req"; }
rec=$(r .record_dir)
mkdir -p "$rec"
tools=$(jq -r '[.operations[] | {"file.read": "Read", "file.write": "Write", "file.edit": "Edit",
  "dir.list": "Glob", "search": "Grep", "shell": "Bash"}[.] // empty] | join(",")' <<< "$req")
argv=("${CLAUDE:-claude}" -p "$(cat "$(r .prompt_file)")" --model "$(r .model)"
  --max-turns "$(r .max_turns)" --max-budget-usd "$(r .max_budget_usd)"
  --allowedTools "$tools" --disallowedTools "Bash,WebFetch,WebSearch,Task,Skill"
  --permission-mode acceptEdits --output-format stream-json --verbose --no-session-persistence)
[[ -n $(r '.effort // ""') ]] && argv+=(--effort "$(r .effort)")
# A request with `json_schema` (a schema file): `--json-schema`, the CLI validates the reply against
# it and re-prompts on a mismatch; the reply is the final result's `structured_output`, written to
# <record>/structured-output.json.
schema=$(r '.json_schema // ""')
[[ -n $schema && -s $schema ]] && argv+=(--json-schema "$(jq -c . "$schema")")
printf '%s\0' "${argv[@]}" | jq -Rs 'split("\u0000") | .[0:2] + ["<prompt_file>"] + .[3:-1]
  | map(if startswith("{") then "<json_schema>" else . end)' > "$rec/argv.json"
start=$(date +%s%3N)
(cd "$(r .cwd)" && "${argv[@]}" < /dev/null > "$rec/events.jsonl" 2> "$rec/stderr.txt")
code=$?
end=$(date +%s%3N)
constrained=false
if [[ -n $schema && -s $schema ]] &&
   jq -se '(map(select(.type == "result")) | last | .structured_output) | type == "object"' "$rec/events.jsonl" > /dev/null 2>&1; then
  jq -s '(map(select(.type == "result")) | last | .structured_output)' "$rec/events.jsonl" > "$rec/structured-output.json"
  constrained=true
fi
jq -s --arg model "$(r .model)" --arg effort "$(r '.effort // "default"')" --argjson code "$code" --argjson ms "$((end - start))" --argjson sc "$constrained" \
  --arg schema "$schema" '
  (map(select(.type == "result")) | last) as $e
  | {adapter: "claude-cli", model: $model, effort: $effort, exit: $code, duration_ms: ($e.duration_ms // $ms), wall_ms: $ms,
     cost_usd: $e.total_cost_usd, num_turns: $e.num_turns, usage: $e.usage, is_error: $e.is_error,
     subtype: $e.subtype, schema_constrained: $sc,
     schema_note: (if $sc then "claude -p --json-schema: the CLI validated the reply against the schema (re-prompting on a mismatch)"
       elif $schema != "" then "a schema was passed, but the result has no structured_output" else null end),
     tool_calls: [.[] | select(.type == "assistant") | .message.content[]? | select(.type == "tool_use")
       | {name, input: (.input | tostring | .[0:300])}]}' "$rec/events.jsonl" 2>/dev/null \
  || jq -n --argjson code "$code" '{adapter: "claude-cli", exit: $code, error: "no readable output"}'
exit $code
