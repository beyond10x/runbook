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
# Prints each run's exit code and its KPIs L1-L6.
#
#   scripts/run-finding-cycles.sh <scratch dir>
set -uo pipefail
sdc=$(cd "$(dirname "$0")/.." && pwd)
s=${1:?scratch dir}
fx=$sdc/driver/examples/sdc/fixtures
mkdir -p "$s/runs"
kpis=${SDC_FINDING_KPIS:-$sdc/driver/target/debug/sdc-finding-kpis}
[[ -x $kpis ]] || kpis=$HOME/beyond10x/sdc/driver/target/debug/sdc-finding-kpis
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
# operating, q2 keeps the person's answer (`answered_by: operator`) and the requirements were written
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
     | .status == "changed" and .answer == $a and .answered_by == "operator"' >/dev/null &&
   jq -se '([.[] | select(.event == "action_finished" and .action == "scope.answer") | .seq] | max) as $s
     | any(.[]; .event == "action_finished" and .action == "requirements.normalise" and .exit == "0" and .seq > $s)' "$ev" >/dev/null; then
  echo "scope-changed: checks pass (q2 the person's answer, answered_by operator; requirements written after scope.answer)"
else
  echo "scope-changed: FAILED (cycle exit $code, or q2 / the requirements order wrong)"
fi
"$kpis" "$s/runs/scope-changed" | sed 's/^/  /'
# intake-decision: runs/trial-2-train's second stop (docs/trial-2.md, "Run notes"). As
# scope-changed, plus: at the first stop one questions.check runs without yq on PATH, the person
# edits q2 and resumes before taking scope.answer, requirements.normalise raises an agent finding
# and a scope-changing one, and the person picks option 3 of the latter (not the default). Passes
# when the cycle ends operating, the check without yq recorded nothing, questions.write ran once,
# and the interview keeps q2 (operator) and the decision (human, option 3).
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
     | select((.id == "q2" and .answer == $a and .answered_by == "operator")
       or (.answer == $d and .answered_by == "human" and .status == "changed"))] | length == 2' >/dev/null &&
   jq -se '[.[] | select(.event == "action_finished")] as $f
     | ([$f[] | select(.action == "questions.write")] | length) == 1
       and ([$f[] | select(.exit == "127")] | length) == 1
       and ([$f[] | select(.exit == "127") | .evidence[]] | length) == 0' "$ev" >/dev/null; then
  echo "intake-decision: checks pass (no record without yq; questions.write once; q2 operator; decision human, option 3)"
else
  echo "intake-decision: FAILED (cycle exit $code, or a record without yq, a rewritten interview, or a lost answer)"
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
