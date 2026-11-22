#!/usr/bin/env bash
# The agent-node executor: one LLM call for one SDC writer action, bound as an argv list
# (`["{dir}/../../../bindings/tools/llm-node.sh", "<action>"]` in driver/examples/sdc/*.bindings.json,
# `[tools/llm-node.sh, <action>, ...]` in bindings/*.yaml). Run with cwd = the case tree's work/.
#
# It builds the prompt from the node's declared inputs only, copies those inputs into a scratch
# directory (in/), lets the LLM write there (out/), and then copies back ONLY the node's declared
# output artifact (for the AEP nodes: a deterministic `aep plan artifact` call made from the LLM's
# proposal). The LLM never writes work/ and never writes evidence: sdc-drive observes the new
# revision, and only the programs bound to the check actions produce evidence.
#
# Environment:
#   SDC_LLM_NODE     the binding's `llm` object (set by sdc-drive): model, max_turns,
#                    max_budget_usd, operations, stub
#   SDC_LLM_ADAPTER  stub (default: the binding's `stub` subcommand of sdc-tool.sh, no LLM) |
#                    dry (print the metaharness command, then run the stub) | metaharness |
#                    claude-cli | a path to an adapter program (contract: llm-adapter-metaharness.sh)
#   SDC_LLM_NODES    optional, space-separated node names: only these go to SDC_LLM_ADAPTER, every
#                    other node runs its stub (one node through the apply path, for example with
#                    tools/llm-adapter-fixture.sh)
#   SDC_LLM_MODEL    overrides the binding's model
#   SDC_LLM_MODEL_<CLASS> overrides both for the node's class (tools/node-classes.json): WRITER
#                    (spec.author, plan.decompose), IMPLEMENT (change.implement), REVIEW, OTHER
#   SDC_LLM_EFFORT, SDC_LLM_EFFORT_<CLASS>  the reasoning effort (low, medium, high, xhigh, max), the same way;
#                    unset: the class's default_efforts in tools/node-classes.json, always passed (request.json `effort`)
#   SDC_LLM_RECORD   where each call's record goes (default work/.sdc/llm); one NNN-<action>/ per call
#   SDC_LLM_LOG      JSON lines, one per call (default $SDC_LLM_RECORD/calls.jsonl)
#   SDC_RUN_SETTINGS the run's run-settings.jsonl (scripts/run-trial.sh): the rework prompt names its
#                    recorded rework cap (`rework_limit.value`, D184); unset: rule-ends.json's rework_limit
#   SDC_FEEDBACK     set by sdc-drive on a retry: the failing check records and their logs
#
# Loom mode (LOOM_CASE set; docs/loom-bindings.md, "Tool changes for Loom's program contract"):
# Loom starts a bound program with PATH and LOOM_CASE alone, the arguments as one JSON object on
# standard input, and on a retry LOOM_RETRY_FEEDBACK naming a loom.retry-feedback/1 document. The
# variables above come from those and from argv entries the binding states after the action:
#   --model <m> --max-turns <n> --max-budget-usd <usd> --operations <op,op,...>  the `llm` object
#   --instructions <json>  the `llm` object's instructions: {"task": "<file>", ...}
#   --adapter <a>  SDC_LLM_ADAPTER        --principal <p>  SDC_PRINCIPAL
#   --stub-fixtures <dir>  SDC_STUB_FIXTURES    --stub-counter <dir>  SDC_STUB_COUNTER
#   a workspace directory .stub (seeded by a suite cycle): SDC_STUB_FIXTURES, and its finding.yaml
#   and finding-2.yaml as SDC_STUB_FINDINGS, when the options and the variables above are not set;
#   .stub/settings.yaml (adapter, nodes, review_mode, ...) and .stub/replies/<name>.yaml (review,
#   implement, coverage, fixture-stub): the scripted model of the old SDC_* variables
#   --claude-credentials <file>  SDC_CLAUDE_CREDENTIALS, the login of the claude-cli and of the metaharness
#                    adapter (each copies it into a private directory for its call: a config home, a
#                    HOME; without it they read CLAUDE_CONFIG_DIR or HOME, which Loom does not give)
#   --stub <argv>...  the `llm` object's stub; last, it takes the rest of the argv
# The case id is SDC_CASE, the arguments SDC_ARGS, the feedback document's records SDC_FEEDBACK
# (each with its log's tail; Loom names no action and no exit of the run that recorded it).
# sdc-findings, sdc-bugfix and sdc-reply come from PATH, else from the driver crate's build in
# this repository, never from HOME. Loom gives no case directory: what reads SDC_CASE_DIR takes
# its driver-mode default or is pending (bindings/PENDING.md).
set -uo pipefail
here=$(cd "$(dirname "$0")" && pwd)
root=$(cd "$here/../.." && pwd)
fx=$root/driver/examples/sdc/fixtures
node=$1
# A binary of the driver crate (driver/src/bin) in Loom mode: on PATH, else its debug build.
sibling() { command -v "$1" 2>/dev/null || echo "$root/driver/target/debug/$1"; }
if [[ -n ${LOOM_CASE:-} ]]; then
  SDC_ARGS=$(cat)
  exec < /dev/null
  [[ -n ${SDC_ARGS//[[:space:]]/} ]] || SDC_ARGS='{}'
  jq -e 'type == "object"' <<< "$SDC_ARGS" > /dev/null 2>&1 ||
    { echo "llm-node: $node could not run: its standard input is not one JSON object" >&2; exit 64; }
  shift
  declare -A opt=()
  with_stub=""
  while (( $# )); do
    case $1 in
      --stub) shift; with_stub=yes; break ;;
      --model|--max-turns|--max-budget-usd|--operations|--instructions|--adapter|--principal|--stub-fixtures|--stub-counter|--claude-credentials)
        (( $# >= 2 )) || { echo "llm-node: $1 takes a value" >&2; exit 64; }
        opt[$1]=$2; shift 2 ;;
      *) echo "llm-node: unknown argument $1" >&2; exit 64 ;;
    esac
  done
  SDC_LLM_NODE=$(jq -cn --arg model "${opt[--model]:-}" --arg mt "${opt[--max-turns]:-}" --arg usd "${opt[--max-budget-usd]:-}" \
    --arg ops "${opt[--operations]:-}" --arg ins "${opt[--instructions]:-}" --arg stub "$with_stub" '{}
    + (if $model == "" then {} else {model: $model} end)
    + (if $mt == "" then {} else {max_turns: ($mt | tonumber)} end)
    + (if $usd == "" then {} else {max_budget_usd: ($usd | tonumber)} end)
    + (if $ops == "" then {} else {operations: ($ops | split(","))} end)
    + (if $ins == "" then {} else {instructions: ($ins | fromjson | if type == "object" then . else error("not an object") end)} end)
    + (if $stub == "" then {} else {stub: $ARGS.positional} end)' --args "$@") ||
    { echo "llm-node: $node could not run: $(for _o in --max-turns --max-budget-usd; do
        [[ -z ${opt[$_o]:-} ]] || jq -en --arg v "${opt[$_o]}" '$v | tonumber' > /dev/null 2>&1 || { echo "$_o takes a number, got ${opt[$_o]}"; break; }; done)$(
        [[ -z ${opt[--instructions]:-} ]] || jq -e 'type == "object"' <<< "${opt[--instructions]}" > /dev/null 2>&1 ||
        echo "--instructions takes a JSON object, got ${opt[--instructions]}")" >&2; exit 64; }
  SDC_REPLY=$(sibling sdc-reply)
  export SDC_LLM_NODE SDC_REPLY SDC_CASE=$LOOM_CASE SDC_ACTION=$node
  [[ -n ${opt[--adapter]:-} ]] && export SDC_LLM_ADAPTER=${opt[--adapter]}
  [[ -n ${opt[--principal]:-} ]] && export SDC_PRINCIPAL=${opt[--principal]}
  [[ -n ${opt[--stub-fixtures]:-} ]] && export SDC_STUB_FIXTURES=${opt[--stub-fixtures]}
  [[ -n ${opt[--stub-counter]:-} ]] && export SDC_STUB_COUNTER=${opt[--stub-counter]}
  # A suite cycle's seeds (cycles/tree-suite.yaml): the workspace directory .stub is the stub's
  # fixture directory, and .stub/finding.yaml, .stub/finding-2.yaml are the findings the stub
  # raises (the old SDC_STUB_FIXTURES, SDC_STUB_FINDINGS). Absent, the tool runs as before.
  if [[ -d $PWD/.stub ]]; then
    [[ -n ${SDC_STUB_FIXTURES:-} ]] || export SDC_STUB_FIXTURES=$PWD/.stub
    [[ -n ${SDC_STUB_FINDINGS:-} ]] || export SDC_STUB_FINDINGS=$PWD/.stub/finding.yaml:$PWD/.stub/finding-2.yaml
    # The scripted model settings and replies (docs/cycles-suite.md, "The layout of .stub/"):
    # .stub/settings.yaml keys and .stub/replies/<name>.yaml into the variables of the old driver.
    # `fixture` as an adapter is the fixture adapter beside this tool. A variable already set wins.
    stub_set() { [[ -n ${!1:-} ]] || export "$1=$2"; }
    if [[ -f $PWD/.stub/settings.yaml ]]; then
      stub_json=$(yq -o=json -I=0 '.' "$PWD/.stub/settings.yaml") || { echo "llm-node: $node could not run: .stub/settings.yaml is not YAML" >&2; exit 64; }
      while IFS=$'\t' read -r k v; do
        case $k in
          adapter) stub_set SDC_LLM_ADAPTER "$([[ $v == fixture ]] && echo "$here/llm-adapter-fixture.sh" || echo "$v")" ;;
          review_adapter) stub_set SDC_REVIEW_ADAPTER "$([[ $v == fixture ]] && echo "$here/llm-adapter-fixture.sh" || echo "$v")" ;;
          nodes) stub_set SDC_LLM_NODES "$v" ;;
          review_mode) stub_set SDC_REVIEW_MODE "$v" ;;
          implement_mode) stub_set SDC_IMPLEMENT_MODE "$v" ;;
          implement_coverage) stub_set SDC_IMPLEMENT_COVERAGE "$v" ;;
          *) echo "llm-node: $node could not run: .stub/settings.yaml has no setting $k" >&2; exit 64 ;;
        esac
      done < <(jq -r 'to_entries[] | [.key, (.value | [.] | flatten | map(tostring) | join(" "))] | @tsv' <<< "$stub_json")
    fi
    for stub_f in "$PWD"/.stub/replies/*.yaml; do
      [[ -f $stub_f ]] || continue
      stub_name=$(basename "$stub_f" .yaml)
      stub_json=$(yq -o=json -I=0 '.' "$stub_f") || { echo "llm-node: $node could not run: .stub/replies/$stub_name.yaml is not YAML" >&2; exit 64; }
      case $stub_name in
        review) stub_set SDC_REVIEW_REPLIES "$stub_json" ;;
        implement) stub_set SDC_IMPLEMENT_REPLIES "$stub_json" ;;
        coverage) stub_set SDC_COVERAGE_REPLIES "$stub_json" ;;
        fixture-stub) stub_set SDC_FIXTURE_STUB "$stub_json" ;;
        *) echo "llm-node: $node could not run: .stub/replies/$stub_name.yaml is no reply list" >&2; exit 64 ;;
      esac
    done
  fi
  [[ -n ${opt[--claude-credentials]:-} ]] && export SDC_CLAUDE_CREDENTIALS=${opt[--claude-credentials]}
  # The retry records as the driver's SDC_FEEDBACK: kind, result, subject and the log, here the
  # tail Loom keeps of the cited blob, written beside the document's copy under .sdc/loom-feedback/.
  if [[ -n ${LOOM_RETRY_FEEDBACK:-} ]]; then
    jq -e '.format == "loom.retry-feedback/1"' "$LOOM_RETRY_FEEDBACK" > /dev/null 2>&1 ||
      { echo "llm-node: $node could not run: LOOM_RETRY_FEEDBACK names no loom.retry-feedback/1 document" >&2; exit 126; }
    fbd=$PWD/.sdc/loom-feedback/$(sha256sum < "$LOOM_RETRY_FEEDBACK" | cut -c1-16)
    mkdir -p "$fbd"
    jq -r '.logs[].blob_sha256 | select(type == "string" and test("^[0-9a-f]{64}$"))' "$LOOM_RETRY_FEEDBACK" | while read -r b; do
      jq -j --arg b "$b" '[.logs[] | select(.blob_sha256 == $b)][0].tail' "$LOOM_RETRY_FEEDBACK" > "$fbd/$b"
    done
    jq --arg d "$fbd" '[.logs[].blob_sha256 | select(type == "string" and test("^[0-9a-f]{64}$"))] as $logged | [.records[] | {kind, result, subject, action: (.action // null), exit: (.exit // null),
      log: (.blob_sha256 as $b | if $b != null and ($logged | index($b)) != null then "\($d)/\($b)" else null end)}]' \
      "$LOOM_RETRY_FEEDBACK" > "$fbd/feedback.json" && SDC_FEEDBACK=$fbd/feedback.json
  fi
fi
# A run that does nothing ends here: exit 0, and sdc-drive is told (SDC_NOT_CALLED, a file it
# reads back) so the run is recorded `not_called` and not counted against max_runs.
not_called() { echo "$1"; [[ -n ${SDC_NOT_CALLED:-} ]] && printf '%s\n' "$1" > "$SDC_NOT_CALLED"; exit 0; }
# A missing program is a program error (exit 127: sdc-drive stops, records nothing), not a result.
for p in jq yq; do
  command -v "$p" >/dev/null 2>&1 || { echo "llm-node: $p is not on PATH: $node could not run" >&2; exit 127; }
done
# The case's aep store keeps aep's own lifecycles: without --root, aep takes the profile of any
# project above the case directory (runs/ inside this repository: development.standard, where a
# draft may only move to proposed), and plan.decompose's moves to active are refused.
aep() { command aep "$@" --root .; }
# aep records who moved an artifact (AEP_ACTOR, else human:$USER): the acting principal, the run's
# SDC_PRINCIPAL when it is a <kind>:<name> principal and no person (no person takes a tool step;
# dry-run is none), else agent:<node>; a move's --executor names the node that ran it.
aep_actor=${SDC_PRINCIPAL:-}
[[ $aep_actor =~ ^[a-z][a-z-]*:[^[:space:]]+$ && $aep_actor != human:* && $aep_actor != person:* ]] || aep_actor=agent:$node
export AEP_ACTOR=$aep_actor

# questions.write writes the interview; it never rewrites one that exists unless a check found it
# invalid (a retry, SDC_FEEDBACK set). An interview whose shape is only not checked yet (after an
# answer, an edit or an amendment) holds answers: in runs/trial-2-train the rewrite after
# amend.upstream wiped the operator's answers and the finding decision.
if [[ $node == questions.write && -s questions.yaml && -z ${SDC_FEEDBACK:-} ]]; then
  not_called "llm-node: questions.write not called: questions.yaml exists and no check found it invalid"
fi
cfg=${SDC_LLM_NODE:-'{}'}
adapter=${SDC_LLM_ADAPTER:-stub}
# The review's reply is a data document made under a response schema, so its class has its own
# adapter (sdc-reply adapters: SDC_REVIEW_ADAPTER, else claude-cli where the writers run on
# metaharness): the model replies through the schema channel, never by writing a file.
if [[ $node == change.review ]]; then
  reply_bin=${SDC_REPLY:-$root/driver/target/debug/sdc-reply}
  [[ -x $reply_bin ]] || { echo "llm-node: sdc-reply not found ($reply_bin): $node could not run" >&2; exit 127; }
  res=$("$reply_bin" adapters --tools "$here" 2>&1) && adapter=$(jq -r '.review.adapter // empty' <<< "$res") && [[ -n $adapter ]] ||
    { echo "llm-node: $node could not run: $(grep -v '^{' <<< "$res")" >&2; exit 126; }
fi
# change.implement as one request (SDC_IMPLEMENT_MODE=request; docs/driver.md "Implement as one
# request"): no tool, the input assembled and the reply checked and written by sdc-reply
# implement-run, on the implement class's schema adapter (sdc-reply adapters: SDC_IMPLEMENT_ADAPTER,
# else as the review's). session, the default, is the session below.
implement_mode=${SDC_IMPLEMENT_MODE:-session}
[[ $implement_mode == @(session|request) ]] ||
  { echo "llm-node: $node could not run: SDC_IMPLEMENT_MODE=$implement_mode is neither session nor request" >&2; exit 126; }
[[ $node == change.implement ]] || implement_mode=session
# change.review on one document of facts with no tools (SDC_REVIEW_MODE=facts,
# story:review-minimal-facts; docs/driver.md "The code review"). session, the default, is the
# session with read tools over the whole change tree.
review_mode=${SDC_REVIEW_MODE:-session}
[[ $review_mode == @(session|facts) ]] ||
  { echo "llm-node: $node could not run: SDC_REVIEW_MODE=$review_mode is neither session nor facts" >&2; exit 126; }
[[ $node == change.review ]] || review_mode=session
if [[ $implement_mode == request ]]; then
  reply_bin=${SDC_REPLY:-$root/driver/target/debug/sdc-reply}
  [[ -x $reply_bin ]] || { echo "llm-node: sdc-reply not found ($reply_bin): $node could not run" >&2; exit 127; }
  res=$("$reply_bin" adapters --tools "$here" 2>&1) && adapter=$(jq -r '.implement.adapter // empty' <<< "$res") && [[ -n $adapter ]] ||
    { echo "llm-node: $node could not run: $(grep -v '^{' <<< "$res")" >&2; exit 126; }
fi
[[ -n ${SDC_LLM_NODES:-} && " $SDC_LLM_NODES " != *" $node "* ]] && adapter=stub
work=$PWD
# change.implement replies, besides its change, with a coverage document (sdc-reply coverage):
# per requirement of the story, built with the lines and a test, or not_built with a reason. It
# goes to impl/.sdc/coverage.json; the binding's postcondition (sdc-reply check-coverage) checks it.
# Only with SDC_IMPLEMENT_COVERAGE=on and the session implementer (docs/improving.md, "Run
# switches"); off, the default, change.implement is main's.
coverage=${SDC_IMPLEMENT_COVERAGE:-off}
[[ $coverage == @(on|off) ]] ||
  { echo "llm-node: $node could not run: SDC_IMPLEMENT_COVERAGE=$coverage is neither on nor off" >&2; exit 126; }
[[ $node == change.implement && $implement_mode == session ]] || coverage=off
if [[ $coverage == on ]]; then
  reply_bin=${SDC_REPLY:-$root/driver/target/debug/sdc-reply}
  [[ -x $reply_bin ]] || { echo "llm-node: sdc-reply not found ($reply_bin): $node could not run" >&2; exit 127; }
fi
# The stub implementer's coverage document, unless the stub tree carries one (a fixture's own).
coverage_stub() {
  [[ $coverage == on && -d $work/impl && ! -f $work/impl/.sdc/coverage.json ]] || return 0
  mkdir -p "$work/impl/.sdc" && "$reply_bin" coverage-stub --work "$work" --tree "$work/impl" > "$work/impl/.sdc/coverage.json"
}

run_stub() {
  local a
  mapfile -t a < <(jq -r '.stub[]' <<< "$cfg")
  "$here/sdc-tool.sh" "${a[@]}"
}

# --- the late-finding loop (docs/trial-2.md, "The late-finding loop") ---------------------------
# The stage agents that may raise a finding, and the tree each one derives from the requirements.
finding_nodes=" requirements.normalise context.default vision.write roadmap.write spec.author plan.decompose change.implement "
declare -A derived=([spec.author]=spec [change.implement]=impl [plan.decompose]=aep)
raises=no
[[ $finding_nodes == *" $node "* ]] && raises=yes

# While a finding waits for a person's decision, a stage agent is not called: whatever it wrote
# would be re-derived once the decision amends the requirements.
if [[ $raises == yes && -f $work/findings.yaml ]] &&
  yq -e '[.findings[] | select(.status == "needs_person" and (.decision // "") == "")] | length > 0' \
    "$work/findings.yaml" >/dev/null 2>&1; then
  not_called "llm-node: $node not called: findings wait for a person ($(yq -r '[.findings[] | select(.status == "needs_person")] | map(.id) | join(" ")' "$work/findings.yaml"))"
fi

# After the node's output is applied: its raised findings go through the program check into
# findings.yaml (a stage agent's finding counts only once that check accepts it), and a derived
# tree records the requirements revision it was derived from (`.derived-from`), so a re-derivation
# after an amendment is always a new revision.
post() {
  local raised=$1
  [[ $raises == yes ]] || return 0
  # SDC_STUB_FINDINGS: one fixture, or several separated by ':' (one per node); the one whose
  # `node` is this node raises.
  local sf="" f
  if [[ -z $raised && -n ${SDC_STUB_FINDINGS:-} ]]; then
    for f in ${SDC_STUB_FINDINGS//:/ }; do
      [[ -f $f && $(yq -r '.node' "$f") == "$node" ]] && { sf=$f; break; }
    done
  fi
  if [[ -n $sf ]]; then
    raised=$sf
    # A fixture with `rounds:` raises rounds[n] on the node's n-th applied run (0-based), then none.
    if yq -e 'has("rounds")' "$sf" >/dev/null 2>&1; then
      mkdir -p "$work/.sdc"
      local c=$work/.sdc/stub-round.$node n
      n=$(cat "$c" 2>/dev/null || echo 0); echo $((n + 1)) > "$c"
      raised=
      if (( n < $(yq '.rounds | length' "$sf") )); then
        raised=$work/.sdc/stub-findings.$node.yaml
        N=$n yq '{"node": .node, "findings": .rounds[env(N)]}' "$sf" > "$raised"
      fi
    fi
  fi
  if [[ -n $raised && -f $raised ]]; then
    local b looked="SDC_FINDINGS, then $root/driver/target/debug/sdc-findings"
    if [[ -n ${LOOM_CASE:-} ]]; then
      b=$(sibling sdc-findings)
      looked="PATH, then $root/driver/target/debug/sdc-findings"
    else
      b=${SDC_FINDINGS:-$root/driver/target/debug/sdc-findings}
    fi
    [[ -x $b ]] || { echo "llm-node: sdc-findings not found (looked at $looked): $node could not run" >&2; exit 127; }
    "$b" --rules "$here/consistency-rules.json" promote "$node" "$raised"
  fi
  local t=${derived[$node]:-}
  if [[ -n $t && -d $work/$t ]]; then
    echo "requirements $(sha256sum < "$work/requirements.yaml" | cut -c1-16) resolved $(yq -r '[.findings[]? | select(.status == "resolved") | .id] | join(" ")' "$work/findings.yaml" 2>/dev/null)" \
      > "$work/$t/.derived-from"
  fi
  return 0
}

# A rejected approval of this writer's output (`approve --result rejected`): SDC_FEEDBACK names
# the record, and its log is the rejection (sdc.rejection/1: approval, revision, reasons,
# citations). The writer gets it as in/rejection.json and addresses each reason.
rejection=""
if [[ -n ${SDC_FEEDBACK:-} && -f $SDC_FEEDBACK ]]; then
  rejection=$(jq -r '[.[] | select(.result == "rejected") | .log // empty] | last // empty' "$SDC_FEEDBACK")
  [[ -n $rejection && -f $rejection ]] && jq -e '.format == "sdc.rejection/1"' "$rejection" > /dev/null 2>&1 || rejection=""
fi
# A rejection of this writer's subject still pending though SDC_FEEDBACK does not carry it (a later
# write by another action moved the subject's revision, so the rejected record is no longer
# current): the case's last rejected approval with no run of this writer after it that exited 0
# and was not `not_called`. It is addressed as if fed back, and the skip rule below does not apply.
# Only a node of class writer or implement (the classes that write the rejected artifact) looks it
# up: a review node never gets a rejection as rework, its own or an approval's of the same subject.
# The node's class (tools/node-classes.json); it also picks SDC_LLM_MODEL_<CLASS> below.
class=$(jq -r --arg n "$node" '[.classes | to_entries[] | select(.value | index($n)) | .key] | first // "other"' "$here/node-classes.json")
cd_=${SDC_CASE_DIR:-}
if [[ ( $class == writer || $class == implement ) && -z $rejection && -n $cd_ && -f $cd_/events.jsonl ]]; then
  rejection=$(jq -rs --arg n "$node" '[.[] | select(.event == "action_finished")] as $f
    | ([$f | to_entries[] | select(.value.result == "rejected") | .key] | last) as $k
    | if $k == null or any($f[$k + 1:][]; .action == $n and .exit == "0" and (.not_called | type) != "string") then empty
      else $f[$k].log // empty end' "$cd_/events.jsonl" 2>/dev/null)
  [[ -n $rejection ]] && rejection=$cd_/blobs/$rejection
  [[ -n $rejection && -f $rejection ]] && jq -e --arg s "$(jq -r --arg n "$node" '.[$n].subject // ""' "$cd_/bindings.json" 2>/dev/null)" \
    '.format == "sdc.rejection/1" and .subject == $s' "$rejection" > /dev/null 2>&1 || rejection=""
fi

if [[ $adapter == stub ]]; then
  run_stub; code=$?
  [[ $code == 0 ]] && { coverage_stub || code=126; }
  [[ $code == 0 ]] && post ""
  # The stub addresses a rejection by recording it in the rejected artifact (the approval's
  # subject, at its `$artifacts` path), so the rework is a new revision: a tree gets `.rework`,
  # a YAML or Markdown file the same lines as a comment.
  t="" lines="" A="" S=""
  if [[ -n ${LOOM_CASE:-} && -z $rejection && -n ${SDC_FEEDBACK:-} && -f $SDC_FEEDBACK ]]; then
    # Loom mode, a person's rejection: the feedback record names the person's action and the
    # subject, and its log is the person's reasons as text.
    A=$(jq -r '[.[] | select(.result == "rejected" and .action != null and .log != null)] | last.action // ""' "$SDC_FEEDBACK")
    S=$(jq -r '[.[] | select(.result == "rejected" and .action != null and .log != null)] | last.subject // ""' "$SDC_FEEDBACK")
    fb_log=$(jq -r '[.[] | select(.result == "rejected" and .action != null and .log != null)] | last.log // ""' "$SDC_FEEDBACK")
    [[ -n $A && -f $fb_log ]] && lines=$(printf 'rework of the rejection of %s:\n%s\n' "$A" "$(sed 's/^/- /' "$fb_log")")
  elif [[ -n $rejection && -n ${LOOM_CASE:-} ]]; then
    A=$(jq -r '.approval // ""' "$rejection") S=$(jq -r '.subject // ""' "$rejection")
    lines=$(jq -r '"rework \(.count) of the rejection of \(.approval) at \(.revision):", (.reasons[] | "- " + .)' "$rejection")
  fi
  if [[ -n ${LOOM_CASE:-} && -n $A && -n $S ]]; then
    # Loom mode: the subject's path in the bindings file beside these tools that binds the approval.
    t=$(for y in "$here"/../*.yaml; do A=$A S=$S yq -r 'select(.actions | any_c(.action == strenv(A))) | .artifacts[strenv(S)] // ""' "$y"; done 2>/dev/null |
      grep -v -e '^$' -e '^---$' | sort -u)
    [[ $(grep -c . <<< "$t") == 1 ]] || t=""
  elif [[ -n $rejection ]]; then
    t=$(jq -r --arg s "$(jq -r '.subject // ""' "$rejection")" '."$artifacts"[$s] // ""' "${SDC_CASE_DIR:-$work/..}/bindings.json" 2>/dev/null)
    lines=$(jq -r '"rework \(.count) of the rejection of \(.approval) at \(.revision):", (.reasons[] | "- " + .)' "$rejection")
  fi
  if [[ $code == 0 && -n $t && -n $lines && -e $work/$t ]]; then
    case $t in
      *.yaml|*.yml) [[ -d $work/$t ]] || sed 's/^/# /' <<< "$lines" >> "$work/$t" ;;
      *.md) printf '\n<!--\n%s\n-->\n' "$lines" >> "$work/$t" ;;
    esac
    [[ -d $work/$t ]] && printf '%s\n' "$lines" > "$work/$t/.rework"
  fi
  exit $code
fi

# A derived tree is not re-derived by the LLM when every decision taken since this node last wrote
# it is the default of a finding this node raised: the prompt has the node follow each default it
# raises (and each open finding's), so the tree already follows those decisions. Only the
# requirements digest in `.derived-from` is renewed (a new revision, so the checks run again). In
# runs/trial-3-haiku-2 every re-derivation raised one new agent finding (f14, f16, f17) until
# spec.author was spent. Not on a retry (a check sent the tree back) and not for the stubs, which
# follow nothing (they model a writer that never converges, cycle story-spent).
t=${derived[$node]:-}
if [[ $raises == yes && -n $t && -z ${SDC_FEEDBACK:-} && -z $rejection && -f $work/$t/.derived-from && -f $work/findings.yaml ]]; then
  first=$(head -n 1 "$work/$t/.derived-from")
  if [[ $first == "requirements "*" resolved"* ]]; then
    was=" ${first#* resolved} "
    new=$(yq -r '.findings[]? | select(.status == "resolved") | .id' "$work/findings.yaml" | while read -r id; do
      [[ $was == *" $id "* ]] || echo "$id"; done | tr '\n' ' ')
    own=$(N="llm:$node" yq -r '[.findings[]? | select(.status == "resolved" and .raised_by == strenv(N)
      and .resolution.default_taken == true) | .id] | join(" ")' "$work/findings.yaml")
    if [[ -n ${new// /} ]] && ! for id in $new; do [[ " $own " == *" $id "* ]] || echo x; done | grep -q x; then
      echo "requirements $(sha256sum < "$work/requirements.yaml" | cut -c1-16) resolved $(yq -r '[.findings[]? | select(.status == "resolved") | .id] | join(" ")' "$work/findings.yaml")" \
        > "$work/$t/.derived-from"
      not_called "llm-node: $node not called: the decisions since $t/ was written are the defaults it raised and follows (${new% }); $t/ marked as derived from the amended requirements"
    fi
  fi
fi

case $adapter in
  metaharness|claude-cli|dry) adapter_bin=$here/llm-adapter-$adapter.sh ;;
  *) adapter_bin=$adapter ;;
esac

rec_root=${SDC_LLM_RECORD:-$work/.sdc/llm}
mkdir -p "$rec_root"
n=$(( $(find "$rec_root" -mindepth 1 -maxdepth 1 -type d | wc -l) + 1 ))
rec=$rec_root/$(printf '%03d' "$n")-$node
cwd=$rec/cwd
mkdir -p "$cwd/in" "$cwd/out"
# status.json (sdc.call-status/1): running now, finished or failed on every exit; written atomically.
call_started=$(date -u +%Y-%m-%dT%H:%M:%S.%3NZ)
call_status() { # $1 state, $2 exit (absent while running)
  local t
  t=$(mktemp "$rec/.status.json.XXXXXX") || return 0
  jq -n --arg s "$1" --argjson pid $$ --arg at "$call_started" --arg exit "${2:-}" \
    --arg fin "$(date -u +%Y-%m-%dT%H:%M:%S.%3NZ)" '{format: "sdc.call-status/1", state: $s, pid: $pid, started_at: $at}
    + (if $exit == "" then {} else {finished_at: $fin, exit: ($exit | tonumber)} end)' > "$t" && mv "$t" "$rec/status.json"
}
call_status running
trap 'c=$?; [[ $c == 0 ]] && call_status finished 0 || call_status failed "$c"' EXIT
attempt=$(( $(find "$rec_root" -mindepth 1 -maxdepth 1 -type d -name "*-$node" | wc -l) ))
epic=$(cut -d. -f2 <<< "${SDC_CASE:-}")
story=$(cut -d. -f3 <<< "${SDC_CASE:-}")
# Loom mode: the names Loom gave the cases, not a cut of the case id (a repeated case is IDEA.epic.2).
if [[ -n ${LOOM_CASE:-} ]]; then
  if [[ -n ${LOOM_PARENT_CASE_NAME:-} ]]; then epic=$LOOM_PARENT_CASE_NAME story=${LOOM_CASE_NAME:-$story}
  elif [[ -n ${LOOM_CASE_NAME:-} ]]; then epic=$LOOM_CASE_NAME story=""; fi
fi

# Copy one declared input from work/ into in/ (a missing one is listed as absent).
inputs=()
input() {
  local p=$1
  if [[ -e $work/$p ]]; then
    mkdir -p "$cwd/in/$(dirname "$p")" && cp -r "$work/$p" "$cwd/in/$p"
    inputs+=("in/$p")
  else
    inputs+=("in/$p (absent)")
  fi
}
# An output edited in place starts as a copy of the current artifact.
seed() { [[ -e $work/$1 ]] && mkdir -p "$cwd/out/$(dirname "$1")" && cp -r "$work/$1" "$cwd/out/$1"; true; }
# sdc.bugfix: the case's commands as the programs will run them (sdc-bugfix checks), as in/checks.yaml;
# `ck <yq expression>` reads it. Commands that cannot be read are a program error, not a writer's.
checks_input() {
  "$here/sdc-tool.sh" bugfix checks > "$cwd/in/checks.yaml" ||
    { echo "llm-node: $node could not run: the case's checks.yaml cannot be read (sdc-bugfix checks)" >&2; exit 126; }
  inputs+=("in/checks.yaml")
}
ck() { yq -r "$1" "$cwd/in/checks.yaml"; }

# --- the instructions: files outside this tool ----------------------------------------------------
# The text the model is told is in instructions/ (docs/driver.md, "Instructions"); the binding's
# `llm.instructions` names each file of the action (task, and the parts it has), and this tool reads
# the files and fills their {{name}} placeholders. repo_root is the nearest directory above this tool
# that holds instructions/, derived once here, so it holds wherever the tools directory sits.
repo_root=$here
until [[ -d $repo_root/instructions || $repo_root == / ]]; do repo_root=$(dirname "$repo_root"); done
shared=instructions/shared
# render <variable> <file under the repository root> [name=value ...]: the file with every {{name}}
# replaced by its value in one pass (a value is never searched for placeholders); trailing newlines
# of the file are dropped. A file that is missing, or names a placeholder without a value, is a program error.
render() {
  local -n _o=$1
  local _f=$repo_root/$2 _t _out="" _m _k _kv
  local -A _v=()
  [[ $2 == instructions/*.md && $2 != /* && ! $2 =~ (^|/)\.\.(/|$) ]] ||
    { echo "llm-node: $node could not run: the instruction path $2 is not instructions/<...>.md without a .. segment" >&2; exit 126; }
  shift 2
  [[ -f $_f ]] || { echo "llm-node: $node could not run: the instruction file ${_f#"$repo_root"/} is missing" >&2; exit 126; }
  for _kv; do _v[${_kv%%=*}]=${_kv#*=}; done
  _t=$(<"$_f")
  while [[ $_t =~ \{\{([a-z_]+)\}\} ]]; do
    _m=${BASH_REMATCH[0]} _k=${BASH_REMATCH[1]}
    [[ -v _v[$_k] ]] || { echo "llm-node: $node could not run: ${_f#"$repo_root"/} names {{$_k}} and no value was given" >&2; exit 126; }
    _out+=${_t%%"$_m"*}${_v[$_k]}
    _t=${_t#*"$_m"}
  done
  _o=$_out$_t
}
# render_part <variable> <part> [name=value ...]: the file the binding's llm.instructions names for <part>.
render_part() {
  local _p=$1 _f
  _f=$(jq -r --arg p "$2" '.instructions[$p] // empty' <<< "$cfg")
  [[ -n $_f ]] || { echo "llm-node: $node could not run: the binding's llm object names no instructions.$2" >&2; exit 126; }
  shift 2
  render "$_p" "$_f" "$@"
}
fill_task() { render_part task task "$@"; }
# One input note per line of the file (a rendered file), added to the Inputs list.
notes() { local _n _l; render _n "$@"; while IFS= read -r _l; do inputs+=("$_l"); done <<< "$_n"; }
notes_part() { local _n _l; render_part _n "$@"; while IFS= read -r _l; do inputs+=("$_l"); done <<< "$_n"; }
# A paragraph added to the task.
append_task() { local _a; render _a "$@"; task="$task"$'\n\n'"$_a"; }

ops='["dir.list","file.edit","file.read","file.write","search"]'
mt=30
case $node in
idea.dissect)
  input idea.yaml; out=dissection.yaml
  fill_task ;;
questions.write)
  input idea.yaml; input dissection.yaml; out=questions.yaml
  fill_task ;;
questions.answer)
  input idea.yaml; input dissection.yaml; input questions.yaml; seed questions.yaml; out=questions.yaml
  fill_task ;;
requirements.normalise)
  input idea.yaml; input dissection.yaml; input questions.yaml; out=requirements.yaml
  fill_task ;;
context.default)
  input idea.yaml; input requirements.yaml; out=context.yaml
  fill_task ;;
vision.write)
  input idea.yaml; input requirements.yaml; input context.yaml; out=aep/vision/product.md
  fill_task ;;
roadmap.write)
  input idea.yaml; input requirements.yaml; input context.yaml; input aep/vision/product.md
  out=aep/initiative/roadmap.md
  fill_task ;;
spec.author)
  input idea.yaml; input requirements.yaml; input context.yaml; input spec; seed spec; out=spec
  mkdir -p "$cwd/in/example-spec" && cp -r "$fx/billing-spec/." "$cwd/in/example-spec/"
  notes_part inputs
  system=$(jq -r '.domain // "system"' <<< "${SDC_ARGS:-null}")
  fill_task system="$system" ;;
plan.decompose)
  input requirements.yaml; input spec
  jq -r '.commands | keys[]' "$work/.sdc/gate/ir.json" > "$cwd/in/commands.txt" 2>/dev/null
  notes_part inputs
  out=aep
  fill_task epic="$epic" ;;
change.implement)
  if [[ $implement_mode == request ]]; then
    # Nothing to read and no operation: sdc-reply implement-run assembles the input from the case
    # and writes the reply's files into out/impl/ (a copy of impl/); apply copies it back.
    seed impl; out=impl; ops='[]'; task=""
  else
  input requirements.yaml; input context.yaml; input spec; input "aep/story/$story.md"; input impl; seed impl
  out=impl
  fill_task story="$story"
  # The coverage document (story:implement-cites-requirements, SDC_IMPLEMENT_COVERAGE=on): the
  # story's requirements and the reply schema by program; the reply is checked against it below and
  # asked again at most twice.
  if [[ $coverage == on ]]; then
  "$reply_bin" coverage --work "$work" > "$cwd/in/story-requirements.json" 2> "$rec/coverage-check.err" &&
    "$reply_bin" coverage --work "$work" --schema > "$rec/coverage-schema.json" 2>> "$rec/coverage-check.err" ||
    { echo "llm-node: $node could not run: no story requirements: $(cat "$rec/coverage-check.err")" >&2; exit 126; }
  node_schema=$rec/coverage-schema.json
  notes_part coverage_inputs
  render_part cov coverage schema="$(jq -c . "$rec/coverage-schema.json")"
  task="$task"$'\n\n'"$cov"
  fi
  fi ;;
change.review)
  # The review (story:review-node-is-a-model): nothing is written; the reply is the decision, under
  # a response schema, and a program checks it (sdc-reply check-review, below). Inputs by program.
  reply_bin=${SDC_REPLY:-$root/driver/target/debug/sdc-reply}
  [[ -x $reply_bin ]] || { echo "llm-node: sdc-reply not found ($reply_bin): $node could not run" >&2; exit 127; }
  case_dir=${SDC_CASE_DIR:-$work/..}
  "$reply_bin" review-inputs --cwd "$cwd" --case-dir "$case_dir" --story "$story" > "$rec/review-inputs.json" 2> "$rec/review-check.err" ||
    { echo "llm-node: change.review has no inputs: $(cat "$rec/review-check.err")"; exit 2; }
  "$reply_bin" schema --cwd "$cwd" --case-dir "$case_dir" --type review > "$rec/reply-schema.json"
  if [[ $review_mode == facts ]]; then
    # One request, no tools (story:review-minimal-facts): the program assembled in/review.md, the
    # only thing the model reads, and the prompt carries its text; the other files of in/ are for
    # the programs that check the reply.
    ops='[]'; mt=4; out=code_review
    review_doc=$cwd/in/review.md
    input_bytes=$(jq -r '.document_bytes' "$rec/review-inputs.json")
    notes_part facts_inputs
    render_part task facts story="$story"
  else
    # SDC_REVIEW_MODE=session (the default): a session with read tools over the whole change tree.
    ops='["dir.list","file.read","search"]'; out=code_review
    notes_part inputs story="$story" base="$(jq -r .base "$rec/review-inputs.json")"
    # The implementer's coverage document, when its tree carries one (SDC_IMPLEMENT_COVERAGE=on):
    # one more input and one more sentence; without it the review is main's.
    cov_line=""
    if [[ $(jq -r '.coverage // false' "$rec/review-inputs.json") == true ]]; then
      notes_part coverage_inputs
      render_part cov_line coverage_note
      cov_line=$'\n'$cov_line
    fi
    fill_task story="$story" coverage_note="$cov_line"
  fi ;;
# sdc.bugfix (docs/bugfix.md): the dissection is a data document under a schema (sdc-bugfix schema
# dissection), checked by dissection.check; the test and the fix are code, in the language of the
# code tree. How the programs test it is the case's checks.yaml (sdc.checks/1, `sdc-bugfix checks`),
# given to the code writers as in/checks.yaml and quoted in their task.
bug.dissect)
  input bug.yaml; input impl; out=bug-dissection.json
  "$here/sdc-tool.sh" bugfix schema dissection > "$rec/schema.json" && node_schema=$rec/schema.json
  fill_task schema="$(jq -c . "$rec/schema.json")" ;;
test.reproduce)
  input bug.yaml; input bug-dissection.json; input impl; out=repro
  checks_input
  fill_task test_file="$(ck .test_file)" test_name="$(ck .test_name)" repro_command="$(ck '.repro | join(" ")')" test_command="$(ck '.test | join(" ")')" ;;
change.fix)
  input bug.yaml; input bug-dissection.json; input repro; input impl; seed impl; out=impl
  checks_input
  repro_file=$(jq -r '.file // ""' "$work/repro/repro.json" 2>/dev/null)
  # A run seeded from a ticket works on a repository that may have dependencies of its own.
  render_part stdlib stdlib
  yq -e '.ticket' "$work/bug.yaml" >/dev/null 2>&1 && stdlib=""
  gate=$(ck '.gate[] | "  - " + .name + ": `" + (.argv | join(" ")) + "`"')
  [[ -n $gate ]] || render_part gate no_gate
  fill_task repro_test="$(jq -r '.test // ""' "$work/repro/repro.json" 2>/dev/null)" repro_file="$repro_file" stdlib="$stdlib" \
    test_command="$(ck '.test | join(" ")')" gate="$gate" ;;
*) echo "llm-node: no agent node $node" >&2; exit 64 ;;
esac
# A run seeded from a ticket (bug.yaml `ticket`, docs/bugfix.md "From a ticket"): the ticket's
# constraints bind the code writers. A run without a ticket keeps its prompt byte for byte.
if [[ $node == test.reproduce || $node == change.fix ]] && yq -e '.ticket' "$work/bug.yaml" >/dev/null 2>&1; then
  constraints=$(yq -r '(.constraints // [])[] | "- " + .' "$work/bug.yaml")
  [[ -z $constraints ]] || append_task "$shared/constraints.md" constraints="$constraints"
fi

# (Request mode: sdc-reply implement-run inlines findings.yaml and asks for findings in the reply.)
if [[ $raises == yes && $implement_mode == session ]]; then
  [[ -f $work/findings.yaml ]] && input findings.yaml
  append_task "$shared/findings.md"
fi

feedback=""
if [[ -n ${SDC_FEEDBACK:-} && -f $SDC_FEEDBACK ]] && jq -e --arg rej "$rejection" 'any(.[]; .result != "rejected" or (.log // "") != $rej)' "$SDC_FEEDBACK" > /dev/null; then
  render fb_head "$shared/feedback.heading.md"
  render fb_none "$shared/feedback.no-output.md"
  {
    echo "$fb_head"
    # The check's own output: the last 80 lines of its log without the build pool's `slot: job`
    # lines, which only say how long the check waited (runs/trial-3-haiku-8). A check that wrote
    # nothing is said so in words (instructions/shared/feedback.no-output.md). The loop reads
    # from a process substitution so that a missing instruction file stops the tool.
    while IFS=$'\x1f' read -r fb_kind fb_result fb_action fb_exit log; do
      render fb_line "$shared/feedback.check.md" kind="$fb_kind" result="$fb_result" action="$fb_action" exit="$fb_exit"
      echo; echo "$fb_line"
      own=$([[ -f $log ]] && grep -v '^slot: job ' "$log" | tail -n 80)
      if [[ -n ${own//[[:space:]]/} ]]; then echo '```'; echo "$own"; echo '```'
      else echo "$fb_none"; fi
    done < <(jq -r --arg rej "$rejection" '.[] | select(.result != "rejected" or (.log // "") != $rej)
      | [.kind, .result, (.action // "?"), (.exit // "?"), (.log // "")] | map(tostring) | join("\u001f")' "$SDC_FEEDBACK")
  } > "$cwd/in/feedback.md"
  notes "$shared/feedback.inputs.md"
  feedback=yes
fi
# This writer's own last run failed (an exit other than 0, a program error or an API break-off):
# nothing of it was applied and no check ran, so its log is the only feedback. It reaches this run
# in in/feedback.md; without it the writer ran again blind until writer_spent (docs/reject-routes.md R9).
if [[ -n $cd_ && -f $cd_/events.jsonl ]]; then
  own=$(jq -rs --arg n "$node" '[.[] | select(.event == "action_finished" and .action == $n)] | last // empty
    | select(.exit | IN("0", "75", "126", "127") | not) | "\(.exit) \(.log // "")"' "$cd_/events.jsonl" 2>/dev/null)
  if [[ -n $own && -f $cd_/blobs/${own#* } ]]; then
    render fb_run "$shared/feedback-run.heading.md" node="$node" exit="${own%% *}"
    { [[ -s $cwd/in/feedback.md ]] && echo
      echo "$fb_run"
      echo; echo '```'; grep -v '^slot: job ' "$cd_/blobs/${own#* }" | tail -n 80; echo '```'; } >> "$cwd/in/feedback.md"
    [[ $feedback == yes ]] || notes "$shared/feedback-run.inputs.md"
    feedback=yes
  fi
fi
if [[ -n $rejection ]]; then
  # The rework cap (D184): the value scripts/run-trial.sh recorded for this leg in
  # run-settings.jsonl (SDC_RUN_SETTINGS), the one the driver enforces; outside a run, the
  # default of rule-ends.json.
  rework_limit=$(jq -rs 'last.rework_limit.value // empty' "${SDC_RUN_SETTINGS:-/dev/null}" 2>/dev/null)
  [[ -n $rework_limit ]] || rework_limit=$(jq -r .rework_limit "$root/driver/examples/sdc/rule-ends.json")
  cp "$rejection" "$cwd/in/rejection.json"
  notes "$shared/rework.inputs.md" approval="$(jq -r .approval "$rejection")" revision="$(jq -r .revision "$rejection")" principal="$(jq -r .principal "$rejection")"
  append_task "$shared/rework.md" approval="$(jq -r .approval "$rejection")" count="$(jq -r .count "$rejection")" rework_limit="$rework_limit"
fi

prompt=$rec/prompt.md
# Request mode: sdc-reply implement-run writes prompt.md (the whole input); in/ only carries
# feedback.md and rejection.json to it.
[[ $implement_mode == request ]] || {
  if [[ $node == change.review && $review_mode == facts ]]; then
    render preamble "$shared/preamble-facts.md" node="$node"
  else
    render preamble "$shared/preamble.md" node="$node"
  fi
  if [[ $node == change.review ]]; then
    if [[ $review_mode == facts ]]; then
      # The review input's exact bytes (the trailing newline kept: the file ends the paragraph).
      doc=$(cat "$review_doc"; echo x)
      render closing "$shared/closing-review-facts.md" review_doc="${doc%x}"
    else
      render closing "$shared/closing-review.md"
    fi
  elif [[ $coverage == on ]]; then
    if [[ -n $rejection ]]; then
      render closing "$shared/closing-coverage-rework.md"
    else
      render closing "$shared/closing-coverage.md"
    fi
  elif [[ -n $rejection ]]; then
    render closing "$shared/closing-rework.md"
  else
    render closing "$shared/closing.md"
  fi
  input_list=$(printf -- '- %s\n' "${inputs[@]}")
  render frame "$shared/frame.md" preamble="$preamble" inputs="$input_list" task="$task" closing="$closing"
  printf '%s\n' "$frame"
} > "$prompt"

# The model: the node's class setting (SDC_LLM_MODEL_<CLASS>; classes in tools/node-classes.json:
# writer, implement, review, other; $class, set above), else SDC_LLM_MODEL, else the binding's llm.model.
# The reasoning effort the same way (SDC_LLM_EFFORT_<CLASS>, else SDC_LLM_EFFORT, else node-classes.json
# default_efforts).
cvar=SDC_LLM_MODEL_${class^^}
model=${!cvar:-${SDC_LLM_MODEL:-$(jq -r '.model // "claude-haiku-5-5"' <<< "$cfg")}}
evar=SDC_LLM_EFFORT_${class^^}
effort=${!evar:-${SDC_LLM_EFFORT:-$(jq -r --arg c "$class" '.default_efforts[$c] // empty' "$here/node-classes.json")}}
jq -n --arg model "$model" --arg prompt "$prompt" --arg cwd "$cwd" --arg rec "$rec" --arg node "$node" \
  --argjson attempt "$attempt" --argjson cfg "$cfg" --argjson ops "$ops" --argjson mt "$mt" --arg effort "$effort" \
  --arg case "${SDC_CASE:-}" --arg case_dir "${SDC_CASE_DIR:-}" '{
    model: $model, effort: (if $effort == "" then null else $effort end), prompt_file: $prompt, cwd: $cwd, record_dir: $rec, node: $node, attempt: $attempt,
    case: $case, case_dir: $case_dir,
    operations: ($cfg.operations // $ops), max_turns: ($cfg.max_turns // $mt),
    max_budget_usd: ($cfg.max_budget_usd // 1)}' > "$rec/request.json"
# A data-document node passes its schema (an adapter that has a response-schema option uses it).
[[ -n ${node_schema:-} ]] && jq --arg s "$node_schema" '. + {json_schema: $s}' "$rec/request.json" > "$rec/request.json.tmp" &&
  mv "$rec/request.json.tmp" "$rec/request.json"
# Request mode: no operation, a turn cap for the CLI's own schema re-prompt, the output limit, and
# the mode (claude-cli passes the prompt on stdin; the fixture adapter answers with a reply).
[[ $implement_mode == request ]] && jq '. + {implement_mode: "request", operations: [], max_turns: 3, max_output_tokens: 32000}' \
  "$rec/request.json" > "$rec/request.json.tmp" && mv "$rec/request.json.tmp" "$rec/request.json"
# Facts review: no operation and at most 4 turns, whatever the binding gives the session review.
[[ $review_mode == facts ]] && jq --argjson mt "$mt" '. + {operations: [], max_turns: $mt}' \
  "$rec/request.json" > "$rec/request.json.tmp" && mv "$rec/request.json.tmp" "$rec/request.json"

# One line per call, for the run log and the viewer.
log_call() {
  jq -c -n --arg node "$node" --arg case "${SDC_CASE:-}" --arg started "$started" --arg rec "$rec" \
    --argjson attempt "$attempt" --argjson code "$code" --argjson acode "$acode" --arg out "$out" --arg class "$class" \
    --arg feedback "$feedback" --argjson ib "${input_bytes:-null}" --slurpfile req "$rec/request.json" --slurpfile res "$rec/response.json" '
    {action: $node, case: $case, started: $started, attempt: $attempt, retry: ($feedback == "yes"),
     adapter: $res[0].adapter, model: $req[0].model, effort: ($req[0].effort // "default"), class: $class, allowed_operations: $req[0].operations,
     max_turns: $req[0].max_turns, max_budget_usd: $req[0].max_budget_usd,
     exit: $acode, applied_exit: $code, output: $out, duration_ms: $res[0].duration_ms,
     cost_usd: $res[0].cost_usd, num_turns: $res[0].num_turns, usage: $res[0].usage,
     offered_tools: $res[0].offered_tools, tool_calls: $res[0].tool_calls, dry: ($res[0].dry // false),
     record: $rec}
    + (if $ib == null then {} else {input_bytes: $ib} end)
    + if $res[0].implement_mode then {implement_mode: $res[0].implement_mode, requests: $res[0].requests,
        continuations: $res[0].continuations, reply_retries: $res[0].reply_retries, cause: $res[0].cause} else {} end' \
    >> "${SDC_LLM_LOG:-$rec_root/calls.jsonl}"
}

# change.review: the reply (the adapter's structured output, else out/review.json) is checked by a
# program; an invalid one is asked again with the check's reason (SDC_REVIEW_RETRIES, default 2),
# each a model call. Exit 0 approved, 1 rejected: stdout is then only the sdc.rejection/1, the step's
# log, which `retry_when` hands to change.implement as in/rejection.json. Exit 2 (an invalid reply,
# or a failed call): nothing is recorded, so an invalid reply is never an approval and never a
# rejection without reasons. 75 as below. Everything else goes to the call record.
if [[ $node == change.review ]]; then
  jq --arg s "$rec/reply-schema.json" '. + {json_schema: $s, review_attempt: 1}' "$rec/request.json" > "$rec/request.json.tmp" &&
    mv "$rec/request.json.tmp" "$rec/request.json"
  started=$(date -u +%Y-%m-%dT%H:%M:%SZ)
  principal="llm-review:$model"
  try=0
  if [[ $adapter == dry ]]; then
    "$adapter_bin" "$rec/request.json" > "$rec/response.json" 2>> "$rec/adapter-stderr.txt"; acode=$?
    run_stub > "$rec/apply.log" 2>&1; code=$?
    log_call
    exit $code
  fi
  while :; do
    rm -f "$rec/structured-output.json" "$cwd/out/review.json"
    "$adapter_bin" "$rec/request.json" > "$rec/response.json" 2>> "$rec/adapter-stderr.txt"
    acode=$?
    [[ -s $rec/response.json ]] || echo '{}' > "$rec/response.json"
    if [[ $acode != 0 ]]; then
      code=2
      [[ $(jq -r '.terminal_reason // ""' "$rec/response.json") == @(api_error|recording_diverged) ]] && code=75
      echo "the review call failed (adapter exit $acode)" > "$rec/review-check.err"
      break
    fi
    reply=$rec/structured-output.json
    [[ -s $reply ]] || reply=$cwd/out/review.json
    "$reply_bin" check-review --cwd "$cwd" --case-dir "$case_dir" --reply "$reply" --principal "$principal" \
      > "$rec/review-check.out" 2> "$rec/review-check.err"
    code=$?
    (( code == 2 && try < ${SDC_REVIEW_RETRIES:-2} )) || break
    try=$((try + 1))
    cp "$rec/response.json" "$rec/response.try$try.json"
    [[ -f $reply ]] && cp "$reply" "$rec/reply.try$try.json"
    render retry "$shared/retry-review.md" reason="$(cat "$rec/review-check.err")"
    printf '\n%s\n' "$retry" >> "$prompt"
    jq --argjson t $((try + 1)) '.review_attempt = $t' "$rec/request.json" > "$rec/request.json.tmp" &&
      mv "$rec/request.json.tmp" "$rec/request.json"
  done
  case $code in
    0) echo "change.review approved by $principal ($(jq -r '.notes | length' "$rec/review-check.out") outside-the-story note(s))" ;;
    1) jq -c . "$rec/review-check.out" ;;
    75) echo "llm-node: change.review could not run: the model's API ended the session; nothing recorded, resume later" ;;
    *) echo "llm-node: change.review recorded nothing after $((try + 1)) attempt(s) on adapter $adapter (call llm/$(basename "$rec")): $(cat "$rec/review-check.err")" ;;
  esac
  log_call
  exit $code
fi

# (Request mode makes its requests below, after apply is defined.)
try=0 coverage_invalid="" coverage_reply=""
if [[ $implement_mode == session ]]; then
started=$(date -u +%Y-%m-%dT%H:%M:%SZ)
# change.implement's coverage document (the adapter's structured output, else out/coverage.json)
# is checked against its schema by program; an invalid one is asked again with the check's reason
# (SDC_COVERAGE_RETRIES, default 2), each a model call with its own line in the call log, the
# files under out/ kept. Still invalid: coverage_invalid, exit 2, nothing applied (R9 feeds it back).
while :; do
  rm -f "$rec/structured-output.json"
  "$adapter_bin" "$rec/request.json" > "$rec/response.json"
  acode=$?
  [[ -s $rec/response.json ]] || echo '{}' > "$rec/response.json"
  [[ $coverage == on && $acode == 0 && $adapter != dry ]] || break
  coverage_reply=$rec/structured-output.json
  [[ -s $coverage_reply ]] || coverage_reply=$cwd/out/coverage.json
  "$reply_bin" check-coverage --shape --work "$work" --tree "$cwd/out/impl" --reply "$coverage_reply" 2> "$rec/coverage-check.err" && break
  (( try < ${SDC_COVERAGE_RETRIES:-2} )) || { coverage_invalid=yes; break; }
  try=$((try + 1))
  code=2; log_call
  cp "$rec/response.json" "$rec/response.try$try.json"
  [[ -f $coverage_reply ]] && cp "$coverage_reply" "$rec/coverage.try$try.json"
  render retry "$shared/retry-coverage.md" reason="$(cat "$rec/coverage-check.err")"
  printf '\n%s\n' "$retry" >> "$prompt"
  jq --argjson t $((try + 1)) '.coverage_attempt = $t' "$rec/request.json" > "$rec/request.json.tmp" &&
    mv "$rec/request.json.tmp" "$rec/request.json"
done
fi

# Apply: copy back only the declared output, through a program where the store is AEP's.
apply() {
  local f
  case $node in
    vision.write)
      [[ -f $cwd/out/vision.json && -f $cwd/out/vision.md ]] || return 1
      rm -f aep/vision/product.md
      aep plan artifact new vision product --store aep --title "$(jq -r .title "$cwd/out/vision.json")" \
        --summary "$(jq -r .summary "$cwd/out/vision.json")" --from "$cwd/out/vision.md" ;;
    roadmap.write)
      f=$cwd/out/roadmap.json
      [[ -f $f ]] && jq -e '.epics | length > 0' "$f" >/dev/null || return 1
      rm -f aep/initiative/roadmap.md aep/epic/*.md
      body=()
      [[ -s $cwd/out/roadmap.md ]] && body=(--from "$cwd/out/roadmap.md")
      aep plan artifact new initiative roadmap --store aep --title "Roadmap" \
        --summary "epics: $(jq -r '[.epics[].name] | join(" ")' "$f")" "${body[@]}" || return 1
      jq -c '.epics[]' "$f" | while IFS= read -r e; do
        aep plan artifact new epic "$(jq -r .name <<< "$e")" --store aep --title "$(jq -r .title <<< "$e")" \
          --summary "$(jq -r .summary <<< "$e")" || exit 1
      done ;;
    plan.decompose)
      f=$cwd/out/stories.json
      [[ -f $f ]] && jq -e '.stories | length > 0' "$f" >/dev/null || return 1
      for s in $(aep plan artifact list --store aep --format json | jq -r --arg e "epic:$epic" \
          '.[] | select(.kind == "story" and any(.relations[]?; .target == $e)) | .path'); do rm -f "aep/$s"; done
      jq -c '.stories[]' "$f" | while IFS= read -r s; do
        name=$(jq -r .name <<< "$s")
        aep plan artifact new story "$name" --store aep --title "$(jq -r .title <<< "$s")" \
          --summary "$(jq -r .summary <<< "$s")" --relate "delivers:epic:$epic" --relate serves:vision:product &&
        aep plan artifact scope "story:$name" --store aep --add impl &&
        aep plan artifact move "story:$name" --to active --store aep --executor "agent:$node" || exit 1
      done || return 1
      [[ $(aep plan artifact list --store aep --format json | jq -r --arg e "epic:$epic" '.[] | select(.id == $e) | .status') == active ]] ||
        aep plan artifact move "epic:$epic" --to active --store aep --executor "agent:$node" ;;
    context.default)
      f=$cwd/out/context.yaml
      [[ -f $f ]] || return 1
      # A person's decision (status changed or accepted) survives a new default pass.
      [[ -f $work/context.yaml ]] && yq -i ". *= (load(\"$work/context.yaml\")
        | with_entries(select(.value.status == \"changed\" or .value.status == \"accepted\")))" "$f"
      cp "$f" "$work/context.yaml" ;;
    questions.answer)
      # The agent may not touch a scope-changing question: those go through scope.answer. Both
      # sides quoted: an unquoted right side of == is a glob pattern, and JSON (`[`, `?`, `*`)
      # never matches itself as one (every real answer was refused in runs/trial-2-train).
      f=$cwd/out/questions.yaml
      [[ -f $f ]] || return 1
      [[ "$(yq -o=json -I=0 '[.questions[] | select(.scope_changing == true)]' "$f")" == \
         "$(yq -o=json -I=0 '[.questions[] | select(.scope_changing == true)]' "$work/questions.yaml")" ]] ||
        { echo "questions.answer changed a scope-changing question: not applied"; return 1; }
      cp "$f" "$work/questions.yaml" ;;
    bug.dissect)
      # The reply under the response schema, when the adapter had one, else the written file.
      f=$cwd/out/bug-dissection.json
      [[ -s $rec/structured-output.json ]] && f=$rec/structured-output.json
      [[ -f $f ]] || return 1
      cp "$f" "$work/bug-dissection.json" ;;
    change.implement)
      # The tree, then the checked coverage document inside it (part of the revision it describes);
      # with the switch off (or in request mode), the tree alone, as every other output.
      if [[ $coverage != on ]]; then
        [[ -e $cwd/out/$out ]] || return 1
        rm -rf "${work:?}/$out" && mkdir -p "$(dirname "$work/$out")" && cp -r "$cwd/out/$out" "$work/$out"
        return
      fi
      [[ -d $cwd/out/impl ]] || return 1
      rm -rf "${work:?}/impl" && cp -r "$cwd/out/impl" "$work/impl" && mkdir -p "$work/impl/.sdc" &&
        jq . "$coverage_reply" > "$work/impl/.sdc/coverage.json" ;;
    test.reproduce)
      [[ -d $cwd/out/repro ]] || return 1
      rm -rf "${work:?}/repro" && cp -r "$cwd/out/repro" "$work/repro" && "$here/sdc-tool.sh" bugfix repro-install ;;
    *)
      [[ -e $cwd/out/$out ]] || return 1
      rm -rf "${work:?}/$out" && mkdir -p "$(dirname "$work/$out")" && cp -r "$cwd/out/$out" "$work/$out" ;;
  esac
}

# change.implement in request mode: sdc-reply implement-run makes the call's requests (one as a
# rule; at most two retries of an invalid reply, at most two continuations by rule), writes the
# reply's files into out/impl/ and the summed response.json; apply copies out/impl/ to impl/.
# Exit 2: the reply stayed invalid (implement_reply_invalid) or incomplete, nothing written; 75 as
# below. Dry assembles the prompt, then runs the stub.
if [[ $implement_mode == request ]]; then
  started=$(date -u +%Y-%m-%dT%H:%M:%SZ)
  case_dir=${SDC_CASE_DIR:-$work/..}
  irun=("$reply_bin" implement-run --rec "$rec" --cwd "$cwd" --case-dir "$case_dir" --story "$story")
  [[ -n $rejection ]] && irun+=(--rework-limit "$rework_limit")
  if [[ $adapter == dry ]]; then
    "${irun[@]}" --assemble-only > "$rec/implement-run.out" 2>&1
    "$adapter_bin" "$rec/request.json" > "$rec/response.json" 2>> "$rec/adapter-stderr.txt"; acode=$?
    run_stub > "$rec/apply.log" 2>&1; code=$?
    [[ $code == 0 ]] && post "" >> "$rec/apply.log" 2>&1
    log_call
    exit $code
  fi
  "${irun[@]}" --adapter "$adapter_bin" > "$rec/implement-run.out" 2>> "$rec/adapter-stderr.txt"
  code=$?
  acode=$code
  [[ -s $rec/response.json ]] || echo '{}' > "$rec/response.json"
  case $code in
    0)
      cat "$rec/implement-run.out"
      apply > "$rec/apply.log" 2>&1; code=$?
      [[ $code == 0 ]] || { echo "llm-node: $node: the output was not applied (exit $code):"; tail -n 20 "$rec/apply.log"; }
      [[ $code == 0 ]] && post "$([[ -f $cwd/out/findings.yaml ]] && echo "$cwd/out/findings.yaml")" >> "$rec/apply.log" 2>&1 ;;
    75) echo "llm-node: $node could not run: $(cat "$rec/implement-run.out"); nothing applied, resume later" ;;
    *) echo "llm-node: $node recorded nothing (call llm/$(basename "$rec"), adapter $adapter): $(cat "$rec/implement-run.out")" ;;
  esac
  log_call
  exit $code
fi

if [[ $adapter == dry ]]; then
  run_stub; code=$?
  [[ $code == 0 ]] && { coverage_stub || code=126; }
  [[ $code == 0 ]] && post "" >> "$rec/apply.log" 2>&1
elif [[ $acode == 0 && -n $coverage_invalid ]]; then
  code=2
  echo "llm-node: $node: coverage_invalid: the coverage document stayed invalid after $((try + 1)) attempt(s) (call llm/$(basename "$rec")), nothing applied: $(cat "$rec/coverage-check.err")"
elif [[ $acode == 0 ]]; then
  apply > "$rec/apply.log" 2>&1; code=$?
  # Why it was not applied goes into the step's log, which the next run reads (R9 above).
  [[ $code == 0 ]] || { echo "llm-node: $node: the output was not applied (exit $code):"; tail -n 20 "$rec/apply.log"; }
  [[ $code == 0 ]] && post "$([[ -f $cwd/out/findings.yaml ]] && echo "$cwd/out/findings.yaml")" >> "$rec/apply.log" 2>&1
elif [[ $(jq -r '.terminal_reason // ""' "$rec/response.json") == recording_diverged ]]; then
  # A re-driven run (sdc-replay) whose recording diverged under the policy stop: exit 75, the driver
  # records nothing and stops; scripts/run-trial.sh ends the run with cause recording_diverged.
  echo "llm-node: $node not re-driven: the recording diverged ($(jq -r '.replay.difference // "" | if type == "array" then join("; ") else . end' "$rec/response.json"))"
  code=75
elif [[ $(jq -r '.terminal_reason // ""' "$rec/response.json") == api_error ]]; then
  # The model's API refused or broke off (a 429, an overload): the node could not run, which
  # says nothing about its subject. Exit 75: sdc-drive records nothing, counts no run and stops;
  # resume later on the same model.
  echo "llm-node: $node could not run: the model's API ended the session (api_error$(
    jq -r '(.api_error_status // empty) | " \(.)"' "$rec/response.json")); nothing applied, resume later"
  code=75
else
  code=$acode
  echo "llm-node: $node: the model call exited $acode: $(jq -r '.terminal_reason // .error // "no reason given" | tostring' "$rec/response.json" 2>/dev/null | head -c 300)"
fi

# Under Loom no sdc-drive runs the binding's postcondition, so the same program check runs here,
# after the writer: sdc-reply check-coverage (the one copy of the rule). Exit 1 is this run's exit
# 8, which the binding records as implementation_change coverage_refused (the writer is sent back,
# no conformance run is due); the check's output is the run's output and so the next run's feedback.
if [[ -n ${LOOM_CASE:-} && $coverage == on && $code == 0 && $adapter != dry ]]; then
  "$reply_bin" check-coverage --work "$work" --tree "$work/impl"; pc=$?
  case $pc in 0) ;; 1) code=8 ;; *) code=126 ;; esac
fi
log_call
exit $code
