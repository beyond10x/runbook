#!/usr/bin/env bash
# The SDC baseline suite, free part (docs/kpis.md § 3): structural KPIs from `canon check` over every
# protocol, and run KPIs of the scripted adversary suite. Compares with a baseline and prints the
# delta. No model is called.
#
#   eval/run.sh <scratch-dir> [--against eval/baseline.json] [--ratchet]
#
# Writes eval/reports/<UTC>.json and .txt. Exit 0 nothing worse, 1 a KPI worse than the baseline,
# 2 the suite could not run. --ratchet writes the candidate as the new baseline when nothing is
# worse.
set -uo pipefail
sdc=$(cd "$(dirname "$0")/.." && pwd)
scratch=$1; shift
against="" ratchet=0
while [[ $# -gt 0 ]]; do
  case $1 in
    --against) against=$2; shift 2 ;;
    --ratchet) ratchet=1; shift ;;
    *) echo "unknown argument $1" >&2; exit 2 ;;
  esac
done
canon=${CANON:-$HOME/beyond10x/canon/target/debug/canon}
drive=${DRIVE:-$sdc/driver/target/debug/sdc-drive}
stamp=$(date -u +%Y%m%dT%H%M%SZ)
mkdir -p "$sdc/eval/reports" "$scratch"
cand=$sdc/eval/reports/$stamp.json
export RUNS=$sdc/eval/reports/$stamp-runs

# Structural: S1 (states, ratchet down) and the findings count (gate at 0) per protocol.
kpis='{}'
protocols=${PROTOCOLS:-$sdc/protocol}
for p in "$protocols"/*.protocol.yaml; do
  line=$("$canon" check --path "$p" 2>&1 | tail -1)
  id=$(sed -n 's/.*protocol `\([^`]*\)`.*/\1/p' <<< "$line")
  states=$(sed -n 's/.*: \([0-9]*\) states.*/\1/p' <<< "$line")
  findings=$(sed -n 's/.* \([0-9]*\) findings.*/\1/p' <<< "$line")
  [[ -n $id && -n $states && -n $findings ]] || { echo "canon check $p: $line" >&2; exit 2; }
  kpis=$(jq --arg id "$id" --argjson s "$states" --argjson f "$findings" \
    '. + {("S1." + $id): {value: $s, dir: "down", gate: "ratchet"},
          ("S3-7.findings." + $id): {value: $f, dir: "eq0", gate: "gate"}}' <<< "$kpis")
done

# Scripted suite: R17 (assertions held), R2 false done and R9 steps of the stub baseline.
DRIVE=$drive CANON=$canon "$sdc/eval/run-adversaries.sh" "$scratch/suite" \
  baseline spec-noop spec-junk spec-fail spec-hang spec-overreach spec-review-writes forbidden \
  story-noop story-readme > "$scratch/suite.out" 2>&1
tsv=$RUNS/summary.tsv
[[ $(wc -l < "$tsv") -gt 1 ]] || { echo "adversary suite wrote no rows" >&2; exit 2; }
held=$(awk -F'\t' 'NR>1 && $7=="true"' "$tsv" | wc -l)
total=$(awk -F'\t' 'NR>1' "$tsv" | wc -l)
fd=$(awk -F'\t' 'NR>1 {s+=$6} END {print s+0}' "$tsv")
steps=$(awk -F'\t' '$1=="baseline" {print $3}' "$tsv")
kpis=$(jq --argjson h "$held" --argjson t "$total" --argjson fd "$fd" --argjson st "${steps:-0}" \
  '. + {"R17.adversary_assertions_held": {value: $h, of: $t, dir: "up", gate: "ratchet"},
        "R2.scripted_false_done": {value: $fd, dir: "down", gate: "ratchet"},
        "R9.baseline_driver_steps": {value: $st, dir: "down", gate: "advisory"}}' <<< "$kpis")
# R10: time to stuck per stuck variant, in adversary runs (an adversary never improves, so fewer is
# less waste; advisory) and in driver steps.
while IFS=$'\t' read -r v verdict st runs _; do
  kpis=$(jq --arg v "$v" --argjson r "$runs" --argjson s "$st" \
    '. + {("R10.runs_to_stuck." + $v): {value: $r, dir: "down", gate: "advisory"},
          ("R10.steps_to_stuck." + $v): {value: $s, dir: "down", gate: "advisory"}}' <<< "$kpis")
done < <(awk -F'\t' 'NR>1 && $2=="stuck"' "$tsv")

examples=${EXAMPLES:-$sdc/driver/examples/sdc}
exdigest=$( (cd "$examples" && find . -type f | LC_ALL=C sort | xargs sha256sum) | sha256sum | cut -c1-16)
layers=$(jq -n --arg c "$(sha256sum < "$canon" | cut -c1-16)" --arg d "$(sha256sum < "$drive" | cut -c1-16)" \
  --arg x "$exdigest" \
  --arg p "$(cat "$protocols"/*.protocol.yaml | sha256sum | cut -c1-16)" \
  --arg e "$(ess --version 2>/dev/null | head -1)" --arg a "$(aep --version 2>/dev/null | head -1)" \
  '{canon: $c, "sdc-drive": $d, bindings: $x, protocols: $p, ess: $e, aep: $a}')
jq -n --arg at "$stamp" --argjson l "$layers" --argjson k "$kpis" \
  '{format: "sdc-eval-baseline/1", suite: "sdc-free", at: $at, layers: $l, kpis: $k}' > "$cand"

# Delta: worse = a gate KPI off 0, or a ratchet or eq0 KPI moved in its bad direction.
report=${cand%.json}.txt
if [[ -n $against && -f $against ]]; then
  jq -r -n --slurpfile b "$against" --slurpfile c "$cand" '
    ($b[0]) as $b | ($c[0]) as $c |
    "suite \($c.suite) against baseline \($b.at)",
    "layers changed: \([$c.layers | to_entries[] | select($b.layers[.key] != .value) | .key] | join(", ") | if . == "" then "none" else . end)",
    ($c.kpis | to_entries[] | . as $e | ($b.kpis[$e.key].value) as $old | $e.value.value as $new |
      (if $old == null then "new"
       elif $new == $old then "same"
       elif ($e.value.dir == "down" or $e.value.dir == "eq0") then (if $new < $old then "better" else "worse" end)
       else (if $new > $old then "better" else "worse" end) end) as $v |
      (if $v == "worse" and $e.value.gate != "advisory" then " FAIL" else "" end) as $f |
      "\($e.key)\t\($old // "-")\t\($new)\t\($v)\($f)")' > "$report"
else
  jq -r '"suite \(.suite), no baseline", (.kpis | to_entries[] | "\(.key)\t-\t\(.value.value)\tnew")' "$cand" > "$report"
fi
column -t -s $'\t' "$report"
if grep -q ' FAIL$' "$report"; then exit 1; fi
if [[ $ratchet == 1 ]]; then cp "$cand" "$sdc/eval/baseline.json"; echo "baseline written: eval/baseline.json"; fi
exit 0
