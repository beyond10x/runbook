#!/usr/bin/env bash
# The late-finding loop's three scripted stub runs (docs/trial-2.md, "Verification"), each a full
# cycle with run-full-cycle.sh into <scratch>/runs/<name> and <scratch>/store-<name>:
#   stub-cycle      no finding: the baseline cycle
#   finding-agent   spec.author's stub raises a missing decision that changes no scope: resolved by
#                   the default, no person
#   finding-person  spec.author's stub raises a scope-changing ambiguity: the run stops for a person
#                   (exit 3 first, without FINDING_DECISION), then the scripted person picks
#                   option 2 and the cycle completes
#   scope-answer    the interview has a scope-changing question (fixtures/scope-question): the run
#                   waits for the capability scope.answer, the scripted person takes it (accepting
#                   the default), and the cycle completes
#   finding-program trial 1's case on the billing idea (fixtures/storage-conflict): the context's
#                   deployment, as a person set it, names a daemon with a data file, requirement r3
#                   keeps drafts in browser storage; context.check's rule finds it, the agent
#                   resolves it (the person's own decision ranks higher), no person
#   scope-changed   as scope-answer, but the person changes the scope-changing answer in
#                   questions.yaml before taking scope.answer, and questions.answer runs through
#                   llm-node.sh's apply step (tools/llm-adapter-fixture.sh, no LLM)
#   intake-decision as scope-changed, plus a check run without yq, edit-resume-approve, and a
#                   scope-changing intake finding the person decides with a non-default option
#   offline-sync    a hosted server whose client works offline: rule offline-hosted stays silent
#   offline-decision the agent's rule amendment contradicts a decision a requirement cites:
#                   reverted, the run stops for a person (exit 3)
#   plan-in-project the case directory lies inside a project with a stricter aep profile: the
#                   plan stage's moves to active still succeed
#   story-decision  a person decides a story-stage finding raised by change.implement: the
#                   writer gets a fresh allowance and the cycle completes
#   story-spent     change.implement raises a new finding on every run: `writer_spent`
#   story-late-person the spent writer's last finding needs a person: `awaiting_person`, then a
#                   fresh allowance
#   spec-duplicate  spec.author raises the same finding twice: the repeat is not added
#   spec-own-default spec.author (apply path) raises a new agent finding on every LLM call: the
#                   amendment records its default, no LLM re-derivation, no writer_spent
#   trace-gap       a requirement traced to a domain with no command: finding, writer sent back, traced
#   trace-gap-drop  the writer never specifies it: a scope decision (the person drops it)
#   trace-constraint a requirement marked a constraint with its reason passes and is counted
# and, through scripts/run-trial.sh with SDC_DECIDER=haiku on the fixture adapter (no LLM):
#   decider-scope-answer the decider answers two open scope-changing questions one by one
#                   (default, free answer) and takes scope.answer; the run closes
#   decider-scope-final as above under SDC_DECIDER_ESCALATE=final: an escalated question takes
#                   its default, escalation_suppressed recorded
#   decider-finding a scope-changing spec finding decided by the decider (option 2); amended
#   decider-finding-final the decider escalates the finding under final: its default taken
#   decider-finding-refused the decider's amendment contradicts decision q3: refused, reverted,
#                   a person decides (exit 3)
#   run-stop        a waiting run is stopped (--stop), a resume refused, --reopen goes on; L7
#   run-stop-decider L7 on a decider's run that waits on a person: reported FAILED
#   autopilot       --autopilot: closed with 0 person principals and L7 0 (A5, escalate final)
#   autopilot-person --autopilot refuses SDC_DECIDER=person (exit 64)
#   autopilot-stop  a stop no holder can take ends an autopilot run (exit 7, rule:autopilot,
#                   cause decision_not_takeable, no message naming a person, L7 0)
#   decider-cite-address, decider-cite-retry, decider-invalid-approval, decider-invalid-scope,
#   decider-reject  autopilot decider replies: an entry and lines cited; an invalid reply asked
#                   again; three invalid: approved_on_checks, the default; a reject: exit 6,
#                   rejected_without_rework_path
#   decider-escalate-nobest  under autopilot an escalate without best is refused and asked again
#   principals      the scripted person records person:stand-in (run labelled stand_in); a
#                   decision with no principal is not applied
#   models          three dry runs: binding models, SDC_LLM_MODEL, SDC_LLM_MODEL_<CLASS>; each call
#                   and decider record names its model, run-settings.jsonl the model per class
#   rework-not-called, rework-pending-rejection, decider-later-obligation, replay-sibling,
#   writer-not-called  runs/trial-3-haiku-4 and runs/trial-3-mixed-3 (see each cycle)
#   cite-dark-spot, cite-fault-sent-back  runs/trial-3-mixed-4 (see each cycle)
#   autopilot-close, autopilot-close-a3   runs/trial-3-haiku-5: the close in autopilot (D140)
#   keep-going-three, keep-going-off, keep-going-repeat  --keep-going (D142)
#   reopen-refused-decision  --reopen puts a refused decision back
#   unstick-end, unstick-rerun, unstick-invalid, unstick-limit, unstick-off  run.unstick (D147)
# Prints each run's exit code and its KPIs L1-L6, then `cycles: N expected, M as expected, K failed:
# <names>`; exits 1 when a cycle missed its expectation.
#
#   scripts/run-finding-cycles.sh <scratch dir>
set -uo pipefail
sdc=$(cd "$(dirname "$0")/.." && pwd)
s=${1:?scratch dir}
fx=$sdc/driver/examples/sdc/fixtures
mkdir -p "$s/runs"
kpis=${SDC_FINDING_KPIS:-$sdc/driver/target/debug/sdc-finding-kpis}
[[ -x $kpis ]] || kpis=$HOME/beyond10x/sdc/driver/target/debug/sdc-finding-kpis
# The cycles and their expectations. A cycle with a check line is as expected when it prints
# "<name>: checks pass" and no "<name>: FAILED"; one without (exit_only) when its exit line shows
# the exit given here. A cycle that prints nothing failed. Exit 1 when any cycle failed.
expected=(stub-cycle finding-agent finding-person-stop finding-person scope-answer finding-program
  scope-changed intake-decision offline-sync offline-decision plan-in-project story-decision story-spent
  story-late-person spec-duplicate spec-own-default trace-gap trace-gap-drop trace-constraint decider-scope-answer decider-scope-final decider-finding decider-finding-final
  decider-finding-refused run-stop run-stop-decider autopilot autopilot-person autopilot-stop
  decider-cite-address decider-cite-retry decider-invalid-approval decider-invalid-scope
  decider-escalate-nobest decider-release-a3 principals models)
# The rejection cycles follow what the protocols give: decider-reject (no rework path) while
# context.approve cannot record `rejected`, else decider-rework-context; decider-rework and
# decider-rework-spent where merge.approve exists.
ctx_rework=no
jq -e '."context.approve".results | any(.[]; . == "rejected")' "$sdc/driver/examples/sdc/sdc-direction.bindings.json" >/dev/null 2>&1 && ctx_rework=yes
if [[ $ctx_rework == no ]]; then expected+=(decider-reject); else expected+=(decider-rework-context decider-rework-model); fi
yq -e '.actions."merge.approve"' "$sdc/protocol/sdc-story.protocol.yaml" >/dev/null 2>&1 &&
  expected+=(decider-rework decider-rework-spent rework-not-called rework-pending-rejection keep-going-three keep-going-off unstick-end)
expected+=(decider-later-obligation replay-sibling writer-not-called)
expected+=(cite-dark-spot cite-fault-sent-back autopilot-close autopilot-close-a3 keep-going-repeat reopen-refused-decision)
expected+=(unstick-rerun unstick-invalid unstick-limit unstick-off)
declare -A exit_only=([stub-cycle]=0 [finding-agent]=0 [finding-person-stop]=3 [finding-person]=0
  [scope-answer]=0 [finding-program]=0)
summary() { # <log>
  local log=$1 name ok=0 failed=()
  for name in "${expected[@]}"; do
    if grep -q "^$name: FAILED" "$log"; then failed+=("$name")
    elif grep -q "^$name: checks pass" "$log"; then ok=$((ok + 1))
    elif [[ -n ${exit_only[$name]:-} ]] && grep -q "^$name: exit ${exit_only[$name]} " "$log"; then ok=$((ok + 1))
    else failed+=("$name")
    fi
  done
  echo "cycles: ${#expected[@]} expected, $ok as expected, ${#failed[@]} failed: ${failed[*]:-none}"
  (( ${#failed[@]} == 0 ))
}
{
cycle() {
  local name=$1; shift
  env RUNS="$s/runs" "$@" "$sdc/scripts/run-full-cycle.sh" "$name" "$s/store-$name" > "$s/$name.txt" 2>&1
  local code=$?
  echo "$name: exit $code ($(tail -n 1 "$s/$name.txt"))"
  return $code
}
cycle stub-cycle
"$kpis" "$s/runs/stub-cycle" | sed 's/^/  /'
cycle finding-agent SDC_STUB_FINDINGS="$fx/finding-spec-agent.yaml"
"$kpis" "$s/runs/finding-agent" | sed 's/^/  /'
cycle finding-person-stop SDC_STUB_FINDINGS="$fx/finding-spec-person.yaml"
cycle finding-person SDC_STUB_FINDINGS="$fx/finding-spec-person.yaml" FINDING_DECISION=option:2
"$kpis" "$s/runs/finding-person" | sed 's/^/  /'
cycle scope-answer SDC_STUB_FIXTURES="$fx/scope-question"
"$kpis" "$s/runs/scope-answer" | sed 's/^/  /'
cycle finding-program SDC_STUB_FIXTURES="$fx/storage-conflict"
"$kpis" "$s/runs/finding-program" | sed 's/^/  /'
# scope-changed: runs/trial-2-train's stop (docs/trial-2.md, "Run notes"). questions.answer goes
# through llm-node.sh's apply step (fixture adapter, no LLM); at the scope.answer stop the person
# changes q2's answer in questions.yaml, then takes scope.answer once. Passes when the cycle ends
# operating, q2 keeps the person's answer (`answered_by: person:stand-in`, the scripted person) and the requirements were written
# after scope.answer.
answer="the billing clerk or the customer, before it is paid"
cycle scope-changed SDC_STUB_FIXTURES="$fx/scope-question" SDC_LLM_NODES=questions.answer \
  SDC_LLM_ADAPTER="$sdc/driver/examples/sdc/tools/llm-adapter-fixture.sh" \
  SCOPE_ANSWERS="$(jq -cn --arg a "$answer" '{q2: $a}')"
code=$?
ev=$s/runs/scope-changed/events/IDEA-1.intake.jsonl
q=$s/store-scope-changed/IDEA-1/work/questions.yaml
if (( code == 0 )) &&
   yq -o=json "$q" | jq -e --arg a "$answer" '.questions[] | select(.id == "q2")
     | .status == "changed" and .answer == $a and .answered_by == "person:stand-in"' >/dev/null &&
   jq -se '([.[] | select(.event == "action_finished" and .action == "scope.answer") | .seq] | max) as $s
     | any(.[]; .event == "action_finished" and .action == "requirements.normalise" and .exit == "0" and .seq > $s)' "$ev" >/dev/null; then
  echo "scope-changed: checks pass (q2 the person's answer, answered_by person:stand-in; requirements written after scope.answer)"
else
  echo "scope-changed: FAILED (cycle exit $code, or q2 / the requirements order wrong)"
fi
"$kpis" "$s/runs/scope-changed" | sed 's/^/  /'
# intake-decision: runs/trial-2-train's second stop (docs/trial-2.md, "Run notes"). As
# scope-changed, plus: at the first stop the person edits q2, one questions.check runs without yq
# on PATH at the new interview revision (the check runs once per revision, so only after the
# edit), the person resumes before taking scope.answer, requirements.normalise raises an agent
# finding and a scope-changing one, and the person picks option 3 of the latter (not the default).
# Passes when the cycle ends operating, the check without yq exited 127 and recorded nothing, a
# questions.check after it recorded (a 127 run leaves the check admissible), questions.write ran once,
# and the interview keeps q2 and the decision (option 3), both person:stand-in.
decision="the customer asks, the billing clerk cancels"
cycle intake-decision SDC_STUB_FIXTURES="$fx/scope-question" SDC_LLM_NODES=questions.answer \
  SDC_LLM_ADAPTER="$sdc/driver/examples/sdc/tools/llm-adapter-fixture.sh" \
  SCOPE_ANSWERS="$(jq -cn --arg a "$answer" '{q2: $a}')" SCOPE_RESUME_FIRST=1 NO_YQ_STEP=questions.check \
  SDC_STUB_FINDINGS="$fx/finding-intake-person.yaml" FINDING_DECISION=option:3
code=$?
ev=$s/runs/intake-decision/events/IDEA-1.intake.jsonl
q=$s/store-intake-decision/IDEA-1/work/questions.yaml
if (( code == 0 )) &&
   yq -o=json "$q" | jq -e --arg a "$answer" --arg d "$decision" '[.questions[]
     | select((.id == "q2" and .answer == $a and .answered_by == "person:stand-in")
       or (.answer == $d and .answered_by == "person:stand-in" and .status == "changed"))] | length == 2' >/dev/null &&
   jq -se '[.[] | select(.event == "action_finished")] as $f
     | ([$f | to_entries[] | select(.value.action == "questions.check" and .value.exit == "127") | .key]) as $k
     | ([$f[] | select(.action == "questions.write")] | length) == 1
       and ($k | length) == 1
       and ([$f[] | select(.exit == "127") | .evidence[]] | length) == 0
       and ([$f[$k[0] + 1:][] | select(.action == "questions.check" and .exit != "127" and (.evidence | length) > 0)] | length) >= 1' "$ev" >/dev/null; then
  echo "intake-decision: checks pass (questions.check without yq exits 127 and records nothing, the check is admitted again and records; questions.write once; q2 and the decision (option 3) person:stand-in)"
else
  echo "intake-decision: FAILED (cycle exit $code, or no single 127 check, a record without yq, no recording check after it, a rewritten interview, or a lost answer)"
fi
"$kpis" "$s/runs/intake-decision" | sed 's/^/  /'
# offline-sync: the context hosts a server and says the client keeps a local copy and syncs
# (`offline` field), a requirement drafts offline: both hold, rule offline-hosted must not fire.
cycle offline-sync SDC_STUB_FIXTURES="$fx/offline-sync"
code=$?
f=$s/store-offline-sync/IDEA-1/work/findings.yaml
if (( code == 0 )) && ! grep -q 'offline-hosted' "$f" 2>/dev/null &&
   [[ $(yq -r '.deployment.value' "$s/store-offline-sync/IDEA-1/work/context.yaml") == "a hosted web server reachable over the internet" ]]; then
  echo "offline-sync: checks pass (no offline-hosted finding, deployment kept)"
else
  echo "offline-sync: FAILED (cycle exit $code, or offline-hosted fired, or the deployment was amended)"
fi
# offline-decision: runs/trial-2-train's rejected context. r3 drafts offline and cites q3, an
# agent's decision "a hosted web server"; the context names that server with no offline path. The
# rule fires; the agent's amendment (deployment "on the user's own machine") contradicts q3, so
# it is reverted and the finding waits for a person with both sides shown: exit 3 at direction.
cycle offline-decision SDC_STUB_FIXTURES="$fx/offline-decision"
code=$?
f=$s/store-offline-decision/IDEA-1/work/findings.yaml
if (( code == 3 )) && yq -o=json "$f" | jq -e '.findings[] | select(.id == "p-offline-hosted-context.deployment")
     | .status == "needs_person" and (.basis | test("decision q3") and test("both sides"))' >/dev/null &&
   [[ $(yq -r '.deployment.value' "$s/store-offline-decision/IDEA-1/work/context.yaml") == "a hosted web server reachable over the internet" ]]; then
  echo "offline-decision: checks pass (amendment reverted, needs_person, basis names q3 and both sides)"
else
  echo "offline-decision: FAILED (cycle exit $code, or the amendment stood)"
fi
# plan-in-project: runs/trial-2-train's plan stop. The case directory lies inside a project whose
# .engineering/project.yaml names the profile development.standard (this repository's own, as for
# runs/ here), where a draft may only move to proposed. plan.decompose goes through llm-node.sh's
# apply step (fixture adapter, fixtures/plan-stories.json); the stub roadmap and the apply step
# move the epic and the story to active in the case's own aep store. Passes when the cycle ends
# operating and plan.decompose ran once.
p=$s/in-project; rm -rf "$p"; mkdir -p "$p/.engineering" "$p/runs"
cp "$sdc/.engineering/project.yaml" "$p/.engineering/"
env RUNS="$p/runs" SDC_LLM_NODES=plan.decompose \
  SDC_LLM_ADAPTER="$sdc/driver/examples/sdc/tools/llm-adapter-fixture.sh" \
  SDC_FIXTURE_STUB='{"plan.decompose":["plan-stories.json","stories.json"]}' \
  "$sdc/scripts/run-full-cycle.sh" plan-in-project "$p/store" > "$s/plan-in-project.txt" 2>&1
code=$?
echo "plan-in-project: exit $code ($(tail -n 1 "$s/plan-in-project.txt"))"
if (( code == 0 )) &&
   jq -se '[.[] | select(.event == "action_finished"
     and .action == "plan.decompose")] | length == 1 and all(.exit == "0")' \
     "$p/store/IDEA-1/children/IDEA-1.billing/children/IDEA-1.billing.plan/events.jsonl" >/dev/null; then
  echo "plan-in-project: checks pass (decompose applied once inside a development.standard project)"
else
  echo "plan-in-project: FAILED (cycle exit $code, or plan.decompose refused or repeated)"
fi
# story-decision: runs/trial-2-train's story stop (R7). change.implement's stub raises a
# scope-changing finding and an agent one on its first run, another agent one on its second
# applied run (fixtures/finding-story-rounds.yaml, `rounds`); the person decides the first with
# option 2. Its decision is an arrival: change.implement gets a fresh max_runs 3. Passes when the
# cycle ends operating with change.implement run 4 times in the story case.
story=IDEA-1/children/IDEA-1.billing/children/IDEA-1.billing.invoices/events.jsonl
cycle story-decision SDC_STUB_FINDINGS="$fx/finding-story-rounds.yaml" FINDING_DECISION=option:2
code=$?
if (( code == 0 )) && jq -se '[.[] | select(.event == "action_finished" and .action == "change.implement")] | length == 4' \
     "$s/store-story-decision/$story" >/dev/null; then
  echo "story-decision: checks pass (a fresh allowance after the person's decision; change.implement 4 runs)"
else
  echo "story-decision: FAILED (cycle exit $code, or change.implement did not run 4 times)"
fi
# story-spent: change.implement raises a new agent finding on each of 4 runs: its allowance of 3
# is spent with no person involved. Passes when the run ends `writer_spent` naming
# change.implement (not `stuck`, not `awaiting_person` on amend.upstream).
cycle story-spent SDC_STUB_FINDINGS="$fx/finding-story-spent.yaml"
code=$?
if (( code == 1 )) && jq -se '(map(select(.event | test("^(stuck|awaiting_person|writer_spent)$"))) | last)
     | .event == "writer_spent" and .spent[0].action == "change.implement" and .spent[0].runs == 3' \
     "$s/store-story-spent/$story" >/dev/null; then
  echo "story-spent: checks pass (writer_spent: change.implement, 3 runs)"
else
  echo "story-spent: FAILED (cycle exit $code, or the stop is not writer_spent on change.implement)"
fi
# story-late-person: runs/trial-2-train's lists story (R8). change.implement's third run raises a
# scope-changing finding: its allowance is spent while the finding waits for a person. The run
# stops awaiting_person with amend.upstream's handoff (handoff_when: a finding waits), not
# writer_spent; the person's decision gives change.implement a fresh allowance. Passes when the
# cycle ends operating with one person stop and no writer_spent.
cycle story-late-person SDC_STUB_FINDINGS="$fx/finding-story-late-person.yaml" FINDING_DECISION=option:2
code=$?
if (( code == 0 )) && jq -se '([.[] | select(.event == "awaiting_person")] | length) == 1
     and ([.[] | select(.event == "writer_spent")] | length) == 0' "$s/store-story-late-person/$story" >/dev/null; then
  echo "story-late-person: checks pass (person stop on the late finding, then a fresh allowance)"
else
  echo "story-late-person: FAILED (cycle exit $code, or writer_spent instead of the person stop)"
fi
# spec-duplicate: runs/trial-3-haiku-2 (f10-f13 = f6-f9, f15 = f14). spec.author raises a finding,
# then raises it again with no key (fixtures/finding-spec-duplicate.yaml). Passes when the cycle ends
# operating with one finding of that text and spec.author run twice (the repeat added nothing).
spec=IDEA-1/children/IDEA-1.billing/children/IDEA-1.billing.spec/events.jsonl
cycle spec-duplicate SDC_STUB_FINDINGS="$fx/finding-spec-duplicate.yaml"
code=$?
if (( code == 0 )) && [[ $(yq '[.findings[] | select(.says | test("cancelled invoice"))] | length' "$s/store-spec-duplicate/IDEA-1/work/findings.yaml") == 1 ]] &&
   jq -se '[.[] | select(.event == "action_finished" and .action == "spec.author")] | length == 2' "$s/store-spec-duplicate/$spec" >/dev/null; then
  echo "spec-duplicate: checks pass (the repeated finding not added; spec.author 2 runs)"
else
  echo "spec-duplicate: FAILED (cycle exit $code, or the repeated finding was added again)"
fi
# spec-own-default: runs/trial-3-haiku-2's stop. spec.author goes through llm-node.sh's apply path
# (fixture adapter) and raises a new agent finding on every LLM call (fixtures/finding-spec-rounds.yaml).
# Passes when the cycle ends operating, no writer_spent, spec.author run twice with one LLM call:
# the amendment only records the default its tree follows, so it is not re-derived by the LLM.
cycle spec-own-default SDC_STUB_FINDINGS="$fx/finding-spec-rounds.yaml" SDC_LLM_NODES=spec.author \
  SDC_LLM_ADAPTER="$sdc/driver/examples/sdc/tools/llm-adapter-fixture.sh"
code=$?
if (( code == 0 )) && jq -se '([.[] | select(.event == "writer_spent")] | length) == 0
     and ([.[] | select(.event == "action_finished" and .action == "spec.author")] | length) == 2' "$s/store-spec-own-default/$spec" >/dev/null &&
   [[ $(jq -s '[.[] | select(.action == "spec.author")] | length' "$s/store-spec-own-default/IDEA-1/work/.sdc/llm/calls.jsonl") == 1 ]]; then
  echo "spec-own-default: checks pass (the writer's own default re-derived without an LLM call; spec.author 2 runs, 1 call)"
else
  echo "spec-own-default: FAILED (cycle exit $code, or writer_spent, or spec.author re-derived by the LLM)"
fi
# The requirement trace (runs/trial-3-haiku-3: r3 traced to a domain with one actor and no command).
# tracelog <cycle>: every spec.trace log of the cycle's spec case, in order.
tracelog() { local d=$s/store-$1/${spec%/events.jsonl}
  jq -r 'select(.event == "action_finished" and .action == "spec.trace") | .log' "$d/events.jsonl" | while read -r b; do cat "$d/blobs/$b"; done; }
# trace-gap: fixtures/trace-gap. The first spec traces r3 to billing.audit (an actor, no command):
# the trace finding p-trace-r3, the writer sent back (the agent's decision names the gap), the
# second spec traces r3 to two views. Passes when the cycle ends operating with that finding.
cycle trace-gap SDC_STUB_FIXTURES="$fx/trace-gap"
code=$?
if (( code == 0 )) && yq -o=json "$s/store-trace-gap/IDEA-1/work/findings.yaml" | jq -e '.findings[] | select(.id == "p-trace-r3")
     | .status == "resolved" and .decided_by == "agent" and .says == "r3: traces to no behaviour (cites billing.audit, which declares no command)"' >/dev/null &&
   jq -se '[.[] | select(.event == "action_finished" and .action == "spec.author")] | length == 2' "$s/store-trace-gap/$spec" >/dev/null &&
   tracelog trace-gap | tail -n 1 | grep -q '3 traced to behaviour, 0 constraint, 0 dropped, 0 untraced'; then
  echo "trace-gap: checks pass (r3 traced to no behaviour: finding, writer sent back, then traced)"
else
  echo "trace-gap: FAILED (cycle exit $code, or no p-trace-r3, or the writer was not sent back)"
fi
# trace-gap-drop: the writer never specifies r3 (fixtures/trace-gap-drop): after it was sent back
# once the gap is a scope decision, not a default; the scripted person drops r3. Passes when the
# cycle ends operating with p-trace-r3 decided by the person and r3 `behaviour: dropped`.
cycle trace-gap-drop SDC_STUB_FIXTURES="$fx/trace-gap-drop" FINDING_DECISION=option:3
code=$?
if (( code == 0 )) && yq -o=json "$s/store-trace-gap-drop/IDEA-1/work/findings.yaml" | jq -e '.findings[] | select(.id == "p-trace-r3")
     | .status == "resolved" and .decided_by == "person:stand-in" and .decision == "drop r3 from this epic'"'"'s scope" and .attempts == 1' >/dev/null &&
   yq -e '.requirements[] | select(.id == "r3") | .behaviour == "dropped"' "$s/store-trace-gap-drop/IDEA-1/work/requirements.yaml" >/dev/null &&
   tracelog trace-gap-drop | tail -n 1 | grep -q '1 dropped (r3), 0 untraced'; then
  echo "trace-gap-drop: checks pass (the gap went to a person after one send-back; r3 dropped by the person)"
else
  echo "trace-gap-drop: FAILED (cycle exit $code, or the gap was waved through, or r3 not dropped)"
fi
# trace-constraint: r3 is a retention constraint with its reason (fixtures/trace-constraint): it
# passes the trace untraced and is counted. Passes when the cycle ends operating with no trace finding.
cycle trace-constraint SDC_STUB_FIXTURES="$fx/trace-constraint"
code=$?
if (( code == 0 )) && ! grep -q 'p-trace' "$s/store-trace-constraint/IDEA-1/work/findings.yaml" 2>/dev/null &&
   tracelog trace-constraint | grep -q '2 traced to behaviour, 1 constraint (r3), 0 dropped, 0 untraced'; then
  echo "trace-constraint: checks pass (r3 a constraint with its reason: passes, counted)"
else
  echo "trace-constraint: FAILED (cycle exit $code, or the constraint was not counted)"
fi

# --- decider cycles: scripts/run-trial.sh with SDC_DECIDER=haiku, every node on its stub and the
# decider on tools/llm-adapter-fixture.sh (scripted replies, SDC_DECIDER_REPLIES), no LLM ---------
fixture=$sdc/driver/examples/sdc/tools/llm-adapter-fixture.sh
targs=()
# run.unstick is off unless a cycle turns it on (SDC_UNSTICK=on): the stops these cycles assert
# are today's.
trial() { # <name> [env ...]: a fresh run-trial.sh run into <scratch>/runs/<name>, with the arguments in targs
  local name=$1; shift
  rm -rf "$s/runs/$name"
  env SDC_RUNS_DIR="$s/runs" SDC_WATCH=0 SDC_LLM_ADAPTER=stub SDC_DECIDER=haiku SDC_DECIDER_ADAPTER="$fixture" SDC_UNSTICK=off "$@" \
    "$sdc/scripts/run-trial.sh" "$name" "$(yq -r .text "$fx/idea.yaml")" "${targs[@]}" > "$s/$name.txt" 2>&1
  local code=$?
  echo "$name: exit $code ($(tail -n 1 "$s/$name.txt" | cut -c1-160))"
  return $code
}
# decider-scope-answer: runs/trial-2b-fresh's first stop. Two scope-changing questions are open at
# scope.answer (fixtures/scope-two); asked to approve scope.answer as a whole, the decider escalates
# (the recorded reply: it read questions.settled false as a failed check). Each question goes to
# the decider instead: q2 by its default, q3 with a free answer; the answers are written as a
# person's are, scope.answer is taken as decider:haiku and intake closes. Passes when the run ends
# 0, q2 and q3 carry decider:haiku (accepted, changed), decisions.jsonl has both as class
# scope_changing_answer, and the decider was shown questions.settled as settled by its decision.
free="the billing clerk who issued it, as a copy"
trial decider-scope-answer SDC_STUB_FIXTURES="$fx/scope-two" SDC_DECIDER_REPLIES="$(jq -cn --arg f "$free" '{
  "scope.answer": {decision: "escalate", answer: null, best: null, cites: ["q2", "q3", "questions.settled"],
    reasons: ["Questions q2 and q3 are scope-changing and still open.", "The claim questions.settled is false at the current revision."]},
  q3: {decision: "approve", answer: $f, best: null, cites: ["d5", "d2"], reasons: ["The clerk who issued an invoice needs to see what the customer was sent."]}}')"
code=$?
r=$s/runs/decider-scope-answer
q=$r/store/IDEA-1/work/questions.yaml
if (( code == 0 )) &&
   yq -o=json "$q" | jq -e --arg f "$free" '[.questions[] | select(
     (.id == "q2" and .status == "accepted" and .answer == .default and .answered_by == "decider:haiku")
     or (.id == "q3" and .status == "changed" and .answer == $f and .answered_by == "decider:haiku"))] | length == 2' >/dev/null &&
   jq -se '[.[] | select(.class == "scope_changing_answer" and .decision == "approve")] | map(.question) == ["q2", "q3"]' "$r/decisions.jsonl" >/dev/null &&
   jq -e '.claims["questions.settled"].settled_by_this_decision == true' "$(ls -d "$r"/decider/*-q2 | head -n 1)/cwd/in/checks.json" >/dev/null; then
  echo "decider-scope-answer: checks pass (q2 default, q3 free answer, both decider:haiku; class scope_changing_answer; questions.settled marked)"
else
  echo "decider-scope-answer: FAILED (run exit $code, or the answers, their records or the claim marks are wrong)"
fi
# decider-scope-final: as decider-scope-answer under SDC_DECIDER_ESCALATE=final, the decider
# escalating q3: no person is asked, q3 takes its default and escalation_suppressed is recorded.
trial decider-scope-final SDC_STUB_FIXTURES="$fx/scope-two" SDC_DECIDER_ESCALATE=final SDC_DECIDER_REPLIES="$(jq -cn '{
  "scope.answer": {decision: "escalate", answer: null, best: null, cites: [], reasons: ["The claim questions.settled is false."]},
  q3: {decision: "escalate", answer: null, best: null, cites: ["d5"], reasons: ["Who else is emailed adds a kind of recipient the idea does not name."]}}')"
code=$?
r=$s/runs/decider-scope-final
q=$r/store/IDEA-1/work/questions.yaml
if (( code == 0 )) &&
   yq -o=json "$q" | jq -e '.questions[] | select(.id == "q3") | .status == "accepted" and .answer == .default and .answered_by == "decider:haiku"' >/dev/null &&
   jq -se '[.[] | select(.question == "q3") | .decision] == ["escalation_suppressed", "approve"]' "$r/decisions.jsonl" >/dev/null; then
  echo "decider-scope-final: checks pass (q3 escalated, suppressed, its default taken)"
else
  echo "decider-scope-final: FAILED (run exit $code, or q3 not at its default, or no escalation_suppressed)"
fi
# decider-finding: trial 2's f1/f9/f16 stops ("nothing for decider:haiku at amend.upstream"). The
# spec stage's scope-changing finding (fixtures/finding-spec-person.yaml) is put to the decider as
# a decision request; it picks option 2. Passes when the run ends 0, the finding is resolved with
# that option and `decided_by: decider:haiku`, the amendment's question is answered by
# decider:haiku, and decisions.jsonl records it with class finding.
opt2="the customer and the billing clerk"
trial decider-finding SDC_STUB_FINDINGS="$fx/finding-spec-person.yaml" SDC_DECIDER_REPLIES="$(jq -cn --arg o "$opt2" '{
  f1: {decision: "approve", answer: $o, best: null, cites: ["r2"], reasons: ["The clerk who issues an invoice is the one asked about it, so the clerk gets the email too."]}}')"
code=$?
r=$s/runs/decider-finding
f=$r/store/IDEA-1/work/findings.yaml
if (( code == 0 )) &&
   yq -o=json "$f" | jq -e --arg o "$opt2" '.findings[] | select(.id == "f1") | .status == "resolved" and .decision == $o and .decided_by == "decider:haiku"' >/dev/null &&
   yq -o=json "$(dirname "$f")/questions.yaml" | jq -e --arg o "$opt2" 'any(.questions[]; .item == "f1" and .answer == $o and .answered_by == "decider:haiku")' >/dev/null &&
   jq -se '[.[] | select(.class == "finding")] | length == 1 and .[0].finding == "f1" and .[0].decision == "approve"' "$r/decisions.jsonl" >/dev/null; then
  echo "decider-finding: checks pass (f1 option 2 by decider:haiku, amended, class finding)"
else
  echo "decider-finding: FAILED (run exit $code, or f1 not decided by the decider, or not recorded)"
fi
# decider-finding-final: as decider-finding under SDC_DECIDER_ESCALATE=final, the decider
# escalating f1 with no best: the default is taken, escalation_suppressed recorded.
trial decider-finding-final SDC_STUB_FINDINGS="$fx/finding-spec-person.yaml" SDC_DECIDER_ESCALATE=final SDC_DECIDER_REPLIES="$(jq -cn '{
  f1: {decision: "escalate", answer: null, best: null, cites: ["r2"], reasons: ["Who else is emailed is the requester'"'"'s call."]}}')"
code=$?
r=$s/runs/decider-finding-final
f=$r/store/IDEA-1/work/findings.yaml
if (( code == 0 )) &&
   yq -o=json "$f" | jq -e '.findings[] | select(.id == "f1") | .status == "resolved" and .decision == .default and .decided_by == "decider:haiku"' >/dev/null &&
   jq -se '[.[] | select(.finding == "f1") | .decision] == ["escalation_suppressed", "approve"]' "$r/decisions.jsonl" >/dev/null; then
  echo "decider-finding-final: checks pass (f1 escalated, suppressed, its default taken)"
else
  echo "decider-finding-final: FAILED (run exit $code, or f1 not at its default, or no escalation_suppressed)"
fi
# decider-finding-refused: fixtures/offline-decision. The agent's amendment contradicted decision
# q3 and went to a person; the decider then takes the same option (its default), which the same
# program check refuses: reverted, the finding waits for a person (exit 3) with the refused
# decision recorded, and the decider is not asked again.
trial decider-finding-refused SDC_STUB_FIXTURES="$fx/offline-decision"
code=$?
r=$s/runs/decider-finding-refused
f=$r/store/IDEA-1/work/findings.yaml
if (( code == 3 )) &&
   yq -o=json "$f" | jq -e '.findings[] | select(.id == "p-offline-hosted-context.deployment")
     | .status == "needs_person" and (.decision // "") == "" and .refused_decision.decided_by == "decider:haiku"
       and (.refused_decision.why | test("q3"))' >/dev/null &&
   jq -se '[.[] | select(.class == "finding")] | length == 1' "$r/decisions.jsonl" >/dev/null &&
   [[ $(yq -r '.deployment.value' "$(dirname "$f")/context.yaml") == "a hosted web server reachable over the internet" ]]; then
  echo "decider-finding-refused: checks pass (the decider's amendment refused and reverted, a person decides)"
else
  echo "decider-finding-refused: FAILED (run exit $code, or the decider's amendment stood, or no refused_decision)"
fi
# run-stop: a run left waiting is stopped with a record (scripts/run-trial.sh <run> --stop). A run
# with the person as decider stops at context.approve (exit 3); 2 s later it is stopped:
# run-stopped.json (sdc.run-stopped/1, verdict and case at the stop), a run.log line, the event
# logs untouched; a resume is refused (exit 8) until --reopen, which renames the record and goes
# on (exit 3 again at the same stop). L7 counts the wait up to the stop's instant; on the
# decider's stopped run (decider-finding-refused) it is a failure.
stopped_ok() { # <run dir> <by> <reason>
  jq -e --arg by "$2" --arg why "$3" '.format == "sdc.run-stopped/1" and (.at | test("^[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}Z$"))
    and .by == $by and .reason == $why and (.verdict_at_stop | type) == "string" and (.case | type) == "string"' "$1/run-stopped.json" >/dev/null
}
evsum() { find "$1/store" -name events.jsonl | sort | xargs cat | sha256sum; }
trial run-stop SDC_DECIDER=person
code=$?
r=$s/runs/run-stop
before=$(evsum "$r")
sleep 2
why="fixture: nobody takes context.approve in this cycle"
SDC_RUNS_DIR="$s/runs" SDC_WATCH=0 SDC_PRINCIPAL=agent:finding-cycles "$sdc/scripts/run-trial.sh" run-stop --stop "$why" > "$s/run-stop-stop.txt" 2>&1
stop=$?
SDC_RUNS_DIR="$s/runs" SDC_WATCH=0 SDC_LLM_ADAPTER=stub SDC_DECIDER=person "$sdc/scripts/run-trial.sh" run-stop > "$s/run-stop-resume.txt" 2>&1
resume=$?
after=$(evsum "$r")
l7=$("$kpis" "$r" --json | jq -c .L7_waiting_on_person_seconds)
echo "run-stop: exit $code, --stop exit $stop, resume exit $resume, L7 $l7"
if (( code == 3 && stop == 0 && resume == 8 )) && stopped_ok "$r" agent:finding-cycles "$why" &&
   jq -e '.verdict_at_stop == "awaiting_authority" and .case == "IDEA-1.direction"' "$r/run-stopped.json" >/dev/null &&
   [[ $before == "$after" ]] && grep -q "^# stopped at .* by agent:finding-cycles: $why" "$r/run.log" &&
   jq -e '.value >= 2 and (.stops | last | .until == "stopped") and .failure == null' <<< "$l7" >/dev/null; then
  SDC_RUNS_DIR="$s/runs" SDC_WATCH=0 SDC_LLM_ADAPTER=stub SDC_DECIDER=person "$sdc/scripts/run-trial.sh" run-stop --reopen > "$s/run-stop-reopen.txt" 2>&1
  reopen=$?
  if (( reopen == 3 )) && [[ ! -e $r/run-stopped.json ]] && ls "$r"/run-stopped.*.json >/dev/null 2>&1 &&
     grep -q '^# reopened at ' "$r/run.log"; then
    echo "run-stop: checks pass (record written, events untouched, resume refused, L7 to the stop, reopen renames and goes on)"
  else
    echo "run-stop: FAILED (--reopen exit $reopen, or the record was not renamed)"
  fi
else
  echo "run-stop: FAILED (the record, the refusal, the untouched events or L7 are wrong)"
fi
r=$s/runs/decider-finding-refused
SDC_RUNS_DIR="$s/runs" SDC_WATCH=0 SDC_PRINCIPAL=agent:finding-cycles "$sdc/scripts/run-trial.sh" decider-finding-refused --stop "fixture: the finding waits for a person" > /dev/null 2>&1
l7=$("$kpis" "$r" --json | jq -c .L7_waiting_on_person_seconds)
if jq -e '.value >= 1 and .failure == true and .decider == "haiku"' <<< "$l7" >/dev/null &&
   "$kpis" "$r" | grep -q '^L7_waiting_on_person_seconds: FAILED '; then
  echo "run-stop-decider: checks pass (a decider's run waiting on a person: L7 $(jq .value <<< "$l7") s, reported FAILED)"
else
  echo "run-stop-decider: FAILED (L7 $l7)"
fi
# autopilot: the eval's run mode (SDC_AUTONOMY=A5, `--autopilot`): the decider holds every
# decision and is final. On a fixture with two open scope-changing questions (fixtures/scope-two)
# and a scope-changing spec finding (finding-spec-person.yaml), the run closes with no person
# principal anywhere: decisions.jsonl, findings.yaml, questions.yaml and the events' grants and
# approvals; L7 is 0, run-settings.jsonl names A5, escalate final, and the run's planning store
# (work/aep) records its transitions by an agent: actor, no human:. With SDC_DECIDER=person it
# refuses to start (exit 64); a stop that needs a person (fixtures/offline-decision: the decider's
# amendment is refused) ends the run with a stop record by rule:autopilot (exit 7).
nonperson='^(decider:|rule:|agent$|non-interactive$|derived$)'
targs=(--autopilot)
trial autopilot SDC_STUB_FIXTURES="$fx/scope-two" SDC_STUB_FINDINGS="$fx/finding-spec-person.yaml"
code=$?
r=$s/runs/autopilot
w=$r/store/IDEA-1/work
l7=$("$kpis" "$r" --json | jq -c .L7_waiting_on_person_seconds)
people=$( { jq -r '.principal' "$r/decisions.jsonl"
            yq -r '.findings[] | select(.status == "resolved") | .decided_by' "$w/findings.yaml"
            yq -r '.questions[] | select(.status != "open") | .answered_by' "$w/questions.yaml"
            find "$r/store" -name events.jsonl -exec jq -r 'select(.event == "authority_recorded" or (.event == "action_finished" and .principal != null)) | .principal' {} +
          } | grep -Evc "$nonperson")
if (( code == 0 && people == 0 )) && jq -e '.value == 0 and .failure == false' <<< "$l7" >/dev/null &&
   jq -se 'all(.autonomy == "A5" and .decider.escalate == "final" and .decider.kind == "haiku")' "$r/run-settings.jsonl" >/dev/null &&
   yq -e '.findings[] | select(.id == "f1") | .decided_by == "decider:haiku"' "$w/findings.yaml" >/dev/null &&
   [[ -d $w/aep ]] && ! grep -rqs 'human:' "$w/aep" && grep -rqs 'actor: "agent:' "$w/aep"; then
  echo "autopilot: checks pass (closed; 0 person principals; L7 0; A5, escalate final; work/aep transitions by agent:, no human:)"
else
  echo "autopilot: FAILED (run exit $code, $people person principal(s), L7 $l7, the settings do not name A5, or work/aep has a human: actor or no agent: one)"
fi
trial autopilot-person SDC_DECIDER=person
code=$?
(( code == 64 )) && [[ ! -d $s/runs/autopilot-person/store ]] &&
  echo "autopilot-person: checks pass (refused to start with SDC_DECIDER=person)" ||
  echo "autopilot-person: FAILED (exit $code)"
trial autopilot-stop SDC_STUB_FIXTURES="$fx/offline-decision"
code=$?
r=$s/runs/autopilot-stop
if (( code == 7 )) && jq -e '.format == "sdc.run-stopped/1" and .by == "rule:autopilot" and .cause == "decision_not_takeable"
     and (.reason | test("amend.upstream in IDEA-1") and (test("person") | not) and (test("/home/|/tmp/") | not))
     and .verdict_at_stop == "awaiting_person"' "$r/run-stopped.json" >/dev/null &&
   ! sed -n '/^round /,$p' "$s/autopilot-stop.txt" | sed 's/awaiting_person//g' | grep -qi 'person' &&
   "$kpis" "$r" --json | jq -e '.L7_waiting_on_person_seconds | .value == 0 and .failure == false' >/dev/null; then
  echo "autopilot-stop: checks pass (exit 7, rule:autopilot, cause decision_not_takeable; step named, no path, no person; L7 0)"
else
  echo "autopilot-stop: FAILED (exit $code, or the stop record's cause or reason, a message naming a person, or L7)"
fi
# Decider replies under autopilot (fixture adapter, scripted replies by key; a list is taken by
# attempt). cite-address: context.approve cites an entry and lines of context.yaml: accepted.
# cite-retry: the first reply cites an unknown id, the second is valid: accepted after 1 retry.
# invalid-approval: three invalid replies at context.approve, every check true: approved_on_checks.
# invalid-scope: three invalid replies to q2 at scope.answer: its default (invalid_reply_default).
# reject: a valid reject of context.approve: exit 6, rejected_without_rework_path, L7 0.
bad='{"decision": "approve", "answer": null, "reasons": ["fixture: cites what is not there"], "cites": ["context.nope"], "best": null}'
good='{"decision": "approve", "answer": null, "reasons": ["fixture: the context fits"], "cites": ["context@{rev}#platform.value", "context.yaml:1-2", "context.valid"], "best": null}'
attempts() { jq -s --arg a "$2" '[.[] | select(.action == $a)]' "$1/decisions.jsonl"; }
trial decider-cite-address SDC_DECIDER_REPLIES="$(jq -cn --argjson g "$good" '{"context.approve": $g}')"
code=$?
a=$(attempts "$s/runs/decider-cite-address" context.approve)
if (( code == 0 )) && jq -e 'length == 1 and .[0].decision == "approve" and .[0].valid == true
     and ([.[0].citations[].form] == ["artifact_revision", "path_line", "id"])' <<< "$a" >/dev/null; then
  echo "decider-cite-address: checks pass (an entry and lines of context.yaml resolve; approved on the first reply)"
else
  echo "decider-cite-address: FAILED (exit $code: $(jq -c '[.[] | {decision, invalid_reason}]' <<< "$a" 2>/dev/null))"
fi
trial decider-cite-retry SDC_DECIDER_REPLIES="$(jq -cn --argjson b "$bad" --argjson g "$good" '{"context.approve": [$b, $g]}')"
code=$?
r=$s/runs/decider-cite-retry
a=$(attempts "$r" context.approve)
if (( code == 0 )) && jq -e 'length == 2 and .[0].decision == "invalid_reply" and (.[0].invalid_reason | test("context.nope"))
     and .[1].decision == "approve" and .[1].attempts == 2' <<< "$a" >/dev/null &&
   (( $(ls -d "$r"/decider/*-context.approve | wc -l) == 2 )) &&
   grep -q 'context.nope' "$(ls -d "$r"/decider/*-context.approve | tail -n 1)/prompt.md"; then
  echo "decider-cite-retry: checks pass (refused once, asked again with the reason, approved; 2 decider calls)"
else
  echo "decider-cite-retry: FAILED (exit $code: $(jq -c '[.[] | {decision, attempts, invalid_reason}]' <<< "$a" 2>/dev/null))"
fi
trial decider-invalid-approval SDC_DECIDER_REPLIES="$(jq -cn --argjson b "$bad" '{"context.approve": [$b, $b, $b]}')"
code=$?
r=$s/runs/decider-invalid-approval
a=$(attempts "$r" context.approve)
if (( code == 0 )) && jq -e '[.[] | .decision] == ["invalid_reply", "invalid_reply", "invalid_reply", "approve"]
     and .[3].fallback == "approved_on_checks"' <<< "$a" >/dev/null && (( $(ls -d "$r"/decider/*-context.approve | wc -l) == 3 )); then
  echo "decider-invalid-approval: checks pass (3 invalid replies, approved_on_checks, the run goes on and closes)"
else
  echo "decider-invalid-approval: FAILED (exit $code: $(jq -c '[.[] | {decision, fallback}]' <<< "$a" 2>/dev/null))"
fi
badq='{"decision": "approve", "answer": "fixture", "reasons": ["fixture: cites what is not there"], "cites": ["context.idea_excerpt"], "best": null}'
trial decider-invalid-scope SDC_STUB_FIXTURES="$fx/scope-two" SDC_DECIDER_REPLIES="$(jq -cn --argjson b "$badq" '{q2: [$b, $b, $b]}')"
code=$?
r=$s/runs/decider-invalid-scope
a=$(jq -s '[.[] | select(.question == "q2")]' "$r/decisions.jsonl")
if (( code == 0 )) && jq -e '[.[] | .decision] == ["invalid_reply", "invalid_reply", "invalid_reply", "approve"]
     and .[3].fallback == "invalid_reply_default"' <<< "$a" >/dev/null &&
   yq -o=json "$r/store/IDEA-1/work/questions.yaml" | jq -e '.questions[] | select(.id == "q2") | .answer == .default and .answered_by == "decider:haiku"' >/dev/null; then
  echo "decider-invalid-scope: checks pass (3 invalid replies to q2, its default taken as invalid_reply_default)"
else
  echo "decider-invalid-scope: FAILED (exit $code: $(jq -c '[.[] | {decision, fallback}]' <<< "$a" 2>/dev/null))"
fi
# escalate-nobest: under autopilot the decider is final: an escalate without best is refused and asked again.
esc='{"decision": "escalate", "answer": null, "reasons": ["fixture: a person should say"], "cites": ["context.kind"], "best": null}'
trial decider-escalate-nobest SDC_DECIDER_REPLIES="$(jq -cn --argjson e "$esc" --argjson g "$good" '{"context.approve": [$e, $g]}')"
code=$?
a=$(attempts "$s/runs/decider-escalate-nobest" context.approve)
if (( code == 0 )) && jq -e '[.[] | .decision] == ["invalid_reply", "approve"] and (.[0].invalid_reason | test("best"))' <<< "$a" >/dev/null &&
   grep -q '"best": <when decision is escalate' "$(ls -d "$s"/runs/decider-escalate-nobest/decider/*-context.approve | head -n 1)/prompt.md"; then
  echo "decider-escalate-nobest: checks pass (autopilot asks for best; an escalate without it is refused and asked again)"
else
  echo "decider-escalate-nobest: FAILED (exit $code: $(jq -c '[.[] | {decision, invalid_reason}]' <<< "$a" 2>/dev/null))"
fi
# Only while context.approve cannot record a rejection (no writer reworks the context): with one,
# the reject is reworked, which the decider-rework cycles check.
if [[ $ctx_rework == no ]]; then
trial decider-reject SDC_DECIDER_REPLIES='{"context.approve": {"decision": "reject", "answer": null, "reasons": ["fixture: the context is not usable"], "cites": ["context.platform"], "best": null}}'
code=$?
r=$s/runs/decider-reject
if (( code == 6 )) && jq -e '.by == "rule:autopilot" and .cause == "rejected_without_rework_path" and .case == "IDEA-1.direction"
     and (.reason | test("^autopilot: context.approve in IDEA-1.direction: ") and (test("person") | not) and (test("/home/|/tmp/") | not))' "$r/run-stopped.json" >/dev/null &&
   ! sed -n '/^round /,$p' "$s/decider-reject.txt" | sed 's/awaiting_person//g' | grep -qi 'person' &&
   "$kpis" "$r" --json | jq -e '.L7_waiting_on_person_seconds | .value == 0 and .failure == false' >/dev/null; then
  echo "decider-reject: checks pass (exit 6, rejected_without_rework_path, step and case named, no person, L7 0)"
else
  echo "decider-reject: FAILED (exit $code, or the stop record: $(jq -c '{cause, reason}' "$r/run-stopped.json" 2>/dev/null))"
fi
else
# decider-rework-context: context.approve rejected every time: context.default reworks the context
# twice with the rejection (the stub appends it to context.yaml as a comment), context.check runs
# on each new revision, the third rejection stops by rule:autopilot with cause rework_spent.
trial decider-rework-context SDC_DECIDER_REPLIES='{"context.approve": {"decision": "reject", "answer": null, "reasons": ["fixture: the context is not usable"], "cites": ["context.platform"], "best": null}}'
code=$?
r=$s/runs/decider-rework-context
ev=$(find "$r/store" -type d -name 'IDEA-1.direction' | head -n 1)/events.jsonl
if (( code == 6 )) && jq -e '.by == "rule:autopilot" and .cause == "rework_spent" and .case == "IDEA-1.direction"
     and (.reason | test("^autopilot: context.approve in IDEA-1.direction: rework_spent") and test("1: .*2: .*3: ")
       and (test("person") | not) and (test("/home/|/tmp/") | not))' "$r/run-stopped.json" >/dev/null &&
   jq -se '[.[] | select(.event == "action_started" and .action == "context.default" and .retry_of == ["context_approval"])] | length == 2' "$ev" >/dev/null &&
   jq -se '[.[] | select(.event == "action_finished" and .action == "context.approve" and .result == "rejected")] | length == 3' "$ev" >/dev/null; then
  echo "decider-rework-context: checks pass (context.default reworked the context twice with the rejection, the third rejection is rework_spent)"
else
  echo "decider-rework-context: FAILED (exit $code, or the stop record: $(jq -c '{cause, reason}' "$r/run-stopped.json" 2>/dev/null))"
fi
# decider-rework-model: as decider-rework-context with context.default on a model adapter
# (llm-adapter-fixture.sh, which writes the same context every time): the writer gets the rejection
# as in/rejection.json and writes the artifact itself, so the stub's rework marker is in no file
# of the run's work/ and the unchanged context makes no new revision (the run stops, not closed).
trial decider-rework-model SDC_LLM_ADAPTER="$fixture" SDC_LLM_NODES=context.default \
  SDC_DECIDER_REPLIES='{"context.approve": {"decision": "reject", "answer": null, "reasons": ["fixture: the context is not usable"], "cites": ["context.platform"], "best": null}}'
code=$?
r=$s/runs/decider-rework-model
w=$r/store/IDEA-1/work
if (( code == 4 )) && [[ -d $w ]] && ! grep -rqs 'of the rejection of ' "$w" &&
   jq -e '.cause == "writer_spent" and (.reason | test("^autopilot: context.default in IDEA-1.direction: writer_spent \\([0-9]+ runs"))' "$r/run-stopped.json" >/dev/null &&
   jq -rs '[.[] | select(.action == "context.default") | .record] | last' "$r/llm-calls.jsonl" | xargs -I{} jq -e '.format == "sdc.rejection/1" and .approval == "context.approve"' {}/cwd/in/rejection.json >/dev/null; then
  echo "decider-rework-model: checks pass (the model writer got in/rejection.json; no rework marker in work/; writer_spent names context.default)"
else
  echo "decider-rework-model: FAILED (exit $code, or a rework marker in work/, no in/rejection.json for the model writer, or the stop record: $(jq -c '{cause, reason}' "$r/run-stopped.json" 2>/dev/null))"
fi
fi
# Rework of a rejected approval under autopilot (docs/autonomy-levels.md, "A rejected approval in
# autopilot"). decider-rework and decider-rework-spent run where the story protocol has
# merge.approve (merge_approval approved or rejected). decider-rework: merge.approve rejected once,
# then approved: change.implement runs again with the rejection (SDC_FEEDBACK; the stub records it
# in impl/.rework), the approval is asked again with prior_rejections, the run closes.
# decider-rework-spent: rejected three times: stop by rule:autopilot, cause rework_spent, the three
# reasons named. decider-rework-none: release.publish rejected; its approval is the capability
# release.approve with no `rejected` result, so the rejection cannot be recorded: cause
# rejected_without_rework_path, nothing recorded.
if yq -e '.actions."merge.approve"' "$sdc/protocol/sdc-story.protocol.yaml" >/dev/null 2>&1; then
  rej='{"decision": "reject", "answer": null, "reasons": ["fixture: the server binds only to 127.0.0.1, so a partner on another device cannot open it"], "cites": ["r1"], "best": null}'
  app='{"decision": "approve", "answer": null, "reasons": ["fixture: the rework addresses the rejection"], "cites": ["r1"], "best": null}'
  trial decider-rework SDC_DECIDER_REPLIES="$(jq -cn --argjson j "$rej" --argjson a "$app" '{"merge.approve": {"rounds": [$j, $a]}}')"
  code=$?
  r=$s/runs/decider-rework
  st=$(find "$r/store" -type d -name 'IDEA-1.*.invoices' | head -n 1)
  a=$(attempts "$r" merge.approve)
  if (( code == 0 )) && jq -e '[.[] | .decision] == ["reject", "approve"]' <<< "$a" >/dev/null &&
     jq -se '[.[] | select(.event == "action_finished" and .action == "change.implement")] | length == 2' "$st/events.jsonl" >/dev/null &&
     jq -se '[.[] | select(.event == "action_started" and .action == "change.implement")] | last | .retry_of == ["merge_approval"]' "$st/events.jsonl" >/dev/null &&
     grep -q '127.0.0.1' "$st/work/impl/.rework" &&
     jq -e '.prior_rejections | length == 1' "$(ls -d "$r"/decider/*-merge.approve | tail -n 1)/cwd/in/decision.json" >/dev/null &&
     [[ $(jq -s '[.[] | select(.kind == "merge_approval" and .result == "rejected")] | length' "$st"/evidence/*.json) == 1 ]]; then
    echo "decider-rework: checks pass (rejected once, change.implement ran again with the rejection, approved, closed)"
  else
    echo "decider-rework: FAILED (exit $code: $(jq -c '[.[] | .decision]' <<< "$a" 2>/dev/null))"
  fi
  trial decider-rework-spent SDC_DECIDER_REPLIES="$(jq -cn --argjson j "$rej" '{"merge.approve": $j}')"
  code=$?
  r=$s/runs/decider-rework-spent
  if (( code == 6 )) && jq -e '.by == "rule:autopilot" and .cause == "rework_spent"
       and (.reason | test("^autopilot: merge.approve in IDEA-1[.][a-z]+[.][a-z]+: rework_spent") and test("1: .*2: .*3: ")
         and (test("person") | not) and (test("/home/|/tmp/") | not))' "$r/run-stopped.json" >/dev/null &&
     ! sed -n '/^round /,$p' "$s/decider-rework-spent.txt" | sed 's/awaiting_person//g' | grep -qi 'person'; then
    echo "decider-rework-spent: checks pass (three rejections: rework_spent by rule:autopilot, the three reasons named)"
  else
    echo "decider-rework-spent: FAILED (exit $code, or the stop record: $(jq -c '{cause, reason}' "$r/run-stopped.json" 2>/dev/null))"
  fi
  # rework-not-called: runs/trial-3-haiku-4's list story. change.implement on llm-node.sh's apply
  # path (fixture adapter); merge.approve rejected once, then approved. The rework (with the
  # rejection) raises an agent finding (fixtures/finding-story-rework.yaml) and writes a tree that
  # does not build (fixtures/rework-not-called/impl.1); the finding gate stops tests.run first, the
  # agent amendment takes the default, and the next run is "not called" (the writer's own default).
  # tests.run then fails on the tree. Passes when that not-called run is recorded `not_called` and
  # not counted: a fourth run (third call) fixes the tree, the merge is approved, the run closes
  # (before: writer_spent, 3 runs, as in haiku-4).
  trial rework-not-called SDC_LLM_ADAPTER="$fixture" SDC_LLM_NODES=change.implement \
    SDC_STUB_FIXTURES="$fx/rework-not-called" SDC_STUB_FINDINGS="$fx/finding-story-rework.yaml" \
    SDC_DECIDER_REPLIES="$(jq -cn --argjson j "$rej" --argjson a "$app" '{"merge.approve": {"rounds": [$j, $a]}}')"
  code=$?
  r=$s/runs/rework-not-called
  st=$(find "$r/store" -type d -name 'IDEA-1.*.invoices' | head -n 1)
  if (( code == 0 )) && [[ -n $st ]] &&
     jq -se '[.[] | select(.event == "action_finished" and .action == "change.implement")] as $c
       | ($c | length) == 4 and ([$c[] | select(.not_called | type == "string")] | length) == 1
       and ([.[] | select(.event == "writer_spent")] | length) == 0' "$st/events.jsonl" >/dev/null &&
     [[ $(jq -s '[.[] | select(.action == "change.implement")] | length' "$r/llm-calls.jsonl") == 3 ]] &&
     jq -e '[.[] | .decision] == ["reject", "approve"]' <<< "$(attempts "$r" merge.approve)" >/dev/null; then
    echo "rework-not-called: checks pass (the not-called run recorded not_called and not counted; a fourth run fixed the tree; approved, closed)"
  else
    echo "rework-not-called: FAILED (exit $code, stop $(jq -c '{cause, reason}' "$r/run-stopped.json" 2>/dev/null | cut -c1-300))"
  fi
  # rework-pending-rejection: the same story case cut back to the moment after the rejection (its
  # events up to the rejected merge.approve), with impl/ derived before the writer's own default
  # was taken: llm-node.sh change.implement without SDC_FEEDBACK. Passes when the writer is called
  # with in/rejection.json (a rejection no write answered yet), not skipped as "not called".
  p=$s/pending; rm -rf "$p"; mkdir -p "$p"
  if [[ -n $st ]]; then
    cp -a "$st" "$p/case" && rm -f "$p/case/work" && cp -rL "$r/store/IDEA-1/work" "$p/work"
    k=$(jq -s '[.[] | select(.event == "action_finished" and .result == "rejected")] | first | .seq' "$st/events.jsonl")
    jq -c --argjson k "$k" 'select(.seq <= $k)' "$st/events.jsonl" > "$p/case/events.jsonl"
    sed -i '1s/ resolved .*$/ resolved /' "$p/work/impl/.derived-from"
    out=$(cd "$p/work" && env -u SDC_FEEDBACK -u SDC_NOT_CALLED -u SDC_STUB_FINDINGS SDC_CASE_DIR="$p/case" SDC_CASE="$(jq -r .id "$st/case.json")" \
      SDC_LLM_ADAPTER="$fixture" SDC_LLM_NODE="$(jq -c '."change.implement".llm' "$st/bindings.json")" SDC_LLM_RECORD="$p/llm" \
      "$sdc/driver/examples/sdc/tools/llm-node.sh" change.implement 2>&1)
    pc=$?
  fi
  if [[ -n $st ]] && (( pc == 0 )) && ! grep -q 'not called' <<< "$out" &&
     jq -e '.format == "sdc.rejection/1" and .approval == "merge.approve"' "$(ls -d "$p"/llm/*-change.implement 2>/dev/null | tail -n 1)/cwd/in/rejection.json" >/dev/null 2>&1; then
    echo "rework-pending-rejection: checks pass (a pending rejection: the writer called with in/rejection.json, the own-default skip not taken)"
  else
    echo "rework-pending-rejection: FAILED (exit ${pc:-none}: $(tail -n 1 <<< "${out:-}" | cut -c1-200))"
  fi
fi
# decider-later-obligation: runs/trial-3-mixed-3's refused release. The autopilot cycle's decider
# records (no new run): release.publish's in/checks.json does not list health.required as open (it
# needs release.healthy, a health_observation of release.observe, admissible only after the
# publish) but under met_after_this_action, and the prompt says it is no condition; vision.approve's
# moves roadmap.required (roadmap.approve comes after it).
rp=$(ls -d "$s"/runs/autopilot/decider/*-release.publish 2>/dev/null | tail -n 1)
va=$(ls -d "$s"/runs/autopilot/decider/*-vision.approve 2>/dev/null | tail -n 1)
if [[ -n $rp && -n $va ]] &&
   jq -e '(.open_obligations | index("health.required") | not) and .met_after_this_action.action == "release.publish"
     and .met_after_this_action.obligations == ["health.required"]' "$rp/cwd/in/checks.json" >/dev/null &&
   grep -q '^`met_after_this_action` .* after release.publish: not a condition of this decision' "$rp/prompt.md" &&
   jq -e '(.open_obligations | index("roadmap.required") | not) and (.met_after_this_action.obligations | index("roadmap.required"))' "$va/cwd/in/checks.json" >/dev/null; then
  echo "decider-later-obligation: checks pass (health.required met after release.publish, roadmap.required after vision.approve: listed apart, not open)"
else
  echo "decider-later-obligation: FAILED (release.publish checks: $(jq -c '{open_obligations, met_after_this_action}' "$rp/cwd/in/checks.json" 2>/dev/null))"
fi
# replay-sibling: runs/trial-3-haiku-4's replay (IDEA-1.todo: 7 of 9). Two stories opened by one
# stories.open (fixtures/plan-two-stories.json through plan.decompose's apply path); the first
# closes, the second's change.implement raises a scope-changing finding and the run stops there
# for a person (exit 3). Passes when `sdc-drive replay` of the stopped tree exits 0: the epic was
# evaluated after the child_closed that rewrote its record.
cycle replay-sibling SDC_LLM_NODES=plan.decompose SDC_LLM_ADAPTER="$fixture" \
  SDC_FIXTURE_STUB='{"plan.decompose":["plan-two-stories.json","stories.json"]}' SDC_STUB_FINDINGS="$fx/finding-story-second.yaml"
code=$?
root=$s/store-replay-sibling/IDEA-1
"$sdc/driver/target/debug/sdc-drive" --canon "${CANON:-$HOME/beyond10x/canon/target/debug/canon}" --case-dir "$root" replay > "$s/replay-sibling.json" 2>&1
rc=$?
if (( code == 3 && rc == 0 )) &&
   jq -se '[.[] | select(.event == "child_closed") | .child_case] | index("IDEA-1.billing.emails")' "$root/children/IDEA-1.billing/events.jsonl" >/dev/null &&
   ! jq -e 'has("termination")' "$root/children/IDEA-1.billing/children/IDEA-1.billing.invoices/case.json" >/dev/null; then
  echo "replay-sibling: checks pass (one story closed, stopped in its sibling; replay exit 0)"
else
  echo "replay-sibling: FAILED (cycle exit $code, replay exit $rc: $(jq -c '[.[] | select(.state_matches_log == false or .decision_matches_log == false) | .case]' "$s/replay-sibling.json" 2>/dev/null))"
fi
# writer-not-called: a copy of replay-sibling's stopped tree. In the waiting story, change.implement
# is bound to a program that only says it is not called (SDC_NOT_CALLED), amend.upstream to
# nothing; the open finding is dismissed and impl/Cargo.toml removed (edits: an arrival), so
# tests.run fails and sends the writer back. Passes when the run stops with the verdict
# writer_not_called (each run not called, none counted), not three runs into writer_spent.
w=$s/not-called; rm -rf "$w"; cp -a "$s/store-replay-sibling" "$w"
sc=$w/IDEA-1/children/IDEA-1.billing/children/IDEA-1.billing.invoices
if [[ -f $sc/bindings.json ]]; then
  jq '."change.implement".argv = ["sh", "-c", "echo \"fixture: not called\" > \"$SDC_NOT_CALLED\""] | del(."amend.upstream")' \
    "$sc/bindings.json" > "$sc/bindings.json.new" && mv "$sc/bindings.json.new" "$sc/bindings.json"
  yq -i '(.findings[] | select(.status == "needs_person")).status = "dismissed"' "$w/IDEA-1/work/findings.yaml"
  rm -f "$w/IDEA-1/work/impl/Cargo.toml"
  wo=$("$sdc/driver/target/debug/sdc-drive" --canon "${CANON:-$HOME/beyond10x/canon/target/debug/canon}" --case-dir "$w/IDEA-1" \
    run --at "$(date -u +%Y-%m-%dT%H:%M:%SZ)" 2> "$s/writer-not-called.err")
  wc_=$?
fi
if [[ -f $sc/bindings.json ]] && (( wc_ == 4 )) && jq -e '.verdict == "writer_not_called"' <<< "$wo" >/dev/null 2>&1 &&
   jq -se '([.[] | select(.event == "writer_not_called")] | length) == 1 and ([.[] | select(.event == "writer_spent")] | length) == 0
     and ([.[] | select(.event == "action_finished" and .action == "change.implement" and (.not_called | type == "string"))] | length) >= 1' "$sc/events.jsonl" >/dev/null; then
  echo "writer-not-called: checks pass (not-called runs, uncounted, then the stop writer_not_called)"
else
  echo "writer-not-called: FAILED (exit ${wc_:-none}: $(jq -r '.verdict // empty' <<< "${wo:-}" 2>/dev/null) $(tail -n 1 <<< "${wo:-}" | cut -c1-200))"
fi
# cite-dark-spot: runs/trial-3-mixed-4's stop. Under autopilot, requirements.normalise writes r1
# citing q2 beside the dark spot d4 that q2 asks about (fixtures/cite-dark-spot) and raises a
# scope-changing finding on r1 (fixtures/finding-intake-scope.yaml); the decider takes its default.
# Passes when the citation is accepted at write time and the decision is applied (f1 resolved by
# decider:haiku, no refused_decision) and the run closes (before: decision_not_takeable, exit 7).
intake=IDEA-1/children/IDEA-1.intake
trial cite-dark-spot SDC_STUB_FIXTURES="$fx/cite-dark-spot" SDC_STUB_FINDINGS="$fx/finding-intake-scope.yaml"
code=$?
r=$s/runs/cite-dark-spot
ci=$r/store/$intake
if (( code == 0 )) && [[ ! -f $r/run-stopped.json ]] &&
   yq -o=json "$r/store/IDEA-1/work/findings.yaml" | jq -e '.findings[] | select(.says | test("cancelled invoice is shown"))
     | .status == "resolved" and .decided_by == "decider:haiku" and .refused_decision == null' >/dev/null &&
   jq -se '[.[] | select(.event == "action_finished" and .action == "requirements.check")] | last | .exit == "0"' "$ci/events.jsonl" >/dev/null &&
   ! jq -r 'select(.event == "action_finished" and .action == "requirements.check") | .log // empty' "$ci/events.jsonl" |
     xargs -I{} cat "$ci/blobs/{}" 2>/dev/null | grep -q ' cites '; then
  echo "cite-dark-spot: checks pass (a dark spot beside its question accepted at write; the decider's decision applied; closed)"
else
  echo "cite-dark-spot: FAILED (exit $code, stop $(jq -c '{cause, reason}' "$r/run-stopped.json" 2>/dev/null | cut -c1-300))"
fi
# cite-fault-sent-back: as cite-dark-spot, but r2 cites the unknown d3 without the question that
# asks about it (fixtures/cite-fault), a fault the decision on r1 did not write. Passes when the
# decision is still applied (not decision_not_takeable), requirements.check names the fault in its
# log, and requirements.normalise is sent back with it until its 3 runs since the decision are
# spent, then the run stops writer_spent naming requirements.normalise (the stub writes the same
# requirements every time).
trial cite-fault-sent-back SDC_STUB_FIXTURES="$fx/cite-fault" SDC_STUB_FINDINGS="$fx/finding-intake-scope.yaml"
code=$?
r=$s/runs/cite-fault-sent-back
ci=$r/store/$intake
if (( code == 4 )) && jq -e '.cause == "writer_spent" and (.reason | test("requirements.normalise in IDEA-1.intake"))' "$r/run-stopped.json" >/dev/null &&
   yq -o=json "$r/store/IDEA-1/work/findings.yaml" | jq -e '.findings[] | select(.says | test("cancelled invoice is shown"))
     | .status == "resolved" and .decided_by == "decider:haiku" and .refused_decision == null' >/dev/null &&
   jq -se '[.[] | select(.event == "writer_spent")] | last | .spent[0] | .action == "requirements.normalise" and .runs == 3
     and (.sent_back[0] | .by == "requirements.check" and (.line | test("r2 cites d3")))' "$ci/events.jsonl" >/dev/null &&
   jq -r 'select(.event == "action_finished" and .action == "requirements.check") | .log' "$ci/events.jsonl" | tail -n 1 |
     xargs -I{} grep -q 'r2 cites d3, an unknown that no question it cites asks about' "$ci/blobs/{}" &&
   [[ $(grep -o 'LLM calls so far [0-9]*' "$s/cite-fault-sent-back.txt" | tail -n 1 | grep -o '[0-9]*$') == \
      "$(find "$r/store" -name events.jsonl -exec cat {} + | jq -s '([.[] | select(.event == "action_started" and .llm == true)] | length)
        - ([.[] | select(.event == "action_finished" and (.not_called | type) == "string")] | length)')" ]]; then
  echo "cite-fault-sent-back: checks pass (the decision applied; the writer sent back with the citation fault, then writer_spent)"
else
  echo "cite-fault-sent-back: FAILED (exit $code, stop $(jq -c '{cause, reason}' "$r/run-stopped.json" 2>/dev/null | cut -c1-300))"
fi
# autopilot-close: runs/trial-3-haiku-5's stop (D140). Under autopilot the decider fixture
# rejects `close` if asked. Passes when the run closes `operating` (exit 0, status.json
# closed_by rule:autopilot), the root's terminated event names rule:autopilot, and no close
# request exists in decider/ or decisions.jsonl (before: the decider was asked, rejected, and
# the run stopped rejected_without_rework_path). autopilot-close-a3: the same reply at A3 is still
# asked for the close and stops the run (exit 6), nothing closed.
crej='{"decision": "reject", "answer": null, "reasons": ["fixture: the close is refused"], "cites": ["r1"], "best": null}'
targs=(--autopilot)
trial autopilot-close SDC_DECIDER_REPLIES="$(jq -cn --argjson j "$crej" '{close: $j}')"
code=$?
r=$s/runs/autopilot-close
if (( code == 0 )) && [[ ! -f $r/run-stopped.json ]] &&
   jq -e '.outcome == "operating" and .closed_by == "rule:autopilot"' "$r/status.json" >/dev/null &&
   jq -se '[.[] | select(.event == "terminated")] | length == 1 and .[0].outcome == "operating" and .[0].principal == "rule:autopilot"' "$r/store/IDEA-1/events.jsonl" >/dev/null &&
   [[ -z $(find "$r/decider" -mindepth 1 -maxdepth 1 -name '*-close' 2>/dev/null) ]] &&
   ! jq -e 'select(.class == "close")' "$r/decisions.jsonl" >/dev/null 2>&1; then
  echo "autopilot-close: checks pass (closed operating by rule:autopilot; the decider was not asked)"
else
  echo "autopilot-close: FAILED (exit $code, stop $(jq -c '{cause, reason}' "$r/run-stopped.json" 2>/dev/null | cut -c1-300))"
fi
targs=()
trial autopilot-close-a3 SDC_AUTONOMY=A3 SDC_DECIDER_REPLIES="$(jq -cn --argjson j "$crej" '{close: $j}')"
code=$?
r=$s/runs/autopilot-close-a3
if (( code == 6 )) && [[ ! -f $r/status.json ]] && [[ -n $(find "$r/decider" -mindepth 1 -maxdepth 1 -name '*-close' 2>/dev/null) ]] &&
   jq -se 'any(.[]; .class == "close" and .decision == "reject" and .principal == "decider:haiku")' "$r/decisions.jsonl" >/dev/null &&
   ! jq -e 'select(.event == "terminated")' "$r/store/IDEA-1/events.jsonl" >/dev/null 2>&1; then
  echo "autopilot-close-a3: checks pass (A3: the decider is asked for the close, rejects, the run stops unclosed)"
else
  echo "autopilot-close-a3: FAILED (exit $code)"
fi
# Keep-going (D142, --autopilot --keep-going). keep-going-three: one run meets three different
# stops: requirements.normalise writes a citation fault three times (fixtures/keep-going: the
# n-th call's requirements.yaml.<n>, then the good one) -> writer_spent; the decider's option 2 on
# the spec finding f1 is refused by the check (SDC_STUB_REFUSE_DECISION simulates the fault) ->
# decision_not_takeable; merge.approve rejected three times -> rework_spent. Passes when each is
# forced once by rule:keep-going (one more allowance, f1's default, the approval), the run closes
# operating with `(forced 3)`, status.json forced 3, three forced lines in defects.jsonl naming the
# three causes, and f1 decided by rule:keep-going. keep-going-repeat: fixtures/offline-decision,
# whose default the check refuses: forced once, refused again by the same check -> stops
# decision_not_takeable (exit 7), one forced line and one `stops` line. keep-going-off: as
# keep-going-three without the flag: stops at the first (writer_spent, exit 4), no defects.jsonl.
opt2="the customer and the billing clerk"
mrej='{"decision": "reject", "answer": null, "reasons": ["fixture: the server binds only to 127.0.0.1"], "cites": ["r1"], "best": null}'
kgreplies=$(jq -cn --arg o "$opt2" --argjson j "$mrej" '{f1: {decision: "approve", answer: $o, best: null, cites: ["r2"],
  reasons: ["fixture: the clerk gets the email too"]}, "merge.approve": $j}')
kgenv=(SDC_STUB_FIXTURES="$fx/keep-going" SDC_STUB_FINDINGS="$fx/finding-spec-person.yaml" SDC_STUB_REFUSE_DECISION="$opt2"
  SDC_DECIDER_REPLIES="$kgreplies")
if yq -e '.actions."merge.approve"' "$sdc/protocol/sdc-story.protocol.yaml" >/dev/null 2>&1; then
  targs=(--autopilot --keep-going)
  trial keep-going-three "${kgenv[@]}"
  code=$?
  r=$s/runs/keep-going-three
  if (( code == 0 )) && jq -e '.outcome == "operating" and .forced == 3' "$r/status.json" >/dev/null &&
     grep -q '^closed IDEA-1: "operating" by rule:autopilot (forced 3)' "$s/keep-going-three.txt" &&
     jq -se 'length == 3 and all(.[]; .forced == true and .principal == "rule:keep-going" and .format == "sdc.run-defect/1"
       and (.reason | length > 0) and (.event_seq | type == "number") and (.llm_calls | type == "number"))
       and ([.[] | .cause] == ["writer_spent", "decision_not_takeable", "rework_spent"])' "$r/defects.jsonl" >/dev/null &&
     yq -o=json "$r/store/IDEA-1/work/findings.yaml" | jq -e '.findings[] | select(.id == "f1") | .status == "resolved"
       and .decided_by == "rule:keep-going" and .decision == "the customer only"' >/dev/null &&
     "$kpis" "$r" --json | jq -e '.forced.value == 3' >/dev/null; then
    echo "keep-going-three: checks pass (writer_spent, decision_not_takeable, rework_spent forced by rule:keep-going; closed (forced 3))"
  else
    echo "keep-going-three: FAILED (exit $code, defects $(jq -sc 'map([.cause, .forced])' "$r/defects.jsonl" 2>/dev/null), stop $(jq -c '{cause, reason}' "$r/run-stopped.json" 2>/dev/null | cut -c1-300))"
  fi
  targs=(--autopilot)
  trial keep-going-off "${kgenv[@]}"
  code=$?
  r=$s/runs/keep-going-off
  if (( code == 4 )) && jq -e '.cause == "writer_spent" and (.reason | test("requirements.normalise in IDEA-1.intake"))' "$r/run-stopped.json" >/dev/null &&
     [[ ! -e $r/defects.jsonl && ! -f $r/status.json ]]; then
    echo "keep-going-off: checks pass (without --keep-going the first stop ends the run: writer_spent, no defects.jsonl)"
  else
    echo "keep-going-off: FAILED (exit $code, stop $(jq -c '{cause, reason}' "$r/run-stopped.json" 2>/dev/null | cut -c1-300))"
  fi
fi
targs=(--autopilot --keep-going)
trial keep-going-repeat SDC_STUB_FIXTURES="$fx/offline-decision"
code=$?
r=$s/runs/keep-going-repeat
if (( code == 7 )) && jq -e '.cause == "decision_not_takeable" and .by == "rule:autopilot"' "$r/run-stopped.json" >/dev/null &&
   jq -se 'length == 2 and .[0].forced == true and .[0].cause == "decision_not_takeable" and .[1].forced == false
     and .[1].cause == "decision_not_takeable" and .[1].continuation == "stops"' "$r/defects.jsonl" >/dev/null &&
   yq -o=json "$r/store/IDEA-1/work/findings.yaml" | jq -e 'any(.findings[]; .refused_decision.decided_by == "rule:keep-going")' >/dev/null; then
  echo "keep-going-repeat: checks pass (the default forced once, refused again by the same check: stops decision_not_takeable)"
else
  echo "keep-going-repeat: FAILED (exit $code, defects $(jq -sc 'map([.cause, .forced])' "$r/defects.jsonl" 2>/dev/null), stop $(jq -c '{cause, reason}' "$r/run-stopped.json" 2>/dev/null | cut -c1-300))"
fi
# Unstick (D147, run.unstick on the fixture adapter: SDC_UNSTICK_REPLIES by stop cause).
# unstick-end: merge.approve rejected every time (as decider-rework-spent): the third rejection is
# the stop rework_spent; run.unstick chooses end:dropped. Passes when the decider's third
# rejection is recorded, explicitly_dropped by agent:unstick directly after it, the story closed
# dropped, the run closed not_delivered by rule:autopilot (exit 0) with `(unstuck 1)`,
# status.json unstuck 1, one defects.jsonl line (unstick, unstuck, the move, the diagnosis, a
# defect class, principal agent:unstick) and the KPIs unstuck 1, not clean.
# unstick-rerun: fixtures/keep-going without --keep-going: requirements.normalise writes a
# citation fault three times -> writer_spent; run.unstick chooses rerun:requirements.normalise.
# Passes when one allowance is granted by agent:unstick, the writer runs once more (fed back as on
# any retry) and the run closes operating with `(unstuck 1)`. unstick-invalid: fixtures/cite-fault
# (the fault every time); every reply names a move not listed: asked again twice, then the run
# stops writer_spent (exit 4) as without run.unstick, one defect line `invalid reply after 2
# retries`. unstick-limit: fixtures/cite-fault with rerun chosen: one allowance, spent again, the
# same cause at the same action is not unstuck twice: stops unstick_spent (exit 4).
# unstick-off: unstick-rerun's run with SDC_UNSTICK=off stops writer_spent as keep-going-off.
udiag="fixture run.unstick: the stop is a defect of the system, named here for the people who fix it."
ureply() { jq -cn --arg m "$1" --arg c "$2" --arg d "$udiag" '{move: $m, diagnosis: $d, defect_class: $c, reasons: ["fixture: \($m)"]}'; }
if yq -e '.actions."merge.approve"' "$sdc/protocol/sdc-story.protocol.yaml" >/dev/null 2>&1; then
  targs=(--autopilot)
  trial unstick-end SDC_UNSTICK=on SDC_DECIDER_REPLIES="$(jq -cn --argjson j "$rej" '{"merge.approve": $j}')" \
    SDC_UNSTICK_REPLIES="$(jq -cn --argjson r "$(ureply end:dropped decider_error)" '{rework_spent: $r}')"
  code=$?
  r=$s/runs/unstick-end
  st=$(find "$r/store" -type d -name 'IDEA-1.*.invoices' | head -n 1)
  if (( code == 0 )) && [[ -n $st && ! -f $r/run-stopped.json ]] &&
     jq -e '.outcome == "not_delivered" and .closed_by == "rule:autopilot" and .unstuck == 1' "$r/status.json" >/dev/null &&
     grep -q '^closed IDEA-1: "not_delivered" by rule:autopilot (unstuck 1)' "$s/unstick-end.txt" &&
     jq -se '[.[] | select(.event != "evaluated")] | .[-3:] | (.[0].event == "action_finished" and .[0].action == "merge.approve"
       and .[0].result == "rejected") and (.[1].event == "decision_recorded" and .[1].decision == "explicitly_dropped"
       and .[1].principal == "agent:unstick") and (.[2].event == "terminated" and .[2].outcome == "dropped")' "$st/events.jsonl" >/dev/null &&
     [[ $(jq -s '[.[] | select(.kind == "merge_approval" and .result == "rejected")] | length' "$st"/evidence/*.json) == 3 ]] &&
     jq -se 'length == 1 and (.[0] | .unstick == true and .unstuck == true and .move == "end:dropped" and .cause == "rework_spent"
       and .principal == "agent:unstick" and .defect_class == "decider_error" and (.diagnosis | length > 20) and .forced == false)' "$r/defects.jsonl" >/dev/null &&
     [[ -s $(ls -d "$r"/unstick/001-rework_spent)/cwd/in/stop.json ]] &&
     jq -e '[.moves[].id] | index("end:dropped") and index("approve:merge.approve") and index("stop")' "$r/unstick/001-rework_spent/cwd/in/stop.json" >/dev/null &&
     "$kpis" "$r" --json | jq -e '.unstuck.value == 1 and .clean_run.value == false and .unstuck.diagnoses[0].move == "end:dropped"' >/dev/null; then
    echo "unstick-end: checks pass (rework_spent: run.unstick ended the story through dropped; closed not_delivered (unstuck 1))"
  else
    echo "unstick-end: FAILED (exit $code, defects $(jq -sc 'map([.cause, .move, .continuation])' "$r/defects.jsonl" 2>/dev/null | cut -c1-300), stop $(jq -c '{cause, reason}' "$r/run-stopped.json" 2>/dev/null | cut -c1-300))"
  fi
fi
targs=(--autopilot)
trial unstick-rerun SDC_UNSTICK=on SDC_STUB_FIXTURES="$fx/keep-going" \
  SDC_UNSTICK_REPLIES="$(jq -cn --argjson r "$(ureply rerun:requirements.normalise writer_error)" '{writer_spent: $r}')"
code=$?
r=$s/runs/unstick-rerun
ci=$r/store/$intake
if (( code == 0 )) && jq -e '.outcome == "operating" and .unstuck == 1' "$r/status.json" >/dev/null &&
   grep -q '^closed IDEA-1: "operating" by rule:autopilot (unstuck 1)' "$s/unstick-rerun.txt" &&
   jq -se '[.[] | select(.event == "allowance_granted")] | length == 1 and .[0].action == "requirements.normalise"
     and .[0].principal == "agent:unstick"' "$ci/events.jsonl" >/dev/null &&
   jq -se '[.[] | select(.event == "action_finished" and .action == "requirements.normalise")] | length == 4' "$ci/events.jsonl" >/dev/null &&
   jq -se '[.[] | select(.event == "action_started" and .action == "requirements.normalise")] | last | .retry_of == ["requirements_shape"]' "$ci/events.jsonl" >/dev/null &&
   jq -se 'length == 1 and (.[0] | .unstuck == true and .move == "rerun:requirements.normalise" and .cause == "writer_spent")' "$r/defects.jsonl" >/dev/null &&
   jq -se 'last.unstick | .on == true and .model == "claude-opus-5-5"' "$r/run-settings.jsonl" >/dev/null; then
  echo "unstick-rerun: checks pass (writer_spent: one allowance by agent:unstick, the writer ran again, closed (unstuck 1))"
else
  echo "unstick-rerun: FAILED (exit $code, defects $(jq -sc 'map([.cause, .move, .continuation])' "$r/defects.jsonl" 2>/dev/null | cut -c1-300), stop $(jq -c '{cause, reason}' "$r/run-stopped.json" 2>/dev/null | cut -c1-300))"
fi
trial unstick-invalid SDC_UNSTICK=on SDC_STUB_FIXTURES="$fx/cite-fault" SDC_STUB_FINDINGS="$fx/finding-intake-scope.yaml" \
  SDC_UNSTICK_REPLIES="$(jq -cn --argjson r "$(ureply approve:nothing other)" '{"*": $r}')"
code=$?
r=$s/runs/unstick-invalid
if (( code == 4 )) && jq -e '.cause == "writer_spent" and (.reason | test("requirements.normalise in IDEA-1.intake"))' "$r/run-stopped.json" >/dev/null &&
   jq -se 'length == 1 and (.[0] | .forced == false and .cause == "writer_spent" and .unstick == true and .unstuck == false and .attempts == 3
     and (.continuation | test("invalid reply after 2 retries: move \"approve:nothing\" is not one of the ids")))' "$r/defects.jsonl" >/dev/null &&
   jq -e '.attempts == 3 and .valid == false' "$r/unstick/001-writer_spent/result.json" >/dev/null &&
   [[ $(grep -c 'run.unstick: reply refused by the check' "$s/unstick-invalid.txt") == 3 ]] && [[ ! -f $r/status.json ]]; then
  echo "unstick-invalid: checks pass (three invalid replies: no move, the run stops writer_spent)"
else
  echo "unstick-invalid: FAILED (exit $code, defects $(jq -sc 'map([.cause, .continuation])' "$r/defects.jsonl" 2>/dev/null | cut -c1-300), stop $(jq -c '{cause, reason}' "$r/run-stopped.json" 2>/dev/null | cut -c1-300))"
fi
trial unstick-limit SDC_UNSTICK=on SDC_STUB_FIXTURES="$fx/cite-fault" SDC_STUB_FINDINGS="$fx/finding-intake-scope.yaml" \
  SDC_UNSTICK_REPLIES="$(jq -cn --argjson r "$(ureply rerun:requirements.normalise writer_error)" '{"*": $r}')"
code=$?
r=$s/runs/unstick-limit
ci=$r/store/$intake
if (( code == 4 )) && jq -e '.cause == "unstick_spent" and .by == "rule:autopilot"
     and (.reason | test("unstick_spent: writer_spent at requirements.normalise in IDEA-1.intake[^ ]* was unstuck once before"))' "$r/run-stopped.json" >/dev/null &&
   jq -se 'length == 1 and .[0].unstuck == true and .[0].move == "rerun:requirements.normalise"' "$r/defects.jsonl" >/dev/null &&
   jq -se '[.[] | select(.event == "allowance_granted")] | length == 1' "$ci/events.jsonl" >/dev/null &&
   [[ $(find "$r/unstick" -mindepth 1 -maxdepth 1 -type d | wc -l) == 1 ]]; then
  echo "unstick-limit: checks pass (one allowance, spent again: the same stop is not unstuck twice, unstick_spent)"
else
  echo "unstick-limit: FAILED (exit $code, defects $(jq -sc 'map([.cause, .move, .continuation])' "$r/defects.jsonl" 2>/dev/null | cut -c1-300), stop $(jq -c '{cause, reason}' "$r/run-stopped.json" 2>/dev/null | cut -c1-300))"
fi
trial unstick-off SDC_UNSTICK=off SDC_STUB_FIXTURES="$fx/keep-going" \
  SDC_UNSTICK_REPLIES="$(jq -cn --argjson r "$(ureply rerun:requirements.normalise writer_error)" '{writer_spent: $r}')"
code=$?
r=$s/runs/unstick-off
if (( code == 4 )) && jq -e '.cause == "writer_spent" and (.reason | test("requirements.normalise in IDEA-1.intake"))' "$r/run-stopped.json" >/dev/null &&
   [[ ! -e $r/defects.jsonl && ! -e $r/unstick && ! -f $r/status.json ]] &&
   jq -se 'last.unstick.on == false' "$r/run-settings.jsonl" >/dev/null; then
  echo "unstick-off: checks pass (SDC_UNSTICK=off: the first stop ends the run as today, writer_spent)"
else
  echo "unstick-off: FAILED (exit $code, stop $(jq -c '{cause, reason}' "$r/run-stopped.json" 2>/dev/null | cut -c1-300))"
fi
# reopen-refused-decision: runs/trial-3-mixed-4 reopened did nothing (f1 stayed needs_person with
# its refused_decision). Leg 1 under autopilot: the decider's option 2 on the spec finding f1 is
# refused by the check (SDC_STUB_REFUSE_DECISION simulates the fault) -> decision_not_takeable,
# exit 7. Leg 2, `--reopen` with the fault fixed (the variable unset): the stored decision is put
# back and applied through the current check; f1 resolved with option 2 by decider:haiku, the run
# closes (exit 0), the first stop record kept as run-stopped.<at>.json.
targs=(--autopilot)
opt2="the customer and the billing clerk"
rdreplies=$(jq -cn --arg o "$opt2" '{f1: {decision: "approve", answer: $o, best: null, cites: ["r2"], reasons: ["fixture: the clerk gets the email too"]}}')
trial reopen-refused-decision SDC_STUB_FINDINGS="$fx/finding-spec-person.yaml" SDC_STUB_REFUSE_DECISION="$opt2" SDC_DECIDER_REPLIES="$rdreplies"
code=$?
r=$s/runs/reopen-refused-decision
SDC_RUNS_DIR="$s/runs" SDC_WATCH=0 SDC_LLM_ADAPTER=stub SDC_DECIDER=haiku SDC_DECIDER_ADAPTER="$fixture" SDC_PRINCIPAL=agent:finding-cycles \
  SDC_STUB_FINDINGS="$fx/finding-spec-person.yaml" SDC_DECIDER_REPLIES="$rdreplies" SDC_UNSTICK=off \
  "$sdc/scripts/run-trial.sh" reopen-refused-decision --reopen --autopilot > "$s/reopen-refused-decision-reopen.txt" 2>&1
reopen=$?
if (( code == 7 && reopen == 0 )) && [[ ! -e $r/run-stopped.json ]] && ls "$r"/run-stopped.*.json >/dev/null 2>&1 &&
   jq -e '.cause == "decision_not_takeable"' "$(ls "$r"/run-stopped.*.json | head -n 1)" >/dev/null &&
   grep -q '^# reopen: 1 refused decision(s) in .* put back for the current check' "$r/run.log" &&
   yq -o=json "$r/store/IDEA-1/work/findings.yaml" | jq -e --arg o "$opt2" '.findings[] | select(.id == "f1") | .status == "resolved"
     and .decided_by == "decider:haiku" and .decision == $o and .refused_decision == null' >/dev/null &&
   "$kpis" "$r" --json | jq -e '.stopped_for_fix.reopens == 1 and .clean_run.value == false
     and .L7_waiting_on_person_seconds.value == 0 and .L7_waiting_on_person_seconds.failure == false' >/dev/null; then
  echo "reopen-refused-decision: checks pass (refused, stopped; reopened with the check fixed: the decision applied, closed; KPIs: 1 reopen, not clean, L7 0)"
else
  echo "reopen-refused-decision: FAILED (exit $code, reopen exit $reopen: $(tail -n 1 "$s/reopen-refused-decision-reopen.txt" | cut -c1-200))"
fi
targs=(--autopilot)
# A refused release (docs/driver.md, "A refused release"). release.publish's approval is the
# capability release.approve, with no `rejected` result and no writer. decider-release-a3: at A3
# the decider's reject ends the run with rejected_without_rework_path (exit 6) and nothing is
# recorded in the release case: the withheld decision is autopilot's only. A copy of that release
# case, awaiting the approval, refuses `sdc-drive decide` (the decision outside the refused
# state). Where the epic protocol has not_released: decider-release-withheld: under autopilot the
# decider rejects release.publish; release.approve is denied by the decider, explicitly_withheld
# recorded by rule:autopilot with the rejection as its log blob, the release closes withheld, the
# epic not_released, the root not_delivered; exit 0, the closed line names the reason,
# status.json says not delivered, no stop record, no person principal, L7 0, replay matches.
rej='{"decision": "reject", "answer": null, "reasons": ["fixture: the release notes name no version"], "cites": ["r1"], "best": null}'
targs=()
trial decider-release-a3 SDC_AUTONOMY=A3 SDC_DECIDER_REPLIES="$(jq -cn --argjson j "$rej" '{"release.publish": $j}')"
code=$?
r=$s/runs/decider-release-a3
rs=$(find "$r/store" -type d -name '*.release' | head -n 1)
cp_=$s/decide-refused; rm -rf "$cp_"; [[ -n $rs ]] && cp -r "$rs" "$cp_"
printf '{"format": "sdc.rejection/1", "reasons": ["fixture"]}\n' > "$s/decide-refused.json"
refusal=$("$sdc/driver/target/debug/sdc-drive" --canon "${CANON:-$HOME/beyond10x/canon/target/debug/canon}" --case-dir "$cp_" decide \
  --at "$(date -u +%Y-%m-%dT%H:%M:%SZ)" --decision explicitly_withheld --outcome withheld --principal rule:autopilot \
  --after-denial release.approve --reasons "$s/decide-refused.json" 2>&1)
rcode=$?
if (( code == 6 )) && [[ -n $rs && ! -f $r/run-stopped.json && ! -f $r/status.json ]] &&
   grep -q 'release.publish in .*rejected_without_rework_path' "$s/decider-release-a3.txt" &&
   jq -e 'length == 0' "$rs/decisions.json" >/dev/null && jq -e 'all(.decision != "denied")' "$rs/authority.json" >/dev/null &&
   (( rcode != 0 )) && grep -q 'taken only directly after `release.approve` was denied' <<< "$refusal" &&
   jq -e 'length == 0' "$cp_/decisions.json" >/dev/null; then
  echo "decider-release-a3: checks pass (A3: rejected_without_rework_path, nothing recorded; decide refused outside the refused state)"
else
  echo "decider-release-a3: FAILED (exit $code, decide exit $rcode: $(tail -n 1 <<< "$refusal"))"
fi
targs=(--autopilot)
if jq -e '.outcomes | has("not_released")' "$sdc/protocol/compiled/sdc-epic.ir.json" >/dev/null 2>&1; then
  trial decider-release-withheld SDC_DECIDER_REPLIES="$(jq -cn --argjson j "$rej" '{"release.publish": $j}')"
  code=$?
  r=$s/runs/decider-release-withheld
  rs=$(find "$r/store" -type d -name '*.release' | head -n 1)
  ep=$(dirname "$(dirname "$rs")")
  blob=$(jq -r 'select(.event == "decision_recorded") | .log' "$rs/events.jsonl" 2>/dev/null)
  people=$( { jq -r '.principal' "$r/decisions.jsonl"
              find "$r/store" -name events.jsonl -exec jq -r 'select(.event == "authority_recorded" or .event == "decision_recorded" or (.event == "action_finished" and .principal != null)) | .principal' {} +
            } | grep -Evc "$nonperson")
  if (( code == 0 && people == 0 )) && [[ ! -f $r/run-stopped.json ]] &&
     grep -q '^closed IDEA-1: "not_delivered" (release withheld: fixture: the release notes name no version)' "$s/decider-release-withheld.txt" &&
     jq -e '.state == "closed" and .outcome == "not_delivered" and .delivered == false' "$r/status.json" >/dev/null &&
     jq -e '. == [{"capability": "release.approve", "decision": "denied"}]' "$rs/authority.json" >/dev/null &&
     jq -e 'length == 1 and .[0].decision == "explicitly_withheld" and .[0].outcome == "withheld" and .[0].principal == "rule:autopilot"' "$rs/decisions.json" >/dev/null &&
     jq -e '.format == "sdc.rejection/1" and .approval == "release.publish" and .reasons == ["fixture: the release notes name no version"]' "$rs/blobs/$blob" >/dev/null &&
     jq -se '[.[] | select(.event == "terminated") | .outcome] == ["withheld"]' "$rs/events.jsonl" >/dev/null &&
     jq -se '[.[] | select(.event == "terminated") | .outcome] == ["not_released"]' "$ep/events.jsonl" >/dev/null &&
     "$kpis" "$r" --json | jq -e '.L7_waiting_on_person_seconds | .value == 0' >/dev/null &&
     jq -e 'all(.state_matches_log and .decision_matches_log)' "$r/replay.json" >/dev/null; then
    echo "decider-release-withheld: checks pass (release withheld by rule:autopilot after the decider's refusal, epic not_released, root not_delivered; exit 0, no person, L7 0, replay matches)"
  else
    echo "decider-release-withheld: FAILED (exit $code, $people person principal(s), status $(jq -c . "$r/status.json" 2>/dev/null), stop $(jq -c '{cause, reason}' "$r/run-stopped.json" 2>/dev/null))"
  fi
fi
targs=()
# principals: the person path keeps a person's principal and nothing else writes one. The scripted
# person of run-full-cycle.sh is a stand-in: finding-person's f1 carries `decided_by:
# person:stand-in` and its run-settings.jsonl says stand_in. A decision edited in by hand with no
# principal is refused by amend (finding-person-stop's work, copied): it stays needs_person, the
# handoff stays due, and nothing writes `human`.
f=$s/store-finding-person/IDEA-1/work/findings.yaml
p=$s/principals; rm -rf "$p"; cp -r "$s/store-finding-person-stop/IDEA-1/work/." "$p/" 2>/dev/null || cp -rL "$s/store-finding-person-stop/IDEA-1/work" "$p"
fid=$(yq -r '.findings[] | select(.status == "needs_person") | .id' "$p/findings.yaml" | head -n 1)
FID=$fid yq -i '(.findings[] | select(.id == strenv(FID))) |= (.decision = .options[1])' "$p/findings.yaml"
out=$(cd "$p" && "$sdc/driver/target/debug/sdc-findings" --rules "$sdc/driver/examples/sdc/tools/consistency-rules.json" amend spec 2>&1)
if yq -e '.findings[] | select(.decided_by != null) | .decided_by == "person:stand-in"' "$f" >/dev/null &&
   jq -se 'any(.stand_in == true and .stand_in_by == "scripts/run-full-cycle.sh")' "$s/runs/finding-person/run-settings.jsonl" >/dev/null &&
   [[ $out == *"no principal"* ]] && FID=$fid yq -e '.findings[] | select(.id == strenv(FID)) | .status == "needs_person" and .decided_by == null' "$p/findings.yaml" >/dev/null &&
   (cd "$p" && "$sdc/driver/examples/sdc/tools/sdc-tool.sh" findings-wait) && ! grep -rq 'human' "$p/questions.yaml"; then
  echo "principals: checks pass (person:stand-in on the person path, stand_in labelled; a decision with no principal refused)"
else
  echo "principals: FAILED (amend said: $(tail -n 1 <<< "$out"))"
fi
# models: the model per LLM node is a run setting (tools/node-classes.json). Three dry runs with
# the decider on the dry adapter (no call; each record names the model it would be called with):
# no setting (every node its binding's llm.model, the decider claude-haiku-5-5); SDC_LLM_MODEL
# for every node; one model per class (SDC_LLM_MODEL_WRITER, _IMPLEMENT, _REVIEW, _DECIDER,
# _OTHER). Passes when every line of llm-calls.jsonl and decisions.jsonl names the expected model
# and run-settings.jsonl records the effective model per class.
dry() { # <name> [env ...]
  local name=$1; shift
  rm -rf "$s/runs/$name"
  env SDC_RUNS_DIR="$s/runs" SDC_WATCH=0 SDC_DECIDER=haiku "$@" "$sdc/scripts/run-trial.sh" --dry "$name" > "$s/$name.txt" 2>&1
}
expect() { # <run> <jq program over {node, class} giving the expected model> <decider model>
  local r=$s/runs/$1
  jq -se --slurpfile c "$sdc/driver/examples/sdc/tools/node-classes.json" '
    def class($n): [$c[0].classes | to_entries[] | select(.value | index($n)) | .key] | first // "other";
    length > 0 and all(.[]; .model == ({node: .action, class: class(.action)} | '"$2"'))' "$r/llm-calls.jsonl" >/dev/null &&
  jq -se --arg d "$3" 'length > 0 and all(.[]; .model == $d)' "$r/decisions.jsonl" >/dev/null
}
dry models-binding
c1=$?
dry models-all SDC_LLM_MODEL=model-all
c2=$?
dry models-class SDC_LLM_MODEL=model-all SDC_LLM_MODEL_WRITER=model-writer SDC_LLM_MODEL_IMPLEMENT=model-implement \
  SDC_LLM_MODEL_REVIEW=model-review SDC_LLM_MODEL_DECIDER=model-decider SDC_LLM_MODEL_OTHER=model-other
c3=$?
binding=$(jq -s 'map(to_entries[] | select(.value.llm != null) | {(.key): .value.llm.model}) | add' "$sdc"/driver/examples/sdc/sdc-*.bindings.json)
echo "models: dry exits $c1 $c2 $c3"
if (( c1 == 0 && c2 == 0 && c3 == 0 )) &&
   expect models-binding "$binding[.node]" claude-haiku-5-5 &&
   expect models-all '"model-all"' claude-haiku-5-5 &&
   expect models-class '{writer: "model-writer", implement: "model-implement", other: "model-other"}[.class]' model-decider &&
   jq -se 'last.models | .writer.model == "model-writer" and .implement.model == "model-implement" and .review.model == "model-review"
     and .decider.model == "model-decider" and .other.model == "model-other" and .writer.source == "SDC_LLM_MODEL_WRITER"' "$s/runs/models-class/run-settings.jsonl" >/dev/null &&
   jq -se 'last.models | .writer.source == "binding" and .writer.nodes["spec.author"] == "claude-haiku-5-5" and .decider.model == "claude-haiku-5-5"' "$s/runs/models-binding/run-settings.jsonl" >/dev/null; then
  echo "models: checks pass (binding default, one model for all, one per class; each call names its model; run settings per class)"
else
  echo "models: FAILED"
fi
} | tee "$s/cycles.log"
summary "$s/cycles.log"
