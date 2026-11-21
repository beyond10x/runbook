#!/usr/bin/env bash
# Evaluate every SDC case run with canon and compare it with its expected.json.
# A run directory holds expected.json and optionally case.yaml, evidence/, authority.yaml,
# decisions.yaml and at.txt; a missing case.yaml or evidence/ is taken from the parent directory.
# Writes decision.json (the real canon-decision/1 output) beside expected.json.
set -euo pipefail

sdc=$(cd "$(dirname "$0")/.." && pwd)
canon=${CANON_DIR:-$HOME/beyond10x/canon}
fail=0

mkdir -p "$sdc/protocol/compiled"
cd "$canon"
for f in "$sdc"/protocol/*.protocol.yaml; do
  cargo run --locked -q -p canon-cli -- compile --path "$f" \
    > "$sdc/protocol/compiled/$(basename "$f" .protocol.yaml).ir.json"
done

while IFS= read -r expected; do
  run=$(dirname "$expected")
  case_file=$run/case.yaml; [[ -f $case_file ]] || case_file=$run/../case.yaml
  # Git keeps no empty directory and Canon refuses a placeholder file in evidence/
  # (docs/limitations/evidence-with-gitkeep): a run with no records of its own and none to take
  # from its parent gets an empty evidence/, so a fresh clone evaluates every run.
  [[ -d $run/evidence || -d $run/../evidence ]] || mkdir "$run/evidence"
  evidence=$run/evidence; [[ -d $evidence ]] || evidence=$run/../evidence
  protocol=$(yq -r '.protocol' "$case_file")
  args=(--ir "$sdc/protocol/compiled/${protocol//./-}.ir.json" --case "$case_file" --evidence "$evidence")
  [[ -f $run/authority.yaml ]] && args+=(--authority "$run/authority.yaml")
  [[ -f $run/decisions.yaml ]] && args+=(--decisions "$run/decisions.yaml")
  [[ -f $run/at.txt ]] && args+=(--at "$(cat "$run/at.txt")")
  cargo run --locked -q -p canon-cli -- evaluate "${args[@]}" > "$run/decision.json"
  actual=$(jq -S '{
    claims: (.claims | map_values(.value)),
    obligations: ((.obligations // []) | map({(.id): .status}) | add // {}),
    actions: ((.actions // {}) | map_values(.status)),
    outcomes: ((.outcomes // {}) | map_values(.status))
  }' "$run/decision.json")
  if diff <(jq -S . "$expected") <(echo "$actual") > /dev/null; then
    echo "pass ${run#"$sdc"/}"
  else
    echo "FAIL ${run#"$sdc"/}"
    diff <(jq -S . "$expected") <(echo "$actual") || true
    fail=1
  fi
done < <(find "$sdc/cases" -name expected.json | sort)
exit $fail
