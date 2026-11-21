#!/usr/bin/env bash
# Grounding check of a proposed recommendation (ess/domains/decision.yaml, sdc.decision.Recommend).
#   decision-grounds.sh <proposal.json> [--case DIR] [--root DIR] [--log decisions.jsonl]
# proposal: {request: <DecisionRequest>, proposal: {recommended_option, strength, default_on_silence,
#            grounds, proposed_by}}. Prints the Recommend input with check_verdict and check_report,
# plus checks (one line per ground) and reuse_of. Exit 0 verified, 1 refused, 64 usage.
# Rules: at least one ground; every citation resolves (path:line in range, record id present,
# artifact@revision current); a ground supports the recommended option. A Decided request in --log
# with the same question_key and subject revision is reused when its grounds still resolve unchanged.
set -uo pipefail
in=${1:?usage: decision-grounds.sh <proposal.json> [--case DIR] [--root DIR] [--log FILE]}; shift
case_dir=. root=$(cd "$(dirname "$0")/.." && pwd) log=/dev/null
while (($#)); do case $1 in --case) case_dir=$2 ;; --root) root=$2 ;; --log) log=$2 ;; *) exit 64 ;; esac; shift 2; done
req=$(jq -c .request "$in") || exit 64
letters=$(jq -c '[.options[].letter]' <<< "$req")

# One citation -> {ok, why, digest}. A digest on a path_line citation must still match.
resolve() {
  local c=$1 form v f n l e d
  form=$(jq -r .form <<< "$c"); v=$(jq -c .value <<< "$c")
  case $form in
    path_line)
      f=$(jq -r .path <<< "$v"); l=$(jq -r .line <<< "$v"); e=$(jq -r ".line_end // $l" <<< "$v")
      for base in "$case_dir" "$root"; do [[ -f $base/$f ]] && { f=$base/$f; break; }; done
      [[ -f $f ]] || { echo "{\"ok\":false,\"why\":\"no file $(jq -r .path <<< "$v")\"}"; return; }
      n=$(wc -l < "$f")
      ((l >= 1 && e >= l && e <= n)) || { echo "{\"ok\":false,\"why\":\"lines $l-$e outside $(jq -r .path <<< "$v") ($n lines)\"}"; return; }
      d=sha256:$(sed -n "${l},${e}p" "$f" | sha256sum | cut -d' ' -f1)
      jq -r '.digest // empty' <<< "$v" | grep -qv "^$d$" && { echo "{\"ok\":false,\"why\":\"changed: lines $l-$e of $(jq -r .path <<< "$v") differ from the cited digest\"}"; return; }
      echo "{\"ok\":true,\"why\":\"$(jq -r .path <<< "$v"):$l-$e\",\"digest\":\"$d\"}" ;;
    record_id)
      local s id hit=no; s=$(jq -r .store <<< "$v"); id=$(jq -r .record_id <<< "$v")
      case $s in
        canon_evidence) jq -e --arg i "$id" 'select(.id == $i)' "$case_dir"/evidence/*.json >/dev/null 2>&1 && hit=yes ;;
        # Record ids: <capability>@<case_revision> (authority), <outcome>/<decision>@<case_revision>
        # (decisions). The revision is matched only when the entry carries one. The bare forms
        # (<capability>, <decision>@<outcome>) go when Canon's story authority-revision-binding lands.
        canon_authority) jq -e --arg i "$id" '($i | if test("@") then {c: sub("@[^@]*$"; ""), r: sub("^.*@"; "")} else {c: ., r: null} end) as $k
          | any(.[]; .capability == $k.c and ($k.r == null or .case_revision == null or .case_revision == $k.r))' "$case_dir/authority.json" >/dev/null 2>&1 && hit=yes ;;
        canon_decisions) jq -e --arg i "$id" 'if ($i | test("/")) then ($i | capture("^(?<o>[^/]*)/(?<d>[^@]*)@(?<r>.*)$")) as $k
            | any(.[]; .outcome == $k.o and .decision == $k.d and (.case_revision == null or .case_revision == $k.r))
          else any(.[]; "\(.decision)@\(.outcome)" == $i) end' "$case_dir/decisions.json" >/dev/null 2>&1 && hit=yes ;;
        sdc_decisions) jq -e --arg i "$id" 'select(.request_id == $i and .state == "Decided")' "$log" >/dev/null 2>&1 && hit=yes ;;
        aep_artifact) [[ -f $case_dir/work/aep/${id%%:*}/${id#*:}.md ]] && hit=yes ;;
      esac
      [[ $hit == yes ]] && echo "{\"ok\":true,\"why\":\"$s $id\"}" || echo "{\"ok\":false,\"why\":\"no record $id in $s\"}" ;;
    artifact_revision)
      local a r cid cj cur; a=$(jq -r .artifact <<< "$v"); r=$(jq -r .revision <<< "$v")
      cid=$(jq -r --arg c "$(jq -r .case_id <<< "$req")" '.case // $c' <<< "$v"); cj=$case_dir/case.json
      [[ $(jq -r .id "$cj" 2>/dev/null) == "$cid" ]] || cj=$case_dir/children/$cid/case.json
      cur=$(jq -r --arg a "$a" 'if $a == "case" then .revision else .artifacts[$a].revision // "absent" end' "$cj" 2>/dev/null || echo "no case $cid")
      [[ $cur == "$r" ]] && echo "{\"ok\":true,\"why\":\"$a@$r current\"}" || echo "{\"ok\":false,\"why\":\"$a@$r is not current ($cur)\"}" ;;
    *) echo "{\"ok\":false,\"why\":\"unknown citation form $form\"}" ;;
  esac
}

# Check a list of grounds; prints {ok, checks, grounds (with digests)}.
check() {
  local gs=$1 out='[]' g r
  while IFS= read -r g; do
    [[ -n $g ]] || continue
    r=$(resolve "$(jq -c .citation <<< "$g")")
    out=$(jq -c --argjson g "$g" --argjson r "$r" '. + [{ground: ($g | if $r.digest then .citation.value.digest = $r.digest else . end), ok: $r.ok, why: $r.why}]' <<< "$out")
  done < <(jq -c '.[]' <<< "$gs")
  jq -c '{ok: all(.[]; .ok), checks: [.[] | "\(if .ok then "ok" else "REFUSED" end) \(.ground.source) \(.why)"], grounds: [.[].ground]}' <<< "$out"
}

# 1. A prior decision on the same question and subject revision, its grounds unchanged, is reused.
reuse=null notes='[]'
while IFS= read -r p; do
  [[ -n $p ]] || continue
  c=$(check "$(jq -c .grounds <<< "$p")"); pid=$(jq -r .request_id <<< "$p")
  if jq -e '.ok and (.grounds | length > 0)' <<< "$c" >/dev/null; then
    reuse=$(jq -c --arg id "$pid" --argjson c "$c" '{recommended_option: .decided_option, strength: "strong", default_on_silence: false, proposed_by: "prior_decision",
      grounds: ([{source: "prior_decision", citation: {form: "record_id", value: {store: "sdc_decisions", record_id: $id}}, supports: .decided_option,
        how: "The same question on the same subject revision was decided \(.decided_option) by \(.decided_by); its grounds are unchanged."}] + $c.grounds)}' <<< "$p"); break
  fi
  notes=$(jq -c --arg n "prior $pid not reused: $(jq -r '[.checks[] | select(startswith("REFUSED"))] | join("; ")' <<< "$c")" '. + [$n]' <<< "$notes")
done < <(jq -c --argjson q "$req" 'select(.state == "Decided" and .question_key == $q.question_key and .subject_artifact == $q.subject_artifact
  and .subject_revision == $q.subject_revision and .request_id != $q.request_id)' "$log" 2>/dev/null)
prop=$(if [[ $reuse != null ]]; then echo "$reuse"; else jq -c .proposal "$in"; fi)

# 2. The proposal (or the reuse) itself.
c=$(check "$(jq -c '.grounds // []' <<< "$prop")")
subj='{"ok":true}'
[[ -f $case_dir/case.json ]] && subj=$(resolve "$(jq -c '{form: "artifact_revision", value: {artifact: .subject_artifact, revision: .subject_revision}}' <<< "$req")")
res=$(jq -n --argjson req "$req" --argjson p "$prop" --argjson c "$c" --argjson L "$letters" --argjson notes "$notes" --argjson reuse "$reuse" --argjson subj "$subj" '
  ([ if $subj.ok then empty else "the subject moved, supersede the request: \($subj.why)" end,
     if ($p.grounds // [] | length) == 0 then "no ground cited" else empty end,
     if $p.recommended_option == null then "no option proposed (the proposer escalated)"
     elif ($L | index($p.recommended_option)) == null then "option \($p.recommended_option) is not an option of the request" else empty end,
     if ($p.strength // "none") == "none" then "strength none" else empty end,
     if $p.default_on_silence and $p.strength != "strong" then "a default on silence needs a strong recommendation" else empty end,
     (($p.grounds // [])[] | .supports as $s | select(($L | index($s)) == null) | "a ground supports \($s), not an option"),
     if any(($p.grounds // [])[]; .supports == $p.recommended_option) | not then "no ground supports \($p.recommended_option)" else empty end,
     ($c.checks[] | select(startswith("REFUSED")))
  ]) as $why
  | {recommend: {request_id: $req.request_id, recommended_option: $p.recommended_option, strength: $p.strength, default_on_silence: $p.default_on_silence,
      grounds: $c.grounds, proposed_by: $p.proposed_by, check_verdict: (if $why == [] then "verified" else "refused" end),
      check_report: (if $why == [] then "\($c.grounds | length) grounds, all resolve" + (if $reuse then "; reuses \($p.grounds[0].citation.value.record_id)" else "" end)
                     else "no recommendation: " + ($why | join("; ")) end)},
     checks: ($notes + $c.checks), reuse_of: (if $reuse then $p.grounds[0].citation.value.record_id else null end)}') || exit 64
echo "$res"
jq -e '.recommend.check_verdict == "verified"' <<< "$res" >/dev/null
