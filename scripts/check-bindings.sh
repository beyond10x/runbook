#!/usr/bin/env bash
# Check every driver/examples/sdc/<protocol>.bindings.json against its compiled protocol: every
# bound action is declared, every produced kind is in the action's may_produce, every placed
# artifact is declared, a `child` binding is on a `case_open` action, and every action that may
# produce evidence or opens a case is bound or is a person's (unbound) action; and its
# bindings/<protocol>.yaml (loom.bindings/1) binds the same actions, less those in
# bindings/PENDING.md, and no action the protocol does not declare, and names each program as
# bindings/tools/<file> (no `..`) or a bare name from PATH; and each names the registration of its
# protocol file (<id with . and _ as ->@<revision>), every `copy` `from:` is the registration of a
# protocol file, and a file that binds a program has `program_root: bindings`; and each program
# states `exits.error` (and a model node `exits.not_called: [79]`). Run
# check-protocols.sh or evaluate-cases.sh first: it reads protocol/compiled/. Then the route check
# (sdc-check-routes) against docs/reject-routes.md (exit 1: findings, the script goes on; exit 2: a
# file it reads is refused, the script prints REFUSED and stops with exit 3). Then the experiment
# records: `sdc-experiment check --all` (a record that fails its check, a stale report, verdict or
# experiments/INDEX.md fails the script), and the documentation site data (`sdc-protocol-site check`).
set -euo pipefail
sdc=$(cd "$(dirname "$0")/.." && pwd)
fail=0
check_exits() {
# The exits of every program binding (docs/loom-bindings.md, "The exit lists"): `exits.error` is
# [75, 126, 127], and [69, 70, 75, 126, 127] for the five bindings the checker names as waiting for
# the build pool (each must carry `--pool-wait`, and no other may); a model node (the
# program `sdc-node`) has `exits.not_called` with the one code 79; no code is in both classes or in
# a `from: exit` result table of the action, and `exits` is on no other action. Loom refuses an
# overlap itself; this fails before a run does. Loom reads two classes, `error` and `not_called`, each
# a list of exit codes: another class, a class that is not a list and an `exits` that is not a
# # mapping are refused by a named line. `check-bindings.sh --exits <root>` runs this check alone over
# <root>/bindings/*.yaml (driver/tests/check_bindings_exits.rs). The pool list is this repository's:
# it applies when <root>/bindings/ holds one of the files it names (sdc-story.yaml, sdc-release.yaml,
# sdc-bugfix.yaml); a root with none of them is the runbook of another repository, whose programs
# are checked for `exits.error` [75, 126, 127] and no `--pool-wait` alone.
exit_findings=$(cd "$sdc" && yq -o=json -I=0 '{"f": filename, "actions": .actions}' bindings/*.yaml | jq -rn '
  ["bindings/sdc-story.yaml:tests.run", "bindings/sdc-release.yaml:acceptance.run",
   "bindings/sdc-bugfix.yaml:repro.run", "bindings/sdc-bugfix.yaml:tests.run", "bindings/sdc-bugfix.yaml:gate.run"] as $pools
  | [inputs] as $docs
  | ([$pools[] | split(":")[0]] | unique) as $poolfiles
  | ([$docs[].f] | any(. as $f | $poolfiles | index($f) != null)) as $own
  | (if $own then $pools else [] end) as $pools
  | ($pools[] | select(. as $p | [$docs[] | .f as $f | .actions[] | "\($f):\(.action)"] | index($p) | not)
     | "\(.): a binding that waits for the build pool is not in bindings/"),
  ($docs[]
  | .f as $f | .actions[]
  | . as $a
  | (($a.executor.kind // "") == "program") as $program
  | (($a.executor.argv // []) | index("--pool-wait") != null) as $waits
  | ($pools | index("\($f):\($a.action)") != null) as $pool
  | (($a.executor.argv[0] // "") == "sdc-node") as $node
  | (if $pool then [69, 70, 75, 126, 127] else [75, 126, 127] end) as $want
  | (if ($a.exits | type) == "object" then $a.exits else {} end) as $ex
  | [$ex | keys[] | select(. != "error" and . != "not_called")] as $unknown
  | (["error", "not_called"] | map(select(. as $c | $ex[$c] != null and ($ex[$c] | type) != "array"))) as $nolist
  | (($a.exits != null and ($a.exits | type) != "object") or ($nolist | length) > 0) as $malformed
  | (($ex.error // []) | if type == "array" then sort else [] end) as $err
  | (($ex.not_called // []) | if type == "array" then sort else [] end) as $nc
  | ([($a.evidence // [])[] | select(.from == "exit") | (.results // {}) | keys[] | select(. != "*") | tonumber]) as $keys
  | if ($program | not) then
      (if $a.exits != null then "\($f) \($a.action): exits on an executor that is not a program" else empty end)
    else
      (if $pool and ($waits | not) then "\($f) \($a.action): a binding that waits for the build pool has no --pool-wait in its argv" else empty end),
      (if $waits and ($pool | not) then "\($f) \($a.action): --pool-wait in the argv of a binding the checker does not list as waiting for the build pool" else empty end),
      (if ($a.exits != null and ($a.exits | type) != "object") then "\($f) \($a.action): exits is not a mapping of exit classes" else empty end),
      ($unknown[] | "\($f) \($a.action): exits.\(.) is not an exit class, Loom reads error and not_called"),
      ($nolist[] | "\($f) \($a.action): exits.\(.) is not a list of exit codes"),
      (if $malformed then empty else
        (if $err != $want then "\($f) \($a.action): exits.error is \($err | tojson), want \($want | tojson)" else empty end),
        (if $node and $nc != [79] then "\($f) \($a.action): exits.not_called is \($nc | tojson), want [79]" else empty end),
        (if ($node | not) and $nc != [] then "\($f) \($a.action): exits.not_called on a program that is no model node" else empty end),
        (if ([$err[], $nc[]] | map(select(. as $c | $keys | index($c))) | length) > 0 then "\($f) \($a.action): an exits code is a key of the result table" else empty end),
        (if ([$err[] | select(. as $c | $nc | index($c))] | length) > 0 then "\($f) \($a.action): a code is in exits.error and exits.not_called" else empty end)
      end)
    end)')
if [[ -n $exit_findings ]]; then
  echo "FAIL exits"; echo "$exit_findings" | sed 's/^/  /'; fail=1
else
  echo "pass exits"
fi
}
if [[ ${1:-} == --exits ]]; then
  sdc=$(cd "${2:?usage: check-bindings.sh --exits <root>}" && pwd)
  check_exits
  exit "$fail"
fi
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
    # The program and node a run starts are named twice as well: the two leading members of `argv`
    # in the YAML and in the JSON file (its `{dir}/../../../bindings/` prefix written as the YAML's
    # path below bindings/) are equal for every action both bind to a program.
    findings+=$(jq -r --slurpfile j "$b" '
      def norm: if type == "string" then ltrimstr("{dir}/../../../bindings/") else . end;
      .[] | .action as $a | select(.executor.argv != null and $j[0][$a].argv != null)
      | (.executor.argv[:2]) as $y | ($j[0][$a].argv[:2] | map(norm)) as $d
      | if $y != $d then "\nbindings/'"$name"'.yaml \($a): argv starts \($y | tojson), driver/examples/sdc/'"$name"'.bindings.json starts \($d | tojson)" else empty end' <<< "$acts")
    # What the route check reads of a binding is named twice: the run reads the YAML, the route
    # check and the site read the JSON copy. For every action both bind, these are equal: the kinds
    # it produces, each kind's results by exit code, the subjects of the evidence, `retry_when`
    # (kind to results), the runs (`max_runs`), whether it is a person's action (`manual`) and
    # whether it hands off (`handoff`). The YAML names the result of a `from: effect` entry as Loom
    # does, `performed` for the JSON exit 0 and `rejected` for exit 6 (docs/loom-bindings.md, D31 and
    # D12); an action with no evidence names no subject in the YAML (D38), so its `subject` is not
    # compared. A difference makes the check judge a run that does not happen. (The effect of an
    # action is read from the protocol, not from either copy.)
    findings+=$(jq -r --slurpfile j "$b" '
      .[] | .action as $a | select($j[0][$a] != null) | $j[0][$a] as $d
      | ([.evidence[]?.kind] | sort) as $yk
      | ([$d.produces] | flatten | map(select(. != null)) | sort) as $dk
      | ([.evidence[]? | .kind as $k | .from as $from | (.results // {}) | to_entries[]
          | [$k, (if $from == "effect" then ({"performed": "0", "rejected": "6"}[.key] // .key) else .key end), .value]] | sort) as $yr
      | ([($d.results // {}) | to_entries[] | .key as $c
          | if ($dk | length) > 1 then (.value | to_entries[]? | [.key, $c, .value]) else [$dk[0], $c, .value] end] | sort) as $dr
      | ([.evidence[]?.subject] | unique) as $ys
      | ([$d.subject] | map(select(. != null)) | unique) as $ds
      | [["produces", $yk, $dk], ["results", $yr, $dr],
         (if ($yk + $dk | length) > 0 then ["subject", $ys, $ds] else empty end),
         ["retry_when", (.retry_when // null), ($d.retry_when // null)],
         ["max_runs", (.budget.max_runs // null), ($d.max_runs // null)],
         ["manual", (.executor.kind == "person"), ($d.manual == true)],
         ["handoff", (.handoff != null), ($d.handoff != null)]][]
      | select(.[1] != .[2])
      | "\nbindings/'"$name"'.yaml \($a): \(.[0]) is \(.[1] | tojson), driver/examples/sdc/'"$name"'.bindings.json has \(.[2] | tojson)"' <<< "$acts")
    while IFS= read -r p; do
      [[ $p =~ ^instructions/.+\.md$&& ! $p =~ (^|/)\.\.(/|$) && -f $sdc/$p ]] ||
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
check_exits
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
# The answers file of `runbook run` (docs/loom-run-decisions.md, "The answers file"): what the
# runner does for a decision of a person step or a handoff before it runs Loom's printed command.
# Its format id is the one the runner reads, every action it names is bound in some
# bindings/*.yaml (an `ApproveAction` one is a person's, a `Handoff` one has `handoff:` text), and
# each program is tools/<file>, executable, or a binary of driver/ (found on PATH by the launcher).
ans=$sdc/bindings/tools/answers.yaml
if [[ -f $ans ]]; then
  ans_findings=""
  ans_json=$(yq -o=json '.' "$ans")
  [[ $(jq -r .format <<< "$ans_json") == runbook.loom-run-answers/1 ]] || ans_findings+=$'\n'"format is not runbook.loom-run-answers/1"
  bound=$(cd "$sdc" && yq -o=json -I=0 '.actions[] | {"action": .action, "kind": .executor.kind, "handoff": (.handoff != null)}' bindings/*.yaml | jq -s .)
  while IFS=$'\t' read -r a on; do
    case $on in
      ApproveAction) q='[.[] | select(.action == $a and .kind == "person")] | length' ;;
      Handoff) q='[.[] | select(.action == $a and .handoff == true)] | length' ;;
      *) ans_findings+=$'\n'"$a: on is $on, not ApproveAction or Handoff"; continue ;;
    esac
    [[ $(jq --arg a "$a" "$q" <<< "$bound") -gt 0 ]] || ans_findings+=$'\n'"$a: no binding of a $on decision in bindings/*.yaml"
  done < <(jq -r '.answers[] | [.action, .on] | @tsv' <<< "$ans_json")
  while IFS= read -r p; do
    if [[ $p == */* ]]; then
      [[ $p =~ ^tools/[^/]+$ && -x $sdc/bindings/$p ]] || ans_findings+=$'\n'"program $p: not an executable bindings/tools/<file>"
    else
      [[ -x $sdc/driver/target/debug/$p ]] || ans_findings+=$'\n'"program $p: not a binary of driver/ (cargo build --bins in driver/)"
    fi
  done < <(jq -r '.answers[] | (.requests[0], .apply[0], (.then[0] // empty))' <<< "$ans_json")
  if [[ -n $ans_findings ]]; then
    echo "FAIL answers file"; echo "${ans_findings#$'\n'}" | sed 's/^/  /'; fail=1
  else
    echo "pass answers file"
  fi
fi
# The reject and cap routes (D148, docs/reject-routes.md): sdc-check-routes finds every state of
# the autopilot where nothing points to a next step; its findings must equal the list in the doc.
(cd "$sdc/driver" && cargo build --quiet --bin sdc-check-routes) || fail=1
routes=0
"$sdc/driver/target/debug/sdc-check-routes" --root "$sdc" --doc "$sdc/docs/reject-routes.md" || routes=$?
if [[ $routes -eq 2 ]]; then
  echo "REFUSED routes: sdc-check-routes refused a file it reads (reasons above); no route was checked"
  exit 3
elif [[ $routes -ne 0 ]]; then
  fail=1
fi
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
