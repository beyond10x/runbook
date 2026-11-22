#!/usr/bin/env bash
# Check every driver/examples/sdc/<protocol>.bindings.json against its compiled protocol: every
# bound action is declared, every produced kind is in the action's may_produce, every placed
# artifact is declared, a `child` binding is on a `case_open` action, and every action that may
# produce evidence or opens a case is bound or is a person's (unbound) action; and its
# bindings/<protocol>.yaml (loom.bindings/1) binds the same actions, less those in
# bindings/PENDING.md, and no action the protocol does not declare, and names each program as
# bindings/tools/<file> (no `..`) or a bare name from PATH; and each names the registration of its
# protocol file (<id with . and _ as ->@<revision>), every `copy` `from:` is the registration of a
# protocol file, and a file that binds a program has `program_root: bindings`. Run
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
    # The instruction files of an agent action (docs/driver.md, "Instructions") are named twice:
    # --instructions in bindings/<name>.yaml and llm.instructions in the JSON file. The two name the
    # same files per action; the value of --instructions is parsed, and every file named is
    # instructions/<...>.md without a `..` segment and exists.
    findings+=$(jq -r --slurpfile j "$b" '
      def ins: (.executor.argv // []) as $v | [range(0; ($v | length) - 1) | select($v[.] == "--instructions") | $v[. + 1]][0];
      .[] | .action as $a
      | (ins | if . == null then null else (try fromjson catch "bad") end) as $y
      | ($j[0][$a].llm.instructions // null) as $d
      | if $y == "bad" then "\nbindings/'"$name"'.yaml \($a): --instructions is not a JSON object"
        elif $j[0][$a] == null then empty
        elif $y != $d then "\nbindings/'"$name"'.yaml \($a): --instructions \($y | tojson) differs from llm.instructions \($d | tojson) of '"$name"'.bindings.json"
        else empty end' <<< "$acts")
    while IFS= read -r p; do
      [[ $p =~ ^instructions/.+\.md$ && ! $p =~ (^|/)\.\.(/|$) && -f $sdc/$p ]] ||
        findings+=$'\n'"$name: instruction file $p is not an existing instructions/<...>.md without .."
    done < <(
      jq -r '(.[] | (.executor.argv // []) as $v | [range(0; ($v | length) - 1) | select($v[.] == "--instructions") | $v[. + 1]][]
          | (try fromjson catch {}) | if type == "object" then .[] else empty end)' <<< "$acts"
      jq -r 'to_entries[] | select(.key | startswith("$") | not) | .value.llm.instructions // {} | .[]' "$b")
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
# The registrations of bindings/*.yaml (docs/loom-bindings.md, "Registrations"): Loom's catalog
# registers protocol/<name>.protocol.yaml as its id with `.` and `_` written as `-`, then
# `@<revision>`, and refuses another major. A file's `protocol:` is that registration, the `from:`
# of every `copy` executor is the registration of some protocol file, and a file that binds a
# program (argv or arguments_from) names `program_root: bindings`.
regs=$(cd "$sdc" && jq -r --slurpfile p <(yq -o=json -I=0 '{"f": filename, "id": .protocol.id, "rev": .protocol.revision}' protocol/*.protocol.yaml) '
  def reg: (.id | gsub("[._]"; "-")) + "@" + (.rev | tostring);
  ($p | map({key: (.f | ltrimstr("protocol/") | rtrimstr(".protocol.yaml")), value: reg}) | from_entries) as $byname
  | ($p | map(reg)) as $all
  | .f as $f | (.f | ltrimstr("bindings/") | rtrimstr(".yaml")) as $n
  | (if $byname[$n] == null then "\($f): no protocol/\($n).protocol.yaml"
     elif .protocol != $byname[$n]
     then "\($f): protocol is \(.protocol), the registration of protocol/\($n).protocol.yaml is \($byname[$n])"
     else empty end),
    (.froms[] | select(. as $x | $all | index($x) | not)
     | "\($f): copy from \(.) is the registration of no protocol file in protocol/"),
    (if .program | not then empty
     elif .root == null then "\($f): binds a program and has no program_root; write program_root: bindings"
     elif .root != "bindings" then "\($f): program_root is \(.root); write program_root: bindings"
     else empty end)' \
  <(yq -o=json -I=0 '{"f": filename, "protocol": .protocol, "root": .program_root,
      "froms": [.actions[].executor | select(.kind == "copy") | .from],
      "program": ([.actions[] | select(.executor.argv != null or .arguments_from != null)] | length > 0)}' bindings/*.yaml))
if [[ -n $regs ]]; then
  echo "FAIL registrations"; echo "$regs" | sed 's/^/  /'; fail=1
else
  echo "pass registrations"
fi
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
# A program Loom starts gets PATH and LOOM_* alone, no HOME: sdc-tool.sh in Loom mode names the ESS
# toolchain directory itself (from the passwd entry), so that the `ess` dispatcher runs. About 0.1 s.
tmp=$(mktemp -d "${TMPDIR:-/tmp}/loom-env.XXXXXX")
cp -r "$sdc/driver/examples/sdc/fixtures/billing-spec" "$tmp/spec"
if out=$(cd "$tmp" && env -i PATH="$PATH" LOOM_CASE=IDEA-1 bash "$sdc/bindings/tools/sdc-tool.sh" spec-validate --action spec.validate < /dev/null 2>&1); then
  echo "pass loom environment"
else
  echo "FAIL loom environment: sdc-tool.sh spec-validate under env -i PATH LOOM_CASE failed"; echo "$out" | sed 's/^/  /'; fail=1
fi
rm -rf "$tmp"
exit $fail
