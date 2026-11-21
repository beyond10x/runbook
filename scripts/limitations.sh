#!/usr/bin/env bash
# Reproduce each Canon limitation the SDC protocols hit; prints each command and its output.
set -uo pipefail

sdc=$(cd "$(dirname "$0")/.." && pwd)
source "$sdc/scripts/evaluate-cases.sh"
canon_resolve || exit 1
lim=$sdc/docs/limitations

show() {
  local out code
  out=$("$canon" "$@" 2>&1)
  code=$?
  {
    echo "\$ canon $*"
    [[ -z $out ]] || printf '%s\n' "$out"
    echo "exit $code"
    echo
  } | sed "s|$sdc/||g"
}

show check --path "$lim/story-with-derived-verified-claim.protocol.yaml"
show check --path "$lim/monolith.protocol.yaml"
show check --path "$lim/intake-with-abandon.protocol.yaml"
show check --path "$lim/epic-imported-evidence.protocol.yaml"
yq '
  del(.evidence_kinds.ess_gate)
  | .evidence_kinds.spec_validation = {} | .evidence_kinds.spec_compile = {}
  | .evidence_kinds.open_markers = {} | .evidence_kinds.spec_synthesis = {}
  | .claims["gate.passed"].true_when = {"all": [
      {"evidence": {"kind": "spec_validation", "result": "valid", "subject": "spec"}},
      {"evidence": {"kind": "spec_compile", "result": "compiled", "subject": "spec"}},
      {"evidence": {"kind": "open_markers", "result": "none", "subject": "spec"}},
      {"evidence": {"kind": "spec_synthesis", "result": "clean", "subject": "spec"}}]}
  | .actions["spec.gate"].may_produce = [{"evidence": "spec_validation"}, {"evidence": "spec_compile"},
      {"evidence": "open_markers"}, {"evidence": "spec_synthesis"}]
' "$sdc/protocol/sdc-story.protocol.yaml" > "$lim/story-with-four-gate-kinds.protocol.yaml"
show check --path "$lim/story-with-four-gate-kinds.protocol.yaml"
show validate --path "$lim/child-outcomes-invalidated.protocol.yaml"

mkdir -p "$lim/missing-artifact/evidence" "$lim/evidence-with-gitkeep/evidence"
printf '%s\n' 'format: canon-case/1' 'id: IDEA-1' 'protocol: sdc' 'revision: c1' 'artifacts:' \
  '  idea: {revision: i1}' '  goals: {revision: none}' > "$lim/missing-artifact/case.yaml"
show evaluate --ir "$sdc/protocol/compiled/sdc.ir.json" --case "$lim/missing-artifact/case.yaml" \
  --evidence "$lim/missing-artifact/evidence"

touch "$lim/evidence-with-gitkeep/evidence/.gitkeep"
show evaluate --ir "$sdc/protocol/compiled/sdc.ir.json" --case "$sdc/cases/01-idea-only/case.yaml" \
  --evidence "$lim/evidence-with-gitkeep/evidence"
