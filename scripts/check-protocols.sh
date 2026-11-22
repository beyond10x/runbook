#!/usr/bin/env bash
# Validate, compile and check every SDC protocol with canon, prove the ESS gate property and compare
# `canon simulate` with the pinned reports, inside the 120 s budget of a gate step.
#
# Gate property: for each protocol whose success outcome leads to implement, test or release, a
# probe copy adds the outcome `probe.passed_without_gate` (the success outcome's own requirement AND
# the ESS gate evidence not TRUE at the current spec revision). `canon check` must report exactly
# that outcome unreachable: no state of the protocol reaches `passed` without the gate evidence.
#
# Two lanes, one ledger (protocol/check-ledger.json, generated, committed):
#   fast (default)  validate and compile every protocol (about 0.1 s each, IR written to
#                   protocol/compiled/), then compare the digest of each protocol's sources with the
#                   ledger. A protocol with a current digest took its `canon check`, probe and
#                   `canon simulate` results from the ledger. A protocol whose digest differs gets
#                   the real checks now, beside the others through the build pool (`slot`), and the
#                   run is named a slow lane run; it exits 1 when the ledger is not current.
#                   Fast lane budget: 120 s; the script exits 1 above it.
#   --slow          the full regeneration: the real checks of every protocol, the ledger rebuilt
#                   and compared byte for byte with the committed one. Run it when the protocols,
#                   the pinned Canon or this script changed.
#   --record        like the default, and writes the ledger entries of the protocols it re-checked.
#   --repin         (or SDC_SIMULATE_REPIN=1) like --slow, rewrites the pinned simulate reports
#                   protocol/simulate/<name>.txt and the ledger for a deliberate change.
# SDC_SIMULATE=off skips the simulate step; --record and --repin refuse it (exit 2).
#
# The digest of a protocol: sha256 over the Canon commit of driver/canon-pin.json, the sha256 of
# this script, of the protocol file and of its pinned simulate report. The seal of a ledger entry:
# sha256 over its digest and its result fields (check, probe, simulate), written by --record; the
# fast lane fails when an entry's result fields no longer match their seal (a hand edit).
set -euo pipefail

sdc=$(cd "$(dirname "$0")/.." && pwd)
source "$sdc/scripts/evaluate-cases.sh"
out=$sdc/protocol/compiled
probes=$sdc/protocol/probes
ledger=$sdc/protocol/check-ledger.json
simulate_dir=$sdc/protocol/simulate
budget=${SDC_PROTOCOLS_BUDGET:-120}
probe_names=" sdc-spec sdc-plan sdc-epic sdc-story sdc-release "

ess_gate='{"evidence": {"kind": "ess_gate", "result": "passed", "subject": "spec"}}'
spec_gate='{"all": [
  {"evidence": {"kind": "spec_validation", "result": "valid", "subject": "spec"}},
  {"evidence": {"kind": "spec_compile", "result": "compiled", "subject": "spec"}},
  {"evidence": {"kind": "open_markers", "result": "none", "subject": "spec"}},
  {"evidence": {"kind": "spec_synthesis", "result": "clean", "subject": "spec"}}]}'

simulate_count() { sed -n 's/^simulated: .*, \([0-9]*\) findings\{0,1\}\( (.*)\)\{0,1\}$/\1/p' "$1"; }
simulate_field() { sed -n "s/^simulated: .*[ ,:] *\([0-9][0-9]*\) $2.*/\1/p" "$1" | head -1; }
simulate_waiting() { simulate_field "$1" waiting | grep . || echo 0; }

elapsed() { awk -v a="$1" -v b="$2" 'BEGIN { printf "%.1f", b - a }'; }

# gen_probe <name>: write protocol/probes/<name>.gate-probe.yaml.
gen_probe() {
  local name=$1 gate=$ess_gate
  [[ $name == sdc-spec ]] && gate=$spec_gate
  GATE=$gate yq '
    .claims["probe.gate"] = {"description": "the ESS gate evidence, read directly", "true_when": env(GATE)}
    | .outcomes["probe.passed_without_gate"] = {
        "description": "passed while the ESS gate evidence is not TRUE",
        "requires": {"all": [.outcomes.passed.requires,
          {"any": [{"claim": "probe.gate", "is": "false"}, {"claim": "probe.gate", "is": "unknown"}]}]}}
  ' "$sdc/protocol/$1.protocol.yaml" > "$probes/$name.gate-probe.yaml"
}

# digest_of <name>: sha256 of the sources of one protocol's checks.
digest_of() {
  local name=$1 report=none
  [[ -f $simulate_dir/$name.txt ]] && report=$(sha256sum < "$simulate_dir/$name.txt" | cut -d' ' -f1)
  printf 'canon %s\nscript %s\nprotocol %s\nreport %s\n' "$pin_commit" \
    "$(sha256sum < "$sdc/scripts/check-protocols.sh" | cut -d' ' -f1)" \
    "$(sha256sum < "$sdc/protocol/$name.protocol.yaml" | cut -d' ' -f1)" "$report" \
    | sha256sum | cut -d' ' -f1
}

# seal_of <ledger file> <name>: sha256 over the digest and the result fields of one ledger entry.
# --record writes it beside them; the fast lane recomputes it, so a hand edit of a result field fails.
seal_of() {
  jq -r --arg n "$2" '.protocols[$n] | "\(.digest)\ncheck \(.check)\nprobe \(.probe // "")\nsimulate \(.simulate)"' "$1" \
    | sha256sum | cut -d' ' -f1
}

# unit <name> <part> <dir> <repin>: one part of the real checks of one protocol. Part `check` is
# `canon check` and `canon simulate` against the pinned report, part `probe` is the gate probe.
# The parts of one protocol are the two costly canon calls and run beside each other. Log in
# <dir>/<name>.<part>.log, result in <dir>/<name>.<part>.json, seconds in <dir>/<name>.<part>.sec.
# Exit 1 when a check fails.
unit() {
  local name=$1 part=$2 dir=$3 repin=$4 f=$sdc/protocol/$1.protocol.yaml
  local ok=1 t0=$EPOCHREALTIME check_line="" probe_line="" sim_line="" rc report code now was
  {
    if [[ $part == check ]]; then
      report=$("$canon" check --path "$f" 2>&1) && rc=0 || rc=$?
      echo "$report"
      check_line=$(grep -m1 '^checked:' <<< "$report" || true)
      (( rc == 0 )) || ok=0
    fi

    if [[ $part == probe ]]; then
      report=$("$canon" check --path "$probes/$name.gate-probe.yaml" 2>&1 || true)
      echo "$report"
      if [[ $(grep -c '^error\|^finding\|unreachable\|unsatisfiable\|unproduced\|bypass' <<< "$report") -eq 1 ]] \
        && grep -q 'unreachable-outcome.*probe.passed_without_gate' <<< "$report"; then
        echo "gate property holds: $name"
        probe_line="gate property holds: only probe.passed_without_gate is unreachable"
      else
        echo "GATE PROPERTY NOT PROVEN: $name"
        ok=0
      fi
    fi

    if [[ $part != check ]]; then
      :
    elif [[ ${SDC_SIMULATE:-on} == off ]]; then
      echo "simulate: skipped (SDC_SIMULATE=off)"
      sim_line=skipped
    else
      report=$dir/$name.sim
      code=0
      "$canon" simulate --path "$f" > "$report" 2> "$report.err" || code=$?
      if (( code > 1 )) || [[ -s $report.err ]]; then
        echo "simulate: $name: canon simulate exit $code"; cat "$report.err"; ok=0
      else
        now=$(simulate_count "$report")
        sim_line=$(printf 'reachable %s, terminable %s, waiting %s, findings %s' \
          "$(simulate_field "$report" reachable)" "$(simulate_field "$report" terminable)" \
          "$(simulate_waiting "$report")" "${now:-?}")
        echo "simulate: $name: $sim_line"
        if (( repin )); then
          cp "$report" "$simulate_dir/$name.txt"
        elif [[ ! -f $simulate_dir/$name.txt ]]; then
          echo "simulate: $name: no pinned report (run with --repin)"; ok=0
        elif ! cmp -s "$simulate_dir/$name.txt" "$report"; then
          was=$(simulate_count "$simulate_dir/$name.txt")
          diff -u --label "pinned/$name.txt" --label "now/$name.txt" "$simulate_dir/$name.txt" "$report" || true
          echo "simulate: $name: findings changed (was ${was:-?}, now ${now:-?})"
          ok=0
        fi
      fi
      rm -f "$report" "$report.err"
    fi
  } > "$dir/$name.$part.log" 2>&1
  elapsed "$t0" "$EPOCHREALTIME" > "$dir/$name.$part.sec"
  jq -n --arg part "$part" --arg check "$check_line" --arg probe "$probe_line" --arg sim "$sim_line" \
    --argjson ok "$ok" \
    '{ok: ($ok == 1)} + (if $part == "check" then {check: $check, simulate: $sim} else {probe: $probe} end)' \
    > "$dir/$name.$part.json"
  (( ok == 1 ))
}

pin_commit=$(jq -r .commit "$sdc/driver/canon-pin.json")

if [[ ${1:-} == --unit ]]; then
  canon_resolve || exit 1
  unit "$2" "$3" "$4" "$5"
  exit $?
fi

lane_slow=0 record=0 repin=0
[[ ${SDC_SIMULATE_REPIN:-} == 1 ]] && repin=1
for arg in "$@"; do
  case $arg in
    --slow) lane_slow=1 ;;
    --record) record=1 ;;
    --repin) repin=1 ;;
    *) echo "usage: check-protocols.sh [--slow | --record | --repin]"; exit 2 ;;
  esac
done
(( repin )) && lane_slow=1
if (( record || repin )) && [[ ${SDC_SIMULATE:-on} == off ]]; then
  echo "--record and --repin with SDC_SIMULATE=off would record unchecked results (simulate: skipped)"; exit 2
fi

t_start=$EPOCHREALTIME
mkdir -p "$out" "$probes" "$sdc/target/protocol-check"
fail=0

# protocol/simulate/CANON_REV is the pinned commit of driver/canon-pin.json (a prefix of it).
simulate_rev=$(tr -d '[:space:]' < "$simulate_dir/CANON_REV")
if [[ -z $simulate_rev || $pin_commit != "$simulate_rev"* ]]; then
  echo "protocol/simulate/CANON_REV ($simulate_rev) is not the commit pinned in driver/canon-pin.json ($pin_commit)"
  exit 1
fi
canon_resolve install || exit 1

work=$(mktemp -d "$sdc/target/protocol-check/run.XXXXXX")
trap 'rm -rf "$work"' EXIT

names=()
for f in "$sdc"/protocol/*.protocol.yaml; do names+=("$(basename "$f" .protocol.yaml)"); done

declare -A t_vc stale t_real
n_stale=0
for name in "${names[@]}"; do
  f=$sdc/protocol/$name.protocol.yaml
  t0=$EPOCHREALTIME
  "$canon" validate --path "$f"
  "$canon" compile --path "$f" > "$out/$name.ir.json"
  [[ $probe_names == *" $name "* ]] && gen_probe "$name"
  t_vc[$name]=$(elapsed "$t0" "$EPOCHREALTIME")
done

[[ -f $ledger ]] || echo '{"format": "runbook.protocol-ledger/1", "protocols": {}}' > "$work/empty-ledger.json"
ledger_in=$ledger; [[ -f $ledger ]] || ledger_in=$work/empty-ledger.json
for name in "${names[@]}"; do
  have=$(jq -r --arg n "$name" '.protocols[$n].digest // ""' "$ledger_in")
  if (( lane_slow )) || [[ $have != "$(digest_of "$name")" ]]; then
    stale[$name]=1; n_stale=$((n_stale + 1))
  elif [[ $(jq -r --arg n "$name" '.protocols[$n].seal // ""' "$ledger_in") != "$(seal_of "$ledger_in" "$name")" ]]; then
    echo "protocol/check-ledger.json: $name: the result fields do not match the seal --record wrote (edited by hand?); run scripts/check-protocols.sh --record"
    fail=1
  fi
done

# The real checks of the protocols the ledger does not cover: one job per costly canon call, beside
# each other through the build pool (one slot each).
pids=()
parts_of() { if [[ $probe_names == *" $1 "* ]]; then echo "check probe"; else echo check; fi; }
for name in "${names[@]}"; do
  [[ -n ${stale[$name]:-} ]] || continue
  for part in $(parts_of "$name"); do
    if command -v slot > /dev/null 2>&1; then
      slot run --tokens 1 -- bash "$sdc/scripts/check-protocols.sh" --unit "$name" "$part" "$work" "$repin" \
        > "$work/$name.$part.slot" 2>&1 &
      pids+=($!)
    else
      bash "$sdc/scripts/check-protocols.sh" --unit "$name" "$part" "$work" "$repin" \
        > "$work/$name.$part.slot" 2>&1 || true
    fi
  done
done
for pid in "${pids[@]}"; do wait "$pid" || true; done

for name in "${names[@]}"; do
  if [[ -n ${stale[$name]:-} ]]; then
    ok=1; secs=0
    for part in $(parts_of "$name"); do
      cat "$work/$name.$part.log" 2> /dev/null || cat "$work/$name.$part.slot"
      if [[ ! -f $work/$name.$part.json ]] || [[ $(jq -r .ok "$work/$name.$part.json") != true ]]; then
        echo "FAILED: $name ($part)"; ok=0; fail=1
      fi
      [[ -f $work/$name.$part.sec ]] && secs=$(awk -v a="$secs" -v b="$(cat "$work/$name.$part.sec")" 'BEGIN { printf "%.1f", a + b }')
    done
    t_real[$name]=$secs
    if (( ok )); then
      jq -s 'add | .ok = true' $(for part in $(parts_of "$name"); do echo "$work/$name.$part.json"; done) \
        > "$work/$name.json"
    fi
  else
    jq -r --arg n "$name" '.protocols[$n] | "ledger: \($n): \(.check)\n" + (if .probe then "ledger: \($n): \(.probe)\n" else "" end) + "ledger: \($n): simulate: \(.simulate)"' "$ledger_in"
  fi
done

# The new ledger: the old entries of the current protocols, the real results of the re-checked ones.
new=$work/new-ledger.json
cp "$ledger_in" "$work/ledger-work.json"
for name in "${names[@]}"; do
  [[ -n ${stale[$name]:-} && -f $work/$name.json ]] || continue
  jq --arg n "$name" --arg d "$(digest_of "$name")" --slurpfile r "$work/$name.json" \
    '.protocols[$n] = ({digest: $d, probe: null} + ($r[0] | del(.ok)))' "$work/ledger-work.json" > "$work/ledger-next.json"
  mv "$work/ledger-next.json" "$work/ledger-work.json"
  jq --arg n "$name" --arg s "$(seal_of "$work/ledger-work.json" "$name")" '.protocols[$n].seal = $s' \
    "$work/ledger-work.json" > "$work/ledger-next.json"
  mv "$work/ledger-next.json" "$work/ledger-work.json"
done
jq -S --arg c "$pin_commit" --argjson names "$(printf '%s\n' "${names[@]}" | jq -R . | jq -s .)" \
  '.format = "runbook.protocol-ledger/1" | .canon = $c | .protocols |= with_entries(select(.key as $k | $names | index($k)))' \
  "$work/ledger-work.json" > "$new"

lane=fast
(( n_stale > 0 )) && lane="slow ($n_stale of ${#names[@]} protocols checked for real)"
if (( fail == 0 && n_stale > 0 )); then
  if (( lane_slow )) && (( ! record )) && (( ! repin )); then
    if cmp -s "$new" "$ledger"; then
      echo "slow lane: ledger regenerated from the sources equals protocol/check-ledger.json byte for byte"
    else
      diff -u --label committed --label regenerated "$ledger" "$new" || true
      echo "slow lane: protocol/check-ledger.json differs from the regeneration (run --record or --repin and commit it)"
      fail=1
    fi
  elif (( record || repin )); then
    cp "$new" "$ledger"
    echo "ledger: protocol/check-ledger.json written for $n_stale protocols"
  else
    echo "ledger stale for: ${!stale[*]}; the real checks passed; run scripts/check-protocols.sh --record and commit protocol/check-ledger.json"
    fail=1
  fi
fi
if (( repin )); then echo "simulate: pinned reports rewritten in protocol/simulate/"; fi

t_end=$EPOCHREALTIME
total=$(elapsed "$t_start" "$t_end")
echo
echo "protocol           validate+compile   real checks (canon seconds, summed over parts)"
for name in "${names[@]}"; do
  if [[ -n ${stale[$name]:-} ]]; then
    printf '%-18s %8ss        %8ss  (slow lane)\n' "$name" "${t_vc[$name]}" "${t_real[$name]}"
  else
    printf '%-18s %8ss        ledger\n' "$name" "${t_vc[$name]}"
  fi
done
echo "lane: $lane"
echo "total: ${total}s (budget of the fast lane: ${budget}s)"
if (( n_stale == 0 )) && awk -v t="$total" -v b="$budget" 'BEGIN { exit !(t > b) }'; then
  echo "OVER BUDGET: ${total}s > ${budget}s"
  fail=1
fi
exit $fail
