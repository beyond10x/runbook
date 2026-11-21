#!/usr/bin/env bash
# LLM adapter: one bounded, non-interactive agent session through metaharness (Claude Code, the
# operator's login on this host). Contract shared by every adapter (docs/driver.md, "LLM
# executor"):
#
#   llm-adapter-<name>.sh <request.json>
#
# request: {model, effort (optional), prompt_file, cwd, operations: [metaharness neutral ops], max_turns,
# max_budget_usd, record_dir, node, attempt}. The adapter runs the session with cwd = request.cwd,
# writes everything it observed under record_dir (argv.json, events.jsonl, transcripts/) and
# prints one response JSON on stdout: {adapter, model, exit, duration_ms, cost_usd, num_turns,
# usage, tool_calls, observed_models, is_error}. Its exit is the session's.
#
# Injection points: SDC_LLM_ENDPOINT (a model gateway root URL: metaharness --model-endpoint, with
# --credentials none) and METAHARNESS (the binary).
set -uo pipefail
for p in jq "${METAHARNESS:-metaharness}"; do command -v "$p" >/dev/null 2>&1 || { echo "llm-adapter-metaharness.sh: $p is not on PATH" >&2; exit 127; }; done # program error, not a result
req=$(cat "$1")
r() { jq -r "$1" <<< "$req"; }
rec=$(r .record_dir)
mkdir -p "$rec/transcripts"

# The frame: the operations admitted here, sealed with metaharness.frame/1's digest (sha256 of the
# compact, key-sorted document without `digest` and `format`).
jq -S --arg node "$(r .node)" --argjson attempt "$(r .attempt)" '{
    workflow: {id: "sdc", version: "1"}, node: {id: $node},
    step: {workflow: "sdc", state: $node, index: 1, attempt: $attempt},
    prior: [], obligations: [{asked_by: null, text: "write the output under out/ only"}],
    reaching: [], next: [], handoff: {handoff: "none"},
    operations: ([.operations[] | {op: .}] | sort_by(.op)), entities: null}' <<< "$req" > "$rec/frame.body.json"
digest=$(jq -cjS . "$rec/frame.body.json" | sha256sum | cut -d' ' -f1)
jq -S --arg d "$digest" '. + {format: "metaharness.frame/1", digest: $d}' "$rec/frame.body.json" > "$rec/frame.json"
rm -f "$rec/frame.body.json"

argv=("${METAHARNESS:-metaharness}" run claude --frame "$rec/frame.json" --decisions frame
  --cwd "$(r .cwd)" --scratch-cwd --model "$(r .model)" --max-turns "$(r .max_turns)"
  --max-budget-usd "$(r .max_budget_usd)" --retain-dir "$rec/transcripts")
[[ -n $(r '.effort // ""') ]] && argv+=(--effort "$(r .effort)")
if [[ -n ${SDC_LLM_ENDPOINT:-} ]]; then
  argv+=(--model-endpoint "$SDC_LLM_ENDPOINT" --credentials none)
fi
argv+=(-p "$(cat "$(r .prompt_file)")")
printf '%s\0' "${argv[@]}" | jq -Rs 'split("\u0000") | .[:-2] + ["<prompt_file>"]' > "$rec/argv.json"

start=$(date +%s%3N)
"${argv[@]}" < /dev/null > "$rec/events.jsonl" 2> "$rec/stderr.txt"
code=$?
end=$(date +%s%3N)

jq -s --arg model "$(r .model)" --arg effort "$(r '.effort // "default"')" --argjson code "$code" --argjson ms "$((end - start))" '
  (map(select(.event == "session.ended")) | last) as $e
  | (map(select(.event == "tool.decided")) | map({key: .call_id, value: .}) | from_entries) as $dec
  | {adapter: "metaharness", model: $model, effort: $effort, exit: $code, duration_ms: ($e.duration_ms // $ms),
     wall_ms: $ms, cost_usd: $e.total_cost_usd, num_turns: $e.num_turns, usage: $e.usage,
     observed_models: $e.observed_models, is_error: $e.is_error, subtype: $e.subtype,
     terminal_reason: $e.terminal_reason, api_error_status: $e.api_error_status,
     offered_tools: (map(select(.event == "session.started")) | last | .offered_tools),
     tool_calls: [.[] | select(.event == "tool.requested")
       | {name, operations, input: (.input | tostring | .[0:300]),
          decision: ($dec[.call_id].decision // $dec[.call_id].verdict // "allowed")}],
     stream_end: (map(select(.event == "stream.closed")) | last | {reason, process})}' \
  "$rec/events.jsonl" 2>/dev/null || jq -n --arg model "$(r .model)" --arg effort "$(r '.effort // "default"')" --argjson code "$code" \
  --argjson ms "$((end - start))" '{adapter: "metaharness", model: $model, effort: $effort, exit: $code, wall_ms: $ms,
  error: "no readable event stream"}'
exit $code
