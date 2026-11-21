#!/usr/bin/env bash
# Turn one decider record (runs/<run>/decider/NNN-*/) into a decision-grounds.sh proposal: the
# request the decision answered, and the decider's reply as a proposed recommendation whose grounds
# are its cites, each cite becoming a path:line citation of the item in the case's work/ files.
#   decision-from-record.sh <record dir> [A0..A5] [holder]    (prints the proposal JSON; reads only)
set -uo pipefail
rec=${1:?usage: decision-from-record.sh <record dir> [level] [holder]}; level=${2:-A4}; holder=${3:-decider}
q=$rec/decide-request.json; out=$rec/cwd/out/decision.json
cdir=$(jq -r .case_dir "$q"); cid=$(jq -r .id "$cdir/case.json")
# A cite is an item id in requirements.yaml, questions.yaml or dissection.yaml of the case's work/.
ground() {
  local id=$1 f l e src
  for f in requirements questions dissection; do
    l=$(grep -n -- "- id: $id\$" "$cdir/work/$f.yaml" 2>/dev/null | head -1 | cut -d: -f1)
    [[ -n $l ]] && break
  done
  if [[ -z $l ]]; then f=requirements; l=0; e=0; src=requirement
  else
    e=$(awk -v l="$l" 'NR > l && /^  - id: / {print NR - 1; exit}' "$cdir/work/$f.yaml"); [[ -n $e ]] || e=$(wc -l < "$cdir/work/$f.yaml")
    case $f in requirements) src=requirement ;; questions) src=prior_decision ;; dissection) src=idea ;; esac
  fi
  jq -cn --arg id "$id" --arg p "work/$f.yaml" --argjson l "$l" --argjson e "$e" --arg s "$src" --arg o "$2" --arg how "$3" \
    '{source: $s, citation: {form: "path_line", value: {path: $p, line: $l, line_end: $e}}, supports: $o, how: "\($id): \($how)"}'
}
d=$(jq -r .decision "$out"); opt=$(case $d in approve) echo A ;; reject) echo B ;; *) echo null ;; esac)
how=$(jq -r '.reasons[0]' "$out")
gs=$(for c in $(jq -r '.cites[]?' "$out"); do ground "$c" "$opt" "$how"; done | jq -sc .)
jq -n --slurpfile q "$q" --arg cid "$cid" --arg lv "$level" --arg h "$holder" --arg opt "$opt" --argjson gs "$gs" '$q[0] as $q | {
  request: {request_id: "DR-\($cid)-\($q.action)-\($q.revision // "close")", state: "Open", case_id: $cid,
    question_key: "\($cid)|\($q.action)|", unblocks: {kind: (if $q.type == "close" then "outcome" else $q.type end), action: $q.action,
      capability: $q.capability, outcome: $q.outcome},
    subject_artifact: ($q.subject // "case"), subject_revision: ($q.revision // "unrecorded"), subject_path: $q.path,
    question: (if $q.type == "close" then "Close the case through \($q.outcome)?" else "Approve \($q.action) on \($q.subject) at \($q.revision)?" end),
    options: (if $q.type == "close"
      then [{letter: "A", does: "decide \($q.outcome)", cost: "", consequence: {admits: [$q.action], blocks: [], produces: "canon_decisions"}},
            {letter: "B", does: "leave it untaken", cost: "", consequence: {admits: [], blocks: [$q.action]}}]
      else [{letter: "A", does: "approve", cost: "", consequence: {admits: [$q.action], blocks: [], grant: "granted", produces: "canon_authority"}},
            {letter: "B", does: "reject", cost: "", consequence: {admits: [], blocks: [$q.action], grant: "denied", produces: "canon_authority"}}] end),
    autonomy_level: $lv, holder: $h, eligible: ["decider:haiku"], addressed_to: "decider:haiku",
    opened_at: "2026-11-21T00:00:00Z", recommendation_strength: "none", default_on_silence: false, grounds: [], decision_reasons: []},
  proposal: {recommended_option: (if $opt == "null" then null else $opt end), strength: "moderate", default_on_silence: false,
    proposed_by: "llm", grounds: $gs}}'
