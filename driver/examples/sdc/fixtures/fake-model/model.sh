#!/usr/bin/env bash
# A fake model CLI for the cycles, called as `claude` (CLAUDE=…/fake-model/claude, through
# tools/llm-adapter-claude-cli.sh) or as `metaharness` (METAHARNESS=…/fake-model/metaharness).
# It writes no file, whatever tools it is offered: its only reply channel is the response schema.
# Given `--json-schema` (claude), the reply is the final result's structured_output; without it
# (metaharness has no such option) the reply is only text, as the model in runs/trial-3-strong-3
# gave it after its file write was refused. The reply: FAKE_MODEL_REPLY (JSON), else an approval
# with no finding. Every call appends its name and argv to FAKE_MODEL_LOG when set.
set -uo pipefail
me=$(basename "$0")
reply=${FAKE_MODEL_REPLY:-'{"decision": "approved", "summary": "fake model: approved", "findings": []}'}
[[ -n ${FAKE_MODEL_LOG:-} ]] && printf '%s\0' "$@" | jq -cRs --arg me "$me" '{cli: $me, argv: (split("\u0000") | .[:-1])}' >> "$FAKE_MODEL_LOG"
schema=no
while (( $# )); do
  [[ $1 == --json-schema ]] && schema=yes
  shift
done
text="I couldn't write out/review.json. Here is the reply: $(jq -c . <<< "$reply")"
case $me in
  claude)
    jq -cn --arg t "$text" '{type: "assistant", message: {content: [{type: "text", text: $t}]}}'
    if [[ $schema == yes ]]; then
      jq -cn --argjson r "$reply" '{type: "result", subtype: "success", is_error: false, num_turns: 1,
        duration_ms: 1, total_cost_usd: 0, result: "", structured_output: $r}'
    else
      jq -cn --arg t "$text" '{type: "result", subtype: "success", is_error: false, num_turns: 1,
        duration_ms: 1, total_cost_usd: 0, result: $t}'
    fi ;;
  metaharness)
    jq -cn '{event: "session.started", offered_tools: []}'
    jq -cn --arg t "$text" '{event: "session.ended", total_cost_usd: 0, num_turns: 1, is_error: false,
      subtype: "success", result: $t}' ;;
  *) echo "fake-model: called as $me, not claude or metaharness" >&2; exit 64 ;;
esac
