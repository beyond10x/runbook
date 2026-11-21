#!/usr/bin/env bash
# The SDC example's tools, one subcommand per binding. Run by sdc-drive with cwd = the case tree's
# shared work/ directory. Each check is deterministic and reports only through its exit code; each
# `stub-*` is a stand-in for an LLM executor: it writes a fixed draft and decides nothing.
set -uo pipefail
here=$(cd "$(dirname "$0")/.." && pwd)
fx=$here/fixtures
mkdir -p .sdc

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
passon() { (( $1 == 126 || $1 == 127 )) && exit "$1"; return 0; }
need yq jq

# The late-finding programs: the driver crate's `sdc-findings` binary (driver/src/bin).
findings() {
  local b=${SDC_FINDINGS:-$here/../../target/debug/sdc-findings}
  [[ -x $b ]] || b=$HOME/beyond10x/sdc/driver/target/debug/sdc-findings
  [[ -x $b ]] || { echo "sdc-tool: sdc-findings not found ($b)" >&2; exit 127; }
  "$b" --rules "$here/tools/consistency-rules.json" "$@"
}
case $1 in
  ess-gate|spec-validate|spec-compile|spec-synthesize|spec-trace|plan-cover|plan-cover-shape) need ess ;;
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
  # As stub-tree: SDC_STUB_FIXTURES may hold <name>.<n> for the n-th call (from 0), else <name>.
  f=$fx/$2
  if [[ -n ${SDC_STUB_FIXTURES:-} ]]; then
    c=${SDC_STUB_COUNTER:-.sdc}/stub-file.$2
    mkdir -p "$(dirname "$c")"
    n=$(cat "$c" 2>/dev/null || echo 0); echo $((n + 1)) > "$c"
    [[ -f $SDC_STUB_FIXTURES/$2 ]] && f=$SDC_STUB_FIXTURES/$2
    [[ -f $SDC_STUB_FIXTURES/$2.$n ]] && f=$SDC_STUB_FIXTURES/$2.$n
  fi
  cp "$f" "$3" ;;
stub-tree)
  # SDC_STUB_FIXTURES may hold the tree too: <name>.<n> on the n-th call (from 0), else <name>.
  # The count is kept in SDC_STUB_COUNTER (default .sdc; the fixture adapter names work/.sdc).
  f=$fx/$2
  if [[ -n ${SDC_STUB_FIXTURES:-} ]]; then
    c=${SDC_STUB_COUNTER:-.sdc}/stub-tree.$2
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
  mkdir -p .sdc/gate
  ess specify validate --path spec --strict-requires &&
  ess specify compile --path spec --out .sdc/gate/ir.json >/dev/null &&
  ! grep -rn 'UNMAPPED:' spec &&
  ess verify conform synthesize --path spec --suite-format 5 --out .sdc/gate/suite.json \
    | grep -q ' 0 refusal occurrence' ;;
spec-validate) ess specify validate --path spec --strict-requires ;;
spec-compile) ess specify compile --path spec --out .sdc/spec-ir.json ;;
spec-scan) grep -rn 'UNMAPPED:' spec; [[ $? == 1 ]] ;;
spec-synthesize)
  ess verify conform synthesize --path spec --suite-format 5 --out .sdc/suite.json \
    | tee /dev/stderr | grep -q ' 0 refusal occurrence' ;;
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
  # implementation: ESS 0.96.0 has no target that runs an outside program). Any other spec also
  # needs the tree's own `task check` (a Taskfile.yml in the tree) to pass, run through slot.
  rm -f ".sdc/report-$SDC_ACTION.json"
  # Build output goes outside the tree (an observation writes nothing into it) and is removed.
  export PYTHONDONTWRITEBYTECODE=1 CARGO_TARGET_DIR=$PWD/.sdc/target-check
  target=interpreted
  [[ $(yq -r '.system' spec/system.yaml) == billing ]] && target=billing
  ess verify conform synthesize --path spec --suite-format 5 --out .sdc/suite.json >/dev/null &&
  ess verify conform run --path spec --target $target --suite .sdc/suite.json --report-format 2 \
    --report-out ".sdc/report-$SDC_ACTION.json" --strict || exit 1
  if [[ $target != billing ]]; then
    [[ -f ${2:?implementation tree}/Taskfile.yml ]] || { echo "no $2/Taskfile.yml"; exit 1; }
    slot task --dir "$PWD/$2" check; rc=$?
    rm -rf "$CARGO_TARGET_DIR"
    exit $rc
  fi ;;
story-tests-run)
  # sdc.story's tests.run: conform-run, then the interim implementation gate. Both always run, so
  # the log names every check that failed.
  "$here/tools/sdc-tool.sh" conform-run "${2:?implementation tree}"; a=$?
  echo "conform-run: exit $a"
  passon $a
  "$here/tools/sdc-tool.sh" impl-gate "$2"; b=$?
  passon $b
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
  out=$(cd .sdc/impl-gate && CARGO_TARGET_DIR=$PWD/target slot cargo test --offline 2>&1)
  rc=$?
  rm -rf .sdc/impl-gate
  grep -E '^(test |test result:|error|warning: unused)|panicked|FAILED' <<< "$out" | tail -n 60 | sed 's/^/  | /'
  ran=$(grep -oE '^test result: [a-zA-Z]+\. [0-9]+ passed; [0-9]+ failed' <<< "$out" | awk '{s += $4 + $6} END {print s + 0}')
  ignored=$(grep -oE '[0-9]+ ignored' <<< "$out" | awk '{s += $1} END {print s + 0}')
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
review-spec) test -f spec/system.yaml ;;
review-change) test -s impl/README.md ;;

# --- the late-finding loop (docs/trial-2.md, "The late-finding loop") ------------------------
# Each stage's check is its own check, then the finding gate: exit 1 when its own check fails,
# 5 (result `finding`) while a finding is open, a consistency rule fires or (spec) a requirement
# traces to no behaviour in the spec, 0 otherwise. The
# story's conformance run checks the gate first, so no test runs while a finding is open.
check-requirements|check-context|spec-trace|plan-cover)
  stage=$(case $1 in check-requirements) echo requirements ;; check-context) echo context ;;
    spec-trace) echo spec ;; plan-cover) echo plan ;; esac)
  "$here/tools/sdc-tool.sh" "$1-shape"; rc=$?
  passon $rc
  (( rc == 0 )) || exit 1
  # spec-trace: the gate also runs the requirement trace (sdc-findings `TRACES`), which needs ess.
  findings gate "$stage"; rc=$?
  passon $rc
  (( rc == 0 )) || exit 5 ;;
story-tests)
  findings gate story || exit 5
  "$here/tools/sdc-tool.sh" story-tests-run "${2:?implementation tree}" ;;
amend) findings amend "${2:?stage}" ;;
# amend.upstream's handoff_when (sdc-drive): exit 0 while a finding waits for a person's decision:
# none yet, or one that names no principal (`decided_by: person:<id>` or `decider:<kind>`).
findings-wait)
  [[ -f findings.yaml ]] &&
  yq -e '[.findings[]? | select(.status == "needs_person" and ((.decision // "") == ""
    or ((.decided_by // "") | test("^(person|decider):.") | not)))] | length > 0' findings.yaml >/dev/null 2>&1 ;;

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
*) echo "unknown tool $1" >&2; exit 64 ;;
esac
