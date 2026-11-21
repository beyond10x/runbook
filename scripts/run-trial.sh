#!/usr/bin/env bash
# Run one SDC trial with the real LLM executor, from an idea to the root outcome, and stop at
# every step that is a person's decision. Run it again to resume after that decision.
#
#   scripts/run-trial.sh <name> [idea text]     real run (adapter $SDC_LLM_ADAPTER, default metaharness)
#   scripts/run-trial.sh --dry <name>           dry run: the stub fixture idea, the stubs write the
#                                               artifacts, every LLM command is printed and none is
#                                               run, and the person's approvals are scripted
#   scripts/run-trial.sh <name> --stop "<reason>"  stop a run that will not go on: writes
#                                               run-stopped.json (sdc.run-stopped/1: format, at,
#                                               by = SDC_PRINCIPAL, reason, verdict_at_stop, case)
#                                               and a run.log line; nothing else is touched
#   scripts/run-trial.sh <name> --reopen        resume a stopped run: the record is renamed
#                                               run-stopped.<at>.json; without it a resume of a
#                                               stopped run is refused (exit 8)
#   scripts/run-trial.sh <name> --bugfix <seed dir> ["<bug report>"]  one bug fix instead of an
#                                               idea (docs/bugfix.md): root BUG-1 on sdc.bugfix, work/
#                                               seeded by `sdc-bugfix seed` from the crate (<seed dir>
#                                               or <seed dir>/crate) and the report (the text, else
#                                               <seed dir>/bug.yaml); with --dry the stubs write
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
# (default 5400). Model per node class (tools/node-classes.json): SDC_LLM_MODEL_WRITER (spec.author,
# plan.decompose), SDC_LLM_MODEL_IMPLEMENT (change.implement), SDC_LLM_MODEL_REVIEW (spec.review,
# change.review: no LLM yet), SDC_LLM_MODEL_OTHER (every other node), else SDC_LLM_MODEL for all
# nodes, else each binding's `llm.model` (claude-haiku-5-5); the decider SDC_LLM_MODEL_DECIDER,
# else SDC_DECIDER_MODEL. run-settings.jsonl `models` records the effective model per class.
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
# `answered_by: decider:<kind>`. At `scope.answer` each open scope-changing question is put to the
# decider on its own (its text, options, default, dissection item and idea excerpt); the answers
# are written as a person's are, then `scope.answer` is taken as `decider:<kind>`; under
# SDC_DECIDER_ESCALATE=final an escalated question takes its default. Each line of
# decisions.jsonl names its `class`: scope_changing_answer, finding, approval or close.
# At amend.upstream's handoff each finding waiting for a decision goes to the decider (its text,
# options, default, cites and what it touches); the answer is written as `decision:` with
# `decided_by: decider:<kind>` and amend.upstream applies it; under final, `best`, else the
# default. A decision the program check refuses (`refused_decision`) waits for a person.
#
# Exit: 0 terminable or closed, 3 waiting on a person, 4 stuck or a driver cap, 5 decider budget,
# 6 refused by the decider (rejected_without_rework_path), 7 autopilot ended at a stop no holder of the run can take, 8 the run is stopped
# (or already stopped), 64 usage.
#
# Run mode: SDC_AUTONOMY=A0..A5, or `--autopilot` (A5, the eval's mode): the decider (haiku unless
# SDC_DECIDER names another; SDC_DECIDER=person is refused) takes every decision, is final, and no
# stop waits on a person: one that would need a person ends the run (exit 7), and every run that
# ends unclosed gets a stop record by rule:autopilot. A0 is SDC_DECIDER=person. A1 to A4 have no
# per-class routing yet and run as SDC_DECIDER says. The level goes into run-settings.jsonl
# (`autonomy`, `autonomy_source`). SDC_STAND_IN=<who> (and --dry's scripted person) labels a run in
# which a script or a session stands in for the person (`stand_in: true`, `stand_in_by`).
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
dry=no stop_reason="" reopen=no autopilot=no pos=()
while (($#)); do
  case $1 in
    --dry) dry=yes ;;
    --autopilot) autopilot=yes ;;
    --stop) stop_reason=${2:-}; [[ -n $stop_reason ]] || { echo "--stop needs a reason"; exit 64; }; shift ;;
    --reopen) reopen=yes ;;
    --bugfix) bugfix=${2:-}; [[ -d $bugfix ]] || { echo "--bugfix needs a seed directory, not '$bugfix'"; exit 64; }; bugfix=$(cd "$bugfix" && pwd); shift ;;
    *) pos+=("$1") ;;
  esac
  shift
done
name=${pos[0]:?usage: run-trial.sh [--dry] <name> [idea text] | <name> --stop "<reason>" | <name> --reopen}
idea_text=${pos[1]:-}
canon=${CANON:-$HOME/beyond10x/canon/target/debug/canon}
drive=${DRIVE:-$sdc/driver/target/debug/sdc-drive}
reply_bin=${SDC_REPLY:-$sdc/driver/target/debug/sdc-reply}
ex=$sdc/driver/examples/sdc
out=${SDC_RUNS_DIR:-$sdc/runs}/$name
store=$out/store
root=$store/IDEA-1
[[ -n ${bugfix:-} || -d $store/BUG-1 ]] && root=$store/BUG-1
log=$out/run.log
max_llm=${SDC_MAX_LLM_CALLS:-30}
max_wall=${SDC_MAX_WALL_SECONDS:-5400}
decider=${SDC_DECIDER:-person}
escalate_mode=${SDC_DECIDER_ESCALATE:-stop}
max_decider=${SDC_MAX_DECIDER_CALLS:-20}
now() { date -u +%Y-%m-%dT%H:%M:%SZ; }

# The stop record (sdc.run-stopped/1): a run that will not go on is stopped with a reason. Only
# run-stopped.json and a run.log line are written; the driver and the event logs are not touched.
stopped=$out/run-stopped.json
write_stop() { # <principal> <reason> [cause]: cause, machine-readable, when a rule stops the run
  local at
  at=$(now)
  jq -n --arg at "$at" --arg by "$1" --arg why "$2" --arg cause "${3:-}" \
    --argjson r "$(tail -n 1 "$out/rounds.jsonl" 2>/dev/null | jq -c . 2>/dev/null || echo null)" \
    '{format: "sdc.run-stopped/1", at: $at, by: $by, reason: $why, verdict_at_stop: ($r.verdict // null),
      case: (($r.case_dir // "") | split("/") | last | if . == "" or . == null then null else . end)}
      + (if $cause == "" then {} else {cause: $cause} end)' > "$stopped.tmp"
  mv "$stopped.tmp" "$stopped"
  echo "# stopped at $at by $1: $2" >> "$log"
  echo "stopped $name: $(jq -c . "$stopped")"
}
if [[ -n $stop_reason ]]; then
  [[ -d $root ]] || { echo "no run $name to stop ($out)"; exit 64; }
  [[ ! -f $stopped ]] || { echo "$name is already stopped: $(jq -c . "$stopped")"; exit 8; }
  [[ -n ${SDC_PRINCIPAL:-} ]] || { echo "--stop needs the principal that stops the run in SDC_PRINCIPAL (person:<id>, agent:<name>)"; exit 64; }
  write_stop "$SDC_PRINCIPAL" "$stop_reason"
  exit 0
fi
if [[ -f $stopped ]]; then
  if [[ $reopen != yes ]]; then
    echo "$name was stopped at $(jq -r .at "$stopped") by $(jq -r .by "$stopped"): $(jq -r .reason "$stopped")"
    echo "to go on: scripts/run-trial.sh $name --reopen"
    exit 8
  fi
  was=$(jq -r .at "$stopped")
  mv "$stopped" "$out/run-stopped.$was.json"
  echo "# reopened at $(now) by ${SDC_PRINCIPAL:-an unnamed principal}: stopped at $was (run-stopped.$was.json)" >> "$log"
fi

case $decider in person|approve|reject|haiku) ;; *) echo "SDC_DECIDER: person, approve, reject or haiku, not $decider"; exit 64 ;; esac

# The run mode (docs/autonomy-levels.md § 3.3): SDC_AUTONOMY, or --autopilot for A5. A0 is the
# person (SDC_DECIDER=person); A5 is autopilot: the decider (haiku unless SDC_DECIDER names another)
# holds every decision and is final (escalate final), and a stop that would need a person ends the
# run (exit 7) with a stop record by rule:autopilot. A1 to A4 have no per-class routing yet: they
# run as SDC_DECIDER says. Without a level it is derived: person A0, a final decider A5, a
# decider that escalates to a person none (no level matches it).
autonomy=${SDC_AUTONOMY:-}
autonomy_source=SDC_AUTONOMY
[[ $autopilot == yes ]] && { [[ -z $autonomy || $autonomy == A5 ]] || { echo "--autopilot is A5, SDC_AUTONOMY says $autonomy"; exit 64; }; autonomy=A5 autonomy_source=--autopilot; }
case $autonomy in
  A5)
    [[ ${SDC_DECIDER:-} != person ]] || { echo "autopilot (A5) has no person: SDC_DECIDER=person is refused; name a decider (haiku, approve, reject) or leave it unset (haiku)"; exit 64; }
    [[ ${SDC_DECIDER_ESCALATE:-final} == final ]] || { echo "autopilot (A5): the decider is final; SDC_DECIDER_ESCALATE=$SDC_DECIDER_ESCALATE is refused"; exit 64; }
    # The decider reads SDC_DECIDER_ESCALATE: under final its prompt asks for `best` and its check
    # refuses an escalate without one.
    decider=${SDC_DECIDER:-haiku} escalate_mode=final
    export SDC_DECIDER_ESCALATE=final ;;
  A0)
    [[ $decider == person ]] || { echo "A0: every decision is the person's; SDC_DECIDER=$decider is refused"; exit 64; } ;;
  A1|A2|A3|A4) ;;
  "")
    autonomy_source=derived
    if [[ $decider == person ]]; then autonomy=A0
    elif [[ $escalate_mode == final ]]; then autonomy=A5
    fi ;;
  *) echo "SDC_AUTONOMY: A0 to A5, not $autonomy"; exit 64 ;;
esac
# A script or a session standing in for the person (SDC_STAND_IN=<who>; --dry's scripted person)
# makes the run an end-to-end test of the person path: labelled stand_in, its numbers no eval's.
stand_in=${SDC_STAND_IN:-}
[[ $dry == yes && $decider == person ]] && stand_in=${stand_in:-scripts/run-trial.sh --dry}

export SDC_LLM_RECORD=$out/llm SDC_LLM_LOG=$out/llm-calls.jsonl
if [[ $dry == yes ]]; then
  export SDC_LLM_ADAPTER=dry SDC_LLM_DRY_LOG=$out/dry-commands.txt SDC_PRINCIPAL=dry-run
else
  export SDC_LLM_ADAPTER=${SDC_LLM_ADAPTER:-metaharness}
  [[ $SDC_LLM_ADAPTER == dry ]] && export SDC_LLM_DRY_LOG=${SDC_LLM_DRY_LOG:-$out/dry-commands.txt}
fi

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

if [[ -n ${bugfix:-} && ! -d $root ]]; then
  # One bug fix (docs/bugfix.md): the seed program stands in for a parent handing in the report,
  # the code tree and the story.
  rm -rf "$out"
  mkdir -p "$out/events"
  watch_start
  : > "$log"
  d "$root" init --protocol "$sdc/protocol/sdc-bugfix.protocol.yaml" --id BUG-1 --bindings "$ex/sdc-bugfix.bindings.json"
  [[ $code == 0 ]] || { echo "init failed: $last"; exit 1; }
  seed=(--from "$bugfix")
  [[ -n $idea_text ]] && seed+=(--text "$idea_text" --story "story:bug-$name")
  (cd "$root/work" && "${SDC_BUGFIX:-$sdc/driver/target/debug/sdc-bugfix}" seed "${seed[@]}") >> "$log" 2>&1 ||
    { echo "seed failed: $(tail -n 3 "$log")"; exit 1; }
  echo "# the bug report: $(cat "$root/work/bug.yaml")" >> "$log"
  leg=started
elif [[ ! -d $root ]]; then
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
[[ $decider == haiku ]] && model=${SDC_LLM_MODEL_DECIDER:-${SDC_DECIDER_MODEL:-claude-haiku-5-5}}
# The effective model per node class (tools/node-classes.json): SDC_LLM_MODEL_<CLASS>, else
# SDC_LLM_MODEL, else each binding's llm.model (per node; `model` is null when they differ). The
# decider: SDC_LLM_MODEL_DECIDER, else SDC_DECIDER_MODEL, else claude-haiku-5-5.
models=$(jq -cn --slurpfile c "$ex/tools/node-classes.json" \
  --argjson b "$(jq -s 'map(to_entries[] | select(.value.llm != null) | {(.key): .value.llm.model}) | add' "$ex"/sdc-*.bindings.json)" \
  --arg all "${SDC_LLM_MODEL:-}" --arg writer "${SDC_LLM_MODEL_WRITER:-}" --arg implement "${SDC_LLM_MODEL_IMPLEMENT:-}" \
  --arg review "${SDC_LLM_MODEL_REVIEW:-}" --arg other "${SDC_LLM_MODEL_OTHER:-}" --arg dclass "${SDC_LLM_MODEL_DECIDER:-}" \
  --arg dmodel "${SDC_DECIDER_MODEL:-}" '
  {writer: $writer, implement: $implement, review: $review, other: $other} as $set
  | ($c[0].classes | del(.decider) | to_entries | map(.key as $k
      | (if $set[$k] != "" then {m: $set[$k], src: "SDC_LLM_MODEL_\($k | ascii_upcase)"}
         elif $all != "" then {m: $all, src: "SDC_LLM_MODEL"} else {m: null, src: "binding"} end) as $e
      | (.value | map({(.): ($e.m // $b[.])}) | add // {}) as $nodes
      | {(.key): {model: ($e.m // ([$nodes[]] | unique | if length == 1 then .[0] else null end)), source: $e.src,
          nodes: $nodes}}) | add)
  + {decider: (if $dclass != "" then {model: $dclass, source: "SDC_LLM_MODEL_DECIDER"}
      elif $dmodel != "" then {model: $dmodel, source: "SDC_DECIDER_MODEL"}
      else {model: "claude-haiku-5-5", source: "default"} end)}')
jq -cn --arg at "$(now)" --arg leg "$leg" --arg kind "$decider" --arg model "$model" --arg esc "$escalate_mode" \
  --argjson models "$models" \
  --argjson llm "$max_llm" --argjson dec "$max_decider" --argjson wall "$max_wall" --arg adapter "$SDC_LLM_ADAPTER" \
  --argjson calls "$(cat "$SDC_LLM_LOG" 2>/dev/null | wc -l)" --arg level "$autonomy" --arg lsrc "$autonomy_source" \
  --arg stand "$stand_in" --arg rq "${SDC_REQUESTER_DECIDER:-}" '{format: "sdc.run-settings/1", at: $at, leg: $leg,
    autonomy: (if $level == "" then null else $level end), autonomy_source: $lsrc,
    per_class_routing: (if ($level | IN("A1", "A2", "A3", "A4")) then "not built: every decision goes to the decider" else null end),
    requester_decider: (if $rq == "" then null else {kind: $rq, principal: "decider:requester", classes: "the requester side of decision-classes.json"} end),
    decider: {kind: $kind, model: (if $model == "" then null else $model end), escalate: $esc,
      max_decider_calls: $dec}, adapter: $adapter, max_llm_calls: $llm, max_wall_seconds: $wall,
    llm_calls_before: $calls, models: $models, stand_in: ($stand != ""), stand_in_by: (if $stand == "" then null else $stand end)}' >> "$out/run-settings.jsonl"
if [[ $leg == started ]]; then
  echo "started $name: adapter $SDC_LLM_ADAPTER, decider $decider${model:+ ($model)}, escalate $escalate_mode, LLM call cap $max_llm, $(yq -r '"idea: " + .text' "$root/work/idea.yaml" 2>/dev/null || yq -r '"bug: " + .text' "$root/work/bug.yaml")"
else
  echo "resuming $name: adapter $SDC_LLM_ADAPTER, decider $decider${model:+ ($model)}, escalate $escalate_mode, LLM call cap $max_llm, $(cat "$SDC_LLM_LOG" 2>/dev/null | wc -l) LLM calls so far"
fi

save() {
  while IFS= read -r c; do
    cp "$c/events.jsonl" "$out/events/$(jq -r .id "$c/case.json").jsonl"
  done < <(find "$store" -name case.json -printf '%h\n' | sort)
}
# Autopilot (A5): no stop waits on a person. A run that ends anywhere but closed (a stop that would
# need a person, a cap, stuck, a refusal) is stopped with a record by rule:autopilot.
# The record names the step (action, case as a run-relative id), the cause (end_cause, also the
# record's `cause`) and the fix it waits on; no absolute path. Causes: rejected_without_rework_path, rework_spent,
# decider_budget_spent, decision_not_takeable (a stop no holder of this run can take: the program
# check refused the decider's decision, or a handoff with nothing for the decider), and the
# driver's verdicts (stuck, writer_spent, writer_not_called, llm_budget_spent, step_budget_spent, wall_time_spent).
end_reason="" end_cause="" end_action="" end_case="" wait_why=""
autopilot_end() {
  local rc=$1 v
  [[ $autonomy == A5 && $rc != 0 && ! -f $stopped && -d $root ]] || return 0
  if [[ -z $end_cause ]]; then
    v=$(jq -r '.verdict // "error"' <<< "${last:-null}" 2>/dev/null || echo error)
    end_cause=$v
    end_case=$(jq -r '(.case_dir // "") | split("/") | last' <<< "${last:-null}" 2>/dev/null)
    end_action=$(jq -r '(.spent // .position.spent // [])[0].action // (.position.not_called // [])[0].action // .action // .next[0].action // ""' <<< "${last:-null}" 2>/dev/null)
    case $v in
      writer_spent) end_reason="$end_action in $end_case: writer_spent ($(jq -r '(.spent // .position.spent // [])[0] | "\(.runs) runs, last exit \(.last_exit)"' <<< "$last")): it waits on a code fix of the writer, then --reopen" ;;
      writer_not_called) end_reason="$end_action in $end_case: writer_not_called ($(jq -r '(.position.not_called // [])[0].reason // ""' <<< "$last" | cut -c1-200)): nothing else is useful; it waits on a fix of the writer or a decision, then --reopen" ;;
      stuck) end_reason="${end_case:-the run}: stuck, no action is admissible and useful: it waits on a code or protocol fix, then --reopen" ;;
      *_budget_spent|wall_time_spent) end_reason="${end_action:+$end_action in }${end_case:-the run}: $v: it waits on a higher cap, then --reopen" ;;
      *) end_reason="${end_case:-the run}: the run ended with exit $rc ($v): it waits on a fix, then --reopen" ;;
    esac
  fi
  write_stop rule:autopilot "autopilot: $end_reason" "$end_cause" || true
}
trap 'rc=$?; autopilot_end "$rc"; save; watch_stop' EXIT

dry_lines() { { wc -l < "${SDC_LLM_DRY_LOG:-/dev/null}"; } 2>/dev/null || echo 0; }
print_dry() { [[ -n ${SDC_LLM_DRY_LOG:-} && -f $SDC_LLM_DRY_LOG ]] && tail -n +"$(($1 + 1))" "$SDC_LLM_DRY_LOG" | sed 's/^/  /'; true; }

# Close the root through an outcome, write decision.json, status.json and replay.json; exit with
# replay's code. A run that closes completed, whichever outcome: status.json (sdc.run-status/1)
# says closed, the outcome and whether it is the delivering one (`operating`), with end_note (why
# the tree ended otherwise, e.g. a withheld release and the decider's first reason).
end_note=""
finish() { # <outcome> [principal that closed it]
  if [[ -n ${2:-} ]]; then d "$root" close --at "$(now)" --outcome "$1" --principal "$2"; else d "$root" close --at "$(now)" --outcome "$1"; fi
  echo "closed ${root##*/}: $(jq -c '.terminated' <<< "$last")${end_note:+ ($end_note)}${2:+ by $2} (exit $code)"
  [[ $code == 0 ]] || exit 4
  jq -n --arg at "$(now)" --arg o "$1" --arg note "$end_note" --arg by "${2:-}" '{format: "sdc.run-status/1", at: $at, state: "closed",
    outcome: $o, delivered: ($o == "operating" or $o == "fixed"), note: (if $note == "" then null else $note end),
    closed_by: (if $by == "" then null else $by end)}' > "$out/status.json"
  "$canon" evaluate --ir "$root/ir.json" --case "$root/case.json" --evidence "$root/evidence" \
    --authority "$root/authority.json" --decisions "$root/decisions.json" --at "$(now)" > "$out/decision.json"
  d "$root" replay
  echo "$last" > "$out/replay.json"
  echo "replay exit $code: $(jq -c 'map({case, state_matches_log, decision_matches_log})' "$out/replay.json")"
  exit "$code"
}

waiting() {
  if [[ $autonomy == A5 ]]; then
    end_cause=decision_not_takeable
    end_case=$(jq -r '(.case_dir // "") | split("/") | last' <<< "$last")
    end_action=$(jq -r '.next[0].action // ""' <<< "$last")
    end_reason="$end_action in $end_case: decision_not_takeable: ${wait_why:-the decider of this run cannot take it}; it waits on a fix of that decision path, then --reopen"
    echo "$end_reason"
    exit 7
  fi
  jq -r '.message' <<< "$last"; echo "then run again: scripts/run-trial.sh $name"; exit 3
}

# One decider decision. Arguments: a request JSON (type, case_dir, action, capability, subject,
# path, revision, question, outcome). Sets $dec (the effective decision: approve or reject; empty
# when it waits on a person) and $ans (a question's answer); appends to decisions.jsonl.
# An invalid reply (not JSON, a cite that does not resolve, a wrong shape) is asked again with the
# check's reason, at most 2 retries, each a decider call of its own (counted against
# SDC_MAX_DECIDER_CALLS, recorded in decider/ and as `invalid_reply` in decisions.jsonl). Still
# invalid: with a person (escalate stop) it waits on the person; under final the fallback is
# taken, recorded as `fallback`: a question or a finding takes its default (invalid_reply_default);
# an approval or a close is approved when every claim it turns on is true and no obligation is
# open but the one it discharges (approved_on_checks), else rejected with the failing checks
# (rejected_on_checks). A reject at an approval: under A5 reworked by its writer (rework), else ended
# by the protocol's end for a refusal (withhold), else the run ends rejected_without_rework_path.
dec_principal=""
class_of() { # <request>: its sdc.decision-classes/1 class (a finding has none of its own: finding)
  jq -r --slurpfile c "$ex/decision-classes.json" 'if .type == "question" then "scope_changing_answer"
    elif .type == "finding" then "finding" elif .type == "close" then "close"
    else .action as $a | ([$c[0].classes | to_entries[] | select(any(.value.actions[]?; endswith("/" + $a))) | .key] | first // "approval") end' <<< "$1"
}
decide() {
  local req=$1 key rec n reply before attempt=1 why="" kind fallback="" cls side
  key=$(jq -r '[(.case_dir | split("/") | last), .action, (.question.id // .finding.id // ""), .revision] | join("|")' <<< "$req")
  kind=$(jq -r .type <<< "$req")
  dec="" ans=""
  # The decision class and its side (decision-classes.json). With SDC_REQUESTER_DECIDER set, a
  # requester-side class goes to the requester agent (decider.sh requester, principal
  # decider:requester); the delivering side keeps SDC_DECIDER. `decider` is local here, so what
  # decide calls (rework, withhold) names the principal that decided.
  cls=$(class_of "$req")
  side=$(jq -r --arg c "$cls" '.classes[$c].side // "delivering"' "$ex/decision-classes.json")
  local decider=$decider
  [[ -n ${SDC_REQUESTER_DECIDER:-} && $side == requester ]] && decider=requester
  dec_principal=decider:$decider
  local llm=no
  [[ $decider == haiku || $decider == requester ]] && llm=yes
  req=$(jq -c --arg c "$cls" --arg s "$side" --arg a "$autonomy" --arg l "$out/decisions.jsonl" '. + {class: $c, side: $s,
    autonomy: (if $a == "" then null else $a end), decisions_log: $l}' <<< "$req")
  if [[ $escalate_mode != final && -f $out/decisions.jsonl ]] &&
     jq -e --arg k "$key" --arg p "decider:$decider" 'select(.principal == $p and .decision == "escalate"
       and ([.case, .action, (.question // .finding // ""), .revision] | join("|")) == $k)' "$out/decisions.jsonl" > /dev/null 2>&1; then
    echo "  decider:$decider escalated $key before: a person decides"
    return 0
  fi
  line() { # $1 decision, $2 extra object
    local x=${2:-}
    [[ -n $x ]] || x='{}'
    jq -cn --arg at "$(now)" --arg p "decider:$decider" --arg d "$1" --argjson q "$req" --argjson r "$reply" --argjson x "$x" '{
      time: $at, case: ($q.case_dir | split("/") | last), type: $q.type,
      class: ({question: "scope_changing_answer", finding: "finding"}[$q.type] // $q.type), action: $q.action,
      question: ($q.question.id // null), finding: ($q.finding.id // null), capability: $q.capability, outcome: $q.outcome,
      revision: $q.revision, decision: $d, answer: $r.answer, principal: $p, reasons: $r.reasons,
      cites: $r.cites, citations: ($r.citations // null), model: $r.model, adapter: $r.adapter, tokens: $r.usage,
      cost_usd: $r.cost_usd, duration_ms: $r.duration_ms, valid: $r.valid, invalid_reason: $r.invalid_reason,
      schema_constrained: ($r.schema_constrained // false), dry: ($r.dry // false), record: $r.record, side: ($q.side // null),
      no_ground_in_goals: $r.no_ground_in_goals, strength: ($r.strength // null), grounds: ($r.grounds // null)} + $x' >> "$out/decisions.jsonl"
  }
  while :; do
    if [[ $llm == yes ]]; then
      n=$(( $(find "$out/decider" -mindepth 1 -maxdepth 1 -type d 2>/dev/null | wc -l) + 1 ))
      (( n <= max_decider )) || { end_cause=decider_budget_spent end_action=$(jq -r .action <<< "$req") end_case=${key%%|*}
        echo "decider budget spent: $max_decider LLM decider calls (SDC_MAX_DECIDER_CALLS)"; exit 5; }
      rec=$out/decider/$(printf '%03d' "$n")-$(jq -r '.question.id // .finding.id // .action' <<< "$req")
    else
      rec=$out/decider/rule
    fi
    mkdir -p "$rec"
    jq --arg rec "$rec" --argjson a "$attempt" --arg why "$why" '. + {record_dir: $rec}
      + (if $a > 1 then {retry: {attempt: $a, invalid_reason: $why}} else {} end)' <<< "$req" > "$rec/decide-request.json"
    before=$(dry_lines)
    reply=$("$ex/tools/decider.sh" "$decider" "$rec/decide-request.json")
    print_dry "$before"
    [[ $(jq -r .valid <<< "$reply") == false && $llm == yes ]] || break
    why=$(jq -r '.invalid_reason // "invalid"' <<< "$reply")
    (( attempt <= 2 )) || break
    line invalid_reply "$(jq -cn --argjson a "$attempt" '{attempt: $a}')"
    echo "  decider:$decider reply refused by the check ($why): asked again ($attempt of 2 retries)"
    attempt=$((attempt + 1))
  done
  dec=$(jq -r .decision <<< "$reply")
  ans=$(jq -r '.answer // ""' <<< "$reply")
  if [[ $(jq -r .valid <<< "$reply") == false && $llm == yes && $escalate_mode == final ]]; then
    # Still invalid after the retries, and nobody else decides: the fallback of the decision kind.
    line invalid_reply "$(jq -cn --argjson a "$attempt" '{attempt: $a}')"
    case $kind in
      question|finding)
        fallback=invalid_reply_default dec=approve
        ans=$(jq -r 'if .type == "question" then .question.default else .finding.default end' <<< "$req")
        reply=$(jq -c --arg a "$ans" --arg why "$why" '.answer = $a | .valid = true
          | .reasons = ["invalid decider reply after 2 retries (\($why)): the default taken"]' <<< "$reply") ;;
      *)
        local failing
        failing=$(checks_failing "$req")
        if [[ $failing == "[]" ]]; then
          fallback=approved_on_checks dec=approve
          reply=$(jq -c --arg why "$why" '.valid = true | .reasons = ["invalid decider reply after 2 retries (\($why)): approved on the checks, every claim this decision turns on is true and no other obligation is open"]' <<< "$reply")
        else
          fallback=rejected_on_checks dec=reject
          reply=$(jq -c --arg why "$why" --argjson f "$failing" '.valid = true | .reasons = ["invalid decider reply after 2 retries (\($why)): rejected on the checks: \($f | join(", "))"]' <<< "$reply")
        fi ;;
    esac
    line "$dec" "$(jq -cn --arg f "$fallback" --argjson a "$attempt" '{fallback: $f, attempts: $a}')"
  elif [[ $dec == escalate && $escalate_mode == final ]]; then
    line escalation_suppressed
    if [[ $kind == question ]]; then
      # A scope-changing question takes its default.
      dec=approve ans=$(jq -r .question.default <<< "$req")
      reply=$(jq -c --arg a "$ans" '.answer = $a | .reasons = ["escalation suppressed (SDC_DECIDER_ESCALATE=final); the default taken"] + .reasons
        | .usage = null | .cost_usd = null' <<< "$reply")
    elif [[ $kind == finding ]]; then
      # A finding takes the decider's best answer, else its default.
      dec=approve ans=$(jq -r --argjson q "$req" '.best // $q.finding.default' <<< "$reply")
      reply=$(jq -c --arg a "$ans" '.answer = $a | .reasons = ["escalation suppressed (SDC_DECIDER_ESCALATE=final); "
        + (if .best then "best answer" else "the default" end) + " taken"] + .reasons | .usage = null | .cost_usd = null' <<< "$reply")
    else
      # A valid escalate under final carries `best` (decider.sh checks it).
      dec=$(jq -r '.best // "reject"' <<< "$reply")
      reply=$(jq -c '.reasons = ["escalation suppressed (SDC_DECIDER_ESCALATE=final); best decision taken"] + .reasons
        | .usage = null | .cost_usd = null' <<< "$reply")
    fi
    line "$dec" '{"suppressed": true}'
  else
    line "$dec" "$( (( attempt > 1 )) && jq -cn --argjson a "$attempt" '{attempts: $a}' || echo '{}')"
  fi
  echo "  decider:$decider $dec $key$( [[ -n $ans ]] && echo " answer: $ans")${fallback:+ ($fallback)}: $(jq -r '.reasons | join(" ")' <<< "$reply")"
  case $dec in
    approve) ;;
    reject)
      end_action=$(jq -r .action <<< "$req") end_case=${key%%|*}
      # Autopilot: a rejected approval is reworked (rework below), not an end.
      if [[ $autonomy == A5 && $kind == approval ]]; then
        rework "$req" "$reply" && { dec="" reworked=yes; return 0; }
        # No writer, but the protocol's end for a refusal (a withheld release).
        [[ -z $end_cause ]] && withhold "$req" "$reply" && { dec="" reworked=yes; return 0; }
      fi
      if [[ -z $end_cause ]]; then
        end_cause=rejected_without_rework_path
        end_reason="$end_action in $end_case: decider:$decider rejected $(jq -r '.subject // "it"' <<< "$req") $(jq -r '.revision // ""' <<< "$req") ($(jq -r '.reasons | join(" ")' <<< "$reply")); rejected_without_rework_path: ${rework_gap:-the protocol has no action that reworks a rejected revision with the reasons}; it waits on that protocol path"
        [[ $autonomy == A5 ]] || end_reason+=", or on a person's edit of work/$(jq -r '.path // ""' <<< "$req") and a resume"
      fi
      echo "$end_reason"
      exit 6 ;;
    *) dec="" ;;
  esac
}


# The checks that keep a decision from being taken on the checks alone, as a JSON list (empty:
# the checks justify approving). An approval turns on its action's precondition (the claims that
# must be true; one the decision settles is no failure), a close on its outcome's condition and on
# every obligation being discharged. Obligations of later steps do not keep an approval back.
checks_failing() {
  local req=$1 ir
  ir=$(jq -r .case_dir <<< "$req")/ir.json
  jq -c --slurpfile ir "$ir" '. as $q | $ir[0] as $ir
    | (if $q.type == "close" then [$ir.outcomes[$q.outcome].requires // {} | .. | objects | .claim? | objects | .id]
       else [$ir.actions[$q.action].precondition // {} | .. | objects | .claim? | objects | select((.is // "true") == "true") | .id] end
       | unique) as $reads
    | [($reads[] | . as $c | ($q.position.claims[$c].value // "unknown") | select(. != "true") | "\($c) is \(.)"),
       (if $q.type == "close" then ($q.position.open_obligations[]? | "obligation \(.) open") else empty end)]' <<< "$req"
}

# Autopilot rework of a rejected approval (docs/autonomy-levels.md, "A rejected approval in
# autopilot"). The rejection (sdc.rejection/1: approval, revision, reasons, citations) is written
# beside the decider record and recorded in the case with `approve --result rejected --reasons`;
# the writer whose binding names the approval's kind in `retry_when` is then useful and gets it as
# in/rejection.json, its checks run on the new revision and the approval is asked again. The count
# is the case's rejected records of the approval (sdc-reply rejections). Returns 0 when the run
# goes on; else sets end_cause and end_reason: rework_spent on the third rejection (the approval
# and the three reasons), rejected_without_rework_path when the approval cannot record a
# rejection or no writer is sent back by it (rework_gap says which).
reworked="" rework_gap=""
rework() { # <request> <decider reply>
  local req=$1 reply=$2 cdir act cap subj rev file prior n kind writers rec
  cdir=$(jq -r .case_dir <<< "$req") act=$(jq -r .action <<< "$req") cap=$(jq -r .capability <<< "$req")
  subj=$(jq -r '.subject // ""' <<< "$req") rev=$(jq -r '.revision // ""' <<< "$req")
  prior=$("$reply_bin" rejections --case-dir "$cdir" --action "$act")
  n=$(( $(jq length <<< "$prior") + 1 ))
  rec=$(jq -r '.record // empty' <<< "$reply")
  file=${rec:-$out}/rejection.json
  [[ -n $rec ]] || file=$out/rejection-$(date +%s%N).json
  jq -n --arg a "$act" --arg c "${cdir##*/}" --arg s "$subj" --arg rev "$rev" --arg p "decider:$decider" --arg at "$(now)" \
    --argjson r "$reply" --argjson n "$n" '{format: "sdc.rejection/1", approval: $a, case: $c, subject: $s, revision: $rev,
      principal: $p, at: $at, reasons: $r.reasons, citations: ($r.citations // [($r.cites // [])[] | {cite: .}]),
      decision_record: $r.record, count: $n}' > "$file"
  rework_file=$file
  d "$cdir" approve --at "$(now)" --capability "$cap" --principal "decider:$decider" --action "$act" --result rejected --reasons "$file"
  if [[ $code != 0 ]]; then
    rework_gap="\`$act\` cannot record a rejected result ($(tail -n 1 <<< "$last" | sed 's/^sdc-drive: //'))"
    return 1
  fi
  echo "  recorded the rejection of $act at $rev as decider:$decider (rejection $n; at most 2 are reworked)"
  if (( n > 2 )); then
    end_cause=rework_spent
    end_reason="$act in ${cdir##*/}: rework_spent: decider:$decider rejected $subj three times: $(jq -r --argjson cur "$(jq -c '.reasons' "$file")" \
      '[(.[] | .reasons), $cur] | to_entries | map("\(.key + 1): \(.value | join(" "))") | join("; ")' <<< "$prior"); it waits on a fix of the writer or of the reasons, then --reopen"
    return 1
  fi
  kind=$(jq -r --arg a "$act" '.[$a].produces // ""' "$cdir/bindings.json")
  writers=$(jq -r --arg k "$kind" --argjson u "$(jq -c '.position.useful // []' <<< "$last")" \
    '[to_entries[] | select((.value.retry_when[$k]? // []) | index("rejected")) | .key] as $w | [$u[] | select(IN($w[]))] | join(" ")' "$cdir/bindings.json")
  if [[ -z $writers ]]; then
    rework_gap="the rejection is recorded, but no writer action is admissible and sent back by a rejected \`$kind\`"
    return 1
  fi
  echo "  rework: $writers runs again with the rejection's reasons (in/rejection.json), then $act is asked again"
  return 0
}

# Autopilot end of a refused approval that has no writer but an end in the protocol: a rejected
# release.publish withholds the release (sdc.release outcome `withheld`, decision
# `explicitly_withheld`). Canon admits that decision from anyone in any state, so this is the
# tools' rule: only under A5, only directly after the decider refused, and only for the actions in
# refusal_end. The refusal is recorded as the protocol represents it, the capability denied by
# the decider (`approve --deny`; the approval has no `rejected` result); then the decision by
# rule:autopilot with the rejection (the decider's reasons and citations) as its log blob
# (`sdc-drive decide --after-denial`, which refuses it in any other state), and the case is closed
# through the outcome. The parents then end through the outcome their evaluation offers (epic
# not_released, root not_delivered). Returns 1 when the action has no such end.
declare -A refusal_end=([release.publish]="explicitly_withheld withheld")
rework_file=""
withhold() { # <request> <decider reply>
  local req=$1 reply=$2 cdir act cap subj dn oc
  act=$(jq -r .action <<< "$req")
  [[ $autonomy == A5 && -n ${refusal_end[$act]:-} && -n $rework_file ]] || return 1
  read -r dn oc <<< "${refusal_end[$act]}"
  cdir=$(jq -r .case_dir <<< "$req") cap=$(jq -r .capability <<< "$req") subj=$(jq -r '.subject // ""' <<< "$req")
  jq -e --arg o "$oc" --arg d "$dn" '.outcomes[$o].requires == {decision: $d}' "$cdir/ir.json" > /dev/null || return 1
  d "$cdir" approve --at "$(now)" --capability "$cap" --principal "decider:$decider" --deny
  [[ $code == 0 ]] || { rework_gap="\`$cap\` could not be denied: $(tail -n 1 <<< "$last")"; return 1; }
  d "$cdir" decide --at "$(now)" --decision "$dn" --outcome "$oc" --principal rule:autopilot --after-denial "$cap" --reasons "$rework_file"
  [[ $code == 0 ]] || { rework_gap="\`$dn\` was refused: $(tail -n 1 <<< "$last" | sed 's/^sdc-drive: //')"; return 1; }
  d "$cdir" close --at "$(now)" --outcome "$oc"
  [[ $code == 0 ]] || { rework_gap="\`$oc\` could not be closed: $(tail -n 1 <<< "$last")"; return 1; }
  end_note="${subj:-$act} $oc: $(jq -r '.reasons[0] // ""' <<< "$reply")"
  echo "  $act refused by decider:$decider: $cap denied, $dn recorded by rule:autopilot, ${cdir##*/} closed $oc"
  return 0
}

# The open scope-changing questions of the interview in case $1, one decider decision each (action
# $2, position $3): each answer is written as a person's is (`answer`, `status: accepted` for the
# default or `changed`, `answered_by: decider:<kind>`). Returns 0 when all were answered, 1 when
# one waits on a person, 2 when none is open; exits on a refusal.
answer_questions() {
  local cdir=$1 act=$2 pos=$3 qf=$1/work/questions.yaml found=no rev q st qs=()
  rev=$(jq -r '.artifacts.questions.revision // "none"' "$cdir/case.json")
  mapfile -t qs < <(yq -o=json -I=0 '.questions[] | select(.scope_changing == true and .status == "open")' "$qf" 2>/dev/null)
  for q in "${qs[@]}"; do
    [[ -n $q ]] || continue
    found=yes
    decide "$(jq -cn --arg c "$cdir" --arg a "$act" --arg rev "$rev" --argjson q "$q" --argjson pos "$pos" \
      '{type: "question", case_dir: $c, action: $a, capability: "scope.answer", subject: "questions",
        path: "questions.yaml", revision: $rev, question: $q, outcome: null, position: $pos}')"
    [[ -n $dec ]] || return 1
    st=changed
    [[ $ans == "$(jq -r .default <<< "$q")" ]] && st=accepted
    ANS=$ans yq -i "(.questions[] | select(.id == \"$(jq -r .id <<< "$q")\")) |= (.answer = strenv(ANS)
      | .status = \"$st\" | .answered_by = \"$dec_principal\")" "$qf"
  done
  [[ $found == yes ]] || return 2
}

# The findings in case $1 that wait for a decision (amend.upstream's handoff, action $2, position
# $3), one decider decision each: the decision is written as a person's is (`decision:` on the
# finding) with `decided_by: decider:<kind>`, and amend.upstream applies it on the next round
# through the same program check as the agent's amendments. A finding whose decider decision
# that check refused (`refused_decision`) waits for a person. Returns as answer_questions.
decide_findings() {
  local cdir=$1 act=$2 pos=$3 ff=$1/work/findings.yaml found=no rev f fid fs=()
  rev=$(jq -r '.artifacts.findings.revision // "none"' "$cdir/case.json")
  mapfile -t fs < <(yq -o=json -I=0 '.findings[] | select(.status == "needs_person" and (.decision // "") == "")' "$ff" 2>/dev/null)
  for f in "${fs[@]}"; do
    [[ -n $f ]] || continue
    found=yes
    fid=$(jq -r .id <<< "$f")
    if jq -e '.refused_decision' <<< "$f" > /dev/null; then
      wait_why="$fid: the program check refused $(jq -r .refused_decision.decided_by <<< "$f")'s decision ($(jq -r .refused_decision.why <<< "$f"))"
      if [[ $autonomy == A5 ]]; then echo "  $wait_why"; else echo "  $wait_why: a person decides"; fi
      return 1
    fi
    decide "$(jq -cn --arg c "$cdir" --arg a "$act" --arg rev "$rev" --argjson f "$f" --argjson pos "$pos" \
      '{type: "finding", case_dir: $c, action: $a, capability: null, subject: "findings",
        path: "findings.yaml", revision: $rev, question: null, finding: $f, outcome: null, position: $pos}')"
    [[ -n $dec ]] || return 1
    ANS=$ans yq -i "(.findings[] | select(.id == \"$fid\")) |= (.decision = strenv(ANS) | .decided_by = \"decider:$decider\")" "$ff"
  done
  [[ $found == yes ]] || return 2
}

# Every pending decision of the stop in $last, through the decider. Returns 0 when all were
# taken; exits on a refusal; returns 1 when one waits on a person.
declare -A taken
decide_all() {
  local cdir pos n act cap subj path rev key rc
  cdir=$(jq -r .case_dir <<< "$last")
  pos=$(jq -c .position <<< "$last")
  while IFS= read -r n; do
    act=$(jq -r .action <<< "$n")
    if jq -e '.command | test(" run --at ")' <<< "$n" > /dev/null; then
      # A handoff: the findings that wait for a decision (amend.upstream), else the open
      # scope-changing questions of the interview.
      rc=0
      if [[ $act == amend.upstream ]]; then
        decide_findings "$cdir" "$act" "$pos" || rc=$?
        (( rc == 2 )) && { wait_why="nothing for decider:$decider at $act: no finding waits for a decision"; echo "$wait_why"; return 1; }
      else
        answer_questions "$cdir" "$act" "$pos" || rc=$?
      fi
      (( rc == 1 )) && return 1
      (( rc == 2 )) && { wait_why="nothing for decider:$decider at $act: no open scope-changing question (questions.check may refuse a non-human answer)"; echo "$wait_why"; return 1; }
      continue
    fi
    cap=$(jq -r '.command | capture("--capability (?<c>[^ ]+)").c' <<< "$n")
    subj=$(jq -r --arg a "$act" '.[$a].subject // ""' "$cdir/bindings.json")
    path=$(jq -r --arg s "$subj" '."$artifacts"[$s] // ""' "$cdir/bindings.json")
    rev=$(jq -r --arg s "$subj" '.artifacts[$s].revision // "none"' "$cdir/case.json")
    key="$cdir|$act|$rev"
    [[ -z ${taken[$key]:-} ]] || { echo "pending again after decider:$decider approved it: $act at $rev in $cdir"; exit 4; }
    # scope.answer is not an approval of a revision: each open scope-changing question gets its
    # own answer from the decider, then scope.answer is taken under the decider's principal. With
    # none open (a person's edits wait for attribution) it is decided as an approval.
    rc=2
    [[ $act == scope.answer ]] && { rc=0; answer_questions "$cdir" "$act" "$pos" || rc=$?; }
    (( rc == 1 )) && return 1
    if (( rc == 2 )); then
      decide "$(jq -cn --arg c "$cdir" --arg a "$act" --arg cap "$cap" --arg s "$subj" --arg p "$path" --arg rev "$rev" \
        --argjson pos "$pos" '{type: "approval", case_dir: $c, action: $a, capability: $cap, subject: $s,
          path: $p, revision: $rev, question: null, outcome: null, position: $pos}')"
      # A reworked rejection: the writer runs in the next round, the approval is asked again after it.
      [[ $reworked == yes ]] && { reworked=""; return 0; }
      [[ -n $dec ]] || return 1
    fi
    taken[$key]=1
    if jq -e '.command | test("--action ")' <<< "$n" > /dev/null; then
      d "$cdir" approve --at "$(now)" --capability "$cap" --principal "$dec_principal" --action "$act"
    else
      d "$cdir" approve --at "$(now)" --capability "$cap" --principal "$dec_principal"
    fi
    echo "  approve $act as $dec_principal: exit $code"
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
      # The close is a decision (class close, docs/autonomy-levels.md § 3.2) for `operating`, and
      # where the outcome requires a decision; an end the protocol reaches by its checks alone
      # (not_delivered, refused) is closed without one. Under A5 `operating` is no decision either
      # (D140): its claims (roadmap.approved, epics.opened, children.passed) are what the approvals
      # upstream already settled, the outcome binds no capability, so rule:autopilot closes it.
      if [[ $decider != person ]] &&
         ! jq -e --arg o "$outcome" 'any(.outcomes[$o].requires | .. | objects; has("decision"))' "$root/ir.json" > /dev/null 2>&1; then
        if [[ $autonomy == A5 ]]; then finish "$outcome" rule:autopilot; fi
        [[ $outcome != operating ]] && finish "$outcome"
      fi
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
