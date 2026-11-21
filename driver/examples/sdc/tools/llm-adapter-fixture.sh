#!/usr/bin/env bash
# LLM adapter for tests, no LLM (contract: llm-adapter-metaharness.sh). It writes the node's stub
# output into the call's out/, so llm-node.sh applies it exactly as it applies a real reply: the
# apply step (for example questions.answer's refusal to take a changed scope-changing question)
# runs, which the stub adapter skips. Only for nodes whose stub is `stub-file <fixture> <artifact>`;
# use it with SDC_LLM_NODES naming those nodes. SDC_STUB_FIXTURES applies as for the stub.
# SDC_FIXTURE_STUB (JSON {node: [fixture, artifact]}) gives a node such a stub when its binding's
# stub is another subcommand (plan.decompose: fixtures/plan-stories.json as stories.json).
#
#   llm-adapter-fixture.sh <request.json>
set -uo pipefail
for p in jq; do command -v "$p" >/dev/null 2>&1 || { echo "llm-adapter-fixture.sh: $p is not on PATH" >&2; exit 127; }; done # program error, not a result
here=$(cd "$(dirname "$0")" && pwd)
cwd=$(jq -r .cwd "$1")
node=$(jq -r .node "$1")
cfg=${SDC_LLM_NODE:-'{}'}
mapfile -t a < <(jq -r '.stub[]?' <<< "$cfg")
o=$(jq -c --arg n "$node" '.[$n] // empty' <<< "${SDC_FIXTURE_STUB:-null}")
[[ -n $o ]] && mapfile -t a < <(jq -r '"stub-file", .[]' <<< "$o")
[[ ${a[0]:-} == stub-file && -n ${a[2]:-} ]] ||
  { echo "llm-adapter-fixture: node $node has no stub-file stub" >&2; exit 64; }
mkdir -p "$cwd/out"
(cd "$cwd" && "$here/sdc-tool.sh" stub-file "${a[1]}" "out/${a[2]}") || exit 1
jq -n --arg node "$node" '{adapter: "fixture", model: "none", exit: 0, duration_ms: 0, cost_usd: 0,
  num_turns: 0, usage: null, tool_calls: [], offered_tools: [], node: $node}'
