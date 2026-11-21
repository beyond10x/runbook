#!/usr/bin/env bash
# Validate, compile and check every SDC protocol with canon, then prove the ESS gate property:
# for each protocol whose success outcome leads to implement, test or release, a probe copy adds
# the outcome `probe.passed_without_gate` (the success outcome's own requirement AND the ESS gate
# evidence not TRUE at the current spec revision). `canon check` must report exactly that outcome
# unreachable: no state of the protocol reaches `passed` without the gate evidence.
set -euo pipefail

sdc=$(cd "$(dirname "$0")/.." && pwd)
source "$sdc/scripts/evaluate-cases.sh"
out=$sdc/protocol/compiled
probes=$sdc/protocol/probes
mkdir -p "$out" "$probes"
fail=0

# protocol/simulate/CANON_REV is the pinned commit of driver/canon-pin.json (a prefix of it).
simulate_dir=$sdc/protocol/simulate
simulate_rev=$(tr -d '[:space:]' < "$simulate_dir/CANON_REV")
pin_commit=$(jq -r .commit "$sdc/driver/canon-pin.json")
if [[ -z $simulate_rev || $pin_commit != "$simulate_rev"* ]]; then
  echo "protocol/simulate/CANON_REV ($simulate_rev) is not the commit pinned in driver/canon-pin.json ($pin_commit)"
  exit 1
fi
canon_resolve install || exit 1

run() { "$canon" "$@"; }

for f in "$sdc"/protocol/*.protocol.yaml; do
  name=$(basename "$f" .protocol.yaml)
  run validate --path "$f"
  run compile --path "$f" > "$out/$name.ir.json"
  run check --path "$f" || fail=1
done

ess_gate='{"evidence": {"kind": "ess_gate", "result": "passed", "subject": "spec"}}'
spec_gate='{"all": [
  {"evidence": {"kind": "spec_validation", "result": "valid", "subject": "spec"}},
  {"evidence": {"kind": "spec_compile", "result": "compiled", "subject": "spec"}},
  {"evidence": {"kind": "open_markers", "result": "none", "subject": "spec"}},
  {"evidence": {"kind": "spec_synthesis", "result": "clean", "subject": "spec"}}]}'

probe() {
  local name=$1 gate=$2
  local probe_file=$probes/$name.gate-probe.yaml
  GATE=$gate yq '
    .claims["probe.gate"] = {"description": "the ESS gate evidence, read directly", "true_when": env(GATE)}
    | .outcomes["probe.passed_without_gate"] = {
        "description": "passed while the ESS gate evidence is not TRUE",
        "requires": {"all": [.outcomes.passed.requires,
          {"any": [{"claim": "probe.gate", "is": "false"}, {"claim": "probe.gate", "is": "unknown"}]}]}}
  ' "$sdc/protocol/$name.protocol.yaml" > "$probe_file"
  local report
  report=$(run check --path "$probe_file" 2>&1 || true)
  echo "$report"
  if [[ $(grep -c '^error\|^finding\|unreachable\|unsatisfiable\|unproduced\|bypass' <<< "$report") -eq 1 ]] \
    && grep -q 'unreachable-outcome.*probe.passed_without_gate' <<< "$report"; then
    echo "gate property holds: $name"
  else
    echo "GATE PROPERTY NOT PROVEN: $name"
    fail=1
  fi
}

probe sdc-spec "$spec_gate"
for name in sdc-plan sdc-epic sdc-story sdc-release; do
  probe "$name" "$ess_gate"
done

# Simulate: `canon simulate` on every protocol with the pinned Canon (driver/canon-pin.json; the
# binary resolved above, built outside the repository), compared byte for byte with
# protocol/simulate/<name>.txt. A different finding set fails with a diff. `--repin` (or
# SDC_SIMULATE_REPIN=1) rewrites the pinned reports for a deliberate change; SDC_SIMULATE=off skips
# the step.
simulate_bin=$canon

simulate_count() { sed -n 's/^simulated: .*, \([0-9]*\) findings\{0,1\}\( (.*)\)\{0,1\}$/\1/p' "$1"; }
simulate_field() { sed -n "s/^simulated: .*[ ,:] *\([0-9][0-9]*\) $2.*/\1/p" "$1" | head -1; }
simulate_waiting() { simulate_field "$1" waiting | grep . || echo 0; }

simulate_step() {
  if [[ ${SDC_SIMULATE:-on} == off ]]; then
    echo "simulate: skipped (SDC_SIMULATE=off)"
    return 0
  fi
  local repin=0 bad=0 report
  [[ ${1:-} == --repin || ${SDC_SIMULATE_REPIN:-} == 1 ]] && repin=1
  report=$(mktemp -d)
  for f in "$sdc"/protocol/*.protocol.yaml; do
    local name code=0 now was
    name=$(basename "$f" .protocol.yaml)
    "$simulate_bin" simulate --path "$f" > "$report/$name.txt" 2> "$report/$name.err" || code=$?
    if (( code > 1 )) || [[ -s $report/$name.err ]]; then
      echo "simulate: $name: canon simulate exit $code"; cat "$report/$name.err"; bad=1; continue
    fi
    now=$(simulate_count "$report/$name.txt")
    printf 'simulate: %s: reachable %s, terminable %s, waiting %s, findings %s\n' "$name" \
      "$(simulate_field "$report/$name.txt" 'reachable')" "$(simulate_field "$report/$name.txt" 'terminable')" \
      "$(simulate_waiting "$report/$name.txt")" "${now:-?}"
    if (( repin )); then
      cp "$report/$name.txt" "$simulate_dir/$name.txt"
    elif [[ ! -f $simulate_dir/$name.txt ]]; then
      echo "simulate: $name: no pinned report (run with --repin)"; bad=1
    elif ! cmp -s "$simulate_dir/$name.txt" "$report/$name.txt"; then
      was=$(simulate_count "$simulate_dir/$name.txt")
      diff -u --label "pinned/$name.txt" --label "now/$name.txt" "$simulate_dir/$name.txt" "$report/$name.txt" || true
      echo "simulate: $name: findings changed (was ${was:-?}, now ${now:-?})"
      bad=1
    fi
  done
  rm -rf "$report"
  if (( repin )); then echo "simulate: pinned reports rewritten in protocol/simulate/"; fi
  return $bad
}

simulate_step "$@" || fail=1
exit $fail
