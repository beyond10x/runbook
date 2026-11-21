#!/usr/bin/env bash
# Check every driver/examples/sdc/<protocol>.bindings.json against its compiled protocol: every
# bound action is declared, every produced kind is in the action's may_produce, every placed
# artifact is declared, a `child` binding is on a `case_open` action, and every action that may
# produce evidence or opens a case is bound or is a person's (unbound) action; and its
# bindings/<protocol>.yaml (loom.bindings/1) binds the same actions, less those in
# bindings/PENDING.md, and no action the protocol does not declare, and names each program as
# bindings/tools/<file> (no `..`) or a bare name from PATH. Run
# check-protocols.sh or evaluate-cases.sh first: it reads protocol/compiled/. Then the route check
# (sdc-check-routes) against docs/reject-routes.md. Then the experiment
# records: `sdc-experiment check --all` (a record that fails its check, a stale report, verdict or
# experiments/INDEX.md fails the script), and the documentation site data (`sdc-protocol-site check`).
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
  # The loom.bindings/1 file beside it (docs/loom-bindings.md): every action bound here is bound
  # there or has a line in bindings/PENDING.md, and every action bound there is declared.
  y=$sdc/bindings/$name.yaml
  if [[ ! -f $y ]]; then
    findings+=${findings:+$'\n'}"no bindings/$name.yaml"
  else
    ya=$(yq -o=json '[.actions[].action]' "$y")
    for a in $(jq -r --argjson y "$ya" 'keys[] | select(startswith("$") | not) | select(. as $a | $y | index($a) | not)' "$b"); do
      grep -qE "^- \`$name\\.bindings\\.json:[0-9]+\` \`${a//./\\.}\`" "$sdc/bindings/PENDING.md" 2>/dev/null ||
        findings+=${findings:+$'\n'}"action $a is bound in the JSON file, not in bindings/$name.yaml, and has no line in bindings/PENDING.md"
    done
    findings+=$(jq -r --slurpfile ir "$ir" '.[] | select($ir[0].actions[.] == null) | "\nbindings/'"$name"'.yaml binds undeclared action \(.)"' <<< "$ya")
    # A bindings file reaches no program outside its own directory (docs/loom-bindings.md, "Tool
    # changes for Loom's program contract"): no argv or arguments_from entry holds a `..` segment,
    # and every program named with a `/` is tools/<file> beside the file, executable. A program
    # named without a `/` comes from PATH.
    acts=$(yq -o=json '.actions' "$y")
    findings+=$(jq -r '.[] | .action as $a | ((.executor.argv // []), (.arguments_from // []))[]
      | select(test("(^|/)\\.\\.(/|$)")) | "\nbindings/'"$name"'.yaml \($a) names \(.): a path with .."' <<< "$acts")
    while IFS=$'\t' read -r a m p; do
      [[ $p == */* ]] || continue
      [[ $p =~ ^tools/[^/]+$ && -f $sdc/bindings/$p && -x $sdc/bindings/$p ]] ||
        findings+=$'\n'"bindings/$name.yaml $a $m names $p: not an executable bindings/tools/<file>"
    done < <(jq -r '.[] | .action as $a | (["executor.argv[0]", .executor.argv[0]?], ["arguments_from[0]", .arguments_from[0]?])
      | select(.[1] != null) | [$a, .[0], .[1]] | @tsv' <<< "$acts")
    findings=${findings#$'\n'}
  fi
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
# The documentation site's protocol data (website/data/protocols/), written by
# sdc-protocol-site from the protocols, the bindings and the pinned check and simulate output.
site_bin=$sdc/driver/target/debug/sdc-protocol-site
if [[ ! -x $site_bin ]]; then
  echo "FAIL site data: no $site_bin; build the driver first (cargo build --bins in driver/)"; fail=1
elif "$site_bin" check --root "$sdc" | sed "s/^/  /"; [[ ${PIPESTATUS[0]} -eq 0 ]]; then
  echo "pass site data"
else
  echo "FAIL site data"; fail=1
fi
exit $fail
