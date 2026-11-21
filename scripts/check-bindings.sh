#!/usr/bin/env bash
# Check every driver/examples/sdc/<protocol>.bindings.json against its compiled protocol: every
# bound action is declared, every produced kind is in the action's may_produce, every placed
# artifact is declared, a `child` binding is on a `case_open` action, and every action that may
# produce evidence or opens a case is bound or is a person's (unbound) action. Run
# check-protocols.sh or evaluate-cases.sh first: it reads protocol/compiled/. Then the route check
# (sdc-check-routes) against docs/reject-routes.md. Then the experiment
# records: `sdc-experiment check --all` (a record that fails its check, a stale report, verdict or
# experiments/INDEX.md fails the script).
set -euo pipefail
sdc=$(cd "$(dirname "$0")/.." && pwd)
fail=0
for b in "$sdc"/driver/examples/sdc/*.bindings.json; do
  name=$(basename "$b" .bindings.json)
  ir=$sdc/protocol/compiled/$name.ir.json
  findings=$(jq -r --slurpfile ir "$ir" '
    ($ir[0]) as $p
    | (to_entries[] | select(.key | startswith("$") | not)
       | .key as $a | .value as $v
       | if $p.actions[$a] == null then "undeclared action \($a)"
         else (([$v.produces] | flatten | map(select(. != null)))[]
                | select(. as $k | [$p.actions[$a].may_produce[]?.evidence] | index($k) | not)
                | "action \($a) may not produce \(.)"),
              (if ($v.child != null) and ($p.actions[$a].case_open == null) then "action \($a) opens no case" else empty end),
              (if ($v.copy != null) and ([$p.actions[$a].may_produce[]?.evidence] | index($v.copy.kind) | not)
               then "action \($a) may not produce copied \($v.copy.kind)" else empty end)
         end),
      ((."$artifacts" // {}) | keys[] | select($p.artifacts[.] == null) | "undeclared artifact \(.)")
  ' "$b")
  unbound=$(jq -r --slurpfile ir "$ir" '. as $b | $ir[0].actions | to_entries[]
    | select($b[.key] == null) | .key' "$b" | paste -sd' ')
  if [[ -n $findings ]]; then
    echo "FAIL $name"; echo "$findings" | sed 's/^/  /'; fail=1
  else
    echo "pass $name (unbound: ${unbound:-none})"
  fi
done
# The reject and cap routes (D148, docs/reject-routes.md): sdc-check-routes finds every state of
# the autopilot where nothing points to a next step; its findings must equal the list in the doc.
(cd "$sdc/driver" && cargo build --quiet --bin sdc-check-routes) || fail=1
"$sdc/driver/target/debug/sdc-check-routes" --root "$sdc" --doc "$sdc/docs/reject-routes.md" || fail=1
# Experiment records (docs/workflow-metrics.md section 7): every record valid, each report, verdict
# and experiments/INDEX.md current. Needs the driver built (cargo build --bins in driver/).
exp_bin=${SDC_EXPERIMENT:-$sdc/driver/target/debug/sdc-experiment}
if [[ ! -x $exp_bin ]]; then
  echo "FAIL experiments: no $exp_bin; build the driver first (cargo build --bins in driver/)"; fail=1
elif (cd "$sdc" && "$exp_bin" check --all --experiments experiments | sed "s/^/  /"; exit "${PIPESTATUS[0]}"); then
  echo "pass experiments"
else
  echo "FAIL experiments"; fail=1
fi
exit $fail
