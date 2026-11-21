#!/usr/bin/env bash
# The agent-node executor: one LLM call for one SDC writer action, bound as an argv list
# (`["{dir}/tools/llm-node.sh", "<action>"]`). Run by sdc-drive with cwd = the case tree's work/.
#
# It builds the prompt from the node's declared inputs only, copies those inputs into a scratch
# directory (in/), lets the LLM write there (out/), and then copies back ONLY the node's declared
# output artifact (for the AEP nodes: a deterministic `aep plan artifact` call made from the LLM's
# proposal). The LLM never writes work/ and never writes evidence: sdc-drive observes the new
# revision, and only the programs bound to the check actions produce evidence.
#
# Environment:
#   SDC_LLM_NODE     the binding's `llm` object (set by sdc-drive): model, max_turns,
#                    max_budget_usd, operations, stub
#   SDC_LLM_ADAPTER  stub (default: the binding's `stub` subcommand of sdc-tool.sh, no LLM) |
#                    dry (print the metaharness command, then run the stub) | metaharness |
#                    claude-cli | a path to an adapter program (contract: llm-adapter-metaharness.sh)
#   SDC_LLM_NODES    optional, space-separated node names: only these go to SDC_LLM_ADAPTER, every
#                    other node runs its stub (one node through the apply path, for example with
#                    tools/llm-adapter-fixture.sh)
#   SDC_LLM_MODEL    overrides the binding's model
#   SDC_LLM_MODEL_<CLASS> overrides both for the node's class (tools/node-classes.json): WRITER
#                    (spec.author, plan.decompose), IMPLEMENT (change.implement), REVIEW, OTHER
#   SDC_LLM_RECORD   where each call's record goes (default work/.sdc/llm); one NNN-<action>/ per call
#   SDC_LLM_LOG      JSON lines, one per call (default $SDC_LLM_RECORD/calls.jsonl)
#   SDC_FEEDBACK     set by sdc-drive on a retry: the failing check records and their logs
set -uo pipefail
here=$(cd "$(dirname "$0")" && pwd)
fx=$here/../fixtures
node=$1
# A run that does nothing ends here: exit 0, and sdc-drive is told (SDC_NOT_CALLED, a file it
# reads back) so the run is recorded `not_called` and not counted against max_runs.
not_called() { echo "$1"; [[ -n ${SDC_NOT_CALLED:-} ]] && printf '%s\n' "$1" > "$SDC_NOT_CALLED"; exit 0; }
# A missing program is a program error (exit 127: sdc-drive stops, records nothing), not a result.
for p in jq yq; do
  command -v "$p" >/dev/null 2>&1 || { echo "llm-node: $p is not on PATH: $node could not run" >&2; exit 127; }
done
# The case's aep store keeps aep's own lifecycles: without --root, aep takes the profile of any
# project above the case directory (runs/ inside this repository: development.standard, where a
# draft may only move to proposed), and plan.decompose's moves to active are refused.
aep() { command aep "$@" --root .; }
# aep records who moved an artifact (AEP_ACTOR, else human:$USER): the acting principal, the run's
# SDC_PRINCIPAL when it is a <kind>:<name> principal and no person (no person takes a tool step;
# dry-run is none), else agent:<node>; a move's --executor names the node that ran it.
aep_actor=${SDC_PRINCIPAL:-}
[[ $aep_actor =~ ^[a-z][a-z-]*:[^[:space:]]+$ && $aep_actor != human:* && $aep_actor != person:* ]] || aep_actor=agent:$node
export AEP_ACTOR=$aep_actor

# questions.write writes the interview; it never rewrites one that exists unless a check found it
# invalid (a retry, SDC_FEEDBACK set). An interview whose shape is only not checked yet (after an
# answer, an edit or an amendment) holds answers: in runs/trial-2-train the rewrite after
# amend.upstream wiped the operator's answers and the finding decision.
if [[ $node == questions.write && -s questions.yaml && -z ${SDC_FEEDBACK:-} ]]; then
  not_called "llm-node: questions.write not called: questions.yaml exists and no check found it invalid"
fi
cfg=${SDC_LLM_NODE:-'{}'}
adapter=${SDC_LLM_ADAPTER:-stub}
[[ -n ${SDC_LLM_NODES:-} && " $SDC_LLM_NODES " != *" $node "* ]] && adapter=stub
work=$PWD

run_stub() {
  local a
  mapfile -t a < <(jq -r '.stub[]' <<< "$cfg")
  "$here/sdc-tool.sh" "${a[@]}"
}

# --- the late-finding loop (docs/trial-2.md, "The late-finding loop") ---------------------------
# The stage agents that may raise a finding, and the tree each one derives from the requirements.
finding_nodes=" requirements.normalise context.default vision.write roadmap.write spec.author plan.decompose change.implement "
declare -A derived=([spec.author]=spec [change.implement]=impl [plan.decompose]=aep)
raises=no
[[ $finding_nodes == *" $node "* ]] && raises=yes

# While a finding waits for a person's decision, a stage agent is not called: whatever it wrote
# would be re-derived once the decision amends the requirements.
if [[ $raises == yes && -f $work/findings.yaml ]] &&
  yq -e '[.findings[] | select(.status == "needs_person" and (.decision // "") == "")] | length > 0' \
    "$work/findings.yaml" >/dev/null 2>&1; then
  not_called "llm-node: $node not called: findings wait for a person ($(yq -r '[.findings[] | select(.status == "needs_person")] | map(.id) | join(" ")' "$work/findings.yaml"))"
fi

# After the node's output is applied: its raised findings go through the program check into
# findings.yaml (a stage agent's finding counts only once that check accepts it), and a derived
# tree records the requirements revision it was derived from (`.derived-from`), so a re-derivation
# after an amendment is always a new revision.
post() {
  local raised=$1
  [[ $raises == yes ]] || return 0
  if [[ -z $raised && -n ${SDC_STUB_FINDINGS:-} && -f $SDC_STUB_FINDINGS ]] &&
    [[ $(yq -r '.node' "$SDC_STUB_FINDINGS") == "$node" ]]; then
    raised=$SDC_STUB_FINDINGS
    # A fixture with `rounds:` raises rounds[n] on the node's n-th applied run (0-based), then none.
    if yq -e 'has("rounds")' "$SDC_STUB_FINDINGS" >/dev/null 2>&1; then
      mkdir -p "$work/.sdc"
      local c=$work/.sdc/stub-round.$node n
      n=$(cat "$c" 2>/dev/null || echo 0); echo $((n + 1)) > "$c"
      raised=
      if (( n < $(yq '.rounds | length' "$SDC_STUB_FINDINGS") )); then
        raised=$work/.sdc/stub-findings.$node.yaml
        N=$n yq '{"node": .node, "findings": .rounds[env(N)]}' "$SDC_STUB_FINDINGS" > "$raised"
      fi
    fi
  fi
  if [[ -n $raised && -f $raised ]]; then
    local b=${SDC_FINDINGS:-$here/../../../target/debug/sdc-findings}
    [[ -x $b ]] || b=$HOME/beyond10x/sdc/driver/target/debug/sdc-findings
    "$b" --rules "$here/consistency-rules.json" promote "$node" "$raised"
  fi
  local t=${derived[$node]:-}
  if [[ -n $t && -d $work/$t ]]; then
    echo "requirements $(sha256sum < "$work/requirements.yaml" | cut -c1-16) resolved $(yq -r '[.findings[]? | select(.status == "resolved") | .id] | join(" ")' "$work/findings.yaml" 2>/dev/null)" \
      > "$work/$t/.derived-from"
  fi
  return 0
}

# A rejected approval of this writer's output (`approve --result rejected`): SDC_FEEDBACK names
# the record, and its log is the rejection (sdc.rejection/1: approval, revision, reasons,
# citations). The writer gets it as in/rejection.json and addresses each reason.
rejection=""
if [[ -n ${SDC_FEEDBACK:-} && -f $SDC_FEEDBACK ]]; then
  rejection=$(jq -r '[.[] | select(.result == "rejected") | .log // empty] | last // empty' "$SDC_FEEDBACK")
  [[ -n $rejection && -f $rejection ]] && jq -e '.format == "sdc.rejection/1"' "$rejection" > /dev/null 2>&1 || rejection=""
fi
# A rejection of this writer's subject still pending though SDC_FEEDBACK does not carry it (a later
# write by another action moved the subject's revision, so the rejected record is no longer
# current): the case's last rejected approval with no run of this writer after it that exited 0
# and was not `not_called`. It is addressed as if fed back, and the skip rule below does not apply.
cd_=${SDC_CASE_DIR:-}
if [[ -z $rejection && -n $cd_ && -f $cd_/events.jsonl ]]; then
  rejection=$(jq -rs --arg n "$node" '[.[] | select(.event == "action_finished")] as $f
    | ([$f | to_entries[] | select(.value.result == "rejected") | .key] | last) as $k
    | if $k == null or any($f[$k + 1:][]; .action == $n and .exit == "0" and (.not_called | type) != "string") then empty
      else $f[$k].log // empty end' "$cd_/events.jsonl" 2>/dev/null)
  [[ -n $rejection ]] && rejection=$cd_/blobs/$rejection
  [[ -n $rejection && -f $rejection ]] && jq -e --arg s "$(jq -r --arg n "$node" '.[$n].subject // ""' "$cd_/bindings.json" 2>/dev/null)" \
    '.format == "sdc.rejection/1" and .subject == $s' "$rejection" > /dev/null 2>&1 || rejection=""
fi

if [[ $adapter == stub ]]; then
  run_stub; code=$?
  [[ $code == 0 ]] && post ""
  # The stub addresses a rejection by recording it in the rejected artifact (the approval's
  # subject, at its `$artifacts` path), so the rework is a new revision: a tree gets `.rework`,
  # a YAML or Markdown file the same lines as a comment.
  t=""
  [[ -n $rejection ]] && t=$(jq -r --arg s "$(jq -r '.subject // ""' "$rejection")" '."$artifacts"[$s] // ""' "${SDC_CASE_DIR:-$work/..}/bindings.json" 2>/dev/null)
  if [[ $code == 0 && -n $t && -e $work/$t ]]; then
    lines=$(jq -r '"rework \(.count) of the rejection of \(.approval) at \(.revision):", (.reasons[] | "- " + .)' "$rejection")
    case $t in
      *.yaml|*.yml) [[ -d $work/$t ]] || sed 's/^/# /' <<< "$lines" >> "$work/$t" ;;
      *.md) printf '\n<!--\n%s\n-->\n' "$lines" >> "$work/$t" ;;
    esac
    [[ -d $work/$t ]] && printf '%s\n' "$lines" > "$work/$t/.rework"
  fi
  exit $code
fi

# A derived tree is not re-derived by the LLM when every decision taken since this node last wrote
# it is the default of a finding this node raised: the prompt has the node follow each default it
# raises (and each open finding's), so the tree already follows those decisions. Only the
# requirements digest in `.derived-from` is renewed (a new revision, so the checks run again). In
# runs/trial-3-haiku-2 every re-derivation raised one new agent finding (f14, f16, f17) until
# spec.author was spent. Not on a retry (a check sent the tree back) and not for the stubs, which
# follow nothing (they model a writer that never converges, cycle story-spent).
t=${derived[$node]:-}
if [[ $raises == yes && -n $t && -z ${SDC_FEEDBACK:-} && -z $rejection && -f $work/$t/.derived-from && -f $work/findings.yaml ]]; then
  first=$(head -n 1 "$work/$t/.derived-from")
  if [[ $first == "requirements "*" resolved"* ]]; then
    was=" ${first#* resolved} "
    new=$(yq -r '.findings[]? | select(.status == "resolved") | .id' "$work/findings.yaml" | while read -r id; do
      [[ $was == *" $id "* ]] || echo "$id"; done | tr '\n' ' ')
    own=$(N="llm:$node" yq -r '[.findings[]? | select(.status == "resolved" and .raised_by == strenv(N)
      and .resolution.default_taken == true) | .id] | join(" ")' "$work/findings.yaml")
    if [[ -n ${new// /} ]] && ! for id in $new; do [[ " $own " == *" $id "* ]] || echo x; done | grep -q x; then
      echo "requirements $(sha256sum < "$work/requirements.yaml" | cut -c1-16) resolved $(yq -r '[.findings[]? | select(.status == "resolved") | .id] | join(" ")' "$work/findings.yaml")" \
        > "$work/$t/.derived-from"
      not_called "llm-node: $node not called: the decisions since $t/ was written are the defaults it raised and follows (${new% }); $t/ marked as derived from the amended requirements"
    fi
  fi
fi

case $adapter in
  metaharness|claude-cli|dry) adapter_bin=$here/llm-adapter-$adapter.sh ;;
  *) adapter_bin=$adapter ;;
esac

rec_root=${SDC_LLM_RECORD:-$work/.sdc/llm}
mkdir -p "$rec_root"
n=$(( $(find "$rec_root" -mindepth 1 -maxdepth 1 -type d | wc -l) + 1 ))
rec=$rec_root/$(printf '%03d' "$n")-$node
cwd=$rec/cwd
mkdir -p "$cwd/in" "$cwd/out"
# status.json (sdc.call-status/1): running now, finished or failed on every exit; written atomically.
call_started=$(date -u +%Y-%m-%dT%H:%M:%S.%3NZ)
call_status() { # $1 state, $2 exit (absent while running)
  local t
  t=$(mktemp "$rec/.status.json.XXXXXX") || return 0
  jq -n --arg s "$1" --argjson pid $$ --arg at "$call_started" --arg exit "${2:-}" \
    --arg fin "$(date -u +%Y-%m-%dT%H:%M:%S.%3NZ)" '{format: "sdc.call-status/1", state: $s, pid: $pid, started_at: $at}
    + (if $exit == "" then {} else {finished_at: $fin, exit: ($exit | tonumber)} end)' > "$t" && mv "$t" "$rec/status.json"
}
call_status running
trap 'c=$?; [[ $c == 0 ]] && call_status finished 0 || call_status failed "$c"' EXIT
attempt=$(( $(find "$rec_root" -mindepth 1 -maxdepth 1 -type d -name "*-$node" | wc -l) ))
epic=$(cut -d. -f2 <<< "${SDC_CASE:-}")
story=$(cut -d. -f3 <<< "${SDC_CASE:-}")

# Copy one declared input from work/ into in/ (a missing one is listed as absent).
inputs=()
input() {
  local p=$1
  if [[ -e $work/$p ]]; then
    mkdir -p "$cwd/in/$(dirname "$p")" && cp -r "$work/$p" "$cwd/in/$p"
    inputs+=("in/$p")
  else
    inputs+=("in/$p (absent)")
  fi
}
# An output edited in place starts as a copy of the current artifact.
seed() { [[ -e $work/$1 ]] && mkdir -p "$cwd/out/$(dirname "$1")" && cp -r "$work/$1" "$cwd/out/$1"; true; }

ops='["dir.list","file.edit","file.read","file.write","search"]'
case $node in
idea.dissect)
  input idea.yaml; out=dissection.yaml
  task="Split the idea (in/idea.yaml, field \`text\`) into typed items. Write out/dissection.yaml with
one top-level key \`items\`, a list. Each item has exactly these keys:
- id: d1, d2, ...
- type: exactly one of fact, wish, belief, unknown, ambiguity, dark_spot
  (fact: stated as true; wish: a wanted outcome; belief: stated, unverified; unknown: a named gap;
  ambiguity: two or more readings; dark_spot: nothing said, but a builder needs it)
- span: a non-empty, exact, case-sensitive substring of the idea text, copied character by
  character (for a dark_spot, the nearest words it is about)
- note: one sentence: what is stated, or what is missing
At least one wish. At most 8 items." ;;
questions.write)
  input idea.yaml; input dissection.yaml; out=questions.yaml
  task="Write the interview. For every item in in/dissection.yaml whose type is belief, unknown,
ambiguity or dark_spot, write exactly one question; no question for any other item. Write
out/questions.yaml with one top-level key \`questions\`, a list. Each question has exactly these keys:
- id: q1, q2, ...
- item: the dissection item id it is about
- question: one sentence
- options: 2 to 4 short answers
- default: the recommended answer, one of the options, the simplest that fits the idea
- scope_changing: true when the answer adds or removes a user-visible capability, a kind of user
  or a component; else false
- status: open
A question with scope_changing: true has three more keys, so that a person can answer it without
reading anything else; a question with scope_changing: false has none of them:
- excerpt: the words of the idea (in/idea.yaml, field \`text\`) the question is about, copied
  character by character, as one string
- default_reason: one sentence: why the default is the recommended answer, citing the idea
- consequences: a list with exactly one entry per option, in the order of options. Each entry has
  exactly two keys, both one sentence in plain words: \`does\` (what is built, or not built, if that
  option is chosen) and \`cost\` (what that option costs: build size, upkeep, or what is given up)
Example of a scope-changing question (idea: 'A recipe box for my family'):
- id: q1
  item: d2
  excerpt: for my family
  question: Should family members have their own sign-in?
  options:
  - No sign-in; one shared recipe box
  - Each family member signs in
  default: No sign-in; one shared recipe box
  default_reason: The idea names one box for the family and no account for anyone.
  consequences:
  - does: Builds one recipe box that anyone with the link opens; no sign-in is built.
    cost: The smallest build; anyone holding the link can change recipes.
  - does: Builds sign-in and a recipe box that only signed-in family members open.
    cost: Accounts and a way to invite and remove members must be built and kept.
  scope_changing: true
  status: open" ;;
questions.answer)
  input idea.yaml; input dissection.yaml; input questions.yaml; seed questions.yaml; out=questions.yaml
  task="Answer the interview. out/questions.yaml is a copy of the current interview: edit it in place.
For every question with scope_changing: false, set \`status: accepted\` and \`answer\` to its default,
or \`status: changed\` and \`answer\` to a better answer when the default contradicts the idea, and
set \`answered_by: non-interactive\`. Leave every question with scope_changing: true exactly as it is,
whatever its status and answer (open, or already answered by a person): they are answered through
scope.answer, by the principal that holds that capability. Change nothing else." ;;
requirements.normalise)
  input idea.yaml; input dissection.yaml; input questions.yaml; out=requirements.yaml
  task="Normalise the idea's facts and wishes and the interview's answers into a requirements list.
Write out/requirements.yaml with one top-level key \`requirements\`, a list. Each requirement has
exactly these keys:
- id: r1, r2, ...
- text: one testable sentence: who does what, or what must hold
- cites: the ids it comes from: dissection items of type fact or wish, and question ids
  (q1, ...) whose answers it uses. Only ids that exist; at least one per requirement
- spec_domains: the specification domains it belongs to, each \`<system>.<domain>\` in lower case
  (for example todo.task). One system name for the whole list; 1 to 3 domains in total
Cite every fact, every wish and every answered question at least once. Keep the list short (at
most 8)." ;;
context.default)
  input idea.yaml; input requirements.yaml; out=context.yaml
  task="Write the project context defaults to out/context.yaml with exactly these top-level keys:
audience, source_language, target_language, platform, kind, deployment, offline, licence. Each is a
mapping with exactly the keys value (a short string), status (always \`default\`) and reason (one
sentence). source_language and target_language are human languages (for example en) of the input
and of the product's text. kind is one of: service, multi-tenant application, CLI, library.
offline says whether and how the product works without a network connection: \`no\`, or \`yes: \`
and how (for example \`yes: the browser client keeps a local copy and syncs when connected\`).
Pick the simplest defaults the requirements allow." ;;
vision.write)
  input idea.yaml; input requirements.yaml; input context.yaml; out=aep/vision/product.md
  task="Write the product vision. Write two files:
- out/vision.json: {\"title\": \"<3 to 6 words>\", \"summary\": \"<one sentence>\"}
- out/vision.md: the vision body in Markdown, starting with \`# <title>\`: who it is for, the
  problem, what it does, what it deliberately does not do (from the requirements), at most 40 lines." ;;
roadmap.write)
  input idea.yaml; input requirements.yaml; input context.yaml; input aep/vision/product.md
  out=aep/initiative/roadmap.md
  task="Write the roadmap. The idea is small, so the roadmap has exactly one epic. Write
out/roadmap.json: {\"summary\": \"<one sentence>\", \"epics\": [{\"name\": \"<one lower-case word>\",
\"title\": \"<2 to 5 words>\", \"summary\": \"<one sentence: what the epic delivers>\"}]}.
Also write out/roadmap.md, the roadmap's body, in markdown without a front matter: sections
'## Goal' (one paragraph from the vision), '## Epics' (per epic: what it delivers, the requirement
ids it covers, for example r1 r2, and what done means), '## Sequence' (the order and why),
'## Out of scope' (from the vision's 'does not do'). Every requirement id appears under one epic." ;;
spec.author)
  input idea.yaml; input requirements.yaml; input context.yaml; input spec; seed spec; out=spec
  mkdir -p "$cwd/in/example-spec" && cp -r "$fx/billing-spec/." "$cwd/in/example-spec/"
  inputs+=("in/example-spec (a complete, valid ESS example to copy the format from)")
  system=$(jq -r '.domain // "system"' <<< "${SDC_ARGS:-null}")
  task="Write the ESS (Executable System Specification) for the requirements into out/spec/.
in/example-spec/ is a complete specification that passes \`ess specify validate --strict-requires\`,
\`ess specify compile\` and \`ess verify conform synthesize\` with 0 refusals: copy its file layout,
keys and constructs exactly, and change only the content. Required:
- out/spec/system.yaml: format: ess/1, system: $system, version: v1, a summary, and \`domains\`
  listing exactly the spec_domains used in in/requirements.yaml
- one file per domain under out/spec/domains/ (as in the example), with the entities, their
  states, the commands (with the refusals they can return) and the events the requirements need
- components.yaml and topology.yaml as in the example, naming your domains
- out/spec/traces.json (as in the example): {\"traces\": {\"r1\": [...], ...}}, for every
  requirement of in/requirements.yaml the full names of the declarations that deliver it. Only
  behaviour counts: a command, a view, or an actor whose \`may\` names commands. A domain, an
  entity, an event, an error, a type or an actor with no \`may\` delivers nothing, so a requirement
  traced only to those fails the trace check. Leave out only a requirement with \`behaviour:
  constraint\` or \`behaviour: dropped\`. If a requirement cannot be specified as behaviour, do not
  trace it to a domain or an actor: raise a finding about it (scope_changing: true)
Keep it as small as the requirements allow: no construct the requirements do not need. Never
write the text UNMAPPED: anywhere. If out/spec/ already holds a specification, fix it rather than
starting over." ;;
plan.decompose)
  input requirements.yaml; input spec
  jq -r '.commands | keys[]' "$work/.sdc/gate/ir.json" > "$cwd/in/commands.txt" 2>/dev/null
  inputs+=("in/commands.txt (every command of the compiled specification)")
  out=aep
  task="Decompose epic '$epic' into 1 to 3 stories. Write out/stories.json:
{\"stories\": [{\"name\": \"<one lower-case word>\", \"title\": \"<2 to 5 words>\",
\"summary\": \"<one sentence, then: covers <command> <command> ...>\"}]}.
Every line of in/commands.txt must appear, spelled exactly, in the summary of exactly one story." ;;
change.implement)
  input requirements.yaml; input context.yaml; input spec; input "aep/story/$story.md"; input impl; seed impl
  out=impl
  task="Implement story '$story' (in/aep/story/$story.md) of the specification in in/spec/, in
out/impl/ (it holds the current implementation, if any: extend it, keep what earlier stories built).
- Rust, edition 2021, standard library only: no dependencies in Cargo.toml (there is no network).
  out/impl/Cargo.toml and code under out/impl/src/. A service is a daemon binary that serves its
  web UI and a JSON API with std::net::TcpListener, takes its port and data file as arguments, and
  can run as several instances side by side; a CLI parses std::env::args.
- Keep the domain logic in a library (src/lib.rs) so tests call it directly.
- Tests: #[test] functions in src (#[cfg(test)] modules) and/or out/impl/tests/*.rs, covering every
  command the story covers, including its refusals; they must pass without network access or a
  running server. At least one test per command; each command's name after its last dot
  (CreateTask for todo.task.CreateTask) appears in its test function's name (snake_case is fine,
  for example create_task) or in a comment directly above it. No #[ignore] and no #[should_panic]
  used to hide a failure.
- out/impl/Taskfile.yml: version: '3', with a task \`check\` that runs \`cargo test --locked\`
  (commit a Cargo.lock: with no dependencies it holds only the package itself).
- out/impl/README.md: how to run it and how it maps to the specification's commands.
You cannot run commands: write code that passes its own tests." ;;
*) echo "llm-node: no agent node $node" >&2; exit 64 ;;
esac

if [[ $raises == yes ]]; then
  [[ -f $work/findings.yaml ]] && input findings.yaml
  task="$task

Findings. If your inputs leave you an ambiguity (two or more readings), a contradiction (two
items that cannot both hold, for example the context against a requirement) or a missing decision
(something you must decide that no input decides), do not decide it silently. Write
out/findings.yaml with one top-level key \`findings\`, a list; each finding has exactly these keys:
- type: ambiguity, contradiction or missing_decision
- cites: the ids of the items involved (r1, q2, d3, or context.<field> such as
  context.deployment); at least one requirement, question or context field; a contradiction cites
  both sides
- says: one sentence
- options: 2 to 4 short answers
- default: the option you would pick, the simplest that fits the idea
- scope_changing: true when an option adds or removes a user-visible capability, a kind of user or
  a component; else false
Then still do the task, following each default. Write no out/findings.yaml when there is
nothing to report. A finding in in/findings.yaml with status resolved is a decision already
taken: follow its \`decision\`; never raise it again. A finding there with status open is already
raised and waits for its decision: follow its \`default\`; never raise it again."
fi

feedback=""
if [[ -n ${SDC_FEEDBACK:-} && -f $SDC_FEEDBACK ]] && jq -e 'any(.[]; .result != "rejected")' "$SDC_FEEDBACK" > /dev/null; then
  {
    echo "# Checks that failed on the current revision of your output"
    jq -r '.[] | select(.result != "rejected") | "\(.kind) = \(.result) (from \(.action // "?")): \(.log // "")"' "$SDC_FEEDBACK" | while IFS= read -r line; do
      echo; echo "## ${line%%: *}"; log=${line#*: }
      [[ -f $log ]] && { echo '```'; tail -n 60 "$log"; echo '```'; }
    done
  } > "$cwd/in/feedback.md"
  inputs+=("in/feedback.md (why the previous attempt was sent back: fix exactly that)")
  feedback=yes
fi
if [[ -n $rejection ]]; then
  cp "$rejection" "$cwd/in/rejection.json"
  inputs+=("in/rejection.json (the approval \`$(jq -r .approval "$rejection")\` of your previous output, revision $(jq -r .revision "$rejection"), was rejected by $(jq -r .principal "$rejection"): its reasons and what they cite)")
  task="$task

Rework. Your previous output was rejected at \`$(jq -r .approval "$rejection")\` (rejection $(jq -r .count "$rejection") of at most 2).
Address every reason in in/rejection.json; change only what a reason asks for. In your final reply,
before the word done, write one line per reason: its number, then where you addressed it as
<file>:<line> (or <file>:<first>-<last>), then what you changed."
fi

prompt=$rec/prompt.md
{
  echo "You are one step (\`$node\`) of a governed software delivery cycle. Work only in the current"
  echo "directory. Read the inputs under in/; write only under out/. You have no shell: no command"
  echo "runs. Programs check what you write afterwards; your words decide nothing."
  echo
  echo "Inputs:"
  printf -- '- %s\n' "${inputs[@]}"
  echo
  echo "Task:"
  echo "$task"
  echo
  if [[ -n $rejection ]]; then
    echo "When the files are written, reply with one line per rejection reason (as above), then the word: done."
  else
    echo "When the files are written, reply with the single word: done."
  fi
} > "$prompt"

# The model: the node's class setting (SDC_LLM_MODEL_<CLASS>; classes in tools/node-classes.json:
# writer, implement, review, other), else SDC_LLM_MODEL, else the binding's llm.model.
class=$(jq -r --arg n "$node" '[.classes | to_entries[] | select(.value | index($n)) | .key] | first // "other"' "$here/node-classes.json")
cvar=SDC_LLM_MODEL_${class^^}
model=${!cvar:-${SDC_LLM_MODEL:-$(jq -r '.model // "claude-haiku-5-5"' <<< "$cfg")}}
jq -n --arg model "$model" --arg prompt "$prompt" --arg cwd "$cwd" --arg rec "$rec" --arg node "$node" \
  --argjson attempt "$attempt" --argjson cfg "$cfg" --argjson ops "$ops" \
  --arg case "${SDC_CASE:-}" --arg case_dir "${SDC_CASE_DIR:-}" '{
    model: $model, prompt_file: $prompt, cwd: $cwd, record_dir: $rec, node: $node, attempt: $attempt,
    case: $case, case_dir: $case_dir,
    operations: ($cfg.operations // $ops), max_turns: ($cfg.max_turns // 30),
    max_budget_usd: ($cfg.max_budget_usd // 1)}' > "$rec/request.json"

started=$(date -u +%Y-%m-%dT%H:%M:%SZ)
"$adapter_bin" "$rec/request.json" > "$rec/response.json"
acode=$?
[[ -s $rec/response.json ]] || echo '{}' > "$rec/response.json"

# Apply: copy back only the declared output, through a program where the store is AEP's.
apply() {
  local f
  case $node in
    vision.write)
      [[ -f $cwd/out/vision.json && -f $cwd/out/vision.md ]] || return 1
      rm -f aep/vision/product.md
      aep plan artifact new vision product --store aep --title "$(jq -r .title "$cwd/out/vision.json")" \
        --summary "$(jq -r .summary "$cwd/out/vision.json")" --from "$cwd/out/vision.md" ;;
    roadmap.write)
      f=$cwd/out/roadmap.json
      [[ -f $f ]] && jq -e '.epics | length > 0' "$f" >/dev/null || return 1
      rm -f aep/initiative/roadmap.md aep/epic/*.md
      body=()
      [[ -s $cwd/out/roadmap.md ]] && body=(--from "$cwd/out/roadmap.md")
      aep plan artifact new initiative roadmap --store aep --title "Roadmap" \
        --summary "epics: $(jq -r '[.epics[].name] | join(" ")' "$f")" "${body[@]}" || return 1
      jq -c '.epics[]' "$f" | while IFS= read -r e; do
        aep plan artifact new epic "$(jq -r .name <<< "$e")" --store aep --title "$(jq -r .title <<< "$e")" \
          --summary "$(jq -r .summary <<< "$e")" || exit 1
      done ;;
    plan.decompose)
      f=$cwd/out/stories.json
      [[ -f $f ]] && jq -e '.stories | length > 0' "$f" >/dev/null || return 1
      for s in $(aep plan artifact list --store aep --format json | jq -r --arg e "epic:$epic" \
          '.[] | select(.kind == "story" and any(.relations[]?; .target == $e)) | .path'); do rm -f "aep/$s"; done
      jq -c '.stories[]' "$f" | while IFS= read -r s; do
        name=$(jq -r .name <<< "$s")
        aep plan artifact new story "$name" --store aep --title "$(jq -r .title <<< "$s")" \
          --summary "$(jq -r .summary <<< "$s")" --relate "delivers:epic:$epic" --relate serves:vision:product &&
        aep plan artifact scope "story:$name" --store aep --add impl &&
        aep plan artifact move "story:$name" --to active --store aep --executor "agent:$node" || exit 1
      done || return 1
      [[ $(aep plan artifact list --store aep --format json | jq -r --arg e "epic:$epic" '.[] | select(.id == $e) | .status') == active ]] ||
        aep plan artifact move "epic:$epic" --to active --store aep --executor "agent:$node" ;;
    context.default)
      f=$cwd/out/context.yaml
      [[ -f $f ]] || return 1
      # A person's decision (status changed or accepted) survives a new default pass.
      [[ -f $work/context.yaml ]] && yq -i ". *= (load(\"$work/context.yaml\")
        | with_entries(select(.value.status == \"changed\" or .value.status == \"accepted\")))" "$f"
      cp "$f" "$work/context.yaml" ;;
    questions.answer)
      # The agent may not touch a scope-changing question: those go through scope.answer. Both
      # sides quoted: an unquoted right side of == is a glob pattern, and JSON (`[`, `?`, `*`)
      # never matches itself as one (every real answer was refused in runs/trial-2-train).
      f=$cwd/out/questions.yaml
      [[ -f $f ]] || return 1
      [[ "$(yq -o=json -I=0 '[.questions[] | select(.scope_changing == true)]' "$f")" == \
         "$(yq -o=json -I=0 '[.questions[] | select(.scope_changing == true)]' "$work/questions.yaml")" ]] ||
        { echo "questions.answer changed a scope-changing question: not applied"; return 1; }
      cp "$f" "$work/questions.yaml" ;;
    *)
      [[ -e $cwd/out/$out ]] || return 1
      rm -rf "${work:?}/$out" && mkdir -p "$(dirname "$work/$out")" && cp -r "$cwd/out/$out" "$work/$out" ;;
  esac
}

if [[ $adapter == dry ]]; then
  run_stub; code=$?
  [[ $code == 0 ]] && post "" >> "$rec/apply.log" 2>&1
elif [[ $acode == 0 ]]; then
  apply > "$rec/apply.log" 2>&1; code=$?
  [[ $code == 0 ]] && post "$([[ -f $cwd/out/findings.yaml ]] && echo "$cwd/out/findings.yaml")" >> "$rec/apply.log" 2>&1
elif [[ $(jq -r '.terminal_reason // ""' "$rec/response.json") == api_error ]]; then
  # The model's API refused or broke off (a 429, an overload): the node could not run, which
  # says nothing about its subject. Exit 75: sdc-drive records nothing, counts no run and stops;
  # resume later on the same model.
  echo "llm-node: $node could not run: the model's API ended the session (api_error$(
    jq -r '(.api_error_status // empty) | " \(.)"' "$rec/response.json")); nothing applied, resume later"
  code=75
else
  code=$acode
fi

# One line per call, for the run log and the viewer.
jq -c -n --arg node "$node" --arg case "${SDC_CASE:-}" --arg started "$started" --arg rec "$rec" \
  --argjson attempt "$attempt" --argjson code "$code" --argjson acode "$acode" --arg out "$out" --arg class "$class" \
  --arg feedback "$feedback" --slurpfile req "$rec/request.json" --slurpfile res "$rec/response.json" '
  {action: $node, case: $case, started: $started, attempt: $attempt, retry: ($feedback == "yes"),
   adapter: $res[0].adapter, model: $req[0].model, class: $class, allowed_operations: $req[0].operations,
   max_turns: $req[0].max_turns, max_budget_usd: $req[0].max_budget_usd,
   exit: $acode, applied_exit: $code, output: $out, duration_ms: $res[0].duration_ms,
   cost_usd: $res[0].cost_usd, num_turns: $res[0].num_turns, usage: $res[0].usage,
   offered_tools: $res[0].offered_tools, tool_calls: $res[0].tool_calls, dry: ($res[0].dry // false),
   record: $rec}' >> "${SDC_LLM_LOG:-$rec_root/calls.jsonl}"
exit $code
