#!/usr/bin/env bash
# Run one full SDC loop, idea to release outcome, with sdc-drive: stub LLM executors, real `ess`,
# real `aep` in the case tree's own planning store. The script plays the person: when the run
# stops awaiting authority or a person, it records the approval through `sdc-drive approve`, as
# principal $PRINCIPAL (default person:stand-in: the script stands in for the person, and its
# run-settings.jsonl says stand_in), then runs again. At the end it closes the root case through its one
# legitimate outcome, saves the final `canon evaluate` decision and replay-checks every case.
#
#   scripts/run-full-cycle.sh <name> <store-dir>
#
# Writes $RUNS/<name>/ (default runs/): run.log (every command and its output), decision.json
# (final canon evaluate of the root), replay.json, events/<case>.jsonl, and a copy of the case store.
#
# Late findings (docs/trial-2.md): SDC_STUB_FINDINGS=<fixture> makes the stub of the fixture's
# `node` raise its findings once. When the run waits on a person for amend.upstream, the scripted
# person decides each needs_person finding with FINDING_DECISION (`option:N` takes option N, any
# other text is the decision itself); without it the script stops there (exit 3).
# SCOPE_ANSWERS='{"qN": "answer"}': at the scope.answer stop the scripted person first changes
# those scope-changing answers in questions.yaml (`status: changed`), then takes scope.answer;
# with SCOPE_RESUME_FIRST=1 it resumes after the edit and takes scope.answer at the next stop.
# NO_YQ_STEP=<action>: at that first stop, after the person's edit, one `step --action <action>`
# runs with yq removed from PATH (an environment failure, as the live view's check in
# runs/trial-2-train).
set -euo pipefail
sdc=$(cd "$(dirname "$0")/.." && pwd)
name=$1 store=$2
source "$sdc/scripts/evaluate-cases.sh"
canon_resolve || exit 1
drive=${DRIVE:-$sdc/driver/target/debug/sdc-drive}
# The scripted person is a stand-in: its principal says so (person:stand-in), and run-settings.jsonl
# labels the run stand_in, an end-to-end test of the person path whose numbers are not eval numbers.
principal=${PRINCIPAL:-person:stand-in}
out=${RUNS:-$sdc/runs}/$name
ex=$sdc/driver/examples/sdc
rm -rf "$out" "$store"
mkdir -p "$out/events"
log=$out/run.log
: > "$log"
jq -cn --arg at "$(date -u +%Y-%m-%dT%H:%M:%SZ)" --arg p "$principal" '{format: "sdc.run-settings/1", at: $at,
  leg: "started", autonomy: "A0", decider: {kind: "person", model: null, escalate: "stop"}, adapter: "stub",
  stand_in: true, stand_in_by: "scripts/run-full-cycle.sh", stand_in_principal: $p}' > "$out/run-settings.jsonl"
minute=0
at() { printf "2026-11-20T%02d:%02d:00Z" $((12 + minute / 60)) $((minute % 60)); }

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

root=$store/IDEA-1
d "$root" init --protocol "$sdc/protocol/sdc.protocol.yaml" --id IDEA-1 --bindings "$ex/sdc.bindings.json"
cp "$ex/fixtures/idea.yaml" "$root/work/idea.yaml"
echo "# the person's idea: $(cat "$root/work/idea.yaml")" >> "$log"

for round in $(seq 1 30); do
  minute=$((minute + 1))
  d "$root" run --at "$(at)"
  if ! jq -e . <<< "$last" > /dev/null 2>&1; then
    echo "run-full-cycle: \`sdc-drive run\` printed no JSON (exit $code); its first line: $(head -n 1 <<< "$last")"
    exit 5
  fi
  verdict=$(jq -r '.verdict' <<< "$last")
  echo "round $round at $(at): $verdict $(jq -r '.case_dir // ""' <<< "$last")"
  case $verdict in
    terminable) break ;;
    awaiting_authority|awaiting_person)
      case_dir=$(jq -r '.case_dir' <<< "$last")
      if jq -e '[.next[]?.action] | index("amend.upstream")' <<< "$last" > /dev/null; then
        [[ -n ${FINDING_DECISION:-} ]] || { echo "  waits on a person for findings (no FINDING_DECISION)"; exit 3; }
        f=$case_dir/work/findings.yaml
        echo "\$ person decides in $f: $FINDING_DECISION" >> "$log"
        if [[ $FINDING_DECISION == option:* ]]; then
          N=${FINDING_DECISION#option:} P=$principal yq -i '(.findings[] | select(.status == "needs_person" and (.decision // "") == ""))
            |= (.decision = .options[env(N) - 1] | .decided_by = strenv(P))' "$f"
        else
          P=$principal yq -i '(.findings[] | select(.status == "needs_person" and (.decision // "") == ""))
            |= (.decision = strenv(FINDING_DECISION) | .decided_by = strenv(P))' "$f"
        fi
        echo "  person decided $(yq -r '[.findings[] | select(.status == "needs_person") | .id + ": " + .decision] | join("; ")' "$f")"
        continue
      fi
      for a in $(jq -r '.position.awaiting_person[], .position.approval_required[]' <<< "$last"); do
        minute=$((minute + 1))
        case $a in
          context.approve|vision.approve|roadmap.approve)
            d "$case_dir" approve --at "$(at)" --capability owner.approve --principal "$principal" --action "$a" ;;
          scope.answer)
            # SCOPE_ANSWERS={"qN": "answer", ...}: the person changes those answers in
            # questions.yaml (status changed), then takes scope.answer once (docs/trial-2.md).
            if [[ -n ${SCOPE_ANSWERS:-} && -z ${scope_answered:-} ]]; then
              scope_answered=yes
              q=$case_dir/work/questions.yaml
              echo "\$ person edits $q: $SCOPE_ANSWERS" >> "$log"
              yq -i "$(jq -r 'to_entries | map("(.questions[] | select(.id == \(.key | tojson))) |= (.answer = \(.value | tojson) | .status = \"changed\")") | join(" | ")' <<< "$SCOPE_ANSWERS")" "$q"
              echo "  person changed $(jq -r 'keys | join(" ")' <<< "$SCOPE_ANSWERS")"
              edited=yes
            fi
            # After the person's edit, not before: a check runs once per interview revision, so it
            # is admissible again only at the new revision the edit made.
            if [[ -n ${NO_YQ_STEP:-} && -z ${no_yq_done:-} ]]; then
              # runs/trial-2-train: the live view ran a check in an environment without yq.
              no_yq_done=yes
              nopath=$(tr ':' '\n' <<< "$PATH" | grep -vxF "$(dirname "$(command -v yq)")" | paste -sd:)
              echo "\$ (PATH without yq) sdc-drive --case-dir ${case_dir#"$store"/} step --action $NO_YQ_STEP" >> "$log"
              set +e
              res=$(PATH=$nopath "$drive" --canon "$canon" --case-dir "$case_dir" step --at "$(at)" --action "$NO_YQ_STEP" 2>&1)
              rc=$?
              set -e
              echo "$res" >> "$log"; echo "exit $rc" >> "$log"; echo >> "$log"
              echo "  $NO_YQ_STEP without yq: exit $rc, evidence $(jq -c '[.evidence[]?.result]' <<< "$res" 2>/dev/null || echo none)"
            fi
            # SCOPE_RESUME_FIRST=1: the person resumes after the edit and approves at the next stop.
            [[ -n ${edited:-} && -n ${SCOPE_RESUME_FIRST:-} ]] && { edited=; break; }
            d "$case_dir" approve --at "$(at)" --capability scope.answer --principal "$principal" --action "$a" ;;
          merge.approve) d "$case_dir" approve --at "$(at)" --capability change.merge --principal "$principal" --action "$a" ;;
          release.publish) d "$case_dir" approve --at "$(at)" --capability release.approve --principal "$principal" ;;
          *) echo "no approval rule for $a"; exit 1 ;;
        esac
        echo "  approved $a: exit $code"
        [[ $code == 0 ]] || exit 1
        break
      done ;;
    *) echo "run ended $verdict"; exit 1 ;;
  esac
done

outcome=$(jq -r '.position.legitimate_outcomes[0]' <<< "$last")
minute=$((minute + 1))
d "$root" close --at "$(at)" --outcome "$outcome"
echo "closed IDEA-1: $(jq -c '.terminated' <<< "$last") (exit $code)"
"$canon" evaluate --ir "$root/ir.json" --case "$root/case.json" --evidence "$root/evidence" \
  --authority "$root/authority.json" --decisions "$root/decisions.json" --at "$(at)" > "$out/decision.json"
echo "final canon evaluate sha256 $(sha256sum < "$out/decision.json" | cut -d' ' -f1)" | tee -a "$log"
d "$root" replay
echo "$last" > "$out/replay.json"
echo "replay exit $code: $(jq -c 'map({case, state_matches_log, decision_matches_log})' "$out/replay.json")"
while IFS= read -r c; do
  cp "$c/events.jsonl" "$out/events/$(jq -r .id "$c/case.json").jsonl"
done < <(find "$store" -name case.json -printf '%h\n' | sort)
cp -a "$store" "$out/store"
