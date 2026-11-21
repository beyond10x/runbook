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
#   scripts/run-trial.sh <name> ... --autopilot --keep-going
#                                               trial and eval runs only (D142): where autopilot
#                                               stops with a cause it can continue past, a line
#                                               goes to defects.jsonl (sdc.run-defect/1) and a
#                                               forced continuation is taken by rule:keep-going;
#                                               at most 12 per run, each cause at a case and
#                                               action once (see keep_going_force)
# Under --autopilot, before a stop that is no cap, run.unstick picks a legal move (D147, see
# unstick(); SDC_UNSTICK=off turns it off, SDC_LLM_MODEL_UNSTICK, SDC_UNSTICK_ADAPTER).
#   scripts/run-trial.sh <name> --autopilot --recorded <source run or recording> [--on-divergence
#     stop|nearest|live] [--live-adapter <a>] [--live-cap <n>] [--inject <fault>]... [--set <k>=<v>]...
#     [--take-recorded <actions>] [--source-tree <commit>]
#                                               re-drive a recorded run: the real driver and programs,
#                                               at every model and decider call the reply the source
#                                               recorded there (driver/src/bin/sdc-replay.rs,
#                                               docs/driver.md "Recorded replay"); exit 9 when the
#                                               recording diverged under the policy stop
#
# Writes $SDC_RUNS_DIR/<name>/ (default runs/): store/IDEA-1/ (the case tree, the layout of
# runs/full-cycle-1/store/), run.log (every sdc-drive command and its output), llm-calls.jsonl (one
# line per LLM call: adapter, model, tokens, cost, duration, allowed operations, tool calls),
# llm/NNN-<action>/ (prompt, frame, argv, event stream, transcript of each call),
# events/<case>.jsonl, and at the end decision.json and replay.json. With a decider also
# decisions.jsonl (one line per decider decision) and decider/NNN-<id>/ (each LLM decider call).
# run-settings.jsonl: one line per start or resume (sdc.run-settings/1: the decider's kind, model,
# escalate mode and call cap, the rework cap `rework_limit` {value, source}, the adapter, the LLM
# call cap, the calls made before the leg).
# fs/ is the file-change record of sdc-watch (scripts/watch-run.sh), running while this script
# runs and started again on a resume; SDC_WATCH=0 skips it.
#
# Limits: SDC_MAX_LLM_CALLS (default 30, over the whole tree), SDC_MAX_WALL_SECONDS per run
# (default 5400). Model per node class (tools/node-classes.json): SDC_LLM_MODEL_WRITER (spec.author,
# plan.decompose), SDC_LLM_MODEL_IMPLEMENT (change.implement), SDC_LLM_MODEL_REVIEW (spec.review,
# change.review: no LLM yet), SDC_LLM_MODEL_OTHER (every other node), else SDC_LLM_MODEL for all
# nodes, else each binding's `llm.model` (claude-haiku-5-5); the decider SDC_LLM_MODEL_DECIDER,
# else SDC_DECIDER_MODEL. run-settings.jsonl `models` records the effective model per class.
# Reasoning effort per class, the same way: SDC_LLM_EFFORT_<CLASS> (WRITER, IMPLEMENT, REVIEW, DECIDER, UNSTICK,
# OTHER), else SDC_LLM_EFFORT, else the class default in tools/node-classes.json (high; the decider, the
# requester and run.unstick xhigh), always passed to the adapter; low, medium, high, xhigh or max, anything else
# exits 64. run-settings.jsonl `efforts` records it per class and for the requester (source `default`).
#
# Decider (SDC_DECIDER): who takes the decisions that wait on a person: the pending approvals, the
# open scope-changing interview questions, and closing the root.
#   person (default)  stop and print the commands (exit 3), as before
#   approve           rule: approve every approval, accept every question's default, close
#   reject            rule: refuse; the refusal is recorded and the run stops (exit 6)
#   model             tools/decider.sh: one bounded LLM call per decision (SDC_DECIDER_MODEL,
#                     default claude-haiku-5-5, through SDC_LLM_ADAPTER); escalate or an invalid
#                     reply stops for a person (exit 3) as today; reject stops (exit 6). Principal
#                     decider:model, whatever the model; the model id is in run-settings.jsonl and
#                     on each decisions.jsonl line (D149). `haiku` is its old name, kept as an alias;
#                     runs before D149 recorded decider:haiku.
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
# (or already stopped), 9 a re-driven run's recording diverged (cause recording_diverged), 64 usage.
#
# Run mode: SDC_AUTONOMY=A0..A5, or `--autopilot` (A5, the eval's mode): the decider (model unless
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
dry=no stop_reason="" reopen=no autopilot=no keep_going=no pos=() recorded="" replay_args=()
while (($#)); do
  case $1 in
    --dry) dry=yes ;;
    --autopilot) autopilot=yes ;;
    --keep-going) keep_going=yes ;;
    --recorded) recorded=${2:-}; [[ -n $recorded ]] || { echo "--recorded needs a source run"; exit 64; }; shift ;;
    --on-divergence|--live-adapter|--live-cap|--inject|--set|--take-recorded|--source-tree) replay_args+=("$1" "${2:-}"); shift ;;
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
# Recorded replay: sdc-replay prepares the new run (replay/plan.json) and names its environment:
# itself as the adapter of every model and decider call, the source's decider, models and caps, the
# changed conditions. A resume of a re-driven run takes the same environment again.
replay_bin=${SDC_REPLAY_BIN:-$sdc/driver/target/debug/sdc-replay}
replay_env=""
if [[ -n $recorded ]]; then
  [[ ! -d $root ]] || { echo "--recorded starts a new run: $name exists ($out)"; exit 64; }
  rm -rf "$out"
  replay_env=$("$replay_bin" prepare --source "$recorded" --out "$out" --tree "$sdc" ${replay_args[@]+"${replay_args[@]}"}) || exit 64
elif (( ${#replay_args[@]} )); then
  echo "${replay_args[0]} needs --recorded <source>"; exit 64
elif [[ -f $out/replay/plan.json && -z $stop_reason ]]; then
  replay_env=$("$replay_bin" env --out "$out") || exit 64
fi
[[ -z $replay_env ]] || while IFS=$'\t' read -r k v; do export "$k=$v"; done < <(jq -r 'to_entries[] | [.key, .value] | @tsv' <<< "$replay_env")
max_llm=${SDC_MAX_LLM_CALLS:-30}
max_wall=${SDC_MAX_WALL_SECONDS:-5400}
decider=${SDC_DECIDER:-person}
[[ $decider == haiku ]] && decider=model
escalate_mode=${SDC_DECIDER_ESCALATE:-stop}
max_decider=${SDC_MAX_DECIDER_CALLS:-20}
# The rework cap (D184): how many rejections of one approval are reworked before rework_spent.
# rule-ends.json `rework_limit` is the default; SDC_MAX_REJECTIONS overrides it when set. The
# effective value goes into run-settings.jsonl (`rework_limit: {value, source}`), and rework()
# here and the writer's rework prompt (tools/llm-node.sh, through SDC_RUN_SETTINGS) read it there.
if [[ -n ${SDC_MAX_REJECTIONS:-} ]]; then rework_cap=$SDC_MAX_REJECTIONS rework_cap_source=SDC_MAX_REJECTIONS
else rework_cap=$(jq -r .rework_limit "$ex/rule-ends.json") rework_cap_source=rule-ends.json; fi
[[ $rework_cap =~ ^[0-9]+$ ]] || { echo "$rework_cap_source: the rework cap is a whole number of reworks (0 or more), not '$rework_cap'"; exit 64; }
# Approvals by rule (docs/driver.md "Approvals by program"): under A5 an approval whose program
# check recorded a passing result for the exact revision is taken by rule:autopilot, no decider
# call. SDC_RULE_APPROVALS=off restores every such approval to the decider; run-settings.jsonl
# records `rule_approvals`. Below A5 it has no effect.
rule_approvals=${SDC_RULE_APPROVALS:-on}
[[ $rule_approvals == on || $rule_approvals == off ]] || { echo "SDC_RULE_APPROVALS: on or off, not '$rule_approvals'"; exit 64; }
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
  # A finding that waits only because the program check refused a decider's (or rule:keep-going's)
  # decision gets that decision back once: amend.upstream applies it through the current check (a
  # fixed checker takes it; refused again, it is stored again and the stop is as before).
  while IFS= read -r ff; do
    n=$(yq '[.findings[] | select(.status == "needs_person" and (.decision // "") == "" and .refused_decision != null)] | length' "$ff" 2>/dev/null || echo 0)
    (( n > 0 )) || continue
    yq -i '(.findings[] | select(.status == "needs_person" and (.decision // "") == "" and .refused_decision != null))
      |= (.decision = .refused_decision.decision | .decided_by = .refused_decision.decided_by | del(.refused_decision))' "$ff"
    echo "# reopen: $n refused decision(s) in ${ff#"$store"/} put back for the current check" >> "$log"
  done < <(find "$store" -name findings.yaml -path '*/work/*' -exec realpath {} + 2>/dev/null | sort -u)
fi

case $decider in person|approve|reject|model) ;; *) echo "SDC_DECIDER: person, approve, reject or model (haiku: its old name), not $decider"; exit 64 ;; esac

# The reasoning effort per node class (SDC_LLM_EFFORT_<CLASS>, SDC_LLM_EFFORT): an unknown value is
# refused here, before anything is created. The adapters' CLIs ignore one with a warning.
accepted=$(jq -r '.efforts | join(", ")' "$ex/tools/node-classes.json")
for v in SDC_LLM_EFFORT SDC_LLM_EFFORT_REQUESTER $(jq -r '.classes | keys[] | "SDC_LLM_EFFORT_" + ascii_upcase' "$ex/tools/node-classes.json"); do
  [[ -z ${!v:-} ]] || jq -e --arg e "${!v}" '.efforts | index($e)' "$ex/tools/node-classes.json" > /dev/null ||
    { echo "$v: '${!v}' is not a reasoning effort; accepted: $accepted (unset: the adapter's own default)"; exit 64; }
done

# The run mode (docs/autonomy-levels.md § 3.3): SDC_AUTONOMY, or --autopilot for A5. A0 is the
# person (SDC_DECIDER=person); A5 is autopilot: the decider (model unless SDC_DECIDER names another)
# holds every decision and is final (escalate final), and a stop that would need a person ends the
# run (exit 7) with a stop record by rule:autopilot. A1 to A4 have no per-class routing yet: they
# run as SDC_DECIDER says. Without a level it is derived: person A0, a final decider A5, a
# decider that escalates to a person none (no level matches it).
autonomy=${SDC_AUTONOMY:-}
autonomy_source=SDC_AUTONOMY
[[ $autopilot == yes ]] && { [[ -z $autonomy || $autonomy == A5 ]] || { echo "--autopilot is A5, SDC_AUTONOMY says $autonomy"; exit 64; }; autonomy=A5 autonomy_source=--autopilot; }
case $autonomy in
  A5)
    [[ ${SDC_DECIDER:-} != person ]] || { echo "autopilot (A5) has no person: SDC_DECIDER=person is refused; name a decider (model, approve, reject) or leave it unset (model)"; exit 64; }
    [[ ${SDC_DECIDER_ESCALATE:-final} == final ]] || { echo "autopilot (A5): the decider is final; SDC_DECIDER_ESCALATE=$SDC_DECIDER_ESCALATE is refused"; exit 64; }
    # The decider reads SDC_DECIDER_ESCALATE: under final its prompt asks for `best` and its check
    # refuses an escalate without one.
    [[ -n ${SDC_DECIDER:-} ]] || decider=model
    escalate_mode=final
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
[[ $keep_going == no || $autonomy == A5 ]] || { echo "--keep-going is for autopilot runs (A5): add --autopilot"; exit 64; }
# A script or a session standing in for the person (SDC_STAND_IN=<who>; --dry's scripted person)
# makes the run an end-to-end test of the person path: labelled stand_in, its numbers no eval's.
stand_in=${SDC_STAND_IN:-}
[[ $dry == yes && $decider == person ]] && stand_in=${stand_in:-scripts/run-trial.sh --dry}

export SDC_LLM_RECORD=$out/llm SDC_LLM_LOG=$out/llm-calls.jsonl SDC_RUN_SETTINGS=$out/run-settings.jsonl
if [[ $dry == yes ]]; then
  export SDC_LLM_ADAPTER=dry SDC_LLM_DRY_LOG=$out/dry-commands.txt SDC_PRINCIPAL=dry-run
else
  export SDC_LLM_ADAPTER=${SDC_LLM_ADAPTER:-metaharness}
  [[ $SDC_LLM_ADAPTER == dry ]] && export SDC_LLM_DRY_LOG=${SDC_LLM_DRY_LOG:-$out/dry-commands.txt}
fi
# Every node class whose reply is a data document runs on an adapter that takes a response schema
# (sdc-reply adapters; the review: SDC_REVIEW_ADAPTER, else claude-cli on a metaharness run). One
# that cannot take one refuses the run before any call (exit 64), naming the class and adapter.
adapters=$("${SDC_REPLY:-$sdc/driver/target/debug/sdc-reply}" adapters --tools "$ex/tools" 2>&1) ||
  { grep -v '^{' <<< "$adapters" | sed 's/^/run-trial.sh: /' >&2; exit 64; }
SDC_REVIEW_ADAPTER=$(jq -r '.review.adapter' <<< "$adapters")
export SDC_REVIEW_ADAPTER

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
# A re-driven run whose recording diverged under the policy stop ends here (exit 9).
replay_check() {
  local f=$out/replay/diverged.json
  [[ -n ${SDC_REPLAY:-} && -f $f ]] || return 0
  end_cause=recording_diverged end_case=$(jq -r .place.case "$f") end_action=$(jq -r .place.name "$f")
  end_reason="recording_diverged: $(jq -r .summary "$f") (policy $(jq -r .policy "$f"))"
  echo "$end_reason"
  exit 9
}
replay_seal() { [[ -z ${SDC_REPLAY:-} ]] || "$replay_bin" seal "$out" > /dev/null || true; }

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
  [[ -n $recorded ]] || rm -rf "$out"
  mkdir -p "$out/events"
  watch_start
  : > "$log"
  expected | tee "$out/expected-llm-calls.txt"
  d "$root" init --protocol "$sdc/protocol/sdc.protocol.yaml" --id IDEA-1 --bindings "$ex/sdc.bindings.json"
  [[ $code == 0 ]] || { echo "init failed: $last"; exit 1; }
  [[ -n $goals || -z ${SDC_REPLAY_GOALS:-} ]] || goals=$SDC_REPLAY_GOALS
  if [[ $dry == yes ]]; then
    cp "$ex/fixtures/idea.yaml" "$root/work/idea.yaml"
  elif [[ -z $idea_text && -n ${SDC_REPLAY_IDEA:-} ]]; then
    cp "$SDC_REPLAY_IDEA" "$root/work/idea.yaml"
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
  # --reopen: a step that ran at its case's current state but recorded nothing (a review whose reply
  # stayed invalid: review_invalid) runs once more on the unchanged state (sdc-drive reopen), so a
  # fixed node or adapter gets its turn; invalid again, the stop names the new call.
  if [[ $reopen == yes ]]; then
    d "$root" reopen --at "$(now)" ${SDC_PRINCIPAL:+--principal "$SDC_PRINCIPAL"}
    [[ $code == 0 ]] || { echo "reopen failed: $last"; exit 1; }
    echo "# reopen: $(jq -r .reopened <<< "$last") case(s) marked: a step that recorded nothing at its case's state runs once more" >> "$log"
  fi
fi

exec > >(tee -a "$out/console.txt") 2>&1

# run.unstick (D147, below at unstick()): on under autopilot unless SDC_UNSTICK=off or its adapter
# (SDC_UNSTICK_ADAPTER, else the decider's, else SDC_LLM_ADAPTER) calls no model (stub, dry).
unstick_on=no
unstick_adapter=${SDC_UNSTICK_ADAPTER:-${SDC_DECIDER_ADAPTER:-$SDC_LLM_ADAPTER}}
if [[ $autonomy == A5 && ${SDC_UNSTICK:-on} != off ]]; then
  case $unstick_adapter in stub|dry) ;; *) unstick_on=yes ;; esac
fi
case $unstick_adapter in metaharness|claude-cli|dry) unstick_adapter_bin=$ex/tools/llm-adapter-$unstick_adapter.sh ;; *) unstick_adapter_bin=$unstick_adapter ;; esac
unstick_model=${SDC_LLM_MODEL_UNSTICK:-claude-opus-5-5}

# Who holds the decisions in this leg, and its caps: one line per start or resume (the UI reads it).
model=
[[ $decider == model ]] && model=${SDC_LLM_MODEL_DECIDER:-${SDC_DECIDER_MODEL:-claude-haiku-5-5}}
# The effective model per node class (tools/node-classes.json): SDC_LLM_MODEL_<CLASS>, else
# SDC_LLM_MODEL, else each binding's llm.model (per node; `model` is null when they differ). The
# decider: SDC_LLM_MODEL_DECIDER, else SDC_DECIDER_MODEL, else claude-haiku-5-5.
models=$(jq -cn --slurpfile c "$ex/tools/node-classes.json" \
  --argjson b "$(jq -s 'map(to_entries[] | select(.value.llm != null) | {(.key): .value.llm.model}) | add' "$ex"/sdc-*.bindings.json)" \
  --arg all "${SDC_LLM_MODEL:-}" --arg writer "${SDC_LLM_MODEL_WRITER:-}" --arg implement "${SDC_LLM_MODEL_IMPLEMENT:-}" \
  --arg review "${SDC_LLM_MODEL_REVIEW:-}" --arg other "${SDC_LLM_MODEL_OTHER:-}" --arg dclass "${SDC_LLM_MODEL_DECIDER:-}" \
  --arg dmodel "${SDC_DECIDER_MODEL:-}" --arg umodel "$unstick_model" '
  {writer: $writer, implement: $implement, review: $review, other: $other} as $set
  | ($c[0].classes | del(.decider, .unstick) | to_entries | map(.key as $k
      | (if $set[$k] != "" then {m: $set[$k], src: "SDC_LLM_MODEL_\($k | ascii_upcase)"}
         elif $all != "" then {m: $all, src: "SDC_LLM_MODEL"} else {m: null, src: "binding"} end) as $e
      | (.value | map({(.): ($e.m // $b[.])}) | add // {}) as $nodes
      | {(.key): {model: ($e.m // ([$nodes[]] | unique | if length == 1 then .[0] else null end)), source: $e.src,
          nodes: $nodes}}) | add)
  + {decider: (if $dclass != "" then {model: $dclass, source: "SDC_LLM_MODEL_DECIDER"}
      elif $dmodel != "" then {model: $dmodel, source: "SDC_DECIDER_MODEL"}
      else {model: "claude-haiku-5-5", source: "default"} end)}
  + {unstick: {model: $umodel, source: (if $ENV.SDC_LLM_MODEL_UNSTICK // "" != "" then "SDC_LLM_MODEL_UNSTICK" else "default" end)}}')
# The effective effort per node class, and for the requester: the first set of SDC_LLM_EFFORT_<CLASS>
# (the requester: _REQUESTER, then _DECIDER), SDC_LLM_EFFORT, else the class's `default_efforts` in
# node-classes.json (source "default"): the level is always passed, never left to the adapter's host.
eff() { # <default level> <variable>...
  local v
  for v in "${@:2}" SDC_LLM_EFFORT; do
    [[ -z ${!v:-} ]] || { jq -cn --arg e "${!v}" --arg s "$v" '{effort: $e, source: $s}'; return; }
  done
  jq -cn --arg e "$1" '{effort: $e, source: "default"}'
}
efforts=$({
  for c in $(jq -r '.classes | keys[]' "$ex/tools/node-classes.json"); do
    jq -cn --arg c "$c" --argjson e "$(eff "$(jq -r --arg c "$c" '.default_efforts[$c]' "$ex/tools/node-classes.json")" "SDC_LLM_EFFORT_${c^^}")" '{($c): $e}'
  done
  jq -cn --argjson e "$(eff "$(jq -r '.default_efforts.requester' "$ex/tools/node-classes.json")" SDC_LLM_EFFORT_REQUESTER SDC_LLM_EFFORT_DECIDER)" '{requester: $e}'
} | jq -cs add)
jq -cn --arg at "$(now)" --arg leg "$leg" --arg kind "$decider" --arg model "$model" --arg esc "$escalate_mode" \
  --argjson models "$models" --argjson adapters "$adapters" --argjson efforts "$efforts" \
  --argjson llm "$max_llm" --argjson dec "$max_decider" --argjson wall "$max_wall" --arg adapter "$SDC_LLM_ADAPTER" \
  --argjson calls "$(cat "$SDC_LLM_LOG" 2>/dev/null | wc -l)" --arg level "$autonomy" --arg lsrc "$autonomy_source" \
  --argjson rcap "$rework_cap" --arg rsrc "$rework_cap_source" --arg ra "$rule_approvals" --arg stand "$stand_in" --arg kg "$keep_going" --arg uon "$unstick_on" --arg uad "$unstick_adapter" --arg umodel "$unstick_model" --arg rq "${SDC_REQUESTER_DECIDER:-}" --arg nodes "${SDC_LLM_NODES:-}" \
  --arg commit "$(git -C "$sdc" rev-parse --short=12 HEAD 2>/dev/null || true)" \
  --arg dirty "$(git -C "$sdc" status --porcelain --untracked-files=no 2>/dev/null | head -c 1 || true)" \
  --argjson replay "$( [[ -n ${SDC_REPLAY:-} ]] && "$replay_bin" settings --out "$out" || echo null)" '{format: "sdc.run-settings/1", at: $at, leg: $leg,
    tree: {commit: (if $commit == "" then null else $commit end), dirty: ($dirty != "")},
    llm_nodes: (if $nodes == "" then null else $nodes end), replay: $replay,
    autonomy: (if $level == "" then null else $level end), autonomy_source: $lsrc, keep_going: ($kg == "yes"),
    unstick: {on: ($uon == "yes"), adapter: $uad, model: $umodel},
    per_class_routing: (if ($level | IN("A1", "A2", "A3", "A4")) then "not built: every decision goes to the decider" else null end),
    requester_decider: (if $rq == "" then null else {kind: $rq, principal: "decider:requester", classes: "the requester side of decision-classes.json"} end),
    decider: {kind: $kind, model: (if $model == "" then null else $model end), escalate: $esc,
      max_decider_calls: $dec}, rework_limit: {value: $rcap, source: $rsrc}, rule_approvals: $ra, adapter: $adapter, max_llm_calls: $llm, max_wall_seconds: $wall,
    llm_calls_before: $calls, models: $models, adapters: $adapters, efforts: $efforts, stand_in: ($stand != ""), stand_in_by: (if $stand == "" then null else $stand end)}' >> "$out/run-settings.jsonl"
ustart=""
[[ $unstick_on == yes ]] && ustart=" run.unstick ($unstick_model),"
[[ -z ${SDC_REPLAY:-} ]] || jq -r .first_line "$SDC_REPLAY"
if [[ $leg == started ]]; then
  echo "started $name: adapter $SDC_LLM_ADAPTER, review adapter $SDC_REVIEW_ADAPTER, decider $decider${model:+ ($model)}, escalate $escalate_mode,$ustart LLM call cap $max_llm, $(yq -r '"idea: " + .text' "$root/work/idea.yaml" 2>/dev/null || yq -r '"bug: " + .text' "$root/work/bug.yaml")"
else
  echo "resuming $name: adapter $SDC_LLM_ADAPTER, review adapter $SDC_REVIEW_ADAPTER, decider $decider${model:+ ($model)}, escalate $escalate_mode,$ustart LLM call cap $max_llm, $(cat "$SDC_LLM_LOG" 2>/dev/null | wc -l) LLM calls so far"
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
# decider_budget_spent, finding_reraised (a program finding raised again after its decision was
# applied), decision_not_takeable (a stop no holder of this run can take: the program
# check refused the decider's decision, or a handoff with nothing for the decider), and the
# driver's verdicts (stuck, writer_spent, writer_not_called, llm_budget_spent, step_budget_spent, wall_time_spent),
# and review_invalid (stuck after a change.review that recorded nothing: its reply stayed invalid).
end_reason="" end_cause="" end_action="" end_case="" wait_why=""
verdict_end() { # <exit code>: end_cause, end_case, end_action and end_reason from the driver's $last
  local rc=$1 v
  v=$(jq -r '.verdict // "error"' <<< "${last:-null}" 2>/dev/null || echo error)
  end_cause=$v
  end_case=$(jq -r '(.case_dir // "") | split("/") | last' <<< "${last:-null}" 2>/dev/null)
  end_action=$(jq -r '(.spent // .position.spent // [])[0].action // (.position.not_called // [])[0].action // .action // .next[0].action // ""' <<< "${last:-null}" 2>/dev/null)
  case $v in
    writer_spent) end_reason="$end_action in $end_case: writer_spent ($(jq -r '(.spent // .position.spent // [])[0] | "\(.runs) runs, last exit \(.last_exit)"' <<< "$last")): it waits on a code fix of the writer, then --reopen" ;;
    writer_not_called) end_reason="$end_action in $end_case: writer_not_called ($(jq -r '(.position.not_called // [])[0].reason // ""' <<< "$last" | cut -c1-200)): nothing else is useful; it waits on a fix of the writer or a decision, then --reopen" ;;
    stuck) end_reason="${end_case:-the run}: stuck, no action is admissible and useful: it waits on a code or protocol fix, then --reopen"
      # Stuck because the case's last step was a review that recorded nothing (its reply stayed
      # invalid after its retries, or its call failed): cause review_invalid, naming the call.
      local cd line
      cd=$(jq -r '.case_dir // ""' <<< "$last" 2>/dev/null)
      line=$(jq -rs '[.[] | select(.event == "action_finished")] | last
        | select(.action == "change.review" and (.exit | IN("0", "1") | not)) | .log // empty' "$cd/events.jsonl" 2>/dev/null)
      line=$([[ -n $line ]] && grep -v '^[[:space:]]*$' "$cd/blobs/$line" 2>/dev/null | tail -n 1)
      if [[ $line == *"change.review recorded nothing after"* ]]; then
        end_cause=review_invalid end_action=change.review
        end_reason="change.review in $end_case: review_invalid (${line#llm-node: }): it waits on a fix of the review node or its adapter, then --reopen"
        end_reason=${end_reason:0:600}
      fi ;;
    *_budget_spent|wall_time_spent) end_reason="${end_action:+$end_action in }${end_case:-the run}: $v: it waits on a higher cap, then --reopen" ;;
    *) end_reason="${end_case:-the run}: the run ended with exit $rc ($v): it waits on a fix, then --reopen" ;;
  esac
}
autopilot_end() {
  local rc=$1
  [[ $autonomy == A5 && $rc != 0 && ! -f $stopped && -d $root ]] || return 0
  [[ -n $end_cause ]] || verdict_end "$rc"
  [[ $keep_going == yes ]] && defect "$end_cause" "$end_case" "$end_action" "$end_reason" null false stops
  write_stop rule:autopilot "autopilot: $end_reason" "$end_cause" || true
}

# Keep-going (D142, --keep-going, trial and eval runs only). defects.jsonl, one line per stop
# autopilot met (sdc.run-defect/1: at, case, action, cause, reason, decision (the decider's reply
# or the request that led to it), event_seq (the case's last event), llm_calls, forced, principal,
# continuation). A forced continuation is recorded under rule:keep-going, never as a decider's or a
# person's act.
defects=$out/defects.jsonl
max_forced=12
forced_count() { [[ -s $defects ]] || { echo 0; return; }; jq -s '[.[] | select(.forced)] | length' "$defects"; }
defect() { # <cause> <case> <action> <reason> <decision JSON> <forced true|false> <continuation>
  local cdir seq
  cdir=$(find "$store" -type d -name "${2:-IDEA-1}" 2>/dev/null | head -n 1)
  seq=$(tail -n 1 "${cdir:-$root}/events.jsonl" 2>/dev/null | jq '.seq // null' 2>/dev/null || echo null)
  jq -cn --arg at "$(now)" --arg c "$2" --arg a "$3" --arg cause "$1" --arg why "$4" --argjson d "${5:-null}" \
    --argjson seq "${seq:-null}" --argjson calls "$(cat "$SDC_LLM_LOG" 2>/dev/null | wc -l)" --argjson f "$6" --arg cont "$7" \
    '{format: "sdc.run-defect/1", at: $at, case: $c, action: $a, cause: $cause, reason: $why, decision: $d,
      event_seq: $seq, llm_calls: $calls, forced: $f, principal: (if $f then "rule:keep-going" else null end),
      continuation: $cont}' >> "$defects"
}
# keep_going_force <cause> <case> <action> <reason> <decision JSON> <continuation>: 0 when the
# forced continuation may be taken (its defect line is written); 1 when the run stops: no
# --keep-going, the same cause at the same case and action was forced before, or 12 were taken
# (end_cause keep_going_spent).
keep_going_force() {
  [[ $keep_going == yes ]] || return 1
  if jq -e --arg c "$1" --arg k "$2" --arg a "$3" 'select(.forced and .cause == $c and .case == $k and .action == $a)' "$defects" > /dev/null 2>&1; then
    end_reason="$4; forced once before at this case and action, so the run stops"
    return 1
  fi
  if (( $(forced_count) >= max_forced )); then
    end_reason="keep_going_spent: $max_forced forced continuations taken; this stop: $1: $4"
    end_cause=keep_going_spent
    return 1
  fi
  defect "$1" "$2" "$3" "$4" "${5:-null}" true "$6"
  echo "  keep-going: $1 at $3 in $2: $6 (rule:keep-going, forced $(forced_count) of $max_forced)"
}

# Unstick (D147). Under autopilot, before a stop for any cause but a cap or a crashed tool, the
# node run.unstick (class unstick, SDC_LLM_MODEL_UNSTICK, default claude-opus-5-5) chooses one of
# the legal moves a program lists for the stopped state (driver/src/bin/sdc-unstick.rs: rerun a
# writer, re-ask a decision, take a default, approve, end through a decision-only outcome, stop),
# with a diagnosis of what is missing. The move is carried out through sdc-drive commands as
# agent:unstick; each use is a line in defects.jsonl (unstick: true, the move, diagnosis,
# defect_class). At most 6 moves per run, each cause at a case and action once: then
# unstick_spent. Off with SDC_UNSTICK=off, and where the adapter has no model (stub, dry). An
# invalid reply after 2 retries falls back to keep-going when it is on, else the stop. Whether it
# is on, its adapter and model are set before run-settings.jsonl (`unstick`).
unstick_bin=${SDC_UNSTICK_BIN:-$sdc/driver/target/debug/sdc-unstick}
max_unstuck=6
unstick_kind="" unstick_answer="" unstick_tried="" rejection_recorded="" round_last=null
unstuck_count() { [[ -s $defects ]] || { echo 0; return; }; jq -s '[.[] | select(.unstuck == true)] | length' "$defects"; }
# unstick <cause> <case> <action key> <reason> <kind> <request JSON|null> <case dir>: 0 when a move
# other than stop was chosen (unstick_kind and unstick_answer say which; rerun, reask and end are
# carried out here, approve and default by the caller); 1 when the run stops.
unstick() {
  local cause=$1 ucase=$2 uact=$3 why=$4 ukind=$5 ureq=${6:-null} udir=$7 n rec attempt=1 invalid="" res mv tgt cap
  unstick_kind="" unstick_answer=""
  [[ $unstick_on == yes ]] || return 1
  unstick_tried=yes
  if jq -e --arg c "$cause" --arg k "$ucase" --arg a "$uact" 'select(.unstuck == true and .cause == $c and .case == $k and .action == $a)' "$defects" > /dev/null 2>&1; then
    end_cause=unstick_spent end_reason="unstick_spent: $cause at $uact in $ucase was unstuck once before; this stop: $why"
    return 1
  fi
  if (( $(unstuck_count) >= max_unstuck )); then
    end_cause=unstick_spent end_reason="unstick_spent: $max_unstuck moves of run.unstick taken; this stop: $cause: $why"
    return 1
  fi
  n=$(( $(find "$out/unstick" -mindepth 1 -maxdepth 1 -type d 2>/dev/null | wc -l) + 1 ))
  rec=$out/unstick/$(printf '%03d' "$n")-$cause
  mkdir -p "$rec"
  jq -n --arg c "$cause" --arg why "$why" --arg k "$ucase" --arg d "$udir" --arg a "$uact" --arg kind "$ukind" --argjson req "$ureq" \
    --argjson rr "$([[ $rejection_recorded == yes ]] && echo true || echo false)" --argjson pos "$(jq -c '.position // null' <<< "$round_last" 2>/dev/null || echo null)" \
    '{cause: $c, reason: $why, case: $k, case_dir: $d, action: $a, kind: $kind, request: $req, rejection_recorded: $rr, position: $pos}' > "$rec/stop.json"
  "$canon" evaluate --ir "$udir/ir.json" --case "$udir/case.json" --evidence "$udir/evidence" --authority "$udir/authority.json" \
    --decisions "$udir/decisions.json" --at "$(now)" > "$rec/decision.json" 2>/dev/null || echo null > "$rec/decision.json"
  "$unstick_bin" moves --stop "$rec/stop.json" > "$rec/moves.json" || return 1
  while :; do
    "$unstick_bin" prompt --stop "$rec/stop.json" --moves "$rec/moves.json" --decision "$rec/decision.json" --record "$rec" \
      --attempt "$attempt" --invalid "$invalid" || return 1
    rm -f "$rec/cwd/out/unstick.json" "$rec/structured-output.json"
    jq -n --argjson attempt "$attempt" --arg model "$unstick_model" --arg effort "$(jq -r .unstick.effort <<< "$efforts")" --arg rec "$rec" --arg case "$ucase" --arg case_dir "$udir" '{
      model: $model, effort: $effort, prompt_file: ($rec + "/prompt.md"), cwd: ($rec + "/cwd"), record_dir: $rec, node: "run.unstick",
      attempt: $attempt, json_schema: ($rec + "/reply-schema.json"), case: $case, case_dir: $case_dir,
      operations: ["dir.list", "file.read", "file.write"],
      max_turns: ($ENV.SDC_UNSTICK_MAX_TURNS // "12" | tonumber),
      max_budget_usd: ($ENV.SDC_UNSTICK_MAX_BUDGET_USD // "1.00" | tonumber)}' > "$rec/request.json"
    "$unstick_adapter_bin" "$rec/request.json" > "$rec/response.json" 2> "$rec/stderr.txt"
    [[ -s $rec/structured-output.json ]] && cp "$rec/structured-output.json" "$rec/cwd/out/unstick.json"
    res=$("$unstick_bin" validate --moves "$rec/moves.json" --reply "$rec/cwd/out/unstick.json") || res='{"valid": false, "why": "sdc-unstick validate failed"}'
    [[ $(jq -r .valid <<< "$res") == true ]] && break
    invalid=$(jq -r '.why // "invalid"' <<< "$res")
    if (( attempt > 2 )); then
      jq --argjson a "$attempt" '. + {attempts: $a}' <<< "$res" > "$rec/result.json"
      echo "  run.unstick: reply refused by the check after 2 retries ($invalid): no move"
      defect_line "$cause" "$ucase" "$uact" "$why" "$res" "run.unstick: invalid reply after 2 retries: $invalid" \
        "$(jq -cn --arg rec "${rec#"$out"/}" --argjson a "$attempt" --arg why "$invalid" '{unstick: true, unstuck: false, move: null,
          invalid: $why, attempts: $a, record: $rec}')"
      return 1
    fi
    echo "  run.unstick: reply refused by the check ($invalid): asked again ($attempt of 2 retries)"
    attempt=$((attempt + 1))
  done
  jq --argjson a "$attempt" '. + {attempts: $a}' <<< "$res" > "$rec/result.json"
  mv=$(jq -c .move <<< "$res") tgt=$(jq -r '.move.target // ""' <<< "$res") cap=$(jq -r '.move.capability // ""' <<< "$res")
  unstick_kind=$(jq -r .kind <<< "$mv") unstick_answer=$(jq -r '.answer // ""' <<< "$mv")
  echo "  run.unstick: $(jq -r .id <<< "$mv") ($(jq -r .defect_class <<< "$res")): $(jq -r .diagnosis <<< "$res")"
  local line
  line=$(jq -c --arg rec "${rec#"$out"/}" '{unstick: true, unstuck: (.move.kind != "stop"), move: .move.id, diagnosis, defect_class,
    reasons, attempts, record: $rec}' "$rec/result.json")
  if [[ $unstick_kind == stop ]]; then
    defect_line "$cause" "$ucase" "$uact" "$why" "$res" "run.unstick chose stop" "$line"
    return 1
  fi
  case $unstick_kind in
    rerun_writer)
      d "$udir" allow --at "$(now)" --action "$tgt" --principal agent:unstick ;;
    reask)
      mkdir -p "$out/unstick-notes"
      jq -r .diagnosis "$rec/result.json" > "$out/unstick-notes/$ucase.$tgt"
      code=0
      if [[ $ukind == finding ]]; then
        yq -i "(.findings[] | select(.id == \"$tgt\")) |= del(.refused_decision)" "$udir/work/findings.yaml" || code=1
      fi ;;
    end)
      # The refusal as the protocol represents it: the capability denied when it was never
      # decided; else the decider's rejection recorded (if it was not) as the case's last step.
      if ! jq -e --arg c "$cap" 'any(.[]; .capability == $c)' "$udir/authority.json" > /dev/null 2>&1; then
        d "$udir" approve --at "$(now)" --capability "$cap" --principal agent:unstick --deny
      elif [[ $rejection_recorded != yes ]]; then
        d "$udir" approve --at "$(now)" --capability "$cap" --principal "decider:$decider" --action "$(jq -r .action <<< "$ureq")" \
          --result rejected --reasons "${rework_file:-$rec/result.json}"
      else
        code=0
      fi
      [[ $code == 0 ]] && d "$udir" decide --at "$(now)" --decision "$(jq -r '.move.decision' <<< "$res")" --outcome "$tgt" \
        --principal agent:unstick --after-denial "$cap" --reasons "$rec/result.json"
      [[ $code == 0 ]] && d "$udir" close --at "$(now)" --outcome "$tgt" --principal agent:unstick ;;
    *) code=0 ;;
  esac
  if [[ $code != 0 ]]; then
    end_reason="$why; run.unstick chose $(jq -r .id <<< "$mv"), which sdc-drive refused: $(tail -n 1 <<< "$last" | cut -c1-200)"
    defect_line "$cause" "$ucase" "$uact" "$why" "$res" "refused by sdc-drive" "$(jq -c '.unstuck = false' <<< "$line")"
    return 1
  fi
  defect_line "$cause" "$ucase" "$uact" "$why" "$res" "$(jq -r .says <<< "$mv")" "$line"
  return 0
}
# A defect line with extra fields (unstick's): <cause> <case> <action> <reason> <decision> <continuation> <extra JSON>
defect_line() {
  defect "$1" "$2" "$3" "$4" "$5" false "$6"
  local t
  t=$(mktemp "$out/.defects.XXXXXX")
  { head -n -1 "$defects"; tail -n 1 "$defects" | jq -c --argjson x "$7" '. + $x + (if $x.unstuck then {principal: "agent:unstick"} else {} end)'; } > "$t" && mv "$t" "$defects"
}
trap 'rc=$?; autopilot_end "$rc"; save; replay_seal; watch_stop' EXIT

dry_lines() { { wc -l < "${SDC_LLM_DRY_LOG:-/dev/null}"; } 2>/dev/null || echo 0; }
print_dry() { [[ -n ${SDC_LLM_DRY_LOG:-} && -f $SDC_LLM_DRY_LOG ]] && tail -n +"$(($1 + 1))" "$SDC_LLM_DRY_LOG" | sed 's/^/  /'; true; }

# Close the root through an outcome, write decision.json, status.json and replay.json; exit with
# replay's code. A run that closes completed, whichever outcome: status.json (sdc.run-status/1)
# says closed, the outcome and whether it is the delivering one (`operating`), with end_note (why
# the tree ended otherwise, e.g. a withheld release and the decider's first reason).
end_note=""
finish() { # <outcome> [principal that closed it]
  if [[ -n ${2:-} ]]; then d "$root" close --at "$(now)" --outcome "$1" --principal "$2"; else d "$root" close --at "$(now)" --outcome "$1"; fi
  # A keep-going run says how many continuations were forced: it is evidence for defects, never a
  # clean end-to-end pass.
  # So does a run that run.unstick moved on: it closed unattended, its diagnoses are defects.
  local forced="" unstuck="" tag=""
  [[ $keep_going == yes || -s $defects ]] && forced=$(forced_count)
  [[ $unstick_on == yes || -s $defects ]] && unstuck=$(unstuck_count)
  [[ $keep_going != yes && $forced == 0 ]] && forced=""
  [[ $unstick_on != yes && $unstuck == 0 ]] && unstuck=""
  [[ -n $forced ]] && tag="forced $forced"
  [[ -n $unstuck ]] && tag="${tag:+$tag, }unstuck $unstuck"
  echo "closed ${root##*/}: $(jq -c '.terminated' <<< "$last")${end_note:+ ($end_note)}${2:+ by $2}${tag:+ ($tag)} (exit $code)"
  [[ $code == 0 ]] || exit 4
  jq -n --arg at "$(now)" --arg o "$1" --arg note "$end_note" --arg by "${2:-}" --arg forced "$forced" --arg unstuck "$unstuck" '{format: "sdc.run-status/1", at: $at, state: "closed",
    outcome: $o, delivered: ($o == "operating" or $o == "fixed"), note: (if $note == "" then null else $note end),
    closed_by: (if $by == "" then null else $by end)} + (if $forced == "" then {} else {forced: ($forced | tonumber)} end)
    + (if $unstuck == "" then {} else {unstuck: ($unstuck | tonumber)} end)' > "$out/status.json"
  "$canon" evaluate --ir "$root/ir.json" --case "$root/case.json" --evidence "$root/evidence" \
    --authority "$root/authority.json" --decisions "$root/decisions.json" --at "$(now)" > "$out/decision.json"
  d "$root" replay
  echo "$last" > "$out/replay.json"
  echo "replay exit $code: $(jq -c 'map({case, state_matches_log, decision_matches_log})' "$out/replay.json")"
  exit "$code"
}

waiting() {
  if [[ $autonomy == A5 ]]; then
    end_case=$(jq -r '(.case_dir // "") | split("/") | last' <<< "$last")
    end_action=$(jq -r '.next[0].action // ""' <<< "$last")
    # A handoff with nothing for the decider: run.unstick, unless it was asked about this stop.
    if [[ -z $unstick_tried && -z $end_cause ]] &&
       unstick decision_not_takeable "$end_case" "$end_action" "$end_action in $end_case: decision_not_takeable: ${wait_why:-the decider of this run cannot take it}" \
         handoff null "$(jq -r '.case_dir // ""' <<< "$last")"; then
      return 0
    fi
    if [[ $end_cause != keep_going_spent && $end_cause != unstick_spent ]]; then
      end_cause=decision_not_takeable
      end_reason="$end_action in $end_case: decision_not_takeable: ${wait_why:-the decider of this run cannot take it}; it waits on a fix of that decision path, then --reopen"
    fi
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
  local req=$1 key rec n reply before attempt=1 why="" kind fallback="" cls side note
  # A re-ask by run.unstick carries its diagnosis as the request's note, once.
  note=$out/unstick-notes/$(jq -r '(.case_dir | split("/") | last) + "." + (.question.id // .finding.id // .action)' <<< "$req")
  if [[ -f $note ]]; then
    req=$(jq -c --rawfile n "$note" '.note = ($n | rtrimstr("\n"))' <<< "$req")
    rm -f "$note"
  fi
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
  dec_principal=decider:$decider taker=decider:$decider
  local llm=no
  [[ $decider == model || $decider == requester ]] && llm=yes
  # How the log names it: decider:model with the model id beside it (D149).
  local who=decider:$decider
  [[ $decider == model ]] && who+=" ($model)"
  req=$(jq -c --arg c "$cls" --arg s "$side" --arg a "$autonomy" --arg l "$out/decisions.jsonl" '. + {class: $c, side: $s,
    autonomy: (if $a == "" then null else $a end), decisions_log: $l}' <<< "$req")
  if [[ $escalate_mode != final && -f $out/decisions.jsonl ]] &&
     jq -e --arg k "$key" --arg p "decider:$decider" 'select((.principal == $p or ($p == "decider:model" and .principal == "decider:haiku"))
       and .decision == "escalate"
       and ([.case, .action, (.question // .finding // ""), .revision] | join("|")) == $k)' "$out/decisions.jsonl" > /dev/null 2>&1; then
    echo "  $who escalated $key before: a person decides"
    return 0
  fi
  line() { # $1 decision, $2 extra object
    local x=${2:-}
    [[ -n $x ]] || x='{}'
    jq -cn --arg at "$(now)" --arg p "${line_principal:-decider:$decider}" --arg rd "$rule_declined" --arg d "$1" --argjson q "$req" --argjson r "$reply" --argjson x "$x" '{
      time: $at, case: ($q.case_dir | split("/") | last), type: $q.type,
      class: ({question: "scope_changing_answer", finding: "finding"}[$q.type] // $q.type), action: $q.action,
      question: ($q.question.id // null), finding: ($q.finding.id // null), capability: $q.capability, outcome: $q.outcome,
      revision: $q.revision, decision: $d, answer: $r.answer, principal: $p, reasons: $r.reasons,
      cites: $r.cites, citations: ($r.citations // null), model: $r.model, effort: $r.effort, adapter: $r.adapter, tokens: $r.usage,
      cost_usd: $r.cost_usd, duration_ms: $r.duration_ms, valid: $r.valid, invalid_reason: $r.invalid_reason,
      schema_constrained: ($r.schema_constrained // false), dry: ($r.dry // false), record: $r.record, side: ($q.side // null),
      no_ground_in_goals: $r.no_ground_in_goals, strength: ($r.strength // null), grounds: ($r.grounds // null)} + $x
      + (if $r.rule then {rule: $r.rule} else {} end) + (if $rd == "" then {} else {rule_declined: $rd} end)' >> "$out/decisions.jsonl"
  }
  # An approval whose program checks passed on the exact revision is taken by rule:autopilot
  # (sdc-reply rule-approval, the guards in rule-ends.json `by_rule`): recorded with the rule, the
  # checks and the evidence ids, no decider call. Only under A5, only for the model decider (not
  # the requester agent, whose reading is the goals'), only when SDC_RULE_APPROVALS is not off.
  # A check that did not pass, no record at the revision, or an earlier rejection: the decider is
  # asked as before, the line carrying `rule_declined`.
  local rule_declined="" line_principal=""
  if [[ $kind == approval && $autonomy == A5 && $rule_approvals == on && $decider == model ]]; then
    local rf res
    rf=$out/rule-approvals/$(tr '|/' '..' <<< "$key")
    mkdir -p "$out/rule-approvals"
    jq -c . <<< "$req" > "$rf.request.json"
    res=$("$reply_bin" rule-approval --request "$rf.request.json" --ends "$ex/rule-ends.json") ||
      res='{"take": false, "covered": true, "why": "sdc-reply rule-approval failed"}'
    printf '%s\n' "$res" > "$rf.json"
    if [[ $(jq -r .take <<< "$res") == true ]]; then
      reply=$(jq -c .decision <<< "$res")
      line_principal=rule:autopilot
      line approve '{"rule_taken": true}'
      dec=approve ans="" dec_principal=rule:autopilot taker=rule:autopilot
      echo "  rule:autopilot approve $key: $(jq -r '.reasons | join(" ")' <<< "$reply")"
      return 0
    fi
    if [[ $(jq -r .covered <<< "$res") == true ]]; then
      rule_declined=$(jq -r .why <<< "$res")
      echo "  rule:autopilot does not take $key ($rule_declined): the decider is asked"
    fi
  fi
  while :; do
    if [[ $llm == yes ]]; then
      n=$(( $(find "$out/decider" -mindepth 1 -maxdepth 1 -type d 2>/dev/null | wc -l) + 1 ))
      (( n <= max_decider )) || { end_cause=decider_budget_spent end_action=$(jq -r .action <<< "$req") end_case=${key%%|*}
        end_reason="$end_action in $end_case: decider_budget_spent ($max_decider LLM decider calls, SDC_MAX_DECIDER_CALLS; the last request: $(jq -r '.question.id // .finding.id // .action' <<< "$req")): it waits on a higher cap or a fix of what spent it, then --reopen"
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
    replay_check
    [[ $(jq -r .valid <<< "$reply") == false && $llm == yes ]] || break
    why=$(jq -r '.invalid_reason // "invalid"' <<< "$reply")
    (( attempt <= ${SDC_DECIDER_RETRIES:-2} )) || break
    line invalid_reply "$(jq -cn --argjson a "$attempt" '{attempt: $a}')"
    echo "  $who reply refused by the check ($why): asked again ($attempt of ${SDC_DECIDER_RETRIES:-2} retries)"
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
        reply=$(jq -c --arg a "$ans" --arg why "$why" --arg r "${SDC_DECIDER_RETRIES:-2}" '.answer = $a | .valid = true
          | .reasons = ["invalid decider reply after \($r) retries (\($why)): the default taken"]' <<< "$reply") ;;
      *)
        local failing
        failing=$(checks_failing "$req")
        if [[ $failing == "[]" ]]; then
          fallback=approved_on_checks dec=approve
          reply=$(jq -c --arg why "$why" --arg r "${SDC_DECIDER_RETRIES:-2}" '.valid = true | .reasons = ["invalid decider reply after \($r) retries (\($why)): approved on the checks, every claim this decision turns on is true and no other obligation is open"]' <<< "$reply")
        else
          fallback=rejected_on_checks dec=reject
          reply=$(jq -c --arg why "$why" --arg r "${SDC_DECIDER_RETRIES:-2}" --argjson f "$failing" '.valid = true | .reasons = ["invalid decider reply after \($r) retries (\($why)): rejected on the checks: \($f | join(", "))"]' <<< "$reply")
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
  echo "  $who $dec $key$( [[ -n $ans ]] && echo " answer: $ans")${fallback:+ ($fallback)}: $(jq -r '.reasons | join(" ")' <<< "$reply")"
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
      local item=""
      [[ $kind == question || $kind == finding ]] && item=" $(jq -r '.question.id // .finding.id' <<< "$req")"
      # run.unstick first: approve and default are taken here as agent:unstick; a rerun, a re-ask or
      # an end it carried out goes on in the next round.
      if unstick "$end_cause" "$end_case" "$end_action$item" "$end_reason" "$kind" "$req" "$(jq -r .case_dir <<< "$req")"; then
        end_cause="" end_reason="" rework_gap="" rework_file="" rejection_deferred=""
        case $unstick_kind in
          approve) dec=approve ans="" taker=agent:unstick ;;
          default) dec=approve ans=$unstick_answer taker=agent:unstick ;;
          *) dec="" reworked=yes ;;
        esac
        return 0
      fi
      # Keep-going: the approval or close is taken by rule:keep-going; a question or a finding
      # takes its default (none: nothing to take without inventing it, the run stops). An approval
      # whose rejection is recorded is settled and cannot be taken: it stops.
      local dflt=""
      [[ $kind == question || $kind == finding ]] && dflt=$(jq -r ".${kind}.default // \"\"" <<< "$req")
      if [[ $end_cause != keep_going_spent && $end_cause != unstick_spent ]] && [[ $kind != question && $kind != finding || -n $dflt ]] &&
         [[ $kind != approval || $rejection_recorded != yes ]] &&
         keep_going_force "$end_cause" "$end_case" "$end_action$( [[ $kind == question || $kind == finding ]] && jq -r '" " + (.question.id // .finding.id)' <<< "$req")" \
           "$end_reason" "$reply" "$( [[ -n $dflt ]] && echo "the default taken: $dflt" || echo "approved")"; then
        end_cause="" end_reason="" rework_gap="" rework_file="" rejection_deferred="" dec=approve ans=$dflt taker=rule:keep-going
        return 0
      fi
      record_deferred
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
# goes on; else sets end_cause and end_reason: rework_spent on the rejection after the last rework (rework_limit + 1, by default the third;
# the approval and every reason), rejected_without_rework_path when the approval cannot record a
# rejection or no writer is sent back by it (rework_gap says which).
reworked="" rework_gap="" taker="" rejection_deferred=""
# The rework cap this leg recorded (D184): the one value the rework prompt reads too.
rework_limit=$(jq -rs 'last.rework_limit.value' "$out/run-settings.jsonl")
# The third rejection rework() left unrecorded, recorded as the decider's when the run stops.
record_deferred() {
  local cdir cap act
  [[ -n $rejection_deferred && -n $rework_file ]] || return 0
  read -r cdir cap act <<< "$rejection_deferred"
  rejection_deferred=""
  d "$cdir" approve --at "$(now)" --capability "$cap" --principal "decider:$decider" --action "$act" --result rejected --reasons "$rework_file"
  echo "  recorded the rejection of $act as decider:$decider before the stop: exit $code"
}
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
  rework_file=$file rejection_recorded="" rejection_deferred=""
  # Keep-going and unstick: the third rejection is not recorded in the case yet (a recorded
  # rejection settles the approval, and neither could take it then); a run that stops anyway
  # records it before the stop (record_deferred).
  if (( n > rework_limit )) && [[ $keep_going == yes || $unstick_on == yes ]]; then
    rejection_deferred="$cdir $cap $act"
    end_cause=rework_spent
    end_reason="$act in ${cdir##*/}: rework_spent: decider:$decider rejected $subj $n times: $(jq -r --argjson cur "$(jq -c '.reasons' "$file")" \
      '[(.[] | .reasons), $cur] | to_entries | map("\(.key + 1): \(.value | join(" "))") | join("; ")' <<< "$prior"); it waits on a fix of the writer or of the reasons, then --reopen"
    return 1
  fi
  d "$cdir" approve --at "$(now)" --capability "$cap" --principal "decider:$decider" --action "$act" --result rejected --reasons "$file"
  if [[ $code != 0 ]]; then
    rework_gap="\`$act\` cannot record a rejected result ($(tail -n 1 <<< "$last" | sed 's/^sdc-drive: //'))"
    return 1
  fi
  rejection_recorded=yes
  echo "  recorded the rejection of $act at $rev as decider:$decider (rejection $n; at most $rework_limit are reworked)"
  if (( n > rework_limit )); then
    end_cause=rework_spent
    end_reason="$act in ${cdir##*/}: rework_spent: decider:$decider rejected $subj $n times: $(jq -r --argjson cur "$(jq -c '.reasons' "$file")" \
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
# The ends by rule are data (driver/examples/sdc/rule-ends.json), read by sdc-check-routes too.
declare -A refusal_end=()
while read -r a dn oc; do refusal_end[$a]="$dn $oc"; done < <(jq -r '.approvals | to_entries[] | select(.value.reject.decision != null)
  | "\(.key | split("/")[1]) \(.value.reject.decision) \(.value.reject.outcome)"' "$ex/rule-ends.json")
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
# default or `changed`, `answered_by: decider:<kind>`). So is each scope-changing answer no one
# with the right to give it gave (`answered_by` empty or non-interactive, check-questions exit 3):
# put to the decider as a question, never as an approval of scope.answer (R2), so a reject is
# refused as for any question (R1). Returns 0 when all were answered, 1 when one waits on a
# person, 2 when none is open or unattributed; exits on a refusal.
answer_questions() {
  local cdir=$1 act=$2 pos=$3 qf=$1/work/questions.yaml found=no rev q st qs=()
  rev=$(jq -r '.artifacts.questions.revision // "none"' "$cdir/case.json")
  mapfile -t qs < <(yq -o=json -I=0 '.questions[] | select(.scope_changing == true and (.status == "open"
    or (.answered_by // "") == "" or .answered_by == "non-interactive"))' "$qf" 2>/dev/null)
  for q in "${qs[@]}"; do
    [[ -n $q ]] || continue
    found=yes
    decide "$(jq -cn --arg c "$cdir" --arg a "$act" --arg rev "$rev" --argjson q "$q" --argjson pos "$pos" \
      '{type: "question", case_dir: $c, action: $a, capability: "scope.answer", subject: "questions",
        path: "questions.yaml", revision: $rev, question: $q, outcome: null, position: $pos}')"
    [[ $reworked == yes ]] && { reworked=""; continue; }
    [[ -n $dec ]] || return 1
    st=changed
    [[ $ans == "$(jq -r .default <<< "$q")" ]] && st=accepted
    ANS=$ans yq -i "(.questions[] | select(.id == \"$(jq -r .id <<< "$q")\")) |= (.answer = strenv(ANS)
      | .status = \"$st\" | .answered_by = \"$taker\")" "$qf"
  done
  [[ $found == yes ]] || return 2
}

# The findings in case $1 that wait for a decision (amend.upstream's handoff, action $2, position
# $3), one decider decision each: the decision is written as a person's is (`decision:` on the
# finding) with `decided_by: decider:<kind>`, and amend.upstream applies it on the next round
# through the same program check as the agent's amendments. A finding whose decider decision
# that check refused (`refused_decision`) is put to the decider again under autopilot with the
# refusal, up to 3 decisions in all (R3); then run.unstick, keep-going, or it waits (a person;
# under autopilot the stop decision_not_takeable). Returns as answer_questions.
decide_findings() {
  local cdir=$1 act=$2 pos=$3 ff=$1/work/findings.yaml found=no rev f fid fs=()
  rev=$(jq -r '.artifacts.findings.revision // "none"' "$cdir/case.json")
  mapfile -t fs < <(yq -o=json -I=0 '.findings[] | select(.status == "needs_person" and (.decision // "") == "")' "$ff" 2>/dev/null)
  for f in "${fs[@]}"; do
    [[ -n $f ]] || continue
    found=yes
    fid=$(jq -r .id <<< "$f")
    # A program finding raised again after the decision on it was applied (sdc-findings `reraised`):
    # a defect of that apply, never a new question (runs/trial-3-small-sonnet-1: p-trace-r5 asked
    # 8 times until the decider budget was spent). The run stops finding_reraised.
    if jq -e '.reraised' <<< "$f" > /dev/null; then
      wait_why="$fid raised again after $(jq -r '.reraised.decided_by // "?"' <<< "$f")'s decision \"$(jq -r '.reraised.decision // ""' <<< "$f" | cut -c1-160)\" was applied ($(jq -r '.reraised.says // ""' <<< "$f" | cut -c1-160))"
      if [[ $autonomy == A5 ]]; then
        end_cause=finding_reraised end_action=$act end_case=$(basename "$cdir")
        end_reason="$act in $end_case: finding_reraised: $wait_why: the apply of that decision did not take effect; it waits on a fix of that apply, then --reopen"
        echo "$end_reason"
        exit 7
      fi
      echo "  $wait_why: a person decides"
      return 1
    fi
    if jq -e '.refused_decision' <<< "$f" > /dev/null; then
      wait_why="$fid: the program check refused $(jq -r .refused_decision.decided_by <<< "$f")'s decision ($(jq -r .refused_decision.why <<< "$f"))"
      # R3, under autopilot: a decider's decision the check refused is put to the decider again
      # with the refusal (`refusal` in its request), until it has decided the finding 3 times; then
      # the stop. Below A5 a person decides it, as before.
      local nd
      nd=$(jq -s --arg c "${cdir##*/}" --arg f "$fid" '[.[] | select(.case == $c and .finding == $f and .decision == "approve")] | length' \
        "$out/decisions.jsonl" 2>/dev/null || echo 0)
      if [[ $autonomy == A5 && $(jq -r '.refused_decision.decided_by // ""' <<< "$f") == decider:* ]] && (( ${nd:-0} < 3 )); then
        echo "  $wait_why: the decider is asked again with the refusal (decision $((nd + 1)) of at most 3)"
        decide "$(jq -cn --arg c "$cdir" --arg a "$act" --arg rev "$rev" --argjson f "$f" --argjson pos "$pos" \
          '{type: "finding", case_dir: $c, action: $a, capability: null, subject: "findings",
            path: "findings.yaml", revision: $rev, question: null, finding: ($f | del(.refused_decision)),
            refusal: $f.refused_decision, outcome: null, position: $pos}')"
        [[ $reworked == yes ]] && { reworked=""; continue; }
        [[ -n $dec ]] || return 1
        ANS=$ans yq -i "(.findings[] | select(.id == \"$fid\")) |= (.decision = strenv(ANS) | .decided_by = \"$taker\"
          | del(.refused_decision))" "$ff"
        continue
      fi
      # run.unstick first: the default as agent:unstick, or the decider asked again (done there).
      if unstick decision_not_takeable "${cdir##*/}" "$act $fid" "$act in ${cdir##*/}: decision_not_takeable: $wait_why" finding \
           "$(jq -cn --arg c "$cdir" --arg a "$act" --argjson f "$f" '{type: "finding", case_dir: $c, action: $a, capability: null, finding: $f}')" "$cdir"; then
        if [[ $unstick_kind == default ]]; then
          ANS=$unstick_answer yq -i "(.findings[] | select(.id == \"$fid\")) |= (.decision = strenv(ANS) | .decided_by = \"agent:unstick\"
            | del(.refused_decision))" "$ff"
        fi
        continue
      fi
      [[ $end_cause == unstick_spent ]] && return 1
      # Keep-going: the finding's default, taken by rule:keep-going and applied by amend.upstream
      # through the same check; a default that check refused already (rule:keep-going's own) stops.
      local dflt
      dflt=$(jq -r '.default // ""' <<< "$f")
      if [[ -n $dflt && $(jq -r .refused_decision.decided_by <<< "$f") != rule:keep-going ]] &&
         keep_going_force decision_not_takeable "${cdir##*/}" "$act $fid" \
           "$act in ${cdir##*/}: decision_not_takeable: $wait_why" "$(jq -c .refused_decision <<< "$f")" "the default taken: $dflt"; then
        ANS=$dflt yq -i "(.findings[] | select(.id == \"$fid\")) |= (.decision = strenv(ANS) | .decided_by = \"rule:keep-going\"
          | del(.refused_decision))" "$ff"
        continue
      fi
      if [[ $autonomy == A5 ]]; then echo "  $wait_why"; else echo "  $wait_why: a person decides"; fi
      return 1
    fi
    decide "$(jq -cn --arg c "$cdir" --arg a "$act" --arg rev "$rev" --argjson f "$f" --argjson pos "$pos" \
      '{type: "finding", case_dir: $c, action: $a, capability: null, subject: "findings",
        path: "findings.yaml", revision: $rev, question: null, finding: $f, outcome: null, position: $pos}')"
    [[ $reworked == yes ]] && { reworked=""; continue; }
    [[ -n $dec ]] || return 1
    ANS=$ans yq -i "(.findings[] | select(.id == \"$fid\")) |= (.decision = strenv(ANS) | .decided_by = \"$taker\")" "$ff"
  done
  [[ $found == yes ]] || return 2
}

# Every pending decision of the stop in $last, through the decider. Returns 0 when all were
# taken; exits on a refusal; returns 1 when one waits on a person.
declare -A taken
decide_all() {
  local cdir pos n act cap subj path rev key rc by
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
    rc=2 by=""
    [[ $act == scope.answer ]] && { rc=0; answer_questions "$cdir" "$act" "$pos" || rc=$?; by=$dec_principal; }
    (( rc == 1 )) && return 1
    if (( rc == 2 )); then
      decide "$(jq -cn --arg c "$cdir" --arg a "$act" --arg cap "$cap" --arg s "$subj" --arg p "$path" --arg rev "$rev" \
        --argjson pos "$pos" '{type: "approval", case_dir: $c, action: $a, capability: $cap, subject: $s,
          path: $p, revision: $rev, question: null, outcome: null, position: $pos}')"
      # A reworked rejection: the writer runs in the next round, the approval is asked again after it.
      [[ $reworked == yes ]] && { reworked=""; return 0; }
      [[ -n $dec ]] || return 1
      by=$taker
    fi
    taken[$key]=1
    if jq -e '.command | test("--action ")' <<< "$n" > /dev/null; then
      d "$cdir" approve --at "$(now)" --capability "$cap" --principal "$by" --action "$act"
    else
      d "$cdir" approve --at "$(now)" --capability "$cap" --principal "$by"
    fi
    echo "  approve $act as $by: exit $code"
    [[ $code == 0 ]] || { jq -r '.message // .' <<< "$last" 2>/dev/null || echo "$last"; exit 4; }
  done < <(jq -c '.next[]' <<< "$last")
}

for round in $(seq 1 40); do
  dry_before=$(dry_lines)
  t0=$(date +%s) s0=$(now)
  d "$root" run --at "$s0" --max-llm-calls "$max_llm" --max-wall-seconds "$max_wall"
  round_last=$last unstick_tried=""
  # A re-driven run whose recording diverged: the round ends there (replay_check below).
  [[ -z ${SDC_REPLAY:-} || ! -f $out/replay/diverged.json ]] || last=$(jq -c '{verdict: "recording_diverged", case_dir: .place.case_dir, message: .summary}' "$out/replay/diverged.json")
  verdict=$(jq -r '.verdict // "error"' <<< "$last" 2>/dev/null || echo error)
  # One line per round for the KPI L2 (autonomous run time between human interventions).
  jq -cn --arg s "$s0" --arg e "$(now)" --argjson t0 "$t0" --argjson t1 "$(date +%s)" --arg v "$verdict" \
    --arg c "$(jq -r '.case_dir // ""' <<< "$last" 2>/dev/null | sed "s|$store/||")" '{started: $s, ended: $e,
    started_epoch: $t0, ended_epoch: $t1, seconds: ($t1 - $t0), verdict: $v, case_dir: $c}' >> "$out/rounds.jsonl"
  echo "round $round: $verdict $(jq -r '.case_dir // ""' <<< "$last" 2>/dev/null | sed "s|$store/||"), LLM calls so far $(jq -r '.llm_calls // "?"' <<< "$last" 2>/dev/null)"
  replay_check
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
        [[ -n $dec ]] && finish "$outcome" "${taker/#decider:*/}"
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
      # run.unstick first, for any verdict but a cap (*_budget_spent, wall_time_spent) or an error.
      if [[ $verdict == writer_spent || $verdict == writer_not_called || $verdict == stuck ]]; then
        verdict_end 4
        wdir=$(jq -r '.case_dir // ""' <<< "$last")
        ukind=stuck
        [[ $verdict == writer_* ]] && ukind=writer
        if [[ -n $wdir ]] && unstick "$end_cause" "$end_case" "$end_action" "$end_reason" "$ukind" null "$wdir"; then
          end_cause="" end_reason="" end_case="" end_action=""; continue
        fi
        last=$round_last
      fi
      # Keep-going: a spent or not-called writer gets one more allowance of its max_runs
      # (`sdc-drive allow`); its failing check records are fed back as on any retry.
      if [[ $verdict == writer_spent || $verdict == writer_not_called ]] && [[ $keep_going == yes && $end_cause != unstick_spent ]]; then
        verdict_end 4
        wdir=$(jq -r '.case_dir // ""' <<< "$last")
        if [[ -n $end_action && -n $wdir ]] &&
           keep_going_force "$end_cause" "$end_case" "$end_action" "$end_reason" "$(jq -c '(.spent // .not_called // [])[0] // null' <<< "$last")" \
             "one more allowance of $end_action's max_runs"; then
          d "$wdir" allow --at "$(now)" --action "$end_action" --principal rule:keep-going
          if [[ $code == 0 ]]; then end_cause="" end_reason="" end_case="" end_action=""; continue; fi
          end_reason="$end_action in $end_case: the allowance was refused: $(tail -n 1 <<< "$last")"
        fi
      fi
      exit 4 ;;
  esac
done
echo "40 rounds without an end"
exit 4
