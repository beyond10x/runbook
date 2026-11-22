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
#   superseded-answer a build contradiction replaces the spec agent's answer: it is superseded, r1
#                   cites only the new decision (D141)
#   spec-duplicate  spec.author raises the same finding twice: the repeat is not added
#   spec-own-default spec.author (apply path) raises a new agent finding on every LLM call: the
#                   amendment records its default, no LLM re-derivation, no writer_spent
#   trace-gap       a requirement traced to a domain with no command: finding, writer sent back, traced
#   trace-gap-drop  the writer never specifies it: a scope decision (the person drops it)
#   trace-constraint a requirement marked a constraint with its reason passes and is counted
#   review-reject-once change.review (fixture adapter) rejects once with a cited finding: the
#                   implementer reworks with in/rejection.json, the second review approves
#   review-minimal-input the reviews of review-reject-once had no tools and one document, whose size
#                   is recorded; an untouched file is listed, not shown, and citable by path
#                   (story:review-minimal-facts)
#   review-invalid  a rejection whose finding cites no line of the change: refused, never recorded
#   review-spent    every review rejects: `writer_spent` on change.implement, sent back by change.review
#   review-real-path   run-trial.sh --autopilot on metaharness (fake model CLIs, fixtures/fake-model):
#                   the review takes the real adapter selection, claude-cli with a response schema
#   review-invalid-stop the review's reply stays invalid there: the run stops review_invalid, not stuck
#   review-adapter-refused SDC_REVIEW_ADAPTER=metaharness: the run refuses to start (exit 64)
#   trace-constraint-decided the person answers a trace gap in runs/trial-3-small-sonnet-1's words:
#                   r3 a constraint, the earlier send-back answer superseded
#   reopen-review-invalid a run stopped review_invalid, reopened on another review adapter: the
#                   review runs again on the unchanged state, the run closes
# and, through scripts/run-trial.sh with SDC_DECIDER=model (some with haiku, its old name) on the fixture adapter (no LLM):
#   decider-scope-answer the decider answers two open scope-changing questions one by one
#                   (default, free answer) and takes scope.answer; the run closes
#   decider-model-name  as above with SDC_DECIDER=haiku (the old name): the decider is decider:model
#                   with its model id beside it (D149)
#   decider-scope-final as above under SDC_DECIDER_ESCALATE=final: an escalated question takes
#                   its default, escalation_suppressed recorded
#   decider-finding a scope-changing spec finding decided by the decider (option 2); amended
#   decider-finding-final the decider escalates the finding under final: its default taken
#   decider-finding-refused the decider's amendment contradicts decision q3: refused, reverted,
#                   a person decides (exit 3)
#   run-stop        a waiting run is stopped (--stop), a resume refused, --reopen goes on; L7
#   run-stop-decider L7 on a decider's run that waits on a person: reported FAILED
#   autopilot       --autopilot: closed with 0 person principals and L7 0 (A5, escalate final)
#   decider-input   the autopilot run's decider records: the close reads published/README.md, no
#                   aep/; Cargo.lock copied; final-mode prompt lines (runs/trial-3-haiku-5)
#   trace-reraised  a decider's decision on a trace gap applied without effect: finding_reraised
#   decider-budget-reason the decider_budget_spent stop names the step and the cap
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
#   conformance-default-off  no conformance observation after close unless SDC_CONFORMANCE=observe
#   review-no-rework-prompt  after a merge.approve rejection the review gets no rejection as rework (D138)
#   rework-cap      one rework cap (D184): rule-ends.json rework_limit, SDC_MAX_REJECTIONS over it,
#                   recorded in run-settings.jsonl and read by the driver and the rework prompt
#   keep-going-three, keep-going-off, keep-going-repeat  --keep-going (D142)
#   reopen-refused-decision  --reopen puts a refused decision back
#   unstick-end, unstick-rerun, unstick-invalid, unstick-limit, unstick-off  run.unstick (D147)
#   decider-question-reject, rework-fresh-allowance, spec-review-rework, spec-interp-unsupported, writer-own-error,
#   implement-percause, writer-sees-test-failure, scope-unattributed-reject, finding-refused-reask,
#   review-invalid-named, and the pending rework-spent-dropped, rework-spent-abandoned: the reject
#   and cap routes of docs/reject-routes.md (R1-R9, R13)
#   coverage-complete, coverage-not-built, coverage-line-outside, coverage-test-missing,
#   coverage-spent, coverage-reply-retry  change.implement's requirement coverage
#                   (story:implement-cites-requirements): complete proceeds; not built, a line
#                   outside its file, a test that does not exist: refused before tests.run, fed back,
#                   fixed; never built: writer_spent; a reply outside the schema asked again
#   bugfix-full, bugfix-not-red, bugfix-edited-test, bugfix-never-red  sdc.bugfix in autopilot on the
#                   pager seed (docs/bugfix.md): fixed; not_red then red then fixed; the fix that
#                   changed the test refused then fixed; not_reproduced
#   bugfix-from-ticket  the same from the invented ticket fixtures/bugfix-pager/ticket.yaml on a git
#                   repository (run-trial.sh --bugfix-ticket --dry): fixed; a dirty worktree refused
#   run-metrics     sdc-run-metrics on decider-rework's run: looping factor, the approval edge,
#                   merge.approve's decisions, a clean close (docs/workflow-metrics.md)
#   implement-request-build, -tests-rework, -review-rework, -invalid-retry, -invalid-named,
#   -path-escape   change.implement as one request (SDC_IMPLEMENT_MODE=request, fixture adapter):
#                   first build, rework after a failed test and after a review rejection, an invalid
#                   reply retried, an invalid reply three times (implement_reply_invalid), a path
#                   escape refused
#   replay-unchanged, replay-reject-stop, replay-reject-nearest, replay-timeout, replay-live-dry,
#   replay-check-fail, replay-recording-clean  the committed recording fixtures/recorded/stub
#                   re-driven by scripts/run-trial.sh --recorded (sdc-replay; see each cycle)
# Prints each run's exit code and its KPIs L1-L6, then `cycles: N expected, M as expected, K failed:
# <names>` and `pending: N (K still failing: ...)`; exits 1 when an expected cycle missed its
# expectation (a pending one never fails the script).
#
#   scripts/run-finding-cycles.sh <scratch dir>
set -uo pipefail
sdc=$(cd "$(dirname "$0")/.." && pwd)
source "$sdc/scripts/evaluate-cases.sh"
canon_resolve || exit 1
s=${1:?scratch dir}
fx=$sdc/driver/examples/sdc/fixtures
mkdir -p "$s/runs"
# The requester cycles ground answers in goals entries through tools/decision-grounds.sh, which runs
# goals/target/debug/sdc-goals; build it here so a fresh checkout does not exit 127 there.
(cd "$sdc/goals" && cargo build --quiet) || { echo "cycles: building goals/ (sdc-goals) failed"; exit 1; }
(cd "$sdc/driver" && cargo build --quiet --bin sdc-run-metrics) || { echo "cycles: building sdc-run-metrics failed"; exit 1; }
metrics=$sdc/driver/target/debug/sdc-run-metrics
kpis=${SDC_FINDING_KPIS:-$sdc/driver/target/debug/sdc-finding-kpis}
[[ -x $kpis ]] || { echo "cycles: sdc-finding-kpis not found (looked at SDC_FINDING_KPIS, then $sdc/driver/target/debug/sdc-finding-kpis)"; exit 1; }
# The cycles and their expectations. A cycle with a check line is as expected when it prints
# "<name>: checks pass" and no "<name>: FAILED"; one without (exit_only) when its exit line shows
# the exit given here. A cycle that prints nothing failed. Exit 1 when any cycle failed.
expected=(stub-cycle finding-agent finding-person-stop finding-person scope-answer finding-program
  scope-changed intake-decision offline-sync offline-decision plan-in-project story-decision story-spent superseded-answer
  story-late-person spec-duplicate spec-own-default trace-gap trace-gap-drop trace-constraint trace-constraint-decided
  review-reject-once review-minimal-input review-invalid review-spent decider-scope-answer decider-model-name decider-scope-final decider-finding decider-finding-final
  decider-finding-refused run-stop run-stop-decider autopilot decider-input trace-reraised decider-budget-reason autopilot-person autopilot-stop
  decider-cite-address decider-cite-retry decider-invalid-approval decider-invalid-scope
  decider-escalate-nobest decider-release-a3 requester-decider requester-no-ground principals models efforts
  review-real-path review-real-path-facts review-invalid-stop review-adapter-refused reopen-review-invalid
  rule-approvals rule-approvals-check-failed rule-approvals-revision-changed rule-approvals-off rule-approvals-a3)
# The rejection cycles follow what the protocols give: decider-reject (no rework path) while
# context.approve cannot record `rejected`, else decider-rework-context; decider-rework and
# decider-rework-spent where merge.approve exists.
ctx_rework=no
jq -e '."context.approve".results | any(.[]; . == "rejected")' "$sdc/driver/examples/sdc/sdc-direction.bindings.json" >/dev/null 2>&1 && ctx_rework=yes
if [[ $ctx_rework == no ]]; then expected+=(decider-reject); else expected+=(decider-rework-context decider-rework-model); fi
yq -e '.actions."merge.approve"' "$sdc/protocol/sdc-story.protocol.yaml" >/dev/null 2>&1 &&
  expected+=(decider-rework decider-rework-spent rework-cap rework-not-called rework-pending-rejection review-no-rework-prompt
    keep-going-three keep-going-off unstick-end run-metrics)
expected+=(decider-later-obligation replay-sibling writer-not-called)
expected+=(cite-dark-spot cite-fault-sent-back autopilot-close autopilot-close-a3 conformance-default-off keep-going-repeat reopen-refused-decision)
expected+=(unstick-rerun unstick-invalid unstick-limit unstick-off)
# Reject and cap routes (docs/reject-routes.md): built routes are expected; a route not built yet
# is pending, reported as `pending: N (K still failing)` and never failing the script.
expected+=(decider-question-reject rework-fresh-allowance spec-review-rework spec-interp-unsupported writer-own-error implement-percause
  writer-sees-test-failure scope-unattributed-reject finding-refused-reask review-invalid-named)
# change.implement's requirement coverage (story:implement-cites-requirements).
expected+=(coverage-complete coverage-not-built coverage-line-outside coverage-test-missing coverage-spent
  coverage-reply-retry)
pending=(rework-spent-dropped rework-spent-abandoned)
expected+=(implement-request-build implement-request-tests-rework implement-request-review-rework implement-request-invalid-retry
  implement-request-invalid-named implement-request-path-escape)
expected+=(bugfix-full bugfix-not-red bugfix-edited-test bugfix-never-red bugfix-from-ticket)
expected+=(replay-unchanged replay-reject-stop replay-reject-nearest replay-timeout replay-live-dry replay-check-fail
  replay-recording-clean rule-approvals-default review-mode-default implement-coverage-default)
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
  local still=() passing=()
  for name in "${pending[@]}"; do
    if grep -q "^$name: checks pass" "$log" && ! grep -q "^$name: FAILED" "$log"; then passing+=("$name"); else still+=("$name"); fi
  done
  echo "pending: ${#pending[@]} (${#still[@]} still failing: ${still[*]:-none}; passing, move to expected: ${passing[*]:-none})"
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
  SDC_LLM_ADAPTER="$sdc/bindings/tools/llm-adapter-fixture.sh" \
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
  SDC_LLM_ADAPTER="$sdc/bindings/tools/llm-adapter-fixture.sh" \
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
  SDC_LLM_ADAPTER="$sdc/bindings/tools/llm-adapter-fixture.sh" \
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
# superseded-answer: runs/trial-3-haiku-5's r1 (D141). spec.author's stub raises a missing decision
# answered by the agent's default "Postgres" (q3, on r1); change.implement's stub raises a
# contradiction citing q3 and r1, answered by the agent's default "the data file" (q4). Passes when
# the cycle ends operating, q3 is superseded by q4, r1 cites q4 and not q3, and r1's text names no
# Postgres and no q3.
cycle superseded-answer SDC_STUB_FINDINGS="$fx/finding-superseded-spec.yaml:$fx/finding-superseded-story.yaml"
code=$?
w=$s/store-superseded-answer/IDEA-1/work
if (( code == 0 )) &&
   yq -o=json "$w/questions.yaml" | jq -e '[.questions[] | select(.id == "q3" or .id == "q4")]
     | (.[0].status == "superseded" and .[0].superseded_by == "q4" and (.[1].answer | test("data file")))' >/dev/null &&
   yq -o=json "$w/requirements.yaml" | jq -e '.requirements[] | select(.id == "r1")
     | ((.cites | index("q4")) != null and (.cites | index("q3")) == null and (.text | test("Postgres|q3") | not))' >/dev/null; then
  echo "superseded-answer: checks pass (q3 superseded by q4; r1 cites q4 only, no Postgres text)"
else
  echo "superseded-answer: FAILED (cycle exit $code, or q3 not superseded, or r1 still cites q3 or names Postgres)"
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
  SDC_LLM_ADAPTER="$sdc/bindings/tools/llm-adapter-fixture.sh"
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
   grep -q '3 traced to behaviour, 0 constraint, 0 dropped, 0 untraced' <<< "$(tracelog trace-gap | tail -n 1)"; then
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
   grep -q '1 dropped (r3), 0 untraced' <<< "$(tracelog trace-gap-drop | tail -n 1)"; then
  echo "trace-gap-drop: checks pass (the gap went to a person after one send-back; r3 dropped by the person)"
else
  echo "trace-gap-drop: FAILED (cycle exit $code, or the gap was waved through, or r3 not dropped)"
fi
# trace-constraint: r3 is a retention constraint with its reason (fixtures/trace-constraint): it
# passes the trace untraced and is counted. Passes when the cycle ends operating with no trace finding.
cycle trace-constraint SDC_STUB_FIXTURES="$fx/trace-constraint"
code=$?
if (( code == 0 )) && ! grep -q 'p-trace' "$s/store-trace-constraint/IDEA-1/work/findings.yaml" 2>/dev/null &&
   grep -q '2 traced to behaviour, 1 constraint (r3), 0 dropped, 0 untraced' <<< "$(tracelog trace-constraint)"; then
  echo "trace-constraint: checks pass (r3 a constraint with its reason: passes, counted)"
else
  echo "trace-constraint: FAILED (cycle exit $code, or the constraint was not counted)"
fi
# trace-constraint-decided: runs/trial-3-small-sonnet-1. As trace-gap-drop, but the person answers
# in the decider's words of that run, the constraint option with more text after it. Passes when the
# cycle ends operating, r3 is `behaviour: constraint` and counted as one, and the agent's first answer
# to p-trace-r3 ("specify r3 in the spec") is superseded by the person's, so r3 cites only the latter.
said="r3 is a constraint, not behaviour: mark r3 as a constraint outside the specification."
cycle trace-constraint-decided SDC_STUB_FIXTURES="$fx/trace-gap-drop" FINDING_DECISION="$said"
code=$?
w=$s/store-trace-constraint-decided/IDEA-1/work
if (( code == 0 )) && yq -e '.requirements[] | select(.id == "r3") | .behaviour == "constraint"' "$w/requirements.yaml" >/dev/null &&
   grep -q '1 constraint (r3), 0 dropped, 0 untraced' <<< "$(tracelog trace-constraint-decided | tail -n 1)" &&
   yq -o=json "$w/questions.yaml" | jq -e --slurpfile r <(yq -o=json "$w/requirements.yaml") '
     [.questions[] | select(.item == "p-trace-r3")] as $qs
     | ($qs | length) == 2 and $qs[0].status == "superseded" and $qs[0].superseded_by == $qs[1].id
       and ($r[0].requirements[] | select(.id == "r3") | (.cites | index($qs[0].id)) == null and (.cites | index($qs[1].id)) != null)' >/dev/null; then
  echo "trace-constraint-decided: checks pass (the constraint answer applied, r3 counted a constraint; the send-back answer superseded)"
else
  echo "trace-constraint-decided: FAILED (cycle exit $code, or r3 not a constraint, or the earlier answer not superseded)"
fi

# change.review as a model call (story:review-node-is-a-model): the review goes through llm-node.sh
# on the fixture adapter (scripted replies, SDC_REVIEW_REPLIES, no LLM) and the program check
# (sdc-reply check-review); every other node runs its stub.
# reviews <cycle>: the story case's code_review records, oldest first, as {result, producer, log}.
reviews() { local d=$s/store-$1/${story%/events.jsonl}
  for e in "$d"/evidence/*.json; do jq -c --slurpfile ev <(jq -c 'select(.event == "action_finished")' "$d/events.jsonl") '
    select(.kind == "code_review") | .id as $id | {result, producer,
      log: ([$ev[] | select(.evidence | index($id)) | .log] | first)}' "$e"; done | jq -s .; }
review_env=(SDC_LLM_NODES=change.review SDC_LLM_ADAPTER="$sdc/bindings/tools/llm-adapter-fixture.sh")
rej='{"decision": "rejected", "summary": "fixture: the README does not say how to run it", "findings": [{"kind": "violation", "at": "impl/README.md:1", "violates": "r1", "says": "fixture: the README names no command to run the service"}]}'
app='{"decision": "approved", "summary": "fixture: the rework addresses the finding", "findings": []}'
# review-reject-once: rejected once with a finding citing impl/README.md:1 and r1, then approved.
# Passes when the cycle ends operating with two code_review records by llm-review, rejected then
# approved, the rejection's log an sdc.rejection/1 citing that line and r1, and change.implement
# run twice, the second time sent back by the review with the rejection (the stub's impl/.rework).
cycle review-reject-once "${review_env[@]}" SDC_REVIEW_REPLIES="$(jq -cn --argjson j "$rej" --argjson a "$app" '{rounds: [$j, $a]}')"
code=$?
d=$s/store-review-reject-once/${story%/events.jsonl}
rv=$(reviews review-reject-once)
lg=$(jq -r '.[0].log // ""' <<< "$rv")
# D138: the second change.review call is a fresh review, not a rework: no in/rejection.json in its
# cwd and no "Rework." line in its prompt.
r2=$(jq -rs '[.[] | select(.action == "change.review")] | .[1].record // empty' "$s/store-review-reject-once/IDEA-1/work/.sdc/llm/calls.jsonl" 2>/dev/null)
if (( code == 0 )) && jq -e '[.[] | .result] == ["rejected", "approved"] and all(.producer == "llm-review")' <<< "$rv" >/dev/null &&
   jq -e '.format == "sdc.rejection/1" and .approval == "change.review" and (.principal | startswith("llm-review:"))
     and .citations == [{"at": "impl/README.md:1", "violates": "r1", "kind": "violation", "in_diff": true}] and .count == 1' "$d/blobs/$lg" >/dev/null &&
   jq -se '[.[] | select(.event == "action_finished" and .action == "change.implement")] | length == 2' "$d/events.jsonl" >/dev/null &&
   jq -se '[.[] | select(.event == "action_started" and .action == "change.implement")] | last | .retry_of == ["code_review"]' "$d/events.jsonl" >/dev/null &&
   grep -q 'impl/README.md:1' "$d/work/impl/.rework" &&
   [[ -n $r2 && -f $r2/prompt.md && ! -e $r2/cwd/in/rejection.json ]] && ! grep -q '^Rework\.' "$r2/prompt.md"; then
  echo "review-reject-once: checks pass (rejected with a cited finding, reworked with the rejection, approved, both by llm-review; the second review got no rejection)"
else
  echo "review-reject-once: FAILED (cycle exit $code: $(jq -c 'map({result, producer})' <<< "$rv" 2>/dev/null); second review: ${r2:-no record}, in/rejection.json $([[ -n $r2 && -e $r2/cwd/in/rejection.json ]] && echo present || echo absent), Rework. lines $(grep -c '^Rework\.' "$r2/prompt.md" 2>/dev/null || echo 0))"
fi
# review-minimal-input (story:review-minimal-facts, SDC_REVIEW_MODE=facts): review-reject-once again
# with the switch on (review-facts-once, the same replies); its reviews got one
# document, no tools and no whole tree; and on that story's tree, with a file the change does not
# touch added to impl/ and mainline/ (a copy of that store), `sdc-reply review-inputs` leaves it out. Passes when both
# review calls offered no operation, took 4 turns at most, recorded the document's size
# (`input_bytes`, the bytes of in/review.md), pasted it into the prompt, and have no tests.txt,
# gate.txt, commands.json or whole `impl/` in `in/`; and the untouched file's text is in neither the
# document nor `in/impl/`, while the changed README (its diff and its text) is, and the
# requirement ids of scope.json are all in the document. The untouched files are in the document's
# listing (path and size; `server.js` with its items `serve` and `GET /health`), a service worker the
# tree lacks is not; `check-review` takes a finding at `impl/server.js` without a line (a listed
# file, citation `listed: true`) and refuses `impl/server.js:1` and `impl/sw.js`.
cycle review-facts-once "${review_env[@]}" SDC_REVIEW_MODE=facts SDC_REVIEW_REPLIES="$(jq -cn --argjson j "$rej" --argjson a "$app" '{rounds: [$j, $a]}')"
calls_log=$s/store-review-facts-once/IDEA-1/work/.sdc/llm/calls.jsonl
rv_calls=$(jq -sc '[.[] | select(.action == "change.review")]' "$calls_log" 2>/dev/null)
rm -rf "$s/minimal-input"; mkdir -p "$s/minimal-input"
cp -a "$s/store-review-facts-once" "$s/minimal-input/store"; sc=$s/minimal-input/store/${story%/events.jsonl}
echo "UNTOUCHED_MARKER" > "$sc/work/impl/UNTOUCHED.txt"; echo "UNTOUCHED_MARKER" > "$sc/work/mainline/UNTOUCHED.txt"
echo "a line the review must see" >> "$sc/work/impl/README.md"
printf 'function serve() {}\napp.get("/health", serve);\n' > "$sc/work/impl/server.js"; cp "$sc/work/impl/server.js" "$sc/work/mainline/server.js"
mi=$(SDC_REVIEW_MODE=facts "$sdc/driver/target/debug/sdc-reply" review-inputs --cwd "$s/minimal-input/call" --case-dir "$sc" --story invoices 2>&1)
mci=$s/minimal-input/call/in
cite() { jq -cn --arg at "$1" --arg v "$(jq -r '.requirements[0].id' "$mci/scope.json")" \
  '{decision: "rejected", summary: "s", findings: [{kind: "violation", at: $at, violates: $v, says: "fixture: absent"}]}' > "$s/minimal-input/reply.json"
  "$sdc/driver/target/debug/sdc-reply" check-review --cwd "$s/minimal-input/call" --case-dir "$sc" --reply "$s/minimal-input/reply.json" --principal p 2>&1; }
cite_listed=$(cite impl/server.js); cite_listed_code=$?
cite_line=$(cite impl/server.js:1); cite_line_code=$?
cite_ghost=$(cite impl/sw.js); cite_ghost_code=$?
first=$(jq -r '.[0].record' <<< "$rv_calls")
if jq -e 'length == 2 and all(.allowed_operations == [] and .max_turns <= 4 and (.input_bytes | type == "number") and .input_bytes > 0)' <<< "$rv_calls" >/dev/null &&
   [[ -n $first && $(jq -r '.[0].input_bytes' <<< "$rv_calls") == $(stat -c %s "$first/cwd/in/review.md") ]] &&
   [[ $(grep -c '^# Review input: story' "$first/prompt.md") == 1 ]] &&
   [[ ! -e $first/cwd/in/tests.txt && ! -e $first/cwd/in/gate.txt && ! -e $first/cwd/in/commands.json ]] &&
   [[ -f $first/cwd/in/impl/README.md ]] &&
   ! grep -rq UNTOUCHED_MARKER "$mci" && [[ ! -e $mci/impl/UNTOUCHED.txt ]] &&
   grep -q 'a line the review must see' "$mci/review.md" &&
   jq -e '.changed_files == ["README.md"] and .listed_files >= 3' <<< "$mi" >/dev/null &&
   grep -q '^## Product files not in the diff' "$mci/review.md" && grep -q '^- UNTOUCHED.txt (17 bytes)$' "$mci/review.md" &&
   grep -q '^- server.js (.* bytes): serve, GET /health$' "$mci/review.md" && ! grep -q 'sw.js' "$mci/review.md" &&
   [[ ! -e $mci/impl/server.js ]] && (( cite_listed_code == 1 && cite_line_code == 2 && cite_ghost_code == 2 )) &&
   jq -e '.citations == [{"at": "impl/server.js", "violates": .citations[0].violates, "kind": "violation", "in_diff": false, "listed": true}]' <<< "$cite_listed" >/dev/null &&
   grep -q 'not a line of the change tree' <<< "$cite_line" && grep -q 'neither a listed product file nor a changed file' <<< "$cite_ghost" &&
   [[ -z $(jq -r '.requirements[].id' "$mci/scope.json" | while read -r id; do grep -q "^- $id: " "$mci/review.md" || echo "$id"; done) ]]; then
  echo "review-minimal-input: checks pass (no tools, one document of $(jq -r '.[0].input_bytes' <<< "$rv_calls") bytes in the prompt; an untouched file is listed, not shown; a listed file is cited by path, an absent one refused)"
else
  echo "review-minimal-input: FAILED (calls: $(jq -c 'map({allowed_operations, max_turns, input_bytes})' <<< "$rv_calls" 2>/dev/null); inputs: $mi; listed cite $cite_listed_code, line cite $cite_line_code, ghost cite $cite_ghost_code)"
fi
# review-invalid: every reply rejects with a finding at a line the README does not have. Passes when
# the reply was asked again twice with the check's reason (SDC_REVIEW_RETRIES), the one change.review
# run exits 2 and records nothing (an observation runs once per case snapshot), the run ends stuck
# (exit 1), and the check's reason names the line: never an approval, never a reasonless rejection.
bad='{"decision": "rejected", "summary": "fixture", "findings": [{"kind": "violation", "at": "impl/README.md:9999", "violates": "r1", "says": "fixture: a line that does not exist"}]}'
cycle review-invalid "${review_env[@]}" SDC_REVIEW_REPLIES="$(jq -cn --argjson j "$bad" '{rounds: [$j]}')"
code=$?
d=$s/store-review-invalid/${story%/events.jsonl}
rr=$(ls -d "$s"/store-review-invalid/IDEA-1/work/.sdc/llm/*-change.review 2>/dev/null | tail -n 1)
if (( code == 1 )) && jq -e 'length == 0' <<< "$(reviews review-invalid)" >/dev/null &&
   jq -se '[.[] | select(.event == "action_finished" and .action == "change.review") | .exit] == ["2"]' "$d/events.jsonl" >/dev/null &&
   [[ -f $rr/reply.try2.json ]] && grep -q 'refused by the program check: invalid review reply' "$rr/prompt.md" &&
   grep -q 'lines 9999-9999 outside impl/README.md' "$rr/review-check.err"; then
  echo "review-invalid: checks pass (a rejection citing no line of the change: refused, nothing recorded, never approved)"
else
  echo "review-invalid: FAILED (cycle exit $code, or a review was recorded)"
fi
# review-spent: every review rejects with the valid finding. Passes when the run ends writer_spent on
# change.implement with 3 runs, each sent back by change.review (exit 1), three rejected records.
cycle review-spent "${review_env[@]}" SDC_REVIEW_REPLIES="$(jq -cn --argjson j "$rej" '{rounds: [$j]}')"
code=$?
if (( code == 1 )) && jq -se '(map(select(.event | test("^(stuck|awaiting_person|writer_spent)$"))) | last)
     | .event == "writer_spent" and .spent[0].action == "change.implement" and .spent[0].runs == 3
       and ([.spent[0].sent_back[].by] == ["change.review", "change.review", "change.review"])' \
     "$s/store-review-spent/$story" >/dev/null &&
   jq -e '[.[] | .result] == ["rejected", "rejected", "rejected"]' <<< "$(reviews review-spent)" >/dev/null; then
  echo "review-spent: checks pass (three rejections: writer_spent on change.implement, sent back by change.review)"
else
  echo "review-spent: FAILED (cycle exit $code, or the stop is not writer_spent sent back by change.review)"
fi

# --- decider cycles: scripts/run-trial.sh with SDC_DECIDER=model, every node on its stub and the
# decider on tools/llm-adapter-fixture.sh (scripted replies, SDC_DECIDER_REPLIES), no LLM ---------
fixture=$sdc/bindings/tools/llm-adapter-fixture.sh
targs=()
# run.unstick is off unless a cycle turns it on (SDC_UNSTICK=on): the stops these cycles assert
# are today's. SDC_RULE_APPROVALS is off by default (on only in the rule-approvals cycles); the
# cycles that assert a decider call on context.approve, roadmap.approve or release.publish also set
# it off explicitly, so they hold whatever the default: autopilot (its
# release.publish and roadmap.approve requests feed decider-later-obligation and decider-input),
# decider-cite-address, decider-cite-retry, decider-invalid-approval, decider-escalate-nobest,
# decider-reject, decider-rework-context, decider-rework-model, decider-release-withheld and
# rework-spent-abandoned.
trial() { # <name> [env ...]: a fresh run-trial.sh run into <scratch>/runs/<name>, with the arguments in targs
  local name=$1; shift
  rm -rf "$s/runs/$name"
  env SDC_RUNS_DIR="$s/runs" SDC_WATCH=0 SDC_LLM_ADAPTER=stub SDC_DECIDER=model SDC_DECIDER_ADAPTER="$fixture" SDC_UNSTICK=off "$@" \
    "$sdc/scripts/run-trial.sh" "$name" "$(yq -r .text "$fx/idea.yaml")" "${targs[@]}" > "$s/$name.txt" 2>&1
  local code=$?
  echo "$name: exit $code ($(tail -n 1 "$s/$name.txt" | cut -c1-160))"
  return $code
}
# decider-scope-answer: runs/trial-2b-fresh's first stop. Two scope-changing questions are open at
# scope.answer (fixtures/scope-two); asked to approve scope.answer as a whole, the decider escalates
# (the recorded reply: it read questions.settled false as a failed check). Each question goes to
# the decider instead: q2 by its default, q3 with a free answer; the answers are written as a
# person's are, scope.answer is taken as decider:model and intake closes. Passes when the run ends
# 0, q2 and q3 carry decider:model (accepted, changed), decisions.jsonl has both as class
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
     (.id == "q2" and .status == "accepted" and .answer == .default and .answered_by == "decider:model")
     or (.id == "q3" and .status == "changed" and .answer == $f and .answered_by == "decider:model"))] | length == 2' >/dev/null &&
   jq -se '[.[] | select(.class == "scope_changing_answer" and .decision == "approve")] | map(.question) == ["q2", "q3"]' "$r/decisions.jsonl" >/dev/null &&
   jq -e '.claims["questions.settled"].settled_by_this_decision == true' "$(ls -d "$r"/decider/*-q2 | head -n 1)/cwd/in/checks.json" >/dev/null; then
  echo "decider-scope-answer: checks pass (q2 default, q3 free answer, both decider:model; class scope_changing_answer; questions.settled marked)"
else
  echo "decider-scope-answer: FAILED (run exit $code, or the answers, their records or the claim marks are wrong)"
fi
# decider-model-name (D149): the strong runs of trial 3 named an Opus decider decider:haiku and
# printed "decider haiku (claude-opus-5-5)". The LLM decider is decider:model whatever its model;
# SDC_DECIDER=haiku is its old name. decider-scope-answer under SDC_DECIDER=haiku with the decider
# model set to claude-opus-5-5. Passes when the run closes, the start line names "decider model
# (claude-opus-5-5)" and no "decider haiku", run-settings.jsonl records kind model with that model,
# every decisions.jsonl line and both answers carry decider:model, and each decision's log line
# names "decider:model (claude-opus-5-5)".
trial decider-model-name SDC_DECIDER=haiku SDC_LLM_MODEL_DECIDER=claude-opus-5-5 SDC_STUB_FIXTURES="$fx/scope-two" \
  SDC_DECIDER_REPLIES="$(jq -cn --arg f "$free" '{
  "scope.answer": {decision: "escalate", answer: null, best: null, cites: ["q2", "q3", "questions.settled"],
    reasons: ["Questions q2 and q3 are scope-changing and still open."]},
  q3: {decision: "approve", answer: $f, best: null, cites: ["d5", "d2"], reasons: ["The clerk who issued an invoice needs to see what the customer was sent."]}}')"
code=$?
r=$s/runs/decider-model-name
if (( code == 0 )) &&
   grep -q '^started decider-model-name: .*decider model (claude-opus-5-5),' "$s/decider-model-name.txt" &&
   ! grep -q 'decider haiku\|decider:haiku' "$s/decider-model-name.txt" &&
   jq -se 'all(.decider.kind == "model" and .decider.model == "claude-opus-5-5")' "$r/run-settings.jsonl" >/dev/null &&
   jq -se 'length >= 3 and all(.principal == "decider:model")' "$r/decisions.jsonl" >/dev/null &&
   yq -o=json "$r/store/IDEA-1/work/questions.yaml" | jq -e '[.questions[] | select(.scope_changing == true) | .answered_by] == ["decider:model", "decider:model"]' >/dev/null &&
   grep -qF '  decider:model (claude-opus-5-5) approve IDEA-1.intake|scope.answer|q3|' "$s/decider-model-name.txt"; then
  echo "decider-model-name: checks pass (SDC_DECIDER=haiku runs as decider:model (claude-opus-5-5): start line, run-settings, decisions, answers, log)"
else
  echo "decider-model-name: FAILED (run exit $code, or the decider is still named haiku somewhere, or the model id is missing)"
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
   yq -o=json "$q" | jq -e '.questions[] | select(.id == "q3") | .status == "accepted" and .answer == .default and .answered_by == "decider:model"' >/dev/null &&
   jq -se '[.[] | select(.question == "q3") | .decision] == ["escalation_suppressed", "approve"]' "$r/decisions.jsonl" >/dev/null; then
  echo "decider-scope-final: checks pass (q3 escalated, suppressed, its default taken)"
else
  echo "decider-scope-final: FAILED (run exit $code, or q3 not at its default, or no escalation_suppressed)"
fi
# decider-finding: trial 2's f1/f9/f16 stops ("nothing for decider:haiku at amend.upstream"). The
# spec stage's scope-changing finding (fixtures/finding-spec-person.yaml) is put to the decider as
# a decision request; it picks option 2. Passes when the run ends 0, the finding is resolved with
# that option and `decided_by: decider:model`, the amendment's question is answered by
# decider:model, and decisions.jsonl records it with class finding.
opt2="the customer and the billing clerk"
trial decider-finding SDC_STUB_FINDINGS="$fx/finding-spec-person.yaml" SDC_DECIDER_REPLIES="$(jq -cn --arg o "$opt2" '{
  f1: {decision: "approve", answer: $o, best: null, cites: ["r2"], reasons: ["The clerk who issues an invoice is the one asked about it, so the clerk gets the email too."]}}')"
code=$?
r=$s/runs/decider-finding
f=$r/store/IDEA-1/work/findings.yaml
if (( code == 0 )) &&
   yq -o=json "$f" | jq -e --arg o "$opt2" '.findings[] | select(.id == "f1") | .status == "resolved" and .decision == $o and .decided_by == "decider:model"' >/dev/null &&
   yq -o=json "$(dirname "$f")/questions.yaml" | jq -e --arg o "$opt2" 'any(.questions[]; .item == "f1" and .answer == $o and .answered_by == "decider:model")' >/dev/null &&
   jq -se '[.[] | select(.class == "finding")] | length == 1 and .[0].finding == "f1" and .[0].decision == "approve"' "$r/decisions.jsonl" >/dev/null; then
  echo "decider-finding: checks pass (f1 option 2 by decider:model, amended, class finding)"
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
   yq -o=json "$f" | jq -e '.findings[] | select(.id == "f1") | .status == "resolved" and .decision == .default and .decided_by == "decider:model"' >/dev/null &&
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
     | .status == "needs_person" and (.decision // "") == "" and .refused_decision.decided_by == "decider:model"
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
if jq -e '.value >= 1 and .failure == true and .decider == "model"' <<< "$l7" >/dev/null &&
   grep -q '^L7_waiting_on_person_seconds: FAILED ' <<< "$("$kpis" "$r")"; then
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
trial autopilot SDC_RULE_APPROVALS=off SDC_STUB_FIXTURES="$fx/scope-two" SDC_STUB_FINDINGS="$fx/finding-spec-person.yaml"
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
   jq -se 'all(.autonomy == "A5" and .decider.escalate == "final" and .decider.kind == "model")' "$r/run-settings.jsonl" >/dev/null &&
   yq -e '.findings[] | select(.id == "f1") | .decided_by == "decider:model"' "$w/findings.yaml" >/dev/null &&
   [[ -d $w/aep ]] && ! grep -rqs 'human:' "$w/aep" && grep -rqs 'actor: "agent:' "$w/aep"; then
  echo "autopilot: checks pass (closed; 0 person principals; L7 0; A5, escalate final; work/aep transitions by agent:, no human:)"
else
  echo "autopilot: FAILED (run exit $code, $people person principal(s), L7 $l7, the settings do not name A5, or work/aep has a human: actor or no agent: one)"
fi
# trace-reraised: runs/trial-3-small-sonnet-1 (17 of 20 decider calls on p-trace-r5 and p-trace-r7).
# The writer never specifies r3 (fixtures/trace-gap-drop); the decider answers p-trace-r3 "specify
# r3 in the spec", so its decision is applied (the writer sent back) without effect. Passes when the
# run stops finding_reraised (exit 7) by rule:autopilot with a reason naming p-trace-r3 and the
# decision, after one decider call on p-trace-r3, not decider_budget_spent.
specify="specify r3 in the spec: name in spec/traces.json the commands or views that deliver it"
targs=(--autopilot)
trial trace-reraised SDC_STUB_FIXTURES="$fx/trace-gap-drop" SDC_DECIDER_REPLIES="$(jq -cn --arg a "$specify" '{"p-trace-r3":
  {decision: "approve", answer: $a, reasons: ["fixture: the writer can name the views that deliver r3"], cites: ["r3"], best: null}}')"
code=$?
r=$s/runs/trace-reraised
if (( code == 7 )) && jq -e '.cause == "finding_reraised" and .by == "rule:autopilot"
     and (.reason | test("finding_reraised: p-trace-r3 raised again after decider:model.s decision \"specify r3"))' "$r/run-stopped.json" >/dev/null &&
   [[ $(jq -s '[.[] | select(.finding == "p-trace-r3" and .decision == "approve")] | length' "$r/decisions.jsonl") == 1 ]]; then
  echo "trace-reraised: checks pass (stopped finding_reraised on the second raise, naming p-trace-r3 and the decision; one decider call)"
else
  echo "trace-reraised: FAILED (exit $code: $(jq -c '{cause, reason}' "$r/run-stopped.json" 2>/dev/null | cut -c1-300))"
fi
# decider-budget-reason: the stop record of decider_budget_spent names the step and the cap
# (runs/trial-3-small-sonnet-1's record said only "autopilot: "). SDC_MAX_DECIDER_CALLS=1 on two open
# scope-changing questions: the second has no call left.
trial decider-budget-reason SDC_STUB_FIXTURES="$fx/scope-two" SDC_MAX_DECIDER_CALLS=1
code=$?
r=$s/runs/decider-budget-reason
if (( code != 0 )) && jq -e '.cause == "decider_budget_spent"
     and (.reason | test("^autopilot: scope.answer in IDEA-1.intake: decider_budget_spent \\(1 LLM decider calls"))' "$r/run-stopped.json" >/dev/null; then
  echo "decider-budget-reason: checks pass (the stop reason names scope.answer in IDEA-1.intake and the cap)"
else
  echo "decider-budget-reason: FAILED (exit $code: $(jq -c '{cause, reason}' "$r/run-stopped.json" 2>/dev/null | cut -c1-300))"
fi
targs=(--autopilot)
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
   ! grep -qi 'person' <<< "$(sed -n '/^round /,$p' "$s/autopilot-stop.txt" | sed 's/awaiting_person//g')" &&
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
trial decider-cite-address SDC_RULE_APPROVALS=off SDC_DECIDER_REPLIES="$(jq -cn --argjson g "$good" '{"context.approve": $g}')"
code=$?
a=$(attempts "$s/runs/decider-cite-address" context.approve)
if (( code == 0 )) && jq -e 'length == 1 and .[0].decision == "approve" and .[0].valid == true
     and ([.[0].citations[].form] == ["artifact_revision", "path_line", "id"])' <<< "$a" >/dev/null; then
  echo "decider-cite-address: checks pass (an entry and lines of context.yaml resolve; approved on the first reply)"
else
  echo "decider-cite-address: FAILED (exit $code: $(jq -c '[.[] | {decision, invalid_reason}]' <<< "$a" 2>/dev/null))"
fi
trial decider-cite-retry SDC_RULE_APPROVALS=off SDC_DECIDER_REPLIES="$(jq -cn --argjson b "$bad" --argjson g "$good" '{"context.approve": [$b, $g]}')"
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
trial decider-invalid-approval SDC_RULE_APPROVALS=off SDC_DECIDER_REPLIES="$(jq -cn --argjson b "$bad" '{"context.approve": [$b, $b, $b]}')"
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
   yq -o=json "$r/store/IDEA-1/work/questions.yaml" | jq -e '.questions[] | select(.id == "q2") | .answer == .default and .answered_by == "decider:model"' >/dev/null; then
  echo "decider-invalid-scope: checks pass (3 invalid replies to q2, its default taken as invalid_reply_default)"
else
  echo "decider-invalid-scope: FAILED (exit $code: $(jq -c '[.[] | {decision, fallback}]' <<< "$a" 2>/dev/null))"
fi
# escalate-nobest: under autopilot the decider is final: an escalate without best is refused and asked again.
esc='{"decision": "escalate", "answer": null, "reasons": ["fixture: a person should say"], "cites": ["context.kind"], "best": null}'
trial decider-escalate-nobest SDC_RULE_APPROVALS=off SDC_DECIDER_REPLIES="$(jq -cn --argjson e "$esc" --argjson g "$good" '{"context.approve": [$e, $g]}')"
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
trial decider-reject SDC_RULE_APPROVALS=off SDC_DECIDER_REPLIES='{"context.approve": {"decision": "reject", "answer": null, "reasons": ["fixture: the context is not usable"], "cites": ["context.platform"], "best": null}}'
code=$?
r=$s/runs/decider-reject
if (( code == 6 )) && jq -e '.by == "rule:autopilot" and .cause == "rejected_without_rework_path" and .case == "IDEA-1.direction"
     and (.reason | test("^autopilot: context.approve in IDEA-1.direction: ") and (test("person") | not) and (test("/home/|/tmp/") | not))' "$r/run-stopped.json" >/dev/null &&
   ! grep -qi 'person' <<< "$(sed -n '/^round /,$p' "$s/decider-reject.txt" | sed 's/awaiting_person//g')" &&
   "$kpis" "$r" --json | jq -e '.L7_waiting_on_person_seconds | .value == 0 and .failure == false' >/dev/null; then
  echo "decider-reject: checks pass (exit 6, rejected_without_rework_path, step and case named, no person, L7 0)"
else
  echo "decider-reject: FAILED (exit $code, or the stop record: $(jq -c '{cause, reason}' "$r/run-stopped.json" 2>/dev/null))"
fi
else
# decider-rework-context: context.approve rejected every time: context.default reworks the context
# twice with the rejection (the stub appends it to context.yaml as a comment), context.check runs
# on each new revision, the third rejection stops by rule:autopilot with cause rework_spent.
trial decider-rework-context SDC_RULE_APPROVALS=off SDC_DECIDER_REPLIES='{"context.approve": {"decision": "reject", "answer": null, "reasons": ["fixture: the context is not usable"], "cites": ["context.platform"], "best": null}}'
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
trial decider-rework-model SDC_RULE_APPROVALS=off SDC_LLM_ADAPTER="$fixture" SDC_LLM_NODES=context.default \
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
  # run-metrics: sdc-run-metrics on decider-rework's run (docs/workflow-metrics.md). The story
  # case's change.implement was taken twice, the second time sent back by the rejected
  # merge.approve: looping factor 2, one approval edge, merge.approve asked twice (one reject);
  # the run closed clean.
  m=$("$metrics" "$r" --json - 2>/dev/null)
  if (( code == 0 )) && jq -e '([.cases[] | select(.case | endswith(".invoices")) | .nodes[] | select(.action == "change.implement")] | first
       | .real_takes == 2 and .looping_factor == 2 and .first_pass_yield == 0 and .retries_by_cause == {"merge.approve:rejected": 1})
     and ([.edges[] | select(.edge == "story: merge.approve:rejected -> change.implement")] | length == 1 and .[0].kind == "approval" and .[0].count == 1)
     and (.decisions.by_action["approval/merge.approve"] | .asked == 2 and .rejected == 1 and .approved == 1)
     and .metrics.end == "closed: operating" and .metrics.clean == true' <<< "$m" >/dev/null; then
    echo "run-metrics: checks pass (change.implement LF 2 sent back by merge.approve:rejected, one approval edge, merge.approve asked 2 rejected 1, closed clean)"
  else
    echo "run-metrics: FAILED ($(jq -c '{end: .metrics.end, clean: .metrics.clean, edges: [.edges[] | {edge, count}]}' <<< "$m" 2>/dev/null | cut -c1-300))"
  fi
  trial decider-rework-spent SDC_DECIDER_REPLIES="$(jq -cn --argjson j "$rej" '{"merge.approve": $j}')"
  code=$?
  r=$s/runs/decider-rework-spent
  if (( code == 6 )) && jq -e '.by == "rule:autopilot" and .cause == "rework_spent"
       and (.reason | test("^autopilot: merge.approve in IDEA-1[.][a-z]+[.][a-z]+: rework_spent") and test("1: .*2: .*3: ")
         and (test("person") | not) and (test("/home/|/tmp/") | not))' "$r/run-stopped.json" >/dev/null &&
     ! grep -qi 'person' <<< "$(sed -n '/^round /,$p' "$s/decider-rework-spent.txt" | sed 's/awaiting_person//g')"; then
    echo "decider-rework-spent: checks pass (three rejections: rework_spent by rule:autopilot, the three reasons named)"
  else
    echo "decider-rework-spent: FAILED (exit $code, or the stop record: $(jq -c '{cause, reason}' "$r/run-stopped.json" 2>/dev/null))"
  fi
  # rework-cap (D184): one effective rework cap. rule-ends.json's rework_limit (2) is the default
  # and SDC_MAX_REJECTIONS overrides it; run-settings.jsonl records the value and its source, and
  # both the driver and the writer's rework prompt read it there. Before, the driver read
  # rule-ends.json and the prompt said "of at most 2", whatever the setting. Runs: decider-rework-spent
  # above (the default); merge.approve rejected every time under SDC_MAX_REJECTIONS=1, which must
  # stop rework_spent at the second rejection after one rework; the same on the fixture adapter for
  # change.implement under SDC_MAX_REJECTIONS=5, whose rework prompt must say "of at most 5"; and
  # SDC_MAX_REJECTIONS=x, which is refused before anything starts (exit 64).
  trial rework-cap SDC_MAX_REJECTIONS=1 SDC_DECIDER_REPLIES="$(jq -cn --argjson j "$rej" '{"merge.approve": $j}')"
  code=$?
  r=$s/runs/rework-cap
  st=$(find "$r/store" -type d -name 'IDEA-1.*.invoices' | head -n 1)
  trial rework-cap-prompt SDC_MAX_REJECTIONS=5 SDC_LLM_ADAPTER="$fixture" SDC_LLM_NODES=change.implement \
    SDC_DECIDER_REPLIES="$(jq -cn --argjson j "$rej" '{"merge.approve": $j}')"
  pr=$(jq -rs '[.[] | select(.action == "change.implement")] | last | .record // empty' "$s/runs/rework-cap-prompt/llm-calls.jsonl" 2>/dev/null)
  trial rework-cap-bad SDC_MAX_REJECTIONS=x
  bad=$?
  if (( code == 6 )) && [[ -n $st ]] &&
     jq -se 'last.rework_limit == {"value": 2, "source": "rule-ends.json"}' "$s/runs/decider-rework-spent/run-settings.jsonl" >/dev/null &&
     jq -se 'all(.rework_limit == {"value": 1, "source": "SDC_MAX_REJECTIONS"})' "$r/run-settings.jsonl" >/dev/null &&
     jq -e '.cause == "rework_spent" and (.reason | test("rejected .* 2 times: 1: .*2: ") and (test("3: ") | not))' "$r/run-stopped.json" >/dev/null &&
     jq -se '[.[] | select(.event == "action_started" and .action == "change.implement" and .retry_of == ["merge_approval"])] | length == 1' "$st/events.jsonl" >/dev/null &&
     grep -q 'rejection 2; at most 1 are reworked' "$s/rework-cap.txt" &&
     [[ -n $pr ]] && grep -q '(rejection 1 of at most 5)' "$pr/prompt.md" &&
     jq -se 'last.rework_limit == {"value": 5, "source": "SDC_MAX_REJECTIONS"}' "$s/runs/rework-cap-prompt/run-settings.jsonl" >/dev/null &&
     (( bad == 64 )) && grep -q "SDC_MAX_REJECTIONS: the rework cap is a whole number" "$s/rework-cap-bad.txt"; then
    echo "rework-cap: checks pass (default 2 from rule-ends.json, SDC_MAX_REJECTIONS=1 stops at the second rejection after one rework, the prompt says of at most 5 under 5, x refused; each recorded in run-settings)"
  else
    echo "rework-cap: FAILED (exit $code, bad $bad, prompt ${pr:-none}, or the stop record: $(jq -c '{cause, reason}' "$r/run-stopped.json" 2>/dev/null | cut -c1-300))"
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
      "$sdc/bindings/tools/llm-node.sh" change.implement 2>&1)
    pc=$?
  fi
  if [[ -n $st ]] && (( pc == 0 )) && ! grep -q 'not called' <<< "$out" &&
     jq -e '.format == "sdc.rejection/1" and .approval == "merge.approve"' "$(ls -d "$p"/llm/*-change.implement 2>/dev/null | tail -n 1)/cwd/in/rejection.json" >/dev/null 2>&1; then
    echo "rework-pending-rejection: checks pass (a pending rejection: the writer called with in/rejection.json, the own-default skip not taken)"
  else
    echo "rework-pending-rejection: FAILED (exit ${pc:-none}: $(tail -n 1 <<< "${out:-}" | cut -c1-200))"
  fi
  # review-no-rework-prompt (ruling D138): as decider-rework (merge.approve rejected once, then
  # approved) with change.review on the fixture adapter. The review after the rejection is a review,
  # not a rework: passes when its call record has no in/rejection.json in its cwd and no "Rework."
  # line in its prompt (before D138 it got the merge.approve rejection as rework).
  trial review-no-rework-prompt SDC_LLM_ADAPTER="$fixture" SDC_LLM_NODES=change.review \
    SDC_DECIDER_REPLIES="$(jq -cn --argjson j "$rej" --argjson a "$app" '{"merge.approve": {"rounds": [$j, $a]}}')"
  code=$?
  r=$s/runs/review-no-rework-prompt
  r2=$(jq -rs '[.[] | select(.action == "change.review")] | .[1].record // empty' "$r/llm-calls.jsonl" 2>/dev/null)
  if (( code == 0 )) && jq -e '[.[] | .decision] == ["reject", "approve"]' <<< "$(attempts "$r" merge.approve)" >/dev/null &&
     [[ -n $r2 && -f $r2/prompt.md && ! -e $r2/cwd/in/rejection.json ]] && ! grep -q '^Rework\.' "$r2/prompt.md"; then
    echo "review-no-rework-prompt: checks pass (merge.approve rejected, then approved; the review after the rejection got no in/rejection.json and no Rework. line)"
  else
    echo "review-no-rework-prompt: FAILED (exit $code; second review: ${r2:-no record}, in/rejection.json $([[ -n $r2 && -e $r2/cwd/in/rejection.json ]] && echo present || echo absent), Rework. lines $(grep -c '^Rework\.' "$r2/prompt.md" 2>/dev/null || echo 0))"
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
"$sdc/driver/target/debug/sdc-drive" --canon "$canon" --case-dir "$root" replay > "$s/replay-sibling.json" 2>&1
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
  wo=$("$sdc/driver/target/debug/sdc-drive" --canon "$canon" --case-dir "$w/IDEA-1" \
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
# decider:model, no refused_decision) and the run closes (before: decision_not_takeable, exit 7).
intake=IDEA-1/children/IDEA-1.intake
trial cite-dark-spot SDC_STUB_FIXTURES="$fx/cite-dark-spot" SDC_STUB_FINDINGS="$fx/finding-intake-scope.yaml"
code=$?
r=$s/runs/cite-dark-spot
ci=$r/store/$intake
if (( code == 0 )) && [[ ! -f $r/run-stopped.json ]] &&
   yq -o=json "$r/store/IDEA-1/work/findings.yaml" | jq -e '.findings[] | select(.says | test("cancelled invoice is shown"))
     | .status == "resolved" and .decided_by == "decider:model" and .refused_decision == null' >/dev/null &&
   jq -se '[.[] | select(.event == "action_finished" and .action == "requirements.check")] | last | .exit == "0"' "$ci/events.jsonl" >/dev/null &&
   ! grep -q ' cites ' <<< "$(jq -r 'select(.event == "action_finished" and .action == "requirements.check") | .log // empty' "$ci/events.jsonl" |
     xargs -I{} cat "$ci/blobs/{}" 2>/dev/null)"; then
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
     | .status == "resolved" and .decided_by == "decider:model" and .refused_decision == null' >/dev/null &&
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
   jq -se 'any(.[]; .class == "close" and .decision == "reject" and .principal == "decider:model")' "$r/decisions.jsonl" >/dev/null &&
   ! jq -e 'select(.event == "terminated")' "$r/store/IDEA-1/events.jsonl" >/dev/null 2>&1; then
  echo "autopilot-close-a3: checks pass (A3: the decider is asked for the close, rejects, the run stops unclosed)"
else
  echo "autopilot-close-a3: FAILED (exit $code)"
fi
# conformance-default-off: the conformance observation after close costs model calls, so it is off
# by default (0.1.4). autopilot-close closed without SDC_CONFORMANCE: it says so and wrote no
# oracle.json. The same close with SDC_CONFORMANCE=observe calls SDC_CONFORM_BIN (here a stub that
# records its arguments) as `observe <run>`.
printf '#!/usr/bin/env bash\necho "$@" > "$2/conform-called"\n' > "$s/conform-stub"; chmod +x "$s/conform-stub"
targs=(--autopilot)
trial conformance-observe SDC_CONFORMANCE=observe SDC_CONFORM_BIN="$s/conform-stub" SDC_DECIDER_REPLIES="$(jq -cn --argjson j "$crej" '{close: $j}')"
code=$?
targs=()
r=$s/runs/conformance-observe
if grep -q '^conformance: not observed (off by default; SDC_CONFORMANCE=observe turns it on)$' "$s/autopilot-close.txt" &&
   [[ ! -e $s/runs/autopilot-close/oracle.json ]] && (( code == 0 )) &&
   [[ $(cat "$r/conform-called" 2>/dev/null) == "observe $r" ]] && ! grep -q '^conformance: not observed' "$s/conformance-observe.txt"; then
  echo "conformance-default-off: checks pass (no observation by default; SDC_CONFORMANCE=observe calls sdc-conform observe <run>)"
else
  echo "conformance-default-off: FAILED (exit $code; default line: $(grep -c '^conformance: not observed (off by default' "$s/autopilot-close.txt"), called: $(cat "$r/conform-called" 2>/dev/null))"
fi
# --- approvals by rule (docs/driver.md "Approvals by program", story:decider-program-approvals).
# Under A5 context.approve, roadmap.approve and release.publish are taken by rule:autopilot when
# the program check that guards each (rule-ends.json `by_rule`) recorded a passing result for the
# exact revision; vision.approve has no program check and stays with the decider, so do merge.approve,
# scope answers and findings. The fixture decider approves whatever it is asked, so "asked" is read
# from decisions.jsonl (principal decider:model) and decider/NNN-<action>.
rule_lines() { jq -sc --arg a "$2" '[.[] | select(.action == $a and .principal == "rule:autopilot")]' "$s/runs/$1/decisions.jsonl"; }
asked_lines() { jq -sc --arg a "$2" '[.[] | select(.action == $a and .principal == "decider:model" and .decision != "invalid_reply")]' "$s/runs/$1/decisions.jsonl"; }
asked_dirs() { find "$s/runs/$1/decider" -mindepth 1 -maxdepth 1 -type d -name "*-$2" 2>/dev/null | wc -l; }
evidence_exists() { [[ -n $(find "$s/runs/$1/store" -path '*/evidence/*' -name "$2.json" 2>/dev/null | head -n 1) ]]; }
rule_ok() { # <run> <action> <check>: every rule line of the action names the rule, the check and evidence on disk
  local run=$1 act=$2 chk=$3 id
  jq -e --arg c "$chk" 'length >= 1 and all(.[]; .decision == "approve" and .model == null and .cost_usd == 0 and .valid == true
    and .rule.principal == "rule:autopilot" and .rule.rule == "program-checks" and (.rule.checks | any(.check == $c))
    and (.cites | length) >= 1 and .cites == [.rule.checks[].evidence])' <<< "$(rule_lines "$run" "$act")" > /dev/null || return 1
  for id in $(jq -r '.[].cites[]' <<< "$(rule_lines "$run" "$act")"); do evidence_exists "$run" "$id" || return 1; done
  (( $(asked_dirs "$run" "$act") == 0 )) && [[ $(jq length <<< "$(asked_lines "$run" "$act")") == 0 ]]
}
all_asked() { # <run>: the decider was asked for context, vision, roadmap and release, a call each
  local a
  for a in context.approve vision.approve roadmap.approve release.publish; do
    [[ $(jq length <<< "$(asked_lines "$1" "$a")") -ge 1 ]] && (( $(asked_dirs "$1" "$a") >= 1 )) || return 1
  done
}
targs=(--autopilot)
trial rule-approvals SDC_RULE_APPROVALS=on
code=$?
r=$s/runs/rule-approvals
m=$("$metrics" "$r" --json - 2>/dev/null)
if (( code == 0 )) && jq -e '.outcome == "operating"' "$r/status.json" >/dev/null &&
   rule_ok rule-approvals context.approve context.check && rule_ok rule-approvals roadmap.approve roadmap.validate &&
   rule_ok rule-approvals release.publish acceptance.run && rule_ok rule-approvals release.publish spec.gate &&
   jq -e 'length >= 1' <<< "$(asked_lines rule-approvals vision.approve)" >/dev/null && (( $(asked_dirs rule-approvals vision.approve) >= 1 )) &&
   jq -e 'length >= 1' <<< "$(asked_lines rule-approvals merge.approve)" >/dev/null &&
   jq -se 'all(.[]; .producer == "rule:autopilot")' "$r"/store/IDEA-1/children/IDEA-1.direction/evidence/*-context.approve.json >/dev/null &&
   jq -se 'last.rule_approvals == "on"' "$r/run-settings.jsonl" >/dev/null &&
   jq -e --argjson n "$(jq -s '[.[] | select(.principal == "rule:autopilot")] | length' "$r/decisions.jsonl")" \
     --argjson d "$(find "$r/decider" -mindepth 1 -maxdepth 1 -type d | wc -l)" '.metrics.decisions_by_rule == $n and $n >= 3 and .metrics.decider_calls == $d
       and (.decisions.by_action["approval/context.approve"] | .by_rule >= 1 and .calls == 0 and .asked == 0)' <<< "$m" >/dev/null; then
  echo "rule-approvals: checks pass (context, roadmap and release taken by rule:autopilot on their checks' evidence ids, no decider call; vision and merge asked; metrics count $(jq .metrics.decisions_by_rule <<< "$m") decisions by rule, $(jq .metrics.decider_calls <<< "$m") decider calls)"
else
  echo "rule-approvals: FAILED (exit $code; rule lines: $(jq -c '[.[] | {action, principal}]' <<< "$(rule_lines rule-approvals context.approve)") decider dirs: $(ls "$r/decider" 2>/dev/null | tr '\n' ' '))"
fi
# A check that did not pass, or a revision that changed after the check, leaves the approval to the
# decider. Canon admits context.approve only on a passing check at the current revision, so the
# cycles make the record on disk disagree with the claim in the window between the driver's stop and
# the approval (fixtures/rule-approvals/drive-tamper.sh, put back before the approval is recorded).
tamper=$fx/rule-approvals/drive-tamper.sh
real_drive=$sdc/driver/target/debug/sdc-drive
declined_ok() { # <run> <word the declined reason names>
  local run=$1 word=$2
  rule_lines "$run" context.approve | jq -e 'length == 0' >/dev/null &&
    jq -e --arg w "$word" 'length == 1 and .[0].valid == true and (.[0].rule_declined | test($w)) and .[0].rule == null' <<< "$(asked_lines "$run" context.approve)" >/dev/null &&
    (( $(asked_dirs "$run" context.approve) == 1 )) &&
    rule_ok "$run" roadmap.approve roadmap.validate && rule_ok "$run" release.publish acceptance.run &&
    jq -se 'all(.[]; .producer == "decider:model")' "$s/runs/$run"/store/IDEA-1/children/IDEA-1.direction/evidence/*-context.approve.json >/dev/null
}
trial rule-approvals-check-failed SDC_RULE_APPROVALS=on DRIVE="$tamper" TAMPER_REAL="$real_drive" TAMPER_MODE=result
code=$?
if (( code == 0 )) && declined_ok rule-approvals-check-failed 'recorded no passing context_defaults \(valid\)'; then
  echo "rule-approvals-check-failed: checks pass (the context check's record read finding: the decider was asked and declined-by-rule is on its line; roadmap and release still by rule)"
else
  echo "rule-approvals-check-failed: FAILED (exit $code: $(jq -c '[.[] | {action, principal, rule_declined}]' "$s/runs/rule-approvals-check-failed/decisions.jsonl" 2>/dev/null | cut -c1-400))"
fi
trial rule-approvals-revision-changed SDC_RULE_APPROVALS=on DRIVE="$tamper" TAMPER_REAL="$real_drive" TAMPER_MODE=revision
code=$?
if (( code == 0 )) && declined_ok rule-approvals-revision-changed 'about context@'; then
  echo "rule-approvals-revision-changed: checks pass (the only passing check is about an earlier revision: the decider was asked; roadmap and release still by rule)"
else
  echo "rule-approvals-revision-changed: FAILED (exit $code: $(jq -c '[.[] | {action, principal, rule_declined}]' "$s/runs/rule-approvals-revision-changed/decisions.jsonl" 2>/dev/null | cut -c1-400))"
fi
# SDC_RULE_APPROVALS=off: today's behaviour, every approval asked, recorded in run-settings.jsonl.
trial rule-approvals-off SDC_RULE_APPROVALS=off
code=$?
r=$s/runs/rule-approvals-off
if (( code == 0 )) && [[ $(jq -s '[.[] | select(.principal == "rule:autopilot")] | length' "$r/decisions.jsonl") == 0 ]] &&
   [[ ! -d $r/rule-approvals ]] && jq -se 'all(.rule_approvals == "off")' "$r/run-settings.jsonl" >/dev/null &&
   all_asked rule-approvals-off; then
  echo "rule-approvals-off: checks pass (SDC_RULE_APPROVALS=off: no rule line, the decider asked for context, vision, roadmap and release; run-settings rule_approvals off)"
else
  echo "rule-approvals-off: FAILED (exit $code)"
fi
# Below A5 nothing changes: the decider is asked for all four, no rule line, the setting recorded.
targs=()
trial rule-approvals-a3 SDC_RULE_APPROVALS=on SDC_AUTONOMY=A3
code=$?
r=$s/runs/rule-approvals-a3
if (( code == 0 )) && [[ $(jq -s '[.[] | select(.principal == "rule:autopilot")] | length' "$r/decisions.jsonl") == 0 ]] &&
   [[ ! -d $r/rule-approvals ]] && jq -se 'last | .autonomy == "A3" and .rule_approvals == "on"' "$r/run-settings.jsonl" >/dev/null &&
   all_asked rule-approvals-a3; then
  echo "rule-approvals-a3: checks pass (A3: the decider asked for context, vision, roadmap and release, no rule line, no rule request)"
else
  echo "rule-approvals-a3: FAILED (exit $code)"
fi
targs=(--autopilot)
# decider-input: the decider records of runs/trial-3-haiku-5's close and its release.publish. The
# close reads published/README.md and no aep/ file: read from autopilot-close-a3, where the decider
# is asked for the close (under A5 rule:autopilot closes and no close record exists, D140; D161).
# On the autopilot run: a tree input keeps its Cargo.lock; a prompt with aep/ inputs says their
# status and revision say nothing about approval; every prompt (the decider is final) says a
# decider's answer is the product owner's own and offers no escalation to a person.
dr=$s/runs/autopilot/decider
close_d=$(find "$s/runs/autopilot-close-a3/decider" -maxdepth 1 -type d -name '*-close' 2>/dev/null | sort | tail -n 1)
lock_d=$(grep -l '"path": "impl"' "$dr"/*/decide-request.json 2>/dev/null | head -n 1)
lock_d=${lock_d%/decide-request.json}
aep_d=$(find "$dr" -maxdepth 1 -type d -name '*-roadmap.approve' 2>/dev/null | head -n 1)
if [[ -n $close_d && -f $close_d/cwd/in/published/README.md && ! -e $close_d/cwd/in/aep ]] &&
   [[ -n $lock_d && -f $lock_d/cwd/in/impl/Cargo.lock ]] &&
   [[ -n $aep_d ]] && grep -q 'belong to the planning tool' "$aep_d/prompt.md" &&
   ! grep -q 'Escalate to a person' "$dr"/*/prompt.md &&
   [[ -z $(grep -L "is the product owner's own decision" "$dr"/*/prompt.md) ]]; then
  echo "decider-input: checks pass (close reads published/README.md, no aep/; impl/Cargo.lock copied; aep status note; decider answers binding, no escalation to a person)"
else
  echo "decider-input: FAILED (close ${close_d:-none}, Cargo.lock ${lock_d:-none}, roadmap ${aep_d:-none}: an input or a prompt line is wrong)"
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
     jq -e '.effort == "xhigh"' "$r/unstick/001-rework_spent/request.json" >/dev/null &&
     jq -se 'last.efforts.unstick == {effort: "xhigh", source: "default"}' "$r/run-settings.jsonl" >/dev/null &&
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
# back and applied through the current check; f1 resolved with option 2 by decider:model, the run
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
     and .decided_by == "decider:model" and .decision == $o and .refused_decision == null' >/dev/null &&
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
refusal=$("$sdc/driver/target/debug/sdc-drive" --canon "$canon" --case-dir "$cp_" decide \
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
  expected+=(decider-release-withheld)
  trial decider-release-withheld SDC_RULE_APPROVALS=off SDC_DECIDER_REPLIES="$(jq -cn --argjson j "$rej" '{"release.publish": $j}')"
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
# requester-decider (story:requester-decider, docs/requester.md § 3): the trial-2 fixture
# (fixtures/trial-2) with the trial-2 goals, SDC_REQUESTER_DECIDER=haiku and the recorded requester
# replies (fixtures/trial-2/requester-replies.json). At A3 the scope-changing questions q1 and q3
# are decided by decider:requester, whose record's in/ holds exactly request.json, the subject
# (questions.yaml), the files the request's grounds cite (dissection.yaml), idea.yaml,
# dissection.yaml, goals.yaml and decisions.json, and whose grounds the grounding check verified;
# the merge goes to the delivering decider (decider:model). requester-no-ground: at A5, q3's reply
# says no_ground_in_goals: its default is taken, marked so, as a suppressed escalation.
t2=$fx/trial-2
req_trial() { # <name> [env ...]
  local name=$1; shift
  rm -rf "$s/runs/$name"
  env SDC_RUNS_DIR="$s/runs" SDC_WATCH=0 SDC_LLM_ADAPTER=stub SDC_DECIDER=haiku SDC_DECIDER_ADAPTER="$fixture" \
    SDC_REQUESTER_DECIDER=haiku SDC_STUB_FIXTURES="$t2" "$@" \
    "$sdc/scripts/run-trial.sh" "$name" "$(yq -r .text "$t2/idea.yaml")" --goals "$sdc/decision/examples/goals/trial-2.yaml" "${targs[@]}" > "$s/$name.txt" 2>&1
  local code=$?
  echo "$name: exit $code ($(tail -n 1 "$s/$name.txt" | cut -c1-160))"
  return $code
}
targs=()
req_trial requester-decider SDC_AUTONOMY=A3 SDC_REQUESTER_REPLIES="$t2/requester-replies.json"
code=$?
r=$s/runs/requester-decider
q1rec=$(ls -d "$r"/decider/*-q1 2>/dev/null | head -n 1)
if (( code == 0 )) &&
   jq -se '[.[] | select(.question == "q1" or .question == "q3") | select(.decision == "approve") | .principal + " " + .side]
     == ["decider:requester requester", "decider:requester requester"]' "$r/decisions.jsonl" >/dev/null &&
   yq -o=json "$r/store/IDEA-1/work/questions.yaml" | jq -e '[.questions[] | select(.id == "q1" or .id == "q3") | .answered_by] == ["decider:requester", "decider:requester"]' >/dev/null &&
   [[ -n $q1rec && "$(cd "$q1rec/cwd/in" && find . -type f | sort | tr '\n' ' ')" == "./decisions.json ./dissection.yaml ./goals.yaml ./idea.yaml ./questions.yaml ./request.json " ]] &&
   jq -e '.recommend.check_verdict == "verified"' "$q1rec/grounds-check.json" "$(ls -d "$r"/decider/*-q3 | head -n 1)/grounds-check.json" >/dev/null &&
   jq -se '[.[] | select(.action == "merge.approve" or .action == "change.merge") | .principal + " " + .side] | length > 0 and all(. == "decider:model delivering")' "$r/decisions.jsonl" >/dev/null; then
  echo "requester-decider: checks pass (q1, q3 by decider:requester on verified goals grounds, its in/ exactly the request, subject, cited files, idea, dissection, goals, decisions; merge by decider:model)"
else
  echo "requester-decider: FAILED (exit $code: $(jq -sc '[.[] | {action, question, principal, side}]' "$r/decisions.jsonl" 2>/dev/null | cut -c1-300))"
fi
targs=(--autopilot)
req_trial requester-no-ground SDC_REQUESTER_REPLIES="$(jq -c '.q3 = {option: null, answer: null, reasons: ["fixture: no entry covers sign-in"], grounds: [], strength: "weak", no_ground_in_goals: true}' "$t2/requester-replies.json")"
code=$?
r=$s/runs/requester-no-ground
if (( code == 0 )) &&
   jq -se '[.[] | select(.question == "q3" and .principal == "decider:requester")] | last
     | .no_ground_in_goals == true and .suppressed == true and .answer == "Sign in with a one-time code sent by email"' "$r/decisions.jsonl" >/dev/null &&
   yq -o=json "$r/store/IDEA-1/work/questions.yaml" | jq -e '.questions[] | select(.id == "q3") | .answer == .default and .answered_by == "decider:requester"' >/dev/null; then
  echo "requester-no-ground: checks pass (A5: no ground for q3, its default taken, marked no_ground_in_goals)"
else
  echo "requester-no-ground: FAILED (exit $code: $(jq -sc '[.[] | select(.question == "q3") | {decision, no_ground_in_goals, suppressed, answer}]' "$r/decisions.jsonl" 2>/dev/null))"
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
out=$(cd "$p" && "$sdc/driver/target/debug/sdc-findings" --rules "$sdc/bindings/tools/consistency-rules.json" amend spec 2>&1)
if yq -e '.findings[] | select(.decided_by != null) | .decided_by == "person:stand-in"' "$f" >/dev/null &&
   jq -se 'any(.stand_in == true and .stand_in_by == "scripts/run-full-cycle.sh")' "$s/runs/finding-person/run-settings.jsonl" >/dev/null &&
   [[ $out == *"no principal"* ]] && FID=$fid yq -e '.findings[] | select(.id == strenv(FID)) | .status == "needs_person" and .decided_by == null' "$p/findings.yaml" >/dev/null &&
   (cd "$p" && "$sdc/bindings/tools/sdc-tool.sh" findings-wait) && ! grep -rq 'human' "$p/questions.yaml"; then
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
  jq -se --slurpfile c "$sdc/bindings/tools/node-classes.json" '
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
   expect models-class '{writer: "model-writer", implement: "model-implement", review: "model-review", other: "model-other"}[.class]' model-decider &&
   jq -se 'last.models | .writer.model == "model-writer" and .implement.model == "model-implement" and .review.model == "model-review"
     and .decider.model == "model-decider" and .other.model == "model-other" and .writer.source == "SDC_LLM_MODEL_WRITER"' "$s/runs/models-class/run-settings.jsonl" >/dev/null &&
   jq -se 'last.models | .writer.source == "binding" and .writer.nodes["spec.author"] == "claude-haiku-5-5" and .decider.model == "claude-haiku-5-5"' "$s/runs/models-binding/run-settings.jsonl" >/dev/null; then
  echo "models: checks pass (binding default, one model for all, one per class; each call names its model; run settings per class)"
else
  echo "models: FAILED"
fi
# --- reject and cap routes (docs/reject-routes.md) ------------------------------------------------
# Each cycle drives a stub run into one reject or cap of the audit and asserts the route the row
# should have. Cycles in `expected` have their route built; those in `pending` assert a route not
# built yet and are reported apart (`pending: N (K still failing)`), never failing the script.
rr_rej='{"decision": "reject", "answer": null, "reasons": ["fixture: the change does not meet r1"], "cites": ["r1"], "best": null}'
rr_app='{"decision": "approve", "answer": null, "reasons": ["fixture: the rework addresses the rejection"], "cites": ["r1"], "best": null}'
rr_story() { find "$s/runs/$1/store" -type d -name 'IDEA-1.*.invoices' | head -n 1; }
targs=(--autopilot)
# decider-question-reject (R1): the decider replies `reject` to the scope-changing question q2. A
# question has no rejection: the check refuses it with its reason, the decider is asked again and
# answers. Passes when the run closes (exit 0) with q2 decided by decider:model after one refused
# reply. Before: exit 6, rejected_without_rework_path.
trial decider-question-reject SDC_STUB_FIXTURES="$fx/scope-two" SDC_DECIDER_REPLIES="$(jq -cn '{q2: [
  {decision: "reject", answer: null, reasons: ["fixture: neither option fits"], cites: ["d4"], best: null},
  {decision: "approve", answer: "the billing clerk, before it is paid", reasons: ["fixture: the idea names only the clerk"], cites: ["d4"], best: null}]}')"
code=$?
r=$s/runs/decider-question-reject
if (( code == 0 )) &&
   jq -se '[.[] | select(.question == "q2") | .decision] == ["invalid_reply", "approve"]
     and ([.[] | select(.question == "q2")][0].invalid_reason | test("a question is decided with approve"))' "$r/decisions.jsonl" >/dev/null &&
   yq -o=json "$r/store/IDEA-1/work/questions.yaml" | jq -e '.questions[] | select(.id == "q2") | .answered_by == "decider:model"' >/dev/null; then
  echo "decider-question-reject: checks pass (a reject of a question refused with its reason, asked again, answered; closed)"
else
  echo "decider-question-reject: FAILED (exit $code, stop $(jq -c '{cause}' "$r/run-stopped.json" 2>/dev/null))"
fi
# rework-fresh-allowance (R6): change.implement's first tree does not build (tests.run sends it
# back), then merge.approve is rejected twice and approved. Each recorded rejection is an arrival,
# so the second rework still runs. Passes when the run closes with 4 change.implement runs and no
# writer_spent. Before: writer_spent after 3 runs at the second rejection (runs/trial-3-haiku-4).
trial rework-fresh-allowance SDC_STUB_FIXTURES="$fx/rework-allowance" \
  SDC_DECIDER_REPLIES="$(jq -cn --argjson j "$rr_rej" --argjson a "$rr_app" '{"merge.approve": {rounds: [$j, $j, $a]}}')"
code=$?
st=$(rr_story rework-fresh-allowance)
if (( code == 0 )) && [[ -n $st ]] &&
   jq -se '([.[] | select(.event == "action_finished" and .action == "change.implement")] | length) == 4
     and ([.[] | select(.event == "writer_spent")] | length) == 0' "$st/events.jsonl" >/dev/null &&
   jq -se '[.[] | select(.action == "merge.approve") | .decision] == ["reject", "reject", "approve"]' "$s/runs/rework-fresh-allowance/decisions.jsonl" >/dev/null; then
  echo "rework-fresh-allowance: checks pass (a test retry and two merge reworks: 4 runs, no writer_spent; closed)"
else
  echo "rework-fresh-allowance: FAILED (exit $code, stop $(jq -c '{cause, reason}' "$s/runs/rework-fresh-allowance/run-stopped.json" 2>/dev/null | cut -c1-240))"
fi
# spec-review-rework (R8): the stub reviewer rejects the first specification (a `.review-reject`
# file in the tree); the rejected spec_review sends spec.author back. Passes when the cycle ends
# operating with spec.author run twice, the second sent back by spec_review. Before: stuck.
cycle spec-review-rework SDC_STUB_FIXTURES="$fx/spec-review-reject"
code=$?
if (( code == 0 )) &&
   jq -se '([.[] | select(.event == "action_finished" and .action == "spec.author")] | length) == 2
     and ([.[] | select(.event == "action_started" and .action == "spec.author")] | last | .retry_of | index("spec_review"))' \
     "$s/store-spec-review-rework/$spec" >/dev/null; then
  echo "spec-review-rework: checks pass (a rejected spec review sends spec.author back; operating)"
else
  echo "spec-review-rework: FAILED (cycle exit $code, or spec.author not sent back by spec_review)"
fi
# spec-interp-unsupported: runs/release-0.1.2.mixed.1 stopped writer_spent at change.implement
# because its spec had scenarios ESS's interpreter reports unsupported ("binding command has an
# unmet outcome obligation"), first met at the story's tests.run, where the implementing writer
# cannot change the spec. The spec stage now runs the synthesized suite against the interpreter.
# fixtures/spec-interp-unsupported/billing-spec.0 is the billing spec plus a binding that invokes a
# command updating an Account the chain never created: it validates, compiles and synthesizes with
# no refusal, and one scenario is unsupported. spec.author (fixture adapter, through llm-node.sh)
# writes it first, then the clean billing spec. Passes when sdc-tool.sh ess-gate refuses the fixture
# naming the scenario and passes the clean spec, spec.synthesize is refused then clean, spec.author
# is sent back by spec_synthesis with the scenario id and ESS's reason in its in/feedback.md, and
# the cycle ends operating. Before: the fixture passed the gate and spec.author ran once.
sg=$s/spec-interp-gate
rm -rf "$sg" && mkdir -p "$sg/bad" "$sg/clean"
cp -r "$fx/spec-interp-unsupported/billing-spec.0" "$sg/bad/spec" && cp -r "$fx/billing-spec" "$sg/clean/spec"
(cd "$sg/bad" && "$sdc/bindings/tools/sdc-tool.sh" ess-gate > "$sg/bad.txt" 2>&1); bad=$?
(cd "$sg/clean" && "$sdc/bindings/tools/sdc-tool.sh" ess-gate > "$sg/clean.txt" 2>&1); clean=$?
cycle spec-interp-unsupported SDC_STUB_FIXTURES="$fx/spec-interp-unsupported" SDC_LLM_ADAPTER="$fixture" SDC_LLM_NODES=spec.author
code=$?
sid="unsupported billing.email.RequestRename/outcome/requested: ESS-CF-TARGET: cannot expose interpreted binding: binding command has an unmet outcome obligation"
fb=$(find "$s/store-spec-interp-unsupported" -path '*-spec.author/cwd/in/feedback.md' 2>/dev/null | sort | tail -n 1)
if (( code == 0 && bad == 1 && clean == 0 )) && grep -qF "$sid" "$sg/bad.txt" && grep -q '^interpreted run: 32 scenarios, all passed$' "$sg/clean.txt" &&
   jq -se '([.[] | select(.event == "action_finished" and .action == "spec.author")] | length) == 2
     and ([.[] | select(.event == "action_started" and .action == "spec.author")] | last | .retry_of | index("spec_synthesis"))
     and ([.[] | select(.event == "action_finished" and .action == "spec.synthesize") | .exit] == ["1", "0"])' \
     "$s/store-spec-interp-unsupported/$spec" >/dev/null &&
   [[ -n $fb ]] && grep -qF "$sid" "$fb"; then
  echo "spec-interp-unsupported: checks pass (the gate refuses a spec its interpreter cannot carry out, naming the scenario; spec.author sent back with it; the clean spec passes; operating)"
else
  echo "spec-interp-unsupported: FAILED (cycle exit $code, gate on the fixture $bad, on the clean spec $clean, feedback ${fb:-none})"
fi
# scope-unattributed-reject (R2): the interview's scope-changing answers q2 and q3 are
# given but unattributed (answered_by non-interactive), none is open, so scope.answer goes to the
# decider as an approval, which it rejects. Route: each unattributed answer is put to the decider
# as a question (as R1). Passes when the run closes with q2 and q3 answered_by decider:model and no
# approval decision on scope.answer. Before: exit 6, rejected_without_rework_path.
trial scope-unattributed-reject SDC_STUB_FIXTURES="$fx/scope-unattributed" \
  SDC_DECIDER_REPLIES='{"scope.answer": {"decision": "reject", "answer": null, "reasons": ["fixture: the answers were given by nobody who may give them"], "cites": ["d4"], "best": null}}'
code=$?
r=$s/runs/scope-unattributed-reject
if (( code == 0 )) &&
   yq -o=json "$r/store/IDEA-1/work/questions.yaml" | jq -e '[.questions[] | select(.scope_changing == true) | .answered_by] == ["decider:model", "decider:model"]' >/dev/null &&
   jq -se '[.[] | select(.type == "approval" and .action == "scope.answer")] | length == 0' "$r/decisions.jsonl" >/dev/null; then
  echo "scope-unattributed-reject: checks pass (unattributed answers put to the decider one by one; closed)"
else
  echo "scope-unattributed-reject: FAILED (exit $code, stop $(jq -c '{cause}' "$r/run-stopped.json" 2>/dev/null))"
fi
# finding-refused-reask (R3): fixtures/offline-decision. The program check refuses the
# decider's decision on the finding. Route: the decider is asked again with the refusal reason, at
# most 2 times, then the stop decision_not_takeable. The fixture decider takes the refused default
# every time, so passes when the finding was decided 3 times, the later requests carry the
# refusal, and the run stops (exit 7) with cause decision_not_takeable. Before: 1 decision, exit 7.
trial finding-refused-reask SDC_STUB_FIXTURES="$fx/offline-decision"
code=$?
r=$s/runs/finding-refused-reask
last_rec=$(ls -d "$r"/decider/*-p-offline-hosted-context.deployment 2>/dev/null | tail -n 1)
if (( code == 7 )) && jq -e '.cause == "decision_not_takeable"' "$r/run-stopped.json" >/dev/null 2>&1 &&
   jq -se '[.[] | select(.finding == "p-offline-hosted-context.deployment" and .decision == "approve")] | length == 3' "$r/decisions.jsonl" >/dev/null &&
   [[ -n $last_rec ]] && grep -qi 'refused' "$last_rec/prompt.md"; then
  echo "finding-refused-reask: checks pass (the refused decision asked again twice with the refusal, then decision_not_takeable)"
else
  echo "finding-refused-reask: FAILED (exit $code, $(jq -s '[.[] | select(.class == "finding")] | length' "$r/decisions.jsonl" 2>/dev/null) finding decision(s))"
fi
# rework-spent-dropped (R4, pending): merge.approve rejected three times. Route: the story ends by
# rule:autopilot through the protocol's `dropped` (explicitly_dropped, the rejection as its
# reasons), the epic and the root end through the outcomes their evaluation offers, the run
# closes not_delivered. Today: exit 6, rework_spent.
trial rework-spent-dropped SDC_DECIDER_REPLIES="$(jq -cn --argjson j "$rr_rej" '{"merge.approve": $j}')"
code=$?
r=$s/runs/rework-spent-dropped
st=$(rr_story rework-spent-dropped)
if (( code == 0 )) && [[ ! -f $r/run-stopped.json && -n $st ]] &&
   jq -e '.state == "closed" and .outcome == "not_delivered"' "$r/status.json" >/dev/null 2>&1 &&
   jq -e 'any(.[]; .decision == "explicitly_dropped" and .principal == "rule:autopilot")' "$st/decisions.json" >/dev/null &&
   jq -se '[.[] | select(.event == "terminated") | .outcome] == ["dropped"]' "$st/events.jsonl" >/dev/null; then
  echo "rework-spent-dropped: checks pass (third rejection: story dropped by rule:autopilot, root not_delivered, closed)"
else
  echo "rework-spent-dropped: FAILED (exit $code, stop $(jq -c '{cause}' "$r/run-stopped.json" 2>/dev/null))"
fi
# rework-spent-abandoned (R5, pending): context.approve rejected three times. Route: the root ends
# `abandoned` (explicitly_abandoned recorded by rule:autopilot with the rejections), the run
# closes. Today: exit 6, rework_spent.
trial rework-spent-abandoned SDC_RULE_APPROVALS=off SDC_DECIDER_REPLIES='{"context.approve": {"decision": "reject", "answer": null, "reasons": ["fixture: the context is not usable"], "cites": ["context.platform"], "best": null}}'
code=$?
r=$s/runs/rework-spent-abandoned
if (( code == 0 )) && [[ ! -f $r/run-stopped.json ]] &&
   jq -e '.state == "closed" and .outcome == "abandoned"' "$r/status.json" >/dev/null 2>&1 &&
   jq -e 'any(.[]; .decision == "explicitly_abandoned" and .principal == "rule:autopilot")' "$r/store/IDEA-1/decisions.json" >/dev/null; then
  echo "rework-spent-abandoned: checks pass (third rejection of the context: root abandoned by rule:autopilot, closed)"
else
  echo "rework-spent-abandoned: FAILED (exit $code, stop $(jq -c '{cause}' "$r/run-stopped.json" 2>/dev/null))"
fi
# review-invalid-named: change.review as a model call (impl/review-node) whose every reply is
# invalid exits 2 and records nothing. Route (fix/review-reply): the stop review_invalid, which
# names change.review, never `stuck`. Without the model review node (change.review has no `llm` binding) it fails.
if jq -e '."change.review".llm' "$sdc/driver/examples/sdc/sdc-story.bindings.json" >/dev/null 2>&1; then
  bad='{"decision": "rejected", "summary": "fixture", "findings": [{"kind": "violation", "at": "impl/README.md:9999", "violates": "r1", "says": "fixture: a line that does not exist"}]}'
  trial review-invalid-named SDC_LLM_NODES=change.review SDC_LLM_ADAPTER="$fixture" SDC_REVIEW_REPLIES="$(jq -cn --argjson j "$bad" '{rounds: [$j]}')"
  code=$?
  r=$s/runs/review-invalid-named
  if (( code != 0 )) && jq -e '.cause != "stuck" and .cause != null and (.reason | test("change.review"))' "$r/run-stopped.json" >/dev/null 2>&1; then
    echo "review-invalid-named: checks pass (invalid review replies: a named stop at change.review, not stuck)"
  else
    echo "review-invalid-named: FAILED (exit $code, stop $(jq -c '{cause}' "$r/run-stopped.json" 2>/dev/null))"
  fi
else
  echo "review-invalid-named: FAILED (change.review is not a model call here: needs impl/review-node)"
fi
# writer-own-error (R9): questions.answer (fixture adapter, apply step) changes the
# scope-changing question q2, which the apply step refuses (exit 1, nothing applied). Route: the
# failed run's own log reaches the next run as in/feedback.md. Passes when the second
# questions.answer call has in/feedback.md naming the refusal.
trial writer-own-error SDC_STUB_FIXTURES="$fx/own-error" SDC_LLM_NODES=questions.answer SDC_LLM_ADAPTER="$fixture"
code=$?
r=$s/runs/writer-own-error
second=$(jq -rs '[.[] | select(.action == "questions.answer") | .record] | .[1] // empty' "$r/llm-calls.jsonl" 2>/dev/null)
if [[ -n $second ]] && grep -q 'changed a scope-changing question' "$second/cwd/in/feedback.md" 2>/dev/null; then
  echo "writer-own-error: checks pass (the refused apply's log fed back to the next run as in/feedback.md)"
else
  echo "writer-own-error: FAILED (exit $code, second call ${second:-none}, no in/feedback.md naming the refusal)"
fi
# implement-percause (R7): change.implement's trees: one that does not build (tests.run
# fails), one with an empty README (the stub review rejects), one that does not build again, then
# a good one. Route: runs counted per retry kind (conformance_run 2, code_review 1, each under 3),
# with a backstop of 8 per arrival. Passes when the run closes with 4 change.implement runs and no
# writer_spent. Before: writer_spent after 3 runs.
trial implement-percause SDC_STUB_FIXTURES="$fx/implement-percause"
code=$?
st=$(rr_story implement-percause)
if (( code == 0 )) && [[ -n $st ]] &&
   jq -se '([.[] | select(.event == "action_finished" and .action == "change.implement")] | length) == 4
     and ([.[] | select(.event == "writer_spent")] | length) == 0' "$st/events.jsonl" >/dev/null; then
  echo "implement-percause: checks pass (two test failures and a review rejection: counted per kind, 4 runs; closed)"
else
  echo "implement-percause: FAILED (exit $code, stop $(jq -c '{cause, reason}' "$s/runs/implement-percause/run-stopped.json" 2>/dev/null | cut -c1-240))"
fi
# writer-sees-test-failure (R13): change.implement (fixture adapter, llm-node.sh) first writes a
# tree whose tests build and one fails with a message, among 45 passing tests; tests.run fails.
# Route: the failing test's own output (panic message included), not the build pool's last line,
# reaches the next run's in/feedback.md. Passes when the second change.implement call's
# in/feedback.md holds the assertion message, no `slot: job` line, and the run closes.
trial writer-sees-test-failure SDC_LLM_ADAPTER="$fixture" SDC_LLM_NODES=change.implement SDC_STUB_FIXTURES="$fx/test-failure"
code=$?
r=$s/runs/writer-sees-test-failure
second=$(jq -rs '[.[] | select(.action == "change.implement") | .record] | .[1] // empty' "$r/llm-calls.jsonl" 2>/dev/null)
if (( code == 0 )) && [[ -n $second ]] &&
   grep -q 'fixture-failure: the second email of an account counts 2' "$second/cwd/in/feedback.md" 2>/dev/null &&
   ! grep -q '^slot: job ' "$second/cwd/in/feedback.md"; then
  echo "writer-sees-test-failure: checks pass (the failing test's message reached the next writer run; closed)"
else
  echo "writer-sees-test-failure: FAILED (exit $code, second call ${second:-none}, feedback without the test's message)"
fi
# --- change.implement as one request (SDC_IMPLEMENT_MODE=request, docs/driver.md "Implement as one
# request"): change.implement on the fixture adapter, which answers each request with the stub tree
# packed as a reply (sdc-reply implement-pack) or a scripted reply (SDC_IMPLEMENT_REPLIES); every
# other node on its stub. sdc-reply implement-run assembles the input, checks the reply, writes it.
req_env=(SDC_IMPLEMENT_MODE=request SDC_LLM_ADAPTER="$fixture" SDC_LLM_NODES=change.implement)
icalls() { jq -cs '[.[] | select(.action == "change.implement")]' "$s/runs/$1/llm-calls.jsonl" 2>/dev/null || echo '[]'; }
iwork() { echo "$s/runs/$1/store/IDEA-1/work"; }
# implement-request-build: the first build in one request. Passes when the run closes with one
# change.implement call of one request (no retry, no continuation), offered no operation, its prompt
# saying there is nothing to read, its record holding the reply schema and the input manifest (a
# first story on an empty tree), and the stub tree written into work/impl through the reply.
trial implement-request-build "${req_env[@]}"
code=$?
c=$(icalls implement-request-build)
rec=$(jq -r '.[0].record // empty' <<< "$c")
if (( code == 0 )) && jq -e 'length == 1 and .[0].implement_mode == "request" and .[0].requests == 1 and .[0].reply_retries == 0
     and .[0].continuations == 0 and .[0].allowed_operations == [] and .[0].applied_exit == 0' <<< "$c" >/dev/null &&
   [[ -n $rec ]] && grep -q 'you have no tools, nothing to read' "$rec/prompt.md" &&
   jq -e '.required == ["files", "note", "findings", "remaining"]' "$rec/reply-schema.json" >/dev/null &&
   jq -e '.tree.first_story == true and .commands != []' "$rec/implement-inputs.json" >/dev/null &&
   cmp -s "$fx/impl/src/lib.rs" "$(iwork implement-request-build)/impl/src/lib.rs"; then
  echo "implement-request-build: checks pass (one request, no operation offered, the reply written into impl/; closed)"
else
  echo "implement-request-build: FAILED (exit $code, calls $(jq -c 'map({implement_mode, requests, reply_retries, applied_exit})' <<< "$c" | cut -c1-200))"
fi
# implement-request-tests-rework: the first reply's tree fails tests.run (fixtures/test-failure).
# Passes when the run closes with two change.implement calls of one request each, the second
# prompt holding the failing test's message (no `slot: job` line) and the current text of the tree.
trial implement-request-tests-rework "${req_env[@]}" SDC_STUB_FIXTURES="$fx/test-failure"
code=$?
c=$(icalls implement-request-tests-rework)
rec=$(jq -r '.[1].record // empty' <<< "$c")
if (( code == 0 )) && jq -e 'length == 2 and all(.requests == 1)' <<< "$c" >/dev/null && [[ -n $rec ]] &&
   grep -q 'fixture-failure: the second email of an account counts 2' "$rec/prompt.md" && ! grep -q '^slot: job ' "$rec/prompt.md" &&
   grep -q '^### impl/tests/billing.rs' "$rec/prompt.md"; then
  echo "implement-request-tests-rework: checks pass (the failing test and the current files in the second request; closed)"
else
  echo "implement-request-tests-rework: FAILED (exit $code, calls $(jq -c 'map({requests, applied_exit})' <<< "$c"))"
fi
# implement-request-review-rework: change.review (fixture) rejects once with the README finding; the
# second implement reply replaces README.md. Passes when the run closes with two change.implement
# calls of one request each, the second prompt holding the rejection's reason and the current
# README, and work/impl/README.md the reworked text.
rw='{"files": [{"path": "README.md", "content": "# billing\n\nRun the service: cargo run -- 8080 billing.json\nRun the tests: task check\n"}], "note": "1. README.md:3 names the command that runs it", "findings": [], "remaining": []}'
irej='{"decision": "rejected", "summary": "fixture: the README does not say how to run it", "findings": [{"kind": "violation", "at": "impl/README.md:1", "violates": "r1", "says": "fixture: the README names no command to run the service"}]}'
iapp='{"decision": "approved", "summary": "fixture: the rework addresses the finding", "findings": []}'
trial implement-request-review-rework "${req_env[@]}" SDC_LLM_NODES="change.implement change.review" \
  SDC_REVIEW_REPLIES="$(jq -cn --argjson j "$irej" --argjson a "$iapp" '{rounds: [$j, $a]}')" \
  SDC_IMPLEMENT_REPLIES="$(jq -cn --argjson r "$rw" '{rounds: ["stub", $r]}')"
code=$?
c=$(icalls implement-request-review-rework)
rec=$(jq -r '.[1].record // empty' <<< "$c")
if (( code == 0 )) && jq -e 'length == 2 and all(.requests == 1)' <<< "$c" >/dev/null && [[ -n $rec ]] &&
   grep -q '^## Rework' "$rec/prompt.md" && grep -q 'fixture: the README names no command to run the service' "$rec/prompt.md" &&
   grep -q '^### impl/README.md' "$rec/prompt.md" && grep -q 'cargo run -- 8080' "$(iwork implement-request-review-rework)/impl/README.md"; then
  echo "implement-request-review-rework: checks pass (the rejection and the current README in the second request, README replaced; closed)"
else
  echo "implement-request-review-rework: FAILED (exit $code, calls $(jq -c 'map({requests, applied_exit})' <<< "$c"))"
fi
# implement-request-invalid-retry: the first reply has no file; the second is the stub tree. Passes
# when the run closes with one change.implement call of two requests, one retry, the second request
# holding the check's error.
trial implement-request-invalid-retry "${req_env[@]}" \
  SDC_IMPLEMENT_REPLIES='{"rounds": [[{"files": [], "note": "", "findings": [], "remaining": []}, "stub"]]}'
code=$?
c=$(icalls implement-request-invalid-retry)
rec=$(jq -r '.[0].record // empty' <<< "$c")
if (( code == 0 )) && jq -e 'length == 1 and .[0].requests == 2 and .[0].reply_retries == 1' <<< "$c" >/dev/null && [[ -n $rec ]] &&
   grep -q 'The program check refused it: `files` is empty' "$rec/req-2/prompt.md"; then
  echo "implement-request-invalid-retry: checks pass (an empty reply refused, retried with the error, accepted; closed)"
else
  echo "implement-request-invalid-retry: FAILED (exit $code, calls $(jq -c 'map({requests, reply_retries})' <<< "$c"))"
fi
# implement-request-invalid-named: every reply lacks a file's content. Passes when each
# change.implement call made three requests (two retries) and ended implement_reply_invalid,
# nothing was written into work/impl, and the run stopped with cause implement_reply_invalid.
trial implement-request-invalid-named "${req_env[@]}" \
  SDC_IMPLEMENT_REPLIES='{"rounds": [[{"files": [{"path": "src/lib.rs"}], "note": "x", "findings": [], "remaining": []}]]}'
code=$?
c=$(icalls implement-request-invalid-named)
if (( code != 0 )) && jq -e 'length >= 1 and all(.requests == 3 and .reply_retries == 2 and .cause == "implement_reply_invalid")' <<< "$c" >/dev/null &&
   [[ ! -e $(iwork implement-request-invalid-named)/impl/src/lib.rs ]] &&
   jq -e '.cause == "implement_reply_invalid"' "$s/runs/implement-request-invalid-named/run-stopped.json" >/dev/null; then
  echo "implement-request-invalid-named: checks pass (three requests per call, nothing written, stopped implement_reply_invalid)"
else
  echo "implement-request-invalid-named: FAILED (exit $code, calls $(jq -c 'map({requests, reply_retries, cause})' <<< "$c"), stop $(jq -c '{cause}' "$s/runs/implement-request-invalid-named/run-stopped.json" 2>/dev/null))"
fi
# implement-request-path-escape: the first reply writes ../escape.rs; the second is the stub tree.
# Passes when nothing named escape.rs exists outside impl/ (call dir or work/), the second request
# holds the refusal naming the path, and the run closes.
trial implement-request-path-escape "${req_env[@]}" \
  SDC_IMPLEMENT_REPLIES='{"rounds": [[{"files": [{"path": "../escape.rs", "content": "fn main() {}\n"}], "note": "", "findings": [], "remaining": []}, "stub"]]}'
code=$?
c=$(icalls implement-request-path-escape)
rec=$(jq -r '.[0].record // empty' <<< "$c")
if (( code == 0 )) && [[ -n $rec ]] && jq -e '.[0].requests == 2' <<< "$c" >/dev/null &&
   grep -qF 'path "../escape.rs": leaves the product tree' "$rec/req-2/prompt.md" &&
   [[ -z $(find "$rec" "$(iwork implement-request-path-escape)" -name escape.rs 2>/dev/null) ]]; then
  echo "implement-request-path-escape: checks pass (../escape.rs refused, nothing written outside impl/; closed)"
else
  echo "implement-request-path-escape: FAILED (exit $code, calls $(jq -c 'map({requests, reply_retries})' <<< "$c"))"
fi
# --- requirement coverage (story:implement-cites-requirements) ---------------------------------
# change.implement on the fixture adapter (llm-node.sh's model path: schema, shape check, apply)
# replies with the coverage documents of SDC_COVERAGE_REPLIES, one round per call; the tree is
# fixtures/impl (src/lib.rs 79 lines; tests create_invoice ... send_email); the story carries r1
# and r2. The binding's postcondition (sdc-reply check-coverage) runs right after each run. Every
# coverage cycle sets SDC_IMPLEMENT_COVERAGE=on (off by default: implement-coverage-default).
cov_doc() { # <r2 entry>: r1 built and cited, then r2 as given
  jq -cn --argjson r2 "$1" '{requirements: [{id: "r1", status: "built", at: [{file: "src/lib.rs", line: 1}], test: "create_invoice"}, $r2]}'; }
cov_good=$(cov_doc '{"id": "r2", "status": "built", "at": [{"file": "src/lib.rs", "line": 2}], "test": "send_email"}')
cov_notbuilt=$(cov_doc '{"id": "r2", "status": "not_built", "reason": "fixture: the email is not sent yet"}')
cov_outside=$(cov_doc '{"id": "r2", "status": "built", "at": [{"file": "src/lib.rs", "line": 999}], "test": "send_email"}')
cov_notest=$(cov_doc '{"id": "r2", "status": "built", "at": [{"file": "src/lib.rs", "line": 2}], "test": "email_is_sent"}')
cov_env=(SDC_IMPLEMENT_COVERAGE=on SDC_LLM_NODES=change.implement SDC_LLM_ADAPTER="$fixture")
cov_rounds() { jq -cn --args '{rounds: [$ARGS.positional[] | fromjson]}' "$@"; }
# impl_takes <story dir>: change.implement's finished runs as {exit, refused kind, evidence count},
# and whether tests.run ran between a refused run and the next implement run.
impl_takes() { jq -sc '[.[] | select(.event == "action_finished" and .action == "change.implement")
  | {exit, refused: (.refused.kind // null), evidence: (.evidence | length)}]' "$1/events.jsonl"; }
tests_after_refusal() { jq -se '[.[] | select(.event == "action_finished")] as $f
  | [range(0; $f | length) as $i | select($f[$i].refused != null)
     | ($f[$i + 1:] | map(.action) | (index("change.implement") // length) as $n | .[:$n] | index("tests.run"))] | all(. == null)' "$1/events.jsonl"; }
cov_feedback() { jq -rs '[.[] | select(.action == "change.implement") | .record] | .[1] // empty' "$s/runs/$1/llm-calls.jsonl" 2>/dev/null; }
# coverage-complete: every requirement built and cited. Passes when the run closes, the one
# implement run recorded its change with no refusal, its log names the passing postcondition, and
# the review (fixture adapter, approving) got the document as in/coverage.json among its inputs.
trial coverage-complete SDC_IMPLEMENT_COVERAGE=on SDC_LLM_NODES="change.implement change.review" SDC_LLM_ADAPTER="$fixture" \
  SDC_COVERAGE_REPLIES="$(cov_rounds "$cov_good")"
code=$?
st=$(rr_story coverage-complete)
rv=$(ls -d "$s"/runs/coverage-complete/llm/*-change.review 2>/dev/null | tail -n 1)
lg=$(jq -rs '[.[] | select(.event == "action_finished" and .action == "change.implement")] | last | .log // empty' "$st/events.jsonl" 2>/dev/null)
if (( code == 0 )) && [[ -n $st && $(impl_takes "$st") == '[{"exit":"0","refused":null,"evidence":1}]' ]] &&
   grep -q 'requirement coverage of story invoices: 2 built and cited (r1 r2)' "$st/blobs/$lg" &&
   jq -e '.requirements | length == 2' "$s/runs/coverage-complete/store/IDEA-1/work/impl/.sdc/coverage.json" >/dev/null 2>&1 &&
   [[ -n $rv ]] && jq -e '.requirements[1].test == "send_email"' "$rv/cwd/in/coverage.json" >/dev/null &&
   grep -q '^- in/coverage.json (read-only' "$rv/prompt.md" && [[ ! -e $rv/cwd/in/impl/.sdc ]]; then
  echo "coverage-complete: checks pass (every requirement cited: the change recorded, the review read in/coverage.json; closed)"
else
  echo "coverage-complete: FAILED (exit $code, implement runs $(impl_takes "$st" 2>/dev/null), review ${rv:-none})"
fi
# coverage-not-built: the first reply says r2 not_built. Passes when that run is refused (no
# evidence, `refused` requirement_coverage), tests.run does not run on it, the next run is sent
# back with retry_of requirement_coverage and an in/feedback.md naming r2, it passes, the run
# closes, and sdc-run-metrics counts the retry as requirement_coverage:refused.
trial coverage-not-built "${cov_env[@]}" SDC_COVERAGE_REPLIES="$(cov_rounds "$cov_notbuilt" "$cov_good")"
code=$?
st=$(rr_story coverage-not-built)
fb=$(cov_feedback coverage-not-built)
if (( code == 0 )) && [[ -n $st && $(impl_takes "$st") == '[{"exit":"0","refused":"requirement_coverage","evidence":0},{"exit":"0","refused":null,"evidence":1}]' ]] &&
   tests_after_refusal "$st" >/dev/null &&
   jq -se '[.[] | select(.event == "action_started" and .action == "change.implement")] | .[1].retry_of == ["requirement_coverage"]' "$st/events.jsonl" >/dev/null &&
   [[ -n $fb ]] && grep -q '^## requirement_coverage = refused (from change.implement' "$fb/cwd/in/feedback.md" &&
   grep -q -- '- r2: not built: fixture: the email is not sent yet' "$fb/cwd/in/feedback.md" &&
   grep -q '^refused requirements: r2$' "$fb/cwd/in/feedback.md" &&
   "$metrics" "$s/runs/coverage-not-built" --json - 2>/dev/null | jq -e '[.nodes[] | select(.node == "story/change.implement")][0].retries_by_cause == {"requirement_coverage:refused": 1}' >/dev/null; then
  echo "coverage-not-built: checks pass (r2 not_built: refused before tests.run, fed back naming r2, the next run passed; metrics requirement_coverage:refused 1; closed)"
else
  echo "coverage-not-built: FAILED (exit $code, implement runs $(impl_takes "$st" 2>/dev/null), feedback ${fb:-none})"
fi
# coverage-line-outside: r2 cited at src/lib.rs:999 (the file has 79 lines). Passes when the first
# run is refused naming that line, the next passes, and the run closes.
trial coverage-line-outside "${cov_env[@]}" SDC_COVERAGE_REPLIES="$(cov_rounds "$cov_outside" "$cov_good")"
code=$?
st=$(rr_story coverage-line-outside)
fb=$(cov_feedback coverage-line-outside)
if (( code == 0 )) && [[ -n $st && $(impl_takes "$st" | jq -c 'map(.refused)') == '["requirement_coverage",null]' ]] &&
   tests_after_refusal "$st" >/dev/null && [[ -n $fb ]] &&
   grep -q -- '- r2: src/lib.rs:999 is outside src/lib.rs (79 lines)' "$fb/cwd/in/feedback.md"; then
  echo "coverage-line-outside: checks pass (a cited line outside its file: refused, fed back, fixed; closed)"
else
  echo "coverage-line-outside: FAILED (exit $code, implement runs $(impl_takes "$st" 2>/dev/null))"
fi
# coverage-test-missing: r2's test email_is_sent does not exist. Passes when the first run is
# refused naming the test, the next passes, and the run closes.
trial coverage-test-missing "${cov_env[@]}" SDC_COVERAGE_REPLIES="$(cov_rounds "$cov_notest" "$cov_good")"
code=$?
st=$(rr_story coverage-test-missing)
fb=$(cov_feedback coverage-test-missing)
if (( code == 0 )) && [[ -n $st && $(impl_takes "$st" | jq -c 'map(.refused)') == '["requirement_coverage",null]' ]] &&
   tests_after_refusal "$st" >/dev/null && [[ -n $fb ]] &&
   grep -q -- '- r2: test email_is_sent: no #\[test\] function of that name in the test sources' "$fb/cwd/in/feedback.md"; then
  echo "coverage-test-missing: checks pass (a named test that does not exist: refused, fed back, fixed; closed)"
else
  echo "coverage-test-missing: FAILED (exit $code, implement runs $(impl_takes "$st" 2>/dev/null))"
fi
# coverage-spent: every reply says r2 not_built. Passes when the run stops writer_spent (the
# existing cause) on change.implement after 3 runs, the kind that ran out is requirement_coverage,
# each run sent back by its postcondition, and tests.run never ran in the story.
trial coverage-spent "${cov_env[@]}" SDC_COVERAGE_REPLIES="$(cov_rounds "$cov_notbuilt")"
code=$?
r=$s/runs/coverage-spent
st=$(rr_story coverage-spent)
if (( code != 0 )) && jq -e '.cause == "writer_spent" and (.reason | test("requirement_coverage"))' "$r/run-stopped.json" >/dev/null 2>&1 &&
   jq -se '(map(select(.event == "writer_spent")) | last | .spent[0]) as $w
     | $w.action == "change.implement" and $w.kind == "requirement_coverage" and $w.runs == 3
       and ([$w.sent_back[].kind] == ["requirement_coverage", "requirement_coverage", "requirement_coverage"])
       and ([.[] | select(.action == "tests.run")] | length == 0)' "$st/events.jsonl" >/dev/null; then
  echo "coverage-spent: checks pass (r2 never built: writer_spent on change.implement after 3 runs, kind requirement_coverage, no tests.run)"
else
  echo "coverage-spent: FAILED (exit $code, stop $(jq -c '{cause, reason}' "$r/run-stopped.json" 2>/dev/null | cut -c1-240))"
fi
# coverage-reply-retry: the first reply of the first call breaks the schema (r1 built with no
# place); llm-node.sh asks again with the check's reason in the same run, the second reply is
# valid. Passes when there is one implement run, its call record holds the refused reply and the
# prompt the reason, two call lines are logged for it, and the run closes.
cov_bad=$(jq -cn '{requirements: [{id: "r1", status: "built", at: [], test: "create_invoice"}, {id: "r2", status: "not_built", reason: "x"}]}')
trial coverage-reply-retry "${cov_env[@]}" SDC_COVERAGE_REPLIES="$(jq -cn --argjson b "$cov_bad" --argjson g "$cov_good" '{rounds: [[$b, $g]]}')"
code=$?
st=$(rr_story coverage-reply-retry)
cr=$(ls -d "$s"/runs/coverage-reply-retry/llm/*-change.implement 2>/dev/null | head -n 1)
if (( code == 0 )) && [[ -n $st && $(impl_takes "$st") == '[{"exit":"0","refused":null,"evidence":1}]' && -n $cr && -f $cr/coverage.try1.json ]] &&
   grep -q 'Your coverage document was refused by the program check: invalid coverage reply: entry 1 (r1): built needs `at`' "$cr/prompt.md" &&
   jq -se --arg r "$cr" '[.[] | select(.record == $r)] | length == 2' "$s/runs/coverage-reply-retry/llm-calls.jsonl" >/dev/null; then
  echo "coverage-reply-retry: checks pass (a reply outside the schema asked again with the reason in the same run; two call lines; closed)"
else
  echo "coverage-reply-retry: FAILED (exit $code, implement runs $(impl_takes "$st" 2>/dev/null), record ${cr:-none})"
fi
# --- end requirement coverage
targs=()
# The review on the adapter path a real run takes (runs/trial-3-strong-3 and -4 stopped stuck at
# their first change.review: on metaharness the review could reply only by writing a file, which
# its read-only operations refused). run-trial.sh --autopilot with SDC_LLM_ADAPTER=metaharness,
# every node but change.review on its stub, the decider on the fixture adapter, and both model CLIs
# replaced by fixtures/fake-model: it writes no file and replies only through a response schema.
targs=(--autopilot)
fm=$fx/fake-model
real_env=(SDC_LLM_ADAPTER=metaharness SDC_LLM_NODES=change.review METAHARNESS="$fm/metaharness" CLAUDE="$fm/claude")
# review-real-path: passes when the run closes, run-settings records the review adapter claude-cli
# (the default on a metaharness run), the review call went to claude with --json-schema and only
# read tools, its reply was schema-constrained, and the story's code_review is approved by llm-review.
rm -f "$s/review-real-path.model.jsonl"
trial review-real-path "${real_env[@]}" FAKE_MODEL_LOG="$s/review-real-path.model.jsonl"
code=$?
r=$s/runs/review-real-path
st=$(find "$r/store" -type d -name 'IDEA-1.*.invoices' 2>/dev/null | head -n 1)
if (( code == 0 )) && [[ ! -f $r/run-stopped.json ]] &&
   jq -se 'last.adapters.review | .adapter == "claude-cli" and .json_schema == true and (.source | startswith("default: claude-cli"))' "$r/run-settings.jsonl" >/dev/null &&
   jq -se '[.[] | select(.action == "change.review")] | length >= 1 and all(.adapter == "claude-cli")' "$r/llm-calls.jsonl" >/dev/null &&
   jq -se 'map(select(.cli == "claude")) | length >= 1 and all(.argv | index("--json-schema") != null
     and (.[index("--allowedTools") + 1] | split(",") | sort == ["Glob", "Grep", "Read"]))' "$s/review-real-path.model.jsonl" >/dev/null &&
   jq -e '.schema_constrained == true' "$(ls -d "$r"/llm/*-change.review | tail -n 1)/response.json" >/dev/null &&
   jq -se '[.[] | select(.kind == "code_review")] | length >= 1 and all(.result == "approved" and .producer == "llm-review")' "$st"/evidence/*.json >/dev/null; then
  echo "review-real-path: checks pass (metaharness run: the review on claude-cli with a response schema and read-only tools, approved, the run closed)"
else
  echo "review-real-path: FAILED (exit $code, or the review was not on claude-cli under a schema: $(jq -c '{cause, reason}' "$r/run-stopped.json" 2>/dev/null | cut -c1-200))"
fi
# review-real-path-facts (SDC_REVIEW_MODE=facts): passes when the run closes, run-settings records the review adapter claude-cli
# (the default on a metaharness run), the review call went to claude with --json-schema and no tools
# (`--tools ""`, no allow list), its prompt carrying the review input, its reply was schema-constrained,
# and the story's code_review is approved by llm-review.
rm -f "$s/review-real-path-facts.model.jsonl"
trial review-real-path-facts "${real_env[@]}" SDC_REVIEW_MODE=facts FAKE_MODEL_LOG="$s/review-real-path-facts.model.jsonl"
code=$?
r=$s/runs/review-real-path-facts
st=$(find "$r/store" -type d -name 'IDEA-1.*.invoices' 2>/dev/null | head -n 1)
if (( code == 0 )) && [[ ! -f $r/run-stopped.json ]] &&
   jq -se 'last.adapters.review | .adapter == "claude-cli" and .json_schema == true and (.source | startswith("default: claude-cli"))' "$r/run-settings.jsonl" >/dev/null &&
   jq -se '[.[] | select(.action == "change.review")] | length >= 1 and all(.adapter == "claude-cli")' "$r/llm-calls.jsonl" >/dev/null &&
   jq -se 'map(select(.cli == "claude")) | length >= 1 and all(.argv | index("--json-schema") != null
     and (.[index("--tools") + 1] == "") and (index("--allowedTools") == null)
     and (.[index("-p") + 1] | contains("# Review input: story") and contains("### impl/")))' "$s/review-real-path-facts.model.jsonl" >/dev/null &&
   jq -e '.schema_constrained == true' "$(ls -d "$r"/llm/*-change.review | tail -n 1)/response.json" >/dev/null &&
   jq -se '[.[] | select(.kind == "code_review")] | length >= 1 and all(.result == "approved" and .producer == "llm-review")' "$st"/evidence/*.json >/dev/null; then
  echo "review-real-path-facts: checks pass (metaharness run: the review on claude-cli with a response schema, no tools and the review input in its prompt, approved, the run closed)"
else
  echo "review-real-path-facts: FAILED (exit $code, or the review was not on claude-cli under a schema: $(jq -c '{cause, reason}' "$r/run-stopped.json" 2>/dev/null | cut -c1-200))"
fi
# review-invalid-stop: the fake model's reply cites a line the README does not have, on every
# attempt. Passes when the run stops by rule:autopilot with cause review_invalid, the reason names
# the story case, the call and the check's reason, and nothing about the review is recorded.
bad='{"decision": "rejected", "summary": "fake", "findings": [{"kind": "violation", "at": "impl/README.md:9999", "violates": "r1", "says": "a line that does not exist"}]}'
trial review-invalid-stop "${real_env[@]}" FAKE_MODEL_REPLY="$bad"
code=$?
r=$s/runs/review-invalid-stop
st=$(find "$r/store" -type d -name 'IDEA-1.*.invoices' 2>/dev/null | head -n 1)
if (( code != 0 )) && jq -e '.by == "rule:autopilot" and .cause == "review_invalid"
     and (.reason | test("change.review in IDEA-1[.][a-z]+[.]invoices: review_invalid") and test("call llm/[0-9]+-change.review")
       and test("lines 9999-9999 outside impl/README.md") and (test("/home/|/tmp/") | not))' "$r/run-stopped.json" >/dev/null &&
   [[ $(jq -s '[.[] | select(.kind == "code_review")] | length' "$st"/evidence/*.json) == 0 ]]; then
  echo "review-invalid-stop: checks pass (the review stayed invalid: the run stopped review_invalid, naming case and call)"
else
  echo "review-invalid-stop: FAILED (exit $code, stop record: $(jq -c '{cause, reason}' "$r/run-stopped.json" 2>/dev/null | cut -c1-200))"
fi
# review-adapter-refused: the review class bound to metaharness, which takes no response schema.
# Passes when run-trial.sh exits 64 before any call, naming the class and the adapter.
rm -f "$s/review-adapter-refused.model.jsonl"
trial review-adapter-refused "${real_env[@]}" SDC_REVIEW_ADAPTER=metaharness FAKE_MODEL_LOG="$s/review-adapter-refused.model.jsonl"
code=$?
if (( code == 64 )) && grep -q 'refused: node class review (spec.review, change.review) replies under a response schema, but its adapter metaharness (SDC_REVIEW_ADAPTER) takes none' "$s/review-adapter-refused.txt" &&
   [[ ! -s $s/review-adapter-refused.model.jsonl ]]; then
  echo "review-adapter-refused: checks pass (a schema class on an adapter without a schema: the run refused to start, exit 64)"
else
  echo "review-adapter-refused: FAILED (exit $code: $(tail -n 1 "$s/review-adapter-refused.txt" | cut -c1-200))"
fi
# efforts: the reasoning effort per LLM node is a run setting like the model (SDC_LLM_EFFORT_<CLASS>,
# SDC_LLM_EFFORT; unset = the class default of tools/node-classes.json, high, the decider xhigh,
# recorded with source `default`: a level is always passed, so a run is the same on any host). Four
# dry runs: no setting; SDC_LLM_EFFORT for every node; one effort per class; an unknown value,
# refused at the start with exit 64 and the accepted values. Passes when every line of
# llm-calls.jsonl and decisions.jsonl names the expected effort, the dry argv carries it, and
# run-settings.jsonl records the effective effort per class. A fifth check runs the claude-cli
# adapter on a stand-in `claude` that records its argv and environment: hermetic (no settings
# sources, MCP servers or slash commands, only the request's tools, the effort passed, a fresh
# config home holding the login copy, CLAUDE_CODE_EFFORT_LEVEL unset, the home removed after).
eexpect() { # <run> <jq program over {node, class} giving the expected effort> <decider effort>
  local r=$s/runs/$1
  jq -se --slurpfile c "$sdc/bindings/tools/node-classes.json" '
    def class($n): [$c[0].classes | to_entries[] | select(.value | index($n)) | .key] | first // "other";
    length > 0 and all(.[]; .effort == ({node: .action, class: class(.action)} | '"$2"'))' "$r/llm-calls.jsonl" >/dev/null &&
  jq -se --arg d "$3" 'length > 0 and all(.[]; .effort == $d)' "$r/decisions.jsonl" >/dev/null
}
eargv() { # <run> <node> <expected effort>: the dry argv of the node's first call
  local a
  a=$(ls -d "$s/runs/$1"/llm/*-"$2" | head -n 1)/argv.json
  jq -e --arg e "$3" '.[index("--effort") + 1] == $e' "$a" >/dev/null
}
dry efforts-default
e1=$?
dry efforts-all SDC_LLM_EFFORT=low
e2=$?
dry efforts-class SDC_LLM_EFFORT=low SDC_LLM_EFFORT_WRITER=high SDC_LLM_EFFORT_IMPLEMENT=xhigh \
  SDC_LLM_EFFORT_REVIEW=medium SDC_LLM_EFFORT_DECIDER=max SDC_LLM_EFFORT_OTHER=medium
e3=$?
rm -rf "$s/runs/efforts-bad"
env SDC_RUNS_DIR="$s/runs" SDC_WATCH=0 SDC_DECIDER=haiku SDC_LLM_EFFORT_WRITER=bogus \
  "$sdc/scripts/run-trial.sh" --dry efforts-bad > "$s/efforts-bad.txt" 2>&1
e4=$?
# the claude-cli adapter on a stand-in claude
h=$s/hermetic; rm -rf "$h"; mkdir -p "$h/cwd"
printf '%s\n' '#!/usr/bin/env bash' 'printf "%s\0" "$@" > "$FAKE_OUT/argv"' 'env > "$FAKE_OUT/env"' \
  '[[ -f $CLAUDE_CONFIG_DIR/.credentials.json ]] && echo yes > "$FAKE_OUT/login"' \
  'echo "{\"type\":\"result\",\"subtype\":\"success\",\"is_error\":false,\"total_cost_usd\":0,\"num_turns\":1,\"usage\":{}}"' > "$h/claude"
chmod +x "$h/claude"; echo '{}' > "$h/login.json"; echo go > "$h/prompt.md"
jq -n --arg p "$h/prompt.md" --arg cwd "$h/cwd" --arg rec "$h/rec" '{model: "m", effort: "high", prompt_file: $p, cwd: $cwd,
  operations: ["dir.list", "file.read"], max_turns: 1, max_budget_usd: 1, record_dir: $rec, node: "n", attempt: 1}' > "$h/req.json"
env -u ANTHROPIC_API_KEY CLAUDE="$h/claude" FAKE_OUT="$h" SDC_CLAUDE_CREDENTIALS="$h/login.json" CLAUDE_CODE_EFFORT_LEVEL=low \
  "$sdc/bindings/tools/llm-adapter-claude-cli.sh" "$h/req.json" > "$h/resp.json" 2>/dev/null
e5=$?
hermetic() {
  jq -Rse 'split("\u0000") as $a | ($a | index("--setting-sources")) as $i | $a[$i + 1] == "" and ($a | index("--strict-mcp-config")) != null
    and ($a | index("--disable-slash-commands")) != null and $a[($a | index("--tools")) + 1] == "Glob,Read"
    and $a[($a | index("--effort")) + 1] == "high"' "$h/argv" >/dev/null &&
  [[ $(sed -n 's/^CLAUDE_CONFIG_DIR=//p' "$h/env") == "$h/rec/config-home."* ]] && ! grep -q '^CLAUDE_CODE_EFFORT_LEVEL=' "$h/env" &&
  [[ $(cat "$h/login") == yes ]] && ! ls -d "$h"/rec/config-home.* >/dev/null 2>&1 &&
  jq -e '.adapter == "claude-cli" and .effort == "high"' "$h/resp.json" >/dev/null
}
echo "efforts: dry exits $e1 $e2 $e3, unknown value exit $e4, stand-in claude exit $e5"
if (( e1 == 0 && e2 == 0 && e3 == 0 && e4 == 64 && e5 == 0 )) && hermetic &&
   eexpect efforts-default '"high"' xhigh &&
   eexpect efforts-all '"low"' low &&
   eexpect efforts-class '{writer: "high", implement: "xhigh", review: "medium", other: "medium"}[.class]' max &&
   eargv efforts-default spec.author high && eargv efforts-all spec.author low &&
   eargv efforts-class spec.author high && eargv efforts-class idea.dissect medium &&
   grep -q "low, medium, high, xhigh, max" "$s/efforts-bad.txt" && [[ ! -e $s/runs/efforts-bad/run-settings.jsonl ]] &&
   jq -se 'last.efforts | [.writer, .implement, .review, .other] | all(.effort == "high" and .source == "default")' "$s/runs/efforts-default/run-settings.jsonl" >/dev/null &&
   jq -se 'last.efforts | [.decider, .requester, .unstick] | all(.effort == "xhigh" and .source == "default")' "$s/runs/efforts-default/run-settings.jsonl" >/dev/null &&
   jq -se 'last.efforts | .writer.effort == "high" and .implement.effort == "xhigh" and .review.effort == "medium"
     and .decider.effort == "max" and .other.effort == "medium" and .writer.source == "SDC_LLM_EFFORT_WRITER"
     and .requester.effort == "max" and .requester.source == "SDC_LLM_EFFORT_DECIDER"' "$s/runs/efforts-class/run-settings.jsonl" >/dev/null &&
   jq -se 'last.efforts | all(.[]; .effort == "low" and .source == "SDC_LLM_EFFORT")' "$s/runs/efforts-all/run-settings.jsonl" >/dev/null; then
  echo "efforts: checks pass (a level always passed: class defaults, one effort for all, one per class; each call names it; the dry argv carries it; run settings per class and requester; an unknown value exits 64; the claude-cli adapter hermetic)"
else
  echo "efforts: FAILED"
fi
# reopen-review-invalid: runs/trial-3-strong-3, -4 and mixed-7, reopened on the fixed tree, stopped
# again in 1 s naming the old call: the review had run at that state, so it was never dispatched
# again. Leg 1: the review on the fixture adapter (SDC_REVIEW_ADAPTER) with an invalid reply:
# review_invalid. Leg 2: --reopen with the adapter swapped back to the default (claude-cli, the
# fake model, a valid reply). Passes when leg 2 makes a new review call on claude-cli, records an
# approved code_review and closes (exit 0), with the reopen marked in the story case.
trial reopen-review-invalid "${real_env[@]}" SDC_REVIEW_ADAPTER="$fixture" \
  SDC_REVIEW_REPLIES="$(jq -cn --argjson j "$bad" '{rounds: [$j]}')"
code1=$?
r=$s/runs/reopen-review-invalid
calls1=$(ls -d "$r"/llm/*-change.review 2>/dev/null | wc -l)
env SDC_RUNS_DIR="$s/runs" SDC_WATCH=0 SDC_DECIDER=haiku SDC_DECIDER_ADAPTER="$fixture" "${real_env[@]}" \
  "$sdc/scripts/run-trial.sh" reopen-review-invalid --reopen --autopilot > "$s/reopen-review-invalid.2.txt" 2>&1
code=$?
echo "reopen-review-invalid: reopen exit $code ($(tail -n 1 "$s/reopen-review-invalid.2.txt" | cut -c1-160))"
st=$(find "$r/store" -type d -name 'IDEA-1.*.invoices' 2>/dev/null | head -n 1)
last_call=$(ls -d "$r"/llm/*-change.review 2>/dev/null | tail -n 1)
if (( code1 != 0 && code == 0 )) && jq -e '.cause == "review_invalid"' "$r"/run-stopped.*.json >/dev/null 2>&1 &&
   (( $(ls -d "$r"/llm/*-change.review | wc -l) == calls1 + 1 )) &&
   jq -e '.adapter == "claude-cli" and .schema_constrained == true' "$last_call/response.json" >/dev/null &&
   jq -se 'any(.[]; .event == "reopened")' "$st/events.jsonl" >/dev/null &&
   jq -se '[.[] | select(.kind == "code_review")] | length == 1 and .[0].result == "approved"' "$st"/evidence/*.json >/dev/null &&
   [[ ! -f $r/run-stopped.json ]]; then
  echo "reopen-review-invalid: checks pass (stopped review_invalid; reopened on another adapter, the review ran again, approved, closed)"
else
  echo "reopen-review-invalid: FAILED (leg 1 exit $code1, reopen exit $code, review calls $calls1 then $(ls -d "$r"/llm/*-change.review 2>/dev/null | wc -l))"
fi
# --- sdc.bugfix (docs/bugfix.md): run-trial.sh --bugfix on the pager seed in autopilot, the stubs
# write (fixtures/bugfix-pager/cycles/<name> through SDC_STUB_FIXTURES). Each closes by
# rule:autopilot with its outcome and replay matches. The record check runs over the case's
# evidence in order: `red` is the last repro_run red, `green` the last test_run passed; the sixth argument, if any, is a jq over decision.json.
bfx=$fx/bugfix-pager
bugfix_cycle() { # <name> <stub cycle or -> <outcome> <jq over the slurped evidence> <what passed> [jq over decision.json]
  local name=$1 cyc=$2 want=$3 check=$4 what=$5 r=$s/runs/$1 code
  rm -rf "$r"
  env SDC_RUNS_DIR="$s/runs" SDC_WATCH=0 SDC_LLM_ADAPTER=stub SDC_DECIDER=approve \
    SDC_STUB_FIXTURES="$([[ $cyc == - ]] || echo "$bfx/cycles/$cyc")" \
    "$sdc/scripts/run-trial.sh" "$name" --bugfix "$bfx" --autopilot > "$s/$name.txt" 2>&1
  code=$?
  echo "$name: exit $code"
  if (( code == 0 )) && [[ ! -f $r/run-stopped.json ]] &&
     jq -e --arg o "$want" '.outcome == $o and .closed_by == "rule:autopilot"' "$r/status.json" >/dev/null &&
     jq -e 'all(.state_matches_log and .decision_matches_log)' "$r/replay.json" >/dev/null && jq -e "${6:-true}" "$r/decision.json" >/dev/null &&
     cat "$r"/store/BUG-1/evidence/*.json | jq -se "$check" >/dev/null; then
    echo "$name: checks pass ($what)"
  else
    echo "$name: FAILED (exit $code, $(jq -c '{outcome, closed_by}' "$r/status.json" 2>/dev/null), stop $(jq -c '{cause, reason}' "$r/run-stopped.json" 2>/dev/null | cut -c1-200))"
  fi
}
red_then_green='(to_entries | map(.value + {at: .key})) as $e | ([$e[] | select(.kind == "repro_run" and .result == "red")] | last) as $red
  | ([$e[] | select(.kind == "test_run" and .result == "passed")] | last) as $green | ([$e[] | select(.kind == "implementation_change")] | last) as $fix
  | $red != null and $green != null and $fix != null and $red.at < $fix.at and $fix.at < $green.at and $green.upstream_revisions.repro == $red.subject_revision
    and $fix.upstream_revisions.repro == $red.subject_revision and $fix.subject_revision == $green.subject_revision and ([$red.subject_revision, $red.upstream_revisions.change, $green.subject_revision] | all(. != null and . != "none")) and $green.subject_revision != $red.upstream_revisions.change'
bugfix_cycle bugfix-full - fixed "$red_then_green" \
  "fixed; red at one change revision, the same repro revision green at the next"
bugfix_cycle bugfix-not-red not-red fixed \
  "$red_then_green and ([.[] | select(.kind == \"repro_run\" and .result == \"not_red\" and .subject_revision != \"none\")] | length) == 1" \
  "a test passing on the seed recorded not_red, the next red, then fixed"
bugfix_cycle bugfix-edited-test edited-test fixed \
  "$red_then_green and ([.[] | select(.kind == \"test_run\") | .result] == [\"failed\", \"passed\"])" \
  "the fix that changed the test refused by tests.run, the next fixed"
grep -q 'the reproducing test was changed' "$s/runs/bugfix-edited-test/store/BUG-1/blobs/"* 2>/dev/null ||
  echo "bugfix-edited-test: FAILED (no log names the changed test)"
bugfix_cycle bugfix-never-red never-red not_reproduced \
  '([.[] | select(.kind == "repro_run" and .result == "red")] | length) == 0 and ([.[] | select(.kind == "implementation_change")] | length) == 0' \
  "every test passes on the seed: not_reproduced, no fix written; the protocol's evaluation refuses the fix" '.claims."repro.red".value == "false" and .actions."change.fix".status == "blocked" and .actions."tests.run".status == "blocked" and .outcomes.fixed.status == "blocked" and .outcomes.not_reproduced.status == "legitimate"'
# bugfix-from-ticket: run-trial.sh --bugfix-ticket --dry --autopilot (the dry adapter writes each
# prompt and calls nothing, the stubs write) on a git repository made from the pager crate, with an
# ignored and an untracked file, from the invented ticket fixtures/bugfix-pager/ticket.yaml. Closes
# fixed by rule:autopilot; mainline/ holds exactly what git ls-files lists, .sdc/seed.json the base
# commit, bug.yaml the ticket's story; change.fix's prompt carries the ticket's constraints and no
# "Standard library only"; the run's output quotes no ticket text. Then the same repository, dirty,
# is refused before any step.
tin=$s/bugfix-from-ticket-input r=$s/runs/bugfix-from-ticket
rm -rf "$tin" "$r" "$s/runs/bugfix-from-ticket-dirty"
tgit() { git -C "$tin/repo" -c user.name=fixture -c user.email=fixture@example.invalid -c commit.gpgsign=false -c core.hooksPath=/dev/null "$@"; }
mkdir -p "$tin" && cp -r "$bfx/crate" "$tin/repo" && printf 'ignored.txt\n' > "$tin/repo/.gitignore" &&
  tgit init -q && tgit add . && tgit commit -q -m fixture
echo ignored > "$tin/repo/ignored.txt"; echo untracked > "$tin/repo/untracked.txt"
head_commit=$(tgit rev-parse HEAD)
C=$head_commit yq '.repository.base.commit = strenv(C) | .repository.base.commit style="double"' "$bfx/ticket.yaml" > "$tin/ticket.yaml"
env SDC_RUNS_DIR="$s/runs" SDC_WATCH=0 SDC_DECIDER=approve \
  "$sdc/scripts/run-trial.sh" bugfix-from-ticket --bugfix-ticket "$tin/ticket.yaml" --dry --autopilot > "$s/bugfix-from-ticket.txt" 2>&1
code=$?
echo "bugfix-from-ticket: exit $code"
w=$r/store/BUG-1/work
fix_prompt=$(ls -d "$r"/llm/*-change.fix 2>/dev/null | tail -n 1)/prompt.md
echo 'pub fn changed() {}' >> "$tin/repo/src/lib.rs"
env SDC_RUNS_DIR="$s/runs" SDC_WATCH=0 SDC_DECIDER=approve \
  "$sdc/scripts/run-trial.sh" bugfix-from-ticket-dirty --bugfix-ticket "$tin/ticket.yaml" --dry --autopilot > "$s/bugfix-from-ticket-dirty.txt" 2>&1
dirty_code=$?
if (( code == 0 )) && [[ ! -f $r/run-stopped.json ]] &&
   jq -e '.outcome == "fixed" and .closed_by == "rule:autopilot"' "$r/status.json" >/dev/null &&
   jq -e 'all(.state_matches_log and .decision_matches_log)' "$r/replay.json" >/dev/null &&
   cat "$r"/store/BUG-1/evidence/*.json | jq -se "$red_then_green" >/dev/null &&
   [[ $(cd "$w/mainline" && find . -type f | sed 's|^\./||' | LC_ALL=C sort) == "$(tgit ls-files | LC_ALL=C sort)" ]] &&
   [[ ! -e $w/mainline/ignored.txt && ! -e $w/mainline/untracked.txt ]] &&
   jq -e --arg c "$head_commit" '.repository.base.commit == $c and .ticket.id == "PAGER-7"' "$w/.sdc/seed.json" >/dev/null &&
   yq -e '.story == "story:ticket-pager-7" and .ticket.id == "PAGER-7" and (.constraints | length) == 2' "$w/bug.yaml" >/dev/null &&
   grep -q '^status: implemented' "$w/aep/story/ticket-pager-7.md" &&
   grep -qF -- '- Keep the public API of the crate unchanged.' "$fix_prompt" && ! grep -q 'Standard library only' "$fix_prompt" &&
   ! grep -qF -e 'never shown' -e 'multiple of the page size' "$s/bugfix-from-ticket.txt" &&
   (( dirty_code == 1 )) && grep -q 'is dirty' "$s/bugfix-from-ticket-dirty.txt"; then
  echo "bugfix-from-ticket: checks pass (fixed from the ticket on a git repository: only tracked files seeded, base recorded, constraints in the prompt, no ticket text in the output; a dirty worktree refused)"
else
  echo "bugfix-from-ticket: FAILED (exit $code, dirty exit $dirty_code, $(jq -c '{outcome, closed_by}' "$r/status.json" 2>/dev/null), stop $(jq -c '{cause, reason}' "$r/run-stopped.json" 2>/dev/null | cut -c1-200))"
fi
# --- recorded replay (docs/driver.md, "Recorded replay"): scripts/run-trial.sh --recorded re-drives
# the committed recording fixtures/recorded/stub with driver/src/bin/sdc-replay as the adapter of
# every model and decider call, no LLM. The recording is a stub run through the fixture adapters,
# packed: SDC_PRINCIPAL=agent:recorded-fixture SDC_LLM_ADAPTER=SDC_DECIDER_ADAPTER=<fixture adapter>
# SDC_DECIDER=haiku SDC_LLM_NODES="idea.dissect questions.write questions.answer
# requirements.normalise context.default spec.author plan.decompose change.implement change.review"
# SDC_FIXTURE_STUB='{"plan.decompose": ["plan-stories.json", "stories.json"]}' run-trial.sh <run>
# "<fixtures/idea.yaml text>" --autopilot, then sdc-replay pack <run> fixtures/recorded/stub.
# Every run switch stays at its default (off): the recording is main's run, with the decider asked
# for context.approve, roadmap.approve and release.publish, and the redrives below run the same.
recording=$fx/recorded/stub
story_case=IDEA-1.billing.invoices
redrive() { # <name> [run-trial.sh replay arguments ...]
  local name=$1; shift
  rm -rf "$s/runs/$name"
  env SDC_RUNS_DIR="$s/runs" SDC_WATCH=0 "$sdc/scripts/run-trial.sh" "$name" --autopilot --recorded "$recording" "$@" > "$s/$name.txt" 2>&1
  local code=$?
  echo "$name: exit $code ($(grep -v '^ ' "$s/$name.txt" | tail -n 1 | cut -c1-160))"
  return $code
}
calls() { jq -c '.replay.calls' "$s/runs/$1/$2"; }
impl_exits() { jq -sc '[.[] | select(.event == "action_finished" and .action == "change.implement") | .exit]' "$s/runs/$1/store/IDEA-1/children/IDEA-1.billing/children/$story_case/events.jsonl"; }
replay_bin=$sdc/driver/target/debug/sdc-replay
n_calls=$(jq '.calls | length' "$recording/recording.json")
# replay-unchanged: the recording re-driven as it is: closed operating as the source, the same
# actions per case (sdc-replay compare), every call recorded, none reused, live or diverged, no
# paid call; each call line marked recorded on the virtual clock; the first line names both trees.
redrive replay-unchanged
code=$?
r=$s/runs/replay-unchanged
if (( code == 0 )) && "$replay_bin" compare "$r" > "$s/replay-unchanged.compare.json" &&
   jq -e --argjson n "$n_calls" '.replay.label == "re-driven: recorded replies only" and .replay.calls.recorded == $n
     and .replay.calls.reused == 0 and .replay.calls.live == 0 and .replay.calls.diverged == 0 and .replay.calls.paid == 0
     and .state == "closed" and .outcome == "operating"' "$r/status.json" >/dev/null &&
   jq -se 'all(.[]; .replay.mode == "recorded" and .replay.clock == "virtual")' "$r/llm-calls.jsonl" >/dev/null &&
   jq -se 'all(.[]; .clock == "virtual" and .seconds == .real_seconds + .virtual_seconds)' "$r/rounds.jsonl" >/dev/null &&
   head -n 1 "$r/console.txt" | grep -q '^re-drive of stub (recorded on tree [0-9a-f]\{12\}) on tree '; then
  echo "replay-unchanged: checks pass (closed operating, same actions in every case, $n_calls calls recorded, 0 reused, live, diverged, paid)"
else
  echo "replay-unchanged: FAILED (run exit $code, or the actions, the end or the call counts differ: $(calls replay-unchanged status.json 2>/dev/null))"
fi
# --- run switches at their defaults (docs/improving.md, "Run switches"): a change in what a run
# does merges behind a switch that is off until its experiment holds. The committed recording is a
# stub run of main's tree before these changes; re-driven with every switch at its default, every
# call has the source's key (node, case, arrival, try), none diverged or live, no prompt digest
# changed and every case took the same actions (sdc-replay compare). Each call's inputs and request
# fields are the recording's, except in/tests.txt: the run's own test log, which carries the ess
# version on PATH and test timings, so under --on-divergence nearest a call that differs only there
# is reused from the same key. Each <switch>-default cycle reads this one run and adds what its own
# change would leave behind.
redrive switches-default --on-divergence nearest
sw_code=$?
sw=$s/runs/switches-default
sw_main() { # the default run is main's run: same calls, inputs, prompts and actions as the recording
  (( sw_code == 0 )) && "$replay_bin" compare "$sw" > "$s/switches-default.compare.json" &&
    jq -e --argjson n "$n_calls" '.replay.calls.recorded + .replay.calls.reused == $n and .replay.calls.diverged == 0
      and .replay.calls.live == 0 and .replay.calls.prompt_changed == 0 and .state == "closed" and .outcome == "operating"' "$sw/status.json" >/dev/null &&
    jq -se 'all(.[]; .mode == "recorded" or (.mode == "reused" and .source != null and (.difference - ["in/tests.txt changed"]) == []))' "$sw/replay/calls.jsonl" >/dev/null
}
sw_calls() { jq -c '.replay.calls' "$sw/status.json" 2>/dev/null; }
# rule-approvals-default: SDC_RULE_APPROVALS unset is off: no rule request, no rule line; the
# decider is asked for context.approve, roadmap.approve and release.publish as on main.
if sw_main && [[ ! -d $sw/rule-approvals ]] && jq -se 'all(.rule_approvals == "off")' "$sw/run-settings.jsonl" >/dev/null &&
   jq -se '[.[] | select(.principal == "rule:autopilot" and (.action | IN("context.approve", "roadmap.approve", "release.publish")))]
     | length == 0' "$sw/decisions.jsonl" >/dev/null &&
   jq -se '[.[] | select(.action | IN("context.approve", "roadmap.approve", "release.publish")) | .principal]
     | length >= 3 and all(startswith("decider:"))' "$sw/decisions.jsonl" >/dev/null; then
  echo "rule-approvals-default: checks pass (SDC_RULE_APPROVALS unset: off; $n_calls calls as main's recording, no prompt changed, the decider asked for context, roadmap and release)"
else
  echo "rule-approvals-default: FAILED (run exit $sw_code; calls $(sw_calls); rule requests: $(ls "$sw/rule-approvals" 2>/dev/null | wc -l))"
fi
# review-mode-default: SDC_REVIEW_MODE unset is session: every review is a session with Glob, Read
# and Grep over the whole change tree and the logs, as on main; no review document, no input_bytes.
sw_reviews=$(ls -d "$sw"/llm/*-change.review 2>/dev/null)
session_inputs() { # every review call's in/ is main's: the logs and commands, no document, no listing
  local d
  [[ -n $sw_reviews ]] || return 1
  for d in $sw_reviews; do
    [[ -f $d/cwd/in/tests.txt && -f $d/cwd/in/gate.txt && -f $d/cwd/in/commands.json && ! -e $d/cwd/in/review.md && ! -e $d/cwd/in/listing.json ]] || return 1
  done
}
if sw_main && jq -se 'last.review_mode == "session"' "$sw/run-settings.jsonl" >/dev/null &&
   jq -se '[.[] | select(.action == "change.review")] | length >= 1 and all(.allowed_operations == ["dir.list", "file.read", "search"]
     and .max_turns == 30 and (has("input_bytes") | not))' "$sw/llm-calls.jsonl" >/dev/null && session_inputs; then
  echo "review-mode-default: checks pass (SDC_REVIEW_MODE unset: session; $(wc -w <<< "$sw_reviews") reviews read the tree with Glob, Read and Grep, as main's recording)"
else
  echo "review-mode-default: FAILED (run exit $sw_code; calls $(sw_calls); review calls: $(jq -sc '[.[] | select(.action == "change.review") | {allowed_operations, max_turns, input_bytes}]' "$sw/llm-calls.jsonl" 2>/dev/null | cut -c1-200))"
fi
# implement-coverage-default: SDC_IMPLEMENT_COVERAGE unset is off: change.implement is asked for no
# coverage document, no postcondition runs and nothing is refused by one, the tree carries no
# .sdc/coverage.json and the review gets no in/coverage.json, as on main.
sw_impl=$(ls -d "$sw"/llm/*-change.implement 2>/dev/null)
no_coverage() { # no call dir of change.implement or change.review holds a coverage input
  local d
  [[ -n $sw_impl ]] || return 1
  for d in $sw_impl; do [[ ! -e $d/cwd/in/story-requirements.json && ! -e $d/coverage-schema.json ]] || return 1; done
  for d in $sw_reviews; do [[ ! -e $d/cwd/in/coverage.json ]] || return 1; done
}
if sw_main && jq -se 'last.implement_coverage == "off"' "$sw/run-settings.jsonl" >/dev/null && no_coverage &&
   [[ -z $(find "$sw/store" -path '*/impl/.sdc/coverage.json' 2>/dev/null) ]] &&
   ! cat "$sw"/store/IDEA-1/children/*/children/*/events.jsonl | jq -se 'any(.[]; .refused != null)' >/dev/null &&
   ! grep -rqs 'sdc-drive: postcondition' "$sw"/store/IDEA-1/children/*/children/*/blobs/; then
  echo "implement-coverage-default: checks pass (SDC_IMPLEMENT_COVERAGE unset: off; no coverage document, no postcondition, no refusal, as main's recording)"
else
  echo "implement-coverage-default: FAILED (run exit $sw_code; calls $(sw_calls); coverage files: $(find "$sw/store" -path '*/impl/.sdc/coverage.json' 2>/dev/null | wc -l))"
fi
# replay-reject-stop: merge.approve's recorded approve replaced by a reject: the rejection is
# recorded and change.implement is sent back with it, a call the source never made
# (change.implement@1.2); under the default policy the run ends there, exit 9, cause
# recording_diverged naming the place.
redrive replay-reject-stop --inject 'reject@merge.approve#1=the change misses a refusal'
code=$?
r=$s/runs/replay-reject-stop
if (( code == 9 )) &&
   jq -e --arg c "$story_case" '.cause == "recording_diverged" and (.reason | test($c + "/change.implement@1.2"))
     and .replay.calls.diverged == 1' "$r/run-stopped.json" >/dev/null &&
   jq -se 'any(.[]; .action == "merge.approve" and .decision == "reject" and .reasons == ["the change misses a refusal"])' "$r/decisions.jsonl" >/dev/null; then
  echo "replay-reject-stop: checks pass (reject recorded, rework asked a call the source never made, stopped recording_diverged)"
else
  echo "replay-reject-stop: FAILED (run exit $code, or no reject, or not stopped recording_diverged at change.implement@1.2)"
fi
# replay-reject-nearest: as above under --on-divergence nearest: the rework reuses the
# implementer's recorded reply, which writes the same revision, so it is sent back until its
# max_runs: writer_spent, the reused calls counted and labelled. run.unstick stays off: the
# recording has no run.unstick call, so run-settings records replay.unstick off and nothing diverges.
redrive replay-reject-nearest --inject 'reject@merge.approve#1=the change misses a refusal' --on-divergence nearest
code=$?
r=$s/runs/replay-reject-nearest
if (( code == 4 )) &&
   jq -e '.cause == "writer_spent" and .replay.calls.reused >= 1 and .replay.calls.diverged == 0
     and .replay.label == "re-driven: with reused replies"' "$r/run-stopped.json" >/dev/null &&
   jq -se 'last | .unstick.on == false and .replay.unstick.on == false' "$r/run-settings.jsonl" >/dev/null &&
   jq -se '[.[] | select(.action == "change.implement") | .replay.mode] | .[0] == "recorded" and (.[1:] | length > 0 and all(. == "reused"))' "$r/llm-calls.jsonl" >/dev/null; then
  echo "replay-reject-nearest: checks pass (the rework reused the recorded reply, writer_spent, reused calls labelled)"
else
  echo "replay-reject-nearest: FAILED (run exit $code, or not writer_spent, or the reused calls not labelled)"
fi
# replay-timeout: the implementer's first call runs into its timeout on the virtual clock: the
# driver records exit timeout ("virtual clock"), the writer's retry reuses the recorded reply
# (nearest) and the run closes operating as the source did.
redrive replay-timeout --inject "timeout@$story_case/change.implement#1" --on-divergence nearest
code=$?
r=$s/runs/replay-timeout
if (( code == 0 )) && [[ $(impl_exits replay-timeout) == '["timeout","0"]' ]] &&
   jq -e '.outcome == "operating" and .replay.calls.faults == 1 and .replay.calls.reused == 1' "$r/status.json" >/dev/null &&
   grep -rqs 'killed after 1500 s (virtual clock' "$r/store/IDEA-1/children/IDEA-1.billing/children/$story_case/blobs"; then
  echo "replay-timeout: checks pass (exit timeout on the virtual clock, the retry reused the recorded reply, closed operating)"
else
  echo "replay-timeout: FAILED (run exit $code, change.implement exits $(impl_exits replay-timeout 2>/dev/null))"
fi
# replay-live-dry: the reject again, under --on-divergence live with the dry adapter and a cap of
# 1: the rework's call goes to the live adapter (printed as `would run: metaharness …`, nothing
# called, nothing written), counted as live and unpaid; the next one ends the run, the cap spent.
redrive replay-live-dry --inject reject@merge.approve#1 --on-divergence live --live-adapter dry --live-cap 1
code=$?
r=$s/runs/replay-live-dry
if (( code == 9 )) &&
   jq -e '.cause == "recording_diverged" and (.reason | test("live call cap is spent")) and .replay.calls.live == 1
     and .replay.calls.paid == 0 and .replay.label == "re-driven: with live model calls"' "$r/run-stopped.json" >/dev/null &&
   (( $(grep -c '^would run: metaharness' "$r/dry-commands.txt") == 1 )); then
  echo "replay-live-dry: checks pass (1 live call through dry, unpaid, the next stopped at the cap)"
else
  echo "replay-live-dry: FAILED (run exit $code, or the live calls, the cap or the dry commands are wrong)"
fi
# replay-check-fail: tests.run's first run fails (the program runs, its run recorded as exit 1)
# and change.implement's max_runs is 2: the writer is sent back with the failure, its reused reply
# writes the same revision, and the run ends writer_spent at 2 runs; the condition is a run setting.
redrive replay-check-fail --inject check-fail@tests.run#1 --set max_runs.implement=2 --on-divergence nearest
code=$?
r=$s/runs/replay-check-fail
if (( code == 4 )) &&
   jq -e '.cause == "writer_spent" and .replay.programs.check_fail == 1' "$r/run-stopped.json" >/dev/null &&
   [[ $(impl_exits replay-check-fail) == '["0","0"]' ]] &&
   jq -se 'last.replay.conditions["max_runs.implement"] == 2' "$r/run-settings.jsonl" >/dev/null &&
   grep -q '(max_runs 2)' "$s/replay-check-fail.txt"; then
  echo "replay-check-fail: checks pass (tests.run failed once by injection, writer_spent at max_runs 2, the condition recorded)"
else
  echo "replay-check-fail: FAILED (run exit $code, change.implement exits $(impl_exits replay-check-fail 2>/dev/null))"
fi
# replay-recording-clean: the committed recording holds no absolute path, no prompt and no
# transcript, and names the tree it was recorded on.
if jq -e '.format == "sdc.recording/1" and (.tree.commit | type) == "string" and (.calls | length) > 0' "$recording/recording.json" >/dev/null &&
   ! grep -rqs '/home/' "$recording" && [[ -z $(find "$recording" \( -name prompt.md -o -name transcripts -o -name 'events.jsonl' -path '*/calls/*' \) -print) ]]; then
  echo "replay-recording-clean: checks pass (no absolute path, prompt or transcript; tree $(jq -r .tree.commit "$recording/recording.json"))"
else
  echo "replay-recording-clean: FAILED"
fi
} | tee "$s/cycles.log"
summary "$s/cycles.log"
