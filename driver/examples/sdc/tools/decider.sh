#!/usr/bin/env bash
# The decider: takes one decision that would otherwise wait on a person (an approval, a
# scope-changing interview question, the root's close), and prints it as one JSON object. It
# records and applies nothing: scripts/run-trial.sh writes decisions.jsonl and runs the
# `sdc-drive approve` (or edits questions.yaml) with principal `decider:<kind>`.
#
#   decider.sh <kind> <request.json>
#
# kind: approve (rule: approve every approval, accept every question's default, close),
#       reject (rule: refuse every decision), haiku (one bounded LLM call per decision),
#       requester (the requester agent, decider:requester: below, "The requester agent").
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
# metaharness default, claude-cli, dry, or a path), model SDC_LLM_MODEL_DECIDER, else SDC_DECIDER_MODEL (default claude-haiku-5-5),
# reasoning effort SDC_LLM_EFFORT_DECIDER, else SDC_LLM_EFFORT, else node-classes.json default_efforts (xhigh), at most
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
case_dir=$(r .case_dir)
work=$case_dir/work
# What a reply may cite, its resolution and the reply schema: the driver crate's sdc-reply.
reply_bin=${SDC_REPLY:-$here/../../../target/debug/sdc-reply}
need_reply() { [[ -x $reply_bin ]] || { echo "decider.sh: sdc-reply not found ($reply_bin)" >&2; exit 127; }; }

# Validate the LLM's reply (out/decision.json) with a program and print the decider's reply:
# shape, decision, answer for questions only, `best` on an escalate, every cite resolves
# (sdc-reply resolve: an id of the case, <artifact>@<revision>[#<entry>], <file>:<line>).
# Invalid or missing: escalate, with the reason (scripts/run-trial.sh retries with it).
validate() {
  local why out=$cwd/out/decision.json acode res='[]' cites=()
  need_reply
  acode=$(jq '.exit // 0' "$rec/response.json" 2>/dev/null || echo 0)
  meta=$(jq -c --arg k haiku --arg rec "$rec" --argjson acode "$acode" '{kind: $k, model, effort, adapter, usage,
    cost_usd, duration_ms, exit: $acode, record: $rec, schema_constrained: (.schema_constrained // false),
    schema_note: (.schema_note // null)}' "$rec/response.json" 2>/dev/null || echo '{}')
  if [[ -s $out ]] && jq -e . "$out" > /dev/null 2>&1; then
    mapfile -t cites < <(jq -r '.cites | if type == "array" then .[] | strings else empty end' "$out" 2>/dev/null)
    (( ${#cites[@]} )) && res=$("$reply_bin" resolve --cwd "$cwd" --case-dir "$case_dir" "${cites[@]}")
  fi
  if [[ ! -s $out ]]; then why="out/decision.json is missing"
  elif ! jq -e . "$out" > /dev/null 2>&1; then why="out/decision.json is not JSON"
  else
    why=$(jq -r --arg type "$type" --arg final "$final" --argjson res "$res" --argjson req "$req" '
      def known: ["answer", "best", "cites", "decision", "reasons"];
      ($req.finding.options // []) as $opts | ($req.finding.sides != null) as $rule
      | if type != "object" then "not a JSON object"
      else .decision as $d | .best as $b | (.cites // []) as $c | .answer as $a | .reasons as $r
      | if (keys - known) != [] then "unknown keys \(keys - known)"
        elif ($d | IN("approve", "reject", "escalate") | not) then "decision is not approve, reject or escalate"
        elif $type == "finding" and $d == "reject" then "a finding is decided with approve and an answer, or escalate"
        elif $type == "question" and $d == "reject" then "a question is decided with approve and an answer, or escalate"
        elif ($r | type) != "array" or ($r | length) == 0 or any($r[]; type != "string" or . == "") then "reasons is not a non-empty list of sentences"
        elif ($c | type) != "array" or any($c[]; type != "string") then "cites is not a list of ids"
        elif any($res[]; .ok | not) then "cites what does not resolve: " + ([$res[] | select(.ok | not) | "\(.cite) (\(.why))"] | join("; "))
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
    jq -c --argjson m "$meta" --argjson res "$res" '{decision, answer: (.answer // null), best: (.best // null), reasons,
      cites: (.cites // []), citations: [$res[] | {cite, form, value}], valid: true} + $m' "$out"
  fi
}

case $kind in
  approve) rule approve "rule decider: approve every pending approval, accept every question's default, close at the legitimate outcome"; exit 0 ;;
  reject) rule reject "rule decider: refuse every decision"; exit 0 ;;
  validate) validate; exit 0 ;; # re-check a recorded reply (record_dir) without a call
  haiku|requester) need_reply ;;
  *) echo "decider: unknown kind $kind (approve, reject, haiku, requester, validate)" >&2; exit 64 ;;
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

# The requester agent (docs/requester.md § 3, story:requester-decider): principal
# decider:requester, for the requester-side classes. Its in/ holds exactly request.json (the
# contract's request: from sdc-decision-request for a question, built here for an approval or a
# close), the subject file, the files the request's grounds cite, idea.yaml, dissection.yaml,
# goals.yaml and decisions.json (its own earlier decisions of this run); nothing else. It writes
# out/reply.json {option, answer, reasons, grounds, strength, no_ground_in_goals}; the grounds go
# through tools/decision-grounds.sh. No ground: an escalate (D→P waits on the requester person;
# under final run-trial.sh takes the request's default, marked no_ground_in_goals).
requester_call() {
  local level qid request subj sp f g reply_f prop chk verdict letter ans dec_ best opts no_ground model node
  level=$(r '.autonomy // "A3"')
  case $type in
    question)
      qid=$(r .question.id)
      request=$("$reply_bin_dir/sdc-decision-request" "$case_dir" "$qid" --level "$level" --holder decider \
        --eligible "decider:requester,${SDC_REQUESTER_PERSON:-person:requester}" --opened-at "$(date -u +%Y-%m-%dT%H:%M:%SZ)" | jq -c .request) ||
        { jq -cn --arg q "$qid" '{decision: "escalate", answer: null, best: null, reasons: ["no decision request for \($q) (sdc-decision-request refused it)"],
            cites: [], kind: "requester", valid: false, invalid_reason: "no decision request"}'; return 0; } ;;
    *)
      sp=$(r '.path // ""')
      request=$(jq -c --arg lvl "$level" --arg sp "$sp" --arg cid "$(jq -r .id "$case_dir/case.json")" '{
        request_id: "DR-\($cid)-\(.action)-\(.revision // "none")", state: "Open", case_id: $cid,
        question_key: "\($cid)|\(.action)", unblocks: {kind: .type, action: .action},
        subject_artifact: (.subject // "case"), subject_revision: .revision,
        subject_path: (if $sp == "" then null else "work/\($sp)" end),
        question: (if .type == "close" then "Close the idea through the outcome \(.outcome)?"
          else "Approve \(.action): the \(.subject) at revision \(.revision)?" end),
        options: [{letter: "A", does: (if .type == "close" then "close through \(.outcome)" else "approve" end)},
                  {letter: "B", does: (if .type == "close" then "do not close" else "reject: the writer reworks it" end)}],
        autonomy_level: $lvl, decision_class: .class, holder: "decider", eligible: ["decider:requester"],
        addressed_to: "decider:requester", grounds: [], decision_reasons: []}' <<< "$req") ;;
  esac
  printf '%s\n' "$request" > "$cwd/in/request.json"
  inputs=("in/request.json (the decision request: question, options, the default and its grounds)")
  rinput() { # <path under work/>: one input file, copied as it is
    [[ -n $1 && -f $work/$1 && ! -e $cwd/in/$1 ]] || return 0
    mkdir -p "$cwd/in/$(dirname "$1")"; cp "$work/$1" "$cwd/in/$1"; inputs+=("in/$1")
  }
  sp=$(jq -r '.subject_path // "" | ltrimstr("work/")' <<< "$request")
  rinput "$sp"
  while IFS= read -r f; do rinput "$f"; done < <(jq -r '.grounds[]?.citation | select(.form == "path_line") | .value.path | ltrimstr("work/")' <<< "$request")
  rinput idea.yaml; rinput dissection.yaml; rinput goals.yaml
  jq -s '[.[] | select(.principal == "decider:requester" and .valid != false)
    | {case, action, question, revision, decision, answer, reasons, grounds}]' "$(r '.decisions_log // "/dev/null"')" > "$cwd/in/decisions.json" 2>/dev/null ||
    echo '[]' > "$cwd/in/decisions.json"
  inputs+=("in/decisions.json (your earlier decisions in this run)")

  prompt=$rec/prompt.md
  {
    echo "You answer for the requester: the person or organisation that asked for this software."
    echo "Its goals, principles, constraints and preferences are in in/goals.yaml; its idea in in/idea.yaml."
    echo "Work only in the current directory. Read the inputs under in/; write only out/reply.json."
    echo
    echo "Inputs:"
    printf -- '- %s\n' "${inputs[@]}"
    echo
    echo "Choose the option (or, for a question, a short answer of your own) that the entries of"
    echo "in/goals.yaml support. Never choose an option that crosses a constraint. Cite every entry you rely"
    echo "on. When no entry covers the decision, or two entries of equal priority conflict, say so with"
    echo "no_ground_in_goals: true; do not guess."
    echo
    echo "Write out/reply.json, exactly these keys:"
    echo '{"option": "<letter of the chosen option, or null with a free answer>",'
    echo ' "answer": "<for a question: a free answer when no option fits; otherwise null>",'
    echo ' "reasons": ["<one sentence each>"],'
    echo ' "grounds": [{"source": "goals", "citation": {"form": "artifact_revision", "value": {"artifact": "goals",'
    echo '     "revision": "<the goals revision>", "entry": "<entry id>"}}, "supports": "<letter, absent for a free answer>",'
    echo '     "how": "<one sentence: how the entry supports it>"}],'
    echo ' "strength": "strong | moderate | weak",'
    echo ' "no_ground_in_goals": true | false}'
    echo "The goals revision is $(jq -r '.artifacts.goals.revision // "none"' "$case_dir/case.json"); idea and prior grounds"
    echo "(source idea, prior_decision) cite lines as {\"form\": \"path_line\", \"value\": {\"path\": \"work/<file>\", \"line\": <n>}}."
    if [[ $(r '.retry.attempt // 1') -gt 1 ]]; then
      echo
      echo "This is attempt $(r .retry.attempt). Your previous reply was refused by the program check:"
      echo "  $(r .retry.invalid_reason)"
    fi
    echo
    echo "When the file is written, reply with the single word: done."
  } > "$prompt"
  model=${SDC_LLM_MODEL_REQUESTER:-${SDC_LLM_MODEL_DECIDER:-${SDC_DECIDER_MODEL:-claude-haiku-5-5}}}
  effort=${SDC_LLM_EFFORT_REQUESTER:-${SDC_LLM_EFFORT_DECIDER:-${SDC_LLM_EFFORT:-$(jq -r .default_efforts.requester "$here/node-classes.json")}}}
  node=requester.$(r '.question.id // .action')
  jq -n --argjson attempt "$(r '.retry.attempt // 1')" --arg model "$model" --arg effort "$effort" --arg prompt "$prompt" --arg cwd "$cwd" --arg rec "$rec" --arg node "$node" \
    --arg case "$(jq -r '.id // ""' "$case_dir/case.json")" --arg case_dir "$case_dir" '{model: $model, effort: (if $effort == "" then null else $effort end), prompt_file: $prompt, cwd: $cwd,
    record_dir: $rec, node: $node, attempt: $attempt, case: $case, case_dir: $case_dir, operations: ["dir.list", "file.read", "file.write"],
    max_turns: ($ENV.SDC_DECIDER_MAX_TURNS // "8" | tonumber), max_budget_usd: ($ENV.SDC_DECIDER_MAX_BUDGET_USD // "0.25" | tonumber)}' > "$rec/request.json"
  "$adapter_bin" "$rec/request.json" > "$rec/response.json"
  acode=$?
  jq -e 'type == "object"' "$rec/response.json" > /dev/null 2>&1 || echo '{}' > "$rec/response.json"
  meta=$(jq -c --arg rec "$rec" --argjson acode "$acode" '{kind: "requester", model, effort, adapter, usage, cost_usd, duration_ms, exit: $acode,
    record: $rec}' "$rec/response.json" 2>/dev/null || echo '{"kind": "requester"}')
  if [[ $adapter == dry ]]; then
    rule approve "dry run: no LLM was called; the approve rule decided" true | jq -c --argjson m "$meta" '. + $m + {dry: true}'
    return 0
  fi
  invalid() { jq -cn --arg why "$1" --argjson m "$meta" '{decision: "escalate", answer: null, best: null,
    reasons: ["invalid requester reply: " + $why], cites: [], valid: false, invalid_reason: $why} + $m'; }
  reply_f=$cwd/out/reply.json
  [[ -s $reply_f ]] && jq -e 'type == "object"' "$reply_f" > /dev/null 2>&1 || { invalid "out/reply.json is missing or not a JSON object"; return 0; }
  no_ground=$(jq -r '.no_ground_in_goals == true' "$reply_f")
  opts=$(jq -c '[.options[].letter]' <<< "$request")
  if [[ $no_ground == true ]]; then
    # No ground in the goals: escalate; under final the default (a question) or best is taken.
    best=null; [[ $type != question ]] && best='"approve"'
    jq -cn --argjson m "$meta" --argjson best "$best" --slurpfile o "$reply_f" '{decision: "escalate", answer: null, best: $best,
      reasons: (["no ground in the goals"] + ($o[0].reasons // [])), cites: [], grounds: [], strength: ($o[0].strength // "weak"),
      no_ground_in_goals: true, valid: true} + $m'
    return 0
  fi
  prop=$rec/proposal.json
  jq -n --argjson req "$request" --slurpfile o "$reply_f" '{request: $req, proposal: ({strength: $o[0].strength, default_on_silence: false,
    grounds: ($o[0].grounds // []), proposed_by: "llm"} + (if $o[0].option != null then {recommended_option: $o[0].option}
    else {recommended_answer: $o[0].answer} end))}' > "$prop" || { invalid "out/reply.json does not have the reply's keys"; return 0; }
  local at_args=(); [[ -n $(jq -r '.opened_at // empty' <<< "$request") ]] || at_args=(--at "$(date -u +%Y-%m-%dT%H:%M:%SZ)")
  chk=$("$here/../../../../tools/decision-grounds.sh" "$prop" --case "$case_dir" "${at_args[@]}" 2>&1)
  printf '%s\n' "$chk" > "$rec/grounds-check.json"
  verdict=$(jq -r '.recommend.check_verdict // "refused"' <<< "$chk" 2>/dev/null || echo refused)
  [[ $verdict == verified ]] || { invalid "the grounding check refused it: $(jq -r '.recommend.check_report // empty' <<< "$chk" 2>/dev/null || tail -n 1 <<< "$chk")"; return 0; }
  letter=$(jq -r '.option // empty' "$reply_f")
  if [[ $type == question ]]; then
    dec_=approve
    if [[ -n $letter ]]; then
      ans=$(jq -r --arg l "$letter" '.options[] | select(.letter == $l) | .consequence.answer // .does' <<< "$request")
    else ans=$(jq -r .answer "$reply_f"); fi
  else
    ans=""; dec_=approve; [[ $letter == B ]] && dec_=reject
  fi
  jq -cn --arg d "$dec_" --arg a "$ans" --arg t "$type" --argjson m "$meta" --argjson c "$chk" --slurpfile o "$reply_f" '{decision: $d,
    answer: (if $t == "question" then $a else null end), best: null, reasons: $o[0].reasons, cites: [],
    grounds: $c.recommend.grounds, citations: [$c.recommend.grounds[] | {cite: ("\(.citation.value.artifact // .citation.value.path)"
      + (if .citation.value.entry then "#\(.citation.value.entry)" else "" end)), form: .citation.form, value: .citation.value}],
    strength: $o[0].strength, no_ground_in_goals: false, option: $o[0].option, grounds_check: $c.recommend.check_report, valid: true} + $m'
}
if [[ $kind == requester ]]; then
  reply_bin_dir=$(dirname "$reply_bin")
  adapter=${SDC_DECIDER_ADAPTER:-${SDC_LLM_ADAPTER:-metaharness}}
  case $adapter in
    metaharness|claude-cli|dry) adapter_bin=$here/llm-adapter-$adapter.sh ;;
    *) adapter_bin=$adapter ;;
  esac
  requester_call
  exit 0
fi

# Inputs: the idea, the intake's results, the direction's artifacts and the subject under review.
inputs=()
input() {
  local p=$1
  [[ -e $work/$p ]] || return 0
  mkdir -p "$cwd/in/$(dirname "$p")"
  # Cargo.lock is kept: without it the decider doubts a `--locked` build the checks recorded
  # (runs/trial-3-haiku-5 release.publish, runs/trial-3-mixed-3 merge.approve and release.publish).
  rsync -a --exclude target --exclude .git --include Cargo.lock --exclude '*.lock' "$work/$p" "$cwd/in/$(dirname "$p")/"
  inputs+=("in/$p")
}
input idea.yaml; input dissection.yaml; input questions.yaml; input requirements.yaml; input context.yaml
# A close judges the result, not the plan: the published README instead of aep/ (runs/trial-3-haiku-5:
# the close read the roadmap's aep `status: draft` as a disagreement with roadmap.approved).
if [[ $type == close ]]; then
  input published/README.md
else
  input aep/vision/product.md; input aep/initiative/roadmap.md; input aep/epic
fi
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
# What only a step after the decided action can meet (sdc-reply later, from the protocol): never an
# open condition of this decision; listed apart under met_after_this_action (runs/trial-3-mixed-3:
# health.required, met by release.observe after release.publish, read as unmet).
later='{"claims": [], "obligations": []}'
[[ -s $ir && $type != close ]] && later=$("$reply_bin" later --ir "$ir" --action "$(r .action)" 2>/dev/null || echo "$later")
jq --argjson reads "$reads" --argjson settles "$settles" --argjson later "$later" '.position | {claims: (if $reads == null then .claims
  else (.claims | with_entries(select(.key | IN($reads[])))) end
  | with_entries(select(.key | IN($later.claims[]) | not))
  | with_entries(if (.key | IN($settles[])) then .value += {settled_by_this_decision: true} else . end)),
  open_obligations: [.open_obligations[]? | select(IN($later.obligations[]) | not)], legitimate_outcomes}
  + ([.open_obligations[]? | select(IN($later.obligations[]))] as $o | ((.claims // {}) | keys - (keys - $later.claims)) as $c
    | if ($o + $c) == [] then {} else {met_after_this_action: {action: $later.action, obligations: $o, claims: $c,
      note: "met after \($later.action), not a condition of this decision"}} end)' <<< "$req" > "$cwd/in/checks.json"
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
# What the reply may cite (sdc-reply citable): the artifacts in in/ at their revisions, and one
# example of each citation form from these inputs.
citable=$("$reply_bin" citable --cwd "$cwd" --case-dir "$case_dir")
# An approval rejected before (sdc-reply rejections: the case's rejected records of this action,
# with their reasons): the decider checks the new revision against them.
prior='[]'
[[ $type == approval ]] && prior=$("$reply_bin" rejections --case-dir "$case_dir" --action "$(r .action)")
jq --argjson item "$item" --argjson touches "$touches" --argjson cit "$citable" --argjson prior "$prior" '{type, case: (.case_dir | split("/") | last), action, capability,
  subject, artifact_path: .path, revision, question, item: $item, idea_excerpt: ($item.span // null),
  finding: (.finding // null), touches: $touches, outcome, citable_artifacts: $cit.artifacts,
  prior_rejections: $prior}' <<< "$req" > "$cwd/in/decision.json"
# The reply schema (sdc-reply schema): decision, answer and cites as enums of what is admissible.
schema_args=(--type "$type")
[[ $final == yes ]] && schema_args+=(--final)
[[ $type == finding ]] && schema_args+=(--options "$(jq -c '.finding.options // []' <<< "$req")")
[[ $type == finding && $(r '.finding.sides != null') == true ]] && schema_args+=(--rule)
"$reply_bin" schema --cwd "$cwd" --case-dir "$case_dir" "${schema_args[@]}" > "$rec/reply-schema.json"

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
if [[ $(jq length <<< "$prior") -gt 0 ]]; then
  what="$what
This approval was rejected before, at an earlier revision, and the writer has reworked it since
(\`prior_rejections\` in in/decision.json: each rejection's revision and reasons). Check whether the
current revision addresses each of those reasons; a reason still not addressed is a reason to reject."
fi

prompt=$rec/prompt.md
{
  echo "You take one decision in a governed software delivery cycle in place of the product owner."
  [[ $final == yes ]] &&
    echo "In this run a decider takes every decision a person would take: an answer with \`answered_by: decider:<kind>\` is the product owner's own decision, as binding as a person's, and is never a reason to reject or escalate."
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
  jq -e '.met_after_this_action' "$cwd/in/checks.json" > /dev/null 2>&1 &&
    echo "\`met_after_this_action\` in in/checks.json lists what can only be met by a step after $(r .action): not a condition of this decision, never a reason to reject it."
  [[ -e $cwd/in/aep ]] &&
    echo "\`status\` and \`revision\` in the front matter of in/aep/ files belong to the planning tool and say nothing about approval: an approval is its claim in in/checks.json."
  echo
  if [[ $final == yes ]]; then
    case $type in
      question) taken="the question's default is taken" ;;
      finding) taken="give \`best\`, the answer that is then taken (without it, the finding's default)" ;;
      *) taken="give \`best\`, the decision that is then taken" ;;
    esac
    echo "No person can be asked in this run, so escalating hands the decision to no one. When the decision"
    echo "would change scope beyond the requirements (or, before requirements exist, beyond the idea), when"
    echo "the evidence conflicts (checks disagree, the artifact contradicts an input), or when you cannot"
    echo "tell: write \`escalate\`, say in \`reasons\` what cannot be told, and $taken."
  else
    echo "Escalate to a person, instead of approving or rejecting, when the decision would change scope"
    echo "beyond the requirements (or, before requirements exist, beyond the idea), when the evidence"
    echo "conflicts (checks disagree, the artifact contradicts an input), or when you cannot tell."
  fi
  echo
  echo "Write out/decision.json, exactly these keys:"
  if [[ $type == finding || $type == question ]]; then
    echo '{"decision": "approve" | "escalate",'
    echo ' "answer": <when approve: the answer string>,'
  else
    echo '{"decision": "approve" | "reject" | "escalate",'
    echo ' "answer": <for a question: the answer string; otherwise null>,'
  fi
  echo ' "reasons": ["<one sentence each, at least one>"],'
  echo ' "cites": ["<what your reasons rest on, each in one of the forms below>"],'
  # Final: what an escalate takes is said once, in the escalate paragraph above.
  if [[ $final == yes && $type == finding ]]; then
    echo ' "best": <when decision is escalate: the answer that is taken; otherwise null>}'
  elif [[ $final == yes && $type == question ]]; then
    echo ' "best": null}'
  elif [[ $final == yes ]]; then
    echo ' "best": <when decision is escalate: "approve" or "reject", the decision that is taken; otherwise null>}'
  else
    echo ' "best": null}'
  fi
  echo
  echo "Citable forms (a program resolves every entry of \`cites\` and refuses the reply if one does not):"
  jq -r '"- an id of this case: a requirement, dissection item, question or finding id, a context field as context.<field>, or a claim id of in/checks.json"
      + (if .examples.id then " (example: \(.examples.id))" else "" end),
    (if .examples.entry then "- an entry of a YAML or JSON input: <artifact>@<revision>#<field path>, with the artifact and revision from `citable_artifacts` in in/decision.json; a list element is named by its id (example: \(.examples.entry))" else empty end),
    (if .examples.line then "- lines of an input file: <file>:<line> or <file>:<first>-<last>, the file as under in/ without in/ (example: \(.examples.line))" else empty end)' <<< "$citable"
  if [[ $(r '.retry.attempt // 1') -gt 1 ]]; then
    echo
    echo "This is attempt $(r .retry.attempt). Your previous reply was refused by the program check:"
    echo "  $(r .retry.invalid_reason)"
    echo "Write out/decision.json again with that fixed."
  fi
  echo
  echo "When the file is written, reply with the single word: done."
} > "$prompt"

model=${SDC_LLM_MODEL_DECIDER:-${SDC_DECIDER_MODEL:-claude-haiku-5-5}}
effort=${SDC_LLM_EFFORT_DECIDER:-${SDC_LLM_EFFORT:-$(jq -r .default_efforts.decider "$here/node-classes.json")}}
node=decide.$(r .action)
jq -n --argjson attempt "$(r '.retry.attempt // 1')" --arg model "$model" --arg effort "$effort" --arg prompt "$prompt" --arg cwd "$cwd" --arg rec "$rec" --arg node "$node" \
  --arg case "$(jq -r '.id // ""' "$(r .case_dir)/case.json" 2>/dev/null)" --arg case_dir "$(r .case_dir)" '{
  model: $model, effort: (if $effort == "" then null else $effort end), prompt_file: $prompt, cwd: $cwd, record_dir: $rec, node: $node,
  attempt: $attempt, json_schema: ($rec + "/reply-schema.json"),
  case: $case, case_dir: $case_dir,
  operations: ["dir.list", "file.read", "file.write"],
  max_turns: ($ENV.SDC_DECIDER_MAX_TURNS // "8" | tonumber),
  max_budget_usd: ($ENV.SDC_DECIDER_MAX_BUDGET_USD // "0.25" | tonumber)}' > "$rec/request.json"

"$adapter_bin" "$rec/request.json" > "$rec/response.json"
acode=$?
jq -e 'type == "object"' "$rec/response.json" > /dev/null 2>&1 || echo '{}' > "$rec/response.json"
# A reply the adapter got under the response schema (structured-output.json) is the reply.
if jq -e 'type == "object"' "$rec/structured-output.json" > /dev/null 2>&1; then
  cp "$rec/structured-output.json" "$cwd/out/decision.json"
fi
case $adapter in
  metaharness) note="not constrained: \`metaharness run claude\` has no response-schema option; the program check and the retry hold the reply" ;;
  fixture|*llm-adapter-fixture.sh) note="not constrained: a scripted reply, no model" ;;
  *) note="not constrained: the adapter passed no response schema" ;;
esac
jq --arg n "$note" '.schema_constrained //= false | if .schema_constrained then . else .schema_note //= $n end' \
  "$rec/response.json" > "$rec/response.json.tmp" && mv "$rec/response.json.tmp" "$rec/response.json"
jq --argjson acode "$acode" '.exit = $acode' "$rec/response.json" > "$rec/response.json.tmp" && mv "$rec/response.json.tmp" "$rec/response.json"

if [[ $adapter == dry ]]; then
  rule approve "dry run: no LLM was called; the approve rule decided" true | jq -c --arg rec "$rec" \
    --slurpfile m "$rec/response.json" '. + {kind: "haiku", model: $m[0].model, effort: $m[0].effort, adapter: "dry", exit: $m[0].exit, record: $rec, dry: true}'
  exit 0
fi
validate

