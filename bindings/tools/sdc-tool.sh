#!/usr/bin/env bash
# The SDC example's tools, one subcommand per binding. Run with cwd = the case tree's shared work/
# directory, by sdc-drive (driver mode) or by Loom (Loom mode: LOOM_CASE is set). Each check is
# deterministic and reports only through its exit code; each `stub-*` is a stand-in for an LLM
# executor: it writes a fixed draft and decides nothing.
#
# Loom mode (docs/loom-bindings.md, "Tool changes for Loom's program contract"): Loom starts a
# bound program with PATH and LOOM_CASE alone and its arguments as one JSON object on standard
# input, which no subcommand here takes. What the driver gives as variables comes from the case id
# (SDC_CASE) and from argv entries the binding states after the subcommand: --action <id>
# (SDC_ACTION), --principal <principal> (SDC_PRINCIPAL), --stub-fixtures <dir> (SDC_STUB_FIXTURES)
# and --stub-counter <dir> (SDC_STUB_COUNTER). They are exported, so the programs started here read
# them as under the driver. sdc-findings and sdc-bugfix come from PATH, else from the driver
# crate's build in this repository, never from HOME. Without LOOM_CASE none of this runs.
set -uo pipefail
here=$(cd "$(dirname "$0")" && pwd)
root=$(cd "$here/../.." && pwd)
fx=$root/driver/examples/sdc/fixtures
if [[ -n ${LOOM_CASE:-} ]]; then
  cat > /dev/null
  exec < /dev/null
  pos=()
  while (( $# )); do
    case $1 in
      --action|--principal|--stub-fixtures|--stub-counter)
        (( $# >= 2 )) || { echo "sdc-tool: $1 takes a value" >&2; exit 64; }
        case $1 in
          --action) export SDC_ACTION=$2 ;;
          --principal) export SDC_PRINCIPAL=$2 ;;
          --stub-fixtures) export SDC_STUB_FIXTURES=$2 ;;
          --stub-counter) export SDC_STUB_COUNTER=$2 ;;
        esac
        shift 2 ;;
      *) pos+=("$1"); shift ;;
    esac
  done
  set -- "${pos[@]}"
  export SDC_CASE=$LOOM_CASE
  # Loom gives no HOME. The `ess` dispatcher takes its toolchains from ESS_TOOLCHAIN_DIR, else
  # XDG_CACHE_HOME/ess/toolchains, else HOME/.cache/ess/toolchains, and refuses to run with none of
  # them; when none is set the tool names the directory itself, from the user's passwd entry.
  # `aep`, `yq` and `jq` need nothing of this kind (they ran under PATH alone).
  if [[ -z ${ESS_TOOLCHAIN_DIR:-} && -z ${XDG_CACHE_HOME:-} && -z ${HOME:-} ]]; then
    user_home=$(getent passwd "$(id -u)" 2>/dev/null | cut -d: -f6)
    [[ -z $user_home ]] || export ESS_TOOLCHAIN_DIR=$user_home/.cache/ess/toolchains
  fi
fi
# --pool-wait <seconds>, both modes: the step's bound for waiting in the build pool's queue
# (docs/driver.md, "The build pool wait"). SDC_POOL_WAIT in the environment wins over it (the
# tests); with neither, a pool command waits as long as the pool lets it.
rest=()
while (( $# )); do
  case $1 in
    --pool-wait)
      (( $# >= 2 )) && [[ $2 =~ ^[0-9]+$ ]] || { echo "sdc-tool: --pool-wait takes whole seconds" >&2; exit 64; }
      SDC_POOL_WAIT=${SDC_POOL_WAIT:-$2}
      shift 2 ;;
    *) rest+=("$1"); shift ;;
  esac
done
set -- "${rest[@]}"
export SDC_POOL_WAIT
mkdir -p .sdc
# One wait bound per step: the outermost call of a step that runs pool commands opens a ledger of
# the milliseconds its pool commands waited, and every nested call (story-tests-run runs
# conform-run and impl-gate as tools of their own) reads and adds to it.
case $1 in
  conform-run|story-tests|story-tests-run|impl-gate)
    if [[ -z ${SDC_POOL_LEDGER:-} ]]; then
      rm -f .sdc/pool-wait .sdc/pool-err.*
      export SDC_POOL_LEDGER=$PWD/.sdc/pool-wait
      echo 0 > "$SDC_POOL_LEDGER"
      trap 'rm -f "$SDC_POOL_LEDGER"' EXIT
    fi ;;
esac
# The ess release every tool here runs: 0.97.3, the release `driver/ess-pin.json` pins (0.96.0 until
# 2026-11-22; the committed recordings, records and experiment runs before that were made with
# 0.96.0). Since 2026-11-21 the `ess` on PATH can be a version dispatcher
# (0.97.1 and later, 0.97.3 included) that runs itself in a case tree, where no `ess-inputs.yaml`
# pins a release; its output
# differs (the conformance line names the ess version, and `--strict-requires` refuses a spec that
# requires an older one), so in/tests.txt and the gates would depend on what is installed.
# ESS_TOOLCHAIN names the release to the dispatcher (a caller's value wins) and ESS_TOOLCHAIN_QUIET
# keeps its "delegating" note out of the logs; a plain ess binary ignores both.
export ESS_TOOLCHAIN=${ESS_TOOLCHAIN:-0.97.3} ESS_TOOLCHAIN_QUIET=${ESS_TOOLCHAIN_QUIET:-1}

# Exit 0 when every line of stdin is "true".
all_true() { ! grep -qv '^true$'; }

# A program the tool needs and cannot find is a program error, exit 127 (sdc-drive records no
# evidence and stops), never a check result: a check that could not run is not `invalid`.
need() {
  local p
  for p; do
    command -v "$p" >/dev/null 2>&1 || { echo "sdc-tool: $p is not on PATH: $1 could not run" >&2; exit 127; }
  done
}
# A nested call's program error is passed on as it is.
# Exit 69 is the pool's: no slot came within the wait bound, nothing ran (pool_run). Exit 70: the
# job started and the pool ended it at its run limit (pool_run).
passon() { (( $1 == 126 || $1 == 127 || $1 == 69 || $1 == 70 )) && exit "$1"; return 0; }
need yq jq

# `slot <command>` with the wait for a slot bounded: a step's wait for the shared build pool is not
# a result about its subject, and the runtime's limit counts it (the registration run of release
# 0.3.0: both `cargo test` jobs of tests.run queued 15 minutes, the step was killed at its 1800 s
# and recorded `timeout`, then the writer was called to repair tests that never ran). slot runs in
# the background; the job's id is on slot's own `slot: job <id> queued ...` line, which it prints
# only for a job that has to wait, and `slot status` shows the job as `queued` until it starts. A
# start is never inferred from a missing line (on a loaded machine the line comes seconds late):
# the line is read for as long as the process lives, and a job that ends without one was admitted
# at once and waited nothing. A
# job still queued when the step's bound (SDC_POOL_WAIT seconds, less what its earlier pool commands
# waited, kept in SDC_POOL_LEDGER) is spent is cancelled and the function returns 69. Once the job
# started, the run is the pool's: its limit is 10 minutes, or the `slot --timeout <t>` a command names (at
# most 30 minutes; none of these calls names one). A job the pool ends at that limit (slot 0.22: exit 124)
# makes the function return 70, "ran over its time": not red tests, not a pool wait. A
# job cancelled then (exit 130) is the command's result, not 69. Without a bound it is plain
# `slot`. SDC_POOL_POLL (seconds, default 2) is the longest pause between two looks at the pool.
# EPOCHREALTIME takes the decimal separator of the locale (LC_ALL=de_DE.UTF-8 gives "1763810464,227523",
# as a systemd unit on a German host runs): the point is put back before the text is split.
now_ms() { local t=${EPOCHREALTIME/,/.}; echo $(( ${t%.*} * 1000 + 10#${t#*.} / 1000 )); }
pool_run() {
  local bound=${SDC_POOL_WAIT:-} ledger=${SDC_POOL_LEDGER:-} poll=${SDC_POOL_POLL:-2} rc
  if [[ -z $bound || -z $ledger ]]; then
    slot "$@"; rc=$?
    (( rc != 124 )) || { echo "sdc-tool: pool: the job ran over its time: the pool ended it at its run limit; the command did not finish" >&2; rc=70; }
    return $rc
  fi
  local spent left errf pid id="" started=no t0 elapsed state pause=0.2 rc
  spent=$(cat "$ledger" 2>/dev/null) || spent=0
  left=$(( bound * 1000 - spent ))
  errf=$(mktemp "${ledger%/*}/pool-err.XXXXXX")
  t0=$(now_ms)
  slot "$@" 2> "$errf" &
  pid=$!
  while kill -0 "$pid" 2>/dev/null; do
    elapsed=$(( $(now_ms) - t0 ))
    if [[ -z $id ]]; then
      # Never inferred from a missing line: on a loaded machine the line comes seconds late. It is
      # read for as long as the process lives.
      id=$(sed -n 's/^slot: job \([0-9][0-9]*\) queued.*/\1/p' "$errf" | head -n 1)
    fi
    if [[ -n $id ]]; then
      state=$(slot status 2>/dev/null | awk -v id="#$id" '$1 == id { print $2; exit }')
      if [[ $state != queued ]]; then
        started=yes
      elif (( elapsed >= left )); then
        slot cancel "$id" > /dev/null 2>&1
        kill "$pid" 2>/dev/null
        wait "$pid" 2>/dev/null
        echo $(( spent + elapsed )) > "$ledger"
        cat "$errf" >&2; rm -f "$errf"
        echo "sdc-tool: pool-wait: job $id was still queued and this step has waited $(( (spent + elapsed) / 1000 )) s of its wait bound of $bound s; cancelled, nothing ran" >&2
        return 69
      fi
    fi
    if [[ $started == yes ]]; then
      echo $(( spent + elapsed )) > "$ledger"
      break
    fi
    sleep "$pause"
    pause=$(awk -v p="$pause" -v m="$poll" 'BEGIN { p *= 2; print (p > m ? m : p) }')
  done
  wait "$pid"
  rc=$?
  if [[ $started != yes ]]; then
    [[ -n $id ]] || id=$(sed -n 's/^slot: job \([0-9][0-9]*\) queued.*/\1/p' "$errf" | head -n 1)
    if [[ -n $id ]]; then
      # It queued and ended between two looks: it ran less than one pause, so what it took was wait.
      echo $(( spent + $(now_ms) - t0 )) > "$ledger"
    else
      # The process ended and never printed a queued line: the pool admitted the job at once, it
      # waited nothing.
      started=yes
      echo "$spent" > "$ledger"
    fi
  fi
  if (( rc == 130 )) && grep -q '^slot: job [0-9]* cancelled' "$errf"; then
    if [[ $started == yes ]]; then
      echo "sdc-tool: pool: job ${id:-?} was cancelled by the pool while it ran (over its run limit, 10 minutes unless the command names a longer one, or by \`slot cancel\`); the command did not finish" >&2
    else
      echo "sdc-tool: pool-wait: job ${id:-?} was cancelled while it was still queued; nothing ran" >&2
      rc=69
    fi
  fi
  if (( rc == 124 )); then
    echo "sdc-tool: pool: job ${id:-?} ran over its time: the pool ended it at its run limit; the command did not finish" >&2
    rc=70
  fi
  cat "$errf" >&2; rm -f "$errf"
  return $rc
}

# A binary of the driver crate (driver/src/bin) in Loom mode: on PATH, else its debug build.
sibling() { command -v "$1" 2>/dev/null || echo "$root/driver/target/debug/$1"; }
# The late-finding programs: the driver crate's `sdc-findings` binary.
findings() {
  local b looked="SDC_FINDINGS, then $root/driver/target/debug/sdc-findings"
  if [[ -n ${LOOM_CASE:-} ]]; then
    b=$(sibling sdc-findings)
    looked="PATH, then $root/driver/target/debug/sdc-findings"
  else
    b=${SDC_FINDINGS:-$root/driver/target/debug/sdc-findings}
  fi
  [[ -x $b ]] || { echo "sdc-tool: sdc-findings not found (looked at $looked): $1 could not run" >&2; exit 127; }
  "$b" --rules "$here/consistency-rules.json" "$@"
}
bugfix() {
  local b=${SDC_BUGFIX:-$root/driver/target/debug/sdc-bugfix}
  [[ -n ${LOOM_CASE:-} ]] && b=$(sibling sdc-bugfix)
  [[ -x $b ]] || { echo "sdc-tool: sdc-bugfix not found ($b)" >&2; exit 127; }
  "$b" "$@"
}
# Run a command; under Loom (the old driver keeps the step's log itself) also keep its log as the
# record of <kind> about the tree <tree> (sdc-reply record): the change.review inputs read it from
# the workspace, since Loom gives a program no case store.  recorded <kind> <tree> <command>...
recorded() {
  local kind=$1 tree=$2 log rc b
  shift 2
  [[ -n ${LOOM_CASE:-} ]] || { "$@"; return; }
  b=$(sibling sdc-reply)
  [[ -x $b ]] || { echo "sdc-tool: sdc-reply not found (looked at PATH, then $root/driver/target/debug/sdc-reply): $* could not run" >&2; exit 127; }
  log=$(mktemp .sdc/record-log.XXXXXX) || exit 126
  "$@" > "$log" 2>&1; rc=$?
  cat "$log"
  "$b" record --kind "$kind" --tree "$tree" --log "$log" > /dev/null || rc=126
  rm -f "$log"
  return $rc
}
# The specification's conformance as the spec stage checks it (spec.synthesize, and spec.gate's
# ess-gate): the suite is synthesized with no refusal, then run against ESS's interpreter of the
# specification (`--target interpreted --strict`), and every scenario must pass. A scenario the
# interpreter reports unsupported, failed or errored is a specification it cannot carry out, which
# no implementation can be held to (release-0.1.2.mixed.1: 8 unsupported scenarios, "binding
# command has an unmet outcome obligation", first met at a story's tests.run, where only the
# implementing writer is sent back). The list of those scenarios, with ESS's reason, ends the
# output: the writer sent back (spec.author) reads the last lines of the check's log. ess 0.96.0 and later
# give the per-scenario detail with `--format json` (ess-conformance-run/2). An ess that writes
# no readable result is a failed check, as before (exit 1).  spec_conform <suite path>
spec_conform() {
  local suite=$1 out rc json
  out=$(ess verify conform synthesize --path spec --suite-format 5 --out "$suite" 2>&1)
  echo "$out"
  grep -q ' 0 refusal occurrence' <<< "$out" || return 1
  json=$(ess verify conform run --path spec --target interpreted --suite "$suite" --report-format 2 \
    --report-out "${suite%.json}-interpreted.json" --strict --format json 2> "${suite%.json}-interpreted.err"); rc=$?
  if ! jq -e '.scenarios | type == "array"' <<< "$json" > /dev/null 2>&1; then
    echo "interpreted run: \`ess verify conform run --target interpreted\` exited $rc with no readable result: $(tail -n 3 "${suite%.json}-interpreted.err")"
    return 1
  fi
  jq -r --argjson rc "$rc" '[.scenarios[] | select(.status != "passed")] as $bad | (.scenarios | length) as $n
    | if ($bad | length) == 0 and $rc == 0 then "interpreted run: \($n) scenarios, all passed"
      else "interpreted run: \($bad | length) of \($n) scenarios did not pass against ESS'"'"'s interpreter of the specification (ess verify conform run --target interpreted --strict, exit \($rc)). The interpreter cannot carry them out, so no implementation can be held to them: change the specification so that each passes.",
        ($bad[] | "  \(.status) \(.scenario): " + ([.checks[]? | select(.status != "passed")
          | (.diagnostic.code // .code // "?") + ": " + ((.diagnostic.observed // []) | join("; "))] | join(" | ")))
      end' <<< "$json"
  [[ $rc == 0 ]] && jq -e 'all(.scenarios[]; .status == "passed")' <<< "$json" > /dev/null
}

case $1 in
  ess|ess-gate|spec-validate|spec-compile|spec-synthesize|spec-trace|plan-cover|plan-cover-shape) need ess ;;
  conform-run|story-tests|story-tests-run|impl-gate) need ess slot ;;
  stub-vision|stub-roadmap|stub-decompose|roadmap-validate|list-epics|list-stories|plan-validate|plan-scope|plan-status) need aep ;;
esac
# The case's aep store keeps aep's own lifecycles, not those of a project above the case directory
# (runs/ inside this repository is under development.standard); see llm-node.sh.
aep() { command aep "$@" --root .; }
# aep records who moved an artifact (AEP_ACTOR, else human:$USER): the acting principal, as in
# llm-node.sh: SDC_PRINCIPAL when it is a <kind>:<name> principal and no person, else agent:<action>.
executor=agent:${SDC_ACTION:-sdc-tool}
aep_actor=${SDC_PRINCIPAL:-}
[[ $aep_actor =~ ^[a-z][a-z-]*:[^[:space:]]+$ && $aep_actor != human:* && $aep_actor != person:* ]] || aep_actor=$executor
export AEP_ACTOR=$aep_actor

case $1 in
# --- stub LLM executors: fixed drafts -------------------------------------------------------
stub-file)
  # SDC_STUB_FIXTURES may hold the file, and <name>.<n> for the n-th call (from 0), counted as
  # stub-tree's calls are.
  f=$fx/$2
  if [[ -n ${SDC_STUB_FIXTURES:-} ]]; then
    [[ -f $SDC_STUB_FIXTURES/$2 ]] && f=$SDC_STUB_FIXTURES/$2
    if compgen -G "$SDC_STUB_FIXTURES/$2.[0-9]*" > /dev/null; then
      c=${SDC_STUB_COUNTER:-.sdc}/stub-file.$2
      mkdir -p "${c%/*}"
      n=$(cat "$c" 2>/dev/null || echo 0); echo $((n + 1)) > "$c"
      [[ -f $SDC_STUB_FIXTURES/$2.$n ]] && f=$SDC_STUB_FIXTURES/$2.$n
    fi
  fi
  cp "$f" "$3" ;;
stub-tree)
  # SDC_STUB_FIXTURES may hold the tree too: <name>.<n> on the n-th call (from 0), else <name>.
  # The count is kept in SDC_STUB_COUNTER (default .sdc; the fixture adapter names work/.sdc).
  f=$fx/$2
  if [[ -n ${SDC_STUB_FIXTURES:-} ]]; then
    c=${SDC_STUB_COUNTER:-.sdc}/stub-tree.${2//\//_}
    n=$(cat "$c" 2>/dev/null || echo 0); echo $((n + 1)) > "$c"
    [[ -d $SDC_STUB_FIXTURES/$2 ]] && f=$SDC_STUB_FIXTURES/$2
    [[ -d $SDC_STUB_FIXTURES/$2.$n ]] && f=$SDC_STUB_FIXTURES/$2.$n
  fi
  rm -rf "$3" && mkdir -p "$3" && cp -r "$f/." "$3/" ;;
stub-vision)
  [[ -f aep/vision/product.md ]] && exit 0
  aep plan artifact new vision product --store aep --title "Invoice billing" \
    --summary "Billing clerks issue invoices and customers are emailed about them" ;;
stub-roadmap)
  [[ -f aep/initiative/roadmap.md ]] && exit 0
  aep plan artifact new initiative roadmap --store aep --title "Roadmap" --summary "epics: billing" &&
  aep plan artifact new epic billing --store aep --title "Billing" --summary "invoices and their emails" ;;
stub-decompose)
  [[ -f aep/story/invoices.md ]] && exit 0
  cmds=$(jq -r '.commands | keys | join(" ")' .sdc/gate/ir.json)
  aep plan artifact new story invoices --store aep --title "Invoices" \
    --summary "covers $cmds" --relate delivers:epic:billing --relate serves:vision:product &&
  aep plan artifact scope story:invoices --store aep --add impl &&
  aep plan artifact move epic:billing --to active --store aep --executor "$executor" &&
  aep plan artifact move story:invoices --to active --store aep --executor "$executor" ;;

# --- sdc.intake checks ----------------------------------------------------------------------
check-idea) yq -e '.text | (tag == "!!str") and (length > 0)' idea.yaml >/dev/null ;;
check-dissection)
  [[ -f dissection.yaml ]] || exit 1
  text=$(yq -r '.text' idea.yaml)
  yq -e '(.items | length) > 0 and ([.items[] | select(.type as $t
    | ["fact","wish","belief","unknown","ambiguity","dark_spot"] | contains([$t]) | not)] | length) == 0' \
    dissection.yaml >/dev/null || exit 1
  yq -r '.items[].span' dissection.yaml | while IFS= read -r span; do
    [[ -n $span && $text == *"$span"* ]] && echo true || echo false
  done | all_true ;;
check-questions)
  # 0: shape valid, settled; 1: shape valid, open; 3: shape valid, a scope-changing answer not
  # given through scope.answer; 2: shape invalid
  [[ -f questions.yaml ]] || exit 2
  yq -e '(.questions | length) > 0 and ([.questions[] | select((.id // "") == "" or (.item // "") == ""
    or (.default // "") == "")] | length) == 0' questions.yaml >/dev/null || exit 2
  # A scope-changing question carries what a person needs to answer it cold: `excerpt`,
  # `default_reason` and `consequences`, a list parallel to `options`, each entry {does, cost}
  # (the shape sdc-decision-request parses). Each missing key is named with the question id.
  lacks=$(yq -o=json questions.yaml | jq -r '
    def txt: type == "string" and (gsub("^\\s+|\\s+$"; "") != "");
    .questions[] | select(.scope_changing == true) | .id as $id
    | [(.options // []) | if type == "array" then .[] | select(txt) else empty end] as $o
    | (.consequences // null) as $c
    | (if ($o | length) < 2 then "options (at least two)" else empty end),
      (if (.excerpt | txt) then empty else "excerpt" end),
      (if (.default_reason | txt) then empty else "default_reason" end),
      (if ($c | type) != "array" or ($c | length) != ($o | length) then "consequences (one entry per option)"
       else ($c | to_entries[] | select((.value | type) != "object" or (.value.does | txt | not) or (.value.cost | txt | not))
             | "consequences[\(.key)] does and cost") end)
    | "\($id): lacks \(.)"') || exit 2
  [[ -z $lacks ]] || { echo "check-questions: scope-changing question lacks a key" >&2; echo "$lacks" >&2; exit 2; }
  open=$(yq -r '.items[] | select(.type == "belief" or .type == "unknown" or .type == "ambiguity"
    or .type == "dark_spot") | .id' dissection.yaml)
  for i in $open; do
    [[ $(yq "[.questions[] | select(.item == \"$i\")] | length" questions.yaml) == 1 ]] || exit 1
  done
  # A scope-changing answer counts when it names the principal that took scope.answer (Canon admits
  # that action only under the capability scope.answer); the agent's own answers say non-interactive.
  yq -e '([.questions[] | select(.scope_changing == true and .status != "open"
    and ((.answered_by // "") == "" or .answered_by == "non-interactive"))] | length) == 0' \
    questions.yaml >/dev/null || exit 3
  yq -e '([.questions[] | select(.status != "accepted" and .status != "changed" and .status != "superseded")] | length) == 0' \
    questions.yaml >/dev/null || exit 1 ;;
check-requirements-shape)
  # The citation rule, the same program the amendment check uses (sdc-findings citation_fault):
  # one line per fault, so a writer sent back reads why (runs/trial-3-mixed-4).
  [[ -f requirements.yaml ]] || { echo "requirements: no requirements.yaml"; exit 1; }
  findings check-requirements; rc=$?
  passon $rc
  (( rc == 0 )) ;;

# --- sdc.direction checks -------------------------------------------------------------------
check-context-shape)
  [[ -f context.yaml ]] || exit 1
  # `offline` is optional (contexts written before it); when present it has the same shape.
  opt=$(yq -e 'has("offline")' context.yaml >/dev/null 2>&1 && echo offline)
  for k in audience source_language target_language platform kind deployment licence $opt; do
    yq -e ".$k.value and (.$k.status == \"default\" or .$k.status == \"accepted\" or .$k.status == \"changed\")" \
      context.yaml >/dev/null && echo true || echo false
  done | all_true ;;
roadmap-validate) [[ -f aep/initiative/roadmap.md ]] && aep plan artifact validate --strict --store aep ;;

# --- ESS: the gate as one program, and its parts --------------------------------------------
ess-gate)
  ess_gate() {
    mkdir -p .sdc/gate
    ess specify validate --path spec --strict-requires &&
    ess specify compile --path spec --out .sdc/gate/ir.json >/dev/null &&
    ! grep -rn 'UNMAPPED:' spec &&
    spec_conform .sdc/gate/suite.json
  }
  recorded ess_gate spec ess_gate ;;
spec-validate) ess specify validate --path spec --strict-requires ;;
spec-compile) ess specify compile --path spec --out .sdc/spec-ir.json ;;
spec-scan) grep -rn 'UNMAPPED:' spec; [[ $? == 1 ]] ;;
spec-synthesize) spec_conform .sdc/suite.json ;;
spec-trace-shape)
  # every domain the specification declares is cited by a requirement
  yq -r '.domains[]' spec/system.yaml | while read -r d; do
    yq -e "[.requirements[].spec_domains[]] | contains([\"$d\"])" requirements.yaml >/dev/null \
      && echo true || echo false
  done | all_true ;;
conform-run)
  # $2: the implementation tree (impl for a story, release for a release). The specification's
  # suite runs against ESS's built-in `billing` reference implementation when the spec is ESS's
  # billing example, else against ESS's interpreter of the spec (which checks the spec, not the
  # implementation: this tool does not start the product; ESS 0.97 has `--target process`, which sdc-conform uses). Any other spec also
  # needs the tree's own `task check` (a Taskfile.yml in the tree) to pass, run through slot.
  rm -f ".sdc/report-${SDC_ACTION:-conform-run}.json"
  # Build output goes outside the tree (an observation writes nothing into it) and is removed.
  export PYTHONDONTWRITEBYTECODE=1 CARGO_TARGET_DIR=$PWD/.sdc/target-check
  target=interpreted
  [[ $(yq -r '.system' spec/system.yaml) == billing ]] && target=billing
  # Each command's stderr goes with its stdout, in order, and the pool's own `slot: job` lines are
  # dropped: the log ends with the failing command's output and a line naming it, which is what a
  # writer sent back reads (runs/trial-3-haiku-8: the log ended with slot's line).
  out=$(ess verify conform synthesize --path spec --suite-format 5 --out .sdc/suite.json 2>&1) ||
    { tail -n 30 <<< "$out"; echo "conform-run: FAILED \`ess verify conform synthesize\`"; exit 1; }
  ess verify conform run --path spec --target $target --suite .sdc/suite.json --report-format 2 \
    --report-out ".sdc/report-${SDC_ACTION:-conform-run}.json" --strict 2>&1; rc=$?
  (( rc == 0 )) || { echo "conform-run: FAILED \`ess verify conform run --target $target\` (exit $rc)"; exit 1; }
  if [[ $target != billing ]]; then
    [[ -f ${2:?implementation tree}/Taskfile.yml ]] || { echo "conform-run: FAILED no $2/Taskfile.yml"; exit 1; }
    out=$(pool_run task --dir "$PWD/$2" check 2>&1); rc=$?
    rm -rf "$CARGO_TARGET_DIR"
    grep -v '^slot: job ' <<< "$out"
    (( rc == 0 || rc == 69 || rc == 70 )) || echo "conform-run: FAILED \`task check\` in $2 (exit $rc)"
    exit $rc
  fi ;;
story-tests-run)
  # sdc.story's tests.run: conform-run, then the interim implementation gate. Both always run, so
  # the log names every check that failed. The log ends with each failed step's last 30 lines under
  # a heading that names it, so a writer sent back reads them even when the full output is long.
  ca=$("$here/sdc-tool.sh" conform-run "${2:?implementation tree}" 2>&1); a=$?
  echo "$ca"; echo "conform-run: exit $a"
  passon $a
  cb=$("$here/sdc-tool.sh" impl-gate "$2" 2>&1); b=$?
  echo "$cb"
  passon $b
  (( a == 0 )) || { echo; echo "# failed: sdc-tool.sh conform-run $2 (exit $a), its last 30 lines"; tail -n 30 <<< "$ca"; }
  (( b == 0 )) || { echo; echo "# failed: sdc-tool.sh impl-gate $2 (exit $b), its last 30 lines"; tail -n 30 <<< "$cb"; }
  (( a == 0 && b == 0 )) ;;
impl-gate)
  # Interim story gate (docs/driver.md, "Interim story gate"), until ESS can run the suite against
  # an outside implementation: the agent's own tests exist, name every command the story covers,
  # and pass. It does not check behaviour against the specification. One line per check.
  tree=${2:?implementation tree} bad=0
  ok() { echo "impl-gate: ok     $1"; }
  no() { echo "impl-gate: FAILED $1"; bad=1; }
  # The commands the story covers: the specification's commands named after "covers" in the
  # story summary; every command of the specification when the case has no story file.
  ess specify compile --path spec --out .sdc/impl-gate-ir.json >/dev/null || { no "commands: spec does not compile"; exit 1; }
  all=$(jq -r '.commands | keys[]' .sdc/impl-gate-ir.json)
  story=$(cut -d. -f3 <<< "${SDC_CASE:-}") sf=""
  # Loom mode: the names Loom gave the cases, not a cut of the case id (a repeated case is IDEA.epic.2).
  if [[ -n ${LOOM_CASE:-} ]]; then
    if [[ -n ${LOOM_PARENT_CASE_NAME:-} ]]; then story=${LOOM_CASE_NAME:-$story}
    elif [[ -n ${LOOM_CASE_NAME:-} ]]; then story=""; fi
  fi
  [[ -n $story ]] && sf=aep/story/$story.md
  if [[ -n $sf && -f $sf ]]; then
    summary=$(yq --front-matter=extract -r '.summary // ""' "$sf")
    [[ $summary == *"covers "* ]] || { no "commands: $sf summary has no 'covers <command> ...'"; exit 1; }
    cmds=$(while read -r c; do grep -qwF "$c" <<< "${summary#*covers }" && echo "$c"; done <<< "$all")
    [[ -n $cmds ]] || { no "commands: $sf covers no command of the specification"; exit 1; }
  else
    echo "impl-gate: no story file for case '${SDC_CASE:-}': covering every command of the specification"
    cmds=$all
  fi
  ncmds=$(wc -l <<< "$cmds")
  echo "impl-gate: commands covered ($ncmds): $(tr '\n' ' ' <<< "$cmds")"
  [[ -d $tree ]] || { no "tree: no $tree/"; exit 1; }
  # The language part: Rust only. Another language adds a branch that sets the same four things:
  # source files, test code (a file of it), test count, and the test run's ran/ignored counts.
  [[ -f $tree/Cargo.toml ]] || { no "source: no $tree/Cargo.toml (a Rust crate is required)"; exit 1; }
  src=$(find "$tree/src" -type f -name '*.rs' 2>/dev/null | LC_ALL=C sort)
  [[ -n $src ]] && ok "source: $tree/Cargo.toml and $(wc -l <<< "$src") .rs file(s) under $tree/src" \
    || no "source: no .rs file under $tree/src"
  # Test code: every .rs under tests/, and each src file from its first #[cfg(test)] on.
  tc=.sdc/impl-gate-testcode.rs
  { find "$tree/tests" -type f -name '*.rs' 2>/dev/null | LC_ALL=C sort | xargs -r cat
    for f in $src; do awk '/#\[cfg\(test\)\]/ {t=1} t' "$f"; done; } > "$tc"
  nt=$(grep -cE '#\[(tokio::)?test\]' "$tc")
  (( nt >= ncmds && nt > 0 )) && ok "tests: $nt #[test] function(s) for $ncmds command(s)" \
    || no "tests: $nt #[test] function(s) in $tree/tests/*.rs and #[cfg(test)] code for $ncmds command(s), need at least one per command"
  # A command is named by its name after the last dot, as written (CreateTask) or snake_case
  # (create_task).
  missing=$(while read -r c; do
    n=${c##*.} s=$(sed -E 's/([a-z0-9])([A-Z])/\1_\2/g' <<< "${c##*.}" | tr '[:upper:]' '[:lower:]')
    grep -qF -e "$n" -e "$s" "$tc" || echo "$c"; done <<< "$cmds")
  [[ -z $missing ]] && ok "names: every covered command is named in the test code" \
    || no "names: not named in the test code (name after the last dot, or its snake_case): $(tr '\n' ' ' <<< "$missing")"
  # Run in a copy, so the observation writes nothing into the tree; the copy (with its target/)
  # is removed afterwards.
  rm -rf .sdc/impl-gate && cp -r "$tree" .sdc/impl-gate && rm -rf .sdc/impl-gate/target
  # The counts come from cargo's stdout alone. Mixed with stderr (2>&1, under slot and load), a
  # stderr line such as `Finished` or `Doc-tests` landed inside `test result: ok. 5 passed`, no
  # count was read and the gate failed "0 tests ran" on a passing suite.
  out=$(cd .sdc/impl-gate && CARGO_TARGET_DIR=$PWD/target pool_run cargo test --offline 2>../impl-gate.stderr)
  rc=$?
  err=$(cat .sdc/impl-gate.stderr)
  rm -rf .sdc/impl-gate .sdc/impl-gate.stderr
  # A panic is shown with its message (the lines after `panicked at` up to a blank line).
  awk '/panicked at/ {print; p = 1; next} p && NF {print; next} {p = 0}
    /^(test |test result:|error|warning: unused|sdc-tool: pool)|FAILED/ {print}' <<< "$err"$'\n'"$out" | tail -n 60 | sed 's/^/  | /'
  passon $rc
  results=$(grep -cE '^test result: ' <<< "$out")
  counted=$(grep -oE '^test result: [a-zA-Z]+\. [0-9]+ passed; [0-9]+ failed; [0-9]+ ignored' <<< "$out")
  # Output that cannot be read is a tool error (126: no evidence, the run stops), never a failure
  # of the writer's tree: a passing run with no `test result:` line, or one that does not parse.
  if (( rc == 0 )) && { (( results == 0 )) || (( $(grep -c . <<< "$counted") < results )); }; then
    echo "impl-gate: cargo test exit 0, its output could not be read ($results \`test result:\` line(s) on stdout, $(grep -c . <<< "$counted") parsed)"
    exit 126
  fi
  ran=$(awk '{s += $4 + $6} END {print s + 0}' <<< "$counted")
  ignored=$(awk '{s += $8} END {print s + 0}' <<< "$counted")
  if (( rc != 0 )); then no "run: cargo test exit $rc"
  elif (( ran < 1 )); then no "run: 0 tests ran"
  elif (( ignored > 0 )); then no "run: $ignored ignored (need 0)"
  else ok "run: cargo test exit 0, $ran tests ran, 0 ignored"
  fi
  exit $bad ;;

# --- arguments computed from the work tree (binding `args_from`) ----------------------------
list-epics)
  # every epic of the planning store, by name
  aep plan artifact list --store aep --format json \
    | jq -c '{epics: [.[] | select(.kind == "epic") | .id | sub("^epic:"; "")]}' ;;
list-stories)
  # the stories that deliver this case's epic (case id IDEA.<epic>[...])
  epic=$(cut -d. -f2 <<< "$SDC_CASE")
  # Loom mode: the epic is the name Loom gave the epic case (LOOM_CASE_NAME) or the parent's name.
  if [[ -n ${LOOM_CASE:-} ]]; then epic=${LOOM_PARENT_CASE_NAME:-${LOOM_CASE_NAME:-$epic}}; fi
  aep plan artifact list --store aep --format json | jq -c --arg e "epic:$epic" \
    '{stories: [.[] | select(.kind == "story" and any(.relations[]?; .relation == "delivers" and .target == $e))
      | .id | sub("^story:"; "")]}' ;;
spec-domain)
  # the system name the requirements' spec domains share (`<system>.<domain>`)
  yq -o=json requirements.yaml | jq -c '{domain: ([.requirements[].spec_domains[]?] | map(split(".")[0]) | unique | first // "system")}' ;;

# --- sdc.plan checks ------------------------------------------------------------------------
plan-validate) aep plan artifact validate --strict --store aep ;;
plan-cover-shape)
  # every command of the compiled specification is named by exactly one story
  [[ -d aep/story ]] || exit 1
  ess specify compile --path spec --out .sdc/cover-ir.json >/dev/null || exit 1
  jq -r '.commands | keys[]' .sdc/cover-ir.json | while read -r c; do
    [[ $(grep -lwF "$c" aep/story/*.md | wc -l) == 1 ]] && echo true || echo false
  done | all_true ;;
plan-scope)
  [[ $(aep plan artifact list --store aep --format json | jq '[.[] | select(.kind == "story")] | length') -gt 0 ]] &&
  aep plan artifact waves --kind story --format json --store aep | jq -e '(.unassessed | length) == 0' ;;
plan-status)
  aep plan artifact list --store aep --format json | jq -e '[.[] | select(.kind == "epic" or .kind == "story")]
    | (map(.kind) | contains(["epic","story"])) and all(.[]; .status == "active")' ;;

# --- sdc.story and sdc.release effects ------------------------------------------------------
merge) mkdir -p mainline && cp -r impl/. mainline/ ;;
release-build)
  rm -rf release && mkdir -p release && cp -r mainline/. release/ &&
  (cd spec && find . -type f | LC_ALL=C sort | xargs sha256sum) | sha256sum | cut -d' ' -f1 \
    > release/PROVENANCE ;;
publish) rm -rf published && mkdir -p published && cp -r release/. published/ ;;
observe) cmp -s published/PROVENANCE release/PROVENANCE ;;
review-spec) test -f spec/system.yaml && [[ ! -e spec/.review-reject ]] ;; # .review-reject: a fixture tree the stub reviewer rejects (docs/reject-routes.md R8)
review-change) test -s impl/README.md ;; # change.review's stub; the review itself: tools/llm-node.sh change.review

# --- the late-finding loop (docs/trial-2.md, "The late-finding loop") ------------------------
# Each stage's check is its own check, then the finding gate: exit 1 when its own check fails,
# 5 (result `finding`) while a finding is open, a consistency rule fires or (spec) a requirement
# traces to no behaviour in the spec, 0 otherwise. The
# story's conformance run checks the gate first, so no test runs while a finding is open.
check-requirements|check-context|spec-trace|plan-cover)
  stage=$(case $1 in check-requirements) echo requirements ;; check-context) echo context ;;
    spec-trace) echo spec ;; plan-cover) echo plan ;; esac)
  "$here/sdc-tool.sh" "$1-shape"; rc=$?
  passon $rc
  (( rc == 0 )) || exit 1
  # spec-trace: the gate also runs the requirement trace (sdc-findings `TRACES`), which needs ess.
  findings gate "$stage"; rc=$?
  passon $rc
  (( rc == 0 )) || exit 5 ;;
story-tests)
  findings gate story || exit 5
  recorded conformance_run "${2:?implementation tree}" "$here/sdc-tool.sh" story-tests-run "$2" ;;
amend) findings amend "${2:?stage}" ;;
# amend.upstream's handoff_when (sdc-drive): exit 0 while a finding waits for a person's decision:
# none yet, or one that names no principal (`decided_by: person:<id>`, `decider:<kind>`, or
# keep-going's `rule:keep-going`, run.unstick's `agent:unstick`).
findings-wait)
  [[ -f findings.yaml ]] &&
  yq -e '[.findings[]? | select(.status == "needs_person" and ((.decision // "") == ""
    or ((.decided_by // "") | test("^((person|decider):.|rule:keep-going$|agent:unstick$)") | not)))] | length > 0' findings.yaml >/dev/null 2>&1 ;;

# --- sdc.intake scope.answer (a manual action under the capability scope.answer) ---------------
scope-answer)
  # $2: the principal that took scope.answer. Every scope-changing question still open takes its
  # default; every scope-changing answer not yet attributed (edited in by that principal) is
  # recorded with it. The person's edits of questions.yaml before the approval are taken here
  # (sdc.intake r6 admits scope.answer on that not yet checked revision).
  [[ -f questions.yaml ]] || { echo "scope-answer: no questions.yaml yet: nothing to answer"; exit 1; }
  bad=$(yq -r '[.questions[] | select(.scope_changing == true and .status == "changed" and (.answer // "") == "") | .id] | join(" ")' questions.yaml) ||
    { echo "scope-answer: questions.yaml does not parse: fix it, then approve again"; exit 1; }
  [[ -z $bad ]] || { echo "scope-answer: status changed but no answer: $bad; set \`answer:\` in questions.yaml, then approve again"; exit 1; }
  P=${2:?principal} yq -i '(.questions[] | select(.scope_changing == true)) |= (
    (select(.status == "open") | .answer = .default | .status = "accepted") // .
    | (select((.answered_by // "") == "" or .answered_by == "non-interactive") | .answered_by = strenv(P)) // .)' \
    questions.yaml ;;
consistency) findings consistency ;;
# --- sdc.bugfix (docs/bugfix.md): its checks are the driver crate's sdc-bugfix binary -----------
bugfix) shift; bugfix "$@" ;;
# --- ess itself, as `runbook run --conformance observe` runs it: the release this script pins
ess) shift; ess "$@" ;;
stub-repro) "$here/sdc-tool.sh" stub-tree "$2" repro && bugfix repro-install ;;
*) echo "unknown tool $1" >&2; exit 64 ;;
esac
