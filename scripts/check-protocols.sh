#!/usr/bin/env bash
# Validate, compile and check every SDC protocol with canon, then prove the ESS gate property:
# for each protocol whose success outcome leads to implement, test or release, a probe copy adds
# the outcome `probe.passed_without_gate` (the success outcome's own requirement AND the ESS gate
# evidence not TRUE at the current spec revision). `canon check` must report exactly that outcome
# unreachable: no state of the protocol reaches `passed` without the gate evidence.
set -euo pipefail

sdc=$(cd "$(dirname "$0")/.." && pwd)
canon=${CANON_DIR:-$HOME/beyond10x/canon}
out=$sdc/protocol/compiled
probes=$sdc/protocol/probes
mkdir -p "$out" "$probes"
fail=0

run() { (cd "$canon" && cargo run --locked -q -p canon-cli -- "$@"); }

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
for name in sdc-plan sdc-epic sdc-story sdc-delivery sdc-release; do
  probe "$name" "$ess_gate"
done
exit $fail
