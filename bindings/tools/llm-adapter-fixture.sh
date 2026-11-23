#!/usr/bin/env bash
# LLM adapter for tests, no LLM (contract: llm-adapter-metaharness.sh). It writes the node's stub
# output into the call's out/, so sdc-node applies it exactly as it applies a real reply: the
# apply step (for example questions.answer's refusal to take a changed scope-changing question)
# runs, which the stub adapter skips. Only for nodes whose stub is `stub-file <fixture> <artifact>`
# or `stub-tree <fixture dir> <tree>` (spec.author); use it with SDC_LLM_NODES naming those nodes. SDC_STUB_FIXTURES applies as for the stub.
# SDC_FIXTURE_STUB (JSON {node: [fixture, artifact]}) gives a node such a stub when its binding's
# stub is another subcommand (plan.decompose: fixtures/plan-stories.json as stories.json); three
# entries name the subcommand too (test.reproduce: ["stub-tree", "bugfix-pager/stub-repro", "repro"]).
#
# The decider's calls (node `decide.<action>`, the decider of `runbook run`) get a scripted reply in
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
# change.implement in request mode (SDC_IMPLEMENT_MODE=request) gets a reply in the record's
# structured-output.json: SDC_IMPLEMENT_REPLIES, else the stub tree packed (see below).
# change.implement's coverage reply (SDC_IMPLEMENT_COVERAGE=on) goes to out/coverage.json: SDC_COVERAGE_REPLIES, `{"rounds":
# [...]}` taken by the request's `attempt` (the node's n-th call record, the last round after
# that), each round a reply or a list by `coverage_attempt`; else the tree fixture's
# .sdc/coverage.json; else the stub document (sdc-reply coverage-stub).
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
# The requester agent's calls (node `requester.<question id or action>`, the requester of `runbook run`) get a
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
  # The request names no case directory under Loom (it gives none): sdc-reply takes the records of
  # the workspace, which is the directory this adapter runs in (where Loom started the node);
  # sdc-reply reads it from there under LOOM_CASE and does not read the directory it is given.
  review_case_dir=$(jq -r .case_dir "$1")
  [[ -n $review_case_dir || -z ${LOOM_CASE:-} ]] || review_case_dir=$PWD
  p=$("${SDC_REPLY:-$here/../../driver/target/debug/sdc-reply}" rejections --case-dir "$review_case_dir" --action change.review | jq length)
  jq -n --argjson r "${SDC_REVIEW_REPLIES:-"{}"}" --argjson p "${p:-0}" --argjson n "$(jq '.review_attempt // 1' "$1")" '
    ($r.rounds // [] | .[$p] // last) | if type == "array" then (.[$n - 1] // last) else . end
    // {decision: "approved", summary: "fixture review: approved", findings: []}' > "$cwd/out/review.json" || exit 1
  jq -n --arg node "$node" --arg model "$model" '{adapter: "fixture", model: $model, exit: 0, duration_ms: 0,
    cost_usd: 0, num_turns: 0, usage: null, tool_calls: [], offered_tools: [], node: $node}'
  exit 0
fi
cfg=${SDC_LLM_NODE:-'{}'}
# change.implement in request mode (request `implement_mode` request): the reply goes to
# <record_dir>/structured-output.json, where claude-cli writes a structured reply.
# SDC_IMPLEMENT_REPLIES (JSON {"rounds": [...]}): the round of the request's `attempt` (the n-th
# change.implement call), the last round after that; a round is a reply or a list by
# `reply_attempt` (the k-th request of the call). "stub", and no scripted reply, is the node's stub
# tree (stub-tree, SDC_STUB_FIXTURES as for the stub) packed as a reply by sdc-reply implement-pack.
if [[ $node == change.implement && $(jq -r '.implement_mode // ""' "$1") == request ]]; then
  rec=$(jq -r .record_dir "$1")
  mkdir -p "$rec"
  r=$(jq -cn --argjson r "${SDC_IMPLEMENT_REPLIES:-"{}"}" --argjson n "$(jq '.attempt // 1' "$1")" --argjson k "$(jq '.reply_attempt // 1' "$1")" '
    ($r.rounds // [] | .[$n - 1] // last) | if type == "array" then (.[$k - 1] // last) else . end // "stub"') || exit 1
  if [[ $r == '"stub"' ]]; then
    mapfile -t a < <(jq -r '.stub[]?' <<< "$cfg")
    w=$PWD
    mkdir -p "$w/.sdc"
    (cd "$rec" && SDC_STUB_COUNTER="$w/.sdc" "$here/sdc-tool.sh" "${a[0]}" "${a[1]}" stub-tree) || exit 1
    "${SDC_REPLY:-$here/../../driver/target/debug/sdc-reply}" implement-pack --tree "$rec/stub-tree" --note "fixture: the stub tree" \
      > "$rec/structured-output.json" || exit 1
  else
    printf '%s\n' "$r" > "$rec/structured-output.json"
  fi
  jq -n --arg node "$node" --arg model "$model" '{adapter: "fixture", model: $model, exit: 0, duration_ms: 0, cost_usd: 0,
    num_turns: 1, usage: null, tool_calls: [], offered_tools: [], schema_constrained: true, node: $node}'
  exit 0
fi
mapfile -t a < <(jq -r '.stub[]?' <<< "$cfg")
o=$(jq -c --arg n "$node" '.[$n] // empty' <<< "${SDC_FIXTURE_STUB:-null}")
[[ -n $o ]] && mapfile -t a < <(jq -r 'if length == 3 then .[] else ("stub-file", .[]) end' <<< "$o")
[[ ${a[0]:-} == stub-file || ${a[0]:-} == stub-tree ]] && [[ -n ${a[2]:-} ]] ||
  { echo "llm-adapter-fixture: node $node has no stub-file or stub-tree stub" >&2; exit 64; }
mkdir -p "$cwd/out"
# Run in the call's directory; a stub-tree's call count stays in work/.sdc (sdc-node runs this in
# work/), so <name>.<n> of SDC_STUB_FIXTURES counts the stub's and this adapter's calls together.
w=$PWD
mkdir -p "$w/.sdc"
(cd "$cwd" && SDC_STUB_COUNTER="$w/.sdc" "$here/sdc-tool.sh" "${a[0]}" "${a[1]}" "out/${a[2]}") || exit 1
# change.implement's coverage reply: the tree fixture's own .sdc/coverage.json (moved out of the
# tree, as a model writes it), else the stub document (sdc-reply coverage-stub). Only with
# SDC_IMPLEMENT_COVERAGE=on; off, the default, the implementer writes its tree alone, as on main.
if [[ $node == change.implement && ${SDC_IMPLEMENT_COVERAGE:-off} == on ]]; then
  rm -f "$cwd/out/coverage.json"
  if [[ -n ${SDC_COVERAGE_REPLIES:-} ]]; then
    jq -n --argjson r "$SDC_COVERAGE_REPLIES" --argjson a "$(jq '.attempt // 1' "$1")" --argjson n "$(jq '.coverage_attempt // 1' "$1")" '
      ($r.rounds // [] | .[$a - 1] // last) | if type == "array" then (.[$n - 1] // last) else . end' > "$cwd/out/coverage.json" || exit 1
    rm -f "$cwd/out/impl/.sdc/coverage.json"
  elif [[ -f $cwd/out/impl/.sdc/coverage.json ]]; then mv "$cwd/out/impl/.sdc/coverage.json" "$cwd/out/coverage.json"
  else "${SDC_REPLY:-$here/../../driver/target/debug/sdc-reply}" coverage-stub --work "$w" --tree "$cwd/out/impl" > "$cwd/out/coverage.json" || exit 1; fi
fi
jq -n --arg node "$node" --arg model "$model" '{adapter: "fixture", model: $model, exit: 0, duration_ms: 0, cost_usd: 0,
  num_turns: 0, usage: null, tool_calls: [], offered_tools: [], node: $node}'
