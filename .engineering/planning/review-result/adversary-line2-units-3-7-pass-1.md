---
format: aep.planning-md/3
id: review-result:adversary-line2-units-3-7-pass-1
kind: review-result
status: active
title: Adversary pass 1 on units 3 and 7 of line 2, wave 2026-11-22-w1, head b2ce1cb
summary: '3 findings, all introduced: 2 NEEDS-CHANGE, 1 INFEASIBLE. Recorded as returned, with two absolute path prefixes replaced by <scratch> and <trees>.'
tags:
- wave-2026-11-22-w1
relations:
- reviews: story:driver-runtime-removed
revision: 1
---
```
unit: 3 (da8542801b) and 7 (56579cc, 4613ed6, b2ce1cb) at b2ce1cb
verdict: NEEDS-CHANGE
cases: executed 401→405 in the driver binary (4 added), 485→486 in the runbook binary (1 added); red 3
origin: introduced 3 / pre-existing 0 / undecided 0
wrote-outside-worktree: <scratch>/removal/line2/adversary-1/ (cases.patch, mutate.sh, auth-report.json, 15 mutant spec copies m-*); driver/target/tmp/decision-contract-mutants/ (build output of my test, inside the tree, ignored by git)
needs-coordinator: no
```

The "before" counts are not the suite run before my cases. The driver count comes from nextest's own line when I ran `decision_contract_mutants` ("4 run, 401 skipped"). The runbook count is the module run (620 skipped plus 1).

## 1. Diff stat (cases only, no implementation file)

```
 driver/tests/decision_contract_mutants.rs | 90 +++++++++++++++++++++++++++++++
 driver/tests/main.rs                      |  1 +
 runbook/tests/loom_run/decisions.rs       | 14 +++++
 3 files changed, 105 insertions(+)
```

`git diff --stat -- '*/src/*' ui/server.mjs ui/web` prints nothing.

## 2. Cases, run alone before the suite

**`driver/tests/decision_contract_mutants.rs`** (module added to `driver/tests/main.rs`). Each test copies the specification to a private directory and weakens one guard of `ess/domains/decision.yaml` by text replacement. It then runs `ess specify validate`, `ess verify conform author` and `ess verify conform run --target interpreted` on the copy. A mutant is killed when any step fails. Each test takes about 1 s.

- `the_unmutated_specification_passes_its_scenarios_on_the_interpreter`: green. It proves the harness is not the cause of the red cases.
- `an_inverted_decide_guard_is_killed`: green. Control for the harness.
- `a_decide_guard_that_holds_for_a_person_only_is_killed`: RED (0.84 s)
  ```
  OptionNotOffered held for a person only and all 19 scenarios still passed: a decider or a rule that decides an unoffered letter is not tested
  ```
- `a_recommend_guard_that_holds_for_a_strong_recommendation_only_is_killed`: RED (1.26 s)
  ```
  OptionNotOffered held for a strong recommendation only and all 19 scenarios still passed: a weak or moderate recommendation of an unoffered letter is not tested
  ```

**`runbook/tests/loom_run/decisions.rs`**: `a_missing_table_stops_for_a_person_at_a5_too_and_calls_no_model`. It is RED (1.6 to 2.5 s):
```
assertion `left == right` failed: a table that cannot be read must stop every decision for a person, at A5 as at A3: runbook run: ended: operating
  left: 0
 right: 8
```

## 3. Suite run after the cases exist

Command: `cd runbook && SLOT_TIMEOUT=10m cargo nextest run --locked --no-fail-fast loom_run::decisions decider::tests`.

Result: 79 run, 78 passed, 1 failed (my A5 case), 8.1 s, exit non-zero.

The driver module ran as 4 tests: 2 passed, 2 failed, 1.3 s. Both gates behind it pass: `slot task check:ess-generated` and the committed suite digest (`ac70d479…`).

## 4. Findings

**F1. Decision guards of `sdc.decision` are executed by no program or gate in this repository (unit 7), `ess/domains/decision.yaml:611`.**
- `ess/components.yaml` owns only `sdc.daemon`. `check:ess-generated` prints `note: no component owns sdc.decision`.
- `runbook/tests/daemon/conformance.rs` counts the 7 `sdc.decision` scenarios as out of domain. They are never `passed`.
- Nothing runs `ess verify conform run --target interpreted` (grep of `Taskfile.yml`, `.github`, tests: none).
- The only gate that notices a changed guard is the digest and name check. That check fails only when the DIGEST file is stale. It passes again once someone re-records.
- I ran mutants of every guard by hand in my scratch (mutate.sh):

| Mutant | Result |
|---|---|
| delete Request `duplicate-letters` | scenario refuses to validate, killed |
| delete Recommend `option-not-offered` | killed |
| delete Decide `option-not-offered` | killed |
| delete `empty-record-id` | killed |
| delete Escalate `target-not-eligible` | killed |
| Escalate guard for decider holder only | killed by the interpreter |
| Escalate guard for open state only | killed by the interpreter |
| Decide guard for `principal_kind == person` only | SURVIVES (19/19 pass) |
| Recommend guard for `strength == strong` only | SURVIVES (19/19 pass) |
| `already-recorded` reverted to `coalesce(...)` | survives (see F3) |

- What measured it: the two red cases above.
- What reaches it: Loom implements `sdc.decision`, and this repository holds the only specification of its refusals. A decider or a rule that Decides an unoffered letter is exactly the autopilot path.
- Fix to name: add scenarios, one Decide by `decider` or `rule` principal with an unoffered letter and one weak or moderate Recommend of an unoffered letter. Run the interpreter in the gate, or keep my module.
- Verdict NEEDS-CHANGE, severity warning, origin introduced.

**F2. A5 ignores the routing table, so a missing table fails open at A5 (unit 3), `runbook/src/loom_run/decider.rs:267-270` (the `if level == AUTONOMY_DECIDES` branch of `route`, entered from `decide` at `:545`).**
- At A5, non-close decisions never load the table. The model decider approves with no table and the run ends 0.
- The brief's invariant says a missing or unreadable table fails closed, "never to a rule or a model".
- The unit's own test `a_table_that_is_missing_or_unusable_is_a_person_s_at_every_level_for_every_class` skips A5 approvals (`if level == "A5" && d["kind"] == "ApproveAction" { continue; }`). The exception is written into the test.
- What measured it: my runbook case, exit 0 where 8 is expected.
- What reaches it: any A5 run from a checkout or run directory without `tools/autonomy-levels.json`. The copy is made from the checkout, and the copy step tolerates a missing table (`bindings_copy.rs:194`, `.ok()`). That path is real, though rare.
- Either the table is required at every level, or the invariant and `docs/loom-run-decisions.md` say that A5 needs no table.
- Verdict NEEDS-CHANGE, severity warning, origin introduced.

**F3. `already-recorded` fix is unobservable (unit 7), `ess/domains/decision.yaml:746`.**
- `defined(produced_record) &&` has no effect, because `empty-record-id` (line 736) already refuses the only input the old `coalesce` predicate misclassified.
- The mutant that restores `coalesce` passes all 19 scenarios, and no scenario can tell them apart.
- The guard is harmless, but the CHANGELOG describes the already-recorded change as a fix.
- The same ordering has a second effect. `empty-record-id` precedes `no-request`, so an empty id on an unknown request answers `EmptyRecordId`, not `NoSuchRequest`. The scenario `ess/scenarios/decision-empty-record-id-is-refused.yaml` does not pin that order. Loom is promised both error names.
- Verdict INFEASIBLE (nothing to break, only contract wording), severity note, origin introduced.

## 5. Attacked and not broken

- The table `tools/autonomy-levels.json` equals § 2.2 of `docs/autonomy-levels.md` in all 14 classes at all 6 levels, which I compared by hand.
- A level below A5 widens nothing:
  - `decider_takes` answers true only for the holder `decider`.
  - `rule`, `advised`, `rule_and_decider` and `person` all stop for a person.
  - A class the table lacks, including the unknown-action default `approval`, has no holder and stops for a person.
  - A broken table (not JSON, wrong format, unknown holder, missing level, missing class, empty file) stops for a person at A1 to A4 and at the close at A5.
- `escalate: final` applies at A5 only (`final_escalate()`).
- Principals: the close by rule is recorded as `rule:autopilot`, a model's answer as `decider:model`, and a person is never named (`run_loop.rs` fallback to `answer.principal`). The ESS text for `answered_by` says "never the principal".
- Seam of units 3 and 4–6 in `bindings_copy.rs` (`routed` and `replay` both in `Plan`, `principal_of` at `:312`) and in `run_loop.rs`: no conflict found. Nothing in the merge drops either side.
- Unit 7:
  - The error names exist in the generated schema: `OptionNotOffered`, `NotEligible`, `DuplicateOptionLetter`, `EmptyRecordId`.
  - The outcome `target-not-eligible` is present.
  - Deleting any one of the five guards fails validation, because the scenarios name the outcomes.
  - Mutants that weaken `duplicate-letters` (distinct by cost, by does) are killed.
  - The driver check for more than 26 options has a test.
  - `decision_grounds` tests the unoffered letter in the recommendation check (`tools/decision-grounds.sh`).

Not examined: whether a model node can overwrite `<run>/tools/autonomy-levels.json` (the table is read from the run copy, which sits beside the tools directory handed to model nodes). I tested nothing there, so this is not a finding.

## 6. Paths written outside the worktree

- `<scratch>/removal/line2/adversary-1/` (listed in the header)
- `<trees>/line2/driver/target/tmp/decision-contract-mutants/` (build output inside the tree)

The patch is `<scratch>/removal/line2/adversary-1/cases.patch`. `git apply --check` accepts it. After `git apply -R` and `git reset -q`, `git status --porcelain` is empty.

```findings
- file: ess/domains/decision.yaml
  line: 611
  category: mutant
  severity: warning
  verdict: NEEDS-CHANGE
  origin: introduced
  message: no program or gate here runs the sdc.decision scenarios (no component owns the domain, the daemon suite counts them out of domain, nothing runs the interpreter); a Decide guard weakened to person-only and a Recommend guard weakened to strong-only both leave all 19 authored scenarios passing, shown by driver/tests/decision_contract_mutants.rs.
- file: runbook/src/loom_run/decider.rs
  line: 267
  category: acceptance
  severity: warning
  verdict: NEEDS-CHANGE
  origin: introduced
  message: at A5 route() never loads the holder table for a non-close decision, so a missing tools/autonomy-levels.json lets the model decider approve and the run ends 0, against the invariant that a missing table fails closed; the unit's own table test skips A5 approvals, and runbook/tests/loom_run/decisions.rs a_missing_table_stops_for_a_person_at_a5_too_and_calls_no_model is red with exit 0 where 8 is expected.
- file: ess/domains/decision.yaml
  line: 746
  category: contract-drift
  severity: note
  verdict: INFEASIBLE
  origin: introduced
  message: the already-recorded predicate change is unobservable because empty-record-id (line 736) precedes it and refuses the only input the old coalesce predicate got wrong; a mutant restoring coalesce passes all 19 scenarios, and an empty id on an unknown request answers EmptyRecordId before NoSuchRequest with no scenario pinning that order.
```
