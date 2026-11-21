#!/usr/bin/env bash
# LLM adapter for tests, no LLM (contract: llm-adapter-metaharness.sh). It writes the node's stub
# output into the call's out/, so llm-node.sh applies it exactly as it applies a real reply: the
# apply step (for example questions.answer's refusal to take a changed scope-changing question)
# runs, which the stub adapter skips. Only for nodes whose stub is `stub-file <fixture> <artifact>`
# or `stub-tree <fixture dir> <tree>` (spec.author); use it with SDC_LLM_NODES naming those nodes. SDC_STUB_FIXTURES applies as for the stub.
# SDC_FIXTURE_STUB (JSON {node: [fixture, artifact]}) gives a node such a stub when its binding's
# stub is another subcommand (plan.decompose: fixtures/plan-stories.json as stories.json); three
# entries name the subcommand too (test.reproduce: ["stub-tree", "bugfix-pager/stub-repro", "repro"]).
#
# The decider's calls (node `decide.<action>`, tools/decider.sh) get a scripted reply in
# out/decision.json: SDC_DECIDER_REPLIES (JSON {key: reply}) by key, the request's question id,
# finding id or action (in/decision.json); without one, approve, with a question's or a finding's
# default as the answer; a list of replies is taken by the request's `attempt` (the n-th, else the
# last); `{"rounds": [...]}` is taken by the number of the approval's earlier rejections
# (`prior_rejections`), each round a reply or a list by attempt. The response names the model the
# request named.
#
# change.review gets a scripted reply in out/review.json: SDC_REVIEW_REPLIES, `{"rounds": [...]}`
# taken by the number of the case's earlier rejected reviews (sdc-reply rejections), the last round
# after that, each round a reply or a list by the request's `review_attempt`; without one, approved
# with no finding.
# change.implement's coverage reply goes to out/coverage.json: SDC_COVERAGE_REPLIES, `{"rounds":
# [...]}` taken by the request's `attempt` (the node's n-th call record, the last round after
# that), each round a reply or a list by `coverage_attempt`; else the tree fixture's
# .sdc/coverage.json; else the stub document (sdc-reply coverage-stub).
# run.unstick (scripts/run-trial.sh) gets out/unstick.json from SDC_UNSTICK_REPLIES (JSON {cause:
# reply}, "*" for any cause; a list by attempt); without one, the move `stop`.
#
#   llm-adapter-fixture.sh <request.json>
set -uo pipefail
[[ ${1:-} == --capabilities ]] && { echo '{"json_schema": true}'; exit 0; } # scripted replies are the schema document
for p in jq; do command -v "$p" >/dev/null 2>&1 || { echo "llm-adapter-fixture.sh: $p is not on PATH" >&2; exit 127; }; done # program error, not a result
here=$(cd "$(dirname "$0")" && pwd)
cwd=$(jq -r .cwd "$1")
node=$(jq -r .node "$1")
model=$(jq -r '.model // "none"' "$1")
if [[ $node == decide.* ]]; then
  mkdir -p "$cwd/out"
  # A list of replies for one key is taken by attempt (the decider's retries): the n-th, else the last.
  jq --argjson r "${SDC_DECIDER_REPLIES:-"{}"}" --argjson n "$(jq '.attempt // 1' "$1")" '(.question.id // .finding.id // .action) as $k
    | (.prior_rejections // [] | length) as $p
    | ($r[$k] | if type == "object" and has("rounds") then (.rounds[$p] // (.rounds | last)) else . end
      | if type == "array" then (.[$n - 1] // last) else . end)
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
if [[ $node == run.unstick ]]; then
  mkdir -p "$cwd/out"
  jq --argjson r "${SDC_UNSTICK_REPLIES:-"{}"}" --argjson n "$(jq '.attempt // 1' "$1")" '($r[.cause] // $r["*"]
    | if type == "array" then (.[$n - 1] // last) else . end)
    // {move: "stop", diagnosis: "fixture run.unstick: no scripted reply, the run stops as it would have.",
      defect_class: "other", reasons: ["no scripted reply"]}' "$cwd/in/stop.json" > "$cwd/out/unstick.json" || exit 1
  jq -n --arg node "$node" --arg model "$model" '{adapter: "fixture", model: $model, exit: 0, duration_ms: 0,
    cost_usd: 0, num_turns: 0, usage: null, tool_calls: [], offered_tools: [], node: $node}'
  exit 0
fi
# The requester agent's calls (node `requester.<question id or action>`, decider.sh requester) get a
# scripted reply in out/reply.json: SDC_REQUESTER_REPLIES (a JSON object, or a file holding one) by
# the question id or action; `{goals_rev}` in it becomes the case's current goals revision. Without
# one: option A, grounded on the idea's first line.
if [[ $node == requester.* ]]; then
  mkdir -p "$cwd/out"
  rr=${SDC_REQUESTER_REPLIES:-"{}"}
  [[ -f $rr ]] && rr=$(cat "$rr")
  grev=$(jq -r '.artifacts.goals.revision // "none"' "$(jq -r .case_dir "$1")/case.json" 2>/dev/null)
  jq --argjson r "$rr" --arg k "${node#requester.}" --arg g "$grev" '($r[$k] // {option: "A", answer: null, strength: "moderate",
      no_ground_in_goals: false, reasons: ["fixture requester: option A"],
      grounds: [{source: "idea", citation: {form: "path_line", value: {path: "work/idea.yaml", line: 1}}, supports: "A", how: "fixture: the idea"}]})
    | walk(if type == "string" then gsub("\\{goals_rev\\}"; $g) else . end)' -n > "$cwd/out/reply.json" || exit 1
  jq -n --arg node "$node" --arg model "$model" '{adapter: "fixture", model: $model, exit: 0, duration_ms: 0,
    cost_usd: 0, num_turns: 0, usage: null, tool_calls: [], offered_tools: [], node: $node}'
  exit 0
fi
if [[ $node == change.review ]]; then
  mkdir -p "$cwd/out"
  p=$("${SDC_REPLY:-$here/../../../target/debug/sdc-reply}" rejections --case-dir "$(jq -r .case_dir "$1")" --action change.review | jq length)
  jq -n --argjson r "${SDC_REVIEW_REPLIES:-"{}"}" --argjson p "${p:-0}" --argjson n "$(jq '.review_attempt // 1' "$1")" '
    ($r.rounds // [] | .[$p] // last) | if type == "array" then (.[$n - 1] // last) else . end
    // {decision: "approved", summary: "fixture review: approved", findings: []}' > "$cwd/out/review.json" || exit 1
  jq -n --arg node "$node" --arg model "$model" '{adapter: "fixture", model: $model, exit: 0, duration_ms: 0,
    cost_usd: 0, num_turns: 0, usage: null, tool_calls: [], offered_tools: [], node: $node}'
  exit 0
fi
cfg=${SDC_LLM_NODE:-'{}'}
mapfile -t a < <(jq -r '.stub[]?' <<< "$cfg")
o=$(jq -c --arg n "$node" '.[$n] // empty' <<< "${SDC_FIXTURE_STUB:-null}")
[[ -n $o ]] && mapfile -t a < <(jq -r 'if length == 3 then .[] else ("stub-file", .[]) end' <<< "$o")
[[ ${a[0]:-} == stub-file || ${a[0]:-} == stub-tree ]] && [[ -n ${a[2]:-} ]] ||
  { echo "llm-adapter-fixture: node $node has no stub-file or stub-tree stub" >&2; exit 64; }
mkdir -p "$cwd/out"
# Run in the call's directory; a stub-tree's call count stays in work/.sdc (llm-node.sh runs this in
# work/), so <name>.<n> of SDC_STUB_FIXTURES counts the stub's and this adapter's calls together.
w=$PWD
mkdir -p "$w/.sdc"
(cd "$cwd" && SDC_STUB_COUNTER="$w/.sdc" "$here/sdc-tool.sh" "${a[0]}" "${a[1]}" "out/${a[2]}") || exit 1
# change.implement's coverage reply: the tree fixture's own .sdc/coverage.json (moved out of the
# tree, as a model writes it), else the stub document (sdc-reply coverage-stub).
if [[ $node == change.implement ]]; then
  rm -f "$cwd/out/coverage.json"
  if [[ -n ${SDC_COVERAGE_REPLIES:-} ]]; then
    jq -n --argjson r "$SDC_COVERAGE_REPLIES" --argjson a "$(jq '.attempt // 1' "$1")" --argjson n "$(jq '.coverage_attempt // 1' "$1")" '
      ($r.rounds // [] | .[$a - 1] // last) | if type == "array" then (.[$n - 1] // last) else . end' > "$cwd/out/coverage.json" || exit 1
    rm -f "$cwd/out/impl/.sdc/coverage.json"
  elif [[ -f $cwd/out/impl/.sdc/coverage.json ]]; then mv "$cwd/out/impl/.sdc/coverage.json" "$cwd/out/coverage.json"
  else "${SDC_REPLY:-$here/../../../target/debug/sdc-reply}" coverage-stub --work "$w" --tree "$cwd/out/impl" > "$cwd/out/coverage.json" || exit 1; fi
fi
jq -n --arg node "$node" --arg model "$model" '{adapter: "fixture", model: $model, exit: 0, duration_ms: 0, cost_usd: 0,
  num_turns: 0, usage: null, tool_calls: [], offered_tools: [], node: $node}'
