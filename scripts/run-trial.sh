#!/usr/bin/env bash
# Run one SDC trial with the real LLM executor, from an idea to the root outcome, and stop at
# every step that is a person's decision. Run it again to resume after that decision.
#
#   scripts/run-trial.sh <name> [idea text]     real run (adapter $SDC_LLM_ADAPTER, default metaharness)
#   scripts/run-trial.sh --dry <name>           dry run: the stub fixture idea, the stubs write the
#                                               artifacts, every LLM command is printed and none is
#                                               run, and the person's approvals are scripted
#
# Writes $SDC_RUNS_DIR/<name>/ (default runs/): store/IDEA-1/ (the case tree, the layout of
# runs/full-cycle-1/store/), run.log (every sdc-drive command and its output), llm-calls.jsonl (one
# line per LLM call: adapter, model, tokens, cost, duration, allowed operations, tool calls),
# llm/NNN-<action>/ (prompt, frame, argv, event stream, transcript of each call),
# events/<case>.jsonl, and at the end decision.json and replay.json. With a decider also
# decisions.jsonl (one line per decider decision) and decider/NNN-<id>/ (each LLM decider call).
# run-settings.jsonl: one line per start or resume (sdc.run-settings/1: the decider's kind, model,
# escalate mode and call cap, the adapter, the LLM call cap, the calls made before the leg).
# fs/ is the file-change record of sdc-watch (scripts/watch-run.sh), running while this script
# runs and started again on a resume; SDC_WATCH=0 skips it.
#
# Limits: SDC_MAX_LLM_CALLS (default 30, over the whole tree), SDC_MAX_WALL_SECONDS per run
# (default 5400). Model: the bindings' `llm.model` (claude-haiku-5-5), or SDC_LLM_MODEL.
#
# Decider (SDC_DECIDER): who takes the decisions that wait on a person: the pending approvals, the
# open scope-changing interview questions, and closing the root.
#   person (default)  stop and print the commands (exit 3), as before
#   approve           rule: approve every approval, accept every question's default, close
#   reject            rule: refuse; the refusal is recorded and the run stops (exit 6)
#   haiku             tools/decider.sh: one bounded LLM call per decision (SDC_DECIDER_MODEL,
#                     default claude-haiku-5-5, through SDC_LLM_ADAPTER); escalate or an invalid
#                     reply stops for a person (exit 3) as today; reject stops (exit 6)
# SDC_DECIDER_ESCALATE=final (eval runs, no person at all): an escalate is recorded as
# `escalation_suppressed` and the decider's `best` decision is taken; an invalid reply has no best
# and counts as reject. SDC_MAX_DECIDER_CALLS (default 20) caps the LLM decider calls (exit 5).
# Every decider decision is taken as principal `decider:<kind>`; questions get
# `answered_by: decider:<kind>`.
#
# Exit: 0 terminable or closed, 3 waiting on a person, 4 stuck or a driver cap, 5 decider budget,
# 6 refused by the decider.
set -euo pipefail
# --goals <file>, anywhere among the arguments: the requester's goals document (sdc.goals/1, or what
# `sdc-goals resolve` printed), copied unchanged to the root case's work/goals.yaml when the run
# starts (artifact `goals` of sdc; intake and direction read it through the shared work/). A
# resume keeps the file in work/; a person edits it there.
goals="" rest=()
while (($#)); do
  if [[ $1 == --goals ]]; then
    goals=${2:-}
    [[ -f $goals ]] || { echo "--goals needs a goals file, not '$goals'"; exit 64; }
    shift 2
  else
    rest+=("$1"); shift
  fi
done
set -- ${rest[@]+"${rest[@]}"}

sdc=$(cd "$(dirname "$0")/.." && pwd)
dry=no
[[ ${1:-} == --dry ]] && { dry=yes; shift; }
name=${1:?usage: run-trial.sh [--dry] <name> [idea text]}
idea_text=${2:-}
canon=${CANON:-$HOME/beyond10x/canon/target/debug/canon}
drive=${DRIVE:-$sdc/driver/target/debug/sdc-drive}
ex=$sdc/driver/examples/sdc
out=${SDC_RUNS_DIR:-$sdc/runs}/$name
store=$out/store
root=$store/IDEA-1
log=$out/run.log
max_llm=${SDC_MAX_LLM_CALLS:-30}
max_wall=${SDC_MAX_WALL_SECONDS:-5400}
decider=${SDC_DECIDER:-person}
escalate_mode=${SDC_DECIDER_ESCALATE:-stop}
max_decider=${SDC_MAX_DECIDER_CALLS:-20}
case $decider in person|approve|reject|haiku) ;; *) echo "SDC_DECIDER: person, approve, reject or haiku, not $decider"; exit 64 ;; esac

export SDC_LLM_RECORD=$out/llm SDC_LLM_LOG=$out/llm-calls.jsonl
if [[ $dry == yes ]]; then
  export SDC_LLM_ADAPTER=dry SDC_LLM_DRY_LOG=$out/dry-commands.txt SDC_PRINCIPAL=dry-run
else
  export SDC_LLM_ADAPTER=${SDC_LLM_ADAPTER:-metaharness}
  [[ $SDC_LLM_ADAPTER == dry ]] && export SDC_LLM_DRY_LOG=${SDC_LLM_DRY_LOG:-$out/dry-commands.txt}
fi
now() { date -u +%Y-%m-%dT%H:%M:%SZ; }

# The file-change recorder (scripts/watch-run.sh, docs/fs-changes.md) runs while this script
# does: started before the first driver call, stopped on every exit. SDC_WATCH=0 skips it; any
# other non-empty SDC_WATCH that is not an executable stays watch-run.sh's default binary.
watching=no
watch_start() {
  [[ ${SDC_WATCH:-} == 0 ]] && return 0
  local bin=${SDC_WATCH:-}
  [[ -x $bin ]] || bin=
  SDC_WATCH=$bin "$sdc/scripts/watch-run.sh" start "$out" && watching=yes ||
    echo "warning: sdc-watch did not start; the run goes on without fs/changes.jsonl"
}
watch_stop() {
  [[ $watching == yes ]] || return 0
  watching=no
  local bin=${SDC_WATCH:-}
  [[ -x $bin ]] || bin=
  SDC_WATCH=$bin "$sdc/scripts/watch-run.sh" stop "$out" || true
}
trap watch_stop EXIT
trap 'exit 130' INT
trap 'exit 143' TERM HUP

# Run sdc-drive, log the command and its output, keep the output in $last and the code in $code.
d() {
  local dir=$1; shift
  echo "\$ sdc-drive --case-dir ${dir#"$store"/} $*" >> "$log"
  set +e
  last=$("$drive" --canon "$canon" --case-dir "$dir" "$@" 2>&1)
  code=$?
  set -e
  echo "$last" >> "$log"
  echo "exit $code" >> "$log"
  echo >> "$log"
}

# The agent-node actions of the happy path: every binding with `llm`; the story's run once per story.
expected() {
  local fixed per_story
  fixed=$(cat "$ex"/sdc-{intake,direction,spec,plan}.bindings.json | jq -s '[.[] | to_entries[] | select(.value.llm != null)] | length')
  per_story=$(jq '[to_entries[] | select(.value.llm != null)] | length' "$ex/sdc-story.bindings.json")
  echo "expected LLM calls on the happy path: $fixed (intake 4, direction 3, spec 1, plan 1) + $per_story per story (plan.decompose writes 1 to 3) = $((fixed + per_story)) to $((fixed + 3 * per_story)); retries raise it, capped at $max_llm over the tree"
}

if [[ ! -d $root ]]; then
  rm -rf "$out"
  mkdir -p "$out/events"
  watch_start
  : > "$log"
  expected | tee "$out/expected-llm-calls.txt"
  d "$root" init --protocol "$sdc/protocol/sdc.protocol.yaml" --id IDEA-1 --bindings "$ex/sdc.bindings.json"
  [[ $code == 0 ]] || { echo "init failed: $last"; exit 1; }
  if [[ $dry == yes ]]; then
    cp "$ex/fixtures/idea.yaml" "$root/work/idea.yaml"
  else
    : "${idea_text:?a new real trial needs the idea text}"
    jq -n --arg t "$idea_text" '{text: $t}' | yq -P > "$root/work/idea.yaml"
  fi
  [[ -z $goals ]] || cp "$goals" "$root/work/goals.yaml"
  echo "# the person's idea: $(cat "$root/work/idea.yaml")" >> "$log"
  leg=started
else
  watch_start
  leg=resumed
fi

exec > >(tee -a "$out/console.txt") 2>&1

# Who holds the decisions in this leg, and its caps: one line per start or resume (the UI reads it).
model=
[[ $decider == haiku ]] && model=${SDC_DECIDER_MODEL:-claude-haiku-5-5}
jq -cn --arg at "$(now)" --arg leg "$leg" --arg kind "$decider" --arg model "$model" --arg esc "$escalate_mode" \
  --argjson llm "$max_llm" --argjson dec "$max_decider" --argjson wall "$max_wall" --arg adapter "$SDC_LLM_ADAPTER" \
  --argjson calls "$(cat "$SDC_LLM_LOG" 2>/dev/null | wc -l)" '{format: "sdc.run-settings/1", at: $at, leg: $leg,
    decider: {kind: $kind, model: (if $model == "" then null else $model end), escalate: $esc,
      max_decider_calls: $dec}, adapter: $adapter, max_llm_calls: $llm, max_wall_seconds: $wall,
    llm_calls_before: $calls}' >> "$out/run-settings.jsonl"
if [[ $leg == started ]]; then
  echo "started $name: adapter $SDC_LLM_ADAPTER, decider $decider${model:+ ($model)}, escalate $escalate_mode, LLM call cap $max_llm, idea: $(yq -r .text "$root/work/idea.yaml")"
else
  echo "resuming $name: adapter $SDC_LLM_ADAPTER, decider $decider${model:+ ($model)}, escalate $escalate_mode, LLM call cap $max_llm, $(cat "$SDC_LLM_LOG" 2>/dev/null | wc -l) LLM calls so far"
fi

save() {
  while IFS= read -r c; do
    cp "$c/events.jsonl" "$out/events/$(jq -r .id "$c/case.json").jsonl"
  done < <(find "$store" -name case.json -printf '%h\n' | sort)
}
trap 'save; watch_stop' EXIT

dry_lines() { { wc -l < "${SDC_LLM_DRY_LOG:-/dev/null}"; } 2>/dev/null || echo 0; }
print_dry() { [[ -n ${SDC_LLM_DRY_LOG:-} && -f $SDC_LLM_DRY_LOG ]] && tail -n +"$(($1 + 1))" "$SDC_LLM_DRY_LOG" | sed 's/^/  /'; true; }

# Close the root through an outcome, write decision.json and replay.json; exit with replay's code.
finish() {
  d "$root" close --at "$(now)" --outcome "$1"
  echo "closed IDEA-1: $(jq -c '.terminated' <<< "$last") (exit $code)"
  [[ $code == 0 ]] || exit 4
  "$canon" evaluate --ir "$root/ir.json" --case "$root/case.json" --evidence "$root/evidence" \
    --authority "$root/authority.json" --decisions "$root/decisions.json" --at "$(now)" > "$out/decision.json"
  d "$root" replay
  echo "$last" > "$out/replay.json"
  echo "replay exit $code: $(jq -c 'map({case, state_matches_log, decision_matches_log})' "$out/replay.json")"
  exit "$code"
}

waiting() { jq -r '.message' <<< "$last"; echo "then run again: scripts/run-trial.sh $name"; exit 3; }

# One decider decision. Arguments: a request JSON (type, case_dir, action, capability, subject,
# path, revision, question, outcome). Sets $dec (the effective decision: approve or reject; empty
# when it waits on a person) and $ans (a question's answer); appends to decisions.jsonl.
decide() {
  local req=$1 key rec n reply line before
  key=$(jq -r '[(.case_dir | split("/") | last), .action, (.question.id // ""), .revision] | join("|")' <<< "$req")
  dec="" ans=""
  if [[ $escalate_mode != final && -f $out/decisions.jsonl ]] &&
     jq -e --arg k "$key" --arg p "decider:$decider" 'select(.principal == $p and .decision == "escalate"
       and ([.case, .action, (.question // ""), .revision] | join("|")) == $k)' "$out/decisions.jsonl" > /dev/null 2>&1; then
    echo "  decider:$decider escalated $key before: a person decides"
    return 0
  fi
  if [[ $decider == haiku ]]; then
    n=$(( $(find "$out/decider" -mindepth 1 -maxdepth 1 -type d 2>/dev/null | wc -l) + 1 ))
    (( n <= max_decider )) || { echo "decider budget spent: $max_decider LLM decider calls (SDC_MAX_DECIDER_CALLS)"; exit 5; }
    rec=$out/decider/$(printf '%03d' "$n")-$(jq -r '.question.id // .action' <<< "$req")
  else
    rec=$out/decider/rule
  fi
  mkdir -p "$rec"
  jq --arg rec "$rec" '. + {record_dir: $rec}' <<< "$req" > "$rec/decide-request.json"
  before=$(dry_lines)
  reply=$("$ex/tools/decider.sh" "$decider" "$rec/decide-request.json")
  print_dry "$before"
  line() { # $1 decision, $2 extra object
    local x=${2:-}
    [[ -n $x ]] || x='{}'
    jq -cn --arg at "$(now)" --arg p "decider:$decider" --arg d "$1" --argjson q "$req" --argjson r "$reply" --argjson x "$x" '{
      time: $at, case: ($q.case_dir | split("/") | last), type: $q.type, action: $q.action,
      question: ($q.question.id // null), capability: $q.capability, outcome: $q.outcome,
      revision: $q.revision, decision: $d, answer: $r.answer, principal: $p, reasons: $r.reasons,
      cites: $r.cites, model: $r.model, adapter: $r.adapter, tokens: $r.usage, cost_usd: $r.cost_usd,
      duration_ms: $r.duration_ms, valid: $r.valid, invalid_reason: $r.invalid_reason,
      dry: ($r.dry // false), record: $r.record} + $x' >> "$out/decisions.jsonl"
  }
  dec=$(jq -r .decision <<< "$reply")
  ans=$(jq -r '.answer // ""' <<< "$reply")
  if [[ $dec == escalate && $escalate_mode == final ]]; then
    line escalation_suppressed
    dec=$(jq -r '.best // "reject"' <<< "$reply")
    reply=$(jq -c '.reasons = ["escalation suppressed (SDC_DECIDER_ESCALATE=final); best decision taken"] + .reasons
      | .usage = null | .cost_usd = null' <<< "$reply")
    line "$dec" '{"suppressed": true}'
  else
    line "$dec"
  fi
  echo "  decider:$decider $dec $key$( [[ -n $ans ]] && echo " answer: $ans"): $(jq -r '.reasons | join(" ")' <<< "$reply")"
  case $dec in
    approve) ;;
    reject) echo "refused by decider:$decider (recorded in $out/decisions.jsonl); a person decides how to go on"; exit 6 ;;
    *) dec="" ;;
  esac
}

# Every pending decision of the stop in $last, through the decider. Returns 0 when all were
# taken; exits on a refusal; returns 1 when one waits on a person.
declare -A taken
decide_all() {
  local cdir pos n act cap subj path rev q key
  cdir=$(jq -r .case_dir <<< "$last")
  pos=$(jq -c .position <<< "$last")
  while IFS= read -r n; do
    act=$(jq -r .action <<< "$n")
    if jq -e '.command | test(" run --at ")' <<< "$n" > /dev/null; then
      # A handoff: the open scope-changing questions of the interview.
      local qf=$cdir/work/questions.yaml found=no
      rev=$(jq -r '.artifacts.questions.revision // "none"' "$cdir/case.json")
      local qs=()
      mapfile -t qs < <(yq -o=json -I=0 '.questions[] | select(.scope_changing == true and .status == "open")' "$qf" 2>/dev/null)
      for q in "${qs[@]}"; do
        [[ -n $q ]] || continue
        found=yes
        decide "$(jq -cn --arg c "$cdir" --arg a "$act" --arg rev "$rev" --argjson q "$q" --argjson pos "$pos" \
          '{type: "question", case_dir: $c, action: $a, capability: null, subject: "questions",
            path: "questions.yaml", revision: $rev, question: $q, outcome: null, position: $pos}')"
        [[ -n $dec ]] || return 1
        local st=changed
        [[ $ans == "$(jq -r .default <<< "$q")" ]] && st=accepted
        ANS=$ans yq -i "(.questions[] | select(.id == \"$(jq -r .id <<< "$q")\")) |= (.answer = strenv(ANS)
          | .status = \"$st\" | .answered_by = \"decider:$decider\")" "$qf"
      done
      [[ $found == yes ]] || { echo "nothing for decider:$decider at $act: no open scope-changing question (questions.check may refuse a non-human answer)"; return 1; }
      continue
    fi
    cap=$(jq -r '.command | capture("--capability (?<c>[^ ]+)").c' <<< "$n")
    subj=$(jq -r --arg a "$act" '.[$a].subject // ""' "$cdir/bindings.json")
    path=$(jq -r --arg s "$subj" '."$artifacts"[$s] // ""' "$cdir/bindings.json")
    rev=$(jq -r --arg s "$subj" '.artifacts[$s].revision // "none"' "$cdir/case.json")
    key="$cdir|$act|$rev"
    [[ -z ${taken[$key]:-} ]] || { echo "pending again after decider:$decider approved it: $act at $rev in $cdir"; exit 4; }
    decide "$(jq -cn --arg c "$cdir" --arg a "$act" --arg cap "$cap" --arg s "$subj" --arg p "$path" --arg rev "$rev" \
      --argjson pos "$pos" '{type: "approval", case_dir: $c, action: $a, capability: $cap, subject: $s,
        path: $p, revision: $rev, question: null, outcome: null, position: $pos}')"
    [[ -n $dec ]] || return 1
    taken[$key]=1
    if jq -e '.command | test("--action ")' <<< "$n" > /dev/null; then
      d "$cdir" approve --at "$(now)" --capability "$cap" --principal "decider:$decider" --action "$act"
    else
      d "$cdir" approve --at "$(now)" --capability "$cap" --principal "decider:$decider"
    fi
    echo "  approve $act as decider:$decider: exit $code"
    [[ $code == 0 ]] || { jq -r '.message // .' <<< "$last" 2>/dev/null || echo "$last"; exit 4; }
  done < <(jq -c '.next[]' <<< "$last")
}

for round in $(seq 1 40); do
  dry_before=$(dry_lines)
  t0=$(date +%s) s0=$(now)
  d "$root" run --at "$s0" --max-llm-calls "$max_llm" --max-wall-seconds "$max_wall"
  verdict=$(jq -r '.verdict // "error"' <<< "$last" 2>/dev/null || echo error)
  # One line per round for the KPI L2 (autonomous run time between human interventions).
  jq -cn --arg s "$s0" --arg e "$(now)" --argjson t0 "$t0" --argjson t1 "$(date +%s)" --arg v "$verdict" \
    --arg c "$(jq -r '.case_dir // ""' <<< "$last" 2>/dev/null | sed "s|$store/||")" '{started: $s, ended: $e,
    started_epoch: $t0, ended_epoch: $t1, seconds: ($t1 - $t0), verdict: $v, case_dir: $c}' >> "$out/rounds.jsonl"
  echo "round $round: $verdict $(jq -r '.case_dir // ""' <<< "$last" 2>/dev/null | sed "s|$store/||"), LLM calls so far $(jq -r '.llm_calls // "?"' <<< "$last" 2>/dev/null)"
  [[ $dry == yes || $SDC_LLM_ADAPTER == dry ]] && print_dry "$dry_before"
  case $verdict in
    terminable)
      outcome=$(jq -r '.position.legitimate_outcomes[0]' <<< "$last")
      [[ $dry == yes && $decider == person ]] && finish "$outcome"
      if [[ $decider != person ]]; then
        decide "$(jq -cn --arg c "$root" --arg o "$outcome" --argjson pos "$(jq -c .position <<< "$last")" \
          --arg rev "$(jq -r '.revision' "$root/case.json")" \
          '{type: "close", case_dir: $c, action: "close", capability: null, subject: null, path: null,
            revision: $rev, question: null, outcome: $o, position: $pos}')"
        [[ -n $dec ]] && finish "$outcome"
      fi
      echo "the idea is operating: close the root case through \`$outcome\` (your decision):"
      echo "  $drive --canon $canon --case-dir $root close --at \"\$(date -u +%Y-%m-%dT%H:%M:%SZ)\" --outcome $outcome"
      exit 0 ;;
    awaiting_authority|awaiting_person)
      if [[ $decider != person ]]; then
        decide_all || waiting
        continue
      fi
      [[ $dry == no ]] && waiting
      # Dry run: the script plays the person (principal dry-run), one decision per round.
      n=$(jq -c '.next[0]' <<< "$last")
      [[ $n != null ]] || { echo "nothing to decide: $(jq -r .message <<< "$last")"; exit 1; }
      cmd=$(jq -r '.command' <<< "$n" | sed "s|\"\$(date -u +%Y-%m-%dT%H:%M:%SZ)\"|$(now)|")
      read -ra argv <<< "$cmd"
      echo "\$ ${cmd}" >> "$log"
      set +e; res=$("${argv[@]}" 2>&1); code=$?; set -e
      echo "$res" >> "$log"; echo "exit $code" >> "$log"; echo >> "$log"
      echo "  dry-run decided $(jq -r .action <<< "$n"): exit $code"
      [[ $code == 0 ]] || exit 1 ;;
    *)
      jq -r '.message // .' <<< "$last" 2>/dev/null || echo "$last"
      exit 4 ;;
  esac
done
echo "40 rounds without an end"
exit 4
