#!/usr/bin/env bash
# LLM adapter for tests, no LLM (contract: llm-adapter-metaharness.sh). It writes the node's stub
# output into the call's out/, so llm-node.sh applies it exactly as it applies a real reply: the
# apply step (for example questions.answer's refusal to take a changed scope-changing question)
# runs, which the stub adapter skips. Only for nodes whose stub is `stub-file <fixture> <artifact>`;
# use it with SDC_LLM_NODES naming those nodes. SDC_STUB_FIXTURES applies as for the stub.
# SDC_FIXTURE_STUB (JSON {node: [fixture, artifact]}) gives a node such a stub when its binding's
# stub is another subcommand (plan.decompose: fixtures/plan-stories.json as stories.json).
#
# The decider's calls (node `decide.<action>`, tools/decider.sh) get a scripted reply in
# out/decision.json: SDC_DECIDER_REPLIES (JSON {key: reply}) by key, the request's question id,
# finding id or action (in/decision.json); without one, approve, with a question's or a finding's
# default as the answer; a list of replies is taken by the request's `attempt` (the n-th, else the
# last). The response names the model the request named.
#
#   llm-adapter-fixture.sh <request.json>
set -uo pipefail
for p in jq; do command -v "$p" >/dev/null 2>&1 || { echo "llm-adapter-fixture.sh: $p is not on PATH" >&2; exit 127; }; done # program error, not a result
here=$(cd "$(dirname "$0")" && pwd)
cwd=$(jq -r .cwd "$1")
node=$(jq -r .node "$1")
model=$(jq -r '.model // "none"' "$1")
if [[ $node == decide.* ]]; then
  mkdir -p "$cwd/out"
  # A list of replies for one key is taken by attempt (the decider's retries): the n-th, else the last.
  jq --argjson r "${SDC_DECIDER_REPLIES:-"{}"}" --argjson n "$(jq '.attempt // 1' "$1")" '(.question.id // .finding.id // .action) as $k
    | ($r[$k] | if type == "array" then (.[$n - 1] // last) else . end)
      // {decision: "approve", answer: (if .type == "question" then .question.default
        elif .type == "finding" then .finding.default else null end),
      reasons: ["fixture decider: approve, the default where there is one"], cites: [], best: null}' \
    "$cwd/in/decision.json" > "$cwd/out/decision.json" || exit 1
  # A scripted cite `<artifact>@{rev}...` names the artifact's current revision (citable_artifacts).
  jq --slurpfile d "$cwd/in/decision.json" '($d[0].citable_artifacts // [] | map({key: .artifact, value: .revision}) | from_entries) as $rev
    | if (.cites | type) == "array" then .cites |= map(if type == "string" and test("^[a-z_]+@\\{rev\\}")
        then (capture("^(?<a>[a-z_]+)@\\{rev\\}(?<rest>.*)$") | "\(.a)@\($rev[.a] // "none")\(.rest)") else . end) else . end' \
    "$cwd/out/decision.json" > "$cwd/out/decision.json.tmp" 2>/dev/null && mv "$cwd/out/decision.json.tmp" "$cwd/out/decision.json"
  rm -f "$cwd/out/decision.json.tmp"
  jq -n --arg node "$node" --arg model "$model" '{adapter: "fixture", model: $model, exit: 0, duration_ms: 0,
    cost_usd: 0, num_turns: 0, usage: null, tool_calls: [], offered_tools: [], node: $node}'
  exit 0
fi
cfg=${SDC_LLM_NODE:-'{}'}
mapfile -t a < <(jq -r '.stub[]?' <<< "$cfg")
o=$(jq -c --arg n "$node" '.[$n] // empty' <<< "${SDC_FIXTURE_STUB:-null}")
[[ -n $o ]] && mapfile -t a < <(jq -r '"stub-file", .[]' <<< "$o")
[[ ${a[0]:-} == stub-file && -n ${a[2]:-} ]] ||
  { echo "llm-adapter-fixture: node $node has no stub-file stub" >&2; exit 64; }
mkdir -p "$cwd/out"
(cd "$cwd" && "$here/sdc-tool.sh" stub-file "${a[1]}" "out/${a[2]}") || exit 1
jq -n --arg node "$node" --arg model "$model" '{adapter: "fixture", model: $model, exit: 0, duration_ms: 0, cost_usd: 0,
  num_turns: 0, usage: null, tool_calls: [], offered_tools: [], node: $node}'
