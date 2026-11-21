#!/usr/bin/env bash
# The decider: takes one decision that would otherwise wait on a person (an approval, a
# scope-changing interview question, the root's close), and prints it as one JSON object. It
# records and applies nothing: scripts/run-trial.sh writes decisions.jsonl and runs the
# `sdc-drive approve` (or edits questions.yaml) with principal `decider:<kind>`.
#
#   decider.sh <kind> <request.json>
#
# kind: approve (rule: approve every approval, accept every question's default, close),
#       reject (rule: refuse every decision), haiku (one bounded LLM call per decision).
# request: {type: approval|question|finding|close, case_dir, action, capability, subject, path,
#           revision, question (the question object, for type question), finding (the finding
#           object of findings.yaml, for type finding), outcome (for type close),
#           position (the stopping case's position: claims, open obligations), record_dir}
# reply:   {decision: approve|reject|escalate, answer (questions and findings), reasons: [..],
#           cites: [..], kind, model, adapter, usage, cost_usd, duration_ms, valid, invalid_reason,
#           dry, record}
# A finding (type finding, amend.upstream's handoff) is decided with `approve` and an `answer`: one
# of its options, or, when it is not a rule's contradiction (no `sides`), an answer of its own; or
# `escalate`. Its in/decision.json adds `touches`: the requirements, prior decisions (answered
# questions) and context fields it cites.
#
# haiku: through the LLM adapters of llm-node.sh (SDC_DECIDER_ADAPTER, else SDC_LLM_ADAPTER:
# metaharness default, claude-cli, dry, or a path), model SDC_LLM_MODEL_DECIDER, else SDC_DECIDER_MODEL (default claude-haiku-5-5), at most
# SDC_DECIDER_MAX_TURNS (8) turns and SDC_DECIDER_MAX_BUDGET_USD (0.25). The LLM reads in/ and
# writes out/decision.json; a program validates it, and an invalid or missing reply is `escalate`.
# Under the dry adapter the command is printed, nothing is called, and the `approve` rule decides
# (marked dry), as llm-node.sh falls back to its stub.
# SDC_DECIDER_ESCALATE=final (eval runs, no person at all): the prompt also asks for `best`
# (approve or reject), required on an escalate; scripts/run-trial.sh records
# `escalation_suppressed` and takes `best` (for a question: its default; for a finding: `best` as
# its answer, else its default). Default `stop`: an escalate waits for a person.
# Claims: in/checks.json marks `settled_by_this_decision` on each claim the decided action's
# precondition admits only while it is not true (the action is there to settle it).
set -uo pipefail
for p in jq yq; do command -v "$p" >/dev/null 2>&1 || { echo "decider.sh: $p is not on PATH" >&2; exit 127; }; done # program error, not a result
here=$(cd "$(dirname "$0")" && pwd)
kind=${1:?usage: decider.sh <approve|reject|haiku> <request.json>}
req=$(cat "${2:?usage: decider.sh <kind> <request.json>}")
r() { jq -r "$1" <<< "$req"; }
type=$(r .type)
final=no
[[ ${SDC_DECIDER_ESCALATE:-stop} == final ]] && final=yes

rule() { # $1 decision, $2 reason
  local answer=null
  [[ $type == question && $1 == approve ]] && answer=$(jq '.question.default' <<< "$req")
  [[ $type == finding && $1 == approve ]] && answer=$(jq '.finding.default' <<< "$req")
  jq -cn --arg d "$1" --arg why "$2" --arg k "$kind" --argjson answer "$answer" --argjson dry "${3:-false}" \
    '{decision: $d, answer: $answer, best: null, reasons: [$why], cites: [], kind: $k, model: null, adapter: "rule",
      usage: null, cost_usd: 0, valid: true, dry: $dry}'
}

rec=$(r .record_dir)
cwd=$rec/cwd
work=$(r .case_dir)/work

# Validate the LLM's reply (out/decision.json) with a program and print the decider's reply:
# shape, decision, answer for questions only, `best` on an escalate, cited ids exist. Invalid or
# missing: escalate.
validate() {
  local ids why out=$cwd/out/decision.json acode
  acode=$(jq '.exit // 0' "$rec/response.json" 2>/dev/null || echo 0)
  meta=$(jq -c --arg k haiku --arg rec "$rec" --argjson acode "$acode" '{kind: $k, model, adapter, usage,
    cost_usd, duration_ms, exit: $acode, record: $rec}' "$rec/response.json" 2>/dev/null || echo '{}')
  # Citable: requirement, dissection item, question and finding ids, context fields, and the
  # claim ids in in/checks.json.
  ids=$( { yq -r '.requirements[].id' "$work/requirements.yaml"; yq -r '.items[].id' "$work/dissection.yaml"
           yq -r '.questions[].id' "$work/questions.yaml"; yq -r '.findings[].id' "$work/findings.yaml"
           yq -r 'keys[] | "context." + .' "$work/context.yaml"; jq -r '.claims // {} | keys[]' "$cwd/in/checks.json"
         } 2>/dev/null | jq -Rsc 'split("\n") | map(select(. != ""))')
  if [[ ! -s $out ]]; then why="out/decision.json is missing"
  elif ! jq -e . "$out" > /dev/null 2>&1; then why="out/decision.json is not JSON"
  else
    why=$(jq -r --arg type "$type" --arg final "$final" --argjson ids "$ids" --argjson req "$req" '
      def known: ["answer", "best", "cites", "decision", "reasons"];
      ($req.finding.options // []) as $opts | ($req.finding.sides != null) as $rule
      | if type != "object" then "not a JSON object"
      else .decision as $d | .best as $b | (.cites // []) as $c | .answer as $a | .reasons as $r
      | if (keys - known) != [] then "unknown keys \(keys - known)"
        elif ($d | IN("approve", "reject", "escalate") | not) then "decision is not approve, reject or escalate"
        elif $type == "finding" and $d == "reject" then "a finding is decided with approve and an answer, or escalate"
        elif ($r | type) != "array" or ($r | length) == 0 or any($r[]; type != "string" or . == "") then "reasons is not a non-empty list of sentences"
        elif ($c | type) != "array" or any($c[]; type != "string") then "cites is not a list of ids"
        elif any($c[]; IN($ids[]) | not) then "cites an id that does not exist: \([$c[] | select(IN($ids[]) | not)])"
        elif $b != null and $d != "escalate" then "best is only for an escalate"
        elif $type == "finding" and $b != null and (($b | type) != "string" or $b == "") then "best is not an answer"
        elif $type == "finding" and $rule and $b != null and ($b | IN($opts[]) | not) then "best is not one of the options of a rule finding"
        elif $type != "finding" and $b != null and ($b | IN("approve", "reject") | not) then "best is not approve or reject"
        elif $final == "yes" and $d == "escalate" and $b == null and ($type | IN("question", "finding") | not) then "an escalate needs best: approve or reject (SDC_DECIDER_ESCALATE=final)"
        elif $type == "question" and ($d == "approve" or $b == "approve") and (($a | type) != "string" or $a == "") then "an approved question needs an answer"
        elif $type == "finding" and $d == "approve" and (($a | type) != "string" or $a == "") then "an approved finding needs an answer"
        elif $type == "finding" and $d == "approve" and $rule and ($a | IN($opts[]) | not) then "the answer to a rule finding is one of its options"
        elif ($type | IN("question", "finding") | not) and $a != null then "answer is only for a question or a finding"
        else "" end end' "$out" 2>&1) || why="validator error: $why"
  fi
  if [[ -n $why ]]; then
    jq -cn --arg why "$why" --argjson m "$meta" '{decision: "escalate", answer: null, best: null,
      reasons: ["invalid decider reply: " + $why], cites: [], valid: false, invalid_reason: $why} + $m'
  else
    jq -c --argjson m "$meta" '{decision, answer: (.answer // null), best: (.best // null), reasons,
      cites: (.cites // []), valid: true} + $m' "$out"
  fi
}

case $kind in
  approve) rule approve "rule decider: approve every pending approval, accept every question's default, close at the legitimate outcome"; exit 0 ;;
  reject) rule reject "rule decider: refuse every decision"; exit 0 ;;
  validate) validate; exit 0 ;; # re-check a recorded reply (record_dir) without a call
  haiku) ;;
  *) echo "decider: unknown kind $kind (approve, reject, haiku, validate)" >&2; exit 64 ;;
esac

adapter=${SDC_DECIDER_ADAPTER:-${SDC_LLM_ADAPTER:-metaharness}}
case $adapter in
  metaharness|claude-cli|dry) adapter_bin=$here/llm-adapter-$adapter.sh ;;
  stub) jq -cn --arg k "$kind" '{decision: "escalate", answer: null, kind: $k, valid: false,
          invalid_reason: "the stub adapter has no LLM", reasons: ["no LLM: SDC_LLM_ADAPTER=stub"], cites: []}'
        exit 0 ;;
  *) adapter_bin=$adapter ;;
esac

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
# The call's exit: the script's, else the adapter's.
trap 'c=$?; (( c == 0 )) && c=${acode:-0}; [[ $c == 0 ]] && call_status finished 0 || call_status failed "$c"' EXIT

# Inputs: the idea, the intake's results, the direction's artifacts and the subject under review.
inputs=()
input() {
  local p=$1
  [[ -e $work/$p ]] || return 0
  mkdir -p "$cwd/in/$(dirname "$p")"
  rsync -a --exclude target --exclude .git --exclude '*.lock' "$work/$p" "$cwd/in/$(dirname "$p")/"
  inputs+=("in/$p")
}
input idea.yaml; input dissection.yaml; input questions.yaml; input requirements.yaml; input context.yaml
input aep/vision/product.md; input aep/initiative/roadmap.md; input aep/epic
story=$(cut -d. -f3 <<< "$(jq -r .id "$(r .case_dir)/case.json")")
[[ -n $story ]] && input "aep/story/$story.md"
path=$(r '.path // ""')
case $path in ""|idea.yaml|dissection.yaml|questions.yaml|requirements.yaml|context.yaml|aep/*) ;; *) input "$path" ;; esac
# Only the claims the decision turns on: an approval's action precondition, a close's outcome
# condition; a question, the precondition of the action that takes its answer (scope.answer), or
# all of them when that action has none. Claims about later steps (release.healthy before
# publish, requirements.valid before scope.answer) would read as failures.
ir=$(r .case_dir)/ir.json
reads=null settles=[]
if [[ -s $ir ]]; then
  case $type in
    approval) reads=$(jq -c --arg a "$(r .action)" '[.actions[$a].precondition // {} | .. | objects | .claim? | objects | .id] | unique' "$ir") ;;
    question|finding) reads=$(jq -c --arg a "$(r .action)" '[.actions[$a].precondition // {} | .. | objects | .claim? | objects | .id] | unique
      | if . == [] then null else . end' "$ir") ;;
    close) reads=$(jq -c --arg o "$(r .outcome)" '[.outcomes[$o].requires // {} | .. | objects | .claim? | objects | .id] | unique' "$ir") ;;
  esac
  # The claims the decided action is there to settle: its precondition admits them only while
  # they are not true (scope.answer: questions.settled false or unknown). Their value is not a
  # failed check, so they are marked (runs/trial-2b-fresh: the decider escalated scope.answer
  # because questions.settled was false). may_produce does not reach them: scope.answer produces
  # no evidence, questions.check checks the revision it writes.
  settles=$(jq -c --arg a "$(r .action)" '[.actions[$a].precondition // {} | .. | objects | .claim? | objects
    | {id, is}] | group_by(.id) | map(select(all(.is != "true")) | .[0].id)' "$ir")
fi
jq --argjson reads "$reads" --argjson settles "$settles" '.position | {claims: (if $reads == null then .claims
  else (.claims | with_entries(select(.key | IN($reads[])))) end
  | with_entries(if (.key | IN($settles[])) then .value += {settled_by_this_decision: true} else . end)),
  open_obligations, legitimate_outcomes}' <<< "$req" > "$cwd/in/checks.json"
inputs+=("in/checks.json (the program checks this decision turns on: each claim's value at the current revisions, the open obligations)")
# A question comes with the dissection item it was asked about and that item's words in the idea.
item=null
if [[ $type == question ]]; then
  item=$(yq -o=json -I=0 ".items[] | select(.id == \"$(r '.question.item // ""')\")" "$work/dissection.yaml" 2>/dev/null)
  [[ -n $item ]] || item=null
fi
# A finding comes with what it touches: the requirements it cites, the prior decisions (answered
# questions) it or those requirements cite, and the context fields it cites.
touches=null
if [[ $type == finding ]]; then
  input findings.yaml
  touches=$(jq -cn --argjson f "$(r .finding)" \
    --argjson req "$(yq -o=json -I=0 . "$work/requirements.yaml" 2>/dev/null || echo null)" \
    --argjson qs "$(yq -o=json -I=0 . "$work/questions.yaml" 2>/dev/null || echo null)" \
    --argjson ctx "$(yq -o=json -I=0 . "$work/context.yaml" 2>/dev/null || echo null)" '
    ($f.cites // []) as $c | [($req.requirements // [])[] | select(.id | IN($c[]))] as $rs
    | ($c + [$rs[].cites[]?]) as $qids
    | {requirements: $rs,
       decisions: [($qs.questions // [])[] | select((.id | IN($qids[])) and (.status == "accepted" or .status == "changed"))
         | {id, question, answer, status, answered_by}],
       context: [$c[] | select(startswith("context.")) | ltrimstr("context.") as $k
         | {field: ("context." + $k), value: $ctx[$k].value, status: $ctx[$k].status}]}')
fi
jq --argjson item "$item" --argjson touches "$touches" '{type, case: (.case_dir | split("/") | last), action, capability,
  subject, artifact_path: .path, revision, question, item: $item, idea_excerpt: ($item.span // null),
  finding: (.finding // null), touches: $touches, outcome}' <<< "$req" > "$cwd/in/decision.json"

case $type in
  approval) what="Decide whether to approve \`$(r .action)\`: the $(r .subject) at revision $(r .revision) (in/$path).
Approve when it is consistent with the idea, the requirements and the context, and every check in
in/checks.json about it holds. Reject when it contradicts a requirement or is not usable; say what is
wrong.
How to read the inputs: AEP artifacts are small by design. The roadmap (in/aep/initiative/roadmap.md)
names its epics in its summary; each epic is its own file in/aep/epic/<name>.md, and the roadmap is
the two together. The requirements cover the whole product; one epic or story need not restate them.
In in/checks.json a claim's \`value\` is the check's result at the current revision;
\`excluded_evidence\` only lists older records set aside, which is normal after a rewrite. The open
obligation of what you decide (for example roadmap.required) is open because your approval is
missing." ;;
  question) what="Answer the interview question in in/decision.json (\`question\`: its text, \`options\`, \`default\`;
\`item\`: the dissection item it was asked about; \`idea_excerpt\`: that item's words in the idea).
Approve with \`answer\` = the default, unless the default contradicts the idea; then \`answer\` is the
option (or a short better answer) that fits the idea. The question is scope-changing: escalate when
every answer, the default included, would add a capability, a kind of user or a component the idea
does not ask for." ;;
  finding) what="Decide the finding in in/decision.json (\`finding\`: \`says\`, \`options\`, \`default\`, \`cites\`;
\`touches\`: the requirements, the prior decisions and the context fields it cites; all findings
in in/findings.yaml). Approve with \`answer\` = the option that fits the idea and the requirements
best$( [[ $(r '.finding.sides != null') == true ]] && echo " (this finding is a rule's contradiction: answer with one of its options exactly)" || echo ", or a short answer of your own when no option fits"). Your answer amends the cited requirements.
Never choose an answer that contradicts or empties a prior decision in \`touches.decisions\` that
a person or an earlier decider took (answered_by not non-interactive): escalate instead. A program
checks the amendment and refuses one that does; then a person decides." ;;
  close) what="Decide whether to close the idea through the outcome \`$(r .outcome)\`. Approve when every
obligation is discharged and the checks in in/checks.json hold; reject when the result does not
meet the requirements." ;;
esac

prompt=$rec/prompt.md
{
  echo "You take one decision in a governed software delivery cycle in place of the product owner."
  echo "Work only in the current directory. Read the inputs under in/; write only out/decision.json."
  echo "You have no shell. A program validates what you write; anything else in your reply is ignored."
  echo
  echo "Inputs:"
  printf -- '- %s\n' "${inputs[@]}" "in/decision.json (what is decided)"
  echo
  echo "Decision:"
  echo "$what"
  echo
  echo "A claim in in/checks.json marked \`settled_by_this_decision\` is one that taking this decision"
  echo "makes true: its value now (false or unknown) is not a failed check."
  echo
  echo "Escalate to a person, instead of approving or rejecting, when the decision would change scope"
  echo "beyond the requirements (or, before requirements exist, beyond the idea), when the evidence"
  echo "conflicts (checks disagree, the artifact contradicts an input), or when you cannot tell."
  echo
  echo "Write out/decision.json, exactly these keys:"
  if [[ $type == finding ]]; then
    echo '{"decision": "approve" | "escalate",'
    echo ' "answer": <when approve: the answer string>,'
  else
    echo '{"decision": "approve" | "reject" | "escalate",'
    echo ' "answer": <for a question: the answer string; otherwise null>,'
  fi
  echo ' "reasons": ["<one sentence each, at least one>"],'
  echo ' "cites": ["<ids your reasons rest on: requirement ids (r1, ...), else dissection item, question or finding ids, context.<field>, or claim ids from in/checks.json>"],'
  if [[ $final == yes && $type == finding ]]; then
    echo ' "best": <when decision is escalate: the answer you would take if no person could be asked; otherwise null>}'
    echo
    echo "No person is available in this run: on escalate, \`best\` is taken, else the finding's default."
  elif [[ $final == yes && $type == question ]]; then
    echo ' "best": null}'
    echo
    echo "No person is available in this run: on escalate, the question's default is taken."
  elif [[ $final == yes ]]; then
    echo ' "best": <when decision is escalate: "approve" or "reject", the decision you would take if no'
    echo '          person could be asked; otherwise null>}'
    echo
    echo "No person is available in this run: on escalate, \`best\` is taken."
  else
    echo ' "best": null}'
  fi
  echo
  echo "When the file is written, reply with the single word: done."
} > "$prompt"

model=${SDC_LLM_MODEL_DECIDER:-${SDC_DECIDER_MODEL:-claude-haiku-5-5}}
node=decide.$(r .action)
jq -n --arg model "$model" --arg prompt "$prompt" --arg cwd "$cwd" --arg rec "$rec" --arg node "$node" \
  --arg case "$(jq -r '.id // ""' "$(r .case_dir)/case.json" 2>/dev/null)" --arg case_dir "$(r .case_dir)" '{
  model: $model, prompt_file: $prompt, cwd: $cwd, record_dir: $rec, node: $node, attempt: 1,
  case: $case, case_dir: $case_dir,
  operations: ["dir.list", "file.read", "file.write"],
  max_turns: ($ENV.SDC_DECIDER_MAX_TURNS // "8" | tonumber),
  max_budget_usd: ($ENV.SDC_DECIDER_MAX_BUDGET_USD // "0.25" | tonumber)}' > "$rec/request.json"

"$adapter_bin" "$rec/request.json" > "$rec/response.json"
acode=$?
jq -e 'type == "object"' "$rec/response.json" > /dev/null 2>&1 || echo '{}' > "$rec/response.json"
jq --argjson acode "$acode" '.exit = $acode' "$rec/response.json" > "$rec/response.json.tmp" && mv "$rec/response.json.tmp" "$rec/response.json"

if [[ $adapter == dry ]]; then
  rule approve "dry run: no LLM was called; the approve rule decided" true | jq -c --arg rec "$rec" \
    --slurpfile m "$rec/response.json" '. + {kind: "haiku", model: $m[0].model, adapter: "dry", exit: $m[0].exit, record: $rec, dry: true}'
  exit 0
fi
validate

