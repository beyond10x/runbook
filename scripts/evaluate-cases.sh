#!/usr/bin/env bash
# Evaluate every SDC case run with canon and compare it with its expected.json.
# A run directory holds expected.json and optionally case.yaml, evidence/, authority.yaml,
# decisions.yaml and at.txt; a missing case.yaml or evidence/ is taken from the parent directory.
# Writes decision.json (the real canon-decision/1 output) beside expected.json.
#
# Canon resolution, shared: the cycle scripts, check-protocols.sh and limitations.sh source this
# file (they get the function; the guard below returns before any step of this script runs).
# `canon_resolve [install]` sets and exports `canon` and `CANON`: the binary named by CANON, else
# the cached install of the commit pinned in driver/canon-pin.json (`install` makes it when
# absent: cargo install into the cache, never a build in a Canon checkout). It refuses, status 1,
# unless `canon --version` equals the pinned version. No checkout of Canon is entered or built.
_canon_pin=$(cd "$(dirname "${BASH_SOURCE[0]}")/../driver" && pwd)/canon-pin.json
canon_resolve() {
  local want rev bin root got
  want=$(jq -r .version "$_canon_pin") rev=$(jq -r .commit "$_canon_pin")
  if [[ -n ${CANON:-} ]]; then
    bin=$CANON
  else
    root=${SDC_SIMULATE_ROOT:-${XDG_CACHE_HOME:-$HOME/.cache}/sdc/canon-simulate/${rev:0:8}}
    bin=$root/bin/canon
    if [[ ! -x $bin && ${1:-} == install ]]; then
      echo "canon: installing the pinned commit $rev into $root"
      cargo install --locked --root "$root" --git https://github.com/beyond10x/canon --rev "$rev" canon-cli \
        || { echo "canon: install of $rev failed"; return 1; }
    fi
    if [[ ! -x $bin ]]; then
      echo "canon: CANON is not set and there is no cached install of the pinned Canon $want ($rev)."
      echo "canon: set CANON=<path of a canon $want binary>, run scripts/check-protocols.sh once to install it,"
      echo "canon: or: cargo install --locked --root $root --git https://github.com/beyond10x/canon --rev $rev canon-cli"
      return 1
    fi
  fi
  got=$("$bin" --version 2>&1 | head -1) || true
  if [[ ${got##* } != "$want" ]]; then
    echo "canon: $bin reports '${got:-nothing}'; Runbook pins Canon $want (driver/canon-pin.json)."
    return 1
  fi
  canon=$bin
  export CANON=$bin
}
if [[ ${BASH_SOURCE[0]} != "$0" ]]; then return 0; fi

set -euo pipefail

sdc=$(cd "$(dirname "$0")/.." && pwd)
fail=0
canon_resolve || exit 1

mkdir -p "$sdc/protocol/compiled"
for f in "$sdc"/protocol/*.protocol.yaml; do
  "$canon" compile --path "$f" \
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
  "$canon" evaluate "${args[@]}" > "$run/decision.json"
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
