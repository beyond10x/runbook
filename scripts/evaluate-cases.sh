#!/usr/bin/env bash
# Evaluate every SDC case run with Canon and compare it with its expected.json.
# A run directory holds expected.json and optionally case.yaml, evidence/, authority.yaml,
# decisions.yaml and at.txt; a missing case.yaml or evidence/ is taken from the parent directory.
# Writes decision.json (the real canon-decision/1 output) beside expected.json.
#
# Canon is the library `runbook` links (runbook/Cargo.lock holds the one pin): every script runs
# `"$runbook" canon <subcommand> ...`. check-protocols.sh and limitations.sh source this file (they
# get the function; the guard below returns before any step of this script runs).
# `runbook_resolve` sets and exports `runbook` and `RUNBOOK_BIN`: the binary named by RUNBOOK_BIN,
# else runbook/target/debug/runbook, built with `cargo build --locked` when it is absent or older
# than the crate's sources. No Canon checkout is entered or built, no `canon` binary is needed.
runbook_resolve() {
  local root bin
  root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
  if [[ -n ${RUNBOOK_BIN:-} ]]; then
    bin=$RUNBOOK_BIN
  else
    bin=$root/runbook/target/debug/runbook
    if [[ ! -x $bin ]] || [[ -n $(find "$root/runbook/src" "$root/runbook/Cargo.toml" "$root/runbook/Cargo.lock" -newer "$bin" -print -quit) ]]; then
      echo "runbook: building $bin"
      (cd "$root/runbook" && SLOT_TIMEOUT=30m cargo build --locked --quiet --bin runbook) \
        || { echo "runbook: the build of runbook failed"; return 1; }
    fi
  fi
  if [[ ! -x $bin ]]; then
    echo "runbook: $bin is not an executable; build it with: cd runbook && cargo build --locked --bin runbook"
    return 1
  fi
  runbook=$bin
  export RUNBOOK_BIN=$bin
}
# Legacy, for the scripts that the removal of the old driver deletes (run-trial.sh,
# run-finding-cycles.sh, run-full-cycle.sh) and `sdc-drive --canon`: `canon` is a one-line program
# that runs `runbook canon`. Nothing else calls it; it goes with those scripts.
canon_resolve() {
  local root shim
  if [[ -n ${CANON:-} ]]; then canon=$CANON; return 0; fi
  runbook_resolve || return 1
  root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
  shim=$root/target/canon-shim
  mkdir -p "$shim"
  printf '#!/bin/sh\nexec "%s" canon "$@"\n' "$runbook" > "$shim/canon.$$"
  chmod +x "$shim/canon.$$"
  mv -f "$shim/canon.$$" "$shim/canon"
  canon=$shim/canon
  export CANON=$canon
}
if [[ ${BASH_SOURCE[0]} != "$0" ]]; then return 0; fi

set -euo pipefail

sdc=$(cd "$(dirname "$0")/.." && pwd)
fail=0
runbook_resolve || exit 1

mkdir -p "$sdc/protocol/compiled"
for f in "$sdc"/protocol/*.protocol.yaml; do
  "$runbook" canon compile --path "$f" \
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
  "$runbook" canon evaluate "${args[@]}" > "$run/decision.json"
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
